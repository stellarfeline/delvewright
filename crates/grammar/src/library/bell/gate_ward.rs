//! **Z2 — the Gatehouse and Outer Ward.** The timed portcullis with the bay you
//! read it from, the boulder stair with the release you can jam before you
//! commit to it, the sally port barred from its far side, the boss threshold,
//! and the spill shaft down to the ward below (REMAKE §3 Z2; §4 entries **O**,
//! **A**, **D**, **W**+**S**, **F**, **M** and **L** — all seven).
//!
//! ```text
//!  seen from above, travel running down the page:
//!
//!   x: 0 ...... strip_depth ...... | .............. the mainline ..............
//!      ┌──────────────────────────┬──────────────────────────────────────────┐
//!      │        solid margin      │  corridor | SPAN | approach | BAY        │  gate_run
//!      ├──────────────────────────┼──────────────────────────────────────────┤
//!      │        solid margin      │  room | alcove row | WALL | approach     │  door_run
//!      ├──────────────────────────┼──────────────────────────────────────────┤
//!      │        solid margin      │  the run | R▓ the stand at the head      │  stand_run
//!      ├──────────────────────────┼──────────────────────────────────────────┤
//!      │        solid margin      │  rough | RUN | rough | pockets           │  stair_run
//!      ├──────────────────────────┼──────────────────────────────────────────┤
//!      │ far room │ BAR │ near rm │D│  the sally port's lane                 │  tee_run
//!      ├──────────────────────────┼──────────────────────────────────────────┤
//!      │        solid margin      │  room | CURTAIN | room                   │  motif_run
//!      ├──────────────────────────┴──────────────────────────────────────────┤
//!      │  ................ the spill shaft: ledge, then landing ............ │  shaft_run
//!      └─────────────────────────────────────────────────────────────────────┘
//!                                   ▲ travel: local Z-max -> Z-min
//!
//!  local Y:  everything above the shaft stands on a plinth `shaft/drop` blocks
//!            thick — the outer ward's own rock. See below.
//! ```
//!
//! Six pieces on the mainline in the order the player meets them, a seventh in
//! the branch beside it, and two facts of the zone's own. They arrive from the
//! cliff road into the bay's end of the passage, read the portcullis before
//! crossing it, meet the ward's threshold and the body waiting beside it, come
//! out at the head of the boulder stair where the release sits in a stand off
//! the lane, run the worn tread with its pockets, pass a barred sally port they
//! can look into and not open, cross the boss threshold — and leave by stepping
//! off the spill shaft's ledge into the ward below, which is the way to **BF2**
//! and cannot be climbed back.
//!
//! # The plinth: what a zone owes a piece that ends one level down
//!
//! [`crate::library::drop_shaft`] builds its entry ledge `drop` blocks up and
//! its landing at the floor, so a zone that puts it anywhere but its `Z`-max end
//! has to raise everything *above* the drop to meet that ledge. Z6 avoided the
//! question by falling into the zone; Z2 cannot — the design has the player walk
//! in at the gate and leave down the shaft.
//!
//! So the zone writes a plinth: a `margin` slab `shaft/drop` thick under the
//! whole upper ward, which puts the upper floor at exactly the height of the
//! shaft's own entry floor. That is the "mass no piece handed a sub-box can know
//! about" that licenses `cliff_road`'s crag — inert rock a player never touches,
//! and the same clause under which Z3, Z4 and Z6 fill their branch strips.
//!
//! Its thickness is **read from the piece** (`par("shaft/drop")`) rather than
//! restated as a zone constant, so the two cannot drift: dial `shaft/drop` and
//! the plinth follows it, which is what makes the one-way gate's teeth — a short
//! drop plus `shaft/rescue_ladder` — mean anything at all. The one thing the
//! zone does guard is that a plinth leaves an upper ward to build in
//! ([`MIN_UPPER`]); a zero-height remainder would be a silent nothing rather
//! than a refusal.
//!
//! **Where the gate below actually bites, measured rather than assumed.** Build
//! the plinth one block *thin* and every gate stays green — correctly, because a
//! one-block mismatch is a step, and a step is walkable in both directions, so
//! nothing the zone claims has changed. At **two** blocks it is a drop the plain
//! walk cannot climb: the ward stops being a route and five gates go red at once
//! (`the_gatehouse_is_a_route_down_and_not_back_up`, the span cut, the sally
//! port, and the zone-wide walkability). That is the honest statement of the
//! tolerance, and it is written here because the tempting version — "the plinth
//! must equal the drop exactly or the seam is a wall" — is false in one direction
//! and would have made a green look stronger than it is.
//!
//! # The branch
//!
//! The sally port is the construction `docs/reference/grammar.md` §5c records
//! and Z3, Z4 and Z6 already use: a strip cut off the side of the zone's box,
//! inert rock except where the branch goes, and a [`crate::library::tee_passage`]
//! in the run beside it carrying the one doorway that opens onto it. The branch
//! box must be **deeper than the junction is long**, guarded rather than assumed,
//! or `far_side_bar`'s own `z(Largest)` turns its wall along the mainline instead
//! of across it.
//!
//! Naming: `watch_bay` and `far_side_bar` both declare `anchor/gate`, and they
//! are two genuinely different gates — the portcullis the bay watches, and the
//! sally port's barred door — so the zone says which is which at the include
//! site and the bar's becomes `anchor/sally-gate`
//! ([`crate::compose::include_renaming`]), exactly as Z6 does.
//!
//! # Missing (REMAKE §3 Z2)
//!
//! Nothing. **D** was the last entry with no rule at all
//! ([`crate::library::disarm_stand`] is it), and the other five were built rules
//! waiting for a zone round.
//!
//! # Gates (`tests/zones.rs`)
//!
//! 1. **The hazard cannot be walked round.** The zone connects end to end, and
//!    with the span's cells deleted it does not — re-walked under the fall model,
//!    because a zone with a drop in it has an edge the plain walk never had.
//! 2. **The bay still sees the whole span after composition.** Teeth:
//!    `gate/obstruct`.
//! 3. **The alcove is blind from the whole zone** — every standable cell of the
//!    ward above the threshold, a far larger set than the piece's own gate
//!    examined. Teeth: `door/expose`.
//! 4. **The ward is a route down, and not back up.** Entry reaches exit under
//!    walk-and-fall; exit does not reach entry under the plain step. Teeth:
//!    `shaft/rescue_ladder` paired with a short `shaft/drop`, which shortens the
//!    plinth with it.
//! 5. **The boulder release cannot be worked from the stair's own run** — bound
//!    against the composed model's stair cells, not the stand's own fixture.
//!    Teeth: `stand/release_in_lane`.
//! 6. **The sally port is sealed, reachable, and reached through one doorway.**
//!    Teeth: `sally/unbarred` and `tee/sealed`.
//! 7. **Nothing on the mainline was turned**, and the branch was turned on
//!    purpose.

use crate::block::BlockState;
use crate::compose::{AnchorRenames, entry};
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, Expr, Program, Reorient};
use crate::library::{
    abse, absp, all_of, alt_when, ambush_door, boulder_stair, call, cmp, dim, disarm_stand,
    drop_shaft, far_side_bar, fill, par, rel, reoriented, split_exact, tee_passage,
    threshold_motif, watch_bay,
};

use super::composed_renaming;

/// The prefix the gated passage — a `watch_bay` — is included under.
const GATE: &str = "gate";
/// The prefix the ward's threshold is included under.
const DOOR: &str = "door";
/// The prefix the boulder release's stand is included under.
const STAND: &str = "stand";
/// The prefix the worn-tread hazard lane is included under.
const STAIR: &str = "stair";
/// The prefix the sally port's junction — a `tee_passage` — is included under.
const TEE: &str = "tee";
/// The prefix the sally port's barred door is included under.
const SALLY: &str = "sally";
/// The prefix the boss threshold's motif is included under.
const MOTIF: &str = "motif";
/// The prefix the spill shaft is included under.
const SHAFT: &str = "shaft";

/// The shallowest upper ward the zone will raise on its plinth: a floor, the
/// three cells of interior the thinnest piece in the chain asks for, and a
/// ceiling.
///
/// The pieces refuse for themselves if they are handed less than they need, and
/// loudly. What this guard is for is the case below that: a plinth as thick as
/// the box leaves a remainder of nothing, and nothing is written silently.
pub const MIN_UPPER: i64 = 5;

/// The Gatehouse and Outer Ward.
///
/// Parameters: `shaft_run` (how much of the zone's length the spill shaft takes,
/// at the exit end and one level down), `motif_run`, `tee_run`, `stair_run`,
/// `stand_run` and `door_run` (the upper ward's pieces; the gated passage takes
/// whatever is left, so a longer zone is a longer approach and never a
/// differently-shaped stair), `strip_depth` (how far off the mainline the sally
/// port's own strip runs), plus every parameter of the eight included pieces
/// under the `gate/`, `door/`, `stand/`, `stair/`, `tee/`, `sally/`, `motif/`
/// and `shaft/` prefixes — including the knobs the gates are shown red with:
/// `gate/obstruct`, `door/expose`, `stand/release_in_lane`, `sally/unbarred`,
/// `tee/sealed`, `shaft/rescue_ladder` and `shaft/drop`. Palette role: `margin`,
/// the zone's own inert mass — the plinth and the branch strip's fill.
pub fn gate_ward() -> Program {
    let gate = watch_bay();
    let door = ambush_door();
    let stand = disarm_stand();
    let stair = boulder_stair();
    let tee = tee_passage();
    let sally = far_side_bar();
    let motif = threshold_motif();
    let shaft = drop_shaft();
    let zone = Program::new("bell_gate_ward", "gate_ward")
        .param("shaft_run", 12)
        .param("motif_run", 10)
        .param("tee_run", 10)
        .param("stair_run", 16)
        .param("stand_run", 10)
        .param("door_run", 10)
        .param("strip_depth", 11)
        .role("margin", BlockState::simple("deepslate"))
        // --- frame -----------------------------------------------------------
        .rule(
            "gate_ward",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("ward_plan"),
            ),
        )
        // One alternative, no `otherwise`. The first clause keeps the sally port
        // a branch rather than a wall across the ward; the second is the plinth's
        // own (see the module note); the rest are the frame constraint of
        // [`super`] applied to each mainline piece in turn, measured against the
        // **mainline's** width rather than the zone's, because that is the box
        // each piece is actually handed. The last is the gated passage's, which
        // takes whatever length the others leave.
        .rule_alts(
            "ward_plan",
            vec![alt_when(
                all_of(vec![
                    cmp(par("strip_depth"), CmpOp::Gt, par("tee_run")),
                    cmp(
                        dim(DimRef::Y),
                        CmpOp::Ge,
                        par("shaft/drop").arith(ArithOp::Add, int_min_upper()),
                    ),
                    cmp(par("shaft_run"), CmpOp::Gt, mainline_width()),
                    cmp(par("motif_run"), CmpOp::Gt, mainline_width()),
                    cmp(par("tee_run"), CmpOp::Gt, mainline_width()),
                    cmp(par("stair_run"), CmpOp::Gt, mainline_width()),
                    cmp(par("stand_run"), CmpOp::Gt, mainline_width()),
                    cmp(par("door_run"), CmpOp::Gt, mainline_width()),
                    cmp(gate_run(), CmpOp::Gt, mainline_width()),
                ]),
                // Low `Z` to high `Z` is the reverse of travel: the level the
                // player leaves by is declared first.
                split_exact(
                    Axis::Z,
                    vec![absp("shaft_run"), rel(1)],
                    vec![call("spill_level"), call("upper_ward")],
                ),
            )],
        )
        // --- the spill shaft's own slice, at the zone's exit end ---------------
        // The strip is not a branch here — there is nothing off the mainline at
        // this end — so it is plain mass, the same inert rock the plinth is.
        .rule(
            "spill_level",
            split_exact(
                Axis::X,
                vec![absp("strip_depth"), rel(1)],
                vec![fill("margin"), call(&entry(SHAFT, &shaft))],
            ),
        )
        // --- the plinth, and the ward standing on it ---------------------------
        // `shaft/drop` is the piece's own parameter, read through the composed
        // program's namespace: the plinth is exactly as thick as the fall, by
        // construction rather than by a constant somebody has to keep in step.
        .rule(
            "upper_ward",
            split_exact(
                Axis::Y,
                vec![abse(par("shaft/drop")), rel(1)],
                vec![fill("margin"), call("upper_plan")],
            ),
        )
        .rule(
            "upper_plan",
            split_exact(
                Axis::X,
                vec![absp("strip_depth"), rel(1)],
                vec![call("branch_strip"), call("mainline")],
            ),
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
                vec![absp("motif_run"), absp("tee_run"), rel(1)],
                vec![fill("margin"), call(&entry(SALLY, &sally)), fill("margin")],
            ),
        )
        // --- the upper ward's own chain ----------------------------------------
        // Read backwards this is the player's route: the bay they arrive at, the
        // threshold, the stair's head, the worn tread, the sally port's junction,
        // and the boss door they leave through.
        .rule(
            "mainline",
            split_exact(
                Axis::Z,
                vec![
                    absp("motif_run"),
                    absp("tee_run"),
                    absp("stair_run"),
                    absp("stand_run"),
                    absp("door_run"),
                    rel(1),
                ],
                vec![
                    call(&entry(MOTIF, &motif)),
                    call(&entry(TEE, &tee)),
                    call(&entry(STAIR, &stair)),
                    call(&entry(STAND, &stand)),
                    call(&entry(DOOR, &door)),
                    call(&entry(GATE, &gate)),
                ],
            ),
        );
    // `watch_bay` and `far_side_bar` both declare `anchor/gate`. They are two
    // different gates — the portcullis the bay watches, and the barred door of
    // the shortcut — so the zone writes down which is which rather than letting a
    // prefix derive it.
    let sally_names: AnchorRenames<'_> = [("gate", "sally-gate")].into_iter().collect();
    composed_renaming(
        zone,
        &[
            (GATE, &gate, AnchorRenames::new()),
            (DOOR, &door, AnchorRenames::new()),
            (STAND, &stand, AnchorRenames::new()),
            (STAIR, &stair, AnchorRenames::new()),
            (TEE, &tee, AnchorRenames::new()),
            (SALLY, &sally, sally_names),
            (MOTIF, &motif, AnchorRenames::new()),
            (SHAFT, &shaft, AnchorRenames::new()),
        ],
    )
}

/// The width of the box every mainline piece is handed: the zone's own width
/// less the branch strip cut off the side of it.
fn mainline_width() -> Expr {
    dim(DimRef::X).arith(ArithOp::Sub, par("strip_depth"))
}

/// What the gated passage gets: whatever length the shaft and the five upper
/// pieces below it leave.
fn gate_run() -> Expr {
    dim(DimRef::Z)
        .arith(ArithOp::Sub, par("shaft_run"))
        .arith(ArithOp::Sub, par("motif_run"))
        .arith(ArithOp::Sub, par("tee_run"))
        .arith(ArithOp::Sub, par("stair_run"))
        .arith(ArithOp::Sub, par("stand_run"))
        .arith(ArithOp::Sub, par("door_run"))
}

/// [`MIN_UPPER`] as an expression.
fn int_min_upper() -> Expr {
    Expr::int(MIN_UPPER)
}
