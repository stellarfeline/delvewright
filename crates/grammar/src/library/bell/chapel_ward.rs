//! **Z4 — the Chapel Ward.** The hub: the place the keep's kitchen shaft drops
//! the player into, with a sealed shortcut beside the way out (REMAKE §3 Z4; §4
//! entries **L** and **F**, which the zone table in [`super`] names as the
//! hub's hardware).
//!
//! ```text
//!  seen from above, travel running down the page:
//!
//!   x: 0 ....... strip_depth ....... | ....... the mainline .......
//!      ┌───────────────────────────┬─────────────────────────────┐
//!      │  far room │ BAR │ near rm │ D │  the junction's lane    │  junction_run
//!      ├───────────────────────────┼─────────────────────────────┤
//!      │      solid margin         │  lane, with the hearth's    │  hearth_run
//!      │                           │  nook off its west side     │
//!      ├───────────────────────────┼─────────────────────────────┤
//!      │                           │                             │
//!      │      solid margin         │  the chute: landing, duct,  │  the rest
//!      │                           │  and the ledge stepped off  │
//!      └───────────────────────────┴─────────────────────────────┘
//!                                    ▲ travel: local Z-max -> Z-min
//!
//!  local Y:  the chute is the piece that is not flat — its entry ledge sits
//!            `chute/drop` blocks above the floor the rest of the zone stands on.
//! ```
//!
//! # The hub is topology, and this is the topology it is
//!
//! A hub is not a shape, it is a **junction with more than one thing hanging off
//! it** — which is exactly what the vocabulary had no way to say until
//! [`crate::library::tee_passage`] landed. Every other §5b rule walls both of its
//! side faces, so a composition was a chain and a chain has no hub in it. This
//! zone is the first one whose whole reason to exist is the branch:
//!
//! * the player arrives **falling**, off the keep's kitchen duct
//!   ([`crate::library::dumbwaiter`], §4 entry **L**) — the landing the zone is
//!   named for;
//! * one bay along, a nook off the west side of the lane carries the ward's
//!   rest point ([`crate::library::hearth_ward`], **BF3**): a detour, not a
//!   room on the route — the lane walks whether or not you take it;
//! * they walk out along the junction's lane
//!   ([`crate::library::tee_passage`]), which carries one doorway in its side;
//! * through that doorway is the sealed half of a souls shortcut
//!   ([`crate::library::far_side_bar`], §4 entry **F**, spec-0016 §2): a room
//!   they can enter, a barred door they cannot pass, and `anchor/unlock` on the
//!   far side of it for whoever comes the long way round.
//!
//! The zone writes two things of its own and no encounter geometry: the `X` cut
//! that carves the branch strip out of the box, and the plain `margin` fill that
//! is the rest of that strip. Both are the "mass no piece handed a sub-box can
//! know about" that licenses `cliff_road`'s crag — nothing a player touches.
//!
//! # Why the branch box is deeper than the junction is long
//!
//! `far_side_bar` opens with `z(Largest)` like everything else, so it turns its
//! travel — and therefore its wall — onto the longer horizontal axis of the box
//! it is handed. A shortcut's wall has to stand **across** the mainline, so the
//! branch box must be deeper (its `X`) than it is wide (its `Z`): `strip_depth >
//! junction_run`, guarded rather than assumed. That is the same box-shaping
//! discipline `docs/reference/grammar.md` §5c documents for the seam, used
//! deliberately instead of fought.
//!
//! # The hearth, and why it took a rule
//!
//! Z4 is the rest ward, and a rest point is `bonfire{anchor}` (spec-0016 §1) — a
//! campaign verb that needs an *anchor* to bind. No rule declared one, and a zone
//! program may not mint an anchor of its own: an anchor name is the campaign's
//! contract with a **rule**, and every anchor in every zone here comes from the
//! piece that built the cell. So the gap was never "a bonfire", it was a
//! mechanism, and [`crate::library::hearth_ward`] is it — somewhere off the road
//! with one declared focus and one way in. The zone composes it between the
//! landing and the junction: the player falls in, steps aside to rest, and walks
//! out past the shortcut they cannot yet open.
//!
//! # Missing (REMAKE §3 Z4)
//!
//! Nothing.
//!
//! # Gates (`tests/zones.rs`)
//!
//! 1. **The hub is a route, and only downward.** Entry reaches exit under
//!    walk-and-fall; exit does not reach entry under the plain step. Teeth:
//!    `chute/rescue_ladder` paired with a short `chute/drop`, the piece's own.
//! 2. **The shortcut is sealed, and it is the bar that seals it.** The mainline
//!    reaches the branch's near room and stops there; `chute/…`-independent
//!    teeth `shortcut/unbarred` open exactly that doorway.
//! 3. **The branch is reached through the junction's doorway and nowhere else.**
//!    Teeth: `junction/sealed` fills the doorway — the branch goes unreachable
//!    while the mainline still walks.
//! 4. **Nothing on the mainline was turned**, and the branch was turned *on
//!    purpose*: the mainline's anchors run in travel order down the zone's own
//!    axis and face along it, and the branch's face across it.
//! 5. **The hearth is reachable from the hub's own route, and off it**: the
//!    zone reaches `anchor/hearth` under walk-and-fall, and deleting the nook
//!    leaves the hub still crossing. Teeth: `hearth/mouth_sealed`.

use crate::block::BlockState;
use crate::compose::entry;
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, Expr, Program, Reorient};
use crate::library::{
    absp, all_of, alt_when, call, cmp, dim, dumbwaiter, far_side_bar, fill, hearth_ward, par, rel,
    reoriented, split_exact, tee_passage,
};

use super::composed;

/// The prefix the kitchen duct is included under.
const CHUTE: &str = "chute";
/// The prefix the rest ward's own nook is included under.
const HEARTH: &str = "hearth";
/// The prefix the junction — a `tee_passage` — is included under.
const JUNCTION: &str = "junction";
/// The prefix the sealed shortcut is included under.
const SHORTCUT: &str = "shortcut";

/// The Chapel Ward.
///
/// Parameters: `strip_depth` (how far off the mainline the branch strip runs,
/// and therefore how deep the shortcut's two rooms are), `junction_run` and
/// `hearth_run` (their own lengths along the zone; the chute takes the rest, so
/// a longer zone is a longer approach and never a differently-shaped junction),
/// plus every parameter of the four included pieces under the `chute/`,
/// `hearth/`, `junction/` and `shortcut/` prefixes — including the knobs the
/// gates are shown red with, `chute/rescue_ladder`, `chute/drop`,
/// `shortcut/unbarred`, `junction/sealed` and `hearth/mouth_sealed`. Palette
/// role: `margin`, the zone's own inert mass.
pub fn chapel_ward() -> Program {
    let chute = dumbwaiter();
    let hearth = hearth_ward();
    let junction = tee_passage();
    let shortcut = far_side_bar();
    let zone = Program::new("bell_chapel_ward", "chapel_ward")
        .param("strip_depth", 9)
        .param("junction_run", 8)
        .param("hearth_run", 8)
        .role("margin", BlockState::with("deepslate", [("axis", "y")]))
        // --- frame -----------------------------------------------------------
        .rule(
            "chapel_ward",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("ward_plan"),
            ),
        )
        // One alternative, no `otherwise`. The first clause is what makes the
        // shortcut a shortcut rather than a wall across the hub (see the module
        // note); the other two are the frame constraint of [`super`], measured
        // against the **mainline's** width rather than the zone's, because that
        // is the box each mainline piece is actually handed.
        .rule_alts(
            "ward_plan",
            vec![alt_when(
                all_of(vec![
                    cmp(par("strip_depth"), CmpOp::Gt, par("junction_run")),
                    cmp(par("junction_run"), CmpOp::Gt, mainline_width()),
                    cmp(par("hearth_run"), CmpOp::Gt, mainline_width()),
                    cmp(chute_run(), CmpOp::Gt, mainline_width()),
                ]),
                split_exact(
                    Axis::X,
                    vec![absp("strip_depth"), rel(1)],
                    vec![call("branch_strip"), call("mainline")],
                ),
            )],
        )
        // --- the branch strip: the shortcut, and inert rock for the rest -------
        // The fill is mass, not a gate. What makes the junction's doorway the
        // only way into the branch is that `far_side_bar` walls its own two side
        // faces — the same §5b property the seam limit came from — and that is
        // asserted on the model rather than argued here.
        .rule(
            "branch_strip",
            split_exact(
                Axis::Z,
                vec![absp("junction_run"), rel(1)],
                vec![call(&entry(SHORTCUT, &shortcut)), fill("margin")],
            ),
        )
        // --- the mainline ------------------------------------------------------
        // Pieces run low to high and travel runs high to low, so this list is the
        // player's route backwards: the junction they walk out along is declared
        // first, and the ledge they step off last.
        .rule(
            "mainline",
            split_exact(
                Axis::Z,
                vec![absp("junction_run"), absp("hearth_run"), rel(1)],
                vec![
                    call(&entry(JUNCTION, &junction)),
                    call(&entry(HEARTH, &hearth)),
                    call(&entry(CHUTE, &chute)),
                ],
            ),
        );
    composed(
        zone,
        &[
            (CHUTE, &chute),
            (HEARTH, &hearth),
            (JUNCTION, &junction),
            (SHORTCUT, &shortcut),
        ],
    )
}

/// The width of the box every mainline piece is handed: the zone's own width
/// less the branch strip cut off the side of it.
fn mainline_width() -> Expr {
    dim(DimRef::X).arith(ArithOp::Sub, par("strip_depth"))
}

/// What the chute gets: whatever length the junction and the rest ward leave.
fn chute_run() -> Expr {
    dim(DimRef::Z)
        .arith(ArithOp::Sub, par("junction_run"))
        .arith(ArithOp::Sub, par("hearth_run"))
}
