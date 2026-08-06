//! **Z5 — the Great Hall and Keep.** Rafters over the hall, a blind corner at
//! the door out of it, and the stores behind that (REMAKE §3 Z5; §4 entries
//! **R**, **A** and **C**).
//!
//! ```text
//!  local Z:  0 .... store_run .... | .... door_run .... | ....... the hall ....... Z-1
//!            [ barrel line + TELL ] [ room|alcove|WALL ] [ nave with its rafters ]
//!                 ^ store_room            ^ ambush_door        ^ rafter_hall
//!                                              travel: Z-max -> Z-min
//! ```
//!
//! Three pieces of vocabulary in the order the player meets them, and no blocks
//! of the zone's own. The composition is what makes the beat sequence a beat
//! sequence: the hall teaches the rafter silhouette from the door it is entered
//! by, the threshold at the far end hides a body the hall gave no reason to
//! expect, and the stores past it hold the one container that is wrong.
//!
//! # Why the pieces meet at all
//!
//! Every W1/W2 rule builds its shell open at both ends of its own travel axis —
//! side walls, floor, and nothing across `Z` — so pieces laid end to end along
//! that axis share a floor and an open seam, and the route runs through them.
//! That is a property of the vocabulary, not a coincidence of these three, and
//! the connectivity gate below is what keeps it true.
//!
//! # Missing (REMAKE §3 Z5)
//!
//! The bait-item gallery (**B**), the kitchen dumbwaiter down to the hub
//! (**L**), and the boss-threshold motif at the Hall Marshal's door (**M**).
//! Named, not faked.
//!
//! # Gates (`tests/zones.rs`)
//!
//! 1. **The doorway is the only route** from the hall to the stores — cut its
//!    column and the zone is severed.
//! 2. **The alcove is blind from the whole hall**, rafters included: a far
//!    larger approach set than the piece's own gate examined, and one that
//!    contains cells six blocks in the air.
//! 3. **Every perch is still visible from `anchor/hall-door`** on the assembled
//!    model. Teeth: `hall/span_beams`.
//! 4. **Exactly one tell**, over a sweep of seeds — the storeroom's
//!    exactly-one invariant is carried by a *recursion*, so this is also the
//!    gate that would catch an include that failed to rewrite a rule's calls to
//!    itself.
//! 5. **Nothing was turned**: anchors in travel order, and a short piece run
//!    refused rather than turned across the zone.

use crate::compose::entry;
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, Program, Reorient};
use crate::library::{
    absp, all_of, alt_when, ambush_door, call, cmp, dim, par, rafter_hall, rel, reoriented, split,
    store_room,
};

use super::composed;

/// The prefix the rafter hall is included under.
const HALL: &str = "hall";
/// The prefix the threshold is included under.
const DOOR: &str = "door";
/// The prefix the storeroom is included under.
const STORES: &str = "stores";

/// The Great Hall and Keep.
///
/// Parameters: `door_run` and `store_run` (how much of the zone's length the
/// threshold and the stores take; the hall gets the rest), plus every parameter
/// of the three included pieces under the `hall/`, `door/` and `stores/`
/// prefixes — including the two knobs the gates are shown red with,
/// `hall/span_beams` and `door/expose`.
pub fn hall_keep() -> Program {
    let hall = rafter_hall();
    let door = ambush_door();
    let stores = store_room();
    let zone = Program::new("bell_hall_keep", "keep")
        .param("door_run", 12)
        .param("store_run", 12)
        .rule(
            "keep",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("keep_plan"),
            ),
        )
        // One alternative, no `otherwise`: every clause is the frame constraint
        // of [`super`], applied to each of the three pieces in turn.
        .rule_alts(
            "keep_plan",
            vec![alt_when(
                all_of(vec![
                    cmp(par("store_run"), CmpOp::Gt, dim(DimRef::X)),
                    cmp(par("door_run"), CmpOp::Gt, dim(DimRef::X)),
                    cmp(
                        dim(DimRef::Z)
                            .arith(ArithOp::Sub, par("store_run"))
                            .arith(ArithOp::Sub, par("door_run")),
                        CmpOp::Gt,
                        dim(DimRef::X),
                    ),
                ]),
                // Low to high is the reverse of travel: the stores are the
                // deepest room and the hall is where the player comes in.
                split(
                    Axis::Z,
                    vec![absp("store_run"), absp("door_run"), rel(1)],
                    vec![
                        call(&entry(STORES, &stores)),
                        call(&entry(DOOR, &door)),
                        call(&entry(HALL, &hall)),
                    ],
                ),
            )],
        );
    composed(zone, &[(HALL, &hall), (DOOR, &door), (STORES, &stores)])
}
