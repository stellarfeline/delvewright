//! **Z5 — the Great Hall and Keep.** Rafters over the hall, a blind corner at
//! the door out of it, the stores behind that, the gallery's baited pedestal,
//! the Hall Marshal's threshold, and the kitchen duct down to the hub (REMAKE §3
//! Z5; §4 entries **R**, **A**, **C**, **B**, **M** and **L** — all six).
//!
//! ```text
//!  local Z:  0 ... duct_run ... | motif | gallery | stores | door | ... the hall ... Z-1
//!            [ landing, duct,   ] [room ] [ lure  ] [barrel] [room] [ nave with its  ]
//!            [ and the ledge    ] [CURT-] [ + its ] [ line ] [alc-] [ rafters        ]
//!            [ stepped off      ] [ AIN ] [ watch ] [+TELL ] [ove ]
//!               ^ dumbwaiter      ^motif  ^bait_stand ^store  ^ambush  ^ rafter_hall
//!                                                                  travel: Z-max -> Z-min
//!
//!  local Y:  everything but the duct stands on a plinth `duct/drop` blocks
//!            thick — the keep's own footing. See below.
//! ```
//!
//! Six pieces of vocabulary in the order the player meets them, and one fact of
//! the zone's own. The composition is what makes the beat sequence a beat
//! sequence: the hall teaches the rafter silhouette from the door it is entered
//! by, the threshold at the far end hides a body the hall gave no reason to
//! expect, the stores past it hold the one container that is wrong, the gallery
//! offers something worth taking with the thing that wants you to take it
//! standing over it in the same view, and the Marshal's door carries the motif
//! taught at Z2 — then the kitchen duct drops the player into the chapel ward
//! and does not let them back.
//!
//! # Why the pieces meet at all
//!
//! Every W1/W2 rule builds its shell open at both ends of its own travel axis —
//! side walls, floor, and nothing across `Z` — so pieces laid end to end along
//! that axis share a floor and an open seam, and the route runs through them.
//! That is a property of the vocabulary, not a coincidence of these six, and the
//! connectivity gate below is what keeps it true.
//!
//! # The plinth, and why it is the same construction Z2 uses
//!
//! [`crate::library::dumbwaiter`] builds its entry ledge `drop` blocks up and its
//! landing at the floor, so a zone that puts it anywhere but its `Z`-max end has
//! to raise everything above the drop to meet that ledge. Z5 is entered on foot
//! and left down the duct, so the zone writes a plinth: a `margin` slab
//! `duct/drop` thick under the whole keep, which puts the keep's floor at exactly
//! the height of the duct's own entry floor. Inert rock a player never touches —
//! the "mass no piece handed a sub-box can know about" that licenses
//! `cliff_road`'s crag.
//!
//! Its thickness is **read from the piece** (`par("duct/drop")`) rather than
//! restated as a zone constant, so the two cannot drift, and the one-way gate's
//! teeth — a short drop plus `duct/rescue_ladder` — shorten the plinth with it.
//! [`super::gate_ward::MIN_UPPER`] is the guard both zones share: a plinth as
//! thick as the box would leave a remainder of nothing, and nothing is written
//! silently.
//!
//! # Missing (REMAKE §3 Z5)
//!
//! Nothing. **B** was the last entry with no rule at all
//! ([`crate::library::bait_stand`] is it), and **L** and **M** were built rules
//! waiting for a zone round.
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
//! 5. **The keep is a route down, and not back up.** Entry reaches exit under
//!    walk-and-fall; exit does not reach entry under the plain step. Teeth:
//!    `duct/rescue_ladder` paired with a short `duct/drop`.
//! 6. **The lure's watcher is legible from the composed gallery** — every cell
//!    of the zone that can see `anchor/bait` can see `anchor/bait-perch`, over
//!    an approach set the piece's own fixture could not offer. Teeth:
//!    `gallery/canopy`.
//! 7. **Nothing was turned**: anchors in travel order, and a short piece run
//!    refused rather than turned across the zone.

use crate::block::BlockState;
use crate::compose::entry;
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, Expr, Program, Reorient};
use crate::library::{
    abse, absp, all_of, alt_when, ambush_door, bait_stand, call, cmp, dim, dumbwaiter, fill, par,
    rafter_hall, rel, reoriented, split_exact, store_room, threshold_motif,
};

use super::composed;
use super::gate_ward::MIN_UPPER;

/// The prefix the rafter hall is included under.
const HALL: &str = "hall";
/// The prefix the threshold is included under.
const DOOR: &str = "door";
/// The prefix the storeroom is included under.
const STORES: &str = "stores";
/// The prefix the baited gallery is included under.
const GALLERY: &str = "gallery";
/// The prefix the Hall Marshal's threshold motif is included under.
const MOTIF: &str = "motif";
/// The prefix the kitchen duct is included under.
const DUCT: &str = "duct";

/// The Great Hall and Keep.
///
/// Parameters: `duct_run` (how much of the zone's length the kitchen duct takes,
/// at the exit end and one level down), `motif_run`, `gallery_run`, `store_run`
/// and `door_run` (the keep's own rooms; the hall takes whatever is left), plus
/// every parameter of the six included pieces under the `hall/`, `door/`,
/// `stores/`, `gallery/`, `motif/` and `duct/` prefixes — including the knobs
/// the gates are shown red with: `hall/span_beams`, `door/expose`,
/// `gallery/canopy`, `duct/rescue_ladder` and `duct/drop`. Palette role:
/// `margin`, the zone's own inert mass — the plinth.
pub fn hall_keep() -> Program {
    let hall = rafter_hall();
    let door = ambush_door();
    let stores = store_room();
    let gallery = bait_stand();
    let motif = threshold_motif();
    let duct = dumbwaiter();
    let zone = Program::new("bell_hall_keep", "keep")
        .param("duct_run", 12)
        .param("motif_run", 12)
        .param("gallery_run", 12)
        .param("store_run", 12)
        .param("door_run", 12)
        .role("margin", BlockState::with("deepslate", [("axis", "y")]))
        .rule(
            "keep",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("keep_plan"),
            ),
        )
        // One alternative, no `otherwise`: the first clause is the plinth's (see
        // the module note), and every other is the frame constraint of [`super`]
        // applied to each of the six pieces in turn.
        .rule_alts(
            "keep_plan",
            vec![alt_when(
                all_of(vec![
                    cmp(
                        dim(DimRef::Y),
                        CmpOp::Ge,
                        par("duct/drop").arith(ArithOp::Add, Expr::int(MIN_UPPER)),
                    ),
                    cmp(par("duct_run"), CmpOp::Gt, dim(DimRef::X)),
                    cmp(par("motif_run"), CmpOp::Gt, dim(DimRef::X)),
                    cmp(par("gallery_run"), CmpOp::Gt, dim(DimRef::X)),
                    cmp(par("store_run"), CmpOp::Gt, dim(DimRef::X)),
                    cmp(par("door_run"), CmpOp::Gt, dim(DimRef::X)),
                    cmp(hall_run(), CmpOp::Gt, dim(DimRef::X)),
                ]),
                // Low to high is the reverse of travel: the duct the player
                // leaves by is the deepest thing in the keep, and the hall is
                // where they come in.
                split_exact(
                    Axis::Z,
                    vec![absp("duct_run"), rel(1)],
                    vec![call(&entry(DUCT, &duct)), call("upper_keep")],
                ),
            )],
        )
        // --- the plinth, and the keep standing on it ---------------------------
        .rule(
            "upper_keep",
            split_exact(
                Axis::Y,
                vec![abse(par("duct/drop")), rel(1)],
                vec![fill("margin"), call("keep_chain")],
            ),
        )
        .rule(
            "keep_chain",
            split_exact(
                Axis::Z,
                vec![
                    absp("motif_run"),
                    absp("gallery_run"),
                    absp("store_run"),
                    absp("door_run"),
                    rel(1),
                ],
                vec![
                    call(&entry(MOTIF, &motif)),
                    call(&entry(GALLERY, &gallery)),
                    call(&entry(STORES, &stores)),
                    call(&entry(DOOR, &door)),
                    call(&entry(HALL, &hall)),
                ],
            ),
        );
    composed(
        zone,
        &[
            (HALL, &hall),
            (DOOR, &door),
            (STORES, &stores),
            (GALLERY, &gallery),
            (MOTIF, &motif),
            (DUCT, &duct),
        ],
    )
}

/// What the hall gets: whatever length the duct and the four rooms leave.
fn hall_run() -> Expr {
    dim(DimRef::Z)
        .arith(ArithOp::Sub, par("duct_run"))
        .arith(ArithOp::Sub, par("motif_run"))
        .arith(ArithOp::Sub, par("gallery_run"))
        .arith(ArithOp::Sub, par("store_run"))
        .arith(ArithOp::Sub, par("door_run"))
}
