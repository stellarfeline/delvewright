//! **Z6 — the Cistern Deep.** The player does not walk into this zone, they
//! fall into it (REMAKE §3 Z6; §4 entries **L**, the dart gallery's **O**,
//! **X** and **E**).
//!
//! ```text
//!  seen from above, travel running down the page:
//!
//!   x: 0 ...... strip_depth ...... | .............. the mainline ..............
//!      ┌──────────────────────────┬──────────────────────────────────────────┐
//!      │                          │  landing | ledge          ^ drop_shaft   │  the shaft
//!      │                          │  bay | span | lane        ^ watch_bay    │  gallery_run
//!      │        solid margin      │  grate row wall           ^ broken_grate │  vent_run
//!      ├──────────────────────────┤──────────────────────────────────────────┤
//!      │ far room │ BAR │ near rm │D│  the sally port's lane   ^ tee_passage  │  sally_run
//!      ├──────────────────────────┼──────────────────────────────────────────┤
//!      │        solid margin      │  open arena               ^ elite_ground │  arena_run
//!      └──────────────────────────┴──────────────────────────────────────────┘
//!                                   ▲ travel: local Z-max -> Z-min
//!
//!  local Y:  the shaft is the only piece that is not flat — its entry ledge sits
//!            `shaft/drop` blocks above the floor every other piece stands on.
//! ```
//!
//! Five pieces of vocabulary in the order the player meets them, and two facts
//! of the zone's own. The player steps off the ledge at local `Z`-max and lands
//! on the cistern floor `shaft/drop` blocks below; from the landing the dart
//! gallery's bay is the first thing in reach, so the volley is read before it is
//! crossed; past the span the cistern's own wall carries the broken grate; then
//! a barred sally port they can look into and not open; and last the deep's
//! floor is the elite's open ground, with the way out on either side of it.
//!
//! # The sally port (**F**), and the two things the zone had to write for it
//!
//! `far_side_bar` is the sealed half of a souls shortcut (spec-0016 §2), and a
//! shortcut is a **branch**: laid in the piece run it would seal the cistern
//! instead of sitting beside it. So the zone cuts a strip off the side of its own
//! box and hands the branch's box to the bar, with a
//! [`crate::library::tee_passage`] in the run beside it carrying the one doorway
//! that opens onto it — the construction `docs/reference/grammar.md` §5c records
//! as the closure of seam limit 3.
//!
//! That means this zone, unlike the four it shipped with, does write blocks: the
//! `X` cut that carves the strip out, and the plain `margin` fill that is the
//! rest of the strip. Both are the "mass no piece handed a sub-box can know
//! about" that licenses `cliff_road`'s crag — inert rock, nothing a player
//! touches. The fill is not what keeps the branch sealed; `far_side_bar` walls
//! its own two side faces, and the gate proves the doorway is the only way in by
//! plugging it.
//!
//! Two consequences worth stating, because both are guarded rather than assumed:
//!
//! * the branch box must be **deeper than the sally run is long**
//!   (`strip_depth > sally_run`), or the bar's own `z(Largest)` turns its wall
//!   along the mainline instead of across it;
//! * every mainline piece's frame guard is measured against the **mainline's**
//!   width, not the zone's, because that is the box it is actually handed. The
//!   sally lane is full mainline width for the same reason the others are, and
//!   that reason is measured: hand the junction a 5-wide box and fill the other
//!   14 of its slice, and the zone's flank-route count drops from 2 to 1 — the
//!   solid remainder walls off the arena's own east band, which gate 3 owes.
//!
//! Naming: `watch_bay` and `far_side_bar` both declare `anchor/gate`, and they
//! are two genuinely different gates, so the zone says which is which at the
//! include site — the bar's becomes `anchor/sally-gate`
//! ([`crate::compose::include_renaming`]).
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
//! Nothing. **F** was Z6's last gap and it is built above; the two reasons it
//! could not be composed — a shared `anchor/gate` and a seam with no junction —
//! are the two seam limits `docs/reference/grammar.md` §5c records as closed.
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
//! 4. **The sally port is sealed, reachable, and reached through one doorway.**
//!    The mainline still crosses the zone with the bar standing; it reaches the
//!    branch's near room and stops there; and plugging the tee's own doorway
//!    makes the branch unreachable while the cistern still runs. Teeth:
//!    `sally/unbarred` and `tee/sealed`.
//! 5. **Nothing on the mainline was turned**, and the branch was turned *on
//!    purpose*: mainline anchors in travel order down the zone's own axis and
//!    facing along it, the branch's facing across it, and a piece run shorter
//!    than the mainline is wide refused rather than turned.

use crate::block::BlockState;
use crate::compose::{AnchorRenames, entry};
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, Expr, Program, Reorient};
use crate::library::{
    absp, all_of, alt_when, broken_grate, call, cmp, dim, drop_shaft, elite_ground, far_side_bar,
    fill, par, rel, reoriented, split_exact, tee_passage, watch_bay,
};

use super::composed_renaming;

/// The prefix the elite ground is included under.
/// The zones **design box** — the region the campaign expands this program
/// over (REMAKE §3).
///
/// Twenty cells each of arena, sally port, grate wall and dart gallery, and the
/// remaining twenty of spill shaft. The mainline is nineteen wide because the
/// arenas flank margins ask for it, and the branch strip twenty-one; ten tall
/// because the shafts ledge sits four blocks over a room that still needs its
/// own headroom.
///
/// It lives here, beside the program, because the box a zone is designed for is
/// a fact about the zone: every guard in the program below is written against
/// it, and a caller that has the program but not the box has half a zone. The
/// gates in `tests/zones.rs` and the sweep driver both read it from here.
pub const REGION: [u32; 3] = [40, 10, 100];

const ARENA: &str = "arena";
/// The prefix the sally port's junction — a `tee_passage` — is included under.
const TEE: &str = "tee";
/// The prefix the sally port's barred door is included under.
const SALLY: &str = "sally";
/// The prefix the broken-grate wall is included under.
const VENT: &str = "vent";
/// The prefix the dart gallery — a `watch_bay` — is included under.
const GALLERY: &str = "gallery";
/// The prefix the spill shaft is included under.
const SHAFT: &str = "shaft";

/// The Cistern Deep.
///
/// Parameters: `arena_run`, `sally_run`, `vent_run` and `gallery_run` (how much
/// of the zone's length each of those four pieces takes; the shaft gets the
/// rest, so a longer zone is a deeper fall run, never a differently-shaped
/// arena), `strip_depth` (how far off the mainline the sally port's own strip
/// runs), plus every parameter of the six included pieces under the `arena/`,
/// `tee/`, `sally/`, `vent/`, `gallery/` and `shaft/` prefixes — including the
/// knobs the gates are shown red with, `shaft/rescue_ladder`, `shaft/drop`,
/// `arena/seal_flank`, `sally/unbarred` and `tee/sealed`.
pub fn cistern_deep() -> Program {
    let arena = elite_ground();
    let tee = tee_passage();
    let sally = far_side_bar();
    let vent = broken_grate();
    let gallery = watch_bay();
    let shaft = drop_shaft();
    let zone = Program::new("bell_cistern_deep", "cistern_deep")
        .param("arena_run", 20)
        .param("sally_run", 20)
        .param("vent_run", 20)
        .param("gallery_run", 20)
        .param("strip_depth", 21)
        .role("margin", BlockState::simple("deepslate"))
        // --- frame -----------------------------------------------------------
        .rule(
            "cistern_deep",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("deep_plan"),
            ),
        )
        // One alternative, no `otherwise`. The first clause is what keeps the
        // sally port a branch rather than a wall (see the module note); the rest
        // are the frame constraint of [`super`] applied to each mainline piece in
        // turn, measured against the **mainline's** width rather than the zone's,
        // because that is the box each piece is actually handed. The last one is
        // the shaft's, which takes whatever length the other four leave.
        .rule_alts(
            "deep_plan",
            vec![alt_when(
                all_of(vec![
                    cmp(par("strip_depth"), CmpOp::Gt, par("sally_run")),
                    cmp(par("arena_run"), CmpOp::Gt, mainline_width()),
                    cmp(par("sally_run"), CmpOp::Gt, mainline_width()),
                    cmp(par("vent_run"), CmpOp::Gt, mainline_width()),
                    cmp(par("gallery_run"), CmpOp::Gt, mainline_width()),
                    cmp(shaft_run(), CmpOp::Gt, mainline_width()),
                ]),
                split_exact(
                    Axis::X,
                    vec![absp("strip_depth"), rel(1)],
                    vec![call("branch_strip"), call("mainline")],
                ),
            )],
        )
        // --- the branch strip: the sally port, and inert rock either side ------
        // The fill is mass, not a gate. What makes the tee's doorway the only way
        // into the branch is that `far_side_bar` walls its own two side faces,
        // and that is asserted on the model by plugging the doorway rather than
        // argued here.
        .rule(
            "branch_strip",
            split_exact(
                Axis::Z,
                vec![absp("arena_run"), absp("sally_run"), rel(1)],
                vec![fill("margin"), call(&entry(SALLY, &sally)), fill("margin")],
            ),
        )
        // --- the mainline ------------------------------------------------------
        // Pieces run low to high and travel runs high to low, so this list is the
        // player's route read backwards: the deep's floor is declared first and
        // the ledge they step off last.
        .rule(
            "mainline",
            split_exact(
                Axis::Z,
                vec![
                    absp("arena_run"),
                    absp("sally_run"),
                    absp("vent_run"),
                    absp("gallery_run"),
                    rel(1),
                ],
                vec![
                    call(&entry(ARENA, &arena)),
                    call(&entry(TEE, &tee)),
                    call(&entry(VENT, &vent)),
                    call(&entry(GALLERY, &gallery)),
                    call(&entry(SHAFT, &shaft)),
                ],
            ),
        );
    // `watch_bay` and `far_side_bar` both declare `anchor/gate`. They are two
    // different gates — one is the hazard the bay watches, one is the barred
    // door of the shortcut — so the zone writes down which is which rather than
    // letting a prefix derive it.
    let sally_names: AnchorRenames<'_> = [("gate", "sally-gate")].into_iter().collect();
    composed_renaming(
        zone,
        &[
            (ARENA, &arena, AnchorRenames::new()),
            (TEE, &tee, AnchorRenames::new()),
            (SALLY, &sally, sally_names),
            (VENT, &vent, AnchorRenames::new()),
            (GALLERY, &gallery, AnchorRenames::new()),
            (SHAFT, &shaft, AnchorRenames::new()),
        ],
    )
}

/// The width of the box every mainline piece is handed: the zone's own width
/// less the branch strip cut off the side of it.
fn mainline_width() -> Expr {
    dim(DimRef::X).arith(ArithOp::Sub, par("strip_depth"))
}

/// What the shaft gets: whatever length the other four mainline pieces leave.
fn shaft_run() -> Expr {
    dim(DimRef::Z)
        .arith(ArithOp::Sub, par("arena_run"))
        .arith(ArithOp::Sub, par("sally_run"))
        .arith(ArithOp::Sub, par("vent_run"))
        .arith(ArithOp::Sub, par("gallery_run"))
}
