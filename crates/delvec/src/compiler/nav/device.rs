//! **A device does what it reads as** (spec-0124): a door the story addresses
//! is the shutter of its way (`DW1069`), a key's effect is in sight or shown
//! (`DW1071`), and every door and fence gate in the built world closes an
//! opening (`DW1072`).
//!
//! The document arms of `DW1069` — a `shut: door` on a connection that cannot
//! hold a door (shape *c*) and a `prop` or `release` written on the key
//! anchored at a door-shut seam (shape *d*) — read the documents alone and are
//! refused at validation ([`check_document`]). Its world arms — a door the
//! story addresses that is no seam's shutter (shape *a*) and a shutter onto
//! nothing (shape *b*) — read the assembled world, as `DW1071` and `DW1072`
//! do, and are the build's.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::siteplan::{SitePlan, door_shut_seam_anchors, seam_anchor};
use delvewright_dsl::{Campaign, Diagnostic, DwCode, ExitTier, Objective};

delvewright_dsl::dw_code! {
    /// `DW1069`: a door the story addresses is not the shutter of a way, or a
    /// door-shut seam is declared where no door can stand (spec-0124 §7.1).
    pub const DW_DOOR_NO_WAY: DwCode = DwCode::new("DW1069", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW1071`: a key's effect is neither in sight of where the key is used
    /// nor shown by a shot (spec-0124 §7.3).
    pub const DW_EFFECT_OUT_OF_SIGHT: DwCode = DwCode::new("DW1071", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW1072`: a door or fence gate in the built world closes no opening
    /// (spec-0124 §7.4).
    pub const DW_DOOR_CLOSES_NOTHING: DwCode = DwCode::new("DW1072", ExitTier::Build);
}

/// **`DW1069`, shapes *c* and *d*** (spec-0124 §5.1, §5.2): over the layout
/// graph, the site plan and the quests. A campaign with no door-shut seam
/// judges nothing and returns no diagnostic.
#[must_use]
pub fn check_document(c: &Campaign) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    let Some(graph) = c.layout_graph.as_ref().map(|g| &g.content) else {
        return d;
    };
    let plan = c.site_plan.as_ref().map(|_| SitePlan::of(c));
    for (ei, e) in graph.edges.iter().enumerate() {
        if !e.door_shut() {
            continue;
        }
        let path = format!("/content/edges/{ei}/gating/shut");
        let class = e.class();
        if class != "walk" {
            d.push(shape_c(
                path,
                format!(
                    "`{}` is a `{class}`: a door stands in a wall between two floors a body \
                     walks, and a {class} has no such opening",
                    e.id()
                ),
            ));
            continue;
        }
        let Some(plan) = plan.as_ref() else {
            continue; // with no site plan there is no seam to stand a door in
        };
        let Some(seam) = plan.seams.iter().find(|s| &s.edge == e.id()) else {
            d.push(shape_c(
                path,
                format!(
                    "`{}` has no seam in the site plan: a door stands in a seam's opening",
                    e.id()
                ),
            ));
            continue;
        };
        let (lo, hi) = seam.opening;
        let ext = |i: usize| hi[i] - lo[i] + 1;
        let along = 2 - seam.normal_axis;
        let door_sized = seam.normal_axis != 1
            && seam.crossing == delvewright_dsl::siteplan::Crossing::Portal
            && ext(along) == 1
            && ext(1) == 2;
        if !door_sized {
            d.push(shape_c(
                path,
                format!(
                    "`{}`'s seam opening is {} wide and {} high{}: a door is one cell wide and \
                     two high, and stands in a wall — give the seam the `door` opening, or keep \
                     the way shut by bars",
                    e.id(),
                    if seam.normal_axis == 1 {
                        ext(0)
                    } else {
                        ext(along)
                    },
                    if seam.normal_axis == 1 {
                        ext(2)
                    } else {
                        ext(1)
                    },
                    if seam.normal_axis == 1 {
                        " through a floor"
                    } else {
                        ""
                    }
                ),
            ));
        }
    }
    let door_seams = door_shut_seam_anchors(c);
    if door_seams.is_empty() {
        return d;
    }
    let quests = &c.quests.content;
    for (ti, g) in quests.timed_gates.iter().enumerate() {
        if g.crush && door_seams.contains(g.gate.as_str()) {
            d.push(shape_c(
                format!("/content/timed_gates/{ti}/crush"),
                format!(
                    "timed gate `{}` crushes at `{}`, a door-shut seam: a leaf is not a \
                     portcullis — drop `crush`, or shut the seam by bars",
                    g.id.as_str(),
                    g.gate.as_str()
                ),
            ));
        }
    }
    for (qi, q) in quests.quests.iter().enumerate() {
        for (oi, o) in q.objectives.iter().enumerate() {
            let Objective::Interact {
                id,
                anchor,
                prop,
                release,
                ..
            } = o
            else {
                continue;
            };
            if !door_seams.contains(anchor.as_str()) {
                continue;
            }
            for (field, written) in [("prop", prop.is_some()), ("release", *release)] {
                if written {
                    d.push(Diagnostic::error(
                        DW_DOOR_NO_WAY,
                        "quests",
                        format!("/content/quests/{qi}/objectives/{oi}/{field}"),
                        format!(
                            "shape d: `interact` `{}` is anchored at `{}`, a door-shut seam, and \
                             writes `{field}`: the seam's door is the pressable, and the verb \
                             that opens the gate releases it to the hand — a second statement \
                             of what the seam already declares. Remove `{field}`",
                            id.as_str(),
                            anchor.as_str()
                        ),
                    ));
                }
            }
        }
    }
    d
}

fn shape_c(path: String, why: String) -> Diagnostic {
    Diagnostic::error(
        DW_DOOR_NO_WAY,
        "layout-graph",
        path,
        format!(
            "shape c: `shut: door` cannot stand here — {why} (spec-0124 §5.1). A door-shut seam is \
             a `walk` through a one-wide, two-high opening in a wall"
        ),
    )
}

/// The door-shut seams of a campaign as `(gate anchor, edge)`, in graph order.
#[must_use]
pub fn door_seam_edges(c: &Campaign) -> BTreeMap<String, String> {
    c.layout_graph
        .as_ref()
        .map(|g| {
            g.content
                .edges
                .iter()
                .filter(|e| e.door_shut())
                .map(|e| (seam_anchor(e.id()), e.id().as_str().to_string()))
                .collect()
        })
        .unwrap_or_default()
}

/// Every gated connection of a campaign that is built shut, by edge id.
#[must_use]
pub fn gated_edges(c: &Campaign) -> BTreeSet<String> {
    c.layout_graph
        .as_ref()
        .map(|g| {
            g.content
                .edges
                .iter()
                .filter(|e| e.shuts())
                .map(|e| e.id().as_str().to_string())
                .collect()
        })
        .unwrap_or_default()
}
