//! **Z2 — the Gatehouse.** The timed portcullis with the bay you read it from,
//! and the threshold at the far end of it (REMAKE §3 Z2; §4 entries **O** and
//! **A**).
//!
//! ```text
//!  local Z:  0 ......... door_run ......... | ............ the gate passage ......... Z-1
//!            [ room | alcove row | WALL | approach ] [ corridor | SPAN | approach | BAY ]
//!                        ^ ambush_door                             ^ watch_bay
//!                                              travel: Z-max -> Z-min
//! ```
//!
//! The player arrives from the cliff road at local `Z`-max, into the bay's end
//! of the passage; reads the hazard span from the bay; crosses it; and meets the
//! ward's threshold at the far end. Both pieces are vocabulary, and the zone
//! writes no blocks at all — it splits the box and calls them, which is the
//! whole of what a zone program is allowed to do.
//!
//! # This is a partial zone, and here is exactly what is missing
//!
//! REMAKE §3 gives Z2 six more things: the boulder stair with its worn-tread
//! lane (**W**), the safe pockets along it (**S**), the boulder jam (**D**), the
//! sally-port far-side bar (**F**), the spill shaft down to BF2 (**L**), and the
//! boss-threshold motif (**M**). What is composed here is the zone's *spine* —
//! the route in, the hazard that cannot be walked round, and the door at the end
//! of it. The rest is named, not faked.
//!
//! **Five of those six are built rules** — `boulder_stair` carries both **W**
//! and **S**, and `far_side_bar`, `drop_shaft` and `threshold_motif` are **F**,
//! **L** and **M**. (This note previously said none of them existed; they landed
//! in the W3/W4 families, and the claim was already stale when it was written.)
//! What Z2 waits on is a zone-program round composing them, not vocabulary. Only
//! the boulder jam (**D**) has no rule.
//!
//! # Gates (`tests/zones.rs`)
//!
//! 1. **The hazard cannot be walked round.** The zone connects end to end, and
//!    with the span's cells deleted it does not.
//! 2. **The bay still sees the whole span after composition.** The piece proved
//!    this in a bare box; a zone can put something in the line, so it is
//!    re-proved on the assembled model. Teeth: `gate/obstruct`, the piece's own
//!    knob, reached through the composed program's parameter namespace.
//! 3. **The alcove is blind from the whole zone.** Not just from the door
//!    piece's own approach, but from every standable cell of the gate passage as
//!    well — a strictly larger set than the vocabulary's gate examined. Teeth:
//!    `door/expose`.
//! 4. **Nothing was turned.** The two pieces' anchors run in travel order along
//!    the zone's own axis, and a piece run shorter than the zone is wide is
//!    refused rather than turned across it.

use crate::compose::entry;
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, Program, Reorient};
use crate::library::ambush_door;
use crate::library::{
    absp, all_of, alt_when, call, cmp, dim, par, rel, reoriented, split, watch_bay,
};

use super::composed;

/// The prefix the gated passage is included under.
const GATE: &str = "gate";
/// The prefix the threshold is included under.
const DOOR: &str = "door";

/// The Gatehouse.
///
/// Parameters: `door_run` (how much of the zone's length the threshold piece
/// takes), plus every parameter of the included pieces under the `gate/` and
/// `door/` prefixes — `gate/approach`, `gate/span`, `door/door_offset` and the
/// two knobs the gates are shown red with, `gate/obstruct` and `door/expose`.
pub fn gate_ward() -> Program {
    let gate = watch_bay();
    let door = ambush_door();
    let zone = Program::new("bell_gate_ward", "gate_ward")
        .param("door_run", 12)
        .rule(
            "gate_ward",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("ward_plan"),
            ),
        )
        // One alternative, no `otherwise`. Both clauses are the frame
        // constraint of [`super`]: a piece must be longer than the zone is wide
        // or its own `z(Largest)` turns it across the route. A refusal names the
        // rule; a turned wall would name nothing and seal the zone.
        .rule_alts(
            "ward_plan",
            vec![alt_when(
                all_of(vec![
                    cmp(par("door_run"), CmpOp::Gt, dim(DimRef::X)),
                    cmp(
                        dim(DimRef::Z).arith(ArithOp::Sub, par("door_run")),
                        CmpOp::Gt,
                        dim(DimRef::X),
                    ),
                ]),
                // Pieces run low to high, and travel runs high to low, so the
                // threshold — the last thing the player meets — is declared
                // first.
                split(
                    Axis::Z,
                    vec![absp("door_run"), rel(1)],
                    vec![call(&entry(DOOR, &door)), call(&entry(GATE, &gate))],
                ),
            )],
        );
    composed(zone, &[(GATE, &gate), (DOOR, &door)])
}
