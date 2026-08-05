//! The corner-ambush alcove — a doorway with a blind pocket beside it (W2
//! entry A, drowned-bell remake step 2).
//!
//! **Original Delvewright content**, not a port: licence `original`. The oldest
//! move in the catalogue — the Thrall that leaps from beside a Cathedral doorway,
//! the Fanged Imp tucked at the right corner of a Stormfoot dead end
//! (`docs/notes/souls-design-language.md` §4.1).
//!
//! ```text
//!  local Z:   0 .......... inside room ...... row  wall  approach ...... Z-1
//!                                              ^     ^      ^ travel starts here
//!  local X:   0    1 .. d      d+1      d+2     d+3 .. X-2    X-1
//!            wall  room       DOOR    ALCOVE      room       wall
//!                             lane
//!                                   travel: local Z-max -> Z-min
//! ```
//!
//! The wall spans the box; one column of it is open. Immediately inside that
//! opening, one cell to the `+X` side, is the alcove — an ordinary standable
//! cell of the room, made blind by nothing more than the wall it hides behind.
//!
//! # The gate that runs backwards
//!
//! Every other staging rule proves something is *visible*. This one proves the
//! opposite, and it is the entry's whole reason to exist: from every standable
//! cell of the approach, the alcove must be **unseeable**. `tests/staging.rs`
//! asserts it cell by cell with the same sightline walk the positive gates use,
//! so "blind" means the same thing here as "clear" does there.
//!
//! Blindness is bought by one geometric fact: the only opening in the wall is
//! one cell wide, and the alcove is not in its lane. Any ray from the approach
//! that reaches the alcove has to pass through the wall column beside the
//! opening. The `expose` knob widens the opening to include the alcove's own
//! lane, which is exactly the mistake the gate exists to catch, and the test
//! watches it catch it.
//!
//! # Fairness, since the ambush is blind on purpose
//!
//! §4.2 of the dossier is firm that an ambush must be discoverable from the
//! position where the player *decides*, and a blind alcove plainly is not. That
//! is the point of the teach/test/twist ladder rather than an argument against
//! this rule: the corner ambush is the **test** rung, and it is fair only after
//! the same delve has taught the shape somewhere legible. The rule declares
//! `anchor/threshold` so a campaign has a place to hang the telegraph that pays
//! for it; it does not pretend the alcove itself is a tell.
//!
//! # Anchors
//!
//! * `anchor/alcove` — the blind cell, facing the door lane. The facing is
//!   derived through a `reorient` that names the across-wall axis as local `Z`,
//!   which is why the alcove is on the `+X` side of the opening and not the
//!   other one (the same construction `cliff_path` uses for its recesses).
//! * `anchor/threshold` — the standable cell in the opening itself, facing the
//!   way the player walks through it. A `close-gate` or a telegraph binds here.
//!
//! Smallest region that expands: X ≥ `door_offset + 5 + expose`, Y ≥ `head + 2`,
//! Z ≥ 5 — and, since the frame turns length onto the longer horizontal axis, at
//! least as long as it is wide.

use crate::block::BlockState;
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, MarkAt, Program, Reorient};

use super::{
    abs, abse, absp, all_of, alt_when, call, cmp, dim, fill, int, marked, par, rel, reoriented,
    split, split_exact, void,
};

/// The shallowest box the rule will cut a threshold in: room, alcove row, wall,
/// and two cells of approach to be refused a view from.
pub const MIN_LENGTH: i64 = 5;

/// The doorway with its flanking alcove.
///
/// Parameters: `head` (interior headroom), `door_height` (how tall the opening
/// is, at most `head`), `door_offset` (cells of wall between the `X`-min side
/// wall and the opening), and `expose` — a test knob, off by default, that
/// widens the opening over the alcove's lane so the negative-visibility gate can
/// be shown to fail when it should. Palette role: `stone`.
pub fn ambush_door() -> Program {
    Program::new("ambush_door", "threshold")
        .param("head", 3)
        .param("door_height", 2)
        .param("door_offset", 2)
        .param("expose", 0)
        .role("stone", BlockState::simple("stone_bricks"))
        // --- frame -----------------------------------------------------------
        .rule(
            "threshold",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("threshold_plan"),
            ),
        )
        // One alternative and no `otherwise`. Every clause is load-bearing for
        // the blindness proof — a door taller than the room, a box too narrow to
        // hold wall/door/alcove/room, an approach with nowhere to stand — so a
        // box that fails one gets a refusal naming the rule, not a smaller
        // ambush that quietly is not one.
        .rule_alts(
            "threshold_plan",
            vec![alt_when(
                all_of(vec![
                    cmp(par("door_height"), CmpOp::Ge, int(2)),
                    cmp(par("head"), CmpOp::Ge, par("door_height")),
                    cmp(par("door_offset"), CmpOp::Ge, int(1)),
                    cmp(
                        dim(DimRef::Y),
                        CmpOp::Ge,
                        par("head").arith(ArithOp::Add, int(2)),
                    ),
                    cmp(
                        dim(DimRef::X),
                        CmpOp::Ge,
                        par("door_offset")
                            .arith(ArithOp::Add, int(5))
                            .arith(ArithOp::Add, par("expose")),
                    ),
                    cmp(dim(DimRef::Z), CmpOp::Ge, int(MIN_LENGTH)),
                ]),
                // `split_exact`, not `split`: two relative pieces under
                // truncation leave the far end of the box with no floor written,
                // and a hole in the floor would break the only-route gate for a
                // reason that has nothing to do with the door.
                split_exact(
                    Axis::Z,
                    vec![rel(1), abs(1), rel(1)],
                    vec![call("inside"), call("wall"), call("plain_slab")],
                ),
            )],
        )
        // --- the two open volumes ---------------------------------------------
        // The room, and against the wall the one row that holds the alcove.
        .rule(
            "inside",
            split_exact(
                Axis::Z,
                vec![rel(1), abs(1)],
                vec![call("plain_slab"), call("alcove_row")],
            ),
        )
        // Plain floor-to-ceiling room, walled at both ends of the width.
        .rule(
            "plain_slab",
            split(
                Axis::X,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("stone"), call("air_column"), fill("stone")],
            ),
        )
        .rule(
            "air_column",
            split(
                Axis::Y,
                vec![abs(1), absp("head"), rel(1)],
                vec![fill("stone"), void(), fill("stone")],
            ),
        )
        // The row against the wall. The door's inside cell and the alcove are
        // separate one-wide pieces so that the alcove is *declared* where it is,
        // rather than computed later from an offset nobody re-checks.
        .rule(
            "alcove_row",
            split(
                Axis::X,
                vec![abs(1), absp("door_offset"), abs(1), abs(1), rel(1), abs(1)],
                vec![
                    fill("stone"),
                    call("air_column"),
                    call("air_column"),
                    call("alcove_column"),
                    call("air_column"),
                    fill("stone"),
                ],
            ),
        )
        .rule(
            "alcove_column",
            split(
                Axis::Y,
                vec![abs(1), absp("head"), rel(1)],
                vec![fill("stone"), call("alcove_air"), fill("stone")],
            ),
        )
        // Turn the scope so its local Z is the across-wall axis: the derived
        // facing is then the negative direction of that axis, which points out
        // of the alcove at the door lane one cell away. The reorientation moves
        // no block; it exists to aim the anchor.
        .rule(
            "alcove_air",
            reoriented(
                Reorient::KEEP.z(AxisSpec::LocalX),
                marked("alcove", MarkAt::CornerMin, void()),
            ),
        )
        // --- the wall ----------------------------------------------------------
        // Solid across, one column open. `expose` widens that column over the
        // alcove's lane — the defect, kept expressible so the gate can catch it.
        .rule(
            "wall",
            split(
                Axis::X,
                vec![
                    abse(par("door_offset").arith(ArithOp::Add, int(1))),
                    abse(int(1).arith(ArithOp::Add, par("expose"))),
                    rel(1),
                ],
                vec![fill("stone"), call("door_column"), fill("stone")],
            ),
        )
        .rule(
            "door_column",
            split(
                Axis::Y,
                vec![abs(1), absp("door_height"), rel(1)],
                vec![
                    fill("stone"),
                    marked("threshold", MarkAt::CornerMin, void()),
                    fill("stone"),
                ],
            ),
        )
}
