//! **Z1 — the Cliff Road.** The owner's mandated set piece at zone scale: a
//! one-wide ledge cut into a sea crag, with a gulf beside it deep enough that
//! being shoved off it is the whole threat (REMAKE §3 Z1, §4 entry K).
//!
//! ```text
//!  world X:  0 .. sea-1     sea      sea+1 ..              travel: Z-max -> Z-min
//!            ┌──────────┬─────────────────────────────┐
//!  the band  │   air    │ ledge │ niches │  crag …    │  <- cliff_path builds this
//!            ├──────────┼─────────────────────────────┤
//!  the gulf  │   air    │        solid crag           │  <- `fall` courses of it
//!            └──────────┴─────────────────────────────┘
//!              ^ the drop                ^ the zone's own mass
//! ```
//!
//! # What the zone adds to `cliff_path`, and why it has to
//!
//! [`crate::library::cliff_path`] guarantees the ledge is one wide and that its
//! niches are one deep — it says nothing about what is beside the ledge, because
//! a rule only owns the box it is handed. "One block wide on the outer edge,
//! cliff on one side" is a fact about the *zone's* box: the road is a shelf near
//! the top of it, the gulf is the courses below and the lane seaward of it. So
//! this program writes exactly two things itself — the crag mass under the road,
//! and the air beside it — and calls the vocabulary for everything a player
//! touches.
//!
//! That air is the point. Without it the ledge is a corridor whose outer wall
//! happens to be missing, and the knockback niche is a hard fight rather than
//! the one it was asked to be.
//!
//! # Gates (`tests/zones.rs`)
//!
//! 1. **The road goes somewhere, along the ledge.** The standable-cell graph
//!    connects the two ends of the zone, and with the ledge lane deleted it does
//!    not — asserted at zone scale, where a mis-sized crag could have left a
//!    second lane the piece-level gate would never have seen.
//! 2. **The drop is a drop.** From every ledge cell, the cell one step seaward
//!    is air and the column under it is clear for at least `fall` blocks. Teeth:
//!    `ledge_shelf` puts a rescue shelf across the gulf just under the road, and
//!    the gate must go red while the road stays walkable.
//! 3. **Every niche opens onto that ledge**, so the shove the recess exists for
//!    is a shove into the gulf and not into a wall.
//!
//! Smallest region: X ≥ `sea` + 3, Y ≥ `fall` + `path/niche_height` + 2, and
//! longer than it is wide net of the gulf (`Z` > `X` − `sea`), or the road piece
//! is turned across the zone — see the module note in [`super`].

use crate::block::BlockState;
use crate::compose::entry;
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, Program, Reorient};
use crate::library::cliff_path;
use crate::library::{
    abs, absp, all_of, alt_when, call, cmp, dim, fill, int, par, rel, reoriented, split, void,
};

use super::composed;

/// The prefix the cliff-path vocabulary is included under.
const PATH: &str = "path";

/// The shallowest gulf the zone will build a road beside, in blocks.
///
/// Below this a shove is survivable and the set piece is a normal fight next to
/// a hole. Vanilla starts charging for a fall above three blocks, so eight is
/// the shortest drop that is unmistakably a drop. What makes it *lethal* — sea,
/// void, rocks — is what the campaign puts at the bottom of the zone, which is
/// the campaign's decision and not the geometry's.
pub const MIN_DROP: i64 = 8;

/// The narrowest gulf, in blocks.
///
/// A one-cell gap is something a player is knocked *across*; the shove the
/// niche exists for has to land in open air.
pub const MIN_GULF: i64 = 3;

/// The Cliff Road.
///
/// Parameters: `sea` (how wide the gulf is), `fall` (how deep), `ledge_shelf` —
/// a test knob, off by default, that lays a shelf across the gulf one course
/// under the road so the drop gate can be shown to fail when it should — and
/// every parameter of the included [`cliff_path`], under the `path/` prefix
/// (`path/spacing_min`, `path/niche_height`, `path/watch_back`). Palette role:
/// `crag`, plus `path/rock` (the corpse prop is not a role — see
/// [`cliff_path`]).
pub fn cliff_road() -> Program {
    let path = cliff_path();
    let zone = Program::new("bell_cliff_road", "cliff_road")
        .param("sea", 3)
        .param("fall", MIN_DROP)
        .param("ledge_shelf", 0)
        .role("crag", BlockState::simple("stone"))
        // --- frame -----------------------------------------------------------
        // The zone's own travel frame, the same one every vocabulary rule uses,
        // so a zone turned 90° turns its pieces with it.
        .rule(
            "cliff_road",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("road_plan"),
            ),
        )
        // One alternative, no `otherwise`: each clause is something only the
        // zone knows, and a zone that cannot honour one is refused rather than
        // built into a road with a survivable drop beside it.
        .rule_alts(
            "road_plan",
            vec![alt_when(
                all_of(vec![
                    cmp(par("fall"), CmpOp::Ge, int(MIN_DROP)),
                    cmp(par("sea"), CmpOp::Ge, int(MIN_GULF)),
                    // Room for the gulf and the road band above it. The band's
                    // height is the *included* rule's requirement, read through
                    // its prefixed parameter rather than restated as a number
                    // that could drift away from it.
                    cmp(
                        dim(DimRef::Y),
                        CmpOp::Ge,
                        par("fall")
                            .arith(ArithOp::Add, par(&qualified("niche_height")))
                            .arith(ArithOp::Add, int(2)),
                    ),
                    // Ledge, recess, backing — `cliff_path`'s own minimum, left
                    // of the gulf.
                    cmp(
                        dim(DimRef::X),
                        CmpOp::Ge,
                        par("sea").arith(ArithOp::Add, int(3)),
                    ),
                    // ...and the road must be longer than it is wide, or its own
                    // frame turns it across the zone (see [`super`]).
                    cmp(
                        dim(DimRef::Z),
                        CmpOp::Gt,
                        dim(DimRef::X).arith(ArithOp::Sub, par("sea")),
                    ),
                ]),
                split(
                    Axis::Y,
                    vec![absp("fall"), rel(1)],
                    vec![call("gulf"), call("shelf")],
                ),
            )],
        )
        // --- the zone's own two facts ------------------------------------------
        // The shelf the road is cut into: air seaward, the road landward. The
        // road piece is the whole remaining width, so `cliff_path`'s backing
        // wall runs on into the crag instead of stopping at a seam.
        .rule(
            "shelf",
            split(
                Axis::X,
                vec![absp("sea"), rel(1)],
                vec![void(), call(&entry(PATH, &path))],
            ),
        )
        // What is under all that: nothing seaward, solid crag landward.
        .rule_alts(
            "gulf",
            vec![
                alt_when(
                    cmp(par("ledge_shelf"), CmpOp::Le, int(0)),
                    call("open_gulf"),
                ),
                // The knob. A shelf across the top course of the gulf — the
                // ledge is untouched and the road still walks end to end, so
                // what the drop gate catches is the missing drop and nothing
                // else.
                alt_when(
                    cmp(par("ledge_shelf"), CmpOp::Ge, int(1)),
                    split(
                        Axis::Y,
                        vec![rel(1), abs(1)],
                        vec![call("open_gulf"), fill("crag")],
                    ),
                ),
            ],
        )
        .rule(
            "open_gulf",
            split(
                Axis::X,
                vec![absp("sea"), rel(1)],
                vec![void(), fill("crag")],
            ),
        );
    composed(zone, &[(PATH, &path)])
}

/// A parameter of the included cliff path, by the name it answers to once it is
/// in this program.
fn qualified(param: &str) -> String {
    format!("{PATH}{}{param}", crate::compose::SEPARATOR)
}
