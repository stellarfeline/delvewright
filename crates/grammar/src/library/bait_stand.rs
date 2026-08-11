//! The bait stand — a thing worth taking, with the body that wants you to take
//! it standing over it in the same view (§4 entry **B**).
//!
//! **Original Delvewright content**, not a port: licence `original`. The
//! catalogue is explicit about which of the dossier's three bait variants this
//! is: **variant 1 only** — lure and ambusher *co-located and both legible*.
//! Variant 3, where taking the bait springs something somewhere else, is banned
//! outright as resented (`docs/notes/souls-design-language.md` §4.2), and the
//! geometry here cannot express it: the rule's whole gate is that the two are in
//! one frame.
//!
//! ```text
//!  local X:  0     1 .. bracket   +1        rest        X-1
//!           wall   the beam       STAND     open room   wall
//!
//!  local Y (the stand's own column):
//!    perch_rise + 1 ..   open air
//!    perch_rise          PERCH        <- anchor/bait-perch, a body stands here
//!    perch_rise - 1      timber       <- the beam, carried in from the side wall
//!    2 .. perch_rise-2   open air     <- what the lure is displayed in
//!    1                   PEDESTAL     <- anchor/bait
//!    0                   floor
//!
//!  along local Z:  [ room ][ perch slice ][ valance slice ][ room ]
//!                                                            travel: Z-max -> Z-min
//! ```
//!
//! # The mechanism, stated without the fiction
//!
//! A **lure** is a declared cell a campaign puts something desirable on, and a
//! **watcher** is a standable cell above it. What makes the pair a mechanism
//! rather than two anchors is the gate between them: *wherever the lure can be
//! seen from, the watcher can be seen too.* Any game with a temptation and a
//! guard wants that — a treasure and its sentry, a terminal and its camera, a
//! chest and its dog. Nothing here knows what is on the pedestal.
//!
//! # Why the beam comes in from the side wall
//!
//! The perch has to stand on something, and what it stands on must not be what
//! hides it. [`super::rafter_hall`] worked this out for a whole truss: an eye on
//! the floor is *below* the beam plane and a perch is *above* it, so a ray from
//! the approach crosses that plane somewhere between the two, and a beam lying
//! in the crossing hides the very body it carries. The same reasoning picks the
//! form here: the beam is a **corbel across local `X`**, occupying only the
//! perch's own `Z` slice, so a ray travelling down the lane crosses the beam
//! plane at a `Z` the beam does not occupy. Fairness is bought by the form, not
//! by a lucky box size.
//!
//! The `canopy` knob puts a valance across the slice in front of the perch — the
//! one cheap way to hide the watcher while leaving the lure in plain sight, which
//! is exactly the defect variant 1 exists to forbid, kept expressible so the gate
//! can be watched catching it.
//!
//! # The gates (`tests/staging.rs`)
//!
//! 1. **The watcher stands over the lure** — same local column, `anchor/bait`
//!    below `anchor/bait-perch`, the perch standable and the pedestal solid.
//!    Bound over three box sizes, because a motif that only lines up at one width
//!    is a coincidence.
//! 2. **Wherever the lure is visible, so is the watcher** — every standable cell
//!    of the approach that can see `anchor/bait` is walked again for
//!    `anchor/bait-perch`, with the same sightline the compiler's `DW0388` uses.
//!    Teeth: `canopy = 1`, which leaves the lure's count untouched and collapses
//!    the watcher's.
//! 3. **The room is still a chain segment** — standable end to end, so a gallery
//!    in a zone's piece run does not sever it. Red: the refusal an undersized box
//!    gets.
//!
//! # Anchors
//!
//! * `anchor/bait` — the pedestal's own top block, not the air over it. A prop
//!   anchor names the prop's block, the same call [`super::store_room`]'s `tell`
//!   makes for its barrel.
//! * `anchor/bait-perch` — the standable cell on the corbel directly above it.
//!
//! Both take the derived facing, down-travel: the same one [`super::rafter_hall`]
//! pays for its perches, and for the same reason — a rule cannot ask for a facing
//! along its own local `+Z` (`docs/reference/grammar.md` §7). A watcher on this
//! perch therefore faces the ground beyond the pedestal rather than the door the
//! player comes in by; the campaign that binds an actor here can override.
//!
//! Smallest region that expands: **[`MIN_WIDTH`] wide, `head + 2` tall, 4 long**
//! — and at least as long as it is wide, which the frame guarantees.

use crate::block::BlockState;
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, MarkAt, Program, Reorient};

use super::{
    abs, abse, absp, all_of, alt_when, call, cmp, dim, fill, int, marked, par, rel, reoriented,
    split, split_exact, void,
};

/// The narrowest box the rule will stand a pedestal in: side wall, one cell of
/// corbel, the stand's own column, a cell of room beside it, and the far wall.
pub const MIN_WIDTH: i64 = 5;

/// The lowest a perch may be hung. The pedestal's block takes local `Y` 1 and
/// what stands on it takes 2, so a beam at `perch_rise - 1` has to clear both.
pub const MIN_RISE: i64 = 4;

/// The bait stand.
///
/// Parameters: `head` (room headroom, which must clear the perch by a cell),
/// `perch_rise` (how far over the floor the watcher stands, at least
/// [`MIN_RISE`]), `bracket` (cells of corbel between the side wall and the
/// stand's column), and `canopy` — a test knob, off by default, that hangs a
/// valance in front of the perch so the co-visibility gate can be shown to fail
/// when it should. Palette roles: `stone` (the shell), `timber` (the corbel),
/// `pedestal` (the block the lure sits on — its own role so a campaign can make
/// it read as a display from across the room).
pub fn bait_stand() -> Program {
    Program::new("bait_stand", "bait_stand")
        .param("head", 5)
        .param("perch_rise", 4)
        .param("bracket", 1)
        .param("canopy", 0)
        .role("stone", BlockState::simple("stone_bricks"))
        .role("timber", BlockState::simple("dark_oak_wood"))
        .role("pedestal", BlockState::simple("chiseled_stone_bricks"))
        // --- frame -----------------------------------------------------------
        .rule(
            "bait_stand",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("stand_plan"),
            ),
        )
        // One alternative, no `otherwise`. Every clause is load-bearing for one
        // of the two gates: a perch with no air over it is not standable, a
        // pedestal with no air over it displays nothing, and a box too narrow to
        // carry a corbel in from the wall has nowhere to hang the watcher. Each
        // failure is a refusal naming the rule, never a stand that quietly is not
        // one.
        .rule_alts(
            "stand_plan",
            vec![alt_when(
                all_of(vec![
                    cmp(
                        dim(DimRef::X),
                        CmpOp::Ge,
                        par("bracket").arith(ArithOp::Add, int(MIN_WIDTH - 1)),
                    ),
                    cmp(dim(DimRef::Z), CmpOp::Ge, int(4)),
                    cmp(
                        dim(DimRef::Y),
                        CmpOp::Ge,
                        par("head").arith(ArithOp::Add, int(2)),
                    ),
                    cmp(par("bracket"), CmpOp::Ge, int(1)),
                    cmp(par("perch_rise"), CmpOp::Ge, int(MIN_RISE)),
                    cmp(
                        par("head"),
                        CmpOp::Ge,
                        par("perch_rise").arith(ArithOp::Add, int(1)),
                    ),
                ]),
                // Low `Z` to high `Z` is the reverse of travel: the valance slice
                // is declared after the perch slice, so it stands between the
                // perch and the approach the player reads it from.
                split_exact(
                    Axis::Z,
                    vec![rel(1), abs(1), abs(1), rel(1)],
                    vec![
                        call("room_slab"),
                        call("perch_slice"),
                        call("valance_slice"),
                        call("room_slab"),
                    ],
                ),
            )],
        )
        // --- plain room --------------------------------------------------------
        .rule(
            "room_slab",
            split(
                Axis::X,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("stone"), call("room_column"), fill("stone")],
            ),
        )
        .rule(
            "room_column",
            split(
                Axis::Y,
                vec![abs(1), absp("head"), rel(1)],
                vec![fill("stone"), void(), fill("stone")],
            ),
        )
        // --- the slice that carries the stand ----------------------------------
        // Wall, the corbel reaching in, the stand's own one-wide column, the rest
        // of the room, wall. The stand is its own piece of the split so that the
        // pedestal and the perch are *declared* in one column rather than
        // computed from two offsets that could drift apart.
        .rule(
            "perch_slice",
            split(
                Axis::X,
                vec![abs(1), absp("bracket"), abs(1), rel(1), abs(1)],
                vec![
                    fill("stone"),
                    call("beam_column"),
                    call("stand_column"),
                    call("room_column"),
                    fill("stone"),
                ],
            ),
        )
        .rule(
            "beam_column",
            split(
                Axis::Y,
                vec![abs(1), absp("head"), rel(1)],
                vec![fill("stone"), call("beam_interior"), fill("stone")],
            ),
        )
        // Open air, the timber course, open air: the corbel, carried in at the
        // height the perch stands on.
        .rule(
            "beam_interior",
            split(
                Axis::Y,
                vec![
                    abse(par("perch_rise").arith(ArithOp::Sub, int(2))),
                    abs(1),
                    rel(1),
                ],
                vec![void(), fill("timber"), void()],
            ),
        )
        .rule(
            "stand_column",
            split(
                Axis::Y,
                vec![abs(1), absp("head"), rel(1)],
                vec![fill("stone"), call("stand_interior"), fill("stone")],
            ),
        )
        // The pedestal, the space the lure is displayed in, the corbel's end, and
        // the cell a body waits in on top of it.
        .rule(
            "stand_interior",
            split(
                Axis::Y,
                vec![
                    abs(1),
                    abse(par("perch_rise").arith(ArithOp::Sub, int(3))),
                    abs(1),
                    abs(1),
                    rel(1),
                ],
                vec![
                    marked("bait", MarkAt::FloorCenter, fill("pedestal")),
                    void(),
                    fill("timber"),
                    marked("bait-perch", MarkAt::FloorCenter, void()),
                    void(),
                ],
            ),
        )
        // --- the slice in front of the perch, and the knob that spoils it ------
        .rule_alts(
            "valance_slice",
            vec![
                alt_when(cmp(par("canopy"), CmpOp::Le, int(0)), call("room_slab")),
                alt_when(
                    cmp(par("canopy"), CmpOp::Ge, int(1)),
                    split(
                        Axis::X,
                        vec![abs(1), rel(1), abs(1)],
                        vec![fill("stone"), call("valance_column"), fill("stone")],
                    ),
                ),
            ],
        )
        .rule(
            "valance_column",
            split(
                Axis::Y,
                vec![abs(1), absp("head"), rel(1)],
                vec![fill("stone"), call("valance_interior"), fill("stone")],
            ),
        )
        // Two courses of timber hung across the whole width at the perch's own
        // height: it blinds the watcher without touching the pedestal, so what
        // the gate catches is a hidden ambusher and not a walled-off room.
        .rule(
            "valance_interior",
            split(
                Axis::Y,
                vec![
                    abse(par("perch_rise").arith(ArithOp::Sub, int(2))),
                    abs(2),
                    rel(1),
                ],
                vec![void(), fill("timber"), void()],
            ),
        )
}
