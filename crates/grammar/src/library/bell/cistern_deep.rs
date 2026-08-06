//! **Z6 — the Cistern Deep.** The player does not walk into this zone, they
//! fall into it (REMAKE §3 Z6; §4 entries **L**, the dart gallery's **O**,
//! **X** and **E**).
//!
//! ```text
//!  local Z:  0 ... arena_run ... | ... vent_run ... | ... gallery_run ... | ... the shaft ... Z-1
//!            [ open arena      ] [ grate row wall ] [ bay | span | lane  ] [ landing | ledge ]
//!                 ^ elite_ground      ^ broken_grate      ^ watch_bay        ^ drop_shaft
//!                                              travel: Z-max -> Z-min
//!
//!  local Y:  the shaft is the only piece that is not flat — its entry ledge sits
//!            `shaft/drop` blocks above the floor every other piece stands on.
//! ```
//!
//! Four pieces of vocabulary in the order the player meets them, and no blocks
//! of the zone's own. The player steps off the ledge at local `Z`-max and lands
//! on the cistern floor `shaft/drop` blocks below; from the landing the dart
//! gallery's bay is the first thing in reach, so the volley is read before it is
//! crossed; past the span the cistern's own wall carries the broken grate; and
//! the deep's floor is the elite's open ground, with the way out on either side
//! of it.
//!
//! # Why the shaft goes at the entry end, and what that costs
//!
//! [`crate::library::drop_shaft`] builds its entry ledge `drop` blocks up and
//! its landing at the floor. Put it anywhere but the zone's `Z`-max end and the
//! piece *above* the drop would have to be raised to meet the ledge — a plinth
//! of stone the zone would have to write itself. Put it at the entry end and the
//! zone writes nothing: the ledge is simply where the zone is entered from, and
//! what is above it is the next zone's problem, not this one's.
//!
//! The cost is that a walker cannot cross this zone with the ±1 step every other
//! zone's gate uses. That is not a weaker claim, it is a different and stronger
//! pair of them: the zone is a route **downward** under the player's own
//! movement model (walk, or step off a ledge and fall), and it is **not** a
//! route back up under the plain step — the whole gallery is unreachable from
//! the whole deep, which is a far larger claim than the two-anchor one the piece
//! proves in a bare box.
//!
//! # Missing (REMAKE §3 Z6)
//!
//! The sally-port far-side bar (**F**). It is built as a rule
//! ([`crate::library::far_side_bar`]) and it still cannot go in this zone, for
//! two independent reasons, both asserted in `tests/zones.rs`: it declares
//! `anchor/gate`, which the dart gallery's own `watch_bay` already declares, and
//! [`crate::compose::include`] does not rename anchors; and a barred door on a
//! linear chain seals the chain, because a shortcut is a **branch** off the
//! mainline and the composition seam has no junction. Named, not faked.
//!
//! # Gates (`tests/zones.rs`)
//!
//! 1. **The cistern is a route, and only downward.** Entry reaches exit under
//!    walk-and-fall; exit does not reach entry under the plain step. Teeth:
//!    `shaft/rescue_ladder`, paired with a short `shaft/drop` as the piece's own
//!    teeth test is.
//! 2. **The volley span cannot be walked round — or fallen past.** The fall
//!    edge is the new risk a drop brings to a zone: it is a movement the piece
//!    gates never had to consider, and it only ever *adds* routes. Cut the span
//!    and the zone must be severed under that permissive model too.
//! 3. **The fight at the bottom is still optional.** A route from the zone's
//!    entry to its exit that never enters the engagement circle, on each side.
//!    Teeth: `arena/seal_flank`.
//! 4. **Nothing was turned**: anchors in travel order down the zone's own axis,
//!    and a piece run shorter than the zone is wide refused rather than turned.

use crate::compose::entry;
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, Program, Reorient};
use crate::library::{
    absp, all_of, alt_when, broken_grate, call, cmp, dim, drop_shaft, elite_ground, par, rel,
    reoriented, split, watch_bay,
};

use super::composed;

/// The prefix the elite ground is included under.
const ARENA: &str = "arena";
/// The prefix the broken-grate wall is included under.
const VENT: &str = "vent";
/// The prefix the dart gallery — a `watch_bay` — is included under.
const GALLERY: &str = "gallery";
/// The prefix the spill shaft is included under.
const SHAFT: &str = "shaft";

/// The Cistern Deep.
///
/// Parameters: `arena_run`, `vent_run` and `gallery_run` (how much of the
/// zone's length each of those three pieces takes; the shaft gets the rest, so
/// a longer zone is a deeper fall run, never a differently-shaped arena), plus
/// every parameter of the four included pieces under the `arena/`, `vent/`,
/// `gallery/` and `shaft/` prefixes — including the knobs the gates are shown
/// red with, `shaft/rescue_ladder`, `shaft/drop` and `arena/seal_flank`.
pub fn cistern_deep() -> Program {
    let arena = elite_ground();
    let vent = broken_grate();
    let gallery = watch_bay();
    let shaft = drop_shaft();
    let zone = Program::new("bell_cistern_deep", "cistern_deep")
        .param("arena_run", 20)
        .param("vent_run", 20)
        .param("gallery_run", 20)
        // --- frame -----------------------------------------------------------
        .rule(
            "cistern_deep",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("deep_plan"),
            ),
        )
        // One alternative, no `otherwise`: every clause is the frame constraint
        // of [`super`], applied to each of the four pieces in turn. The last one
        // is the shaft's, which takes whatever length the other three leave.
        .rule_alts(
            "deep_plan",
            vec![alt_when(
                all_of(vec![
                    cmp(par("arena_run"), CmpOp::Gt, dim(DimRef::X)),
                    cmp(par("vent_run"), CmpOp::Gt, dim(DimRef::X)),
                    cmp(par("gallery_run"), CmpOp::Gt, dim(DimRef::X)),
                    cmp(
                        dim(DimRef::Z)
                            .arith(ArithOp::Sub, par("arena_run"))
                            .arith(ArithOp::Sub, par("vent_run"))
                            .arith(ArithOp::Sub, par("gallery_run")),
                        CmpOp::Gt,
                        dim(DimRef::X),
                    ),
                ]),
                // Pieces run low to high and travel runs high to low, so this
                // list is the player's route read backwards: the deep's floor is
                // declared first and the ledge they step off last.
                split(
                    Axis::Z,
                    vec![
                        absp("arena_run"),
                        absp("vent_run"),
                        absp("gallery_run"),
                        rel(1),
                    ],
                    vec![
                        call(&entry(ARENA, &arena)),
                        call(&entry(VENT, &vent)),
                        call(&entry(GALLERY, &gallery)),
                        call(&entry(SHAFT, &shaft)),
                    ],
                ),
            )],
        );
    composed(
        zone,
        &[
            (ARENA, &arena),
            (VENT, &vent),
            (GALLERY, &gallery),
            (SHAFT, &shaft),
        ],
    )
}
