//! The drop shaft — a floor that spills you down and gives you no way back
//! (W4 entry L/1, drowned-bell remake step 2).
//!
//! **Original Delvewright content**, not a port: licence `original`. The
//! plainest one-way hardware in the vocabulary: an entry ledge, an open drop,
//! a landing far enough below that there is nothing to climb.
//!
//! ```text
//!  local Y        landing zone (low Z)        entry zone (high Z)
//!  drop+head..    ####air (roof, if any)#### ####air (roof, if any)####
//!  drop+1         air ......................  air (entry interior)
//!  drop           air ......................  ###### entry floor
//!  1..drop-1      air (open fall) ..........  ###### pillar (solid)
//!  0              ###### landing floor        ###### pillar (solid)
//!                 travel: local Z-max -> Z-min
//! ```
//!
//! Two Z-zones, directly adjacent — nothing needs a separate "gap" piece,
//! because the fall *is* the boundary between them. The entry floor sits
//! `drop` blocks above the landing floor with no ramp, stair or ladder between;
//! the landing's own interior is built `drop + head` cells tall, so the open
//! column above it reaches all the way up to the entry's own ceiling height.
//! Stepping off the entry ledge finds nothing underfoot and falls clean to the
//! landing floor below.
//!
//! # The gate
//!
//! Directionality, proved the family's way: graph connectivity over standable
//! cells, not block placement — but a *player's* movement is not an NPC's, so
//! the connectivity model has to be honest about that. `crate::nav`'s NPC
//! pathing (`reachable_walkable`) never risks more than a one-block drop, which
//! is right for an escorted NPC and wrong for a player who can walk off a
//! ledge. `tests/staging.rs`'s `reachable_with_fall` models the player: the
//! ordinary ±1-step edges `cliff_path` and `ambush_door` already use, plus a
//! one-way **fall** — stepping off a standable cell into an adjacent column
//! with nothing underfoot, landing on the first solid floor below, however far
//! that is.
//!
//! Two assertions, not one, because either alone proves the wrong thing:
//!
//! 1. `anchor/spill` reaches `anchor/landing` under that model — the structure
//!    is not simply broken in two pieces.
//! 2. `anchor/landing` does **not** reach `anchor/spill`, under the plain ±1
//!    step walk `cliff_path` already uses (no fall edge helps a climb — a fall
//!    only ever points down). Proving the negative under the *stricter* model
//!    used for gate 1 would be circular; using the plain walk here is the
//!    stronger, more honest claim. Teeth: a `rescue_ladder` knob notches every
//!    column of the entry floor but the one `anchor/spill` stands on, and —
//!    set alongside a short enough `drop` that the notch actually bridges the
//!    gap — the same check must find a way up.
//!
//! # Anchors
//!
//! * `anchor/spill` — the entry ledge's brink cell, the last standable cell
//!   before the floor stops. Facing is derived (negative local `Z`), pointing
//!   down-path at the drop — the direction a body finds nothing under.
//! * `anchor/landing` — the landing floor cell directly below the brink,
//!   facing further down the exit run.
//!
//! Smallest region that expands: **3 wide, `drop + head + 1` tall, 4 long**
//! (2 cells per zone at minimum) — and at least as long as it is wide, since
//! the frame turns length onto the longer horizontal axis.

use crate::block::BlockState;
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, Program, Reorient};

use super::{
    abs, abse, absp, all_of, alt_when, at_offset, call, cmp, dim, fill, int, marked, par, rel,
    reoriented, split, split_exact, void,
};

/// The drop shaft.
///
/// Parameters: `drop` (blocks of fall — also the pillar height under the entry
/// floor), `head` (interior headroom, both zones), `rescue_ladder` — a test
/// knob, off by default, that notches every column of the entry floor but the
/// far one so the no-way-back gate can be shown to fail when it should
/// (paired with a short `drop` in the test that exercises it — see the module
/// note). Palette role: `rock` (the whole shell).
pub fn drop_shaft() -> Program {
    Program::new("drop_shaft", "drop_shaft")
        .param("drop", 4)
        .param("head", 3)
        .param("rescue_ladder", 0)
        .role("rock", BlockState::simple("stone"))
        // --- frame -----------------------------------------------------------
        .rule(
            "drop_shaft",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("shaft_plan"),
            ),
        )
        // One alternative and no `otherwise`: a shaft shorter than its own drop
        // is not a smaller shaft, it is not one at all.
        .rule_alts(
            "shaft_plan",
            vec![alt_when(
                all_of(vec![
                    cmp(dim(DimRef::X), CmpOp::Ge, int(3)),
                    cmp(dim(DimRef::Z), CmpOp::Ge, int(4)),
                    cmp(par("drop"), CmpOp::Ge, int(2)),
                    cmp(par("head"), CmpOp::Ge, int(2)),
                ]),
                split_exact(
                    Axis::Z,
                    vec![rel(1), rel(1)],
                    vec![call("landing_zone"), call("entry_zone")],
                ),
            )],
        )
        // --- landing (low Z, low Y) --------------------------------------------
        .rule(
            "landing_zone",
            split(
                Axis::X,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("rock"), call("landing_column"), fill("rock")],
            ),
        )
        // Floor, then a `drop + head` tall open column — tall enough to reach
        // the entry's own ceiling height, so the fall from the ledge above has
        // clear air the whole way down.
        .rule(
            "landing_column",
            split(
                Axis::Y,
                vec![
                    abs(1),
                    abse(par("drop").arith(ArithOp::Add, par("head"))),
                    rel(1),
                ],
                vec![
                    fill("rock"),
                    marked(
                        "landing",
                        at_offset(
                            centered_x(),
                            int(0),
                            dim(DimRef::Z).arith(ArithOp::Sub, int(1)),
                        ),
                        void(),
                    ),
                    fill("rock"),
                ],
            ),
        )
        // --- entry (high Z, high Y) --------------------------------------------
        .rule(
            "entry_zone",
            split(
                Axis::X,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("rock"), call("entry_column"), fill("rock")],
            ),
        )
        .rule(
            "entry_column",
            split(
                Axis::Y,
                vec![absp("drop"), abs(1), absp("head"), rel(1)],
                vec![
                    fill("rock"),
                    call("entry_floor"),
                    // The far (`X`-max) column, deliberately not centred:
                    // `rescue_ladder` notches every *other* column of
                    // `entry_floor`, and marking `spill` on the one reserved
                    // column is what keeps the teeth test honest — the brink
                    // never stands on a cell the knob is about to hollow out
                    // from under it.
                    marked(
                        "spill",
                        at_offset(dim(DimRef::X).arith(ArithOp::Sub, int(1)), int(0), int(0)),
                        void(),
                    ),
                    fill("rock"),
                ],
            ),
        )
        // The floor a body stands on to look into the drop — solid, except when
        // `rescue_ladder` notches every column but the far one (reserved for
        // `anchor/spill`'s own support) to void, giving a foothold at the
        // pillar's own top that a diagonal step can bridge into the interior
        // from. See the module note: this only closes the loop when `drop` is
        // short enough for one notch to reach.
        .rule_alts(
            "entry_floor",
            vec![
                alt_when(cmp(par("rescue_ladder"), CmpOp::Le, int(0)), fill("rock")),
                alt_when(
                    cmp(par("rescue_ladder"), CmpOp::Ge, int(1)),
                    split(Axis::X, vec![rel(1), abs(1)], vec![void(), fill("rock")]),
                ),
            ],
        )
}

/// The centre of the current scope's local `X`, for a mark that has to sit in
/// the middle of a corridor rather than at a corner.
fn centered_x() -> crate::ir::Expr {
    dim(DimRef::X)
        .arith(ArithOp::Sub, int(1))
        .arith(ArithOp::Div, int(2))
}
