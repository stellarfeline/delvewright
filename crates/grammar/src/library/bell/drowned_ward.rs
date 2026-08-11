//! **Z3 — the Drowned Lower Ward.** A flooded crossing under a keeper's
//! gatehouse, the lower ward's open fight beyond it, and a shortcut door
//! beside the way out (REMAKE §3 Z3; §4 entries **T**, **E** and **F**).
//!
//! ```text
//!  seen from above, travel running down the page:
//!
//!   x: 0 ...... strip_depth ...... | .............. the mainline ..............
//!      ┌──────────────────────────┬──────────────────────────────────────────┐
//!      │                          │  flood │berm│ flood      ^ causeway       │  the rest
//!      │        solid margin      │  ═════ gatehouse ═════   ^   (its post)   │
//!      ├──────────────────────────┼──────────────────────────────────────────┤
//!      │ far room │ BAR │ near rm │D│  the junction's lane   ^ tee_passage    │  junction_run
//!      ├──────────────────────────┼──────────────────────────────────────────┤
//!      │        solid margin      │  open ward floor         ^ elite_ground   │  ward_run
//!      └──────────────────────────┴──────────────────────────────────────────┘
//!                                   ▲ travel: local Z-max -> Z-min
//!
//!  local Y:  the causeway is the piece that is not flat — its berm stands
//!            `ward/rise` blocks over the floor the rest of the zone walks on.
//! ```
//!
//! Three pieces in the order the player meets them, plus the branch. They come
//! in single file along a berm with drowned ward either side of it, watched the
//! whole way by the keeper on the gatehouse; they pass **under** the gatehouse
//! rather than onto it; out the far side is a junction with a barred door in its
//! flank; and past that is the lower ward's own floor, with the way out on
//! either side of the fight.
//!
//! # What this zone could not be written until now
//!
//! `causeway` was a **terminus**. Its far face was the guard post's own plinth,
//! solid from the ward floor to the post's floor, so a zone could put nothing
//! after it and would end at a wall if it put it last. The rule now exposes
//! `berm_gate`, which carries the berm's own column through the station at berm
//! height, and the zone turns it on — that is the whole of what Z3 was waiting
//! for, and it is a knob on the piece rather than geometry this program writes.
//!
//! # The one seam arithmetic this zone owes
//!
//! Every other piece in the vocabulary stands a body on its own floor, one cell
//! up. The causeway stands one on a berm `rise` cells up, and a seam is only
//! walkable in **both** directions when the step across it is at most one — the
//! `connected` walk's whole rule. So the zone pins `ward/rise` to
//! [`BERM_STEP`] rather than inheriting the piece's default: at the default of
//! 3 the seam is a two-block drop, which walk-and-fall would cross downhill and
//! nothing would cross back. A zone whose shortcut exists to be walked the other
//! way cannot afford a one-way seam it did not declare.
//!
//! The branch is the construction Z4 and Z6 already use, for the reason
//! `docs/reference/grammar.md` §5c records: a shortcut is beside the route, not
//! on it, so the zone cuts a strip off its own side, hands the branch box to
//! `far_side_bar` shaped deeper-than-wide, and puts a `tee_passage` in the run
//! beside it to carry the one doorway that opens onto it.
//!
//! Naming: `causeway` and `elite_ground` both declare `anchor/elite`, and they
//! are two genuinely different actors — the keeper who watches the crossing, and
//! whatever holds the lower ward — so the zone says which is which at the
//! include site and the keeper's becomes `anchor/keeper-elite`
//! ([`crate::compose::include_renaming`]).
//!
//! # What this zone deliberately does not claim
//!
//! **No zone-length flank bypass.** `elite_ground` promises open floor either
//! side of its circle, and Z0 and Z6 re-bind that claim across the whole zone.
//! Here it would be false by construction and it would be dishonest to assert a
//! weaker version of it quietly: the causeway is a **one-wide** crossing, so no
//! band of floor runs the length of this zone at all. The arena's bypass is
//! still real, and it is bound where it is true — inside the arena's own run
//! (`tests/zones.rs`).
//!
//! # Missing (REMAKE §3 Z3)
//!
//! Nothing. **T**, **E** and **F** are all composed above.
//!
//! # Gates (`tests/zones.rs`)
//!
//! 1. **The ward is a route, both ways**, under the plain ±1 step — this zone
//!    has no one-way hardware, so it owes the stronger of the two models.
//!    Teeth: `ward/berm_gate = 0` puts the plinth back and the zone is severed,
//!    while the causeway's own crossing still walks — so what went red is the
//!    way past the gatehouse, not the gatehouse.
//! 2. **The keeper still commands the crossing after composition** — a
//!    sightline from `anchor/keeper-elite` to every standable berm cell in the
//!    campaign's box, not the piece fixture's. Teeth: `ward/obstruct`.
//! 3. **The fight is still optional**, bound where it is true: two lanes past
//!    the circle across the arena's own run. Teeth: `ring/seal_flank`.
//! 4. **The shortcut is sealed, reachable, and reached through one doorway.**
//!    Teeth: `shortcut/unbarred` and `junction/sealed`.

use crate::block::BlockState;
use crate::compose::{AnchorRenames, entry};
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, Expr, Program, Reorient};
use crate::library::{
    absp, all_of, alt_when, call, causeway, cmp, dim, elite_ground, far_side_bar, fill, par, rel,
    reoriented, split_exact, tee_passage,
};

use super::composed_renaming;

/// The prefix the flooded crossing — a `causeway` — is included under.
/// The zones **design box** — the region the campaign expands this program
/// over (REMAKE §3).
///
/// Twenty cells of lower ward, twenty of junction with the shortcuts strip beside
/// it, and the remaining twenty of flooded crossing. Nineteen-wide mainline and
/// twenty-one-deep strip; ten tall because the causeway stacks a berm, a
/// gatehouse lane and the keepers own headroom.
///
/// It lives here, beside the program, because the box a zone is designed for is
/// a fact about the zone: every guard in the program below is written against
/// it, and a caller that has the program but not the box has half a zone. The
/// gates in `tests/zones.rs` and the sweep driver both read it from here.
pub const REGION: [u32; 3] = [40, 10, 60];

const WARD: &str = "ward";
/// The prefix the lower ward's arena is included under.
const RING: &str = "ring";
/// The prefix the shortcut's junction — a `tee_passage` — is included under.
const JUNCTION: &str = "junction";
/// The prefix the barred shortcut door is included under.
const SHORTCUT: &str = "shortcut";

/// How far the berm stands over the floor every other piece of this zone walks
/// on — and therefore the height of the step at the seam between them.
///
/// One, because `connected` steps at most one block: two would make the seam a
/// drop crossable only downhill. See the module note.
pub const BERM_STEP: i64 = 2;

/// The Drowned Lower Ward.
///
/// Parameters: `ward_run` and `junction_run` (how much of the zone's length the
/// arena and the junction take; the crossing gets the rest, so a longer zone is
/// a longer causeway and never a differently-shaped arena), `strip_depth` (how
/// far off the mainline the shortcut's own strip runs), plus every parameter of
/// the four included pieces under the `ward/`, `ring/`, `junction/` and
/// `shortcut/` prefixes — including the knobs the gates are shown red with,
/// `ward/berm_gate`, `ward/obstruct`, `ring/seal_flank`, `shortcut/unbarred`
/// and `junction/sealed`. Palette role: `margin`, the zone's own inert mass.
pub fn drowned_ward() -> Program {
    // The crossing is the only piece the zone configures, and both settings are
    // facts about the seam rather than taste: the post has to be passable at all
    // (`berm_gate`), and the berm has to meet its neighbours' floor within one
    // step (`rise`). Both stay exposed as `ward/…`, so a campaign can still
    // dial them — and gate 1's teeth are exactly that.
    let mut ward = causeway();
    ward.set_param("berm_gate", 1)
        .expect("the causeway carries a berm gate");
    ward.set_param("rise", BERM_STEP)
        .expect("the causeway carries a berm rise");
    let ring = elite_ground();
    let junction = tee_passage();
    let shortcut = far_side_bar();
    let zone = Program::new("bell_drowned_ward", "drowned_ward")
        .param("ward_run", 20)
        .param("junction_run", 20)
        .param("strip_depth", 21)
        .role("margin", BlockState::simple("deepslate"))
        // --- frame -----------------------------------------------------------
        .rule(
            "drowned_ward",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("lower_ward_plan"),
            ),
        )
        // One alternative, no `otherwise`. The first clause is what keeps the
        // shortcut a branch rather than a wall across the ward; the rest are the
        // frame constraint of [`super`] applied to each mainline piece in turn,
        // measured against the **mainline's** width rather than the zone's,
        // because that is the box each piece is actually handed. The last is the
        // crossing's, which takes whatever length the other two leave.
        .rule_alts(
            "lower_ward_plan",
            vec![alt_when(
                all_of(vec![
                    cmp(par("strip_depth"), CmpOp::Gt, par("junction_run")),
                    cmp(par("ward_run"), CmpOp::Gt, mainline_width()),
                    cmp(par("junction_run"), CmpOp::Gt, mainline_width()),
                    cmp(crossing_run(), CmpOp::Gt, mainline_width()),
                ]),
                split_exact(
                    Axis::X,
                    vec![absp("strip_depth"), rel(1)],
                    vec![call("branch_strip"), call("mainline")],
                ),
            )],
        )
        // --- the branch strip: the shortcut, and inert rock either side --------
        // The fill is mass, not a gate. What makes the junction's doorway the
        // only way into the branch is that `far_side_bar` walls its own two side
        // faces, and that is asserted on the model by plugging the doorway
        // rather than argued here.
        .rule(
            "branch_strip",
            split_exact(
                Axis::Z,
                vec![absp("ward_run"), absp("junction_run"), rel(1)],
                vec![
                    fill("margin"),
                    call(&entry(SHORTCUT, &shortcut)),
                    fill("margin"),
                ],
            ),
        )
        // --- the mainline ------------------------------------------------------
        // Pieces run low to high and travel runs high to low, so this list is
        // the player's route read backwards: the lower ward's floor is declared
        // first and the flooded crossing they arrive along last.
        .rule(
            "mainline",
            split_exact(
                Axis::Z,
                vec![absp("ward_run"), absp("junction_run"), rel(1)],
                vec![
                    call(&entry(RING, &ring)),
                    call(&entry(JUNCTION, &junction)),
                    call(&entry(WARD, &ward)),
                ],
            ),
        );
    // `causeway` and `elite_ground` both declare `anchor/elite`. They are two
    // different actors — the keeper of the crossing and whatever holds the lower
    // ward — so the zone writes down which is which rather than letting a prefix
    // derive it.
    let keeper: AnchorRenames<'_> = [("elite", "keeper-elite")].into_iter().collect();
    composed_renaming(
        zone,
        &[
            (WARD, &ward, keeper),
            (RING, &ring, AnchorRenames::new()),
            (JUNCTION, &junction, AnchorRenames::new()),
            (SHORTCUT, &shortcut, AnchorRenames::new()),
        ],
    )
}

/// The width of the box every mainline piece is handed: the zone's own width
/// less the branch strip cut off the side of it.
fn mainline_width() -> Expr {
    dim(DimRef::X).arith(ArithOp::Sub, par("strip_depth"))
}

/// What the crossing gets: whatever length the arena and the junction leave.
fn crossing_run() -> Expr {
    dim(DimRef::Z)
        .arith(ArithOp::Sub, par("ward_run"))
        .arith(ArithOp::Sub, par("junction_run"))
}
