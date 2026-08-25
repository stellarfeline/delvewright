//! A room a body can walk across with ordinary decoration lying on its floor.
//!
//! **Original Delvewright content**, licence `original`. It is a corpus example
//! rather than a piece of the building vocabulary: nobody composes a zone out of
//! this. It exists because of what it BINDS.
//!
//! # What this program is for
//!
//! Before spec-0056 the grammar walk answered "can a body occupy this cell" with
//! *air, or a block whose name ends in `_skull`* — everything else was a full
//! solid cube. A torch was a wall. So was a candle, a carpet, a pressure plate
//! and a tuft of grass, and in a room of ordinary height (floor, two courses,
//! ceiling) a single course of decoration across the floor **severs the room**:
//! the decorated cell is not standable because a body cannot be in it, and the
//! cell above it is not standable because the ceiling eats its headroom. The
//! whole far half of the room becomes floor nothing can reach.
//!
//! That was not a hypothetical. It is the first thing anybody hits who lights a
//! room, and it made every zero-collision block unusable anywhere a player walks.
//! This program is that room, in the corpus, judged on every build — so the
//! defect cannot come back without a red.
//!
//! ```text
//!  local Y (bottom to top):   floor · decoration + walk clearance · ceiling
//!  local Z:  0 = the wall with the doorway;  Z-1 = the back wall
//!  local X:  0 and X-1 are walls; the decoration runs wall to wall across the
//!            middle course of the room, so anything that reads it as solid cuts
//!            the room in two.
//! ```
//!
//! # Why it is the corpus's only `reachable_floor` claim
//!
//! It is the gate this defect fails, and before this entry the gate bound
//! **zero** over the whole library — green because nothing asked it anything
//! (CLAUDE.md, the unbound vacuity mode). A room with a doorway and a roof is the
//! smallest honest thing to ask it about.
//!
//! # What a creator building something else binds to it
//!
//! The decoration roles are the whole surface: `torch`, `candle`, `carpet` and
//! `plate` are four palette bindings, and a creator swaps them for whatever their
//! own fiction lays on a floor — grass and flowers in a meadow, rails and a lever
//! in a mine, snow in a drift. The claim the program makes is about the *class*,
//! not about these five blocks: nothing here is a wall, and the room stays
//! walkable whatever is bound.
//!
//! Smallest region that expands: X ≥ 9 (the decoration stripe wants 2 + 2 + 2 and
//! at least one more cell, inside two walls), Y ≥ 4 (floor, two courses of
//! clearance, ceiling — two is what `standable` asks for), Z ≥ 5.

use crate::block::BlockState;
use crate::geom::Axis;
use crate::ir::{CmpOp, DimRef, Program};

use super::{abs, all_of, alt_when, call, cmp, dim, fill, int, rel, split, split_exact, void};

/// The narrowest room the decoration stripe fits in: two walls plus the
/// 2 + 2 + 2 + at-least-1 stripe.
pub const MIN_WIDTH: i64 = 9;

/// A room with its floor dressed, and a doorway to walk in through.
///
/// No size parameters: the room is the box it is given. Palette roles: `stone`
/// (shell), and the four decoration bindings `torch`, `candle`, `carpet`,
/// `plate` — every one of them a block vanilla gives no collision box a body
/// meets, and every one of them a full solid cube to the walk before spec-0056.
pub fn decorated_room() -> Program {
    Program::new("decorated_room", "room")
        .role("stone", BlockState::simple("stone_bricks"))
        .role("torch", BlockState::simple("torch"))
        .role(
            "candle",
            BlockState::with(
                "white_candle",
                [("candles", "3"), ("lit", "true"), ("waterlogged", "false")],
            ),
        )
        .role("carpet", BlockState::simple("red_carpet"))
        .role(
            "plate",
            BlockState::with("stone_pressure_plate", [("powered", "false")]),
        )
        // Floor, room, ceiling. The ceiling is what makes the defect bite: with
        // open sky over it a body would simply stand on top of the decoration.
        .rule_alts(
            "room",
            vec![alt_when(
                all_of(vec![
                    cmp(dim(DimRef::X), CmpOp::Ge, int(MIN_WIDTH)),
                    cmp(dim(DimRef::Y), CmpOp::Ge, int(4)),
                    cmp(dim(DimRef::Z), CmpOp::Ge, int(5)),
                ]),
                split(
                    Axis::Y,
                    vec![abs(1), rel(1), abs(1)],
                    vec![fill("stone"), call("walls_x"), fill("stone")],
                ),
            )],
        )
        .rule(
            "walls_x",
            split(
                Axis::X,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("stone"), call("walls_z"), fill("stone")],
            ),
        )
        .rule(
            "walls_z",
            split(
                Axis::Z,
                vec![abs(1), rel(1), abs(1)],
                vec![call("near_wall"), call("interior"), fill("stone")],
            ),
        )
        // One doorway at grade, so `ground_entry` has somewhere to start the
        // walk from. A sealed room binds zero and proves nothing.
        .rule(
            "near_wall",
            split_exact(
                Axis::X,
                vec![rel(1), abs(1), rel(1)],
                vec![fill("stone"), void(), fill("stone")],
            ),
        )
        // The decoration sits on the floor; everything above it is clearance.
        .rule(
            "interior",
            split(
                Axis::Y,
                vec![abs(1), rel(1)],
                vec![call("decor_band"), void()],
            ),
        )
        // One course, wall to wall, across the middle of the room.
        .rule(
            "decor_band",
            split_exact(
                Axis::Z,
                vec![rel(1), abs(1), rel(1)],
                vec![void(), call("decor_stripe"), void()],
            ),
        )
        .rule(
            "decor_stripe",
            split_exact(
                Axis::X,
                vec![abs(2), abs(2), abs(2), rel(1)],
                vec![fill("torch"), fill("candle"), fill("carpet"), fill("plate")],
            ),
        )
}
