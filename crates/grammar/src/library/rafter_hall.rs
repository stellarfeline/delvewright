//! The rafter perch — a hall whose roof timbers are somewhere a body waits
//! (W2 entry R, drowned-bell remake step 2).
//!
//! **Original Delvewright content**, not a port: licence `original`. The
//! reference is DS3's Cathedral of the Deep, the densest ambush level in the
//! series and the one most criticised for ambush *monoculture*
//! (`docs/notes/souls-design-language.md` §4.1). Both halves of that are built
//! in here: the hall gains rafters an enemy can stand on, and the rule refuses
//! to build them densely enough to become the level's only idea.
//!
//! ```text
//!  local Y     local X:  0    1..b     b+1 .. X-b-2      X-b-1..X-2   X-1
//!  h-1 (head)            ##   air      air               air          ##
//!  h-2 (perch course)    ##   PERCH    open span         PERCH        ##
//!  h-3 (beam course)     ##   timber   open span         timber       ##
//!  1 .. h-4 (the nave)   ##   air      air               air          ##
//!  0  (floor)            ##   ######   ######            ######       ##
//!                             ^ corbel from the wall     ^ and from the other
//!
//!  along local Z:  |slice|  gap  |slice|  gap  |slice| ...   travel: Z-max -> Z-min
//!                  ^ every `beam_period` cells; sides alternate
//! ```
//!
//! # Why the centre span is open — the geometry that chose the form
//!
//! The obvious truss is a beam spanning wall to wall every few cells, with the
//! perch on the beam's centre. It cannot pass this entry's own sightline gate,
//! and not for want of tuning: an eye on the hall floor is *below* the beam
//! plane and a perch is *above* it, so every ray from the doorway to a perch has
//! to cross that plane. The crossing covers a run of about `0.42 × distance`
//! cells along the hall, so past roughly nine cells of hall the run is longer
//! than the gap between beams and some nearer beam is always in it. A full-span
//! truss hides its own far rafters — which is a fair description of Cathedral of
//! the Deep, and a bad one for a space we promise is legible from the door.
//!
//! So the timbers are **corbels**: they carry `bracket` cells in from each side
//! wall and stop, leaving the nave's centre open from the floor to the head
//! course. A ray from the doorway to any perch crosses the beam plane while it
//! is still only 16–58% of the way across to that perch's wall — inside the open
//! span, every time, at every hall length. The gate is met *by the form*, not by
//! a lucky size. The `span_beams` knob puts the full-span truss back so the test
//! can watch the gate go red.
//!
//! # The density cap is a guard, not a wish
//!
//! The entry asks for at most one perch per 24 floor cells. Perches come one per
//! truss slice and slices come every `beam_period` cells, so the count is
//! `ceil(Z / beam_period)` and the cap is arithmetic the rule can check before
//! it builds: `X·Z·beam_period ≥ 24·Z + 24·beam_period` (the `+ 24·period` pays
//! for the rounding up). A hall too narrow for its own truss is a refusal naming
//! the rule, never a hall quietly over the cap.
//!
//! # Anchors
//!
//! * `anchor/perch-<i>` — the standable cell at a corbel's inner end. Numbered
//!   along local `Z`, i.e. **against travel** (see the module note on the W1
//!   local frame): `perch-1` is the one deepest into the hall.
//! * `anchor/hall-door` — the floor cell at the centre of the approach end, the
//!   position the sightline gate reads the hall from.
//!
//! Every facing here is the derived one, pointing down the hall along travel: a
//! perched body watches the ground the player is walking into. Aiming the two
//! sides *across* at the nave instead would need a facing along local `+X`,
//! which `mark` cannot express — the same gap `docs/reference/grammar.md` §7
//! already files, met a second time.
//!
//! Smallest region that expands: X ≥ `2·bracket + 3`, Y ≥ 3, Z ≥ `beam_period`
//! — and, for the truss, Y ≥ 6 plus the density cap above. A hall under 6 tall
//! is a legal hall with no truss in it, not an error.

use crate::block::BlockState;
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, Program, Reorient, Side};

use super::{
    abs, abse, absp, all_of, alt_when, at_offset, call, cmp, dim, face, fill, int, marked,
    marked_each, par, rel, reoriented, split, split_repeat, void,
};

/// The hall height at which the truss layer appears. Below it a hall is a hall
/// and nothing else — the layer needs a floor, two cells of nave, the beam
/// course, the perch course and a cell of head clearance over it.
pub const TRUSS_MIN_HEIGHT: i64 = 6;

/// Floor cells the density cap demands per perch (`docs/notes/souls-design-language.md`
/// §4.1: the Cathedral's charge is monoculture, and the answer is a budget).
pub const FLOOR_CELLS_PER_PERCH: i64 = 24;

/// The shortest nave a trussed hall keeps under its rafters, in cells.
const NAVE_CLEARANCE: i64 = 2;

/// The rafter-perch hall.
///
/// Parameters: `beam_period` (cells between truss slices), `bracket` (how far a
/// corbel reaches in from its wall), and `span_beams` — a test knob, off by
/// default, that closes the open centre span so the sightline gate can be shown
/// to fail when it should. Palette roles: `stone` (the shell), `timber` (the
/// rafters).
pub fn rafter_hall() -> Program {
    Program::new("rafter_hall", "hall")
        .param("beam_period", 4)
        .param("bracket", 2)
        .param("span_beams", 0)
        .role("stone", BlockState::simple("stone_bricks"))
        .role("timber", BlockState::with("dark_oak_wood", [("axis", "y")]))
        // --- frame -----------------------------------------------------------
        // Length onto the longer horizontal axis, up stays up: the W1 frame, so
        // every derived facing points down the hall along travel.
        .rule(
            "hall",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("hall_shell"),
            ),
        )
        // Side walls, then the volume between them. Splitting these off first is
        // what lets every guard below measure the *interior* width, which is
        // what the density cap is actually about.
        .rule(
            "hall_shell",
            split(
                Axis::X,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("stone"), call("hall_column"), fill("stone")],
            ),
        )
        // The two shapes, and they are mutually exclusive on height: overlapping
        // guards would be a *distribution* (§2 of the grammar reference), and
        // "does this hall have rafters" is a fact about the box, not a taste.
        .rule_alts(
            "hall_column",
            vec![
                alt_when(
                    all_of(vec![
                        cmp(dim(DimRef::Y), CmpOp::Ge, int(TRUSS_MIN_HEIGHT)),
                        cmp(par("beam_period"), CmpOp::Ge, int(2)),
                        cmp(dim(DimRef::Z), CmpOp::Ge, par("beam_period")),
                        cmp(
                            dim(DimRef::X),
                            CmpOp::Ge,
                            par("bracket")
                                .arith(ArithOp::Mul, int(2))
                                .arith(ArithOp::Add, int(1)),
                        ),
                        density_cap(),
                    ]),
                    call("trussed_hall"),
                ),
                alt_when(
                    all_of(vec![
                        cmp(dim(DimRef::Y), CmpOp::Lt, int(TRUSS_MIN_HEIGHT)),
                        cmp(dim(DimRef::Y), CmpOp::Ge, int(NAVE_CLEARANCE + 1)),
                    ]),
                    call("plain_hall"),
                ),
            ],
        )
        // Floor, nave, the two-cell truss band, head clearance over the perches.
        // The band is where "the truss layer at h-2" lands: the beam course is
        // h-3 and the cell a body occupies on it is h-2.
        .rule(
            "trussed_hall",
            split(
                Axis::Y,
                vec![abs(1), rel(1), abs(2), abs(1)],
                vec![fill("stone"), call("nave"), call("truss_band"), void()],
            ),
        )
        // A hall under six tall: the same floor and the same door anchor, and no
        // rafters. A variant, not a failure — the campaign that binds
        // `anchor/hall-door` does not have to know which it got.
        .rule(
            "plain_hall",
            split(
                Axis::Y,
                vec![abs(1), rel(1)],
                vec![fill("stone"), call("nave")],
            ),
        )
        // The walkable volume, and the one anchor that is about the *player*:
        // the floor cell at the centre of the approach end. Declared here
        // because this is the only scope that spans the hall's whole length.
        .rule(
            "nave",
            marked(
                "hall-door",
                at_offset(
                    dim(DimRef::X)
                        .arith(ArithOp::Sub, int(1))
                        .arith(ArithOp::Div, int(2)),
                    int(0),
                    dim(DimRef::Z).arith(ArithOp::Sub, int(1)),
                ),
                void(),
            ),
        )
        // --- the truss ---------------------------------------------------------
        // Slices every `beam_period`, alternating which side carries the perch.
        // One tiled pattern rather than two: a repeating split cycles its
        // children, so the alternation is the pattern's own shape and no rule has
        // to know how many slices there will be.
        .rule(
            "truss_band",
            split_repeat(
                Axis::Z,
                vec![abs(1), gap(), abs(1), gap()],
                vec![call("slice_left"), void(), call("slice_right"), void()],
            ),
        )
        .rule(
            "slice_left",
            truss_slice("corbel_marked_high", "corbel_plain"),
        )
        .rule(
            "slice_right",
            truss_slice("corbel_plain", "corbel_marked_low"),
        )
        // A corbel is two courses: the timber, and the cell over it a body
        // stands in. Both sides of every slice carry one — a truss with a
        // bracket on one side only is not a truss — but only one of them is
        // declared a perch, which is what keeps the count inside the cap.
        .rule("corbel_plain", corbel(void()))
        .rule(
            "corbel_marked_high",
            corbel(marked_each("perch", face(Axis::X, Side::Max), void())),
        )
        .rule(
            "corbel_marked_low",
            corbel(marked_each("perch", face(Axis::X, Side::Min), void())),
        )
        // The nave's air, all the way up through the truss band — and the knob
        // that fills it in. `span_beams` is not a style: it is the full-span
        // truss the module note explains this rule cannot use, kept expressible
        // so the sightline gate can be watched failing.
        .rule_alts(
            "open_span",
            vec![
                alt_when(cmp(par("span_beams"), CmpOp::Le, int(0)), void()),
                alt_when(
                    cmp(par("span_beams"), CmpOp::Ge, int(1)),
                    split(Axis::Y, vec![abs(1), abs(1)], vec![fill("timber"), void()]),
                ),
            ],
        )
}

/// The gap between truss slices: the period less the slice itself.
fn gap() -> crate::ir::Size {
    abse(par("beam_period").arith(ArithOp::Sub, int(1)))
}

/// One truss slice across the hall: corbel, open nave, corbel.
fn truss_slice(low: &str, high: &str) -> crate::ir::Node {
    split(
        Axis::X,
        vec![absp("bracket"), rel(1), absp("bracket")],
        vec![call(low), call("open_span"), call(high)],
    )
}

/// A corbel: the timber course, then whatever the perch course is.
fn corbel(top: crate::ir::Node) -> crate::ir::Node {
    split(Axis::Y, vec![abs(1), abs(1)], vec![fill("timber"), top])
}

/// The density cap, stated as arithmetic the guard can decide.
///
/// Perches are one per truss slice and slices are `ceil(Z / beam_period)`, so
/// the cap `perches · 24 ≤ X · Z` becomes, after clearing the division and
/// paying for the rounding up (`ceil(Z/p) ≤ Z/p + 1`):
///
/// ```text
/// X · Z · p  ≥  24 · Z  +  24 · p
/// ```
///
/// `X` here is the *interior* width, because `hall_shell` has already taken the
/// side walls off — a wall is not floor a player stands on.
fn density_cap() -> crate::ir::Cond {
    let capacity = dim(DimRef::X)
        .arith(ArithOp::Mul, dim(DimRef::Z))
        .arith(ArithOp::Mul, par("beam_period"));
    let demand = int(FLOOR_CELLS_PER_PERCH)
        .arith(ArithOp::Mul, dim(DimRef::Z))
        .arith(
            ArithOp::Add,
            int(FLOOR_CELLS_PER_PERCH).arith(ArithOp::Mul, par("beam_period")),
        );
    cmp(capacity, CmpOp::Ge, demand)
}
