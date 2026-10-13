//! **Routing a whole-campaign red to its district** (spec-0103 §8.3).
//!
//! `delvec build` writes `validation/districts.json`: every place, station,
//! synthesized anchor, quest, objective, trigger, wave, actor, assembly, killing
//! volume, pulse, NPC and detail row, with its district or `whole`; and every
//! leg of `validation/critical-path-waypoints.json` with the district of the
//! place it starts in and the place it ends in. A campaign that declares no
//! district writes none — the whole is the only owner, and there is nothing to
//! route.
//!
//! The same ledger routes a diagnostic ([`Attribution`]). A diagnostic about an
//! object a fragment wrote is first relocated to the fragment
//! ([`super::fragment::Fragments::relocate`]) and carries `[district/<x>]` at
//! the head of its message; a diagnostic with no object pointer — a build
//! refusal, which names objects in its prose — carries it when the objects it
//! names belong to exactly one district. A finding that names objects of two
//! districts, or of none, is the planner's and carries nothing, and so does
//! every diagnostic about an object of the whole's documents.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::{Campaign, Diagnostic};
use serde_json::{Value, json};

use super::Partition;
use super::fragment::Fragments;

/// The word the ledger writes for an object of the whole.
pub const WHOLE: &str = "whole";

fn owner_of(fragments: &Fragments, stage: &str, list: &str, i: usize) -> String {
    fragments
        .origin(stage, list, i)
        .map_or_else(|| WHOLE.to_string(), |(d, _)| d.to_string())
}

/// **Every object of the campaign, with its owner** — id → `district/<x>` or
/// `whole`, class by class. The one table the build writes and the router
/// reads.
#[must_use]
pub fn owners(
    c: &Campaign,
    fragments: &Fragments,
) -> BTreeMap<&'static str, BTreeMap<String, String>> {
    let p = Partition::of(c);
    let mut t: BTreeMap<&'static str, BTreeMap<String, String>> = BTreeMap::new();
    if let Some(g) = c.layout_graph.as_ref().map(|g| &g.content) {
        for n in &g.nodes {
            let d = p.of_node(n.id.0.as_str()).unwrap_or(WHOLE).to_string();
            t.entry("places")
                .or_default()
                .insert(n.id.0.clone(), d.clone());
            for s in &n.stations {
                t.entry("stations")
                    .or_default()
                    .insert(s.anchor.as_str().to_string(), d.clone());
            }
        }
    }
    for a in delvewright_dsl::synthesized_anchors(c) {
        let d = p.of_anchor(&a).unwrap_or(WHOLE).to_string();
        t.entry("anchors").or_default().insert(a, d);
    }
    let q = &c.quests.content;
    for (i, quest) in q.quests.iter().enumerate() {
        let d = owner_of(fragments, "quests", "quests", i);
        for o in &quest.objectives {
            t.entry("objectives")
                .or_default()
                .insert(o.id().as_str().to_string(), d.clone());
        }
        t.entry("quests")
            .or_default()
            .insert(quest.id.as_str().to_string(), d);
    }
    macro_rules! list {
        ($field:ident, $key:literal) => {
            for (i, x) in q.$field.iter().enumerate() {
                t.entry($key).or_default().insert(
                    x.id.as_str().to_string(),
                    owner_of(fragments, "quests", $key, i),
                );
            }
        };
    }
    list!(triggers, "triggers");
    list!(waves, "waves");
    list!(actors, "actors");
    list!(assemblies, "assemblies");
    list!(lethal_volumes, "lethal_volumes");
    list!(pulses, "pulses");
    for (i, n) in c.npcs.content.npcs.iter().enumerate() {
        t.entry("npcs").or_default().insert(
            n.id.as_str().to_string(),
            owner_of(fragments, "npcs", "npcs", i),
        );
    }
    for (i, row) in c
        .detail_plan
        .iter()
        .flat_map(|p| p.content.details.iter())
        .enumerate()
    {
        t.entry("details").or_default().insert(
            row.place.0.clone(),
            owner_of(fragments, "detail-plan", "details", i),
        );
    }
    t
}

/// The place whose cells hold `cell`, by the frames' owned boxes.
fn place_at(
    frames: &[(String, Vec<delvewright_dsl::siteplan::Aabb>)],
    cell: [i64; 3],
) -> Option<&str> {
    frames
        .iter()
        .find(|(_, owned)| {
            owned
                .iter()
                .any(|(lo, hi)| (0..3).all(|k| cell[k] >= lo[k] && cell[k] <= hi[k]))
        })
        .map(|(n, _)| n.as_str())
}

/// **`validation/districts.json`**, or `None` for a campaign that declares no
/// district. `waypoints` is the build's `critical-path-waypoints.json`.
#[must_use]
pub fn document(c: &Campaign, fragments: &Fragments, waypoints: Option<&Value>) -> Option<Value> {
    let p = Partition::of(c);
    if !p.is_declared() {
        return None;
    }
    let owners = owners(c, fragments);
    let frames: Vec<(String, Vec<delvewright_dsl::siteplan::Aabb>)> = if c.site_plan.is_some() {
        crate::compiler::detail::frames(c)
            .into_iter()
            .map(|(f, _)| (f.node.0.clone(), f.owned.clone()))
            .collect()
    } else {
        Vec::new()
    };
    let mut legs = Vec::new();
    for (i, leg) in waypoints
        .and_then(|w| w.get("legs"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let at = |k: &str| -> Option<[i64; 3]> {
            let a = leg.get(k)?.as_array()?;
            Some([
                a.first()?.as_i64()?,
                a.get(1)?.as_i64()?,
                a.get(2)?.as_i64()?,
            ])
        };
        let from = at("from").and_then(|c| place_at(&frames, c));
        let to = at("to").and_then(|c| place_at(&frames, c));
        let dist = |n: Option<&str>| n.map(|n| p.of_node(n).unwrap_or(WHOLE).to_string());
        legs.push(json!({
            "leg": i,
            "from": from,
            "from_district": dist(from),
            "to": to,
            "to_district": dist(to),
        }));
    }
    let examined: usize = owners.values().map(BTreeMap::len).sum();
    let mut doc = json!({
        "districts": p.ids,
        "examined": examined,
        "legs": legs,
    });
    for (class, rows) in owners {
        doc[class] = json!(rows);
    }
    Some(doc)
}

/// **The router**: how a diagnostic finds its district.
#[derive(Debug, Clone, Default)]
pub struct Attribution {
    fragments: Fragments,
    /// id → owner, over every object class.
    ids: BTreeMap<String, String>,
}

impl Attribution {
    /// The router for a parsed campaign and its fragments. Inert (routes
    /// nothing) for a campaign that declares no district.
    #[must_use]
    pub fn of(c: &Campaign, fragments: &Fragments) -> Attribution {
        if !Partition::of(c).is_declared() {
            return Attribution::default();
        }
        let mut ids = BTreeMap::new();
        for rows in owners(c, fragments).into_values() {
            for (id, d) in rows {
                ids.entry(id).or_insert(d);
            }
        }
        Attribution {
            fragments: fragments.clone(),
            ids,
        }
    }

    /// The district a message's named objects belong to, when they belong to
    /// exactly one.
    #[must_use]
    pub fn of_message(&self, message: &str) -> Option<String> {
        if self.ids.is_empty() {
            return None;
        }
        let mut found: BTreeSet<&str> = BTreeSet::new();
        for token in message
            .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '/' || ch == '-' || ch == '_'))
        {
            if let Some(d) = self.ids.get(token)
                && d != WHOLE
            {
                found.insert(d);
            }
        }
        (found.len() == 1).then(|| found.iter().next().expect("one").to_string())
    }

    /// **Route one diagnostic**: relocated to its fragment and tagged with its
    /// district, or returned unchanged.
    #[must_use]
    pub fn route(&self, d: &Diagnostic) -> Diagnostic {
        let mut out = d.clone();
        if self.ids.is_empty() && self.fragments.is_empty() {
            return out;
        }
        if out.message.starts_with("[district/") {
            return out;
        }
        let district = match self.fragments.relocate(&mut out) {
            Some(x) => Some(x),
            // An object of the whole's documents is the whole's.
            None if out.path.starts_with("/content/") => None,
            None => self.of_message(&out.message),
        };
        if let Some(x) = district {
            out.message = format!("[{x}] {}", out.message);
        }
        out
    }
}
