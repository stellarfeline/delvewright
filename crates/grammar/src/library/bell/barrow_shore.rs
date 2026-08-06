//! **Z0 — the Barrow Shore.** The delve's first room: one open fight on open
//! ground, with the way past it on both sides (REMAKE §3 Z0; §4 entry **E**).
//!
//! ```text
//!  local X:  west flank | ....... engagement circle ....... | east flank
//!  local Z:  exit run   | ....... engagement circle ....... | entry run
//!                                              travel: Z-max -> Z-min
//! ```
//!
//! # A one-piece zone, and an honest account of what that proves
//!
//! [`crate::library::elite_ground`] builds the whole of Z0's §4 vocabulary, so
//! this program composes exactly one piece and writes no blocks of its own.
//! **There is no seam here**, and it would be dishonest to dress the gate below
//! up as a composition proof: it re-binds the arena's own flank claim to the
//! *campaign's* box rather than the piece fixture's, and that is all. What the
//! program earns is the other thing a zone is for — a zone is one grammar
//! program (REMAKE §2), so what a campaign binds is `barrow_shore`, and the
//! counts its gate reports are the zone's.
//!
//! The frame guard is correspondingly narrow, and worth stating so nobody
//! reads more into it later. [`super`]'s constraint — a piece run shorter than
//! the zone is wide gets turned across the route — collapses, for a zone whose
//! one piece *is* the zone, to `Z > X`. And because the zone's own frame opens
//! with `z(Largest)` too, the box is normalised before the guard ever sees it,
//! so the only shape it can refuse is a **square** one: the case where which
//! face is the entry is a tie-break rather than a fact. A box wider than it is
//! long does not turn the arena relative to the zone — the zone turns with it,
//! coherently, and an open room rotated is the same open room.
//!
//! # Gates (`tests/zones.rs`)
//!
//! 1. **Two flank lanes, end to end across the zone.** The west and east bands
//!    (`X` strictly outside the circle's radius) each connect the zone's entry
//!    end to its exit end. Teeth: `arena/seal_flank` walls off one band or
//!    both, and the counted total drops from 2 to 1 to 0 while the zone stays
//!    walkable through the middle — so what went red is the bypass, not the
//!    room.
//! 2. **A square box is refused**, which is the whole of what the frame guard
//!    can catch here, asserted rather than assumed.
//!
//! Smallest region: whatever [`crate::library::elite_ground`] asks for (both
//! horizontal extents ≥ `2*radius + 1 + 2*flank_margin + 2`, `head + 2` tall),
//! and strictly longer than it is wide.

use crate::compose::entry;
use crate::ir::{AxisSpec, CmpOp, DimRef, Program, Reorient};
use crate::library::elite_ground;
use crate::library::{alt_when, call, cmp, dim, reoriented};

use super::composed;

/// The prefix the elite ground is included under.
const ARENA: &str = "arena";

/// The Barrow Shore.
///
/// Parameters: every parameter of the included [`elite_ground`] under the
/// `arena/` prefix — `arena/radius`, `arena/flank_margin`, `arena/approach`,
/// `arena/head`, and the knob the flank gate is shown red with,
/// `arena/seal_flank`. Palette role: `arena/stone`.
pub fn barrow_shore() -> Program {
    let arena = elite_ground();
    let zone = Program::new("bell_barrow_shore", "barrow_shore")
        // --- frame -----------------------------------------------------------
        .rule(
            "barrow_shore",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("shore_plan"),
            ),
        )
        // One alternative, no `otherwise`. The clause is the frame constraint of
        // [`super`] written for a zone whose one piece is the zone: the run must
        // be longer than the zone is wide. See the module note for how little
        // that can catch here, and why it is still the right guard to write.
        .rule_alts(
            "shore_plan",
            vec![alt_when(
                cmp(dim(DimRef::Z), CmpOp::Gt, dim(DimRef::X)),
                call(&entry(ARENA, &arena)),
            )],
        );
    composed(zone, &[(ARENA, &arena)])
}
