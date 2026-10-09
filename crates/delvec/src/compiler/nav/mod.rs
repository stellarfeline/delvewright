//! Compile-time navigation over the solved voxel grid (spec-0008 addendum).
//!
//! The compiler owns the assembled geometry, so two v0.4 verbs are made
//! collision-safe *by construction* here rather than trusting downstream runtime
//! behaviour (CLAUDE.md "no hacks at any layer"):
//!
//! - **`move-npc`** walks a real path planned by A* over the placed-world block
//!   data (pieces + the solver's socket seals). Waypoints step only through
//!   passable cells and stand on solid ground — no wall-clipping (owner playtest
//!   finding). An unroutable move is [`DW_MOVE_UNROUTABLE`] (`DW0307`), a compile
//!   error, not a runtime glitch.
//! - **`cutscene`** camera dollies are validated to pass only through non-solid
//!   blocks — both the authored waypoint polyline and the client-rendered
//!   keyframe chords ([`crate::compiler::camera`]). Cameras fly (exempt from walkability)
//!   but must not clip a solid; a violation is [`DW_CUTSCENE_CLIP`] (`DW0308`).
//!   Shots are also held to the angular-rate budget ([`DW_CAMERA_SPIN`],
//!   `DW0347`).
//!
//! **Gate cells are passable.** A `ResolvedAnchor::Gate` region is a
//! compiler-managed openable threshold (an `open-gate` effect fills it with air),
//! never a wall, so its cells are treated as passable for both planning and the
//! camera check. Modelling a sealed gate as an obstacle would wrongly forbid an
//! NPC from walking through a doorway the campaign opens.
//!
//! Determinism (ADR-0006): the solid set is a `BTreeSet`, the A* frontier breaks
//! ties on `(f, g, cell)` in a fixed order, and neighbour expansion order is
//! fixed — same DSL + seed → identical waypoints.

mod route;
mod world;
pub use route::*;
pub use world::*;

mod actor;
mod ambush;
mod checkpoint;
mod cutscene;
mod furniture;
mod hazard;
mod horizon;
mod lane;
mod leave;
mod lethal;
mod npc;
mod respawn;
mod sea;
mod shortcut;
mod staging;
mod stealth;
mod timed_gate;
mod trap;
mod view;
mod wave;
pub use actor::*;
pub use ambush::*;
pub use checkpoint::*;
pub use cutscene::*;
pub use furniture::*;
pub use hazard::*;
pub use horizon::*;
pub use lane::*;
pub use leave::*;
pub use lethal::*;
pub use npc::*;
pub use respawn::*;
pub use sea::*;
pub use shortcut::*;
pub use staging::*;
pub use stealth::*;
pub use timed_gate::*;
pub use trap::*;
pub use view::*;
pub use wave::*;

#[cfg(test)]
mod testkit;

use delvewright_dsl::Verb;

use delvewright_dsl::QuestEffect;

use crate::compiler::plan::Plan;
use delvewright_dsl::Diagnostic;
use delvewright_dsl::{DwCode, ExitTier};

delvewright_dsl::dw_code! {
    /// `DW0991`: a forced critical-path leg climbs a **climbable the world does
    /// not keep** (spec-0099) — a ladder whose back is not a sturdy face, a vine
    /// with no face held, a weeping, twisting or cave vine grown from nothing.
    ///
    /// The model reads such a block as air, because the game removes it at the
    /// first shape update it receives. Derived by counterfactual, exactly like
    /// [`DW_LETHAL_ON_CRITICAL_PATH`] and its family: the leg is re-routed with
    /// every unheld climbable credited as if it hung, and if THAT world routes,
    /// the missing hold closed the leg and the blocks are named with the hold
    /// each lacks. A code of its own rather than a [`DW_CRITICAL_UNROUTABLE`]
    /// variant because the author is looking at a ladder in the piece and must be
    /// told why it is not there — not sent to hunt a wedged doorway.
    pub const DW_CLIMB_UNHELD: DwCode = DwCode::new("DW0991", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0307`: a `move-npc` destination unreachable by any walkable path from the
    /// NPC's position over the assembled geometry.
    pub const DW_MOVE_UNROUTABLE: DwCode = DwCode::new("DW0307", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0308`: a `cutscene` camera dolly path that passes through a solid block.
    pub const DW_CUTSCENE_CLIP: DwCode = DwCode::new("DW0308", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0347`: a `cutscene` shot whose aim sweeps faster than the angular budget
    /// ([`crate::compiler::camera::MAX_AIM_DEG_PER_TICK`], 6 °/tick = 120 °/s) — a pan that
    /// fast at 20 Hz is nausea-tier and provably bad *before* it ships. Typical
    /// cause: a `look_at` subject too close to a fast dolly. See the camera dossier
    /// (`docs/notes/camera-dossier.md` §1) for the budget's derivation.
    pub const DW_CAMERA_SPIN: DwCode = DwCode::new("DW0347", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0311`: a consecutive pair of player-visited critical-path anchors that no
    /// walkable path connects over the assembled geometry (with no inter-area
    /// transport between them) — the player would be stranded. Turns the whole
    /// "assembled seams aren't walkable" bug class — a prefab regen that wedges a
    /// doorway shut or opens a void gap, which otherwise only a runtime bot catches
    /// — into a compile error.
    pub const DW_CRITICAL_UNROUTABLE: DwCode = DwCode::new("DW0311", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0510`: the party's only route to a critical-path objective runs through a
    /// declared **lethal volume** (DSL v0.10, spec-0031).
    ///
    /// A volume that kills whatever enters it is, for a route, a volume no route may
    /// enter — so its cells are impassable in the navigation world, exactly as a
    /// `close-gate`'s sealed region is solid, and a forced leg that has no other way
    /// through fails. It is a code of its own rather than a [`DW_CRITICAL_UNROUTABLE`]
    /// variant because the *fix* is different in kind: the geometry is fine and the
    /// prefab is fine, and the author needs to be told which volume they must move,
    /// shrink or route around — not sent to look for a wedged doorway that does not
    /// exist. Derived from a counterfactual: the leg is re-routed over the identical
    /// world with lethality removed, and the volumes covering that route are named.
    pub const DW_LETHAL_ON_CRITICAL_PATH: DwCode = DwCode::new("DW0510", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0317`: a gate the placed world authors **shut at world-load** blocks a
    /// forced critical-path leg, and nothing the party is guaranteed to do opens it
    /// before that leg.
    ///
    /// The defect it closes is a modelling default, not a missing lint. The occupancy
    /// model cleared every gate region unconditionally — "assume the gate the player
    /// needs is opened" — so a gate's state in the static model was a function of what
    /// *sealed* it and never of what *opened* it. That default can only ever fail to
    /// notice an obstruction, never invent one, and the mistake an author actually
    /// makes is forgetting to open a door. A campaign whose one `open-gate` is
    /// missing then compiles clean and the runtime bot says *"No path to the goal!"*
    /// — a symptom, naming nothing. `tests/gate_world_load_seal.rs` holds that
    /// red→green pair on the in-repo `hello-world` fixture.
    ///
    /// A code of its own rather than a [`DW_CRITICAL_UNROUTABLE`] variant for exactly
    /// the reason `DW0510` is one: the geometry is right, the prefab is right, and the
    /// repair is a missing `open-gate` on a **named** anchor. Derived the same way, by
    /// counterfactual — the leg is re-routed over the identical world with the
    /// world-load gate seals lifted, and the gates covering that route are named.
    ///
    /// No surface is being asked for: "the delve must be completable" is ADR-0005,
    /// day one.
    pub const DW_GATE_NEVER_OPENED: DwCode = DwCode::new("DW0317", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0544`: a forced critical-path leg depends on standing where a runtime region
    /// write leaves **fluid** — water or lava.
    ///
    /// A `fill-region` / `close-gate` / shortcut seal whose block is a fluid does not
    /// build floor: it replaces whatever was in the box with something a body sinks
    /// through ([`crate::compiler::plan::RegionWrite::Flood`]). Sibling of
    /// [`DW_LETHAL_ON_CRITICAL_PATH`] and derived the same way — the leg is re-routed
    /// over the identical world with every runtime fluid fill treated as solid, and if
    /// *that* world routes, the fluid is what closed the leg and the boxes are named.
    ///
    /// A code of its own rather than a [`DW_CRITICAL_UNROUTABLE`] variant for the
    /// reason `DW0510` is: the prefab is innocent. The author is looking at a box they
    /// filled on purpose and needs to be told that filling it with water is what took
    /// the footing away — not sent hunting for a wedged doorway that is not there.
    pub const DW_FLUID_FILL_ON_CRITICAL_PATH: DwCode = DwCode::new("DW0544", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0546`: a forced critical-path leg stands on footing laid by a beat the party
    /// is **not forced to play** — a plank dropped by a sprung trap, a stair repaired by
    /// a bought offer, a bridge lowered from a shortcut's far side.
    ///
    /// The general form of an asymmetry that runs through every runtime write: a solid
    /// block answers two questions at once, and only one of them is conservative when
    /// the firing is uncertain. *Is the party blocked?* — assume it happened; assuming a
    /// wall can only make the proof harder. *Can the party stand there?* — assume it did
    /// not; assuming floor is what makes the proof easier, and easier is the direction
    /// that ships. The model therefore carries an unforced fill as impassable AND not
    /// floor ([`World::with_unforced`]), which is the pointwise-worst of the two futures
    /// and sound in both.
    ///
    /// Derived by counterfactual, exactly like [`DW_LETHAL_ON_CRITICAL_PATH`],
    /// [`DW_GATE_NEVER_OPENED`] and [`DW_FLUID_FILL_ON_CRITICAL_PATH`]: the leg is
    /// re-routed over the identical world with every unforced fill credited as ordinary
    /// floor ([`RegionState::as_if_forced`]) — which is precisely the model this compiler
    /// ran before forcedness reached the geometry — and if *that* world routes, the
    /// unforced footing is what closed the leg and the boxes are named with the beats
    /// that lay them.
    ///
    /// A code of its own rather than a [`DW_CRITICAL_UNROUTABLE`] variant for the reason
    /// its three siblings are: the prefab is innocent and the geometry reads open. The
    /// author is looking at a `fill-region` they wrote on purpose and must be told that
    /// its *root* is the defect — the box is right, the block is right, the beat is
    /// skippable — because "no collision-free path" would send them to hunt a wedged
    /// doorway that is not there.
    ///
    /// Like [`DW_GATE_NEVER_OPENED`], the rule asks for no surface: it detects that
    /// what a campaign already says is unsound.
    pub const DW_UNFORCED_FOOTING: DwCode = DwCode::new("DW0546", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0315`: a `set-checkpoint` (spec-0012) that would strand the party — from the
    /// checkpoint cell, a remaining required critical-path anchor is no longer
    /// walkable (a checkpoint behind a one-way drop). Re-roots the DW0311 reachability
    /// at the checkpoint.
    pub const DW_CHECKPOINT_STRANDED: DwCode = DwCode::new("DW0315", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0316`: a `set-checkpoint` anchor with no standable footing on the final
    /// assembled model (a trap-trigger / hazard / mid-air cell), so the party would
    /// respawn into the void or a wall.
    pub const DW_CHECKPOINT_UNSTANDABLE: DwCode = DwCode::new("DW0316", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0378`: a `timed-gate` (spec-0016 §4) that is a coin flip rather than a
    /// timing read — the set of entry phases from which a walking player clears the
    /// span before the gate shuts covers **less than 20% of the cycle**.
    /// All-phase passability is explicitly NOT the requirement: a gate
    /// that punishes bad timing is the point. A gate that punishes *every* timing is
    /// not a skill check, it is a slot machine, and no amount of learning the level
    /// makes it fair.
    pub const DW_TIMED_GATE_COIN_FLIP: DwCode = DwCode::new("DW0378", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0918`: a `volley` (spec-0022) whose cadence is a coin flip rather than a
    /// timing read — a player standing anywhere in the kill zone when a salvo lands
    /// cannot leave the zone before the next salvo in at least **20% of the
    /// interval**. The volley's counterplay is LEAVING the zone (spec-0022: "a
    /// decision, not a lucky strafe"); an interval too short for the walk out makes
    /// that decision unavailable. The same body model and floor as
    /// [`DW_TIMED_GATE_COIN_FLIP`] (`DW0378`), through the one [`timing_read`].
    pub const DW_VOLLEY_COIN_FLIP: DwCode = DwCode::new("DW0918", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0388`: a **timed hazard** (spec-0016 §4 addendum) the player cannot
    /// observe before committing to it — no standable cell exists that is clear of
    /// the hazard's lethal span, reachable without entering it, and has line of
    /// sight to it.
    ///
    /// The souls dossier's strongest and most universal finding (§5.3, §2.2 axis 5):
    /// what the real games guarantee about a periodic hazard is not a duty-cycle
    /// ratio but that you can **stand somewhere safe and watch a full cycle before
    /// committing**. You can stand outside Sen's Fortress and watch a blade swing;
    /// you cannot see inside the Capra room. [`DW_TIMED_GATE_COIN_FLIP`] (`DW0378`)
    /// measures the ratio — the dossier's own verdict is that if only one of the two
    /// proofs can be afforded it should be this one, not the 20%.
    pub const DW_HAZARD_UNOBSERVABLE: DwCode = DwCode::new("DW0388", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0393`: a `timed-gate`'s `disarm` affordance is not usable
    /// **before** the gate is committed to — its cell has no standable footing, or is
    /// walkable from the campaign entry only through the gate span itself.
    ///
    /// The disarm is the third rung of the souls hazard ladder (dossier §5.2):
    /// readable, avoidable, and finally *disable-able*. A jam lever the party can
    /// only pull after surviving the crossing disables nothing — it is a reward for
    /// having already beaten the hazard, dressed as counterplay. This is the same
    /// clause `DW0373` puts on a shortcut's unlock and `DW0342` puts on a trap's
    /// disarm, stated once for the gate: the affordance must be reachable while the
    /// hazard is still ahead of you.
    pub const DW_TIMED_GATE_DISARM_UNREACHABLE: DwCode = DwCode::new("DW0393", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0376`: an `ambush` (spec-0016 §3) with no counterplay — with every
    /// ambusher standing where it will stand, no rest point (a checkpoint, a bonfire,
    /// or the campaign entry) is walkable from the trigger cell any more. The player
    /// is sealed in a pocket with the ambush and can only trade blows blind.
    ///
    /// This is deliberately NOT a telegraph requirement. The un-telegraphed ambush is
    /// core souls vocabulary: dying uninformed once is how
    /// the level teaches, and determinism guarantees the second attempt meets the same
    /// ambushers in the same cells. What the engine owes the informed player is a
    /// *play* — a retreat, luring ground, a positioning line — and that is what this
    /// proves exists.
    pub const DW_AMBUSH_NO_COUNTERPLAY: DwCode = DwCode::new("DW0376", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0373`: a `shortcut` (spec-0016 §2) whose far-side `unlock` affordance is
    /// not reachable while the gate is still sealed — the LONG route does not exist,
    /// so the mechanism that opens the shortcut can never be pulled and the gate is
    /// dead scenery. The whole pattern is "earn the far side the hard way, then open
    /// the door forever"; without a hard way there is nothing to earn.
    pub const DW_SHORTCUT_NO_LONG_ROUTE: DwCode = DwCode::new("DW0373", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0374`: a `shortcut` (spec-0016 §2) that **leaks** — opening its gate does not
    /// shorten the walk from the campaign entry to its own `unlock` affordance, so the
    /// unlock is not on the far side of anything. The pattern is "earn the far side
    /// the hard way, then open the door forever"; if the door is irrelevant to
    /// reaching the mechanism that opens it, the loop-back moment — which IS the
    /// design — never happens. The classic form is an `unlock` placed on the NEAR
    /// side of its own gate.
    pub const DW_SHORTCUT_NO_GAIN: DwCode = DwCode::new("DW0374", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0379`: **retry cost** (spec-0016 §7, warning tier) — the proven walk from a
    /// rest point to a beat it can respawn the party into is longer than
    /// [`RETRY_BUDGET_TICKS`]. Dying must be an investment, not a commute: past the
    /// budget the loop stops teaching and starts taxing. A **warning**, deliberately:
    /// a long walk can be the authored point (a pilgrimage, a set-piece approach),
    /// and the compiler will not overrule that — it names the distance and leaves the
    /// judgement to the owner's QA hour.
    pub const DW_RETRY_COST: DwCode = DwCode::new("DW0379", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0380`: **optional-elite bypass** (spec-0016 §7, warning tier) — an enemy the
    /// critical path never requires the party to kill has no route around it: every
    /// proven forward leg passes inside its aggro radius, so "optional" is a lie and
    /// the fight is mandatory in everything but the objective list.
    ///
    /// The Tree Sentinel pattern — a powerful optional enemy near the start, fight it
    /// or walk around it — is explicitly legitimate, and
    /// this is the one obligation it carries: the walk-around has to exist.
    pub const DW_OPTIONAL_ELITE_UNAVOIDABLE: DwCode = DwCode::new("DW0380", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0386`: a TD `lane` (spec-0016 §6) whose polyline does not survive contact
    /// with the assembled world — a waypoint anchor that resolves nowhere, a
    /// waypoint with no standable footing, a leg the squad cannot walk, or a leg
    /// **10 blocks or shorter**. The spacing rule is not taste: vanilla re-rolls a
    /// patrol target to a random point once the patroller is within 10 blocks of it,
    /// so a tighter lane is a lane the engine quietly stops following — the squad
    /// wanders, and it reads as working-but-drunk rather than as a bug.
    pub const DW_LANE_GEOMETRY: DwCode = DwCode::new("DW0386", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0478`: **the respawn-point safe zone** (spec-0016 §1) — a cell the party
    /// comes back to life on sits inside some hostile force's aggro range.
    ///
    /// A respawn point is where the party returns after a death and where a
    /// `respawns_on_rest` wave is put back on its feet. If it stands inside a
    /// hostile's perception radius, dying drops the party into contact on the tick
    /// they arrive: the retry loop stops teaching and becomes a soft-lock — a despair
    /// machine you cannot rest your way out of. Error tier, not advisory: unlike the
    /// §7 pacing lints there is no reading of this geometry that is the authored
    /// point.
    ///
    /// **The object class is the respawn point, not the verb that places it.** A
    /// `bonfire` and a `set-checkpoint` are siblings of one sum type — the DSL says
    /// so in as many words ("the sibling of [`Verb::SetCheckpoint`]"), they
    /// resolve to one [`crate::compiler::plan::CheckpointPlan`] distinguished only by `rest`,
    /// and vanilla returns a dead player to either by the identical `spawnpoint`
    /// mechanism. Binding this proof to `rest == true` therefore made it a hook on
    /// one variant and not its sibling: `nobodys-cave-island` shipped three
    /// `set-checkpoint`s and five unleashed hostiles for twenty-two owner rounds
    /// while this check examined ZERO objects and reported green (CLAUDE.md, *a
    /// capability belongs to the object class it acts on*; the staging gate's
    /// `UNBOUND` verdict, row `bell-08`).
    ///
    /// The widening onto `set-checkpoint` asks for nothing to be written: the
    /// verdict is a function of geometry the campaign already declares, and a
    /// campaign that trips it was always soft-locked. The widening is a defect fixed
    /// in the proof, not a requirement added to the document — the six live
    /// violations it found on the shipped island are what it exists for.
    pub const DW_RESPAWN_IN_AGGRO: DwCode = DwCode::new("DW0478", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0327`: a `begin-stealth` (spec-0014) zone that is unstandable, or unreachable
    /// from the player's position at the beat that activates the stealth check.
    pub const DW_STEALTH_ZONE: DwCode = DwCode::new("DW0327", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0355`: a **punishing** `begin-stealth` whose grace window cannot be beaten —
    /// from a position a player legally occupies the instant the beat arms (the
    /// activating objective's anchor, or any checkpoint that can respawn them into the
    /// running session), no zone is reachable within `grace_ticks` at sprint speed over
    /// the assembled geometry. DW0327 proves cover *exists and is reachable*; this
    /// proves it is reachable **in time**. Without it a beat that arms under the
    /// player's feet at the most exposed cell in the room kills every player — machine
    /// or human — a fixed couple of seconds later, and if the checkpoint it respawns
    /// them at is also outside cover, the retry loop never terminates. A structurally
    /// unavoidable death is not 初见杀 (spec-0016), it is a broken beat.
    pub const DW_STEALTH_ONSET: DwCode = DwCode::new("DW0355", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0342`: a **lethal** trap (spec-0011) whose trigger cell lies on the forced
    /// critical path with no discharge — not avoidable (the trigger cell is a required
    /// path cell), not survivable (`rearm`, so a respawn walk-back re-triggers it →
    /// soft-loop), and not disarmable (no disarm affordance reachable before it). The
    /// player is provably killed or soft-looped. Analysis-tier (exit 2) like `DW0312`:
    /// a content-design mistake, not a geometry defect. (Renumbered from the spec's
    /// stale `DW0314`.)
    pub const DW_TRAP_LETHAL_UNAVOIDABLE: DwCode = DwCode::new("DW0342", ExitTier::Analysis);
}

delvewright_dsl::dw_code! {
    /// `DW0325`: a `move-actor` destination unreachable by the actor's footprint over
    /// the assembled geometry, or an actor spawn/destination anchor that does not
    /// resolve to a placeable cell (spec-0014). Names the actor, the leg, and the
    /// first blocked cell.
    pub const DW_ACTOR_UNROUTABLE: DwCode = DwCode::new("DW0325", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0410`: a staged walk (`move-actor` / `move-npc`) whose path is blocked by a
    /// gate that an **earlier effect in its own timeline** sealed with `close-gate`
    /// (round-8 island playtest; see [`crate::compiler::timeline`]).
    ///
    /// Distinct from `DW0325`/`DW0307` by construction: those fire when the leg is
    /// unwalkable on the open world at all, this one when the leg *is* walkable on
    /// the open world and only the timeline's own `close-gate` makes it impossible.
    /// The planner routes over the timeline-adjusted world first, so a legal
    /// alternative route around the seal is simply taken and no diagnostic is raised
    /// — this fires only when the sealed world admits no route.
    pub const DW_GATE_TIMELINE: DwCode = DwCode::new("DW0410", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0488`: one content-keyed walk driver is shared by occurrences that do not
    /// stand in the same place when they fire, so the shared driver's first waypoint
    /// is the wrong cell for at least one of them and that occurrence opens with a
    /// teleport.
    ///
    /// `move-npc` / `move-actor` drivers are deduped by `(body, to)` — two
    /// beats that walk the same character to the same mark share one emitted
    /// function, and that function's waypoint polyline starts where the FIRST
    /// occurrence's branch leaves the body. That was a documented limitation for as
    /// long as the dedup existed; it is a diagnostic now because the failure it
    /// produces is invisible in the DSL and unmistakable on a server (the body
    /// vanishes from where it stood and re-appears at the other occurrence's
    /// origin).
    ///
    /// Distinct from [`DW_MOVE_UNROUTABLE`]/[`DW_ACTOR_UNROUTABLE`], which fire when
    /// a leg has no route at all: here every leg is perfectly routable and the defect
    /// is that they cannot share one route.
    pub const DW_MOVE_ORIGIN_SHARED: DwCode = DwCode::new("DW0488", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0921`: **a place a body can get into and not out of.** From a cell of the
    /// proven route, a body walking, falling and jumping ([`World::body_moves`]) can
    /// reach a cell from which no walk, fall or jump leads back to the route — a
    /// garden bed ringed by a hedge it jumped onto and dropped off, a pit it fell
    /// into. The player is soft-locked there: nothing but leaving the game gets
    /// them out.
    ///
    /// It is judged once per quest configuration the critical path passes through
    /// ([`World::region_state_at`] over each leg's arrival), with that
    /// configuration's gates as they stand and that configuration's own route cells
    /// as the place a body must get back to. That is the whole of the author's
    /// control over it, and it needs no declaration of its own: a room the story
    /// shuts the party into holds the objective the story is waiting on, so the
    /// party standing in it stands among that configuration's route cells, and a
    /// room shut until a later beat opens it is only a trap when the beat is out of
    /// reach from inside — which is what this refuses.
    pub const DW_BODY_CANNOT_LEAVE: DwCode = DwCode::new("DW0921", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0314`: an exported critical-path waypoint is not standable in the FINAL
    /// assembled world (settled + water-flooded + relight fixtures) **as that leg's own
    /// runtime region writes leave it**. A build-time self-check over the very cells the
    /// harness will replay: it makes it structurally impossible to ship a waypoint the
    /// game floods or walls (the water-flow / post-nav-mutation divergence class).
    ///
    /// The qualifier is load-bearing, because a leg is not walked over the bare
    /// assembled world. A campaign may lay floor at runtime — a repaired stair, a
    /// lowered bridge, a placed plank — and the leg that crosses it is routed over the
    /// world those writes produce ([`LegRoute::proven_world`]). Judging the bare world
    /// here instead refused every such route: the plank is not in the assembled model,
    /// so its cells read "no floor" and a correct campaign could not ship.
    ///
    /// Every cell a leg exports comes from `find_path` over the world this check now
    /// rebuilds, so it can only fire if a later pass mutates a cell nav relied on or an
    /// endpoint resolves off the walkable set — in which case it is a compiler/assembly
    /// defect to escalate, never a cell to nudge. That is the case it is kept for: an
    /// edit batch that buries a room the content needs walkable is still caught,
    /// because a terrain edit is not a runtime region write and no leg state restores
    /// it.
    pub const DW_WAYPOINT_NOT_STANDABLE: DwCode = DwCode::new("DW0314", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0724`: a visual-tier camera's eye cell is occupied (a solid block or water)
    /// in the FINAL assembled world — the frame would render the inside of a block
    /// instead of the scene, and a picture of the inside of a block is
    /// indistinguishable from a picture of a featureless room.
    ///
    /// The property belongs to **a camera**, not to one kind of camera. Every shot
    /// `crate::compiler::render_plan` derives — spawn, per-piece interior, seam, NPC, interact
    /// anchor, gate, and the first-person `pov` shots — puts an eye at a point in the
    /// assembled world, and every one of them can land inside geometry. Binding this
    /// to the `pov` kind alone was an accident of which kind needed it first: a seam
    /// camera stands four blocks along the seal's axis one cell under the ceiling, on
    /// the tile's centre column, which is exactly where a hanging lantern is, and the
    /// resulting flat frame was invisible to every build.
    ///
    /// Whether a violation is a defect of the *derivation* or of the *geometry*
    /// depends on the kind, and the message says which. A `pov` eye sits at 1.62
    /// above a DW0314-proven-standable waypoint, so it is clear by construction and a
    /// violation means the derivation changed (or a later pass mutated the cell) —
    /// fix the derivation, never the waypoint. Every other kind takes a fixed offset
    /// from authored geometry, so a violation is that geometry standing where the
    /// review camera has to be, and the repair is the piece.
    pub const DW_CAMERA_EYE_OCCLUDED: DwCode = DwCode::new("DW0724", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0322`: **boundary safety** (spec-0017 invariant 4) — after a world edit,
    /// the reachable walk region fails the "one step off the proven ground is
    /// survivable and recoverable" guarantee the greenfield generator's bounding
    /// berm used to provide *physically*. What that means is a property of the
    /// world-generator [`Ambient`], so the code names one rule stated per horizon:
    ///
    /// * `horizon: void` — a reachable walkable cell borders a **void drop**: a
    ///   horizontally adjacent column the player can step (or open a gate) into
    ///   with no support of any kind below, so the step leaves the world.
    /// * `horizon: ocean` — a reachable walkable cell borders **water the player
    ///   cannot get out of**: the pinned superflat puts bedrock under every column,
    ///   so nothing can fall out of an ocean world and the void premise is vacuous;
    ///   the real hazard the ocean horizon introduced (`plan::OCEAN_BASE_Y`) is
    ///   *stranding* — a player who ends up in the sea with no shoreline to climb
    ///   back onto is out of the delve just as permanently as one who fell out of a
    ///   void world. See [`verify_boundary_safety`] for the exact model.
    pub const DW_EDIT_BORDERS_VOID: DwCode = DwCode::new("DW0322", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0318`: **a body of fluid runs out of the built world**, stated against the
    /// world-generator [`Ambient`] the way [`DW_EDIT_BORDERS_VOID`] already is.
    ///
    /// The piece-level containment rule (`DW0800`, `delvec grammar` /
    /// `delvec prefab`) proves that every fluid source in a piece has something in
    /// each of the five cells it would run into — *within that piece's own bytes*.
    /// A run direction that leaves the piece's outer face it counts and explicitly
    /// does not judge, because what is beyond a face is not in those bytes:
    /// **whatever the piece is placed against decides where that water goes.** This
    /// is the check that decides it, and it is the reason that sentence is now true.
    ///
    /// At placement the neighbour is known, and it is one of exactly three things:
    ///
    /// * **another placed piece** — the water runs into cells that piece authored,
    ///   and that piece's own `DW0800` governs them. Not a finding here.
    /// * **the ocean horizon's ambient** — the pinned superflat puts water from
    ///   `floor_top+1` to sea level and stone below it in every column the content
    ///   did not build, so a shore's water meets the sea it depicts. Not a finding:
    ///   the same premise that makes the void branch of `DW0322` vacuous under
    ///   `horizon: ocean` makes this one vacuous too.
    /// * **the void horizon's nothing** — and then the water falls out of the
    ///   world. Vanilla runs it down, forever, on the server's own clock before any
    ///   player arrives: an infinite waterfall off the edge of the map, in a delve
    ///   nobody rendered it into. That is the finding.
    ///
    /// It is the exact fluid analogue of [`crate::compiler::assembled::DW_GRAVITY_DESPAWN`]
    /// (`DW0313`), which fails the build when a placed *gravity* block falls out of
    /// a void world. The solid case was covered from the beginning; this is the
    /// fluid case, and the asymmetry is all that made it a hole rather than a
    /// policy.
    ///
    /// Both branches **aggregate**, like `DW0322`: one report per run naming up to
    /// [`BOUNDARY_LIST_LIMIT`] cells plus the totals, so a one-cell dribble and a
    /// whole coastline pouring into nothing are distinguishable without re-probing.
    pub const DW_FLUID_LEAVES_WORLD: DwCode = DwCode::new("DW0318", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0851`: **the sea is in the walk region** — a cell a body was proved to
    /// stand on holds water once the world loads.
    ///
    /// The world model holds water in two disjoint places and only one of them
    /// reaches walkability. [`crate::compiler::assembled::Occupancy::flooded`] is seeded from
    /// the *assembled block map* — prefab-authored sources and waterlogged blocks —
    /// and every downstream proof reads it. The **ambient sea** is not in that block
    /// map at all: under [`Ambient::Ocean`] the world generator puts water in every
    /// column the content did not build, and [`World::ambient_water`] is the only
    /// thing that knows it. So the sea never reached `flooded`, never reached
    /// `is_occupied`, and never reached [`World::is_standable`]: **a cell inside a
    /// placed piece that the sea will fill was proved standable, and nothing could
    /// see it.**
    ///
    /// ## The model
    ///
    /// The question is asked of **the walk**, and of nothing else: the denominator is
    /// the reachable standable set the build already computed, which is where the
    /// party goes. For each of those cells the proof reads what the delivered world
    /// puts there. The sea gets into a placed piece two ways, and both are seeds of
    /// one flow:
    ///
    /// 1. **An open face** — a non-blocking cell *inside* the built volume, in the
    ///    sea's own band (`floor_top < y ≤ level`), 6-adjacent to an ambient sea
    ///    cell. That is where the sea is already touching the content.
    /// 2. **A waterloggable block the placement hands to the sea.** `/place template`
    ///    carries the fluid already in a cell onto the block it writes there, so a
    ///    stair, a fence, a pane, a chest or a set of iron bars placed below the sea
    ///    plane comes out `waterlogged=true` whatever the prefab said — and a
    ///    waterlogged cell is a genuine water source that spreads into its
    ///    neighbours. This is not a corner: it is what actually happened. The
    ///    tidewatch field case's staircase came out waterlogged four cells below the
    ///    surface, ran down its own treads, and put both of the delve's objectives
    ///    under water at `[260,61,4]` and `[260,61,8]` — through a hull whose open
    ///    contact face was, correctly, zero cells wide. The gallery's own
    ///    `ocean-horizon` point did the same thing with 10 blocks and 367 cells.
    /// 3. **Flow** — [`crate::compiler::assembled::flood`], the same function the block map's
    ///    water runs through: infinite-water source formation, then 7-level decay
    ///    with infinite downward fall. Deliberately **not** a second physics, so a
    ///    room cannot be judged wet by one model and dry by the other.
    /// 4. **Confinement** — every non-built cell 6-adjacent to the built volume is
    ///    added to the barrier set, so the flow stays inside the content instead of
    ///    wandering across an ocean that is already water. What leaves the built
    ///    volume is `DW0318`'s question, not this one.
    /// 5. **Verdict** — a walk cell whose **own** cell the flow reaches. That is
    ///    where the body's feet go, and the delve says a body stands there.
    ///
    /// ## Why the foot cell, and what wading is
    ///
    /// The line was drawn at the head cell once, on the argument that a body whose
    /// feet are wet and whose head is dry is wading and vanilla lets it walk. That is
    /// true about vanilla and wrong about a delve: the field case is a two-scene
    /// campaign whose every objective stands in shin-deep sea, and a head-cell
    /// verdict passed it. So the foot cell decides. A walk cell that is dry underfoot
    /// and merely *touches* water — at head height or beside it — is the shoreline,
    /// and it is **counted and named** rather than judged, because a shoreline 26
    /// cells wide and one 2000 cells wide are different maps.
    ///
    /// ## Direction of error
    ///
    /// Same contract as the block map's flood, for the same reason: the model may
    /// call a cell wet that vanilla leaves dry, never the reverse. The seeds are
    /// entered as *sources* where vanilla would start them one level down, so a wide
    /// contact face fills further than the game would. Over-marking turns a proof red
    /// — caught, escalated, and answerable by walling the face or lifting the floor;
    /// under-marking is a wet cell shipping as proven dry.
    ///
    /// ## What this is not
    ///
    /// Not the shoreline outside the content. A shore piece that authors its own
    /// water up to the waterline (`DW0344`, spec-0048) has that water in the block
    /// map already: those cells are `flooded`, therefore not standable, therefore
    /// never in the walk region, and this proof has nothing to say about them. Wading
    /// into the sea off a beach is a body leaving the walk region, which is
    /// `DW0322`'s question.
    pub const DW_SEA_ENTERS_WALK: DwCode = DwCode::new("DW0851", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0442`: a `volley`'s gallery slot has no clear line of fire to a standable
    /// cell of its declared kill zone. The compile-time form of the owner's
    /// saturation ruling — a volley must BLANKET its zone, so a cell
    /// the slot cannot reach is a hole a player could stand in and be safe by
    /// accident. Escaping a volley must be a decision (leave the zone), never a
    /// lucky step.
    pub const DW_VOLLEY_ZONE_UNCOVERED: DwCode = DwCode::new("DW0442", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0444`: a trap-payload region is unusable — a `volley` kill zone with no
    /// standable cell, or a `collapse` region with nothing to drop / nothing to
    /// land on.
    pub const DW_TRAP_REGION_EMPTY: DwCode = DwCode::new("DW0444", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0445`: the critical path is not completable once a `collapse` has fired.
    pub const DW_COLLAPSE_BURIES_PATH: DwCode = DwCode::new("DW0445", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW0446`: a `volley`'s `from_anchor` cell is not clear, so the projectile
    /// would be summoned inside solid geometry and never leave it.
    pub const DW_VOLLEY_SLOT_OCCLUDED: DwCode = DwCode::new("DW0446", ExitTier::Build);
}

/// Every quest effect in the campaign (objective-complete, quest-complete, and
/// trigger effects), matching the emitter's `all_campaign_effects` traversal —
/// each effect ahead of the ones nested in its `sequence` steps / `on_arrive`
/// bundle (spec-0014), so nav planning sees moves and cutscenes wherever they
/// appear. Pre-0.6 campaigns have no nesting, so the flattened list equals the
/// shallow one and output stays byte-identical.
///
/// Defined as [`crate::compiler::timeline::walk`] with the per-effect gate states dropped:
/// the two share **one** traversal, so the effect a planner is looking at and the
/// timeline state attributed to it can never drift out of alignment.
fn all_effects<'a>(plan: &'a Plan) -> Vec<&'a QuestEffect> {
    crate::compiler::timeline::walk(plan)
        .into_iter()
        .map(|(e, _)| e)
        .collect()
}

/// Whether the campaign uses any verb that needs the voxel `World` (`move-npc` or
/// `cutscene`). When false, the emitter skips building the occupancy model, so
/// v0.2/v0.3 output is untouched.
pub fn needs_world(plan: &Plan) -> bool {
    all_effects(plan).iter().any(|e| {
        matches!(
            &e.verb,
            Verb::MoveNpc { .. } | Verb::Cutscene { .. } | Verb::MoveActor { .. }
        )
    })
    // The critical-path walkability check (DW0311) also needs the occupancy model.
        || has_walkable_critical_leg(plan)
    // The checkpoint (DW0315/DW0316) and stealth-zone (DW0327) proofs, v0.6, need
    // the assembled occupancy model too, as does the trap proof (DW0342, spec-0011).
        || !plan.checkpoints.is_empty()
        || !plan.stealth_beats.is_empty()
    // A loop's slab, span and tiling are judged over the assembled world
    // (spec-0086 §4).
        || !plan.loops.is_empty()
        || !plan.traps.is_empty()
}

/// The spec-0016 §7 pacing lints. **Warning tier** — every finding here is a
/// design judgement the compiler can measure but must not overrule, so these
/// return diagnostics rather than failing the build.
///
/// 1. [`DW_RETRY_COST`] (`DW0379`) — bonfire/checkpoint → the DEEPEST beat it
///    respawns into, over the proven path, must be under
///    [`RETRY_BUDGET_TICKS`]. Dying should be an investment, not a commute.
/// 2. [`DW_OPTIONAL_ELITE_UNAVOIDABLE`] (`DW0380`) — an enemy no critical-path
///    `kill` objective requires must have a route around it. The Tree Sentinel
///    pattern is legitimate; a "walk around it" you cannot walk around is not.
pub fn pacing_lints(plan: &Plan, world: &World) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    out.extend(retry_cost_lint(plan, world));
    out.extend(optional_elite_lint(plan, world));
    out
}
