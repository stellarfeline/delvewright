//! What a player's click actually reaches at an anchor — the single authority
//! for the body every `strike`/`use` trigger is dispatched from.
//!
//! ## The defect this module exists to make impossible
//!
//! `EnvTrigger` is already the campaign's general "click a thing, run anything"
//! verb: any anchor, both clicks, the full `QuestEffect` vocabulary, flag gates
//! and `once`. Nothing about the *response* layer is missing. What was missing is
//! underneath it — **the trigger's body is a point at a cell, and an object in
//! the scene is a shape.**
//!
//! A standalone trigger summons one `minecraft:interaction` of `width:1.0f,
//! height:2.0f` at its anchor cell. That is right for a lever in open air and
//! wrong for anything solid or larger than a block:
//!
//! * on the `souls-shortcut` fixture, a `use` trigger at the shortcut's gate
//!   anchor summons a body at AABB `[4,65,6]..[5,67,7]` inside a doorway slab
//!   occupying `[4,65,6]..[6,68,7]`. Every face of the body is flush with the
//!   block or strictly interior to it, so vanilla — which bounds its entity
//!   raycast by the block hit distance and takes the entity only when it is
//!   *strictly* nearer — never reaches it. **The trigger compiles green, emits,
//!   and can be pressed from no angle at all.**
//! * a doorway is six cells; a point body covers one of them, so five sixths of
//!   the object answers nothing even when the geometry is otherwise fine.
//!
//! `close-gate` already solved exactly this, privately, inside one verb: its seal
//! arms one interaction per **shell cell** of the region, each one block plus
//! [`crate::emit::SEAL_MARGIN`] so it protrudes past the block it stands in
//! ([`crate::plan::SealHintPlan`]). That machinery was never available to
//! anything else, which is why the same press works on a sealed boulder and not
//! on a barred shortcut door.
//!
//! This module lifts the question out of every individual verb. One function
//! answers *what does a click at this anchor land on*, and both the emitter
//! ([`crate::emit::env_trigger_setup`]) and the collision proof
//! ([`crate::eclipse`]) read it, so the two can no longer disagree about whether
//! a trigger summoned a body — a disagreement that is invisible in the DSL and
//! only shows up as a dead click in a playtest.
//!
//! ## Riding, and why it is not an optimisation
//!
//! Where a compiler-owned interaction set already covers the anchor, the trigger
//! **rides** it: its `dw_trig_<id>` tag is added to those entities and it summons
//! nothing. A second co-located box is an exact ray-pick tie that resolves by
//! iteration order, which is `DW0422` and which is what killed the island's
//! boulder hint. One cell, one hitbox.

use crate::plan::{Plan, ResolvedAnchor};
use delvewright_dsl::{DwCode, ExitTier};

/// `DW0426`: a click trigger is anchored where a player can never click.
///
/// The unbound-vacuity class, as a diagnostic. The trigger declares an anchor, a
/// click and a full effect bundle; validation is happy, emission runs, and the
/// press lands on nothing — so the beat simply never happens and every board
/// stays green. This is the shape of the gap the whole task came from, and the
/// single most valuable thing here: it is the check that would have caught it.
pub const DW_TRIGGER_UNPRESSABLE: DwCode = DwCode::new("DW0426", ExitTier::Build);

/// What a click at an anchor lands on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Body {
    /// A compiler-owned interaction set already covers this anchor; the trigger
    /// rides it and summons nothing. Carries the tag ridden and a human name for
    /// diagnostics.
    Rides {
        /// The entity tag whose set the trigger joins.
        tag: String,
        /// What owns that set (`close-gate seal`, `shortcut door`, …).
        owner: &'static str,
    },
    /// The anchor names a **region**: arm one protruding box per clickable cell.
    Region(Vec<[i32; 3]>),
    /// The anchor names a point in open space: the ordinary `1.0f x 2.0f` body.
    /// Unchanged from before this module, so every campaign that only ever
    /// anchored triggers in the air is byte-identical.
    Point([i32; 3]),
    /// Nothing here resolves — `DW0426`.
    Nothing,
}

/// The body a `strike`/`use` trigger anchored at `anchor` is dispatched from.
///
/// Resolution order is most-specific-first, and every arm is a place a hitbox
/// **already** exists or provably should:
///
/// 1. a `close-gate` seal over this gate anchor — ride `dw_seal_<safe>`;
/// 2. a `shortcut` whose gate this is — ride `dw_ws_<safe>`, the sealed-side
///    body (see [`crate::wrongside`] for why that placement is also the side
///    test);
/// 3. any other **gate region** anchor — arm the region's own clickable shell,
///    which is what a point body fails to do and the whole reason this exists;
/// 4. a point anchor — the ordinary body, untouched;
/// 5. nothing — `DW0426`.
///
/// The NPC case is deliberately **not** here: a `strike` on an NPC's stand anchor
/// rides that NPC's own dialogue hitbox, but whether it does depends on the
/// trigger's *kind* rather than on the place, so it stays with the caller that
/// knows the kind ([`crate::emit`]).
pub fn body_at(plan: &Plan, anchor: &str) -> Body {
    if let Some(s) = plan.seal_hints.iter().find(|s| s.anchor == anchor) {
        return Body::Rides {
            tag: format!("dw_seal_{}", s.safe),
            owner: "close-gate seal",
        };
    }
    if let Some(sc) = plan
        .shortcuts
        .iter()
        .find(|sc| sc.gate_anchor == anchor && sc.sealed_side.is_some())
    {
        return Body::Rides {
            tag: format!("dw_ws_{}", sc.safe),
            owner: "shortcut door",
        };
    }
    for ((_, name), resolved) in &plan.anchors {
        if name != anchor {
            continue;
        }
        return match resolved {
            ResolvedAnchor::Gate { from, to, .. } => Body::Region(shell_cells(*from, *to)),
            ResolvedAnchor::Point { pos, .. } => Body::Point(*pos),
        };
    }
    Body::Nothing
}

/// One line of human-readable prose naming what a click at an anchor lands on.
pub fn describe(body: &Body) -> String {
    match body {
        Body::Rides { owner, tag } => format!("rides the {owner}'s own hitboxes (`{tag}`)"),
        Body::Region(cells) => format!("arms {} clickable cell(s) of the region", cells.len()),
        Body::Point(p) => format!("a 1.0x2.0 body in open air at {p:?}"),
        Body::Nothing => "nothing — DW0426".to_string(),
    }
}

/// What the `DW0426` proof resolved a body for, on this build.
///
/// Emitted as `validation/press-bodies.json`. `DW0426` is an error, so a build
/// that ships proves *no press lands on nothing* — but that sentence is equally
/// true of a campaign with no click triggers at all, and the two are the same
/// silence from outside. This ledger is the difference (CLAUDE.md: *every
/// validation artifact states its binding count*), and it doubles as the record
/// of WHICH body each press got: a trigger that rides a seal and one that arms a
/// six-cell doorway shell are both green here and behave completely differently
/// under a crosshair.
#[derive(Clone, Debug, Default)]
pub struct PressLedger {
    /// `(trigger id, click kind, anchor, what the click lands on)`, in campaign
    /// declaration order.
    pub presses: Vec<(String, String, String, String)>,
}

impl PressLedger {
    /// Record one resolved press.
    pub fn push(&mut self, trigger: &str, kind: &str, anchor: &str, body: &str) {
        self.presses.push((
            trigger.to_string(),
            kind.to_string(),
            anchor.to_string(),
            body.to_string(),
        ));
    }

    /// The ledger as the `validation/press-bodies.json` artifact.
    pub fn to_json(&self) -> serde_json::Value {
        let n = self.presses.len();
        let mut o = serde_json::json!({
            "code": DW_TRIGGER_UNPRESSABLE,
            "presses": self
                .presses
                .iter()
                .map(|(id, kind, anchor, body)| serde_json::json!({
                    "trigger": id,
                    "click": kind,
                    "anchor": anchor,
                    "body": body,
                }))
                .collect::<Vec<_>>(),
            "examined": n,
            "unbound": n == 0,
        });
        if n == 0 {
            o["reason"] = serde_json::json!(
                "this campaign arms no `strike`/`use` trigger on an anchor at all, so nothing \
                 here can be pressed and DW0426 had nothing to resolve a body for. A press that \
                 lands on nothing is the defect this proof exists for; a campaign with no \
                 presses has not passed it, it is outside it"
            );
        }
        o
    }
}

/// The shell cells of an inclusive region: every cell with at least one
/// axis-neighbour outside it, ascending `(x, y, z)`.
///
/// A cell buried inside the region has six occupied neighbours, so no face of it
/// can ever be in a crosshair and arming it would ship an entity nothing can
/// reach. The same rule `close-gate`'s seal applies, for the same reason.
fn shell_cells(a: [i32; 3], b: [i32; 3]) -> Vec<[i32; 3]> {
    let lo = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
    let hi = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];
    let mut out = Vec::new();
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                let interior = (lo[0] < x && x < hi[0])
                    && (lo[1] < y && y < hi[1])
                    && (lo[2] < z && z < hi[2]);
                if !interior {
                    out.push([x, y, z]);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-block-deep doorway is all shell — every cell of it is clickable, and
    /// a point body would have covered one of six.
    #[test]
    fn a_doorway_slab_is_entirely_clickable() {
        assert_eq!(shell_cells([4, 65, 6], [5, 67, 6]).len(), 6);
    }

    /// A solid cube's buried centre is not armed: nothing can ever reach it.
    #[test]
    fn a_buried_cell_is_not_armed() {
        let cells = shell_cells([0, 0, 0], [2, 2, 2]);
        assert_eq!(cells.len(), 26, "27 cells less the buried one");
        assert!(!cells.contains(&[1, 1, 1]));
    }

    /// Ascending order, so emission is byte-stable across builds (ADR-0006).
    #[test]
    fn shell_order_is_deterministic() {
        let cells = shell_cells([4, 65, 6], [5, 67, 6]);
        let mut sorted = cells.clone();
        sorted.sort();
        assert_eq!(cells, sorted);
    }
}
