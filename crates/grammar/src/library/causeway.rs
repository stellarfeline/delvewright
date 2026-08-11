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
//!  one cross-section, everywhere along the piece (`section`):
//!  local X:  wall | flank (rel) | spine (abs 1) | flank (rel) | wall
//!            ward:  flood                berm            flood
//!            post:  plinth               the post's own column
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
//! # One cross-section, because the spine's `X` is a fact about the piece
//!
//! Every slice of the piece — the ward and both `Z`-slices of the post — is
//! laid out by [`section`]: wall, flank, a **1-wide spine**, flank, wall. Only
//! the bodies differ (flood/berm in the ward, plinth/post column at the
//! station). The spine is therefore at one `X` by construction rather than by
//! two arithmetics agreeing.
//!
//! They did not agree. The post used to be one full-width column marking its
//! own centre, `(X-2-1)/2` from the interior's edge, while the berm sits
//! `ceil((X-3)/2)` from it — the same cell only when `X` is odd. At every even
//! width the guard stood over the **flood**, one cell off its own causeway, and
//! the sightline gate below went from 0 blind cells to 20 of 22. Both fixtures
//! happened to be odd, so the gate was green and bound to a geometry the rule
//! does not otherwise promise. Sharing the section is what closes that: a rule
//! whose gate is "the post commands the spine" cannot have the spine's position
//! be a coincidence.
//!
//! # `berm_gate` — a post that is a gatehouse rather than a plug
//!
//! By default the post's plinth is solid from the ward floor to its own floor,
//! and the piece is a **terminus**: its far face carries nothing standable, so
//! nothing can be chained past it (a grammar orientation is a permutation
//! without reflection, so turning the piece round does not move the post to the
//! other end either).
//!
//! `berm_gate = 1` carries the spine's own column *through* the station at berm
//! height: the berm continues under the cantilever and tunnels through the
//! plinth, and the course that roofs it **is** the post's floor. The post is
//! untouched by construction — same floor, same headroom, same
//! `anchor/elite`, and the lane's whole clearance lies below `rise + 1`, which
//! is the lowest a sightline from the post ever descends. What changes is only
//! that the piece stops being a terminus.
//!
//! It needs [`MIN_GATE_RISE`] of `tower_rise`: two cells of clearance and the
//! course of floor over them. A shorter post is refused rather than built with
//! a crawlspace under it.
//!
//! Configured, not campaign-shaped: "the raised thing across the route is a
//! gatehouse you pass under, not a plug" is a mechanism. What it is a gatehouse
//! *for* — a drowned ward's keeper, a toll bridge, a checkpoint — is the
//! caller's palette and the caller's anchor.
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
//!    `watch_bay` uses, **at every width** — odd and even — and with the lane
//!    open as well as shut. Teeth: `obstruct = 1` stands one pillar in the
//!    causeway's own column, high enough that it does not touch the two cells
//!    `standable` needs clear, and the same check must find at least one
//!    causeway cell it can no longer see — while the causeway stays walkable
//!    end to end, so what is caught is blindness, not impassability.
//! 3. **`berm_gate` opens a lane and nothing else.** With it on, the piece's
//!    entry face reaches its exit face under the walker of gate 1; the post's
//!    own floor stays unreachable from that lane (it is a post, not a landing);
//!    and gate 2 still holds. With it off, the exit face is unreachable — the
//!    terminus the rule is by default.
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
//! 3` long** — and at least as long as it is wide. With `berm_gate` on,
//! `tower_rise` is additionally at least [`MIN_GATE_RISE`].

use crate::block::BlockState;
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, Node, Program, Reorient};

use super::{
    abs, abse, absp, all_of, alt_when, any_of, at_offset, call, cmp, dim, fill, int, marked, par,
    rel, reoriented, split, split_exact, void,
};

/// The shortest post `berm_gate` will tunnel: the two cells `standable`
/// requires clear over the berm, plus the one solid course that is the lane's
/// ceiling and the post's own floor at the same time.
pub const MIN_GATE_RISE: i64 = 3;

/// The causeway.
///
/// Parameters: `rise` (blocks the causeway berm stands above the ward floor),
/// `head` (interior headroom, both the causeway and the guard station),
/// `tower_rise` (blocks the guard station's floor sits above causeway height),
/// `guard_len` (the guard station zone's own length), `berm_gate` — off by
/// default: carry the berm through the guard station at berm height, so the
/// post is a gatehouse the route passes under instead of the terminus it
/// otherwise is — and `obstruct`, a test knob, off by default, that stands one
/// pillar in the causeway's line of sight so the sightline gate can be shown to
/// fail when it should. Palette roles: `stone` (the shell and berm), `water`
/// (the flood).
pub fn causeway() -> Program {
    Program::new("causeway", "ward_plan")
        .param("rise", 3)
        .param("head", 3)
        .param("tower_rise", 4)
        .param("guard_len", 2)
        .param("berm_gate", 0)
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
                    // The lane is an obligation the knob adds, not a different
                    // rule: either it is off, or the post is tall enough to be
                    // tunnelled without eating the floor the guard stands on.
                    any_of(vec![
                        cmp(par("berm_gate"), CmpOp::Le, int(0)),
                        cmp(par("tower_rise"), CmpOp::Ge, int(MIN_GATE_RISE)),
                    ]),
                ]),
                split_exact(
                    Axis::Z,
                    vec![absp("guard_len"), rel(1)],
                    vec![call("guard_station"), call("flooded_ward")],
                ),
            )],
        )
        // --- the flooded ward (near end, low elevation) -------------------------
        .rule("flooded_ward", section("flood_column", "causeway_column"))
        // Floor, then water almost to the roof — not a shallow pool. Total
        // non-roof height matches `causeway_column`/`post_flank` so all three
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
        // SAME headroom one cell further toward the ward with **no** pillar
        // under it. A post whose own support pillar stands between the guard
        // and the causeway blinds the guard on its own nearest cells — a
        // downward sightline from `tower_rise` up has to cross the pillar's own
        // height while still over the pillar's own footprint. The cantilever is
        // a plain corbel (the same move `rafter_hall` uses to keep a perch's
        // sightline clear of its own truss): the roof keeps going, the mass
        // underneath does not.
        .rule(
            "guard_station",
            split(
                Axis::Z,
                vec![abse(par("guard_len").arith(ArithOp::Sub, int(1))), abs(1)],
                vec![call("guard_support"), call("guard_cantilever")],
            ),
        )
        .rule("guard_support", section("post_flank", "post_column"))
        .rule(
            "guard_cantilever",
            section("cantilever_flank", "cantilever_column"),
        )
        // The plinth either side of the spine: solid to the post's floor, the
        // post's own room above it, roof.
        .rule("post_flank", plinth_column(fill("stone"), void()))
        // The spine of the station, and the whole of what `berm_gate` changes.
        // Both alternatives are written out here rather than delegated to two
        // sub-rules so that `anchor/elite` keeps one declaring rule whichever is
        // taken — the name a campaign reads back is a contract, and it should
        // not depend on a knob.
        .rule_alts(
            "post_column",
            vec![
                alt_when(
                    cmp(par("berm_gate"), CmpOp::Le, int(0)),
                    plinth_column(fill("stone"), elite_room()),
                ),
                // Berm, the lane's clearance, then ONE course of stone — which
                // is the lane's ceiling and the post's floor at the same time,
                // so the post above it is bit-for-bit the post above the solid
                // plinth: `rise + (tower_rise - 1) + 1 == rise + tower_rise`.
                alt_when(
                    cmp(par("berm_gate"), CmpOp::Ge, int(1)),
                    split(
                        Axis::Y,
                        vec![
                            absp("rise"),
                            abse(par("tower_rise").arith(ArithOp::Sub, int(1))),
                            abs(1),
                            absp("head"),
                            rel(1),
                        ],
                        vec![
                            fill("stone"),
                            void(),
                            fill("stone"),
                            elite_room(),
                            fill("stone"),
                        ],
                    ),
                ),
            ],
        )
        // The corbel: the same courses as `post_flank`, with nothing in them.
        .rule("cantilever_flank", plinth_column(void(), void()))
        // ...and its spine, which is where the lane enters the station. Without
        // the gate it is corbel like its flanks; with it, the berm simply keeps
        // going, and stops `rise` blocks below anything the guard's own
        // sightline passes through.
        .rule_alts(
            "cantilever_column",
            vec![
                alt_when(
                    cmp(par("berm_gate"), CmpOp::Le, int(0)),
                    plinth_column(void(), void()),
                ),
                alt_when(
                    cmp(par("berm_gate"), CmpOp::Ge, int(1)),
                    split(
                        Axis::Y,
                        vec![
                            absp("rise"),
                            abse(par("tower_rise").arith(ArithOp::Add, par("head"))),
                            rel(1),
                        ],
                        vec![fill("stone"), void(), fill("stone")],
                    ),
                ),
            ],
        )
}

/// The piece's one cross-section: outer wall, flank, the route's own 1-wide
/// **spine**, flank, outer wall.
///
/// Every slice of the causeway is this shape and only the bodies differ, which
/// is what puts the berm and the post's own column at the same `X` by
/// construction — see the module note on why that is not a tidiness argument.
///
/// `split_exact`, not `split`: the two flanks are relative and truncation would
/// leave the far one short, i.e. a strip of unwritten (air) cells running the
/// length of a wall that is supposed to be solid.
fn section(flank: &str, spine: &str) -> Node {
    split_exact(
        Axis::X,
        vec![abs(1), rel(1), abs(1), rel(1), abs(1)],
        vec![
            fill("stone"),
            call(flank),
            call(spine),
            call(flank),
            fill("stone"),
        ],
    )
}

/// The guard station's courses: everything below the post's floor (`base`),
/// the post's own room, roof. `base` is what tells a plinth from a corbel, and
/// `room` is what tells the column carrying `anchor/elite` from the mass beside
/// it — the room itself is the same air either way.
fn plinth_column(base: Node, room: Node) -> Node {
    split(
        Axis::Y,
        vec![
            abse(par("rise").arith(ArithOp::Add, par("tower_rise"))),
            absp("head"),
            rel(1),
        ],
        vec![base, room, fill("stone")],
    )
}

/// The post's own room, carrying `anchor/elite` at the centre of whatever
/// column it is given — the spine, today one cell wide.
fn elite_room() -> Node {
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
    )
}
