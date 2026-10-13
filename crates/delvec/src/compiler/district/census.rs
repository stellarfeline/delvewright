//! **`delvec district list`** (spec-0103 §4.2): one row per district — what it
//! holds and where its edges are — beside the partitioning guide's size advice.
//!
//! The bounds are the guide's (`references/partitioning.md`, *The bounds*), and
//! they **advise**: a district outside them prints `DW1007` beside its row with
//! the bound it crossed, and the verb exits 0. The partition is a creative
//! judgement; this confirms what was declared and never refuses it.
//!
//! The same census answers "how far is this district" for a planner reading a
//! worker's branch (§9.5): places detailed of total, quests expanded of planned.

use std::collections::BTreeSet;

use delvewright_dsl::{Campaign, Diagnostic, Edge};
use serde::Serialize;

use super::{DW_DISTRICT_SIZE, Partition};

/// The guide's upper bound on places in one district (authored, on two
/// campaigns; `references/partitioning.md`).
pub const MAX_PLACES: usize = 12;

/// The guide's upper bound on acts of the quest plan in one district.
pub const MAX_ACTS: usize = 1;

/// One district's row.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Row {
    /// `district/<x>`.
    pub district: String,
    /// The note the graph records for it, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Places a body reaches.
    pub reached: usize,
    /// Scenery places (`reached: false`).
    pub scenery: usize,
    /// Edges with a seam whose both ends are inside.
    pub seams_inside: usize,
    /// Edges with a seam that cross its boundary.
    pub boundary_seams: usize,
    /// `carry` edges that cross its boundary.
    pub boundary_carries: usize,
    /// Stations declared on its places.
    pub stations: usize,
    /// Beats bound to its places.
    pub beats: usize,
    /// Planned quests the plan assigns it.
    pub planned_quests: usize,
    /// Of those, quests stage 5 expands in this tree.
    pub expanded_quests: usize,
    /// Places with a `details[]` row.
    pub detailed: usize,
    /// Critical-path steps standing in it.
    pub path_steps: usize,
    /// The acts its planned quests belong to.
    pub acts: Vec<u32>,
    /// The bounds of the guide it crosses, in words — empty within them.
    pub advice: Vec<String>,
}

/// The census of every declared district, in the graph's order.
#[must_use]
pub fn census(c: &Campaign) -> Vec<Row> {
    let partition = Partition::of(c);
    let Some(graph) = c.layout_graph.as_ref().map(|g| &g.content) else {
        return Vec::new();
    };
    let expanded: BTreeSet<&str> = c
        .quests
        .content
        .quests
        .iter()
        .map(|q| q.id.as_str())
        .collect();
    let detailed: BTreeSet<&str> = c
        .detail_plan
        .iter()
        .flat_map(|p| p.content.details.iter().map(|d| d.place.0.as_str()))
        .collect();
    let mut rows = Vec::new();
    for district in &graph.districts {
        let id = district.id.as_str();
        let inside = |n: &str| partition.of_node(n) == Some(id);
        let mut r = Row {
            district: id.to_string(),
            note: district.note.clone(),
            ..Row::default()
        };
        let mut beat_places: BTreeSet<&str> = BTreeSet::new();
        for b in &graph.beats {
            if inside(b.node.0.as_str()) {
                r.beats += 1;
                beat_places.insert(b.node.0.as_str());
            }
        }
        let mut acted = false;
        for n in &graph.nodes {
            if !inside(n.id.0.as_str()) {
                continue;
            }
            if n.reached {
                r.reached += 1;
                if !n.stations.is_empty() || beat_places.contains(n.id.0.as_str()) {
                    acted = true;
                }
            } else {
                r.scenery += 1;
            }
            r.stations += n.stations.len();
            if detailed.contains(n.id.0.as_str()) {
                r.detailed += 1;
            }
        }
        for e in &graph.edges {
            let (a, b) = (inside(e.a().0.as_str()), inside(e.b().0.as_str()));
            if a && b && e.has_seam() {
                r.seams_inside += 1;
            } else if a != b {
                if e.has_seam() {
                    r.boundary_seams += 1;
                } else if matches!(e, Edge::Carry { .. }) {
                    r.boundary_carries += 1;
                }
            }
        }
        r.path_steps = graph
            .critical_path
            .iter()
            .filter(|n| inside(n.0.as_str()))
            .count();
        let mut acts: BTreeSet<u32> = BTreeSet::new();
        for q in &c.quest_plan.content.quests {
            if q.district.as_ref().map(|d| d.as_str()) == Some(id) {
                r.planned_quests += 1;
                acts.insert(q.act);
                if expanded.contains(q.id.as_str()) {
                    r.expanded_quests += 1;
                }
            }
        }
        r.acts = acts.into_iter().collect();
        if !acted {
            r.advice.push(
                "below the lower bound: no reached place of it holds a quest beat or a declared \
                 station — geometry nobody acts in belongs to the district whose continuity it \
                 shares"
                    .to_string(),
            );
        }
        let places = r.reached + r.scenery;
        if places > MAX_PLACES {
            r.advice.push(format!(
                "above the upper bound: {places} place(s), more than the guide's {MAX_PLACES}"
            ));
        }
        if r.acts.len() > MAX_ACTS {
            r.advice.push(format!(
                "above the upper bound: its quests span {} acts, more than the guide's {MAX_ACTS}",
                r.acts.len()
            ));
        }
        rows.push(r);
    }
    rows
}

/// `DW1007` beside every row outside the guide's bounds — advisories, never
/// refusals.
#[must_use]
pub fn advisories(rows: &[Row]) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for r in rows {
        for a in &r.advice {
            out.push(Diagnostic::warning(
                DW_DISTRICT_SIZE,
                "layout-graph",
                "/content/districts",
                format!(
                    "district `{}` is {a}. This is the partitioning guide's advice \
                     (`references/partitioning.md`, *The bounds*) and it refuses nothing: a \
                     district is sized by meaning, and one outside the bounds passes with its \
                     count stated. Re-read the partition against the guide's five tests.",
                    r.district
                ),
            ));
        }
    }
    out
}

/// The binding line every run prints, the zero said as what it is.
#[must_use]
pub fn line(rows: &[Row]) -> String {
    if rows.is_empty() {
        return "0 district(s): the whole — one author, no partition.".to_string();
    }
    let advised = rows.iter().filter(|r| !r.advice.is_empty()).count();
    format!(
        "{} district(s) over {} place(s); {} outside the guide's bounds (DW1007, advice).",
        rows.len(),
        rows.iter().map(|r| r.reached + r.scenery).sum::<usize>(),
        advised
    )
}

/// The table a person reads.
#[must_use]
pub fn table(rows: &[Row]) -> String {
    let mut out = String::from(
        "| district | places (reached + scenery) | seams inside | boundary seams | boundary carries \
         | stations | beats | quests (expanded of planned) | detailed | path steps | acts |\n\
         |---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    for r in rows {
        out.push_str(&format!(
            "| `{}` | {} ({} + {}) | {} | {} | {} | {} | {} | {} of {} | {} of {} | {} | {} |\n",
            r.district,
            r.reached + r.scenery,
            r.reached,
            r.scenery,
            r.seams_inside,
            r.boundary_seams,
            r.boundary_carries,
            r.stations,
            r.beats,
            r.expanded_quests,
            r.planned_quests,
            r.detailed,
            r.reached + r.scenery,
            r.path_steps,
            if r.acts.is_empty() {
                "none".to_string()
            } else {
                r.acts
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            },
        ));
    }
    out
}
