//! **Z1 — the Cliff Road.** The owner's mandated set piece at zone scale: a
//! **switchback** cut into a sea crag, one block wide on the outer edge, the
//! gulf beside it deep enough that being shoved off it is the whole threat
//! (REMAKE §3 Z1, §4 entry K).
//!
//! ```text
//!  local X:  0 .. band-1        band .. band+sea-1        band+sea ..
//!            ┌───────────────┬──────────────────────┬───────────────────┐
//!  the band  │ far leg ⟲     │       the gulf       │     near leg      │  <- two cliff_paths
//!            │ crag│niche│ ·  │         air          │  · │niche│crag …  │
//!            ├───────────────┼──────────────────────┼───────────────────┤
//!  the gulf  │  solid crag   │         air          │    solid crag     │  <- `fall` courses
//!            └───────────────┴──────────────────────┴───────────────────┘
//!                            ^ · = the two ledge lanes — open air one cell
//!                              wide, each on its own leg's gulf face, so a
//!                              shove off either lands in the same gulf
//!
//!  local Z:  0 .. turn_run-1          turn_run ..
//!            ┌──────────────┬────────────────────────────────────────────┐
//!            │  the head    │  the two legs, running opposite ways       │
//!            └──────────────┴────────────────────────────────────────────┘
//!            ^ solid to road level: the hairpin the player turns on
//!
//!  travel: in along the near leg (Z-max → Z-min), round the head, out along
//!          the far leg (Z-min → Z-max). Both drops are into the same gulf.
//! ```
//!
//! # Why there are two legs and not one
//!
//! The design's fairness mechanism does not work without the switchback. §4 K
//! makes a first-time player's survival depend on the niche being "VISIBLE as a
//! shadowed recess **from the previous switchback**" — the observability rule
//! (§2.2-5). A one-deep recess off a one-wide ledge is invisible from anywhere
//! *along* its own path, deliberately ([`crate::library::cliff_path()`]); the
//! only place it can be read from is another leg looking **across**. A single
//! run has no other leg, so teach → test → twist would be teaching with nothing
//! visible to learn from.
//!
//! A hairpin's second leg is the first leg **turned round** — a half-turn about
//! the vertical, which is a *rotation*, proper and chirality-preserving, and not
//! a reflection ([`crate::geom::Orientation::is_rotation`]). A frame carries a
//! sign per local axis ([`crate::geom::Mirror`]), so the far leg is literally
//! the near leg under [`crate::ir::Reorient::turned`]: same rule, same
//! parameters, no mirrored copy of anything.
//!
//! That is why the gulf is between the legs rather than under a stack of them.
//! Both legs must keep a real drop, and a leg stacked over another leg fills the
//! lower one's gulf and takes its headroom; two legs either side of one gulf
//! each keep the full `fall`, and — the point — each looks straight into the
//! other's recesses.
//!
//! # What the zone adds, and why it has to
//!
//! [`crate::library::cliff_path()`] guarantees the ledge is one wide and its
//! niches one deep; it says nothing about what is beside the ledge, because a
//! rule only owns the box it is handed. So this program writes exactly three
//! things itself — the crag mass under each road, the air between them, and the
//! solid head the hairpin turns on — and calls the vocabulary for everything a
//! player touches. The head is mass and absence, which is a zone's own business;
//! it contains no encounter geometry.
//!
//! # Anchors
//!
//! Two legs means two sets, named at the include sites rather than derived from
//! the prefixes (`crate::compose` never qualifies an anchor):
//! `anchor/near-niche-<i>` / `anchor/near-niche-watch-<i>` on the way in,
//! `anchor/far-niche-<i>` / `anchor/far-niche-watch-<i>` on the way out.
//!
//! # Gates (`tests/zones.rs`)
//!
//! 1. **The road goes somewhere, and only along the ledges.** The standable-cell
//!    graph connects the near leg's entry to the far leg's exit, and with either
//!    ledge lane deleted it does not.
//! 2. **The drop is a drop**, from every ledge cell of **both** legs — each
//!    toward its own gulf face, which are opposite world directions. Teeth:
//!    `ledge_shelf`.
//! 3. **Every niche opens onto its ledge**, so the shove lands in the gulf.
//! 4. **Every niche on the far leg is visible from the near leg** — the
//!    fairness §4 K rests on, asserted with a count rather than described.
//!    Teeth: `gulf_screen`, a column down the middle of the gulf that leaves
//!    both roads walkable and both drops lethal and blinds every crossing
//!    sightline.
//!
//! Smallest region: X ≥ 2·3 + `sea`, Y ≥ `fall` + `niche_height` + 2, and each
//! leg longer than the zone is wide net of the gulf, or a leg piece is turned
//! across the zone — see the module note in [`super`].

use crate::block::BlockState;
use crate::compose::{AnchorRenames, entry};
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, Program, Reorient};
use crate::library::cliff_path;
use crate::library::{
    abs, abse, absp, all_of, alt_when, call, cmp, dim, fill, int, par, rel, reoriented, split, void,
};

use super::composed_renaming;

/// The prefix the leg the player walks **in** on is included under.
const NEAR: &str = "near";

/// The prefix the leg the player walks **out** on is included under.
const FAR: &str = "far";

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
/// niche exists for has to land in open air. Three is also what the sightline
/// gate's teeth need — a screen column with air on both sides of it, so
/// blinding the crossing does not accidentally take a drop away too.
pub const MIN_GULF: i64 = 3;

/// The narrowest band a leg is built in: `cliff_path`'s own ledge / recess /
/// backing minimum.
pub const MIN_BAND: i64 = 3;

/// The shortest head the hairpin will turn on, in cells of travel.
///
/// One cell is a corner a player clips; two is a landing. It is a *length*
/// along the zone, not a width — the head spans the whole cross-section, so it
/// always reaches from one ledge to the other.
pub const MIN_TURN: i64 = 2;

/// The Cliff Road.
///
/// Parameters: `sea` (how wide the gulf between the legs is), `fall` (how deep),
/// `turn_run` (how long the head the hairpin turns on is), two test knobs, and
/// every parameter of the two included [`cliff_path`]s under the `near/` and
/// `far/` prefixes (`near/spacing_min`, `far/watch_back`, …). Palette role:
/// `crag`, plus `near/rock` and `far/rock` — so the two legs can be styled
/// apart or together. (The corpse prop is not a role: see [`cliff_path`].)
///
/// The knobs, both off by default and both moving no block when they are:
/// `ledge_shelf` lays a shelf across the gulf one course under the roads, so the
/// drop gate can be shown failing; `gulf_screen` stands a one-cell column down
/// the middle of the gulf through the niche band, so the cross-leg sightline
/// gate can be. Neither touches a ledge, so each reds exactly one gate.
pub fn cliff_road() -> Program {
    let path = cliff_path();
    let zone = Program::new("bell_cliff_road", "cliff_road")
        .param("sea", MIN_GULF)
        .param("fall", MIN_DROP)
        .param("turn_run", 4)
        .param("ledge_shelf", 0)
        .param("gulf_screen", 0)
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
        // built into a switchback with a survivable drop beside it.
        .rule_alts(
            "road_plan",
            vec![alt_when(
                all_of(vec![
                    cmp(par("fall"), CmpOp::Ge, int(MIN_DROP)),
                    cmp(par("sea"), CmpOp::Ge, int(MIN_GULF)),
                    cmp(par("turn_run"), CmpOp::Ge, int(MIN_TURN)),
                    // Room for the gulf and the road band above it. The band's
                    // height is the *included* rules' requirement, read through
                    // their prefixed parameters rather than restated as a number
                    // that could drift away from them. Both legs are checked:
                    // they are separate parameters and a campaign may set one.
                    cmp(
                        dim(DimRef::Y),
                        CmpOp::Ge,
                        par("fall")
                            .arith(ArithOp::Add, par(&qualified(NEAR, "niche_height")))
                            .arith(ArithOp::Add, int(2)),
                    ),
                    cmp(
                        dim(DimRef::Y),
                        CmpOp::Ge,
                        par("fall")
                            .arith(ArithOp::Add, par(&qualified(FAR, "niche_height")))
                            .arith(ArithOp::Add, int(2)),
                    ),
                    // Two bands of ledge/recess/backing, one either side of the
                    // gulf.
                    cmp(
                        dim(DimRef::X),
                        CmpOp::Ge,
                        par("sea").arith(ArithOp::Add, int(2 * MIN_BAND)),
                    ),
                    // ...and each leg must be longer than it is wide, or its own
                    // frame turns it across the zone (see [`super`]). The
                    // non-gulf width over-approximates a single band, which is
                    // the safe direction to be wrong in.
                    cmp(
                        dim(DimRef::Z).arith(ArithOp::Sub, par("turn_run")),
                        CmpOp::Gt,
                        dim(DimRef::X).arith(ArithOp::Sub, par("sea")),
                    ),
                ]),
                // The head first: a split visits its pieces low to high, and the
                // hairpin is at local Z-min, where the near leg's travel ends.
                split(
                    Axis::Z,
                    vec![absp("turn_run"), rel(1)],
                    vec![call("turn_head"), call("legs")],
                ),
            )],
        )
        // --- the zone's own three facts ---------------------------------------
        // The head of the inlet: solid crag up to and including the roads' floor
        // course, open air above. That single course is what carries a player
        // from one ledge across to the other, and it is the only place the two
        // legs touch.
        .rule(
            "turn_head",
            split(
                Axis::Y,
                vec![abse(par("fall").arith(ArithOp::Add, int(1))), rel(1)],
                vec![fill("crag"), void()],
            ),
        )
        .rule(
            "legs",
            split(
                Axis::Y,
                vec![absp("fall"), rel(1)],
                vec![call("gulf"), call("shelf")],
            ),
        )
        // The road band: the far leg turned round, the gulf, the near leg. Each
        // leg gets half of what the gulf leaves, so widening the zone widens
        // both backings and never moves a ledge off its gulf face.
        .rule(
            "shelf",
            split(
                Axis::X,
                vec![rel(1), absp("sea"), rel(1)],
                vec![call("far_leg"), call("gulf_air"), call("near_leg")],
            ),
        )
        // The leg the player walks out on, **turned round**: same rule, same
        // parameters, a half-turn about the vertical. Its travel therefore runs
        // back up the zone and its ledge sits on the gulf face — the near leg's
        // ledge is at its band's low-X edge, and reversing local X puts this
        // one's at its band's high-X edge, which is the same gulf.
        .rule(
            "far_leg",
            reoriented(Reorient::KEEP.turned(), call(&entry(FAR, &path))),
        )
        .rule("near_leg", call(&entry(NEAR, &path)))
        // The air the two legs look across. The knob stands one column in the
        // middle of it — never against either ledge, so both drops survive and
        // only the sightline gate reds.
        .rule_alts(
            "gulf_air",
            vec![
                alt_when(cmp(par("gulf_screen"), CmpOp::Le, int(0)), void()),
                alt_when(
                    cmp(par("gulf_screen"), CmpOp::Ge, int(1)),
                    split(
                        Axis::X,
                        vec![abs(1), abs(1), rel(1)],
                        vec![void(), fill("crag"), void()],
                    ),
                ),
            ],
        )
        // What is under all that: crag under each road, nothing between them.
        .rule_alts(
            "gulf",
            vec![
                alt_when(
                    cmp(par("ledge_shelf"), CmpOp::Le, int(0)),
                    call("open_gulf"),
                ),
                // The knob. A shelf across the top course of the gulf — both
                // ledges are untouched and both roads still walk, so what the
                // drop gate catches is the missing drop and nothing else.
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
                vec![rel(1), absp("sea"), rel(1)],
                vec![fill("crag"), void(), fill("crag")],
            ),
        );
    composed_renaming(
        zone,
        &[
            (NEAR, &path, leg_anchors("near-niche", "near-niche-watch")),
            (FAR, &path, leg_anchors("far-niche", "far-niche-watch")),
        ],
    )
}

/// The anchor stems one leg carries.
///
/// Both legs are the same program, so both declare `niche` and `niche-watch`;
/// the seam never derives a name from a prefix, so the zone says which leg is
/// which. One helper rather than two literals at the include sites because the
/// two stems of a leg must stay in step — a leg whose recesses were renamed and
/// whose watch cells were not is a pairing no expansion-time collision would
/// catch.
fn leg_anchors(niche: &'static str, watch: &'static str) -> AnchorRenames<'static> {
    [("niche", niche), ("niche-watch", watch)]
        .into_iter()
        .collect()
}

/// A parameter of one included cliff path, by the name it answers to once it is
/// in this program.
fn qualified(prefix: &str, param: &str) -> String {
    format!("{prefix}{}{param}", crate::compose::SEPARATOR)
}
