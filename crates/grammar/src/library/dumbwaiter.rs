//! The dumbwaiter — a walled vertical duct with no ladder for the return trip
//! (W4 entry L/2, drowned-bell remake step 2).
//!
//! **Original Delvewright content**, not a port: licence `original`. Where
//! [`crate::library::drop_shaft`] is a floor that simply stops (a cliff you
//! walk off), the dumbwaiter is deliberate hardware: an enclosed shaft, narrow
//! enough to read as a mechanism rather than a hazard, that you have to choose
//! to step into.
//!
//! ```text
//!  local X:  wall  margin  duct_core  margin  wall     (duct_zone only)
//!  local Z:  |<- landing_run (rel) ->|<- duct_zone (abs) ->|<- entry_run (rel) ->|
//!                low Y, low Z                              high Y, high Z
//!                                     travel: local Z-max -> Z-min
//! ```
//!
//! `landing_run` and `entry_run` are the same shell `drop_shaft` uses — a
//! floor, an interior tall enough to reach the other zone's ceiling height, a
//! roof. `duct_zone` sits between them and re-splits its own `X` independently
//! of both: a solid margin on each side, and a `duct_core` down the middle that
//! is narrower than either room. The margins are what make this a *duct* and
//! not a second cliff — a body can only enter the shaft at the one column
//! where the floor stops, not by walking off anywhere along the boundary.
//!
//! `duct_core`'s own column is built exactly like `landing_run`'s — floor at
//! low `Y`, open the rest of the way up — so the open shaft above it reaches
//! `entry_run`'s ceiling height and a step off `entry_run`'s ledge into the
//! core's airspace finds nothing underfoot, precisely as `drop_shaft` builds
//! its own fall.
//!
//! # The gate
//!
//! The same two assertions as `drop_shaft`, against the same
//! `reachable_with_fall` / plain-walk pair (see that module's note for why the
//! model has to be that permissive going forward and that strict going back):
//!
//! 1. `anchor/hatch` reaches `anchor/landing` under the fall-capable walk.
//! 2. `anchor/landing` does **not** reach `anchor/hatch` under the plain ±1
//!    step walk. Teeth: the same `rescue_ladder` knob `drop_shaft` uses,
//!    notching every column of `entry_run`'s floor but the one `anchor/hatch`
//!    stands on — wide enough to be certain of overlapping wherever
//!    `duct_core`'s own runtime-split margins actually put it — and, paired
//!    with a short enough `drop` that the notch bridges the gap, the same
//!    check must find a way up.
//!
//! # Anchors
//!
//! * `anchor/hatch` — the entry ledge's brink cell, where the floor stops and
//!   the duct begins. Facing derived, pointing down-path into the shaft.
//! * `anchor/landing` — the landing floor cell nearest the duct, facing further
//!   down the exit run.
//!
//! Smallest region that expands: **`duct_width + 4` wide, `drop + head + 1`
//! tall, `duct_len + 4` long** (2 cells per room, `duct_len` for the shaft
//! itself) — and at least as long as it is wide.

use crate::block::BlockState;
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, Program, Reorient};

use super::{
    abs, abse, absp, all_of, alt_when, at_offset, call, cmp, dim, fill, int, marked, par, rel,
    reoriented, split, split_exact, void,
};

/// The dumbwaiter.
///
/// Parameters: `drop` (blocks of fall), `head` (interior headroom), `duct_len`
/// (`Z` length of the walled shaft segment), `duct_width` (its `X` width),
/// `rescue_ladder` — the same test knob `drop_shaft` documents. Palette role:
/// `rock` (the whole shell).
pub fn dumbwaiter() -> Program {
    Program::new("dumbwaiter", "dumbwaiter")
        .param("drop", 4)
        .param("head", 3)
        .param("duct_len", 2)
        .param("duct_width", 1)
        .param("rescue_ladder", 0)
        .role("rock", BlockState::simple("stone"))
        // --- frame -----------------------------------------------------------
        .rule(
            "dumbwaiter",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("duct_plan"),
            ),
        )
        .rule_alts(
            "duct_plan",
            vec![alt_when(
                all_of(vec![
                    cmp(
                        dim(DimRef::X),
                        CmpOp::Ge,
                        par("duct_width").arith(ArithOp::Add, int(4)),
                    ),
                    cmp(
                        dim(DimRef::Z),
                        CmpOp::Ge,
                        par("duct_len").arith(ArithOp::Add, int(4)),
                    ),
                    cmp(par("drop"), CmpOp::Ge, int(2)),
                    cmp(par("head"), CmpOp::Ge, int(2)),
                    cmp(par("duct_width"), CmpOp::Ge, int(1)),
                    cmp(par("duct_len"), CmpOp::Ge, int(1)),
                ]),
                split_exact(
                    Axis::Z,
                    vec![rel(1), absp("duct_len"), rel(1)],
                    vec![call("landing_run"), call("duct_zone"), call("entry_run")],
                ),
            )],
        )
        // --- landing room (low Z, low Y) ---------------------------------------
        .rule(
            "landing_run",
            split(
                Axis::X,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("rock"), call("landing_column"), fill("rock")],
            ),
        )
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
        // --- the walled duct (middle Z) -----------------------------------------
        .rule(
            "duct_zone",
            split_exact(
                Axis::X,
                vec![rel(1), absp("duct_width"), rel(1)],
                vec![fill("rock"), call("duct_core"), fill("rock")],
            ),
        )
        // The core is built exactly like the landing room's own column: floor
        // low, open the whole way up. It is what makes the shaft a shaft rather
        // than a second cliff — the same shape, just narrower and walled.
        .rule(
            "duct_core",
            split(
                Axis::Y,
                vec![
                    abs(1),
                    abse(par("drop").arith(ArithOp::Add, par("head"))),
                    rel(1),
                ],
                vec![fill("rock"), void(), fill("rock")],
            ),
        )
        // --- entry room (high Z, high Y) ----------------------------------------
        .rule(
            "entry_run",
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
                    // The far column — see `drop_shaft`'s identical mark for
                    // why this is deliberately not the centred one.
                    marked(
                        "hatch",
                        at_offset(dim(DimRef::X).arith(ArithOp::Sub, int(1)), int(0), int(0)),
                        void(),
                    ),
                    fill("rock"),
                ],
            ),
        )
        // Solid, except when `rescue_ladder` notches every column but the far
        // one to void — see `drop_shaft`'s identical rule for why the far
        // column stays reserved, and its module note for why this only closes
        // the loop when `drop` is short enough for one notch to reach. Here
        // the wide notch also has to overlap wherever `duct_core` actually
        // landed (its own margins split at runtime); reserving only the one
        // column `anchor/hatch` needs and notching everything else is what
        // guarantees that overlap regardless.
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

/// The centre of the current scope's local `X`.
fn centered_x() -> crate::ir::Expr {
    dim(DimRef::X)
        .arith(ArithOp::Sub, int(1))
        .arith(ArithOp::Div, int(2))
}
