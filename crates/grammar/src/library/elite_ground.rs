//! Elite ground — an open arena around a single anchor, with room to go around
//! it (W4 entry E, drowned-bell remake step 2).
//!
//! **Original Delvewright content**, not a port: licence `original`. Dossier
//! §3.3's bypass-legibility charge against a fog-gate arena is that the fight
//! is compulsory the moment you can see it; the answer this rule builds is
//! geometric, not a rule the campaign has to remember — there is simply floor
//! on both sides of the engagement circle, all the way from the entry to the
//! exit.
//!
//! ```text
//!  local X:  wall | west flank | ....... circle (diam) ....... | east flank | wall
//!  local Z:  entry run (approach) | ....... circle (diam) ....... | exit run (approach)
//!                                  travel: local Z-max -> Z-min
//! ```
//!
//! One open room, uniformly floored — no wall anywhere inside it. The
//! "engagement circle" is not built as a separate piece; it is the square of
//! cells within Chebyshev distance `radius` of `anchor/elite`, which the
//! `radius`/margin/approach arithmetic places exactly in the middle of an
//! otherwise perfectly ordinary floor. That is the geometric form of "no
//! fog-gate motif": there is no threshold, no doorway and no separating wall
//! between the circle and the floor around it for a fog-gate rule to have
//! occupied in the first place.
//!
//! # The gates
//!
//! 1. **The circle is open ground, at least 9×9.** `radius` is guarded at
//!    `>= 4` — a smaller circle is refused, not quietly built (no `otherwise`)
//!    — and every cell within Chebyshev distance `radius` of `anchor/elite` is
//!    asserted standable: 81 cells at the default.
//! 2. **Two proven flank lanes.** The west band (`X` strictly left of the
//!    circle) and the east band (strictly right of it) each connect the entry
//!    end to the exit end by `connected`, the same technique `cliff_path`
//!    uses — two *counted* routes, not an eyeballed "there is space on the
//!    sides". Teeth: `seal_flank` walls off the west band, the east band, or
//!    both across the circle's own length — exactly the shape a fog-gate
//!    motif would take, a wall between the open floor and the bypass — and the
//!    route count the same check reports drops from 2 to 1 or 0.
//!
//! # Anchors
//!
//! * `anchor/elite` — the circle's centre, floor height. Where a campaign
//!   places the actor this ground is built for.
//!
//! Smallest region that expands: **both horizontal extents ≥ `2*radius + 1 +
//! 2*flank_margin + 2`**, `head + 2` tall. The width requirement is the
//! larger of the two the rule actually checks (flank margins dwarf the
//! approach runs at the defaults), so it is also the true minimum on `Z` —
//! the same shape `castle`'s "both horizontal extents ≥ `2*large_tower + 2`"
//! states for the identical reason. At `radius`'s enforced floor of 4, that is
//! already a 19-a-side box holding a 9×9 open circle.

use crate::block::BlockState;
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, MarkAt, Program, Reorient};

use super::{
    abs, abse, absp, all_of, alt_when, call, cmp, dim, fill, int, marked, modulo_is, par, rel,
    reoriented, split, split_exact, void,
};

/// The engagement circle's enforced minimum radius — Chebyshev distance from
/// `anchor/elite`, so the floor it guarantees is `2*4 + 1 = 9` cells across.
pub const MIN_RADIUS: i64 = 4;

/// Elite ground.
///
/// Parameters: `radius` (Chebyshev radius of the open circle, floor 4),
/// `flank_margin` (floor cells each flank band carries beyond the circle),
/// `approach` (the entry/exit run's own length, each end), `head` (interior
/// headroom), `seal_flank` — a test knob, `0` by default: `1` walls off the
/// west band, `2` the east band, `3` both, across the circle's own length, so
/// the flank-route count can be shown to drop when it should. Palette role:
/// `stone`.
pub fn elite_ground() -> Program {
    Program::new("elite_ground", "arena_plan")
        .param("radius", MIN_RADIUS)
        .param("flank_margin", 4)
        .param("approach", 4)
        .param("head", 3)
        .param("seal_flank", 0)
        .role("stone", BlockState::simple("stone_bricks"))
        // --- frame -----------------------------------------------------------
        .rule(
            "arena_plan",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("arena_alts"),
            ),
        )
        // One alternative, no `otherwise`: a circle under the entry's own
        // stated minimum is not a smaller arena, it is not one at all.
        .rule_alts(
            "arena_alts",
            vec![alt_when(
                all_of(vec![
                    cmp(par("radius"), CmpOp::Ge, int(MIN_RADIUS)),
                    cmp(par("flank_margin"), CmpOp::Ge, int(1)),
                    cmp(par("approach"), CmpOp::Ge, int(1)),
                    cmp(par("head"), CmpOp::Ge, int(2)),
                    cmp(
                        dim(DimRef::X),
                        CmpOp::Ge,
                        diameter().arith(
                            ArithOp::Add,
                            par("flank_margin")
                                .arith(ArithOp::Mul, int(2))
                                .arith(ArithOp::Add, int(2)),
                        ),
                    ),
                    cmp(
                        dim(DimRef::Z),
                        CmpOp::Ge,
                        diameter().arith(ArithOp::Add, par("approach").arith(ArithOp::Mul, int(2))),
                    ),
                    cmp(
                        dim(DimRef::Y),
                        CmpOp::Ge,
                        par("head").arith(ArithOp::Add, int(2)),
                    ),
                ]),
                // The exit run is the one relative piece: any `Z` beyond the
                // documented minimum lengthens it, never the circle or the
                // entry run, so `radius` and `approach` stay exact controls.
                split(
                    Axis::Z,
                    vec![rel(1), abse(diameter()), absp("approach")],
                    vec![
                        call("floor_column"),
                        call("circle_band"),
                        call("floor_column"),
                    ],
                ),
            )],
        )
        // The circle's own `Z` band: same `X` layout, so the flank walls (when
        // `seal_flank` is on) only ever occupy this band's length — the
        // approach runs on either end are never touched by the knob.
        .rule(
            "circle_band",
            split_exact(
                Axis::X,
                vec![abs(1), rel(1), abse(diameter()), rel(1), abs(1)],
                vec![
                    fill("stone"),
                    call("west_flank"),
                    call("elite_column"),
                    call("east_flank"),
                    fill("stone"),
                ],
            ),
        )
        // `seal_flank` is a 2-bit knob read arithmetically rather than via a
        // logical OR (the IR has one; this reads no worse and needs no extra
        // import): bit 0 (`% 2`) is "west sealed", bit 1 (`/ 2 % 2`) is "east
        // sealed" — `1`, `2` and `3` are west, east and both.
        .rule_alts(
            "west_flank",
            vec![
                alt_when(modulo_is(par("seal_flank"), 2, 0), call("floor_column")),
                alt_when(modulo_is(par("seal_flank"), 2, 1), fill("stone")),
            ],
        )
        .rule_alts(
            "east_flank",
            vec![
                alt_when(
                    modulo_is(par("seal_flank").arith(ArithOp::Div, int(2)), 2, 0),
                    call("floor_column"),
                ),
                alt_when(
                    modulo_is(par("seal_flank").arith(ArithOp::Div, int(2)), 2, 1),
                    fill("stone"),
                ),
            ],
        )
        // Plain open floor: the shell every band and every run shares. No wall
        // divides one from the next — that absence *is* gate 2.
        .rule(
            "floor_column",
            split(
                Axis::Y,
                vec![abs(1), absp("head"), rel(1)],
                vec![fill("stone"), void(), fill("stone")],
            ),
        )
        .rule(
            "elite_column",
            split(
                Axis::Y,
                vec![abs(1), absp("head"), rel(1)],
                vec![
                    fill("stone"),
                    marked("elite", MarkAt::FloorCenter, void()),
                    fill("stone"),
                ],
            ),
        )
}

/// `2*radius + 1` — the circle's own width and length.
fn diameter() -> crate::ir::Expr {
    par("radius")
        .arith(ArithOp::Mul, int(2))
        .arith(ArithOp::Add, int(1))
}
