//! The tee passage — a chain segment with a doorway in its *side* (W4 entry J,
//! the junction the drowned-bell remake's shortcuts hang off).
//!
//! **Original Delvewright content**, not a port: licence `original`. Every other
//! rule in the vocabulary walls both of its side faces, so a composition is a
//! chain along one axis and a zone has no way to hand a piece a box *off* the
//! route. A `far_side_bar` laid in that chain therefore seals the zone instead
//! of sitting beside it — the opposite of a shortcut (spec-0016 §2). This rule
//! is the missing vocabulary: a lane that still chains end to end, and opens one
//! doorway sideways for whatever the zone parks next to it.
//!
//! ```text
//!  local X:   0        1 .. X-2      X-1
//!            door wall  the lane     solid side wall
//!
//!  seen from above, travel running down the page:
//!
//!      │####################│      the branch box the zone hands to
//!      │####     ▓▓▓▓▓▓▓▓▓▓▓│      `far_side_bar` sits out here, to
//!      │####  D  ▓ the lane ▓│ ◀── local X-min, on the far side of D
//!      │####     ▓▓▓▓▓▓▓▓▓▓▓│
//!      │####################│      travel: local Z-max -> Z-min
//! ```
//!
//! # Why this is vocabulary and not a new primitive
//!
//! The IR already expresses "a chain segment whose one side face carries a
//! doorway": `ambush_door` and `far_side_bar` are both a wall-with-one-opening,
//! merely turned 90° from where a branch needs it. What was missing was a rule
//! that turns it, not a node kind that could not be written before — and the
//! no-hack rule cuts *against* widening the IR for something the layer below
//! already says. The two alternatives that were considered and rejected are
//! recorded in `docs/reference/grammar.md` §5c.
//!
//! # The gates
//!
//! 1. **The lane is still a chain segment** — standable end to end along local
//!    `Z`, so a tee dropped into a zone's piece run does not become the thing
//!    that severs it.
//! 2. **Exactly one opening, in exactly one side face** — every cell of both
//!    side-wall planes is solid except the `door_height` cells of the doorway,
//!    counted rather than eyeballed. A second gap would be a branch nobody
//!    declared and a route nobody proved.
//! 3. **The doorway is beside the route, not on it** — deleting the doorway's
//!    own column leaves the lane connected end to end. That is the whole
//!    difference between this rule and `far_side_bar`, and it is asserted rather
//!    than argued.
//!
//! Teeth for 2 and 3: `sealed = 1` fills the doorway with the shell material and
//! nothing else changes, so the opening count drops to zero while the lane walks
//! exactly as before.
//!
//! # Anchors
//!
//! * `anchor/branch-door` — the doorway's own floor cell, facing **across**
//!   travel at the box the branch occupies. The facing is derived through a
//!   `reorient` naming the across-lane axis as local `Z`, the same construction
//!   `cliff_path`'s recesses and `ambush_door`'s alcove use; it is why the
//!   doorway is in the local `X`-min face and not the other one.
//!
//! Smallest region that expands: **3 wide, `head + 2` tall, [`MIN_LENGTH`]
//! long** — and, since the frame turns length onto the longer horizontal axis,
//! at least as long as it is wide.

use crate::block::BlockState;
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, MarkAt, Program, Reorient};

use super::{
    abs, absp, all_of, alt_when, call, cmp, dim, fill, int, marked, par, rel, reoriented, split,
    split_exact, void,
};

/// The narrowest box the rule will cut a lane in: door wall, one cell of lane,
/// far side wall.
pub const MIN_WIDTH: i64 = 3;

/// The shortest run the rule will lay: a cell of wall on each side of the
/// doorway, so the opening is a doorway in a wall and not the whole face.
pub const MIN_LENGTH: i64 = 3;

/// The tee passage.
///
/// Parameters: `head` (interior headroom), `door_height` (how tall the side
/// doorway is, at most `head`), `sealed` — a test knob, off by default, that
/// fills the doorway with the shell material so the one-opening and
/// beside-the-route gates can be shown to fail when they should. Palette role:
/// `rock`.
pub fn tee_passage() -> Program {
    Program::new("tee_passage", "passage")
        .param("head", 3)
        .param("door_height", 2)
        .param("sealed", 0)
        .role("rock", BlockState::simple("stone_bricks"))
        // --- frame -----------------------------------------------------------
        .rule(
            "passage",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("passage_plan"),
            ),
        )
        // One alternative, no `otherwise`. A box too narrow for a lane between
        // two walls, or too short to leave wall on both sides of the doorway, is
        // not a smaller tee — it is a wall with a hole in it, which is a
        // different rule. A refusal naming this one is the honest answer.
        .rule_alts(
            "passage_plan",
            vec![alt_when(
                all_of(vec![
                    cmp(dim(DimRef::X), CmpOp::Ge, int(MIN_WIDTH)),
                    cmp(dim(DimRef::Z), CmpOp::Ge, int(MIN_LENGTH)),
                    cmp(par("door_height"), CmpOp::Ge, int(1)),
                    cmp(par("head"), CmpOp::Ge, par("door_height")),
                    cmp(
                        dim(DimRef::Y),
                        CmpOp::Ge,
                        par("head").arith(ArithOp::Add, int(2)),
                    ),
                ]),
                split(
                    Axis::X,
                    vec![abs(1), rel(1), abs(1)],
                    vec![call("door_wall"), call("lane_column"), fill("rock")],
                ),
            )],
        )
        // --- the lane ---------------------------------------------------------
        // Open along the whole local Z, which is what makes the piece chainable:
        // both ends of the run are interior, so a neighbour meets air.
        .rule(
            "lane_column",
            split(
                Axis::Y,
                vec![abs(1), absp("head"), rel(1)],
                vec![fill("rock"), void(), fill("rock")],
            ),
        )
        // --- the side wall, and its one doorway --------------------------------
        // `split_exact`, not `split`: two relative pieces under truncation leave
        // the far end of the wall unwritten, and an unwritten cell is air — a
        // second opening in the one face whose solidity this rule exists to
        // promise. The odd block goes to the earliest share, so the doorway sits
        // at or just past the middle of the run, deterministically.
        .rule(
            "door_wall",
            split_exact(
                Axis::Z,
                vec![rel(1), abs(1), rel(1)],
                vec![fill("rock"), call("door_column"), fill("rock")],
            ),
        )
        .rule(
            "door_column",
            split(
                Axis::Y,
                vec![abs(1), absp("door_height"), rel(1)],
                vec![fill("rock"), call("branch_door"), fill("rock")],
            ),
        )
        // Turn the scope so its local Z is the across-lane axis: the derived
        // facing is then the negative direction of that axis, which points out
        // through the doorway at the branch. The reorientation moves no block;
        // it exists to aim the anchor, exactly as `ambush_door`'s alcove does.
        .rule(
            "branch_door",
            reoriented(
                Reorient::KEEP.z(AxisSpec::LocalX),
                marked(
                    "branch-door",
                    MarkAt::FloorCenter,
                    call("doorway_or_sealed"),
                ),
            ),
        )
        .rule_alts(
            "doorway_or_sealed",
            vec![
                alt_when(cmp(par("sealed"), CmpOp::Le, int(0)), void()),
                alt_when(cmp(par("sealed"), CmpOp::Ge, int(1)), fill("rock")),
            ],
        )
}
