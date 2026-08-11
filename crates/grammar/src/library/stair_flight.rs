//! The stair flight — the vocabulary's first route a body can walk **up**.
//!
//! **Original Delvewright content**, not a port: licence `original`. A walled
//! shaft with a level landing at each end and a rising run of single-block
//! treads between them, gated on being walkable in **both** directions — which
//! is the exact negation of the gate [`super::drop_shaft`] and
//! [`super::dumbwaiter`] owe.
//!
//! ```text
//!  local Y      head landing (low Z)                 run                foot landing (high Z)
//!  y0+n+1..     air ................................ air .............. air
//!  y0+n         #### landing floor                   air               air
//!  y0+2         ####                       ####|air                    air
//!  y0+1         ####                       ########|air                #### foot floor
//!  y0           ####                       ###############             ####
//!                                          travel: local Z-max -> Z-min, climbing
//! ```
//!
//! # Is a climbing run expressible without a per-iteration index? Yes.
//!
//! [`super::boulder_stair`]'s module says a rule "cannot ask a repeated slice to
//! climb a block per iteration — there is no index the IR exposes to a `Size`".
//! That is true of `split_repeat`, and `split_repeat` is the wrong verb: a
//! repeat is a **tiling**, every tile handed the same pattern, so of course no
//! tile can know it is the fourth. It is not true of the IR.
//!
//! The index a stair needs is not a counter, it is **the box that is left**, and
//! a self-call already carries that. `run` fills one course of its own floor and
//! hands the rest of the box — one shorter in `Y`, one tread shorter in `Z` — to
//! itself; the guard reads those remaining dimensions, and the recursion stops
//! when either runs out. Every level's floor is one block above its parent's
//! because it *is* one block above its parent's, not because anything counted.
//! [`super::store_room`] already banks on exactly this shape ("a rule has no
//! memory, so the invariant is in the derivation's shape") to place exactly one
//! odd barrel without a counter; a stair is the same trick aimed at `Y`.
//!
//! So **no IR change was needed**, and the entry that said one was is corrected
//! rather than worked around (`docs/reference/grammar.md` §5b, §7).
//!
//! The one thing recursion spends that a tiling would not is derivation depth:
//! two levels per tread, against `Limits::max_depth`'s default of 256. A flight
//! past roughly 125 treads is a `DepthLimit` diagnostic rather than a hang,
//! which is the budget doing its job (§4) — and a caller who means to build one
//! raises the limit explicitly.
//!
//! # What a straight flight cannot do, and what would fix it
//!
//! A flight's rise is bounded by its run: `n = min(Y - head - 1,
//! (Z - 2*landing_run) / tread)` treads, so a shaft climbs at most about
//! `Z / tread` blocks. A tall tower over a small footprint therefore needs
//! several flights stacked, which needs a **switchback**: a second flight
//! climbing back the way the first came.
//!
//! That is *not* the blocker the vocabulary has been recording. The recorded
//! reason a switchback is unbuildable is that "a grammar orientation is a
//! permutation without reflection", and that is the right answer to the wrong
//! question. An orientation cannot mirror a piece — but a **rule body can be
//! written mirrored**, and for a stair that is one line: this rule peels its
//! treads off the local `Z`-max end (`[rel, abs]`, recursion first), and the
//! same rule written `[abs, rel]` peels them off `Z`-min and climbs the other
//! way. Two lanes side by side in `X`, one of each, joined at the top of the
//! first, is a dogleg that doubles the rise per unit of length and recurses for
//! as many flights as `Y` allows.
//!
//! Not built here — the round's obligation was an ascending route and its gate,
//! and a second run shape is a design decision, not a corollary. It is recorded
//! because "needs a mirroring orientation" is a **false blocker**, and a false
//! blocker in the file every future round reads is worth more than a missing
//! feature.
//!
//! # The gates
//!
//! Calibrated against the L family's, deliberately: the same `standable`
//! predicate and the same plain ±1-step `connected` walk, so the vocabulary has
//! one model of "can a body get there" and not two.
//!
//! `connected`'s edge relation is **symmetric** (a step of `dy` has a matching
//! step of `-dy`), so "walks up" and "walks down" are literally one claim under
//! it. Both are asserted anyway, and that is the point rather than a redundancy:
//! `drop_shaft`'s gate is `!connected(landing, spill)` and this one is
//! `connected(foot, head)` — the same predicate, the same direction of travel,
//! opposite verdicts. Anything that made one of them lie would make the other
//! lie too, which is what "calibrated against one model" buys.
//!
//! A walk gate alone would be vacuous on a **flat** corridor, which walks
//! perfectly in both directions, so the second gate measures the rise off the
//! declared anchors and pins it, and its control is `boulder_stair` — flat by
//! construction — read in the same box by the same code.
//!
//! `broken_step` is the teeth. It raises one tread by one extra course, at the
//! last climbing level (a guard on the run's own remaining `Z`, so no index is
//! needed there either): one riser becomes 2 and the next becomes 0. The shaft
//! still looks like a stair, everything below the break still walks, and the
//! head landing becomes unreachable — which is exactly how a stair fails in
//! practice, and exactly what a gate that only ever proves the shaft is not
//! demolished would miss.
//!
//! # Anchors
//!
//! * `anchor/stair-foot` — the low landing's floor centre, facing down-travel at
//!   the climb.
//! * `anchor/stair-head` — the high landing's floor centre.
//! * `anchor/stair-step-<i>` — every tread's own floor cell. **Numbered against
//!   travel**, as everything in this vocabulary is (`super`'s frame note): a
//!   split visits its pieces low-to-high while the recursion peels from the high
//!   end, so `stair-step-1` is the topmost tread and the last one the player
//!   meets. The gate reads them in index order and asserts the `Y` *decreases*
//!   by exactly one a step.
//!
//! Smallest region that expands: **`MIN_WIDTH` × (`head` + 1 + `MIN_STEPS`) ×
//! (2·`landing_run` + `MIN_STEPS`·`tread`)** — 3 × 7 × 12 at the defaults — and
//! at least as long as it is wide, since the frame turns length onto the longer
//! horizontal axis.

use crate::block::BlockState;
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, MarkAt, Program, Reorient};

use super::{
    abs, absp, all_of, alt_else, alt_when, any_of, call, cmp, dim, fill, int, marked, marked_each,
    par, rel, reoriented, split_exact, void,
};

/// The narrowest shaft: a wall, a one-cell lane, a wall.
pub const MIN_WIDTH: i64 = 3;

/// The fewest treads the rule will lay. Two would be a doorstep and one is a
/// kerb; a flight is refused below three, so `anchor/stair-head` is always at
/// least two blocks above `anchor/stair-foot` and the rise gate can never be
/// green on something that is not a climb.
pub const MIN_STEPS: i64 = 3;

/// The stair flight.
///
/// Parameters: `head` (headroom over every tread and landing), `tread` (cells of
/// run per block of rise — 1 is a ladder-steep stair, the default 2 a walkable
/// one), `landing_run` (cells of level floor at each end), `broken_step` — a
/// test knob, off by default, that raises the last tread by one extra course so
/// the both-ways gate can be shown failing on a stair with exactly one
/// unclimbable riser. Palette role: `rock` (the whole shell — walls, the mass
/// under the run, both landings).
pub fn stair_flight() -> Program {
    Program::new("stair_flight", "stair_flight")
        .param("head", 3)
        .param("tread", 2)
        .param("landing_run", 3)
        .param("broken_step", 0)
        .role("rock", BlockState::simple("stone"))
        // --- frame -----------------------------------------------------------
        .rule(
            "stair_flight",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("flight_plan"),
            ),
        )
        // One alternative and no `otherwise`: a box that cannot hold `MIN_STEPS`
        // treads is not a shallower stair, it is a refusal naming the rule. A
        // stair silently flattened to two steps would pass a walk gate and fail
        // the thing the rule is for.
        .rule_alts(
            "flight_plan",
            vec![alt_when(
                all_of(vec![
                    cmp(dim(DimRef::X), CmpOp::Ge, int(MIN_WIDTH)),
                    cmp(
                        dim(DimRef::Y),
                        CmpOp::Ge,
                        par("head")
                            .arith(ArithOp::Add, int(1))
                            .arith(ArithOp::Add, int(MIN_STEPS)),
                    ),
                    cmp(
                        dim(DimRef::Z),
                        CmpOp::Ge,
                        par("landing_run").arith(ArithOp::Mul, int(2)).arith(
                            ArithOp::Add,
                            par("tread").arith(ArithOp::Mul, int(MIN_STEPS)),
                        ),
                    ),
                    cmp(par("head"), CmpOp::Ge, int(2)),
                    cmp(par("tread"), CmpOp::Ge, int(1)),
                    cmp(par("landing_run"), CmpOp::Ge, int(1)),
                ]),
                split_exact(
                    Axis::X,
                    vec![abs(1), rel(1), abs(1)],
                    vec![fill("rock"), call("shaft_column"), fill("rock")],
                ),
            )],
        )
        // The lane between the two walls: the foot landing at the approach end
        // (local `Z`-max), everything else handed to the run. A split visits its
        // pieces low-to-high, so the landing is the *last* child.
        .rule(
            "shaft_column",
            split_exact(
                Axis::Z,
                vec![rel(1), absp("landing_run")],
                vec![call("run"), call("foot_landing")],
            ),
        )
        .rule(
            "foot_landing",
            split_exact(
                Axis::Y,
                vec![abs(1), rel(1)],
                vec![
                    fill("rock"),
                    marked("stair-foot", MarkAt::FloorCenter, void()),
                ],
            ),
        )
        // --- the run, which is one rule calling itself ------------------------
        // Each level lays one course of floor across everything it still owns
        // and hands the remainder to itself, one block higher and one tread
        // shorter. The "index" is the box: `dim(Y)` and `dim(Z)` shrink, the
        // guard reads them, and the recursion stops when either runs out. This
        // is `store_room`'s state-machine trick aimed at `Y` instead of at a
        // barrel row — see the module note.
        .rule_alts(
            "run",
            vec![
                alt_when(
                    all_of(vec![
                        cmp(
                            dim(DimRef::Z),
                            CmpOp::Ge,
                            par("tread").arith(ArithOp::Add, par("landing_run")),
                        ),
                        cmp(
                            dim(DimRef::Y),
                            CmpOp::Ge,
                            par("head").arith(ArithOp::Add, int(2)),
                        ),
                    ]),
                    split_exact(
                        Axis::Y,
                        vec![abs(1), rel(1)],
                        vec![fill("rock"), call("run_above")],
                    ),
                ),
                // Whatever is left when the climb stops: the high landing. Its
                // floor is the course its parent just laid, so it is level with
                // the last tread rather than one above it — a flight arrives on
                // its landing, it does not step up onto it.
                alt_else(marked("stair-head", MarkAt::FloorCenter, void())),
            ],
        )
        // The air over one level: the tread at the approach end, the rest of the
        // climb beyond it. Two alternatives, exact complements of each other, so
        // the choice is a decision and not a weighted draw (grammar.md §2).
        .rule_alts(
            "run_above",
            vec![
                alt_when(
                    any_of(vec![
                        cmp(par("broken_step"), CmpOp::Le, int(0)),
                        cmp(dim(DimRef::Z), CmpOp::Ge, last_climb()),
                    ]),
                    split_exact(
                        Axis::Z,
                        vec![rel(1), absp("tread")],
                        vec![call("run"), call("tread_air")],
                    ),
                ),
                alt_when(
                    all_of(vec![
                        cmp(par("broken_step"), CmpOp::Ge, int(1)),
                        cmp(dim(DimRef::Z), CmpOp::Lt, last_climb()),
                    ]),
                    split_exact(
                        Axis::Z,
                        vec![rel(1), absp("tread")],
                        vec![call("run"), call("tread_raised")],
                    ),
                ),
            ],
        )
        .rule(
            "tread_air",
            marked_each("stair-step", MarkAt::FloorCenter, void()),
        )
        // The teeth: one extra course under one tread. The riser below it
        // becomes 2 and the riser above it becomes 0, so the stair still reads
        // as a stair and still walks up to the break — and stops there.
        .rule(
            "tread_raised",
            split_exact(
                Axis::Y,
                vec![abs(1), rel(1)],
                vec![
                    fill("rock"),
                    marked_each("stair-step", MarkAt::FloorCenter, void()),
                ],
            ),
        )
}

/// The remaining run at which the *next* level would stop climbing — i.e. the
/// last level that lays a tread. `broken_step` uses it to pick exactly one
/// tread out of the recursion without an index: the box knows which level it is
/// because it knows how much run is left.
fn last_climb() -> crate::ir::Expr {
    par("tread")
        .arith(ArithOp::Mul, int(2))
        .arith(ArithOp::Add, par("landing_run"))
}
