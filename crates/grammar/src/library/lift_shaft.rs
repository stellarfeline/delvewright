//! The lift shaft — the static geometry a counterweight lift rides in (W4
//! entry L/4, drowned-bell remake step 2).
//!
//! **Original Delvewright content**, not a port: licence `original`. This rule
//! builds no moving part, and that is the point: a lift is a `sequence` of
//! runtime state, region fill/clear and teleport-by-region authored entirely in
//! campaign JSON (spec-0031), so **nothing moves**. The car is *filled* at the
//! floor it is called to, its riders are teleported, and the car it left is
//! *cleared*. What that sequence needs from a prefab is a hole with a landing
//! per floor and cells it can name — which is ordinary static work, and is all
//! this rule does.
//!
//! ```text
//!  local X:   0 .. mass ..   lane (lane cells)   .. mass .. X-1
//!  local Z:   0 .. back mass ..  lane  | face (1)          Z-1
//!                                        ^ the approach side: one doorway per storey
//!
//!  local Y, bottom to top:
//!    sill+2*storey ┊ air (lane)          ┊ #### solid face
//!    sill+storey   ┊ STATION 2 (car deck)┊ ##DOORWAY## + anchor/lift-call-2
//!    sill          ┊ STATION 1           ┊ ##DOORWAY## + anchor/lift-call-1
//!    1 .. sill-1   ┊ open shaft (anchor/lift-pit at the bottom of it)
//!    0             ┊ #### shaft floor
//! ```
//!
//! # The contract, taken from the merged lift rather than invented
//!
//! `crates/delvec/tests/fixtures/lift` is the shipped lift, and every cell it
//! needs is read off **one anchor per floor**, four ways: `fill-region {anchor,
//! extent [1,0,1]}` builds the car's deck, `clear-region` on the same box takes
//! the old one away, `teleport {to: anchor}` puts the riders on it, and
//! `give-effect {in: {anchor, extent [1,1,1]}}` gathers everyone aboard. So a
//! station is one cell, and a runtime region is a box **centred** on it with
//! unsigned half-extents — which is why the lane is guarded at
//! [`MIN_LANE`] and the station sits at its centre: at `extent [1,0,1]` the car
//! is 3×3, and a narrower lane would have the campaign's own `fill-region`
//! writing the car through the shaft wall.
//!
//! spec-0031 records the rest of that finding as open — "a runtime region cannot
//! name a cell at an offset from an anchor… so a lift's geometry is authored in
//! NBT rather than in campaign JSON, **and no prefab in the library ships a
//! shaft**". This rule is that prefab. The two cells the spec names as
//! unaddressable from campaign JSON are declared here instead:
//! `anchor/lift-station-<i>` (the deck, which is also the arrival cell — the
//! fixture's `fill` is a bottom slab, so deck and rider share one cell) and
//! `anchor/lift-pit` (the shaft-bottom volume a rider who steps into an empty
//! shaft lands in).
//!
//! # A shaft is a hole, and the hole is the hazard
//!
//! The lane is **air from the shaft floor to the top of the last storey**. When
//! the car is elsewhere a landing opens onto nothing, and a body that walks
//! through it falls to `anchor/lift-pit`. That is not a defect to be walled off
//! later: the L family is one-way hardware, and this rule owes the same pair of
//! claims [`super::drop_shaft`] and [`super::dumbwaiter`] owe — the pit is
//! reachable from a landing under walk-and-fall, and the landing is **not**
//! reachable from the pit under the plain ±1 step. `sill` is what makes the
//! second true, and the calibration control is the same rule at `sill = 2`,
//! where the drop is one block and both walks connect.
//!
//! # Why a repeat and not a recursion
//!
//! [`super::stair_flight`] climbs by calling itself, because every tread has to
//! know how high the last one was. A shaft's storeys are **identical** — same
//! doorway, same station, same wall above it — so the tiling a
//! `split_repeat` gives is exactly right and no storey has to know which one it
//! is. `marked_each` numbers them anyway. The one thing a tiling cannot do is
//! leave a remainder: an uncovered slice is unwritten, and an unwritten cell is
//! air — a hole in the face. So the rule refuses a box whose storeys do not
//! divide it (`(Y - sill) % storey == 0`) rather than shipping a shaft with its
//! top wall missing.
//!
//! # Anchors
//!
//! * `anchor/lift-station-<i>` — the car's deck cell at storey `i`, at the
//!   lane's horizontal centre. Numbered **bottom up**: `lift-station-1` is the
//!   lowest, which is the order a lift's floors are numbered in and the order a
//!   `split` visits its pieces in.
//! * `anchor/lift-call-<i>` — the solid jamb cell beside storey `i`'s doorway,
//!   at the landing's own standing level: where a campaign hangs the call
//!   control. Deliberately **outside** the car's 3×3 footprint, or the first
//!   `fill-region` of a ride would bury it.
//! * `anchor/lift-pit` — the standable cell at the bottom of the shaft, one
//!   course above the shaft floor: the volume a campaign makes lethal.
//!
//! Smallest region that expands: **`lane + 2` × (`sill` + `storey`) ×
//! (`lane` + 2)** — and at least as deep as it is wide, since the frame turns
//! length onto the longer horizontal axis and this rule's length is its
//! *depth*, the axis the landing face is on.

use crate::block::BlockState;
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, MarkAt, Program, Reorient};

use super::{
    abs, absp, all_of, alt_when, at_offset, call, cmp, dim, fill, int, marked, marked_each, par,
    rel, reoriented, split_exact, split_repeat, void,
};

/// The narrowest clear cross-section the rule will cut.
///
/// Three, because a runtime region is a box centred on its anchor with unsigned
/// half-extents: the shipped lift's car is `extent [1,0,1]`, i.e. 3×3 about the
/// station, and a 1- or 2-wide lane would have the campaign's own `fill-region`
/// writing the car's deck through the shaft wall. A shaft narrower than a body
/// rides in is a [`super::dumbwaiter`], which is a different rule.
pub const MIN_LANE: i64 = 3;

/// The lift shaft.
///
/// Parameters: `lane` (the clear cross-section, floor [`MIN_LANE`]), `storey`
/// (cells of rise between stations), `sill` (cells of open shaft below the
/// lowest station — the drop, and the reason the shaft is one-way), and
/// `door_height` (how tall each landing doorway is). `sealed` is a test knob,
/// off by default, that fills every landing doorway with the shell material so
/// the one-opening-per-storey gate can be shown to fail when it should. Palette
/// role: `rock` (the whole shell).
pub fn lift_shaft() -> Program {
    Program::new("lift_shaft", "lift_shaft")
        .param("lane", MIN_LANE)
        .param("storey", 5)
        .param("sill", 6)
        .param("door_height", 2)
        .param("sealed", 0)
        .role("rock", BlockState::simple("stone"))
        // --- frame -----------------------------------------------------------
        .rule(
            "lift_shaft",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("shaft_plan"),
            ),
        )
        // One alternative and no `otherwise`. A box that cannot hold the lane
        // with mass round it, or whose storeys do not divide the rise it is
        // given, is not a shorter shaft — it is a shaft with a wall missing, and
        // a refusal naming the rule is the honest answer.
        .rule_alts(
            "shaft_plan",
            vec![alt_when(
                all_of(vec![
                    cmp(par("lane"), CmpOp::Ge, int(MIN_LANE)),
                    cmp(par("door_height"), CmpOp::Ge, int(2)),
                    // A storey has to be taller than its own doorway, or the
                    // face is doorway all the way up and "one opening per
                    // storey" means nothing.
                    cmp(
                        par("storey"),
                        CmpOp::Ge,
                        par("door_height").arith(ArithOp::Add, int(1)),
                    ),
                    // The shaft floor, plus at least one cell of pit over it.
                    cmp(par("sill"), CmpOp::Ge, int(2)),
                    cmp(
                        dim(DimRef::X),
                        CmpOp::Ge,
                        par("lane").arith(ArithOp::Add, int(2)),
                    ),
                    cmp(
                        dim(DimRef::Z),
                        CmpOp::Ge,
                        par("lane").arith(ArithOp::Add, int(2)),
                    ),
                    cmp(rise(), CmpOp::Ge, par("storey")),
                    // The tiling leaves no remainder — see the module note on
                    // why an unwritten slice is a hole rather than a shorter
                    // shaft. `modulo_is` takes a literal modulus and this one is
                    // a knob, so the comparison is written out.
                    cmp(rise().arith(ArithOp::Rem, par("storey")), CmpOp::Eq, int(0)),
                ]),
                split_exact(
                    Axis::X,
                    vec![rel(1), absp("lane"), rel(1)],
                    vec![fill("rock"), call("shaft_slab"), fill("rock")],
                ),
            )],
        )
        // The lane's own slab, front to back: solid mass behind the shaft, the
        // hole itself, and the one face the landings are cut in. The face is the
        // last child, i.e. at local `Z`-max — the approach side, where the frame
        // note puts the route.
        .rule(
            "shaft_slab",
            split_exact(
                Axis::Z,
                vec![rel(1), absp("lane"), abs(1)],
                vec![fill("rock"), call("lane_stack"), call("face_stack")],
            ),
        )
        // --- the hole ---------------------------------------------------------
        .rule(
            "lane_stack",
            split_exact(
                Axis::Y,
                vec![absp("sill"), rel(1)],
                vec![call("pit_zone"), call("lane_storeys")],
            ),
        )
        // The bottom: a floor, and the cell above it a falling body lands on.
        .rule(
            "pit_zone",
            split_exact(
                Axis::Y,
                vec![abs(1), rel(1)],
                vec![
                    fill("rock"),
                    marked("lift-pit", MarkAt::FloorCenter, void()),
                ],
            ),
        )
        .rule(
            "lane_storeys",
            split_repeat(Axis::Y, vec![absp("storey")], vec![call("lane_storey")]),
        )
        // One storey of hole. The station is the lane's horizontal centre at the
        // storey's own floor level — the same level as the doorway opposite it,
        // because both are the first course of the same slice.
        .rule(
            "lane_storey",
            marked_each(
                "lift-station",
                at_offset(centered(DimRef::X), int(0), centered(DimRef::Z)),
                void(),
            ),
        )
        // --- the landing face --------------------------------------------------
        .rule(
            "face_stack",
            split_exact(
                Axis::Y,
                vec![absp("sill"), rel(1)],
                vec![fill("rock"), call("face_storeys")],
            ),
        )
        .rule(
            "face_storeys",
            split_repeat(Axis::Y, vec![absp("storey")], vec![call("face_storey")]),
        )
        .rule(
            "face_storey",
            split_exact(
                Axis::Y,
                vec![absp("door_height"), rel(1)],
                vec![call("door_row"), fill("rock")],
            ),
        )
        // `split_exact`, not the truncating default: two relative jambs under
        // truncation leave the far end of the face unwritten, and an unwritten
        // cell is air — a second opening in the one plane this rule promises is
        // solid but for its doorway.
        .rule(
            "door_row",
            split_exact(
                Axis::X,
                vec![rel(1), abs(1), rel(1)],
                vec![call("call_jamb"), call("doorway"), fill("rock")],
            ),
        )
        // The call control's cell: solid wall beside the opening, at the
        // landing's own standing level. Outside the car's footprint on purpose —
        // see the module note.
        .rule(
            "call_jamb",
            marked_each("lift-call", MarkAt::FloorCenter, fill("rock")),
        )
        .rule_alts(
            "doorway",
            vec![
                alt_when(cmp(par("sealed"), CmpOp::Le, int(0)), void()),
                alt_when(cmp(par("sealed"), CmpOp::Ge, int(1)), fill("rock")),
            ],
        )
}

/// How much of the box the storeys are tiled across: everything above the sill.
fn rise() -> crate::ir::Expr {
    dim(DimRef::Y).arith(ArithOp::Sub, par("sill"))
}

/// The centre of the current scope along one local axis.
fn centered(of: DimRef) -> crate::ir::Expr {
    dim(of)
        .arith(ArithOp::Sub, int(1))
        .arith(ArithOp::Div, int(2))
}
