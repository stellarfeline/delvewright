//! The causeway — a flooded ward with a raised 1-wide spline through it, and a
//! guard post that oversees the whole crossing (W4 entry T, drowned-bell
//! remake step 2).
//!
//! **Original Delvewright content**, not a port: licence `original`.
//!
//! ```text
//!  local Z:  0 ...... guard_station (abs) ...... flooded_ward (rel) ...... Z-1
//!            far, elevated                        near, approach
//!                                       travel: local Z-max -> Z-min
//!
//!  local X (flooded_ward):  wall | flood (rel) | causeway (abs 1) | flood (rel) | wall
//! ```
//!
//! `flooded_ward`'s floor rule is deliberately extreme: the flood zones are
//! **water from the floor almost to the ceiling**, not a shallow pool with a
//! walkable rim — the ward's whole claim is that off the spline there is
//! nothing to stand on, and a body-height air pocket above the water would
//! quietly make that false. The causeway itself is a solid berm a body walks
//! on top of, `rise` blocks above the ward floor, with the same headroom as
//! every other room in the vocabulary.
//!
//! `guard_station` sits at the far end, and it is deliberately **not flush**
//! with the causeway: its floor is `tower_rise` blocks above causeway height.
//! A flush guard post cannot be obstructed without also breaking the
//! causeway's own walkability — eye height (1.62) over a same-height watch
//! cell and target mass (1.0) over a same-height causeway cell both fall
//! inside the exact two-cell band `standable` requires clear, so nothing can
//! block that sightline without also sealing the crossing. Elevating the post
//! opens a real sightline geometry to obstruct, the same reason
//! `rafter_hall`'s perches are corbels and not a floor: a post that can be
//! stood *under* is not a post, and here a sightline that cannot be tested is
//! not a gate. Precedent: `rafter_hall`'s perches are similarly not reachable
//! from the nave — "not a mezzanine" applies here as "not a landing".
//!
//! # The gates
//!
//! 1. **The causeway is standable end to end; stepping off it is not.** Every
//!    causeway cell is standable and connects the approach end to the guard
//!    station end (`connected`, the same technique `cliff_path` uses); every
//!    flood cell is asserted directly not standable — its foot cell is water,
//!    which is not air, so it fails `passable` outright.
//! 2. **The guard station commands the causeway.** `anchor/elite` sees every
//!    standable causeway cell, walked with the same Amanatides–Woo traversal
//!    `watch_bay` uses. Teeth: `obstruct = 1` stands one pillar in the
//!    causeway's own column, high enough that it does not touch the two cells
//!    `standable` needs clear, and the same check must find at least one
//!    causeway cell it can no longer see — while the causeway stays walkable
//!    end to end, so what is caught is blindness, not impassability.
//!
//! # Anchors
//!
//! * `anchor/causeway-head` — the causeway's own near end (floor centre at the
//!   approach), for a campaign to hang an entry telegraph on.
//! * `anchor/elite` — the guard station's floor centre, where a campaign
//!   places the actor this post is for.
//!
//! Smallest region that expands: **5 wide** (wall, flood, causeway, flood,
//! wall, each at least 1), **`rise + tower_rise + head` tall**, **`guard_len +
//! 3` long** — and at least as long as it is wide.

use crate::block::BlockState;
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, Program, Reorient};

use super::{
    abs, abse, absp, all_of, alt_when, at_offset, call, cmp, dim, fill, int, marked, par, rel,
    reoriented, split, split_exact, void,
};

/// The causeway.
///
/// Parameters: `rise` (blocks the causeway berm stands above the ward floor),
/// `head` (interior headroom, both the causeway and the guard station),
/// `tower_rise` (blocks the guard station's floor sits above causeway height),
/// `guard_len` (the guard station zone's own length), `obstruct` — a test
/// knob, off by default, that stands one pillar in the causeway's line of
/// sight so the sightline gate can be shown to fail when it should. Palette
/// roles: `stone` (the shell and berm), `water` (the flood).
pub fn causeway() -> Program {
    Program::new("causeway", "ward_plan")
        .param("rise", 3)
        .param("head", 3)
        .param("tower_rise", 4)
        .param("guard_len", 2)
        .param("obstruct", 0)
        .role("stone", BlockState::simple("stone"))
        .role("water", BlockState::simple("water"))
        // --- frame -----------------------------------------------------------
        .rule(
            "ward_plan",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("ward_alts"),
            ),
        )
        // One alternative, no `otherwise`: a ward too small to hold a real berm
        // and a real post is not a smaller causeway, it is not one at all.
        .rule_alts(
            "ward_alts",
            vec![alt_when(
                all_of(vec![
                    cmp(dim(DimRef::X), CmpOp::Ge, int(5)),
                    cmp(
                        dim(DimRef::Z),
                        CmpOp::Ge,
                        par("guard_len").arith(ArithOp::Add, int(3)),
                    ),
                    // At least one cell of pillared support behind the
                    // cantilever's own single open cell.
                    cmp(par("guard_len"), CmpOp::Ge, int(2)),
                    cmp(par("rise"), CmpOp::Ge, int(2)),
                    cmp(par("head"), CmpOp::Ge, int(2)),
                    cmp(par("tower_rise"), CmpOp::Ge, int(1)),
                    cmp(
                        dim(DimRef::Y),
                        CmpOp::Ge,
                        par("rise")
                            .arith(ArithOp::Add, par("tower_rise"))
                            .arith(ArithOp::Add, par("head")),
                    ),
                ]),
                split_exact(
                    Axis::Z,
                    vec![absp("guard_len"), rel(1)],
                    vec![call("guard_station"), call("flooded_ward")],
                ),
            )],
        )
        // --- the flooded ward (near end, low elevation) -------------------------
        .rule(
            "flooded_ward",
            split_exact(
                Axis::X,
                vec![abs(1), rel(1), abs(1), rel(1), abs(1)],
                vec![
                    fill("stone"),
                    call("flood_column"),
                    call("causeway_column"),
                    call("flood_column"),
                    fill("stone"),
                ],
            ),
        )
        // Floor, then water almost to the roof — not a shallow pool. Total
        // non-roof height matches `causeway_column`/`post_column` so all three
        // share one flat ceiling: `1 + (rise + tower_rise + head - 1) ==
        // rise + tower_rise + head`. Reaching guard-station height, not just
        // causeway height, is what lets the elevated post see over the
        // boundary into the ward at all — a lower ceiling here would block its
        // own sightline before `obstruct` ever gets a chance to.
        .rule(
            "flood_column",
            split(
                Axis::Y,
                vec![
                    abs(1),
                    abse(
                        par("rise")
                            .arith(ArithOp::Add, par("tower_rise"))
                            .arith(ArithOp::Add, par("head"))
                            .arith(ArithOp::Sub, int(1)),
                    ),
                    rel(1),
                ],
                vec![fill("stone"), fill("water"), fill("stone")],
            ),
        )
        // The berm, then an interior reaching the same height as the guard
        // station's own floor — see the note on `flood_column`.
        .rule(
            "causeway_column",
            split(
                Axis::Y,
                vec![
                    absp("rise"),
                    abse(par("tower_rise").arith(ArithOp::Add, par("head"))),
                    rel(1),
                ],
                vec![
                    fill("stone"),
                    marked(
                        "causeway-head",
                        at_offset(int(0), int(0), dim(DimRef::Z).arith(ArithOp::Sub, int(1))),
                        call("open_or_obstructed"),
                    ),
                    fill("stone"),
                ],
            ),
        )
        .rule_alts(
            "open_or_obstructed",
            vec![
                alt_when(cmp(par("obstruct"), CmpOp::Le, int(0)), void()),
                // One solid cell, level with the guard station's own floor —
                // well above the two cells `standable` needs clear at the
                // causeway's foot, and directly in the path any sightline down
                // from the post has to cross.
                alt_when(
                    cmp(par("obstruct"), CmpOp::Ge, int(1)),
                    split(
                        Axis::Y,
                        vec![absp("tower_rise"), abs(1), rel(1)],
                        vec![void(), fill("stone"), void()],
                    ),
                ),
            ],
        )
        // --- the guard station (far end, elevated) -------------------------------
        // Two Z-slices, not one: `guard_support` carries the post's own pillar
        // (and `anchor/elite`, at its near edge), `guard_cantilever` is the
        // SAME floor and headroom one cell further toward the ward with **no**
        // pillar under it. A post whose own support pillar stands between the
        // guard and the causeway blinds the guard on its own nearest cells —
        // a downward sightline from `tower_rise` up has to cross the pillar's
        // own height while still over the pillar's own footprint. The
        // cantilever is a plain corbel (the same move `rafter_hall` uses to
        // keep a perch's sightline clear of its own truss): the floor keeps
        // going, the mass underneath does not.
        .rule(
            "guard_station",
            split(
                Axis::Z,
                vec![abse(par("guard_len").arith(ArithOp::Sub, int(1))), abs(1)],
                vec![call("guard_support"), call("guard_cantilever")],
            ),
        )
        .rule(
            "guard_support",
            split(
                Axis::X,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("stone"), call("post_column"), fill("stone")],
            ),
        )
        .rule(
            "guard_cantilever",
            split(
                Axis::X,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("stone"), call("post_column_open"), fill("stone")],
            ),
        )
        .rule(
            "post_column",
            split(
                Axis::Y,
                vec![
                    abse(par("rise").arith(ArithOp::Add, par("tower_rise"))),
                    absp("head"),
                    rel(1),
                ],
                vec![
                    fill("stone"),
                    marked(
                        "elite",
                        at_offset(
                            dim(DimRef::X)
                                .arith(ArithOp::Sub, int(1))
                                .arith(ArithOp::Div, int(2)),
                            int(0),
                            int(0),
                        ),
                        void(),
                    ),
                    fill("stone"),
                ],
            ),
        )
        .rule(
            "post_column_open",
            split(
                Axis::Y,
                vec![
                    abse(par("rise").arith(ArithOp::Add, par("tower_rise"))),
                    absp("head"),
                    rel(1),
                ],
                vec![void(), void(), fill("stone")],
            ),
        )
}
