//! **Z7 — the Bell Tower.** The zone that is *climbed*: a stair up the tower's
//! solid base, BF5's rope room at the head of it, the loft over that, the boss
//! threshold, the Bellkeeper's ring, and the counterweight shaft that drops out
//! of the whole thing (REMAKE §3 Z7; §4 entries **R**, **M**, **E** and **L**).
//!
//! ```text
//!  seen from above, travel running down the page:
//!
//!   x: 0 ...... strip_depth ...... | .............. the mainline ..............
//!      ┌──────────────────────────┬──────────────────────────────────────────┐
//!      │        solid margin      │  the boss ring          ^ elite_ground   │  ring_run
//!      ├──────────────────────────┼──────────────────────────────────────────┤
//!      │        solid margin      │  the boss door          ^ threshold_motif│  door_run
//!      ├──────────────────────────┼──────────────────────────────────────────┤
//!      │  LIFT SHAFT (full Y)  │D││  the landing's lane     ^ tee_passage    │  tee_run
//!      ├──────────────────────────┼──────────────────────────────────────────┤
//!      │        solid margin      │  the loft               ^ rafter_hall    │  loft_run
//!      ├──────────────────────────┼──────────────────────────────────────────┤
//!      │        solid margin      │  BF5, the rope room     ^ hearth_ward    │  hearth_run
//!      ├──────────────────────────┼──────────────────────────────────────────┤
//!      │        solid margin      │  the ascent             ^ stair_flight   │  the rest
//!      └──────────────────────────┴──────────────────────────────────────────┘
//!                                   ▲ travel: local Z-max -> Z-min
//!
//!  local Y:  the four upper pieces stand on the tower's own plinth, `climb - 1`
//!            courses of solid mass; the flight is the only thing that reaches
//!            them, and the shaft's strip is the only thing that passes through.
//! ```
//!
//! # Both of this zone's recorded blockers were stale, and how they fell is the
//! whole of its design
//!
//! **"Nothing in the vocabulary climbs" — closed by
//! [`crate::library::stair_flight`].** A flight is a walled shaft with a level
//! landing at each end, gated on the plain ±1 walk in *both* directions. This
//! zone is where it earns its keep: the mainline is a chain like every other
//! zone's, except that one link in it is 8 blocks taller at its far end than at
//! its near one.
//!
//! **`counterweight_lift` — struck, not blocked.** The lift is campaign JSON
//! (spec-0031): runtime state, region fill/clear, teleport-by-region. Nothing
//! moves, so the grammar never has to express motion. What it does have to
//! express is the hole — [`crate::library::lift_shaft`], the one rule this zone
//! needed and the prefab spec-0031 records the library as lacking.
//!
//! # The one seam arithmetic this zone owes, and why it is declared and guarded
//! rather than derived in place
//!
//! Every other zone's pieces share one floor. Here four of them stand a storey
//! up, on mass this program writes — the licensed kind, "a fact about the zone's
//! box that no piece of vocabulary can know". The plinth is exactly as thick as
//! the flight climbs, or the upper storey's floor and the flight's head landing
//! are not the same surface and the zone is two rooms with a step between them.
//!
//! A flight's rise is a fact about the box it is handed, not a knob: it lays one
//! tread every `tread` cells until its run is spent, so the head landing sits
//! `(flight_run - 2·landing_run) / tread` courses above the foot's own floor.
//! That expression is [`treads`], and it is written out here rather than trusted
//! to prose.
//!
//! It cannot, however, be *spent* where the plinth is cut. A split size is
//! evaluated in the scope it is written in, and `dim(Z)` inside the upper
//! storey's own box is the upper run, not the zone's length — the first draft of
//! this zone cut a plinth of −4 courses and was refused by the interpreter,
//! which is the cheapest possible way to learn that. So the zone declares
//! [`CLIMB`] and the plan **guards the identity** `climb == treads()` at the one
//! scope where the whole length is visible. A campaign that dials
//! `flight/tread`, `flight/landing_run` or any of the four runs gets a refusal
//! naming the rule, never a tower with a step at its own landing.
//!
//! The same number is what the shaft's `sill` has to equal, for the same reason
//! and with the same guard: a shaft does not otherwise care where its lowest
//! station is, and this one's has to be the floor the landing doorway opens onto.
//!
//! # The two doorways that have to meet, and the guard that was NOT written
//!
//! The landing is a [`crate::library::tee_passage`] whose side doorway opens on
//! the shaft's own landing doorway, and the two pieces are turned 90° to each
//! other: the tee's is centred along the zone's `Z`, the shaft's along its own
//! local `X`, which *is* the zone's `Z`. That looked like it wanted a parity
//! guard — on an even run a `split_exact` gives the odd block to the earliest
//! share, so surely the two centres land a cell apart.
//!
//! **They do not, and the guard was deleted rather than kept as decoration.**
//! Both doorways are placed by the same `Rounding::Start` rule counting from the
//! same end of the same run, so they agree at every length; measured at
//! `tee_run` 20 and 21, both doorways landed on the same cell and the landing
//! was reachable either way. A guard whose red never happens is a green that
//! binds to nothing, which is the failure mode this project spends the most
//! effort on, so what stands in its place is a measurement: the zone's own gate
//! walks from the entry face to the shaft's landing sill, and the drift it
//! catches is a real one — moving `lift_shaft`'s lane off centre (`rel(1)` →
//! `abs(1)` on its first split) reds it with "the tower cannot reach its own
//! lift landing" while every other gate stays green.
//!
//! # BF5, and the gap that closed between one round and the next
//!
//! This zone shipped its first round with one entry named as missing: the rope
//! room. A rest point is `bonfire{anchor}` (spec-0016 §1), no rule declared an
//! anchor for one, and the honest thing was to name it rather than mint an
//! anchor a zone has no right to. [`crate::library::hearth_ward`] landed in the
//! very next round for Z4's own hearth, so the gap is closed here by composing
//! it — a lane with a sheltered nook, `anchor/hearth` inside, sitting at the head
//! of the stair where the climb ends. The rule knows nothing about fire; what it
//! guarantees is somewhere off the road, reachable one way, with one declared
//! focus, and that is exactly what a rest point binds to.
//!
//! # Still not here, and it cannot be
//!
//! The lift's **far** station stands in
//! whatever zone the ride lands in (REMAKE says Z4), and a zone program builds
//! one box. What Z7 ships is the shaft head — the top station, the drop below
//! it, and a bottom that is lethal rather than open, so the piece is complete on
//! its own terms rather than depending on a neighbour that does not exist yet.
//!
//! # Gates (`tests/zones.rs`)
//!
//! 1. **The tower is a route, and the route climbs.** Entry to exit under the
//!    plain ±1 step — this zone has no one-way hardware on its mainline, so it
//!    owes the stronger of the two walks — and the exit face stands [`RISE`]
//!    blocks above the entry face, measured off the model rather than off the
//!    anchors. Control: `boulder_stair` in the same box, which is flat, so a
//!    green here cannot be a flat chain wearing a stair's anchors. Teeth:
//!    `flight/broken_step`, which raises one tread by an extra course and severs
//!    the zone while every piece is still standing.
//! 2. **Every loft perch is still visible from the loft's own door** after
//!    composition, on a hall the zone made 19 wide. Teeth: `loft/span_beams`.
//! 3. **The boss threshold is mandatory** (§4 entry **M**, the dual of the fog
//!    gate): cut the doorband's own slice and the ring is unreachable while the
//!    loft still walks.
//! 4. **The Bellkeeper's fight keeps a lane on each side** (§4 entry **E**),
//!    bound inside the ring's own run. Teeth: `ring/seal_flank`.
//! 5. **The shaft is entered through one doorway, and it only drops.** The pit
//!    is reachable from the mainline under walk-and-fall and the mainline is not
//!    reachable from the pit under the plain step — the L family's pair, the
//!    same predicates `drop_shaft` and `dumbwaiter` are gated on. Teeth:
//!    `tee/sealed`, which plugs the landing doorway and leaves the shaft
//!    unreachable while the tower still walks. The same gate asserts the tee's
//!    doorway and the shaft's landing are one column — the seam between two
//!    pieces turned 90° to each other — and that is what an off-centre lane
//!    reds.
//! 6. **The plinth arithmetic is guarded, not hoped**: a `shaft/sill` that
//!    disagrees with the flight's own rise is a refusal naming the rule.
//!
//! BF5 carries no gate of its own here, and that is deliberate rather than an
//! omission: `hearth_ward`'s three claims — the lane is still a chain segment,
//! the focus is a detour rather than a corridor, exactly one way in — are all
//! about the piece's own box, and `tests/staging.rs` binds them there. What
//! composition could break is that the rope room severs the climb, and gate 1
//! walks straight through it.

use crate::block::BlockState;
use crate::compose::entry;
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, Expr, Program, Reorient};
use crate::library::{
    abse, absp, all_of, alt_when, call, cmp, dim, elite_ground, fill, hearth_ward, int, lift_shaft,
    par, rafter_hall, rel, reoriented, split_exact, stair_flight, tee_passage, threshold_motif,
};

use super::composed;

/// The prefix the Bellkeeper's ring is included under.
const RING: &str = "ring";
/// The prefix the boss threshold is included under.
const DOOR: &str = "door";
/// The prefix the lift landing's junction — a `tee_passage` — is included under.
const TEE: &str = "tee";
/// The prefix the loft is included under.
const LOFT: &str = "loft";
/// The prefix BF5's rope room — a `hearth_ward` — is included under.
const HEARTH: &str = "hearth";
/// The prefix the ascent is included under.
const FLIGHT: &str = "flight";
/// The prefix the counterweight shaft is included under.
const SHAFT: &str = "shaft";

/// How many courses the flight climbs in the fixture box — the number of treads
/// it lays, and therefore the local `Y` its head landing stands at.
///
/// A **declared** number rather than a derived one, and the difference matters:
/// a `split`'s size is evaluated in the scope it is written in, and the scope
/// that owns the plinth is the upper storey's own box, whose `Z` is the upper
/// run and not the zone's. So the derivation [`treads`] cannot be spent where
/// the plinth is cut. What the plan does instead is state the identity as a
/// guard — `climb == treads()`, at the one scope where the zone's whole length
/// is visible — so the declared number is checked against the flight's real rise
/// on every expansion, and a box that would climb differently is refused rather
/// than built with a step at the seam.
pub const CLIMB: i64 = 9;

/// How far the tower's upper storey stands over its own ground floor: what a
/// body gains between the two end faces of the zone.
///
/// One less than [`CLIMB`], because a flight's foot landing is already a course
/// up — see [`plinth`]. Restated as a constant because the gate reads it off the
/// model (the `Y` difference between the zone's two end faces) rather than off
/// the anchors, and a number the test derived from the program's own arithmetic
/// would prove nothing.
pub const RISE: i32 = CLIMB as i32 - 1;

/// The lowest station of the shaft: the upper storey's own standing level, which
/// is [`CLIMB`] courses above the tower's floor.
///
/// This is the value `shaft/sill` is pinned to below, and the plan's guard is
/// what keeps the two in step.
pub const SILL: i64 = CLIMB;

/// The Bell Tower.
///
/// Parameters: `ring_run`, `door_run`, `tee_run` and `loft_run` (how much of the
/// zone's length each upper piece takes — `hearth_run` is BF5's rope room; the
/// flight gets the rest, so a longer zone is a taller climb and never a
/// differently-shaped ring), `strip_depth`
/// (how far off the mainline the shaft's own strip runs), plus every parameter
/// of the seven included pieces under the `ring/`, `door/`, `tee/`, `loft/`,
/// `hearth/`, `flight/` and `shaft/` prefixes — including the knobs the gates are shown red
/// with, `flight/broken_step`, `loft/span_beams`, `ring/seal_flank`,
/// `tee/sealed` and `shaft/sill`. Palette roles: `plinth` (the mass the upper
/// storey stands on) and `margin` (the inert rock the shaft's strip is cut out
/// of).
pub fn bell_tower() -> Program {
    let ring = elite_ground();
    let door = threshold_motif();
    let tee = tee_passage();
    let loft = rafter_hall();
    let hearth = hearth_ward();
    let flight = stair_flight();
    // The one piece this zone configures, and it is a fact about the seam rather
    // than taste: the shaft's lowest station has to be the upper storey's own
    // floor, or the landing doorway opens into the plinth. The knob stays
    // exposed as `shaft/sill` so a campaign can still dial it — and the plan's
    // guard is what turns a dial into a refusal instead of a silent hole.
    let mut shaft = lift_shaft();
    shaft
        .set_param("sill", SILL)
        .expect("the lift shaft carries a sill");
    let zone = Program::new("bell_bell_tower", "tower")
        .param("ring_run", 20)
        .param("door_run", 20)
        .param("tee_run", 21)
        .param("loft_run", 20)
        .param("hearth_run", 20)
        .param("strip_depth", 22)
        .param("climb", CLIMB)
        .role("plinth", BlockState::simple("deepslate_bricks"))
        .role("margin", BlockState::simple("deepslate"))
        // --- frame -----------------------------------------------------------
        .rule(
            "tower",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("tower_plan"),
            ),
        )
        // One alternative, no `otherwise`. Clause 1 keeps the shaft a branch
        // rather than a wall across the tower; clauses 2-6 are the frame
        // constraint of [`super`] applied to each mainline piece in turn,
        // measured against the **mainline's** width because that is the box each
        // is actually handed; clause 7 ties the declared [`CLIMB`] to the
        // flight's real rise and clause 8 keeps `Z` and not `Y` the thing that
        // bounds it, which is what makes [`plinth`] exact; clauses 9-10 are the
        // shaft's own seam. Every one of them has been watched refusing; a
        // clause whose red never happens was written here once and deleted (see
        // the module note on the two doorways).
        .rule_alts(
            "tower_plan",
            vec![alt_when(
                all_of(vec![
                    cmp(par("strip_depth"), CmpOp::Gt, par("tee_run")),
                    cmp(par("ring_run"), CmpOp::Gt, mainline_width()),
                    cmp(par("door_run"), CmpOp::Gt, mainline_width()),
                    cmp(par("tee_run"), CmpOp::Gt, mainline_width()),
                    cmp(par("loft_run"), CmpOp::Gt, mainline_width()),
                    cmp(par("hearth_run"), CmpOp::Gt, mainline_width()),
                    cmp(flight_run(), CmpOp::Gt, mainline_width()),
                    cmp(par("climb"), CmpOp::Eq, treads()),
                    cmp(
                        dim(DimRef::Y),
                        CmpOp::Ge,
                        par("climb")
                            .arith(ArithOp::Add, par("flight/head"))
                            .arith(ArithOp::Add, int(1)),
                    ),
                    cmp(par("shaft/sill"), CmpOp::Eq, par("climb")),
                    cmp(
                        dim(DimRef::Y).arith(ArithOp::Sub, par("shaft/sill")),
                        CmpOp::Eq,
                        par("shaft/storey"),
                    ),
                ]),
                split_exact(
                    Axis::X,
                    vec![absp("strip_depth"), rel(1)],
                    vec![call("branch_strip"), call("mainline")],
                ),
            )],
        )
        // --- the branch strip: the shaft, and inert rock either side -----------
        // The shaft runs the strip's **whole height**, which is the one thing
        // that makes it a hub-opener: it passes clean through the plinth the
        // upper storey stands on. The fill either side is mass, not a gate —
        // what makes the tee's doorway the only way in is that `lift_shaft`
        // walls its own three other faces, and that is asserted on the model by
        // plugging the doorway rather than argued here.
        .rule(
            "branch_strip",
            split_exact(
                Axis::Z,
                vec![abse(upper_reach()), absp("tee_run"), rel(1)],
                vec![fill("margin"), call(&entry(SHAFT, &shaft)), fill("margin")],
            ),
        )
        // --- the mainline ------------------------------------------------------
        // Two things, not five: the storey the tower carries, and the flight up
        // to it. Pieces run low to high and travel runs high to low, so the
        // flight — where the player comes in — is last.
        .rule(
            "mainline",
            split_exact(
                Axis::Z,
                vec![abse(upper_run()), rel(1)],
                vec![call("upper_storey"), call(&entry(FLIGHT, &flight))],
            ),
        )
        // The tower's own mass, and the storey standing on it. This is the only
        // block this zone writes that a player ever stands on top of.
        .rule(
            "upper_storey",
            split_exact(
                Axis::Y,
                vec![abse(plinth()), rel(1)],
                vec![fill("plinth"), call("upper_chain")],
            ),
        )
        // The player's route read backwards again: the ring is declared first
        // and the loft they arrive in last.
        .rule(
            "upper_chain",
            split_exact(
                Axis::Z,
                vec![
                    absp("ring_run"),
                    absp("door_run"),
                    absp("tee_run"),
                    absp("loft_run"),
                    rel(1),
                ],
                vec![
                    call(&entry(RING, &ring)),
                    call(&entry(DOOR, &door)),
                    call(&entry(TEE, &tee)),
                    call(&entry(LOFT, &loft)),
                    call(&entry(HEARTH, &hearth)),
                ],
            ),
        );
    // No rename: the seven pieces declare seven disjoint sets of stems — `elite`,
    // `threshold-narrate`, `branch-door`, `hall-door`/`perch-<i>`,
    // `stair-foot`/`stair-head`/`stair-step-<i>`, `hearth`, and the three
    // `lift-*`. A zone
    // that needed one would write it out here, as Z3 and Z6 do.
    composed(
        zone,
        &[
            (RING, &ring),
            (DOOR, &door),
            (TEE, &tee),
            (LOFT, &loft),
            (HEARTH, &hearth),
            (FLIGHT, &flight),
            (SHAFT, &shaft),
        ],
    )
}

/// The width of the box every mainline piece is handed: the zone's own width
/// less the branch strip cut off the side of it.
fn mainline_width() -> Expr {
    dim(DimRef::X).arith(ArithOp::Sub, par("strip_depth"))
}

/// How much of the zone's length the upper storey takes.
fn upper_run() -> Expr {
    par("ring_run")
        .arith(ArithOp::Add, par("door_run"))
        .arith(ArithOp::Add, par("tee_run"))
        .arith(ArithOp::Add, par("loft_run"))
        .arith(ArithOp::Add, par("hearth_run"))
}

/// How far along the strip the shaft's own slice starts: the runs of the two
/// upper pieces that sit past the landing.
fn upper_reach() -> Expr {
    par("ring_run").arith(ArithOp::Add, par("door_run"))
}

/// What the flight gets: whatever length the five upper pieces leave.
fn flight_run() -> Expr {
    dim(DimRef::Z).arith(ArithOp::Sub, upper_run())
}

/// How many courses the flight climbs — the number of treads it lays in the run
/// this zone hands it, which is also the `Y` its head landing stands at.
///
/// [`crate::library::stair_flight`] spends `landing_run` at each end of its box
/// and one `tread` of run per course of rise, and stops when either runs out.
/// The plan's guard is what keeps `Y` from being the one that runs out first, so
/// here only the run matters.
fn treads() -> Expr {
    flight_run()
        .arith(
            ArithOp::Sub,
            par("flight/landing_run").arith(ArithOp::Mul, int(2)),
        )
        .arith(ArithOp::Div, par("flight/tread"))
}

/// The tower's plinth: as many courses of mass as the flight climbs, less one.
///
/// Written against the declared [`CLIMB`] and not against [`treads`], because a
/// split size is evaluated in its own scope and this one is cut inside the upper
/// storey's box, where `dim(Z)` is the upper run rather than the zone's length.
/// The plan's guard is what ties the two together.
///
/// One less, because a flight *arrives on* its head landing rather than stepping
/// up onto it — the landing's floor is the last course the run laid, not a new
/// one — while every other rule in the vocabulary lays its own floor at the
/// bottom of the box it is handed. So the upper storey's box starts one course
/// below the level a body stands on, and the two surfaces are the same surface.
fn plinth() -> Expr {
    par("climb").arith(ArithOp::Sub, int(1))
}
