//! The watch bay — observability hardware for a gated passage (W1 entry O,
//! drowned-bell remake step 2).
//!
//! **Original Delvewright content**, not a port: licence `original`.
//!
//! A timed hazard the player cannot *read* before committing to it is a coin
//! flip, and the compiler says so: `DW0388` refuses a `timed-gate` span or a
//! `volley` kill zone with no standable watch cell that is clear of the span,
//! reachable before it, and has a sightline to it. This rule is the other half
//! of that contract — the geometry that makes the proof pass by construction,
//! so a campaign never has to discover at build time that the corridor it
//! generated has nowhere to stand and look.
//!
//! ```text
//!  local Z:  0 .......... span ...... approach ....... bay_zone (3)
//!            far          hazard      standoff         |bay|back wall|
//!                                                        ^ open toward the span
//!  local X:  0    1  2      3       4..            X-1
//!            wall |bay |  divider   lane           wall     travel: Z-max -> Z-min
//! ```
//!
//! The bay is a roofed 2×2 pocket at the approach end, walled on three sides
//! and open only toward the hazard, with the passage lane running past it — so
//! stepping into it is a choice the player makes and stepping out of it puts
//! them back on the road. Its anchor's facing is derived and therefore points
//! down-passage at the span, which is why travel runs toward local `Z`-min (see
//! the module note on the W1 local frame).
//!
//! # The gate
//!
//! An unobstructed sightline from the bay to **every** standable cell of the
//! hazard span, at a standoff of at least `MIN_STANDOFF`. That is deliberately
//! stronger than `DW0388`, which asks for sight to *some* cell at 5: the whole
//! point of generating the bay rather than hoping for one is that the campaign
//! proof cannot then fail. `tests/staging.rs` walks the line cell by cell with
//! the same Amanatides–Woo traversal the compiler uses, and the `obstruct` knob
//! below exists so that check can be shown to have teeth.
//!
//! The standoff is enforced *by the rule*: an `approach` under `MIN_STANDOFF`
//! leaves no applicable alternative, so a caller who dials it down gets a
//! refusal instead of a bay that quietly stopped being one.
//!
//! # Anchors
//!
//! * `anchor/watch` — the bay's standing cell nearest its open face, facing the
//!   span. This is the cell `DW0388` would look for.
//! * `anchor/gate` — the hazard span's floor centre, for the campaign to bind
//!   its `timed-gate` or `volley` to. A point, not a region: region anchors are
//!   not yet expressible by a rule (`docs/reference/grammar.md` §7), and this
//!   rule does not invent one.
//!
//! Smallest region that expands: X ≥ 6, Y ≥ `head` + 2, Z ≥ `approach` +
//! `span` + 4.

use crate::block::BlockState;
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, MarkAt, Program, Reorient};

use super::{
    abs, absp, all_of, alt_when, call, cmp, dim, fill, int, marked, par, rel, reoriented, split,
    split_exact, void,
};

/// The shortest approach the rule will build a bay behind, in blocks.
///
/// `DW0388` refuses a watch cell closer than 5 to the span — one second of
/// sprint, so that sight from the lip of the hazard does not count as safety.
/// The rule leaves a block of margin over the proof it has to survive.
pub const MIN_STANDOFF: i64 = 6;

/// Cells the bay zone takes along the passage: two of bay, one of back wall.
const BAY_ZONE: i64 = 3;

/// The gated passage with its watch bay.
///
/// Parameters: `approach` (blocks of standoff between the bay and the span),
/// `span` (the hazard's length along the passage), `head` (passage headroom),
/// `bay_height` (the bay's interior height, which must be under `head`), and
/// `obstruct` — a test knob, off by default, that stands one pillar in the bay's
/// line of sight so the sightline gate can be shown to fail when it should.
/// Palette role: `stone`.
pub fn watch_bay() -> Program {
    Program::new("watch_bay", "gate_passage")
        .param("approach", 8)
        .param("span", 3)
        .param("head", 4)
        .param("bay_height", 2)
        .param("obstruct", 0)
        .role("stone", BlockState::simple("stone"))
        // --- frame -----------------------------------------------------------
        .rule(
            "gate_passage",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("passage_plan"),
            ),
        )
        // One alternative and no `otherwise`: every clause here is a promise the
        // rule makes to `DW0388` or to its own geometry, so failing one is a
        // refusal, never a smaller bay.
        .rule_alts(
            "passage_plan",
            vec![alt_when(
                all_of(vec![
                    cmp(par("approach"), CmpOp::Ge, int(MIN_STANDOFF)),
                    cmp(par("bay_height"), CmpOp::Lt, par("head")),
                    cmp(dim(DimRef::X), CmpOp::Ge, int(6)),
                    cmp(
                        dim(DimRef::Y),
                        CmpOp::Ge,
                        par("head").arith(ArithOp::Add, int(2)),
                    ),
                    cmp(
                        dim(DimRef::Z),
                        CmpOp::Ge,
                        par("approach")
                            .arith(ArithOp::Add, par("span"))
                            .arith(ArithOp::Add, int(BAY_ZONE + 1)),
                    ),
                ]),
                split(
                    Axis::Z,
                    vec![rel(1), absp("span"), absp("approach"), abs(BAY_ZONE)],
                    vec![
                        call("corridor"),
                        call("hazard_span"),
                        call("approach_run"),
                        call("bay_zone"),
                    ],
                ),
            )],
        )
        // --- plain passage -----------------------------------------------------
        .rule(
            "corridor",
            split(
                Axis::X,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("stone"), call("corridor_column"), fill("stone")],
            ),
        )
        .rule(
            "corridor_column",
            split(
                Axis::Y,
                vec![abs(1), absp("head"), rel(1)],
                vec![fill("stone"), void(), fill("stone")],
            ),
        )
        // --- the hazard span ---------------------------------------------------
        .rule(
            "hazard_span",
            split(
                Axis::X,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("stone"), call("span_column"), fill("stone")],
            ),
        )
        // The mark sits on the *air* piece, not on the span box: a hazard anchor
        // is a cell a body stands in, and the span box's floor centre is the
        // floor block itself.
        .rule(
            "span_column",
            split(
                Axis::Y,
                vec![abs(1), absp("head"), rel(1)],
                vec![
                    fill("stone"),
                    marked("gate", MarkAt::FloorCenter, void()),
                    fill("stone"),
                ],
            ),
        )
        // --- the approach, and the knob that spoils it -------------------------
        .rule_alts(
            "approach_run",
            vec![
                alt_when(cmp(par("obstruct"), CmpOp::Le, int(0)), call("corridor")),
                alt_when(
                    cmp(par("obstruct"), CmpOp::Ge, int(1)),
                    // `split_exact`, not `split`: two relative pieces under
                    // truncation would leave the far end of the approach with no
                    // floor, and a hole in the floor is not the defect this knob
                    // is meant to inject.
                    split_exact(
                        Axis::Z,
                        vec![rel(1), abs(1), rel(1)],
                        vec![call("corridor"), call("pillar_slice"), call("corridor")],
                    ),
                ),
            ],
        )
        // One column of stone in the bay's own lane. It blinds the bay without
        // sealing the passage — so what the gate catches is blindness, and not
        // some other failure wearing its name.
        .rule(
            "pillar_slice",
            split(
                Axis::X,
                vec![abs(1), abs(1), rel(1), abs(1)],
                vec![
                    fill("stone"),
                    fill("stone"),
                    call("corridor_column"),
                    fill("stone"),
                ],
            ),
        )
        // --- the bay -----------------------------------------------------------
        // Across the passage: outer wall, the bay, a divider, the lane that runs
        // past it, outer wall. The divider is what leaves the bay one open face.
        .rule(
            "bay_zone",
            split(
                Axis::X,
                vec![abs(1), abs(2), abs(1), rel(1), abs(1)],
                vec![
                    fill("stone"),
                    call("bay_column"),
                    fill("stone"),
                    call("corridor_column"),
                    fill("stone"),
                ],
            ),
        )
        // Along the passage: two cells of bay at the low-Z (span-facing) end,
        // then the back wall.
        .rule(
            "bay_column",
            split(
                Axis::Z,
                vec![abs(2), abs(1)],
                vec![call("bay_room"), fill("stone")],
            ),
        )
        // Floor, interior, roof. The anchor lands on the interior's floor centre,
        // which on a 2×2 rounds down to the corner nearest the open face.
        .rule(
            "bay_room",
            split(
                Axis::Y,
                vec![abs(1), absp("bay_height"), rel(1)],
                vec![
                    fill("stone"),
                    marked("watch", MarkAt::FloorCenter, void()),
                    fill("stone"),
                ],
            ),
        )
}
