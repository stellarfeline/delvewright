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
//!   keyframe chords ([`crate::camera`]). Cameras fly (exempt from walkability)
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

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

use delvewright_dsl::{CameraWaypoint, Lethality, QuestEffect, TrapReset};

use crate::plan::{Plan, RegionEvent, RegionWrite, ResolvedAnchor, Step, TrapPlan};
use delvewright_dsl::Diagnostic;
use delvewright_dsl::DwCode;

/// `DW0307`: a `move-npc` destination unreachable by any walkable path from the
/// NPC's position over the assembled geometry.
pub const DW_MOVE_UNROUTABLE: DwCode = DwCode::every_version("DW0307");
/// `DW0308`: a `cutscene` camera dolly path that passes through a solid block.
pub const DW_CUTSCENE_CLIP: DwCode = DwCode::every_version("DW0308");
/// `DW0347`: a `cutscene` shot whose aim sweeps faster than the angular budget
/// ([`crate::camera::MAX_AIM_DEG_PER_TICK`], 6 °/tick = 120 °/s) — a pan that
/// fast at 20 Hz is nausea-tier and provably bad *before* it ships. Typical
/// cause: a `look_at` subject too close to a fast dolly. See the camera dossier
/// (`docs/notes/camera-dossier.md` §1) for the budget's derivation.
pub const DW_CAMERA_SPIN: DwCode = DwCode::every_version("DW0347");
/// `DW0311`: a consecutive pair of player-visited critical-path anchors that no
/// walkable path connects over the assembled geometry (with no inter-area
/// transport between them) — the player would be stranded. Turns the whole
/// "assembled seams aren't walkable" bug class — a prefab regen that wedges a
/// doorway shut or opens a void gap, which otherwise only a runtime bot catches
/// — into a compile error.
pub const DW_CRITICAL_UNROUTABLE: DwCode = DwCode::every_version("DW0311");
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
pub const DW_LETHAL_ON_CRITICAL_PATH: DwCode = DwCode::every_version("DW0510");
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
/// `every_version`, and the choice is load-bearing. The fence's question is
/// whether the rule requires a campaign to HAVE something it may not have had
/// (`since`) or detects that what it already says is wrong (`every_version`): this
/// is the second. `open-gate` has existed since v0.4, no surface is being asked
/// for, and "the delve must be completable" is ADR-0005, day one. Fencing it at
/// the current version would also make it **vacuous on every live campaign** —
/// `hollow-vigil` declares 0.3.0 and `nobodys-cave-island` 0.8.0 — which is the
/// unfenced-vacuity failure in the opposite direction.
pub const DW_GATE_NEVER_OPENED: DwCode = DwCode::every_version("DW0317");
/// `DW0544`: a forced critical-path leg depends on standing where a runtime region
/// write leaves **fluid** — water or lava.
///
/// A `fill-region` / `close-gate` / shortcut seal whose block is a fluid does not
/// build floor: it replaces whatever was in the box with something a body sinks
/// through ([`crate::plan::RegionWrite::Flood`]). Sibling of
/// [`DW_LETHAL_ON_CRITICAL_PATH`] and derived the same way — the leg is re-routed
/// over the identical world with every runtime fluid fill treated as solid, and if
/// *that* world routes, the fluid is what closed the leg and the boxes are named.
///
/// A code of its own rather than a [`DW_CRITICAL_UNROUTABLE`] variant for the
/// reason `DW0510` is: the prefab is innocent. The author is looking at a box they
/// filled on purpose and needs to be told that filling it with water is what took
/// the footing away — not sent hunting for a wedged doorway that is not there.
pub const DW_FLUID_FILL_ON_CRITICAL_PATH: DwCode = DwCode::every_version("DW0544");
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
/// `every_version`, on the same test [`DW_GATE_NEVER_OPENED`] states: the rule asks
/// for no surface a campaign may not have (every effect root it names has existed
/// since the root was added) and detects that what a campaign already says is
/// unsound. Fencing it at the current version would leave it vacuous on every live
/// campaign, all of which declare below it.
pub const DW_UNFORCED_FOOTING: DwCode = DwCode::every_version("DW0546");
/// `DW0315`: a `set-checkpoint` (spec-0012) that would strand the party — from the
/// checkpoint cell, a remaining required critical-path anchor is no longer
/// walkable (a checkpoint behind a one-way drop). Re-roots the DW0311 reachability
/// at the checkpoint.
pub const DW_CHECKPOINT_STRANDED: DwCode = DwCode::every_version("DW0315");
/// `DW0316`: a `set-checkpoint` anchor with no standable footing on the final
/// assembled model (a trap-trigger / hazard / mid-air cell), so the party would
/// respawn into the void or a wall.
pub const DW_CHECKPOINT_UNSTANDABLE: DwCode = DwCode::every_version("DW0316");
/// `DW0378`: a `timed-gate` (spec-0016 §4) that is a coin flip rather than a
/// timing read — the set of entry phases from which a walking player clears the
/// span before the gate shuts covers **less than 20% of the cycle**.
/// All-phase passability is explicitly NOT the requirement: a gate
/// that punishes bad timing is the point. A gate that punishes *every* timing is
/// not a skill check, it is a slot machine, and no amount of learning the level
/// makes it fair.
pub const DW_TIMED_GATE_COIN_FLIP: DwCode = DwCode::every_version("DW0378");
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
pub const DW_HAZARD_UNOBSERVABLE: DwCode = DwCode::every_version("DW0388");
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
pub const DW_TIMED_GATE_DISARM_UNREACHABLE: DwCode = DwCode::every_version("DW0393");
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
pub const DW_AMBUSH_NO_COUNTERPLAY: DwCode = DwCode::every_version("DW0376");
/// `DW0373`: a `shortcut` (spec-0016 §2) whose far-side `unlock` affordance is
/// not reachable while the gate is still sealed — the LONG route does not exist,
/// so the mechanism that opens the shortcut can never be pulled and the gate is
/// dead scenery. The whole pattern is "earn the far side the hard way, then open
/// the door forever"; without a hard way there is nothing to earn.
pub const DW_SHORTCUT_NO_LONG_ROUTE: DwCode = DwCode::every_version("DW0373");
/// `DW0374`: a `shortcut` (spec-0016 §2) that **leaks** — opening its gate does not
/// shorten the walk from the campaign entry to its own `unlock` affordance, so the
/// unlock is not on the far side of anything. The pattern is "earn the far side
/// the hard way, then open the door forever"; if the door is irrelevant to
/// reaching the mechanism that opens it, the loop-back moment — which IS the
/// design — never happens. The classic form is an `unlock` placed on the NEAR
/// side of its own gate.
pub const DW_SHORTCUT_NO_GAIN: DwCode = DwCode::every_version("DW0374");
/// `DW0379`: **retry cost** (spec-0016 §7, warning tier) — the proven walk from a
/// rest point to a beat it can respawn the party into is longer than
/// [`RETRY_BUDGET_TICKS`]. Dying must be an investment, not a commute: past the
/// budget the loop stops teaching and starts taxing. A **warning**, deliberately:
/// a long walk can be the authored point (a pilgrimage, a set-piece approach),
/// and the compiler will not overrule that — it names the distance and leaves the
/// judgement to the owner's QA hour.
pub const DW_RETRY_COST: DwCode = DwCode::every_version("DW0379");
/// `DW0380`: **optional-elite bypass** (spec-0016 §7, warning tier) — an enemy the
/// critical path never requires the party to kill has no route around it: every
/// proven forward leg passes inside its aggro radius, so "optional" is a lie and
/// the fight is mandatory in everything but the objective list.
///
/// The Tree Sentinel pattern — a powerful optional enemy near the start, fight it
/// or walk around it — is explicitly legitimate, and
/// this is the one obligation it carries: the walk-around has to exist.
pub const DW_OPTIONAL_ELITE_UNAVOIDABLE: DwCode = DwCode::every_version("DW0380");
/// `DW0386`: a TD `lane` (spec-0016 §6) whose polyline does not survive contact
/// with the assembled world — a waypoint anchor that resolves nowhere, a
/// waypoint with no standable footing, a leg the squad cannot walk, or a leg
/// **10 blocks or shorter**. The spacing rule is not taste: vanilla re-rolls a
/// patrol target to a random point once the patroller is within 10 blocks of it,
/// so a tighter lane is a lane the engine quietly stops following — the squad
/// wanders, and it reads as working-but-drunk rather than as a bug.
pub const DW_LANE_GEOMETRY: DwCode = DwCode::every_version("DW0386");
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
/// so in as many words ("the sibling of [`QuestEffect::SetCheckpoint`]"), they
/// resolve to one [`crate::plan::CheckpointPlan`] distinguished only by `rest`,
/// and vanilla returns a dead player to either by the identical `spawnpoint`
/// mechanism. Binding this proof to `rest == true` therefore made it a hook on
/// one variant and not its sibling: `nobodys-cave-island` shipped three
/// `set-checkpoint`s and five unleashed hostiles for twenty-two owner rounds
/// while this check examined ZERO objects and reported green (CLAUDE.md, *a
/// capability belongs to the object class it acts on*; the staging gate's
/// `UNBOUND` verdict, row `bell-08`).
///
/// [`Binds::EveryVersion`], and the widening onto `set-checkpoint` does not
/// change that. A [`Binds::Since`] fence grandfathers campaigns against a new
/// **authoring obligation** — a field they must now write. This rule asks for
/// nothing to be written: its verdict is a function of geometry the campaign
/// already declares, and a campaign that trips it was always soft-locked. The
/// widening is a defect fixed in the proof, not a requirement added to the
/// document, so fencing it would grandfather the soft-lock rather than the
/// paperwork — and the six live violations it found on the shipped island are
/// what that would have preserved.
pub const DW_RESPAWN_IN_AGGRO: DwCode = DwCode::every_version("DW0478");
/// `DW0327`: a `begin-stealth` (spec-0014) zone that is unstandable, or unreachable
/// from the player's position at the beat that activates the stealth check.
pub const DW_STEALTH_ZONE: DwCode = DwCode::every_version("DW0327");
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
pub const DW_STEALTH_ONSET: DwCode = DwCode::every_version("DW0355");
/// `DW0342`: a **lethal** trap (spec-0011) whose trigger cell lies on the forced
/// critical path with no discharge — not avoidable (the trigger cell is a required
/// path cell), not survivable (`rearm`, so a respawn walk-back re-triggers it →
/// soft-loop), and not disarmable (no disarm affordance reachable before it). The
/// player is provably killed or soft-looped. Analysis-tier (exit 2) like `DW0312`:
/// a content-design mistake, not a geometry defect. (Renumbered from the spec's
/// stale `DW0314`.)
pub const DW_TRAP_LETHAL_UNAVOIDABLE: DwCode = DwCode::every_version("DW0342");

/// A resolved stealth zone `(anchor name, centre cell, half-extents)`.
type ZoneCell = (String, [i32; 3], [u32; 3]);
/// A stealth beat probe for [`verify_stealth`]: `(zones, firing step)`.
type StealthProbe = (Vec<ZoneCell>, usize);

/// `DW0325`: a `move-actor` destination unreachable by the actor's footprint over
/// the assembled geometry, or an actor spawn/destination anchor that does not
/// resolve to a placeable cell (spec-0014). Names the actor, the leg, and the
/// first blocked cell.
pub const DW_ACTOR_UNROUTABLE: DwCode = DwCode::every_version("DW0325");

/// `DW0410`: a staged walk (`move-actor` / `move-npc`) whose path is blocked by a
/// gate that an **earlier effect in its own timeline** sealed with `close-gate`
/// (round-8 island playtest; see [`crate::timeline`]).
///
/// Distinct from `DW0325`/`DW0307` by construction: those fire when the leg is
/// unwalkable on the open world at all, this one when the leg *is* walkable on
/// the open world and only the timeline's own `close-gate` makes it impossible.
/// The planner routes over the timeline-adjusted world first, so a legal
/// alternative route around the seal is simply taken and no diagnostic is raised
/// — this fires only when the sealed world admits no route.
pub const DW_GATE_TIMELINE: DwCode = DwCode::every_version("DW0410");

/// `DW0488`: one content-keyed walk driver is shared by occurrences that do not
/// stand in the same place when they fire, so the shared driver's first waypoint
/// is the wrong cell for at least one of them and that occurrence opens with a
/// teleport.
///
/// `move-npc` / `move-actor` drivers are deduped by `(body, to_anchor)` — two
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
pub const DW_MOVE_ORIGIN_SHARED: DwCode = DwCode::every_version("DW0488");

/// The branch condition a staging effect fires under: the per-effect
/// `requires_flags` / `forbids_flags` gate (DSL v0.6).
///
/// This exists because walk origins **chain** — each leg starts where the body's
/// previous leg left it — and that chain used to be a single flat sequence per
/// body, walked in campaign effect order with no regard for which branch each leg
/// belonged to. Owner playtest, island round 15: choosing to *wait* teleported
/// Eurylochus out of the cave down to the beach and walked him 35 seconds back
/// up, because the `flag/flee`-gated leg to the gangplank — a leg that cannot
/// fire on the branch the player took — had overwritten the origin the
/// `flag/wait`-gated leg to the alcove inherited. `npc/perimedes` had the same
/// defect on the same branch, unreported.
///
/// The rule this type enforces is the compiler's usual one (see
/// [`crate::continuity`]): chain only from what is **provably** already true.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BranchGate {
    /// Flags that must be set for the effect to fire.
    requires: BTreeSet<String>,
    /// Flags that must be unset for the effect to fire.
    forbids: BTreeSet<String>,
}

impl BranchGate {
    /// The gate an effect carries.
    fn of(eff: &QuestEffect) -> Self {
        Self {
            requires: eff
                .requires_flags()
                .iter()
                .map(|f| f.as_str().to_string())
                .collect(),
            forbids: eff
                .forbids_flags()
                .iter()
                .map(|f| f.as_str().to_string())
                .collect(),
        }
    }

    /// Whether the effect is unconditional (always fires).
    fn is_unconditional(&self) -> bool {
        self.requires.is_empty() && self.forbids.is_empty()
    }

    /// Does this gate provably hold on **every** timeline where `other` fires?
    ///
    /// True when `other`'s conditions are a superset of this one's: a leg gated on
    /// nothing always fired; a leg gated on `flag/flee` has provably fired by the
    /// time another `flag/flee` leg runs. It is deliberately *not* true for two
    /// legs gated on different flags — `flag/wait` does not prove `flag/flee`, so
    /// the flee leg is skipped when chaining into the wait leg, which is exactly
    /// the fix.
    ///
    /// The direction is conservative on purpose. The compiler cannot prove two
    /// flags are mutually exclusive (nothing in the DSL says `flag/wait` and
    /// `flag/flee` cannot both be set), so this never *asserts* that a skipped
    /// leg did not fire — it only declines to assume that it did, and falls back
    /// to the most recent staging the branch does prove. That can only ever move
    /// an origin from "certainly wrong on this branch" to "correct on the branch
    /// the DSL describes"; it cannot invent a route.
    fn implied_by(&self, other: &BranchGate) -> bool {
        self.requires.is_subset(&other.requires) && self.forbids.is_subset(&other.forbids)
    }
}

/// The **driver-name suffix** a branch gate contributes.
///
/// A walk driver is content-keyed by the body and its destination, and that key
/// used to be the whole story. It is not: two beats can legitimately walk the
/// same character to the same mark **from different places**, one per branch —
/// the island's Eurylochus reaches `anchor/gangplank` from the cave if the party
/// flees at the cheese, and from the upper sheep pen if they stay and escape
/// under the rams. Both are correct content; one emitted driver cannot carry
/// both origins, and before this the second beat silently ran the first beat's
/// polyline, teleporting the body across the map to start.
///
/// Including the gate in the key gives each branch its own driver. Unconditional
/// walks contribute the **empty** suffix, so every campaign that never gated a
/// walk keeps byte-identical function names.
pub fn gate_key(eff: &QuestEffect) -> String {
    BranchGate::of(eff).key()
}

impl BranchGate {
    /// A short, deterministic, filename-safe key for this gate ("" when
    /// unconditional). Derived from the sorted flag names, so it cannot depend on
    /// declaration order or map iteration.
    fn key(&self) -> String {
        if self.is_unconditional() {
            return String::new();
        }
        let mut acc: u64 = 0xcbf2_9ce4_8422_2325;
        for (tag, set) in [("+", &self.requires), ("-", &self.forbids)] {
            for f in set {
                for b in tag.bytes().chain(f.bytes()) {
                    acc ^= b as u64;
                    acc = acc.wrapping_mul(0x0000_0100_0000_01b3);
                }
            }
        }
        format!("_b{acc:08x}")
    }
}

/// One staged position for a body, and the branch condition it was staged under.
#[derive(Clone, Debug)]
struct Staging {
    /// The branch this staging happened on.
    gate: BranchGate,
    /// Where it left the body (snapped floor cell).
    pos: [i32; 3],
    /// The facing it left the body in, when the leg planned one.
    yaw: Option<i32>,
}

/// The staging history of every walked body, in campaign effect order.
type StagingHistory = BTreeMap<String, Vec<Staging>>;

/// The most recent staging of `body` that provably already happened on the branch
/// a leg gated by `gate` runs on — the origin that leg's walk must start from.
///
/// Walks the history backwards and takes the first entry whose own gate is
/// implied by `gate`. `None` means nothing in the history is provable on this
/// branch, and the caller falls back to the body's declared home anchor — the
/// pre-chaining behaviour, which is right precisely when no prior leg is proven.
fn chained_staging<'a>(
    history: &'a StagingHistory,
    body: &str,
    gate: &BranchGate,
) -> Option<&'a Staging> {
    history
        .get(body)?
        .iter()
        .rev()
        .find(|s| s.gate.implied_by(gate))
}

/// Record where a leg left a body, on the branch it ran on.
fn record_staging(
    history: &mut StagingHistory,
    body: &str,
    gate: BranchGate,
    pos: [i32; 3],
    yaw: Option<i32>,
) {
    history
        .entry(body.to_string())
        .or_default()
        .push(Staging { gate, pos, yaw });
}

/// `DW0488` for a deduped occurrence whose branch-correct origin is not the one
/// the shared driver was planned from.
fn shared_origin_error(
    verb: &str,
    body: &str,
    to_anchor: &str,
    planned_from: [i32; 3],
    planned_gate: &BranchGate,
    actual_from: [i32; 3],
    this_gate: &BranchGate,
) -> NavError {
    let describe = |g: &BranchGate| {
        if g.is_unconditional() {
            "unconditionally".to_string()
        } else {
            let mut parts = Vec::new();
            if !g.requires.is_empty() {
                parts.push(format!(
                    "requires {}",
                    g.requires.iter().cloned().collect::<Vec<_>>().join(", ")
                ));
            }
            if !g.forbids.is_empty() {
                parts.push(format!(
                    "forbids {}",
                    g.forbids.iter().cloned().collect::<Vec<_>>().join(", ")
                ));
            }
            format!("when it {}", parts.join(" and "))
        }
    };
    NavError {
        code: DW_MOVE_ORIGIN_SHARED,
        message: format!(
            "{verb}: `{body}` walks to `{to_anchor}` from two different places, but both beats \
             share ONE emitted walk driver, so one of them opens by teleporting the body across \
             the map. The driver is planned from {planned_from:?} (the occurrence that fires \
             {}), while the occurrence that fires {} leaves the body at {actual_from:?}. A walk \
             driver is content-keyed by `(body, destination)`, so it can carry only one origin. \
             Prescription: give the two beats distinct destinations (a second anchor a step apart \
             reads identically in play), or walk the body to a shared staging mark first so both \
             occurrences start from the same cell",
            describe(planned_gate),
            describe(this_gate),
        ),
    }
}

/// Default NPC walking speed in blocks/tick (spec-0008 §5; owner spike). Used when
/// a `move-npc` effect omits `speed`.
pub const DEFAULT_SPEED: f64 = 0.15;

/// An entity's collision footprint over the voxel grid: the set of column offsets
/// it occupies horizontally and the number of vertical cells it needs clear
/// (`ceil(height)`). Standing feet-centred on a cell, an entity of `width <= 1`
/// occupies a single column; a taller entity needs more headroom (the warden, 2.9
/// tall, needs 3 cells vs a player's 2 — so it cannot walk a 2-high gap a player
/// fits). Drives footprint-aware standability + A* so a `move-actor` path is
/// walkable for the ACTUAL puppet, not a generic 1×2 humanoid (spec-0014).
#[derive(Debug, Clone)]
pub struct Footprint {
    /// Horizontal column offsets `[dx, dz]` the body occupies (feet cell = `[0, 0]`).
    cols: Vec<[i32; 2]>,
    /// Vertical cells of clearance the body needs (`ceil(height)`, min 1).
    height: i32,
}

impl Footprint {
    /// The footprint for the given hitbox `width` × `height` in blocks. Feet-centred
    /// on a cell: columns are the unit cells the width-wide AABB overlaps; height is
    /// `ceil(height)` (min 1).
    pub fn for_dims(width: f64, height: f64) -> Footprint {
        let half = width / 2.0;
        let lo = (0.5 - half).floor() as i32;
        let hi = (0.5 + half - 1e-9).floor() as i32;
        let mut cols = Vec::new();
        for dx in lo..=hi {
            for dz in lo..=hi {
                cols.push([dx, dz]);
            }
        }
        if cols.is_empty() {
            cols.push([0, 0]);
        }
        let h = (height.ceil() as i32).max(1);
        Footprint { cols, height: h }
    }

    /// The default humanoid footprint (player / villager / mannequin: 0.6 × 1.8 →
    /// single column, 2 cells tall). Byte-identical to the pre-spec-0014 walkability
    /// model, so `move-npc` and critical-path routing are unchanged.
    pub fn player() -> Footprint {
        Footprint::for_dims(0.6, 1.8)
    }
}

/// The standing hitbox `(width, height)` in blocks for a vanilla entity id
/// (spec-0014 per-entity dims table) — the ONE table in the compiler that knows
/// how big a mob's body is. Covers the 1.21.11 mobs an actor or a re-dressed NPC
/// mannequin is likely to wear; anything unlisted falls back to the humanoid
/// default (0.6 × 1.95).
///
/// Two consumers, deliberately sharing one source of truth: [`entity_footprint`]
/// quantizes it to walkable cells for actor routing, and [`crate::eclipse`] uses
/// the raw floats for the sub-block body-vs-affordance overlap test (`DW0359`) —
/// a rule the cell-quantized view could not state honestly (a 1.4-wide iron
/// golem occupies three columns of *clearance* but its body is only 1.4 blocks
/// of *hitbox*).
pub fn entity_dims(entity: &str) -> (f64, f64) {
    match entity.strip_prefix("minecraft:").unwrap_or(entity) {
        "warden" => (0.9, 2.9),
        "iron_golem" => (1.4, 2.7),
        "ravager" => (1.95, 2.2),
        "hoglin" | "zoglin" => (1.4, 1.4),
        "sheep" | "goat" | "pig" | "cow" | "mooshroom" | "wolf" | "fox" | "panda" => (0.9, 1.4),
        "villager" | "zombie" | "husk" | "zombie_villager" => (0.6, 1.95),
        "skeleton" | "stray" | "wither_skeleton" => (0.6, 1.99),
        "creeper" | "enderman" => (0.6, 1.9),
        "allay" | "vex" => (0.35, 0.6),
        // The player's own row reads the metrics table, so the body every proof
        // in this engine routes is the body `delvec metrics` publishes. The mobs
        // around it stay literals: they are not the player, and a metrics table
        // that enumerated the 1.21.11 mob roster would be a registry dump.
        "armor_stand" | "player" | "mannequin" => (
            delvewright_dsl::metrics::PLAYER_WIDTH,
            delvewright_dsl::metrics::PLAYER_HEIGHT,
        ),
        _ => (0.6, 1.95),
    }
}

/// The collision height of a vanilla fence / wall / closed fence gate, in blocks
/// 1.5, half a block above the cell it sits in.
pub const BARRIER_HEIGHT: f64 = 1.5;

/// The entity id whose body a stage-2 NPC actually wears in the shipped delve.
/// A skinned NPC is summoned as `minecraft:mannequin` — the player model, not the
/// declared `base_entity` (see `emit::npc_summon_commands`) — so every geometric
/// proof about NPC bodies ([`crate::eclipse`], [`crate::clearance`]) must model
/// what ships, not what is declared. One helper, so the two cannot drift.
pub fn npc_body_entity(n: &delvewright_dsl::Npc) -> String {
    match &n.skin {
        Some(_) => "minecraft:mannequin".to_string(),
        None => n.base_entity.clone(),
    }
}

/// The entity id whose body a stage-5 actor wears — the actor's counterpart of
/// [`npc_body_entity`], same mannequin rule.
pub fn actor_body_entity(a: &delvewright_dsl::Actor) -> String {
    match &a.skin {
        Some(_) => "minecraft:mannequin".to_string(),
        None => a.entity.clone(),
    }
}

/// The hitbox footprint for a vanilla entity id (spec-0014 per-entity dims table).
/// Standing hitboxes for the 1.21.11 mobs an actor is likely to puppet; anything
/// unlisted falls back to the humanoid default (0.6 × 1.95). Width only matters
/// past 1.0 (sub-block mobs are single-column); height gates vertical clearance.
pub fn entity_footprint(entity: &str) -> Footprint {
    let (w, h) = entity_dims(entity);
    Footprint::for_dims(w, h)
}

/// How far to search for a standable floor cell when a `move-npc` endpoint anchor
/// is a solid affordance (altar / gate bars / wall marker) the NPC must stop in
/// front of rather than stand inside.
pub const SNAP_RADIUS: i32 = 3;

/// A reachability root: a declared cell, plus the AABB the snap that seats it may
/// not leave — the assembled piece that DECLARES the anchor.
///
/// The confinement exists because [`World::snap_in_bounds`] (and so
/// [`World::snap`]) chooses the nearest standable cell by squared distance and
/// **nothing else**: it does not care that solid geometry stands between the
/// anchor and the cell it lands on. An anchor a campaign must declare in a room's
/// ceiling — every spec-0022 `collapse` payload has one — is therefore closer to
/// the cell on top of the ROOF than to the floor below it, and snaps up through
/// the ceiling onto a component no player can ever walk to. Every proof rooted
/// there then reasons about the roof: boundary safety
/// ([`verify_boundary_safety`]) demanded a safe edge on a bare platform in a void
/// world, which is unsatisfiable by construction.
///
/// This is the same leak [`World::confined_standable_cells`] closed for wave
/// seating, one layer up: there a flood from a wave anchor crossed a
/// socket seam into the neighbouring piece, here a *snap* crosses a ceiling into
/// nothing. The piece AABB is the boundary in both cases because it is the only
/// shape that says "the room this anchor was authored inside".
///
/// Only the seating is confined. The walk that follows is deliberately not: a
/// player who reaches a room reaches whatever it connects to, and confining the
/// flood would shrink the region a boundary proof examines — a weaker check, not
/// a more correct one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnchorRoot {
    /// The declared cell (a `Point` anchor's position, a `Gate` anchor's `from`).
    pub at: [i32; 3],
    /// World AABB `(min, max)` of the piece that declares it.
    pub within: ([i32; 3], [i32; 3]),
}

/// A build diagnostic raised by navigation planning (mapped to exit 3, `DW03xx`).
#[derive(Debug)]
pub struct NavError {
    /// The stable diagnostic code (`DW0307` / `DW0308`).
    pub code: DwCode,
    /// Human-readable explanation, naming the offending NPC / endpoints / segment.
    pub message: String,
}

/// A planned `move-npc`: the resolved endpoints plus the per-tick waypoint
/// polyline the emitter teleports the NPC body + interaction hitbox along.
/// `waypoints[0]` is the origin and `waypoints.last()` is exactly the integer
/// target cell; there are `ticks() + 1` entries.
#[derive(Debug, Clone)]
pub struct MovePlan {
    /// The moving NPC id (`npc/…`).
    pub npc: String,
    /// The destination anchor id (`anchor/…`).
    pub to_anchor: String,
    /// The integer target cell (feet), for the arrival assertion.
    pub target: [i32; 3],
    /// The A* **cell** path this leg walks, start to target inclusive — the route
    /// before [`resample`] turns it into per-tick positions. Kept because the
    /// per-tick positions answer *where the body is* while a traversal proof
    /// ([`crate::traversal`]) must ask *what move the body made*: which cell it
    /// entered, and which cell it stepped up onto.
    pub cells: Vec<[i32; 3]>,
    /// Per-tick world positions along the walked path.
    pub waypoints: Vec<[f64; 3]>,
    /// Per-waypoint yaw (degrees), the bearing of the segment the body is walking
    /// (see [`yaws_along`]). Without it a tp'd body keeps a stale yaw and glides
    /// backwards — owner playtest, island round 13.
    pub yaws: Vec<i32>,
    /// The branch-gate component of this driver's content key ([`gate_key`]);
    /// empty for an unconditional walk.
    pub gate_key: String,
}

impl MovePlan {
    /// The final tick index (`waypoints.len() - 1`).
    pub fn ticks(&self) -> usize {
        self.waypoints.len().saturating_sub(1)
    }
}

/// The **world-generator ambient** the placed geometry sits in — what a column
/// the compiler modelled nothing into actually contains in the delivered world
/// (spec-0013 `horizon`).
///
/// The assembled model ([`crate::assembled`]) knows only cells a prefab, a socket
/// seal or an edit wrote. Everything else is "absent", and what *absent* means is
/// a property of the level generator, not of the content: under `horizon: void`
/// an absent column is bottomless; under `horizon: ocean` it is the pinned
/// bedrock/stone/water superflat, so there is no void anywhere in the world and
/// stepping off the land is swimming. Boundary safety ([`verify_boundary_safety`])
/// is the one proof whose premise is exactly this, so the ambient rides on the
/// [`World`] rather than being re-derived (or, as before, silently assumed to be
/// `Void`) at the call site.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Ambient {
    /// `horizon: void` (default/absent) — an empty superflat layer list. Every
    /// column the content did not build is bottomless.
    #[default]
    Void,
    /// `horizon: ocean` — the pinned bedrock/stone/water superflat ([`Sea`]).
    Ocean(Sea),
}

/// The ocean horizon's ambient sea: a global water plane topping out at
/// [`Sea::level`], solid ground from [`Sea::floor_top`] down, and air above —
/// present in **every** column except those a placed piece overwrote (the
/// world's [`World::built`] volume).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sea {
    /// Y of the topmost ambient water block (`crate::plan::SEA_LEVEL`, 62).
    pub level: i32,
    /// Y of the topmost ambient solid block — the sea floor
    /// (`crate::plan::SEA_FLOOR_TOP_Y`, 54). Ambient water occupies
    /// `floor_top+1 ..= level`.
    pub floor_top: i32,
}

impl Ambient {
    /// The ambient a campaign's `horizon` declares (spec-0013). Purely the
    /// generator's own facts: **where the content ends is not one of them** —
    /// that is [`built_volume`], a property of the assembled world under every
    /// horizon (see [`World::built`]).
    pub fn of_plan(plan: &Plan) -> Ambient {
        match plan.campaign.world.content.horizon {
            Some(delvewright_dsl::Horizon::Ocean) => Ambient::Ocean(Sea {
                level: crate::plan::SEA_LEVEL,
                floor_top: crate::plan::SEA_FLOOR_TOP_Y,
            }),
            _ => Ambient::Void,
        }
    }

    /// The horizon's own name, for a message or a ledger.
    pub fn name(&self) -> &'static str {
        match self {
            Ambient::Void => "void",
            Ambient::Ocean(_) => "ocean",
        }
    }
}

/// **Where the content decided what is there**: every placed piece's prefab id
/// paired with its inclusive world AABB, in plan order (area order, entry piece
/// first — deterministic, ADR-0006).
///
/// `/place template` writes the whole box, air included, so inside a box the
/// piece's bytes decide and the world generator does not apply; outside every
/// box the generator's [`Ambient`] does. Two proofs ask that one question — the
/// ocean's `ambient_water`, and [`measure_fluid_escape`] — and it is a fact
/// about the ASSEMBLED WORLD, not about the sea, which is why it lives on
/// [`World`] rather than on [`Sea`]. It was a field of `Sea` first; a world
/// under `horizon: void` therefore had no idea where its own content ended, and
/// water that ran off the last piece met nothing that could judge it.
pub fn built_volume(plan: &Plan) -> Vec<BuiltPiece> {
    plan.areas
        .iter()
        .flat_map(|a| a.pieces.iter().map(|p| (p.prefab_id.clone(), p.bbox())))
        .collect()
}

/// A collision/standability model of the assembled world (spec-0008 addendum),
/// derived from the shared gravity-settled assembled-world model
/// ([`crate::assembled`]): every placed prefab block, plus the solver's socket
/// seals, with gate thresholds cleared and unsupported falling blocks settled
/// Cells absent from both sets are passable (interior air, opened
/// sockets, gate thresholds, and any cell a gravity block fell out of).
///
/// Water is modelled separately from solids: `flooded` holds every cell
/// a conservative superset of vanilla water flow reaches (see
/// [`crate::assembled::assembled_occupancy`]). A flooded cell is **impassable** (a
/// walker cannot stand or pass through it) yet is **not solid floor** (you cannot
/// stand *on* a water surface) — the two sets are disjoint and both gate
/// standability, so nav / wave seating / relight / waypoint export never treat a
/// flooded cell as walkable ground.
///
/// Collision classes ([`crate::assembled::Occupancy`]): `solid` holds
/// only full-cube cells (passage-blocking AND valid floor); `tall` holds 1.5-tall
/// fence/wall cells (passage-blocking, **never** valid floor — a walking player
/// cannot jump 1.5, so a fence-top is not standable and the old full-solid model's
/// "stand on the fence" routes are gone); `use_gates` holds closed fence-gate
/// cells, passable for the **player** via an adventure-legal right-click (a
/// "use-gate" edge, exported to the harness) but impassable for NPC/actor/wave
/// walkers ([`World::without_gate_use`]) — and never valid floor either, so no
/// route stands on a gate-top. Because a tall/gate cell is never floor, the cell
/// above it has no footing, which also models the barrier's upper half blocking
/// walk-overs at `y+1` for free.
///
/// Partial floor heights: `partial` records, for a `solid` cell whose
/// walkable top face sits **below** the cell top (a bottom slab at 8/16, a snow
/// drift, a `dirt_path` at 15/16), that true height. It is what makes
/// [`World::neighbors_fp`] a physical step rule rather than a cell-adjacency rule
/// — see [`MAX_AUTO_STEP_16`] / [`MAX_JUMP_RISE_16`].
/// One declared lethal volume as the navigation model carries it: `(id, box)`,
/// the box being inclusive world-space corners.
type LethalRegion = (String, ([i32; 3], [i32; 3]));

/// One placed piece as the built volume carries it: `(prefab id, box)`, the box
/// being inclusive world-space corners. Same shape as [`LethalRegion`] and for
/// the same reason — a proof that refuses over a region has to be able to NAME
/// the region, or the author is sent to look at geometry that was never wrong.
pub type BuiltPiece = (String, ([i32; 3], [i32; 3]));

pub struct World {
    solid: BTreeSet<[i32; 3]>,
    tall: BTreeSet<[i32; 3]>,
    use_gates: BTreeSet<[i32; 3]>,
    flooded: BTreeSet<[i32; 3]>,
    /// For each `solid` cell whose walkable top face sits **below** the cell
    /// top, that height in sixteenths. Absent = a full cube. Feeds
    /// the physical step rule in [`World::neighbors_fp`].
    partial: BTreeMap<[i32; 3], u8>,
    /// Cells inside a declared **lethal volume** (DSL v0.10, spec-0031).
    ///
    /// A volume that kills whatever enters it is, for a route, a volume no route
    /// may enter — so these cells are **impassable**, exactly like a flooded one,
    /// and for the same reason: a walker that goes there does not come out the
    /// other side. They are deliberately *not* added to `solid`: a lethal cell is
    /// not floor, so nothing may stand on top of one either. Empty for every
    /// campaign that declares no volume, which is what keeps routing, standability
    /// and every downstream proof byte-identical.
    lethal: BTreeSet<[i32; 3]>,
    /// The declared volumes behind `lethal`, as `(id, box)`, in declaration
    /// order. Carried so a route failure can NAME the volume that caused it
    /// rather than report an unroutable leg over geometry that looks open — the
    /// difference between `DW0510` and a `DW0311` that sends the author to fix a
    /// prefab that was never wrong.
    lethal_regions: Vec<LethalRegion>,
    /// Cells another proof has FORCED solid as its own premise — a `collapse`'s
    /// settled debris, an ambush's occupied cells, a timed gate's shut span, an
    /// aggro sphere ([`World::with_sealed`]).
    ///
    /// A runtime `clear-region` may not remove one. Clearing a region says "the
    /// blocks the campaign put here are gone"; it does not say "the hazard another
    /// proof is reasoning about never happened". Without this, a `clear-region`
    /// laid over a collapse's rubble would delete the rubble from that proof's
    /// world and `DW0445` would go quietly green — a new verb weakening an existing
    /// check, which is the one thing a new verb may never do.
    pinned: BTreeSet<[i32; 3]>,
    /// **What the world authors shut at world-load**: one entry per gate anchor
    /// whose region the placed prefabs fill with a block
    /// ([`crate::assembled::GateSeal`]), as a world-load `Fill` at step 0.
    ///
    /// This is a property of the WORLD, not of any verb: the bars in
    /// `hello-room`'s doorway are there because the `.nbt` puts them there, and
    /// they stay there until something clears them. It lives beside the region
    /// model rather than on the [`Plan`] because the plan is built before the
    /// `.nbt` bytes are read, and because the one question it answers — *what is
    /// solid at this point of the quest DAG* ([`World::region_state_at`]) — is
    /// already this type's.
    ///
    /// Every gate is listed, sealed or not, because the ledger this backs
    /// (`validation/gate-seal.json`) has to state what was EXAMINED and not only
    /// what was found; [`World::modelled_seals`] is the subset the model treats as
    /// shut. Empty for a synthetic test world and for every campaign with no gate
    /// anchor at all — which is what keeps those routings byte-identical.
    world_load_seals: Vec<crate::assembled::GateSeal>,
    /// Gate regions a `timed-gate`'s clock owns (spec-0016 §4) — measured like any
    /// other gate and never modelled as shut, because the clock clears them twice
    /// a cycle from world-load. See [`World::with_world_load_seals`].
    clocked_gates: BTreeSet<([i32; 3], [i32; 3])>,
    /// **Where the party can be carried instead of walking** — every declared
    /// `teleport`'s source volume ([`Plan::transit_teleports`]). A walked leg whose
    /// start lies in one of these is not judged against a world-load gate seal: the
    /// party may never stand there long enough to need the door. Empty for every
    /// campaign that declares no `teleport`.
    transit_teleports: Vec<([i32; 3], [i32; 3])>,
    /// Cells a **runtime fluid fill** has flooded on this view
    /// ([`crate::plan::RegionWrite::Flood`], [`World::with_flooded`]) — a subset of
    /// `flooded`, kept apart from the prefab-authored water only so the
    /// counterfactual [`World::without_runtime_flood`] can put exactly these cells
    /// back and no others. Empty on the base world and on every campaign that never
    /// fills a region with a fluid, which is what keeps every other campaign's
    /// proofs byte-identical.
    flood_written: BTreeSet<[i32; 3]>,
    /// The boxes behind `flood_written`, in region order — who to blame for a route
    /// that only exists when a fluid is mistaken for floor. Same job as
    /// `lethal_regions`, for the same reason: without it the author gets "no
    /// collision-free path" over geometry that looks open.
    flood_regions: Vec<([i32; 3], [i32; 3])>,
    /// What the *unmodelled* columns contain (spec-0013 `horizon`). Defaults to
    /// [`Ambient::Void`] — the pre-0.6 world and every synthetic test world —
    /// and is set from the plan by [`World::from_plan`] /
    /// [`World::with_ambient`]. Read only by [`verify_boundary_safety`] and
    /// [`measure_fluid_escape`]; it deliberately does **not** feed the
    /// walkability sets, so routing, standability and every other proof stay
    /// byte-identical.
    ambient: Ambient,
    /// **Where the content ends** ([`built_volume`]): every placed piece's
    /// prefab id and inclusive world AABB. Empty on a synthetic test world,
    /// which is the fail-CLOSED direction for [`measure_fluid_escape`] — a
    /// world with no known built volume can prove nothing contained, rather than
    /// proving everything contained. Set beside `ambient` by
    /// [`World::with_ambient`], whose signature takes both so no call site can
    /// set the premise and forget the extent.
    built: Vec<BuiltPiece>,
}

/// The step rule's three constants, taken from the metrics table (spec-0049 §2)
/// rather than declared here.
///
/// The direction of the import is what the single-authority obligation means
/// concretely: the exported player metrics ARE these constants at compile time,
/// not a second table that agrees with them, so `delvec metrics` cannot describe
/// a walker this model does not route. Their derivations are on the definitions
/// (`dsl::metrics::MAX_AUTO_STEP_16` is vanilla's 0.6-block `maxUpStep` rounded
/// down to a sixteenth; `MAX_JUMP_RISE_16` is the ≈1.2522-block apex), and they
/// stay private to this module because the step rule is this module's, while the
/// numbers are the table's.
use delvewright_dsl::metrics::{FULL_16, MAX_AUTO_STEP_16, MAX_JUMP_RISE_16};

impl World {
    /// Build the occupancy model from the plan's placed pieces and the structure
    /// `.nbt` bytes, via the shared assembled-world model. Every non-air cell of
    /// that settled map is a solid cell here — so a `sand`/`gravel` floor that
    /// falls out of the void world is passable (a hole), exactly as in game
    /// — not a phantom floor the model wrongly seats mobs on.
    pub fn from_plan(plan: &Plan, structures: &BTreeMap<String, Vec<u8>>) -> Self {
        let assembled = crate::assembled::assemble(plan, structures);
        let seals = assembled.gate_seals.clone();
        Self::from_occupancy(crate::assembled::occupancy_of(
            assembled.blocks,
            &assembled.open_gates,
        ))
        .with_ambient(Ambient::of_plan(plan), built_volume(plan))
        .with_lethal(plan)
        .with_world_load_seals(plan, seals)
    }

    /// This world with the measured world-load gate seals it should carry
    /// ([`World::world_load_seals`]).
    ///
    /// Two classes of measured seal are deliberately **dropped** here, and both
    /// drops are optimism the model states rather than hides:
    ///
    /// * A gate authored **open** (`blocked == 0`) is not a seal at all. The
    ///   island boulder is twenty-seven cells of air until a `close-gate` fills
    ///   it, and that firing is already in [`Plan::region_events`].
    /// * A **timed gate**'s region (spec-0016 §4) is filled and cleared by its own
    ///   clock, twice a cycle, from world-load. Whatever the prefab authors there,
    ///   the region is open for part of every cycle, so modelling it as
    ///   permanently shut would refuse a campaign that plays. The clock's own
    ///   proofs own that region; this one declines it.
    ///
    /// A **shortcut**'s gate is kept: `Plan::build` already registers it as a
    /// world-load `Fill` for exactly this reason, and a duplicate write of the
    /// same region at the same step is the same verdict either way — keeping it
    /// is what lets the diagnostic NAME a shortcut door that walls off the only
    /// route.
    pub fn with_world_load_seals(
        mut self,
        plan: &Plan,
        seals: Vec<crate::assembled::GateSeal>,
    ) -> World {
        self.clocked_gates = plan.timed_gates.iter().map(|g| g.gate_region).collect();
        self.world_load_seals = seals;
        self.transit_teleports = plan.transit_teleports.clone();
        self
    }

    /// The measured gates the model actually treats as **shut at world-load** —
    /// the two exclusions [`World::with_world_load_seals`] documents.
    fn modelled_seals(&self) -> impl Iterator<Item = &crate::assembled::GateSeal> {
        self.world_load_seals
            .iter()
            .filter(|s| s.sealed() && !self.clocked_gates.contains(&s.region))
    }

    /// Whether any gate is modelled shut — the cheap guard that keeps a campaign
    /// with no sealed gate on exactly its old routing.
    fn has_world_load_seals(&self) -> bool {
        self.modelled_seals().next().is_some()
    }

    /// Whether `cell` sits inside a declared `teleport` source volume — i.e. the
    /// party may be carried off it rather than walk away from it
    /// ([`Plan::transit_teleports`]).
    fn is_teleport_source(&self, cell: [i32; 3]) -> bool {
        self.transit_teleports.iter().any(|(lo, hi)| {
            (0..3).all(|i| lo[i].min(hi[i]) <= cell[i] && cell[i] <= lo[i].max(hi[i]))
        })
    }

    /// Every gate anchor whose world-load seal a route's `cells` pass through, in
    /// `(area, anchor)` order — the blame list [`DW_GATE_NEVER_OPENED`] names.
    fn gate_seals_over(&self, cells: &[[i32; 3]]) -> Vec<&crate::assembled::GateSeal> {
        self.modelled_seals()
            .filter(|s| {
                let (lo, hi) = s.region;
                cells
                    .iter()
                    .any(|c| (0..3).all(|i| lo[i].min(hi[i]) <= c[i] && c[i] <= lo[i].max(hi[i])))
            })
            .collect()
    }

    /// The world-load gate ledger, for the binding count a validation artifact must
    /// state (CLAUDE.md): every gate examined, and how many of them the model
    /// treats as shut.
    pub fn gate_seal_ledger(&self) -> serde_json::Value {
        crate::assembled::gate_seal_ledger(&self.world_load_seals, self.modelled_seals().count())
    }

    /// Whether the layout resolved any gate anchor at all — a campaign with none
    /// emits no ledger, so a ledger that exists and reports zero is a finding.
    pub fn has_gate_anchors(&self) -> bool {
        !self.world_load_seals.is_empty()
    }

    /// This world with the plan's declared **lethal volumes** (DSL v0.10,
    /// spec-0031) marked impassable.
    ///
    /// This is where "a volume that kills" becomes "a volume no proof may route
    /// through", and it is deliberately applied to the base world rather than
    /// per-leg the way a `close-gate`'s seal is: a seal has a point in the quest
    /// DAG before which the region is open, and a lethal volume has none — it
    /// kills from world-load, in every branch, on every leg. A campaign with no
    /// volume gets the identical world back (the set is empty), so nothing moves
    /// for anybody who has not declared one.
    fn with_lethal(mut self, plan: &Plan) -> World {
        for v in &plan.lethal_volumes {
            self.lethal
                .extend(crate::assembled::region_cells(v.region.0, v.region.1));
            self.lethal_regions.push((v.id.clone(), v.region));
        }
        self
    }

    /// A copy of this world with **no** lethal volumes — the counterfactual the
    /// `DW0510` diagnostic is derived from.
    ///
    /// A route that fails on the real world and succeeds on this one failed
    /// *because of* a lethal volume, and the cells it would have walked name which
    /// volumes. Without the counterfactual the author gets "no collision-free
    /// path" over geometry that looks perfectly open — the reachability report
    /// that sends someone to fix the prefab.
    fn without_lethal(&self) -> World {
        World {
            solid: self.solid.clone(),
            tall: self.tall.clone(),
            use_gates: self.use_gates.clone(),
            flooded: self.flooded.clone(),
            partial: self.partial.clone(),
            lethal: BTreeSet::new(),
            lethal_regions: Vec::new(),
            pinned: self.pinned.clone(),
            world_load_seals: self.world_load_seals.clone(),
            clocked_gates: self.clocked_gates.clone(),
            transit_teleports: self.transit_teleports.clone(),
            flood_written: self.flood_written.clone(),
            flood_regions: self.flood_regions.clone(),
            ambient: self.ambient.clone(),
            built: self.built.clone(),
        }
    }

    /// Whether this world carries any lethal-volume cell at all. Call sites skip
    /// the counterfactual clone entirely when it does not.
    pub fn has_lethal(&self) -> bool {
        !self.lethal.is_empty()
    }

    /// Whether `c` lies inside a declared lethal volume.
    pub fn is_lethal(&self, c: [i32; 3]) -> bool {
        self.lethal.contains(&c)
    }

    /// How many cells this world's lethal volumes occupy — the binding count the
    /// `validation/lethal-gate.json` ledger states out loud.
    pub fn lethal_cells(&self) -> usize {
        self.lethal.len()
    }

    /// The ids of the lethal volumes covering any of `cells`, in declaration
    /// order — who to blame for a route that only exists when lethality is
    /// ignored. Deterministic: declaration order, no hashing (ADR-0006).
    fn lethal_volumes_over(&self, cells: &[[i32; 3]]) -> Vec<&str> {
        self.lethal_regions
            .iter()
            .filter(|(_, (lo, hi))| {
                cells
                    .iter()
                    .any(|c| (0..3).all(|i| lo[i] <= c[i] && c[i] <= hi[i]))
            })
            .map(|(id, _)| id.as_str())
            .collect()
    }

    /// Build the walkability model from a collision-classified [`Occupancy`]
    /// — the sets map across one-to-one.
    pub fn from_occupancy(occ: crate::assembled::Occupancy) -> Self {
        World {
            solid: occ.solid,
            tall: occ.tall,
            use_gates: occ.use_gates,
            flooded: occ.flooded,
            partial: occ.partial,
            lethal: BTreeSet::new(),
            lethal_regions: Vec::new(),
            pinned: BTreeSet::new(),
            world_load_seals: Vec::new(),
            clocked_gates: BTreeSet::new(),
            transit_teleports: Vec::new(),
            flood_written: BTreeSet::new(),
            flood_regions: Vec::new(),
            ambient: Ambient::Void,
            built: Vec::new(),
        }
    }

    /// This world with its world-generator [`Ambient`] declared (spec-0013) and
    /// its [`built_volume`] recorded. The occupancy sets are untouched — both are
    /// *premises* ([`verify_boundary_safety`], [`measure_fluid_escape`]), not
    /// geometry.
    ///
    /// The two travel in one argument list on purpose. They answer the same
    /// question from opposite sides — *what is in a column the content did not
    /// build* and *which columns are those* — and a call site that declared the
    /// first without the second is how a void world came to have no idea where
    /// its own content ended.
    pub fn with_ambient(mut self, ambient: Ambient, built: Vec<BuiltPiece>) -> Self {
        self.ambient = ambient;
        self.built = built;
        self
    }

    /// Whether `c` falls inside a placed piece's AABB — i.e. whether the
    /// content, rather than the world generator, decided what is in that cell.
    pub fn is_built(&self, c: [i32; 3]) -> bool {
        self.built
            .iter()
            .any(|(_, (lo, hi))| (0..3).all(|a| lo[a] <= c[a] && c[a] <= hi[a]))
    }

    /// Whether `c` is ambient sea water: inside the ocean generator's water
    /// layers and not overwritten by a placed piece. Always `false` under
    /// [`Ambient::Void`], which has no water anywhere.
    fn ambient_water(&self, c: [i32; 3]) -> bool {
        match &self.ambient {
            Ambient::Void => false,
            Ambient::Ocean(sea) => c[1] > sea.floor_top && c[1] <= sea.level && !self.is_built(c),
        }
    }

    /// Build the occupancy model exactly like [`World::from_plan`], then add
    /// `extra_solid` cells (the relight pass's colliding fixtures — campfire /
    /// floor lantern — so post-relight nav verification sees them; spec-0010). A
    /// fixture that adds no collision (torch, wall/hanging fixtures, embedded
    /// shroomlight) contributes nothing here.
    pub fn from_plan_with_extra(
        plan: &Plan,
        structures: &BTreeMap<String, Vec<u8>>,
        extra_solid: &BTreeSet<[i32; 3]>,
    ) -> Self {
        let mut world = Self::from_plan(plan, structures);
        world.solid.extend(extra_solid.iter().copied());
        world
    }

    /// Whether a cell is occupied by a solid block in the assembled world.
    pub fn solid_at(&self, c: [i32; 3]) -> bool {
        self.is_solid(c)
    }

    /// A copy of this world with `extra` cells forced solid — a `close-gate`'s
    /// sealed region for the completability proof (DSL v0.6). The base occupancy
    /// model treats every gate cell as passable; sealing a gate for the legs that
    /// occur after it closes makes a path that must re-cross it fail routing.
    fn with_sealed(&self, extra: &BTreeSet<[i32; 3]>) -> World {
        let mut solid = self.solid.clone();
        solid.extend(extra.iter().copied());
        // A sealed gate cell is a full-cube wall, never a partial floor.
        let mut partial = self.partial.clone();
        for c in extra {
            partial.remove(c);
        }
        // These cells are this proof's premise from here on: a later runtime clear
        // may not undo them (see `World::pinned`).
        let mut pinned = self.pinned.clone();
        pinned.extend(extra.iter().copied());
        World {
            solid,
            tall: self.tall.clone(),
            use_gates: self.use_gates.clone(),
            flooded: self.flooded.clone(),
            partial,
            lethal: self.lethal.clone(),
            lethal_regions: self.lethal_regions.clone(),
            pinned,
            world_load_seals: self.world_load_seals.clone(),
            clocked_gates: self.clocked_gates.clone(),
            transit_teleports: self.transit_teleports.clone(),
            flood_written: self.flood_written.clone(),
            flood_regions: self.flood_regions.clone(),
            ambient: self.ambient.clone(),
            built: self.built.clone(),
        }
    }

    /// A copy of this world with `extra` cells **emptied of blocks** — the dual of
    /// [`World::with_sealed`], and what a runtime `clear-region` (or an
    /// `open-gate`, whose gate cells the assembled model already holds empty) does
    /// to the geometry from its point in the quest DAG.
    ///
    /// "Emptied of blocks" is exactly the four block-derived classes: a cleared
    /// cell is no longer a full cube, a 1.5-tall barrier, a closed fence gate, or a
    /// partial floor. `flooded` is deliberately **left alone**: a `fill … air`
    /// against a cell the model floods does not remove the water, it lets the water
    /// back in, so a cleared cell the model already knows is wet stays impassable.
    ///
    /// The one case this does not model is a clear that *opens* a dry region into
    /// adjacent water — the model would call the new cells dry and the server would
    /// flood them. Re-deriving the flood needs the block map, which this collision
    /// view does not carry; until it does, that campaign's route proof is optimistic
    /// and the limitation is stated here and in `docs/reference/compiler.md` rather
    /// than left to be discovered.
    fn with_cleared(&self, extra: &BTreeSet<[i32; 3]>) -> World {
        let mut w = World {
            solid: self.solid.clone(),
            tall: self.tall.clone(),
            use_gates: self.use_gates.clone(),
            flooded: self.flooded.clone(),
            partial: self.partial.clone(),
            lethal: self.lethal.clone(),
            lethal_regions: self.lethal_regions.clone(),
            pinned: self.pinned.clone(),
            world_load_seals: self.world_load_seals.clone(),
            clocked_gates: self.clocked_gates.clone(),
            transit_teleports: self.transit_teleports.clone(),
            flood_written: self.flood_written.clone(),
            flood_regions: self.flood_regions.clone(),
            ambient: self.ambient.clone(),
            built: self.built.clone(),
        };
        for c in extra {
            if w.pinned.contains(c) {
                continue; // another proof's premise, not a block this write owns
            }
            w.solid.remove(c);
            w.tall.remove(c);
            w.use_gates.remove(c);
            w.partial.remove(c);
        }
        w
    }

    /// A copy of this world with `extra` cells holding **free fluid** — the third
    /// thing a runtime region write can leave behind
    /// ([`crate::plan::RegionWrite::Flood`]), beside [`World::with_sealed`]'s wall
    /// and [`World::with_cleared`]'s empty box.
    ///
    /// A flooded cell is impassable and **never floor**, so this is not a weaker
    /// seal: it blocks passage exactly as a wall does, and additionally denies the
    /// footing a wall would have provided. Which is the whole point — `fill-region
    /// … minecraft:water` was previously routed as a wall *and* walked on top of.
    ///
    /// The written cells stop being any block class (a fill carries no `replace`
    /// filter, so it overwrites whatever was there), are recorded in
    /// `flood_written`/`flood_regions` for [`World::without_runtime_flood`], and are
    /// **pinned** for the same reason a seal is: they are this proof's premise, and
    /// a later `clear-region` laid over them may not quietly delete the water.
    fn with_flooded(&self, extra: &BTreeSet<[i32; 3]>, regions: &[([i32; 3], [i32; 3])]) -> World {
        let mut w = World {
            solid: self.solid.clone(),
            tall: self.tall.clone(),
            use_gates: self.use_gates.clone(),
            flooded: self.flooded.clone(),
            partial: self.partial.clone(),
            lethal: self.lethal.clone(),
            lethal_regions: self.lethal_regions.clone(),
            pinned: self.pinned.clone(),
            world_load_seals: self.world_load_seals.clone(),
            clocked_gates: self.clocked_gates.clone(),
            transit_teleports: self.transit_teleports.clone(),
            flood_written: self.flood_written.clone(),
            flood_regions: self.flood_regions.clone(),
            ambient: self.ambient.clone(),
            built: self.built.clone(),
        };
        for c in extra {
            w.solid.remove(c);
            w.tall.remove(c);
            w.use_gates.remove(c);
            w.partial.remove(c);
            w.flooded.insert(*c);
            w.pinned.insert(*c);
            w.flood_written.insert(*c);
        }
        w.flood_regions.extend_from_slice(regions);
        w
    }

    /// A copy of this world with `extra` cells holding a block that **may or may not
    /// be there** — a solid laid by a beat the party is not forced to play
    /// ([`RegionState::unforced`]).
    ///
    /// The cell is made impassable and **not floor**: impassable because the party
    /// may arrive to find the box walled, not floor because they may equally arrive
    /// to find it as the world built it. `tall` is exactly that class already — the
    /// model's word for "blocks passage, and nothing stands on top" — so this reuses
    /// it rather than inventing a fifth occupancy set for a property that has one.
    ///
    /// **A cell the base world already holds solid is left alone, and that is what
    /// keeps this from refusing correct campaigns.** If the box was floor before the
    /// write, then it is floor whether or not the write happens: both futures agree,
    /// there is nothing uncertain about standing there, and only a fill over a cell
    /// the world does NOT already floor can lend the path footing it might not have.
    /// So the rule binds precisely to laying NEW floor, which is the defect, and not
    /// to re-surfacing existing floor, which is decoration.
    fn with_unforced(&self, extra: &BTreeSet<[i32; 3]>) -> World {
        let mut w = World {
            solid: self.solid.clone(),
            tall: self.tall.clone(),
            use_gates: self.use_gates.clone(),
            flooded: self.flooded.clone(),
            partial: self.partial.clone(),
            lethal: self.lethal.clone(),
            lethal_regions: self.lethal_regions.clone(),
            pinned: self.pinned.clone(),
            world_load_seals: self.world_load_seals.clone(),
            clocked_gates: self.clocked_gates.clone(),
            transit_teleports: self.transit_teleports.clone(),
            flood_written: self.flood_written.clone(),
            flood_regions: self.flood_regions.clone(),
            ambient: self.ambient.clone(),
            built: self.built.clone(),
        };
        for c in extra {
            if w.solid.contains(c) {
                continue; // floor either way — no uncertainty to model
            }
            w.use_gates.remove(c);
            w.partial.remove(c);
            w.tall.insert(*c);
            // This proof's premise from here on, exactly as a seal or a flood is.
            w.pinned.insert(*c);
        }
        w
    }

    /// This world as of one point in the quest DAG: every region a runtime write
    /// has filled forced solid, every region a runtime write has cleared emptied,
    /// every region a runtime write has filled with a fluid flooded
    /// ([`RegionState`]).
    ///
    /// **The one place** that knows what a runtime region write does to the
    /// geometry a proof reasons over. `close-gate`, `open-gate`, `fill-region`,
    /// `clear-region` and a shortcut's world-load seal all arrive here as
    /// [`crate::plan::RegionEvent`]s and none of them carries its own copy of the
    /// rule.
    fn with_region_state(&self, st: &RegionState) -> World {
        // Writes are applied last-to-strictest, so where two regions overlap the
        // more restrictive answer wins and the result needs no tie-break on
        // declaration order (ADR-0006). A fill beats a clear — a proof that
        // survives the seal is the conservative answer — an UNFORCED fill beats a
        // forced one, being a wall that additionally may not be stood on, and a
        // flood beats them all, because a flooded cell is everything a walled cell
        // is (impassable) and one thing more (not floor).
        self.with_cleared(&st.cleared)
            .with_sealed(&st.solid)
            .with_unforced(&st.unforced)
            .with_flooded(&st.flooded, &st.flood_regions)
    }

    /// Whether any cell of this world was flooded by a **runtime** fluid fill.
    /// Call sites skip the counterfactual clone entirely when it is false, which is
    /// every campaign that does not fill a region with water or lava.
    pub fn has_runtime_flood(&self) -> bool {
        !self.flood_written.is_empty()
    }

    /// A copy of this world in which every runtime fluid fill is treated as a
    /// **solid** fill instead — the counterfactual the fluid diagnostic is derived
    /// from, and precisely the model's behaviour before a written block's identity
    /// was consulted at all.
    ///
    /// A route that fails on the real world and succeeds on this one failed
    /// *because* the campaign filled a region with a fluid and something needed to
    /// stand there. Without the counterfactual the author is handed "no
    /// collision-free path" over a box they can see is full — the reachability
    /// report that sends someone to fix a prefab that was never wrong.
    fn without_runtime_flood(&self) -> World {
        let mut w = World {
            solid: self.solid.clone(),
            tall: self.tall.clone(),
            use_gates: self.use_gates.clone(),
            flooded: self.flooded.clone(),
            partial: self.partial.clone(),
            lethal: self.lethal.clone(),
            lethal_regions: self.lethal_regions.clone(),
            pinned: self.pinned.clone(),
            world_load_seals: self.world_load_seals.clone(),
            clocked_gates: self.clocked_gates.clone(),
            transit_teleports: self.transit_teleports.clone(),
            flood_written: BTreeSet::new(),
            flood_regions: Vec::new(),
            ambient: self.ambient.clone(),
            built: self.built.clone(),
        };
        for c in &self.flood_written {
            w.flooded.remove(c);
            w.solid.insert(*c);
        }
        w
    }

    /// The runtime fluid-fill boxes a walk over `cells` **depends on**, in region
    /// order — who to blame for a route that only exists when a fluid is mistaken
    /// for floor.
    ///
    /// A route cell is where the body IS; the cell below it is what holds the body
    /// up, and that is the one a fluid fill usually takes away — `fill … water` at
    /// y=64 leaves the walk at y=65 standing on nothing while the route polyline
    /// never enters the box at all. Blaming only the occupied cells reports "(none)"
    /// for the commonest case there is, which is how a correct diagnostic still
    /// tells the author nothing. Both are checked; a box that covers either is
    /// named.
    ///
    /// Deterministic: region order, no hashing (ADR-0006).
    fn flood_regions_over(&self, cells: &[[i32; 3]]) -> Vec<([i32; 3], [i32; 3])> {
        let touched: Vec<[i32; 3]> = cells
            .iter()
            .flat_map(|c| [*c, [c[0], c[1] - 1, c[2]]])
            .collect();
        self.flood_regions
            .iter()
            .filter(|(lo, hi)| {
                touched
                    .iter()
                    .any(|c| (0..3).all(|i| lo[i] <= c[i] && c[i] <= hi[i]))
            })
            .copied()
            .collect()
    }

    /// A copy of this world for **autonomous** walkers that cannot use gates —
    /// wave mobs seated at spawn. Opening a fence gate is a right-click
    /// USE, so for a mob acting on its own a closed gate is exactly a 1.5-tall
    /// fence: the use-gate cells are folded into the tall-barrier set, and the
    /// seating flood neither seats a mob in a gate threshold nor spills through
    /// one. Scripted walks (`move-npc` / `move-actor`) deliberately do NOT use
    /// this view — see [`plan_moves`], and [`crate::traversal`]'s `DW0452` for
    /// the proof that keeps that choice honest. A world with no use-gates is returned
    /// unchanged in content (call sites skip the clone via
    /// [`World::has_use_gates`]).
    pub fn without_gate_use(&self) -> World {
        let mut tall = self.tall.clone();
        tall.extend(self.use_gates.iter().copied());
        World {
            solid: self.solid.clone(),
            tall,
            use_gates: BTreeSet::new(),
            flooded: self.flooded.clone(),
            partial: self.partial.clone(),
            lethal: self.lethal.clone(),
            lethal_regions: self.lethal_regions.clone(),
            pinned: self.pinned.clone(),
            world_load_seals: self.world_load_seals.clone(),
            clocked_gates: self.clocked_gates.clone(),
            transit_teleports: self.transit_teleports.clone(),
            flood_written: self.flood_written.clone(),
            flood_regions: self.flood_regions.clone(),
            ambient: self.ambient.clone(),
            built: self.built.clone(),
        }
    }

    /// Whether any closed fence-gate (use-gate) cell exists in this world.
    pub fn has_use_gates(&self) -> bool {
        !self.use_gates.is_empty()
    }

    /// Whether `c` is a closed fence-gate cell — a "use-gate": the player walks
    /// through it after an adventure-legal right-click. Exported per
    /// leg in the critical-path waypoint metadata so the harness knows the edge.
    pub fn is_use_gate(&self, c: [i32; 3]) -> bool {
        self.use_gates.contains(&c)
    }

    /// Build a [`World`] directly from a set of solid cells, with no water (test /
    /// synthetic entry point; the relight unit tests build a world without a full
    /// [`Plan`]).
    pub fn from_solid_cells(solid: BTreeSet<[i32; 3]>) -> Self {
        World {
            solid,
            tall: BTreeSet::new(),
            use_gates: BTreeSet::new(),
            flooded: BTreeSet::new(),
            partial: BTreeMap::new(),
            lethal: BTreeSet::new(),
            lethal_regions: Vec::new(),
            pinned: BTreeSet::new(),
            world_load_seals: Vec::new(),
            clocked_gates: BTreeSet::new(),
            transit_teleports: Vec::new(),
            flood_written: BTreeSet::new(),
            flood_regions: Vec::new(),
            ambient: Ambient::Void,
            built: Vec::new(),
        }
    }

    /// Build a [`World`] directly from disjoint solid + flooded cell sets (test /
    /// synthetic entry point for the flood-aware standability rules).
    pub fn from_solid_and_flooded(solid: BTreeSet<[i32; 3]>, flooded: BTreeSet<[i32; 3]>) -> Self {
        World {
            solid,
            tall: BTreeSet::new(),
            use_gates: BTreeSet::new(),
            flooded,
            partial: BTreeMap::new(),
            lethal: BTreeSet::new(),
            lethal_regions: Vec::new(),
            pinned: BTreeSet::new(),
            world_load_seals: Vec::new(),
            clocked_gates: BTreeSet::new(),
            transit_teleports: Vec::new(),
            flood_written: BTreeSet::new(),
            flood_regions: Vec::new(),
            ambient: Ambient::Void,
            built: Vec::new(),
        }
    }

    /// Whether a cell is a valid standing position (feet + head passable, solid
    /// ground below). Public wrapper over the internal walkability rule so the
    /// relight pass (spec-0010) can collect reachable walkable cells.
    pub fn is_standable(&self, c: [i32; 3]) -> bool {
        self.standable(c)
    }

    /// Whether a cell is unoccupied — neither a solid block nor water-flooded, so a
    /// camera eye placed in it sees open air rather than the inside of a block.
    /// Public wrapper for the visual-tier clear-eye self-check
    /// ([`verify_camera_eyes`]).
    pub fn is_clear(&self, c: [i32; 3]) -> bool {
        !self.is_occupied(c)
    }

    /// The top face of the **full-cube-class solid** occupying `c`, in sixteenths
    /// of a block, or `None` when no such block is there. A bottom slab answers
    /// `8`, a `dirt_path` `15`, a plain stone `16` — i.e. exactly the collision
    /// volume `c.y ..= c.y + top/16`, honouring the partial-floor table.
    /// Public so the body-clearance proof ([`crate::clearance`]) can
    /// intersect a real entity AABB against real block volumes rather than
    /// against whole cells.
    pub fn solid_top_16(&self, c: [i32; 3]) -> Option<u8> {
        if !self.solid.contains(&c) {
            return None;
        }
        Some(
            self.partial
                .get(&c)
                .copied()
                .unwrap_or(crate::assembled::FULL_HEIGHT_16),
        )
    }

    /// Whether `c` holds a **1.5-block-tall barrier** — a fence, a wall, or a
    /// closed fence gate ([`crate::assembled::is_tall_barrier`] /
    /// [`crate::assembled::is_fence_gate`]). Its collision volume rises
    /// [`BARRIER_HEIGHT`] from the cell floor but is a narrow post/panel
    /// horizontally, which is why the clearance proof treats it as advisory
    /// rather than as a wall.
    pub fn is_barrier(&self, c: [i32; 3]) -> bool {
        self.tall.contains(&c) || self.use_gates.contains(&c)
    }

    /// The nearest standable cell to `c` within `radius` (itself if already
    /// standable), broken deterministically by `(distance², cell)`; `None` if
    /// none. Public wrapper over `snap_standable` for the relight pass.
    pub fn snap(&self, c: [i32; 3], radius: i32) -> Option<[i32; 3]> {
        self.snap_standable(c, radius)
    }

    /// Every standable cell reachable by a walk (one-block step up/down, cardinal)
    /// from any of `starts`, over the assembled geometry. Deterministic BFS over a
    /// `BTreeSet` frontier with fixed neighbour order (ADR-0006). Starts that are
    /// not themselves standable are snapped within [`SNAP_RADIUS`] first; an
    /// unsnappable start contributes nothing.
    pub fn reachable_walkable(&self, starts: &[[i32; 3]]) -> BTreeSet<[i32; 3]> {
        self.flood_walkable(starts.iter().filter_map(|&s| self.snap(s, SNAP_RADIUS)))
    }

    /// [`World::reachable_walkable`] with each root's **seating** confined to the
    /// AABB that declares it ([`AnchorRoot`]). Only the snap is confined; the walk
    /// itself is unbounded, because a player who reaches a room does reach what it
    /// connects to.
    pub fn reachable_walkable_rooted(&self, roots: &[AnchorRoot]) -> BTreeSet<[i32; 3]> {
        self.flood_walkable(roots.iter().filter_map(|r| self.seat(r)))
    }

    /// Where a root actually puts a walker: the nearest standable cell to
    /// `root.at` **inside `root.within`**. `None` when the declaring piece offers
    /// no footing within [`SNAP_RADIUS`] — a root that seats nowhere contributes
    /// nothing, exactly as an unsnappable start does.
    fn seat(&self, root: &AnchorRoot) -> Option<[i32; 3]> {
        let (lo, hi) = root.within;
        self.snap_in_bounds(root.at, SNAP_RADIUS, &|c| {
            (0..3).all(|i| lo[i] <= c[i] && c[i] <= hi[i])
        })
    }

    /// The walk closure of already-seated cells: deterministic BFS over a
    /// `BTreeSet` frontier with the fixed neighbour order (ADR-0006). The one flood
    /// both [`World::reachable_walkable`] and [`World::reachable_walkable_rooted`]
    /// run, so confining a root changes *where the walk starts* and nothing else.
    fn flood_walkable(&self, seats: impl IntoIterator<Item = [i32; 3]>) -> BTreeSet<[i32; 3]> {
        let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
        let mut queue: std::collections::VecDeque<[i32; 3]> = std::collections::VecDeque::new();
        for cell in seats {
            if seen.insert(cell) {
                queue.push_back(cell);
            }
        }
        while let Some(cur) = queue.pop_front() {
            for n in self.neighbors(cur) {
                if seen.insert(n) {
                    queue.push_back(n);
                }
            }
        }
        seen
    }

    /// The union of every cell on a critical-path walked leg (the A* paths
    /// [`check_critical_path`] validates) plus every `move-npc` waypoint cell — the
    /// "required nav path cells" the relight pass must never occupy or obstruct
    /// (spec-0010), and the cell set the lethal-trap proof calls "forced".
    ///
    /// Routed over the **causally-sealed** per-leg world, exactly like
    /// [`check_critical_path`]. Before that fix this walked the base
    /// open world while completability was proven under seals, so the two
    /// disagreed about which cells the player is actually forced across: a leg the
    /// player can only walk as a detour *because* a `close-gate` shut the direct
    /// route was routed through the (still open) gate here, and the trap proof
    /// then declared a lethal plate on the detour "avoidable" — a provable death
    /// the build shipped green.
    pub fn required_path_cells(&self, plan: &Plan, moves: &[MovePlan]) -> BTreeSet<[i32; 3]> {
        let mut cells: BTreeSet<[i32; 3]> = BTreeSet::new();
        // Critical-path legs.
        for leg in self.walked_legs(plan) {
            cells.extend(leg.cells);
        }
        // move-npc waypoint cells (floored).
        for m in moves {
            for w in &m.waypoints {
                cells.insert([
                    w[0].floor() as i32,
                    w[1].floor() as i32,
                    w[2].floor() as i32,
                ]);
            }
        }
        cells
    }

    /// The proven A* cell route for every **walked** critical-path leg (transport
    /// hops skipped), each routed over that leg's causally-sealed world — the one
    /// leg model shared by [`check_critical_path`] (the completability proof),
    /// [`World::required_path_cells`] (relight + the lethal-trap forced-cell set)
    /// and [`critical_path_routes`] (the exported harness waypoints).
    ///
    /// They are unified on purpose. Were the proof to run under `close-gate`
    /// seals while the trap analysis and the waypoint export ran over the open
    /// world, the compiler could export a bot route through a gate the campaign
    /// had already sealed, and could call a trap on a forced detour
    /// "avoidable". A leg that
    /// fails to snap or route is omitted — it cannot occur once
    /// [`check_critical_path`] has passed, and before that the DW0311 error is the
    /// diagnostic that matters.
    fn walked_legs(&self, plan: &Plan) -> Vec<LegRoute> {
        self.walked_legs_sealed(plan)
            .into_iter()
            .map(|(leg, _)| leg)
            .collect()
    }

    /// [`World::walked_legs`], each leg paired with the gate cells sealed while the
    /// player walks it ([`leg_seal`]). The trap proof needs the seal itself, not
    /// just the route: a disarm affordance is only genuinely reachable "before the
    /// trap" if it is reachable under the gate state in force at that point.
    fn walked_legs_sealed(&self, plan: &Plan) -> Vec<(LegRoute, BTreeSet<[i32; 3]>)> {
        route_walked_legs(
            self,
            &critical_positions(plan),
            &plan.region_events,
            &|g, s| plan.gate_fired_before(g, s),
        )
    }

    /// The standable cells confined to the AABB `bounds`, reachable by a walk from
    /// `anchor` (snapped to the nearest standable cell inside `bounds`), returned in
    /// ascending BFS step-distance order from that start with a fixed `(y, z, x)`
    /// tie-break. Seats spawn-wave mobs on validated footing near their anchor
    /// `bounds` is the anchor's own assembled piece, so the flood-fill
    /// never leaves that room even where a mated socket is open air — a wave
    /// cannot string its mobs across a socket seam into the neighbouring piece,
    /// which is how a flock ends up spread toward void. Empty
    /// when no standable cell exists inside `bounds` within reach of the anchor.
    ///
    /// Deterministic (ADR-0006): BFS over a `VecDeque` with the fixed neighbour
    /// order, then a total sort on `(distance, y, z, x)`.
    pub fn confined_standable_cells(
        &self,
        anchor: [i32; 3],
        bounds: ([i32; 3], [i32; 3]),
    ) -> Vec<[i32; 3]> {
        let (lo, hi) = bounds;
        let in_bounds = |c: [i32; 3]| (0..3).all(|i| lo[i] <= c[i] && c[i] <= hi[i]);
        // A wave anchor often marks a solid affordance (a totem, a marker block) the
        // mobs stand *around*, not inside: snap the start to the nearest standable
        // floor cell within the room before flooding.
        let Some(start) = self.snap_in_bounds(anchor, SNAP_RADIUS, &in_bounds) else {
            return Vec::new();
        };
        let mut dist: BTreeMap<[i32; 3], u32> = BTreeMap::new();
        let mut queue: std::collections::VecDeque<[i32; 3]> = std::collections::VecDeque::new();
        dist.insert(start, 0);
        queue.push_back(start);
        while let Some(cur) = queue.pop_front() {
            let d = dist[&cur] + 1;
            for n in self.neighbors(cur) {
                if in_bounds(n) && !dist.contains_key(&n) {
                    dist.insert(n, d);
                    queue.push_back(n);
                }
            }
        }
        let mut cells: Vec<[i32; 3]> = dist.keys().copied().collect();
        cells.sort_by_key(|c| (dist[c], c[1], c[2], c[0]));
        cells
    }

    /// The **aggro ring**: standable cells inside `bounds`, walk-reachable from
    /// `anchor`, at a straight-line distance in `[radius - tolerance, radius]`
    /// from the anchor's snapped cell — and able to see it.
    ///
    /// This is the placement model for `summon: aggro-edge` (spec-0016 §6): a
    /// non-raider wave materializes at the boundary of its own perception, so it
    /// acquires a target the instant it exists and closes under pure native AI.
    ///
    /// The band is deliberately one-sided — **at or just inside `radius`, never
    /// beyond it**. A cell one block outside the mob's own `follow_range` looks
    /// identical on a map and is a different mechanic entirely: the mob spawns,
    /// perceives nobody, and stands there until a player walks closer. The
    /// spec's "±tolerance" reading admits that cell; this does not, and stricter
    /// is the only safe direction for a rule whose failure is silent.
    ///
    /// Reachability is inherited from [`World::confined_standable_cells`] — a mob
    /// summoned into a sealed pocket at the right distance would never arrive —
    /// and line-of-sight is what makes the ring an *aggro* ring rather than a
    /// circle of coordinates: vanilla's nearest-attackable-target goal is
    /// sight-gated, so a cell that cannot see the defended point summons a mob
    /// that stands there.
    ///
    /// Deterministic (ADR-0006): integer squared distances throughout, ordered
    /// **outermost first** — the edge of perception is where the fiction puts
    /// them — with a fixed `(-d², y, z, x)` tie-break. `Vec` order is the summon
    /// order.
    pub fn annulus_standable_cells(
        &self,
        anchor: [i32; 3],
        bounds: ([i32; 3], [i32; 3]),
        radius: f64,
        tolerance: f64,
    ) -> Vec<[i32; 3]> {
        let Some(centre) = self.ring_centre(anchor, bounds) else {
            return Vec::new();
        };
        let lo_d = (radius - tolerance).max(0.0);
        let (lo2, hi2) = (lo_d * lo_d, radius * radius);
        let mut ring: Vec<(i64, [i32; 3])> = self
            .confined_standable_cells(anchor, bounds)
            .into_iter()
            .filter_map(|c| {
                let d2: i64 = (0..3)
                    .map(|i| i64::from(c[i] - centre[i]).pow(2))
                    .sum::<i64>();
                let d2f = d2 as f64;
                (d2f >= lo2 && d2f <= hi2 && self.has_line_of_sight(c, centre)).then_some((d2, c))
            })
            .collect();
        ring.sort_by_key(|(d2, c)| (-*d2, c[1], c[2], c[0]));
        ring.into_iter().map(|(_, c)| c).collect()
    }

    /// The cell an aggro ring is measured from: the defended anchor snapped to
    /// standable footing inside `bounds`. A defended point is usually an
    /// affordance the party stands *around* (a fire, a totem, a heart), so the
    /// raw anchor cell is routinely solid. Public because the generated PackTest
    /// asserts ring distance against exactly this centre — the runtime assertion
    /// and the compile-time placement must measure from the same origin or the
    /// test proves nothing about the mechanic.
    pub fn ring_centre(&self, anchor: [i32; 3], bounds: ([i32; 3], [i32; 3])) -> Option<[i32; 3]> {
        let (lo, hi) = bounds;
        let in_bounds = |c: [i32; 3]| (0..3).all(|i| lo[i] <= c[i] && c[i] <= hi[i]);
        self.snap_in_bounds(anchor, SNAP_RADIUS, &in_bounds)
    }

    /// Whether a standing entity at cell `a` can see one at cell `b`: the segment
    /// between their eye points (1.5 blocks above each cell's floor, the vanilla
    /// mob eye height) crosses no camera-blocking geometry. Reuses the cutscene
    /// clip traversal, so "can this be seen through" has exactly one definition in
    /// the compiler.
    fn has_line_of_sight(&self, a: [i32; 3], b: [i32; 3]) -> bool {
        let eye = |c: [i32; 3]| {
            let p = cell_center(c);
            [p[0], p[1] + 1.5, p[2]]
        };
        walk_cells(eye(a), eye(b), |c| self.blocks_camera(c)).is_none()
    }

    /// Nearest standable cell to `c` within `radius` that also satisfies `accept`,
    /// broken deterministically by `(distance², cell)`; `None` if none. The
    /// `accept` predicate confines the search (e.g. to one piece's AABB).
    fn snap_in_bounds(
        &self,
        c: [i32; 3],
        radius: i32,
        accept: &impl Fn([i32; 3]) -> bool,
    ) -> Option<[i32; 3]> {
        let mut best: Option<(i32, [i32; 3])> = None;
        for dy in -radius..=radius {
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    let n = [c[0] + dx, c[1] + dy, c[2] + dz];
                    if !accept(n) || !self.standable(n) {
                        continue;
                    }
                    let d2 = dx * dx + dy * dy + dz * dz;
                    match best {
                        Some((bd, bc)) if (bd, bc) <= (d2, n) => {}
                        _ => best = Some((d2, n)),
                    }
                }
            }
        }
        best.map(|(_, n)| n)
    }

    /// Snap a walked-leg endpoint (`from`/`to`) to the cell the player stands on.
    ///
    /// Normally the nearest standable cell to the visited anchor. For a **talk-to**
    /// target (`off_cell`), the anchor is the NPC's own occupied cell (the mannequin
    /// stands there and its interaction hitbox fills it): the player stands within
    /// interaction range *beside* the NPC, so exclude the anchor cell itself and
    /// take the nearest OTHER standable cell. Flooded cells are already
    /// excluded (they are not standable), so a shore NPC never resolves onto a
    /// water-tongue cell.
    fn snap_endpoint(&self, c: [i32; 3], off_cell: bool) -> Option<[i32; 3]> {
        if off_cell {
            self.snap_in_bounds(c, SNAP_RADIUS, &|n| n != c)
        } else {
            self.snap_standable(c, SNAP_RADIUS)
        }
    }

    fn is_solid(&self, c: [i32; 3]) -> bool {
        self.solid.contains(&c)
    }

    /// Whether a cell contains block geometry a cutscene camera must not fly
    /// through: a full-cube solid, a 1.5-tall fence/wall, or a fence gate.
    /// Water does not clip a camera.
    ///
    /// **Public**: a declared sightline (`DW0821`) asks exactly this question of
    /// exactly these cells — whether a line of sight is stopped by geometry —
    /// and a second predicate for it would be a second opinion about what a
    /// fence does to a view.
    pub fn blocks_camera(&self, c: [i32; 3]) -> bool {
        self.solid.contains(&c) || self.tall.contains(&c) || self.use_gates.contains(&c)
    }

    /// Whether a cell is occupied — a solid block, a 1.5-tall barrier (fence /
    /// wall), **or** flooded by water. An occupied cell
    /// cannot hold a walker's feet or head, and cannot be jumped through. Water
    /// blocks passage but, unlike a solid, is never a floor; a tall barrier
    /// likewise blocks passage but is never a floor (not standable on top). A
    /// use-gate cell is deliberately NOT occupied here: the player passes it with
    /// a right-click (walkers that cannot are routed on
    /// [`World::without_gate_use`]).
    fn is_occupied(&self, c: [i32; 3]) -> bool {
        self.solid.contains(&c)
            || self.tall.contains(&c)
            || self.flooded.contains(&c)
            || self.lethal.contains(&c)
    }

    /// Whether a cell is a valid standing position: the feet-cell and the
    /// head-cell above it are both passable (neither solid nor flooded), with
    /// **solid** ground directly below (an entity is 2 blocks tall and needs a
    /// floor — a water surface is not standable, so the floor must be solid, not
    /// merely occupied).
    fn standable(&self, c: [i32; 3]) -> bool {
        self.standable_fp(c, &Footprint::player())
    }

    /// Footprint-aware standability (spec-0014): every occupied column has its
    /// `height` feet+body cells passable with solid floor directly below. For the
    /// player footprint (single column, 2 tall) this is exactly the pre-0.6 rule.
    fn standable_fp(&self, c: [i32; 3], fp: &Footprint) -> bool {
        fp.cols.iter().all(|&[dx, dz]| {
            let base = [c[0] + dx, c[1], c[2] + dz];
            self.is_solid([base[0], base[1] - 1, base[2]])
                && (0..fp.height).all(|dy| !self.is_occupied([base[0], base[1] + dy, base[2]]))
        })
    }

    /// The nearest standable cell to `c` (itself if already standable), searched
    /// outward in a bounded box and broken deterministically by
    /// `(distance², cell)`. `None` if nothing standable is within `radius`.
    ///
    /// A `move-npc` target anchor is often a solid affordance — an altar, a gate
    /// bar row, a wall marker — that the NPC should walk *up to*, not *into*
    /// (owner's "lands inside a wall" finding). Snapping resolves the walk to the
    /// floor cell in front of such an anchor.
    fn snap_standable(&self, c: [i32; 3], radius: i32) -> Option<[i32; 3]> {
        self.snap_standable_fp(c, radius, &Footprint::player())
    }

    /// Footprint-aware nearest-standable snap (spec-0014), used by `move-actor`
    /// endpoint resolution so a wide/tall puppet snaps to a cell IT can stand on.
    fn snap_standable_fp(&self, c: [i32; 3], radius: i32, fp: &Footprint) -> Option<[i32; 3]> {
        if self.standable_fp(c, fp) {
            return Some(c);
        }
        let mut best: Option<(i32, [i32; 3])> = None;
        for dy in -radius..=radius {
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    let n = [c[0] + dx, c[1] + dy, c[2] + dz];
                    if !self.standable_fp(n, fp) {
                        continue;
                    }
                    let d2 = dx * dx + dy * dy + dz * dz;
                    match best {
                        Some((bd, bc)) if (bd, bc) <= (d2, n) => {}
                        _ => best = Some((d2, n)),
                    }
                }
            }
        }
        best.map(|(_, n)| n)
    }

    /// The walkable top face of the block directly below cell `c`, in sixteenths
    /// of a block above that block's own cell floor (16 = a full cube).
    fn floor_top_16(&self, support: [i32; 3]) -> i64 {
        self.partial
            .get(&support)
            .copied()
            .unwrap_or(crate::assembled::FULL_HEIGHT_16) as i64
    }

    /// The **true feet height** of a walker standing in cell `c`, in sixteenths of
    /// a block (absolute, so two standing cells can be differenced directly).
    ///
    /// The standing-cell convention is unchanged — the feet cell is the cell above
    /// the support — but the height it denotes is no longer assumed to be the cell
    /// floor: standing on a bottom slab puts the feet at `y - 0.5`, not `y`
    /// — a bottom slab puts them half a block down. For a multi-column footprint
    /// the walker rests on the **highest**
    /// supporting face, as vanilla's AABB does.
    fn feet_16_fp(&self, c: [i32; 3], fp: &Footprint) -> i64 {
        let base = (c[1] as i64 - 1) * FULL_16;
        fp.cols
            .iter()
            .map(|&[dx, dz]| base + self.floor_top_16([c[0] + dx, c[1] - 1, c[2] + dz]))
            .max()
            .unwrap_or(base + FULL_16)
    }

    /// Standable cardinal neighbours of `c`, allowing a one-cell step up or down.
    /// Fixed order for determinism.
    ///
    /// A step **up** past the auto-step budget is a jump: the entity's head sweeps
    /// through the cell `height` above its feet at the source, so that cell must be
    /// clear or it head-bonks and the move is physically impossible (a mineflayer
    /// bot refuses it with "No path to the goal!"). Modelling that jump-clearance
    /// here — not just the destination's standability — keeps a routed/exported
    /// path actually walkable: an assembled seam that ramps up under a low ceiling
    /// becomes a `DW0311` build error instead of a runtime strand on geometry the
    /// compiler wrongly "proved" connected.
    ///
    /// **Public**, and the widening is the point rather than a convenience: the
    /// step rule is the engine's ONE answer to "can a body get from here to
    /// there", and the stage-5 blockout battery (`crate::blockout`) asks that
    /// question of a whole map — is any two places' geometry joined anywhere the
    /// site plan did not allocate a seam (`DW0838`). A private rule leaves that
    /// battery with nothing to reuse and a hand-rolled step rule to write, which
    /// is `CLAUDE.md`'s second review shape: a general mechanism privately
    /// re-implemented, working perfectly, and silently not the rule every other
    /// proof in this compiler is taken under.
    pub fn neighbors(&self, c: [i32; 3]) -> Vec<[i32; 3]> {
        self.neighbors_fp(c, &Footprint::player())
    }

    /// Footprint-aware standable neighbours (spec-0014), gated by the **physical
    /// rise** between the two standing surfaces rather than by cell adjacency:
    ///
    /// - rise ≤ [`MAX_AUTO_STEP_16`] — a walk-up. No jump, so no headroom is
    ///   required above the source cell. This is what admits the step onto a bottom
    ///   slab under a low ceiling that the old full-cube rule wrongly refused.
    /// - rise ≤ [`MAX_JUMP_RISE_16`] — a jump; the swept head cell must be clear.
    /// - anything higher is **impossible** and is refused. The load-bearing case:
    ///   standing on a bottom slab and "stepping" onto a full block one cell up is
    ///   a 1.5-block rise the old model proved as an ordinary `+1` step.
    ///
    /// Vertical candidates stay `{0, -1, +1}` cells. A `+2`-cell move can be
    /// physically legal between two very thin floors, but leaving it out only ever
    /// *refuses* a route, never proves one — the safe direction.
    fn neighbors_fp(&self, c: [i32; 3], fp: &Footprint) -> Vec<[i32; 3]> {
        const HORIZ: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
        let head_clear_to_jump = fp
            .cols
            .iter()
            .all(|&[dx, dz]| !self.is_occupied([c[0] + dx, c[1] + fp.height, c[2] + dz]));
        let here = self.feet_16_fp(c, fp);
        let mut out = Vec::new();
        for (dx, dz) in HORIZ {
            for dy in [0i32, -1, 1] {
                let n = [c[0] + dx, c[1] + dy, c[2] + dz];
                if !self.standable_fp(n, fp) {
                    continue;
                }
                let rise = self.feet_16_fp(n, fp) - here;
                if rise > MAX_JUMP_RISE_16 {
                    continue; // above the jump apex: no player can make this step
                }
                if rise > MAX_AUTO_STEP_16 && !head_clear_to_jump {
                    continue; // needs a jump, and there is no room to jump here
                }
                out.push(n);
            }
        }
        out
    }

    /// A* over standable cells from `start` to `goal`, returning the cell path
    /// (inclusive of both ends) or `None` if unreachable. Deterministic: the
    /// frontier is ordered by `(f, g, cell)` and neighbours expand in a fixed
    /// order.
    ///
    /// **Public** for the same reason [`World::neighbors`] is: the pacing
    /// measurement (`DW0822`'s second call site) is the length of the route a
    /// body really walks along the layout graph's own critical path, and a
    /// second router would measure a second world.
    pub fn find_path(&self, start: [i32; 3], goal: [i32; 3]) -> Option<Vec<[i32; 3]>> {
        self.find_path_fp(start, goal, &Footprint::player())
    }

    /// Footprint-aware A* (spec-0014) over the **terrain-shaped** step cost
    /// ([`step_cost_16`]) — a wider/taller footprint additionally prunes cells the
    /// puppet cannot occupy. Deterministic: frontier ordered by `(f, g, cell)`,
    /// fixed neighbour order, integer costs only.
    ///
    /// The heuristic is horizontal Manhattan distance scaled by [`STEP_COST_16`],
    /// the cost of a perfectly flat step. Since no step is ever cheaper than that,
    /// `h` stays **admissible and consistent**, so A* still returns a true
    /// minimum-cost path and never needs to reopen a closed node — the cost change
    /// is a change of *preference among valid routes*, never of which routes exist.
    fn find_path_fp(
        &self,
        start: [i32; 3],
        goal: [i32; 3],
        fp: &Footprint,
    ) -> Option<Vec<[i32; 3]>> {
        if start == goal {
            return self.standable_fp(start, fp).then(|| vec![start]);
        }
        if !self.standable_fp(start, fp) || !self.standable_fp(goal, fp) {
            return None;
        }
        let h =
            |c: [i32; 3]| ((c[0] - goal[0]).abs() + (c[2] - goal[2]).abs()) as u32 * STEP_COST_16;
        let mut g_score: BTreeMap<[i32; 3], u32> = BTreeMap::new();
        let mut came_from: BTreeMap<[i32; 3], [i32; 3]> = BTreeMap::new();
        let mut open: BinaryHeap<Reverse<(u32, u32, [i32; 3])>> = BinaryHeap::new();
        g_score.insert(start, 0);
        open.push(Reverse((h(start), 0, start)));
        while let Some(Reverse((_f, g, cur))) = open.pop() {
            if cur == goal {
                let mut path = vec![cur];
                let mut node = cur;
                while let Some(&prev) = came_from.get(&node) {
                    path.push(prev);
                    node = prev;
                }
                path.reverse();
                return Some(path);
            }
            // Skip stale heap entries (a cheaper route was already recorded).
            if g > *g_score.get(&cur).unwrap_or(&u32::MAX) {
                continue;
            }
            let here = self.feet_16_fp(cur, fp);
            for n in self.neighbors_fp(cur, fp) {
                let tentative = g + step_cost_16(here, self.feet_16_fp(n, fp));
                if tentative < *g_score.get(&n).unwrap_or(&u32::MAX) {
                    came_from.insert(n, cur);
                    g_score.insert(n, tentative);
                    open.push(Reverse((tentative + h(n), tentative, n)));
                }
            }
        }
        None
    }
}

/// The cost of one perfectly **flat** cardinal step, in sixteenths of a block of
/// level walking. Every A* cost is denominated in this unit so the elevation
/// penalty below can be expressed in the same currency as horizontal distance
/// (and so the whole cost function stays integer — ADR-0006 forbids float
/// comparisons deciding a path).
const STEP_COST_16: u32 = FULL_16 as u32;

/// What one block of **elevation change** costs, expressed as a multiple of the
/// same distance walked on the flat (round-8 owner playtest).
///
/// The defect this fixes: with a distance-only cost, every route of equal length
/// is equally good, so the planner walked the herd and the giant along the
/// straight line over the greenfield's bumpy 1-step terrain — bobbing up and down
/// a block a dozen times — while the flat cleared road two columns to the side
/// cost the same 2-step detour it always did and never won. Staged walks are
/// *photographed*: a body that pogos over lumps reads as broken even though every
/// step is legal, and the built road exists precisely to be walked.
///
/// **Why 2.** A rise past [`MAX_AUTO_STEP_16`] is a jump, and vanilla's jump arc
/// is ≈12 ticks airborne against ≈4.6 ticks to walk one block on the flat — so
/// clearing a 1-block rise really does cost about 2.5 blocks of walking time.
/// Two is the integer under that: enough that the planner pays a genuine detour
/// to stay level (a 1-block bump must be worth ~2 blocks of going around), but
/// not so much that it invents long absurd circuits to dodge a single step. It is
/// deliberately *under* the physical figure — the safe direction, since
/// overpaying for flatness is what would distort routes on legitimately sloped
/// terrain (the mountain ramp, the beach grade).
///
/// Measured on the island (round 8): the beach→pen walk crossed the greenfield at
/// `x=7` with 24 blocks of cumulative elevation change; at weight 2 it moves onto
/// the built path spine (`x=9..11`, flat at `y=63`) and the bobbing is gone. The
/// weight is applied per sixteenth, so a slab or a `dirt_path` lip (a 1/16-15/16
/// partial floor) costs proportionally less than a full block — the planner is
/// not driven off intentional slab stairs by the same rule that keeps it off
/// lumpy ground.
const ELEV_WEIGHT: u32 = 2;

/// The cost of stepping between two standing surfaces whose **true feet heights**
/// (sixteenths, absolute — [`World::feet_16_fp`]) are `from` and `to`: one flat
/// step plus [`ELEV_WEIGHT`] per sixteenth of height change, up or down.
///
/// Up and down are charged alike: the owner's complaint is *bobbing*, and a path
/// that drops a block only to climb it back is exactly as ugly as the reverse.
/// Never zero, so the heuristic stays admissible and A* terminates.
fn step_cost_16(from: i64, to: i64) -> u32 {
    STEP_COST_16 + (to - from).unsigned_abs() as u32 * ELEV_WEIGHT
}

/// The world position an entity standing in cell `c` occupies: the **horizontal
/// centre** of the cell, on its floor.
///
/// A Minecraft block cell `(x, y, z)` spans `[x, x+1)` on each horizontal axis, and
/// an entity's position is the centre of its AABB. Emitting the bare integer cell
/// coordinate therefore parks the body on the *corner* where four columns meet: a
/// 0.6-wide villager at `x = 7.0` spans `[6.7, 7.3]`, i.e. 70 % of it sits inside
/// column 6 — inside the wall, whenever the proven-walkable cell is 7. That is the
/// owner's "the NPC visibly passes through blocks" defect (island QA: 234 of 385
/// waypoints on the beach→cave walk had the body AABB inside a solid). The `+0.5`
/// is the whole fix: on a cardinal path through cell centres the AABB stays inside
/// the proven-walkable columns.
///
/// This is the single conversion for **every entity the compiler places or moves**;
/// block-targeting commands (`setblock`/`fill`/`place`/`spawnpoint`) keep integer
/// cell coordinates, which is what they take.
pub fn cell_center(c: [i32; 3]) -> [f64; 3] {
    [c[0] as f64 + 0.5, c[1] as f64, c[2] as f64 + 0.5]
}

/// Resample a cell path into per-tick waypoints at `speed` blocks/tick along the
/// polyline through the cell centres ([`cell_center`]). Guarantees the final
/// waypoint is exactly the goal cell's centre and at least one step exists.
///
/// **Vertical steps are L-shaped, not diagonal.** A one-block step up inserts an
/// intermediate vertex directly above the source cell (rise in place, then cross at
/// the new height); a step down crosses at the source height, then drops. A straight
/// lerp between the two cell centres would sweep the body through the *corner* of
/// the step block — the same "inside the geometry" artifact at a stair that the
/// centring fixes along a wall. Both legs of the L stay inside cells the neighbour
/// rule already proved clear (`standable_fp` + the jump head-clearance check).
fn resample(cells: &[[i32; 3]], speed: f64) -> Vec<[f64; 3]> {
    let mut pts: Vec<[f64; 3]> = Vec::with_capacity(cells.len() * 2);
    for (i, c) in cells.iter().enumerate() {
        let p = cell_center(*c);
        if i > 0 {
            let prev = cells[i - 1];
            match c[1] - prev[1] {
                // step up: rise over the source column first, then cross.
                1 => pts.push([prev[0] as f64 + 0.5, c[1] as f64, prev[2] as f64 + 0.5]),
                // step down: cross at the source height first, then drop.
                -1 => pts.push([p[0], prev[1] as f64, p[2]]),
                _ => {}
            }
        }
        pts.push(p);
    }
    if pts.len() == 1 {
        return vec![pts[0]];
    }
    // Cumulative arc length at each vertex.
    let mut cum = vec![0.0f64];
    for w in pts.windows(2) {
        let d = ((w[1][0] - w[0][0]).powi(2)
            + (w[1][1] - w[0][1]).powi(2)
            + (w[1][2] - w[0][2]).powi(2))
        .sqrt();
        cum.push(cum.last().unwrap() + d);
    }
    let total = *cum.last().unwrap();
    let speed = if speed > 0.0 { speed } else { DEFAULT_SPEED };
    let ticks = ((total / speed).ceil() as i64).max(1) as usize;
    let mut out = Vec::with_capacity(ticks + 1);
    for t in 0..=ticks {
        let d = total * (t as f64) / (ticks as f64);
        let p = point_at(&pts, &cum, d);
        // Round to 0.01 block — far finer than needed at 0.15 blk/tick, and keeps
        // the emitted per-tick `tp` coordinates short and stable.
        out.push([round2(p[0]), round2(p[1]), round2(p[2])]);
    }
    *out.last_mut().unwrap() = *pts.last().unwrap();
    out
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// The point at arc length `d` along the polyline `pts` with cumulative lengths
/// `cum`.
fn point_at(pts: &[[f64; 3]], cum: &[f64], d: f64) -> [f64; 3] {
    let total = *cum.last().unwrap();
    if d <= 0.0 || total == 0.0 {
        return pts[0];
    }
    if d >= total {
        return *pts.last().unwrap();
    }
    // Find the segment containing `d`.
    let mut i = 0;
    while i + 1 < cum.len() && cum[i + 1] < d {
        i += 1;
    }
    let seg = cum[i + 1] - cum[i];
    let f = if seg > 0.0 { (d - cum[i]) / seg } else { 0.0 };
    let a = pts[i];
    let b = pts[i + 1];
    [
        a[0] + (b[0] - a[0]) * f,
        a[1] + (b[1] - a[1]) * f,
        a[2] + (b[2] - a[2]) * f,
    ]
}

/// The absolute world position of an NPC's home anchor (its spawn cell), which is
/// where a `move-npc` walk begins.
fn npc_start(plan: &Plan, npc_id: &str) -> Option<[i32; 3]> {
    let npc = plan
        .campaign
        .npcs
        .content
        .npcs
        .iter()
        .find(|n| n.id.as_str() == npc_id)?;
    let area = plan.npc_area(npc_id)?;
    plan.point(area, npc.anchor.as_str())
}

/// Resolve a `move-npc` destination: the anchor in the NPC's own area, else any
/// area (first match). Mirrors the emitter's `movenpc_target`.
fn move_target(plan: &Plan, npc_id: &str, to_anchor: &str) -> Option<[i32; 3]> {
    if let Some(area) = plan.npc_area(npc_id)
        && let Some(pos) = plan.point(area, to_anchor)
    {
        return Some(pos);
    }
    for ((_, name), resolved) in &plan.anchors {
        if name == to_anchor {
            return match resolved {
                ResolvedAnchor::Point { pos, .. } => Some(*pos),
                ResolvedAnchor::Gate { from, .. } => Some(*from),
            };
        }
    }
    None
}

/// Plan every `move-npc` in the campaign into a walked-path [`MovePlan`], deduped
/// by `(npc, to_anchor)` in first-seen order. `DW0307` when a move is unroutable.
/// Each NPC's successive moves **chain**: the first leg starts at the stage-2
/// anchor, every later leg at the previous leg's target (round-6; see
/// [`plan_actor_moves`]). Two moves sharing `(npc, to_anchor)` still share one
/// content-keyed driver, planned from the first occurrence's origin (documented
/// limitation of the content key).
///
/// **Use-gate cells are walkable edges here**: routing through the
/// openable threshold is strictly more faithful than the old full-solid model,
/// which "proved" the same legs by hopping the body over a fence-top. Only
/// autonomous placement (wave seating) uses the no-gate-use view — a spawned mob
/// really cannot pass a closed gate on its own.
///
/// This edge used to be justified by "the beat's fiction controls the gate" (the
/// island ram leaves its pen only after the player has opened the pen gate to
/// reach it). **Nothing proved that fiction**, and island round 21 is what it
/// cost: the mountain pen's gate shipped `open=false` and sixteen legs walked
/// through it, in a cell the owner herself had to squeeze around. The edge is
/// still available here — a route that names the offending cell is a better
/// diagnostic than an unroutable [`DW_MOVE_UNROUTABLE`] — but
/// [`crate::traversal`] now fails the build on it (`DW0452`) unless the gate is
/// genuinely open in the world the delve ships.
pub fn plan_moves(plan: &Plan, world: &World) -> Result<Vec<MovePlan>, NavError> {
    let mut out = Vec::new();
    let mut seen: BTreeSet<(String, String, String)> = BTreeSet::new();
    // Chained origins (round-6): each NPC's next walk starts from its LAST staged
    // location — the previous move's (snapped) target — not its declared anchor.
    // Planning every leg from the declared anchor made a second consecutive
    // `move-npc` on the same NPC degenerate (worst case start == target → a
    // single-waypoint instant teleport instead of a walk).
    //
    // The chain is **branch-aware** (island round 16): a leg only inherits an
    // origin a leg on its own branch actually produced, so a `flag/flee`-gated
    // walk can no longer hand its destination to the `flag/wait`-gated walk that
    // follows it in declaration order. See [`BranchGate`] for the defect this
    // fixes and why the rule is stated as implication rather than exclusion.
    let mut history: StagingHistory = StagingHistory::new();
    // The cell route planned for each `(npc, to_anchor)` driver, so a deduped
    // repeat occurrence can be re-checked against its own timeline's seals.
    let mut planned: BTreeMap<(String, String, String), Vec<[i32; 3]>> = BTreeMap::new();
    // The yaw each planned driver ends on, so a deduped repeat chains the same
    // facing forward as the first occurrence did.
    let mut planned_end_yaw: BTreeMap<(String, String, String), i32> = BTreeMap::new();
    // The origin each driver was planned from, and the branch of the occurrence
    // that planned it — so a deduped occurrence standing somewhere else is
    // `DW0488` instead of a silent teleport.
    let mut planned_origin: BTreeMap<(String, String, String), ([i32; 3], BranchGate)> =
        BTreeMap::new();
    let mut cache = SealCache::default();
    for (eff, seal) in crate::timeline::walk(plan) {
        let QuestEffect::MoveNpc {
            npc,
            to_anchor,
            speed,
            ..
        } = eff
        else {
            continue;
        };
        let gate = BranchGate::of(eff);
        // The world this walk actually happens in: gates this timeline already
        // shut are solid. Empty seal ⇒ the base world, unchanged.
        let leg_world: &World = match cache.index_of(world, &seal) {
            Some(i) => &cache.worlds[i],
            None => world,
        };
        let anchor_pos =
            move_target(plan, npc.as_str(), to_anchor.as_str()).ok_or_else(|| NavError {
                code: DW_MOVE_UNROUTABLE,
                message: format!(
                    "move-npc: destination anchor `{}` for NPC `{}` did not resolve to a world \
                     position — use a `to_anchor` that the NPC's area prefab provides",
                    to_anchor.as_str(),
                    npc.as_str()
                ),
            })?;
        let target = leg_world
            .snap_standable(anchor_pos, SNAP_RADIUS)
            .ok_or_else(|| NavError {
                code: DW_MOVE_UNROUTABLE,
                message: format!(
                    "move-npc: no standable floor cell near destination anchor `{}` {anchor_pos:?} \
                 for NPC `{}` — the anchor is walled in or over void; place `{}` beside walkable \
                 floor the npc can stand on",
                    to_anchor.as_str(),
                    npc.as_str(),
                    to_anchor.as_str(),
                ),
            })?;
        let gkey = gate.key();
        let key = (
            npc.as_str().to_string(),
            to_anchor.as_str().to_string(),
            gkey.clone(),
        );
        if !seen.insert(key.clone()) {
            // Deduped: shares the first occurrence's driver, so it walks the
            // already-planned path — which must still be clear under THIS
            // occurrence's timeline seals (DW0410; see `plan_actor_moves`).
            if !seal.is_empty()
                && let Some(cells) = planned.get(&key)
            {
                let sealed = seal_cells(&seal);
                if cells.iter().any(|c| sealed.contains(c)) {
                    return Err(gate_timeline_error(
                        "move-npc",
                        npc.as_str(),
                        to_anchor.as_str(),
                        cells[0],
                        target,
                        &seal,
                    ));
                }
            }
            // This occurrence walks the driver the FIRST occurrence planned, so
            // it starts at that driver's origin — which is only correct if this
            // occurrence's own branch leaves the body there too (`DW0488`).
            if let Some((planned_from, planned_gate)) = planned_origin.get(&key) {
                let here = chained_staging(&history, npc.as_str(), &gate).map(|s| s.pos);
                if let Some(here) = here
                    && here != *planned_from
                {
                    return Err(shared_origin_error(
                        "move-npc",
                        npc.as_str(),
                        to_anchor.as_str(),
                        *planned_from,
                        planned_gate,
                        here,
                        &gate,
                    ));
                }
            }
            // The walk still ends here, so the NPC's next leg chains from this
            // target — and from the facing the shared driver leaves the body in.
            record_staging(
                &mut history,
                npc.as_str(),
                gate,
                target,
                planned_end_yaw.get(&key).copied(),
            );
            continue;
        }
        let prior = chained_staging(&history, npc.as_str(), &gate);
        let start = match prior.map(|s| s.pos) {
            Some(pos) => pos,
            None => {
                let home = npc_start(plan, npc.as_str()).ok_or_else(|| NavError {
                    code: DW_MOVE_UNROUTABLE,
                    message: format!(
                        "move-npc: NPC `{}` has no resolved home anchor to walk from — give the \
                         npc a stage-2 `anchor` that its area's prefab provides, so the walk has \
                         a start",
                        npc.as_str()
                    ),
                })?;
                // The NPC walks up to a solid affordance, not into it: snap the
                // home endpoint to the floor cell nearest the anchor.
                leg_world.snap_standable(home, SNAP_RADIUS).unwrap_or(home)
            }
        };
        let seed_yaw = prior.and_then(|s| s.yaw);
        let cells = match leg_world.find_path(start, target) {
            Some(cells) => cells,
            // Routable open, unroutable sealed ⇒ this timeline's own `close-gate`
            // is the cause (DW0410); otherwise the geometry never connected (DW0307).
            None if !seal.is_empty() && world.find_path(start, target).is_some() => {
                return Err(gate_timeline_error(
                    "move-npc",
                    npc.as_str(),
                    to_anchor.as_str(),
                    start,
                    target,
                    &seal,
                ));
            }
            None => {
                return Err(NavError {
                    code: DW_MOVE_UNROUTABLE,
                    message: format!(
                        "move-npc: NPC `{}` cannot walk from its last staged location {start:?} \
                         (home anchor `{}`) to `{}` {anchor_pos:?} (floor {target:?}) — no \
                         collision-free path over the solved geometry. Route the move within one \
                         connected area (a wall/void/closed gate separates start and \
                         destination), or split it into shorter reachable hops",
                        npc.as_str(),
                        plan_npc_anchor(plan, npc.as_str()),
                        to_anchor.as_str(),
                    ),
                });
            }
        };
        planned.insert(key.clone(), cells.clone());
        planned_origin.insert(key.clone(), (start, gate.clone()));
        let waypoints = resample(&cells, speed.unwrap_or(DEFAULT_SPEED));
        // Seed: the facing this body already has — the exit yaw of the previous
        // leg **on this branch** if this NPC has walked before, else the yaw its
        // summon gave it (the home anchor's declared facing,
        // `emit::npc_summon_commands`).
        let seed = seed_yaw.unwrap_or_else(|| npc_spawn_yaw(plan, npc.as_str()));
        let yaws = yaws_along(&waypoints, seed);
        let end_yaw = yaws.last().copied().unwrap_or(seed);
        record_staging(&mut history, npc.as_str(), gate, target, Some(end_yaw));
        planned_end_yaw.insert(key, end_yaw);
        out.push(MovePlan {
            npc: npc.as_str().to_string(),
            to_anchor: to_anchor.as_str().to_string(),
            target,
            cells,
            waypoints,
            yaws,
            gate_key: gkey,
        });
    }
    Ok(out)
}

/// The yaw an NPC's summon gives it: its home anchor's declared `facing`, exactly
/// as `emit::npc_summon_commands` resolves it. The seed a walk starts from, so a
/// move that opens with no horizontal motion keeps the body's authored facing
/// instead of snapping it south.
fn npc_spawn_yaw(plan: &Plan, npc_id: &str) -> i32 {
    let facing = (|| {
        let npc = plan
            .campaign
            .npcs
            .content
            .npcs
            .iter()
            .find(|n| n.id.as_str() == npc_id)?;
        let area = plan.npc_area(npc_id)?;
        match plan
            .anchors
            .get(&(area.to_string(), npc.anchor.to_string()))
        {
            Some(ResolvedAnchor::Point { facing, .. }) => facing.clone(),
            _ => None,
        }
    })();
    crate::emit::facing_yaw(facing.as_deref())
}

/// A planned `move-actor` (spec-0014): resolved endpoints, the per-tick waypoint
/// polyline the emitter teleports the puppet along, and a yaw per waypoint tangent
/// to the path (a wrong yaw moonwalks). `ticks() + 1` entries.
#[derive(Debug, Clone)]
pub struct ActorMovePlan {
    /// The moving actor id (`actor/…`).
    pub actor: String,
    /// The destination anchor id (`anchor/…`).
    pub to_anchor: String,
    /// The integer target cell (feet), for the arrival assertion.
    pub target: [i32; 3],
    /// The A* **cell** path this leg walks, start to target inclusive — see
    /// [`MovePlan::cells`].
    pub cells: Vec<[i32; 3]>,
    /// Per-tick world positions along the walked path.
    pub waypoints: Vec<[f64; 3]>,
    /// Per-waypoint yaw (degrees), tangent to the path (facing the next step).
    pub yaws: Vec<i32>,
    /// The branch-gate component of this driver's content key ([`gate_key`]);
    /// empty for an unconditional walk.
    pub gate_key: String,
}

impl ActorMovePlan {
    /// The final tick index (`waypoints.len() - 1`).
    pub fn ticks(&self) -> usize {
        self.waypoints.len().saturating_sub(1)
    }
}

/// The stage-5 actor with this id, if declared.
fn actor_of<'a>(plan: &'a Plan, actor_id: &str) -> Option<&'a delvewright_dsl::Actor> {
    plan.campaign
        .quests
        .content
        .actors
        .iter()
        .find(|a| a.id.as_str() == actor_id)
}

/// Resolve an anchor name to a world point by scanning every area (first match) —
/// actors carry no area, so their anchors resolve globally like `open-gate`.
fn actor_anchor_pos(plan: &Plan, anchor: &str) -> Option<[i32; 3]> {
    for ((_, name), resolved) in &plan.anchors {
        if name == anchor {
            return Some(match resolved {
                ResolvedAnchor::Point { pos, .. } => *pos,
                ResolvedAnchor::Gate { from, .. } => *from,
            });
        }
    }
    None
}

/// The MC yaw (degrees, 0 = +z/south) for a horizontal movement delta, or `None`
/// for no horizontal motion. `yaw = atan2(-dx, dz)`.
fn yaw_of(dx: f64, dz: f64) -> Option<i32> {
    if dx.abs() < 1e-6 && dz.abs() < 1e-6 {
        return None;
    }
    let deg = (-dx).atan2(dz).to_degrees();
    let mut y = deg.round() as i32 % 360;
    if y < 0 {
        y += 360;
    }
    Some(y)
}

/// A yaw per waypoint, each the **exact bearing of the segment about to be
/// walked** (no smoothing: a corner turns on the tick it is taken); the last
/// reuses the previous. A body tp'd without a matching yaw moonwalks — shown by
/// packet evidence for puppets and visible in play for NPCs.
///
/// `seed` is the facing the body already has, used for any leading waypoints with
/// no horizontal motion of their own (a walk that opens with `resample`'s vertical
/// step-up leg, or a degenerate zero-length move). An established facing is never
/// overwritten with a fabricated south.
fn yaws_along(waypoints: &[[f64; 3]], seed: i32) -> Vec<i32> {
    let n = waypoints.len();
    let mut yaws = vec![0i32; n];
    // Forward pass: each waypoint faces its NEXT step; the final waypoint reuses the
    // last motion direction (so arrival keeps the walk facing, not a snap to south).
    let mut last = seed;
    for i in 0..n {
        if i + 1 < n {
            let a = waypoints[i];
            let b = waypoints[i + 1];
            if let Some(y) = yaw_of(b[0] - a[0], b[2] - a[2]) {
                last = y;
            }
        }
        yaws[i] = last;
    }
    yaws
}

/// The first cell along the straight start→target line the actor's footprint cannot
/// stand on — a best-effort "first blocked cell" for the `DW0325` message.
fn first_blocked_fp(world: &World, start: [i32; 3], target: [i32; 3], fp: &Footprint) -> [i32; 3] {
    let d = [
        target[0] - start[0],
        target[1] - start[1],
        target[2] - start[2],
    ];
    let steps = d[0].abs().max(d[1].abs()).max(d[2].abs()).max(1);
    for s in 0..=steps {
        let cell = [
            start[0] + d[0] * s / steps,
            start[1] + d[1] * s / steps,
            start[2] + d[2] * s / steps,
        ];
        if !world.standable_fp(cell, fp) {
            return cell;
        }
    }
    target
}

/// The world cells a timeline's sealed gate regions fill (see [`crate::timeline`]).
fn seal_cells(seal: &crate::timeline::GateState) -> BTreeSet<[i32; 3]> {
    let mut cells = BTreeSet::new();
    for &(lo, hi) in seal.keys() {
        cells.extend(crate::assembled::region_cells(lo, hi));
    }
    cells
}

/// Memoized timeline-sealed views of the world.
///
/// A staged walk is routed over the world with the gates its own timeline has
/// already shut forced solid ([`World::with_sealed`]). Building that view clones
/// the whole occupancy model, and a campaign typically has many walks sharing the
/// same handful of gate states, so views are cached by their region set. Keyed by
/// a sorted `Vec<Region>` and stored in insertion order: deterministic, no
/// hash-order iteration (ADR-0006).
#[derive(Default)]
struct SealCache {
    index: BTreeMap<Vec<crate::timeline::Region>, usize>,
    worlds: Vec<World>,
}

impl SealCache {
    /// The index of the sealed view for `seal`, or `None` when nothing is sealed
    /// (the caller then uses the base world — which is what keeps a campaign with
    /// no `close-gate` byte-identical: no clone, no different world, same routes).
    fn index_of(&mut self, base: &World, seal: &crate::timeline::GateState) -> Option<usize> {
        if seal.is_empty() {
            return None;
        }
        let key: Vec<crate::timeline::Region> = seal.keys().copied().collect();
        if let Some(&i) = self.index.get(&key) {
            return Some(i);
        }
        self.worlds.push(base.with_sealed(&seal_cells(seal)));
        let i = self.worlds.len() - 1;
        self.index.insert(key, i);
        Some(i)
    }
}

/// The `DW0410` diagnostic for a staged walk the timeline's own `close-gate`
/// makes impossible: names the verb, the mover, the leg, and every gate anchor
/// sealed ahead of it, plus the three ways out.
fn gate_timeline_error(
    verb: &str,
    mover: &str,
    to_anchor: &str,
    start: [i32; 3],
    target: [i32; 3],
    seal: &crate::timeline::GateState,
) -> NavError {
    let gates: Vec<&str> = seal.values().map(|s| s.as_str()).collect();
    NavError {
        code: DW_GATE_TIMELINE,
        message: format!(
            "{verb}: `{mover}` cannot walk the leg {start:?} → `{to_anchor}` {target:?} — the \
             route exists on the open world, but an EARLIER effect in this same timeline sealed \
             gate {} with `close-gate`, and no route remains once it is shut. The walk would \
             step through solid blocks at runtime. Move the walk before the `close-gate` (a \
             lower `at_ticks` / earlier position in the bundle), reopen the gate with \
             `open-gate` before the walk, or route the walk to a destination reachable on the \
             sealed side",
            gates
                .iter()
                .map(|g| format!("`{g}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// Plan every `move-actor` into a walked-path [`ActorMovePlan`] over the actor's
/// footprint, deduped by `(actor, to_anchor)` in first-seen order. `DW0325` when a
/// move is unroutable (names actor, leg, first blocked cell). Each actor's
/// successive moves **chain** — first leg from the declared spawn anchor, every
/// later leg from the previous leg's target (round-6 fix; see the loop comment).
/// Two moves sharing `(actor, to_anchor)` still share one content-keyed driver,
/// planned from the first occurrence's origin (documented limitation).
///
/// Use-gate cells are walkable edges for a scripted puppet walk, exactly as for
/// `move-npc` (see [`plan_moves`]): the island ram's pen→mouth leg crosses the pen
/// gate the player has just opened — through the threshold, no longer over the
/// fence-top the full-solid model wrongly proved.
///
/// **Timeline gates (round 8).** Each walk is planned over the world with the
/// gates its own timeline already sealed forced solid ([`crate::timeline`]), so a
/// legal way around a shut gate is found when one exists, and `DW0410` is raised
/// only when none does. A deduped repeat occurrence re-verifies the *already
/// planned* path against its own timeline's seals — that is the path the shared
/// driver will actually walk.
pub fn plan_actor_moves(plan: &Plan, world: &World) -> Result<Vec<ActorMovePlan>, NavError> {
    let mut out = Vec::new();
    let mut seen: BTreeSet<(String, String, String)> = BTreeSet::new();
    // Chained origins (round-6, live-server proven): a SECOND consecutive
    // `move-actor` on the same actor must start from the actor's CURRENT staged
    // location — the previous move's (snapped) target — not its declared spawn
    // anchor. Planning every leg from the declared anchor degenerated the
    // island's t=260 mouth→fire-pit walk into a single-waypoint instant teleport
    // (start == declared anchor == target), so the giant snapped instead of
    // walking on camera. Keyed by actor id, in campaign effect order (the same
    // deterministic order the dedup uses).
    // Branch-aware, exactly as `plan_moves`: a
    // puppet leg inherits only an origin its own branch produced. The-wake's bier
    // walked to the tide line from the GROUND branch's grave for the same reason
    // the island's Eurylochus walked from the beach.
    let mut history: StagingHistory = StagingHistory::new();
    // The cell route planned for each `(actor, to_anchor)` driver, so a deduped
    // repeat occurrence can be re-checked against its own timeline's seals.
    let mut planned: BTreeMap<(String, String, String), Vec<[i32; 3]>> = BTreeMap::new();
    // The yaw each planned driver ends on, so a deduped repeat chains it forward.
    let mut planned_end_yaw: BTreeMap<(String, String, String), i32> = BTreeMap::new();
    // The origin each driver was planned from + the branch that planned it, for
    // `DW0488`.
    let mut planned_origin: BTreeMap<(String, String, String), ([i32; 3], BranchGate)> =
        BTreeMap::new();
    let mut cache = SealCache::default();
    for (eff, seal) in crate::timeline::walk(plan) {
        let QuestEffect::MoveActor {
            actor,
            to_anchor,
            speed,
            ..
        } = eff
        else {
            continue;
        };
        let gate = BranchGate::of(eff);
        // The world this walk actually happens in: gates this timeline already
        // shut are solid. Empty seal ⇒ the base world, unchanged.
        let leg_world: &World = match cache.index_of(world, &seal) {
            Some(i) => &cache.worlds[i],
            None => world,
        };
        let a = actor_of(plan, actor.as_str()).ok_or_else(|| NavError {
            code: DW_ACTOR_UNROUTABLE,
            message: format!(
                "move-actor: unknown actor `{}` — declare it in the stage-5 `actors` list",
                actor.as_str()
            ),
        })?;
        let fp = entity_footprint(&a.entity);
        let dest = actor_anchor_pos(plan, to_anchor.as_str()).ok_or_else(|| NavError {
            code: DW_ACTOR_UNROUTABLE,
            message: format!(
                "move-actor: destination anchor `{}` for actor `{}` did not resolve to a world \
                 position — use a `to_anchor` some area's prefab provides",
                to_anchor.as_str(),
                actor.as_str()
            ),
        })?;
        let target = leg_world
            .snap_standable_fp(dest, SNAP_RADIUS, &fp)
            .ok_or_else(|| NavError {
                code: DW_ACTOR_UNROUTABLE,
                message: format!(
                    "move-actor: no cell the `{}` footprint can stand on near destination anchor \
                     `{}` {dest:?} for actor `{}` — the anchor is walled in, too low a ceiling for \
                     this mob, or over void",
                    a.entity,
                    to_anchor.as_str(),
                    actor.as_str()
                ),
            })?;
        let gkey = gate.key();
        let key = (
            actor.as_str().to_string(),
            to_anchor.as_str().to_string(),
            gkey.clone(),
        );
        if !seen.insert(key.clone()) {
            // Deduped: this occurrence shares the first occurrence's content-keyed
            // driver, so the path it walks is the one already planned. It still has
            // its OWN timeline, and a gate shut in *this* timeline would send that
            // shared path through solid blocks — so re-check the planned route
            // against these seals rather than waving the repeat through (DW0410).
            if !seal.is_empty()
                && let Some(cells) = planned.get(&key)
            {
                let sealed = seal_cells(&seal);
                if cells.iter().any(|c| sealed.contains(c)) {
                    return Err(gate_timeline_error(
                        "move-actor",
                        actor.as_str(),
                        to_anchor.as_str(),
                        cells[0],
                        target,
                        &seal,
                    ));
                }
            }
            // Shared driver, so this occurrence starts at the origin the first
            // one planned — correct only if this branch leaves the puppet there.
            if let Some((planned_from, planned_gate)) = planned_origin.get(&key) {
                let here = chained_staging(&history, actor.as_str(), &gate).map(|s| s.pos);
                if let Some(here) = here
                    && here != *planned_from
                {
                    return Err(shared_origin_error(
                        "move-actor",
                        actor.as_str(),
                        to_anchor.as_str(),
                        *planned_from,
                        planned_gate,
                        here,
                        &gate,
                    ));
                }
            }
            // The walk still ends here, so the actor's next leg chains from this
            // target — and from the facing the shared driver leaves the puppet in.
            record_staging(
                &mut history,
                actor.as_str(),
                gate,
                target,
                planned_end_yaw.get(&key).copied(),
            );
            continue;
        }
        let prior = chained_staging(&history, actor.as_str(), &gate);
        let start = match prior.map(|s| s.pos) {
            Some(pos) => pos,
            None => {
                let start_anchor =
                    actor_anchor_pos(plan, a.anchor.as_str()).ok_or_else(|| NavError {
                        code: DW_ACTOR_UNROUTABLE,
                        message: format!(
                            "move-actor: actor `{}` spawn anchor `{}` did not resolve to a world \
                             position — use a spawn `anchor` some area's prefab provides",
                            actor.as_str(),
                            a.anchor.as_str()
                        ),
                    })?;
                leg_world
                    .snap_standable_fp(start_anchor, SNAP_RADIUS, &fp)
                    .unwrap_or(start_anchor)
            }
        };
        let cells = match leg_world.find_path_fp(start, target, &fp) {
            Some(cells) => cells,
            // Unroutable in the timeline-correct world. Which diagnostic depends on
            // *why*: if the open world routes it, the campaign's own `close-gate` is
            // what makes it impossible (DW0410) — otherwise the geometry simply does
            // not connect, which is the long-standing DW0325.
            None if !seal.is_empty() && world.find_path_fp(start, target, &fp).is_some() => {
                return Err(gate_timeline_error(
                    "move-actor",
                    actor.as_str(),
                    to_anchor.as_str(),
                    start,
                    target,
                    &seal,
                ));
            }
            None => {
                let blocked = first_blocked_fp(leg_world, start, target, &fp);
                return Err(NavError {
                    code: DW_ACTOR_UNROUTABLE,
                    message: format!(
                        "move-actor: actor `{}` ({}) cannot walk the leg {start:?} (last staged \
                         location; spawn anchor `{}`) → `{}` {target:?} — no collision-free path \
                         for its footprint over the assembled geometry (first blocked cell \
                         ~{blocked:?}). Route the move within one connected area, widen the \
                         corridor/ceiling for this mob, or split it into shorter reachable hops",
                        actor.as_str(),
                        a.entity,
                        a.anchor.as_str(),
                        to_anchor.as_str(),
                    ),
                });
            }
        };
        planned.insert(key.clone(), cells.clone());
        planned_origin.insert(key.clone(), (start, gate.clone()));
        let waypoints = resample(&cells, speed.unwrap_or(DEFAULT_SPEED));
        // Seed: the facing the puppet already has — the exit yaw of the previous
        // leg **on this branch**, else the actor's declared spawn `facing`
        // (`emit::actor_facing_yaw`).
        let seed = prior
            .and_then(|s| s.yaw)
            .unwrap_or_else(|| crate::emit::facing_yaw(a.facing.map(|f| f.token())));
        let yaws = yaws_along(&waypoints, seed);
        let end_yaw = yaws.last().copied().unwrap_or(seed);
        record_staging(&mut history, actor.as_str(), gate, target, Some(end_yaw));
        planned_end_yaw.insert(key, end_yaw);
        out.push(ActorMovePlan {
            actor: actor.as_str().to_string(),
            to_anchor: to_anchor.as_str().to_string(),
            target,
            cells,
            waypoints,
            yaws,
            gate_key: gkey,
        });
    }
    Ok(out)
}

/// Verify every declared actor's spawn anchor resolves to a world position (the
/// puppet has somewhere to spawn). `DW0325` when it does not. Needs no `World` (a
/// spawn is a summon, not a walk), so it runs even for spawn-only campaigns.
pub fn check_actor_placement(plan: &Plan) -> Result<(), NavError> {
    for a in &plan.campaign.quests.content.actors {
        if actor_anchor_pos(plan, a.anchor.as_str()).is_none() {
            return Err(NavError {
                code: DW_ACTOR_UNROUTABLE,
                message: format!(
                    "actor `{}` spawn anchor `{}` did not resolve to a world position — use an \
                     `anchor` some area's prefab provides",
                    a.id.as_str(),
                    a.anchor.as_str()
                ),
            });
        }
    }
    Ok(())
}

/// The home-anchor id of an NPC, for diagnostics (or `?` if unknown).
fn plan_npc_anchor(plan: &Plan, npc_id: &str) -> String {
    plan.campaign
        .npcs
        .content
        .npcs
        .iter()
        .find(|n| n.id.as_str() == npc_id)
        .map(|n| n.anchor.as_str().to_string())
        .unwrap_or_else(|| "?".to_string())
}

/// The camera dolly world points of a cutscene (anchor + offset, block centres) —
/// the exact points the emitter lerps between. Shared with the emitter so the
/// air-corridor check validates what actually ships.
pub fn camera_points(plan: &Plan, path: &[CameraWaypoint]) -> Vec<[f64; 3]> {
    path.iter()
        .map(|w| anchor_offset_point(plan, w.anchor.as_str(), w.offset))
        .collect()
}

/// The world point a cutscene's `look_at` subject resolves to (DSL v0.6) — the
/// same anchor + offset block-centre convention as [`camera_points`], so a
/// waypoint and a look target at the same anchor/offset name the same point.
pub fn camera_look_point(plan: &Plan, target: &delvewright_dsl::CameraTarget) -> [f64; 3] {
    anchor_offset_point(plan, target.anchor.as_str(), target.offset)
}

/// Resolve `anchor + offset` to a block-centre world point (the shared cutscene
/// camera convention). An unresolved anchor falls back to the layout origin —
/// referential validation reports it separately.
pub(crate) fn anchor_offset_point(plan: &Plan, anchor: &str, offset: [i32; 3]) -> [f64; 3] {
    let base = plan
        .anchors
        .iter()
        .find(|((_, name), _)| name == anchor)
        .map(|(_, r)| match r {
            ResolvedAnchor::Point { pos, .. } => *pos,
            ResolvedAnchor::Gate { from, .. } => *from,
        })
        .unwrap_or([0, crate::plan::BASE_Y, 0]);
    [
        (base[0] + offset[0]) as f64 + 0.5,
        (base[1] + offset[1]) as f64 + 0.5,
        (base[2] + offset[2]) as f64 + 0.5,
    ]
}

/// Validate every cutscene camera dolly (per shot: a multi-shot cutscene
/// hard-cuts between shots, so only the within-shot dolly is a corridor the
/// camera actually flies):
///
/// - **`DW0308` (authored polyline)**: the waypoint polyline passes only
///   through non-solid blocks (cameras fly but must not clip a solid). Names
///   the offending shot, segment and clipping block.
/// - **`DW0308` (rendered chords)**: the client draws straight chords between
///   the emitted keyframes ([`crate::camera::plan_shot`] — the tween is
///   client-side and linear, spike-measured), which can cut a corner of the
///   authored polyline by up to [`crate::camera::CHORD_POS_TOLERANCE`]. The
///   chord polyline is what actually ships, so it is ray-checked too.
/// - **`DW0347` (angular budget)**: the shot's peak aim rate must stay within
///   [`crate::camera::MAX_AIM_DEG_PER_TICK`]. An over-budget pan is a
///   provably nauseating shot — an error, not a warning: the fix (more camera
///   distance, a longer shot, or a hard cut between two shots) is always
///   available, and a red check is information (CLAUDE.md debug doctrine).
pub fn check_cutscenes(
    plan: &Plan,
    world: &World,
    moves: &[MovePlan],
    actor_moves: &[ActorMovePlan],
) -> Result<(), NavError> {
    for (eff, ctx) in crate::camera::cutscene_units(plan.campaign) {
        let Some(shots) = eff.cutscene_shots() else {
            continue;
        };
        let mut offset: i32 = 0;
        for (si, shot) in shots.iter().enumerate() {
            let ex = crate::camera::expand_shot(plan, moves, actor_moves, shot, &ctx, offset);
            offset += ex.ticks + 1;
            let pts = ex.clip_polyline();
            if let Some((seg, cell)) = first_clip(world, pts) {
                return Err(NavError {
                    code: DW_CUTSCENE_CLIP,
                    message: format!(
                        "cutscene: shot {si} camera dolly segment {seg} (from {:?} to {:?}) clips \
                         a solid block at {cell:?} — cameras must fly through open air; move the \
                         segment's waypoint `anchor`/`offset` (or the shot's `bearing`/`dist` for \
                         a styled shot) so the whole path clears solid blocks",
                        round3(pts[seg]),
                        round3(pts[seg + 1]),
                    ),
                });
            }
            let frames = ex.frames();
            let chord: Vec<[f64; 3]> = frames.frames.iter().map(|f| f.pos).collect();
            if let Some((seg, cell)) = first_clip(world, &chord) {
                return Err(NavError {
                    code: DW_CUTSCENE_CLIP,
                    message: format!(
                        "cutscene: shot {si} client-rendered dolly chord {seg} (keyframe {:?} to \
                         {:?}) clips a solid block at {cell:?} — the client tweens straight \
                         between keyframes, cutting inside the authored waypoint corner; move the \
                         nearby waypoint `anchor`/`offset` a block outward so the smoothed path \
                         also clears",
                        round3(chord[seg]),
                        round3(chord[seg + 1]),
                    ),
                });
            }
            let rate = ex.max_aim_deg_per_tick();
            if rate > crate::camera::MAX_AIM_DEG_PER_TICK {
                return Err(NavError {
                    code: DW_CAMERA_SPIN,
                    message: format!(
                        "cutscene: shot {si} pans at {rate} deg/tick, over the {} deg/tick \
                         (120 deg/s) budget — at 20 Hz that reads as a spin, not a shot \
                         (comfortable is <= 2 deg/tick). Move the camera path farther from its \
                         `look_at` subject, lengthen `seconds`, or split the move into two shots \
                         (the hard cut between shots is the idiomatic fast reframe)",
                        crate::camera::MAX_AIM_DEG_PER_TICK,
                    ),
                });
            }
        }
    }
    Ok(())
}

/// The first `(segment index, block cell)` where a camera dolly polyline passes
/// through a solid block, or `None` if the whole path is air.
///
/// **Exact, not sampled**. This used to step each segment at ≤ 0.25
/// blocks and floor the sample point, which misses any cell the segment only
/// grazes: a shot can cut a block corner, enter and leave the cell entirely
/// between two samples, and ship as "provably clear". The clip test is now a
/// 3-D grid walk (Amanatides–Woo digital differential analyser) that visits
/// **every** cell the segment intersects, in order, with no error term at all —
/// so `DW0308` can no longer be dodged by geometry that happens to fall between
/// two sample points.
///
/// Deterministic (ADR-0006): integer cell stepping driven by exact ratios; ties
/// (a segment passing exactly through a cell corner) resolve on the fixed axis
/// order x, y, z.
fn first_clip(world: &World, pts: &[[f64; 3]]) -> Option<(usize, [i32; 3])> {
    for (seg, w) in pts.windows(2).enumerate() {
        if let Some(cell) = walk_cells(w[0], w[1], |c| world.blocks_camera(c)) {
            return Some((seg, cell));
        }
    }
    None
}

/// Walk every unit cell the segment `a → b` passes through, in order, returning
/// the first for which `hit` holds. Amanatides–Woo voxel traversal: from the
/// starting cell, repeatedly advance along whichever axis reaches its next cell
/// boundary soonest. An axis with zero delta never steps (its `t_max` is
/// infinite). Both endpoint cells are included.
///
/// **Public**, and `FnMut` rather than `Fn`: the camera clip wants the FIRST
/// blocking cell and stops, while a blocked sightline (`DW0821`) owes its reader
/// EVERY blocking cell, because a walk sheet that names one cell of a wall has
/// not told anybody where the wall is. One traversal, two questions, and the
/// difference lives entirely in the closure.
pub fn walk_cells(
    a: [f64; 3],
    b: [f64; 3],
    mut hit: impl FnMut([i32; 3]) -> bool,
) -> Option<[i32; 3]> {
    let mut cell = [
        a[0].floor() as i32,
        a[1].floor() as i32,
        a[2].floor() as i32,
    ];
    let end = [
        b[0].floor() as i32,
        b[1].floor() as i32,
        b[2].floor() as i32,
    ];
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let mut step = [0i32; 3];
    // `t` is the fraction of the segment consumed; `t_max[i]` is the fraction at
    // which the next boundary on axis `i` is crossed, `t_delta[i]` the fraction one
    // whole cell costs on that axis.
    let mut t_max = [f64::INFINITY; 3];
    let mut t_delta = [f64::INFINITY; 3];
    for i in 0..3 {
        if d[i] > 0.0 {
            step[i] = 1;
            t_max[i] = ((cell[i] + 1) as f64 - a[i]) / d[i];
            t_delta[i] = 1.0 / d[i];
        } else if d[i] < 0.0 {
            step[i] = -1;
            t_max[i] = (cell[i] as f64 - a[i]) / d[i];
            t_delta[i] = -1.0 / d[i];
        }
    }
    if hit(cell) {
        return Some(cell);
    }
    // A segment crosses at most |Δcell| boundaries per axis; the bound makes the
    // loop provably terminating even against a degenerate (NaN-free) input.
    let budget: i64 = (0..3)
        .map(|i| (end[i] - cell[i]).unsigned_abs() as i64)
        .sum();
    for _ in 0..budget {
        // Advance on the axis whose next boundary comes soonest (fixed x, y, z
        // tie-break keeps a corner crossing deterministic).
        let axis = if t_max[0] <= t_max[1] && t_max[0] <= t_max[2] {
            0
        } else if t_max[1] <= t_max[2] {
            1
        } else {
            2
        };
        if t_max[axis] > 1.0 {
            break; // the next boundary lies past the segment's end
        }
        cell[axis] += step[axis];
        t_max[axis] += t_delta[axis];
        if hit(cell) {
            return Some(cell);
        }
    }
    // The end cell is always tested, even if float error stopped the walk short.
    if cell != end && hit(end) {
        return Some(end);
    }
    None
}

fn round3(p: [f64; 3]) -> [f64; 3] {
    [
        (p[0] * 1000.0).round() / 1000.0,
        (p[1] * 1000.0).round() / 1000.0,
        (p[2] * 1000.0).round() / 1000.0,
    ]
}

/// Every quest effect in the campaign (objective-complete, quest-complete, and
/// trigger effects), matching the emitter's `all_campaign_effects` traversal —
/// each effect ahead of the ones nested in its `sequence` steps / `on_arrive`
/// bundle (spec-0014), so nav planning sees moves and cutscenes wherever they
/// appear. Pre-0.6 campaigns have no nesting, so the flattened list equals the
/// shallow one and output stays byte-identical.
///
/// Defined as [`crate::timeline::walk`] with the per-effect gate states dropped:
/// the two share **one** traversal, so the effect a planner is looking at and the
/// timeline state attributed to it can never drift out of alignment.
fn all_effects<'a>(plan: &'a Plan) -> Vec<&'a QuestEffect> {
    crate::timeline::walk(plan)
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
            e,
            QuestEffect::MoveNpc { .. } | QuestEffect::Cutscene { .. } | QuestEffect::MoveActor { .. }
        )
    })
    // The critical-path walkability check (DW0311) also needs the occupancy model.
        || has_walkable_critical_leg(plan)
    // The checkpoint (DW0315/DW0316) and stealth-zone (DW0327) proofs, v0.6, need
    // the assembled occupancy model too, as does the trap proof (DW0342, spec-0011).
        || !plan.checkpoints.is_empty()
        || !plan.stealth_beats.is_empty()
        || !plan.traps.is_empty()
}

/// The player-visited critical-path positions in order, each tagged with whether
/// the player was teleported here by an inter-area transport on the preceding
/// step (a ride, not a walk). `select-class` / `assert-complete` steps carry no
/// position and are skipped.
/// A player-visited critical-path position, with the metadata walked-leg routing
/// needs. Replaces the bare `([i32;3], bool)` tuple so a talk-to target — whose
/// anchor cell is the NPC's own occupied cell — can be endpoint-snapped correctly.
#[derive(Debug, Clone, Copy)]
struct VisitedPos {
    /// The raw visited anchor cell (an NPC stand, altar, chest, wave marker, …).
    pos: [i32; 3],
    /// The player rides an inter-area transport INTO this position, so the move
    /// here is a teleport, not a walk to validate/export.
    transport_before: bool,
    /// This position is a talk-to NPC anchor: the player stands *within interaction
    /// range beside* the NPC, never on the mannequin-occupied anchor cell, so the
    /// goal snap must exclude that cell.
    talk_to: bool,
    /// The originating `critical_path` step index (v0.6): lets the checkpoint /
    /// stealth proofs select the positions at or after a firing step.
    src_step: usize,
}

/// How many critical-path **legs** the completability proof routes — consecutive
/// visited pairs, minus the inter-area teleport hops it skips. The binding count
/// `validation/lethal-gate.json` reports for `DW0510`: a lethal volume proven over
/// zero legs is a vacuous green, and this is what makes that legible.
pub fn critical_leg_count(plan: &Plan) -> usize {
    critical_positions(plan)
        .windows(2)
        .filter(|pair| !pair[1].transport_before)
        .count()
}

fn critical_positions(plan: &Plan) -> Vec<VisitedPos> {
    positions_of(&plan.critical_path, &plan.critical_path_transport)
}

/// [`critical_positions`] over an arbitrary exported step list — the shared core,
/// split out so a spec-0025 **branch path** (a different sequence of
/// the same step shapes, with its own transport markers) yields its own visited
/// positions in its own step space. `src_step` indices are indices into `steps`.
fn positions_of(steps: &[Step], transports: &[Option<[i32; 3]>]) -> Vec<VisitedPos> {
    let mut out = Vec::new();
    let mut transport_pending = false;
    for (i, step) in steps.iter().enumerate() {
        let pos = match step {
            Step::TalkTo { pos, .. }
            | Step::Reach { pos, .. }
            | Step::Kill { pos, .. }
            | Step::Collect { pos, .. }
            | Step::Interact { pos, .. } => Some(*pos),
            Step::SelectClass { .. } | Step::AssertComplete { .. } => None,
        };
        if let Some(pos) = pos {
            out.push(VisitedPos {
                pos,
                transport_before: transport_pending,
                talk_to: matches!(step, Step::TalkTo { .. }),
                src_step: i,
            });
            transport_pending = false;
        }
        // A transport marker on step `i` teleports the player when that step's
        // objective completes — i.e. before the *next* visited position is reached,
        // so the move INTO that next position is a ride, not a walk to validate.
        if transports.get(i).and_then(|t| *t).is_some() {
            transport_pending = true;
        }
    }
    out
}

/// Whether the campaign has at least one consecutive pair of player-visited
/// critical-path positions with no inter-area transport between them — a leg the
/// player must walk, hence one DW0311 must validate.
fn has_walkable_critical_leg(plan: &Plan) -> bool {
    critical_positions(plan)
        .windows(2)
        .any(|w| !w[1].transport_before)
}

/// Validate that every consecutive pair of player-visited critical-path anchors is
/// connected by a walkable A* path over the assembled geometry (unless the player
/// rides an inter-area transport between them). This is the compile-time counterpart
/// to the runtime critical-path bot: it makes an unwalkable assembled seam — a
/// prefab whose regenerated geometry wedged a doorway shut or opened a void gap — a
/// build failure ([`DW_CRITICAL_UNROUTABLE`], `DW0311`) instead of a bot surprise.
///
/// Endpoints are snapped to the nearest standable floor cell (an anchor often marks
/// a solid affordance — an altar, a wave marker, an NPC stand — the player walks up
/// to, not into), exactly as `move-npc` planning does.
pub fn check_critical_path(plan: &Plan, world: &World) -> Result<(), NavError> {
    route_visited(
        world,
        &critical_positions(plan),
        &plan.region_events,
        &|g, s| plan.gate_fired_before(g, s),
    )
}

/// What every runtime-written region has become, as of one point in the quest DAG:
/// the cells a write has made solid, and the cells a write has cleared.
///
/// One value, because it is one question. A proof that asked only "what is walled
/// off" is the shape the capability had while `close-gate` owned it privately, and
/// it is why the other half — "what is now open" — had no answer for anything but a
/// gate, whose cells the assembled model happens to hold open unconditionally.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct RegionState {
    /// Cells a runtime fill has made solid by this point.
    solid: BTreeSet<[i32; 3]>,
    /// Cells a runtime clear has emptied by this point.
    cleared: BTreeSet<[i32; 3]>,
    /// Cells a runtime fill has filled with **fluid** by this point
    /// ([`crate::plan::RegionWrite::Flood`]) — impassable, and never floor.
    ///
    /// A third set rather than a flag on `solid`, because the two answers differ in
    /// the direction that matters: a body cannot walk through either, and can stand
    /// on only one. Folding a water fill into `solid` is what let the route proof
    /// walk a party across a pond in mid-air.
    flooded: BTreeSet<[i32; 3]>,
    /// The boxes behind `flooded`, in region order — carried so a route failure can
    /// NAME the write that caused it instead of reporting unroutable geometry that
    /// looks perfectly open, exactly as [`World::lethal_regions`] does for a lethal
    /// volume.
    flood_regions: Vec<([i32; 3], [i32; 3])>,
    /// Cells an **unforced** fill has written by this point: a solid block laid from
    /// a beat the party may never play ([`crate::plan::RegionEvent::is_forced`]).
    ///
    /// A fourth set for the same reason `flooded` is a third: the two answers differ
    /// in the direction that matters. The party may arrive to find this box walled,
    /// so the proof may not walk *through* it; the party may equally arrive to find
    /// it as the world built it, so the proof may not stand *on* it. That is the
    /// pointwise-worst of the two futures, and it is the only reading of an unforced
    /// fill that is sound in both.
    ///
    /// Folding it into `solid` is what let a forced leg cross a chasm on a plank a
    /// trapped chest lays — provably completable, and physically unwalkable for a
    /// party that never opened the chest.
    unforced: BTreeSet<[i32; 3]>,
    /// The boxes behind `unforced`, each with the beat that lays it in words —
    /// carried for the same reason `flood_regions` is, so a route failure can NAME
    /// the beat instead of reporting geometry that reads perfectly open.
    unforced_regions: Vec<UnforcedBox>,
}

/// One box an unforced fill writes, with the beat that lays it in words — the blame
/// unit [`DW_UNFORCED_FOOTING`] reports.
type UnforcedBox = (([i32; 3], [i32; 3]), String);

/// Per region, the causally-latest write that precedes a leg: its firing step, what
/// it leaves, whether the party is forced to cause it, and the beat to blame if they
/// are not. Forcedness travels WITH the winner, so latest-write-wins needs no special
/// case for a forced write landing on top of an unforced one.
type LatestWrite = (usize, RegionWrite, bool, String);

impl RegionState {
    /// Nothing has been written by this point — the caller routes the base world
    /// and clones nothing.
    fn is_empty(&self) -> bool {
        self.solid.is_empty()
            && self.cleared.is_empty()
            && self.flooded.is_empty()
            && self.unforced.is_empty()
    }

    /// This state as it would be **if every unforced fill were credited** — the
    /// counterfactual [`DW_UNFORCED_FOOTING`] is derived from, and exactly the model
    /// this compiler ran before a firing's forcedness reached the geometry.
    ///
    /// A leg that routes here and nowhere else failed *because* the only footing it
    /// had was laid by a beat nobody has to play.
    fn as_if_forced(&self) -> RegionState {
        let mut st = self.clone();
        st.solid.extend(st.unforced.iter().copied());
        st.unforced.clear();
        st
    }
}

/// The unforced boxes a route's `cells` stand in or on, each named with the beat that
/// lays it — the cell itself and the cell **below** it, because footing is what this
/// asks about, exactly as [`World::flood_regions_over`] does.
///
/// A free function over the ledger rather than a method on [`RegionState`], because
/// the caller has already handed the state's other halves to the route it proved and
/// must not be made to keep the whole value alive to say what went wrong.
fn unforced_blame_over(regions: &[UnforcedBox], cells: &[[i32; 3]]) -> Vec<String> {
    let touched: Vec<[i32; 3]> = cells
        .iter()
        .flat_map(|c| [*c, [c[0], c[1] - 1, c[2]]])
        .collect();
    regions
        .iter()
        .filter(|((lo, hi), _)| {
            touched
                .iter()
                .any(|c| (0..3).all(|i| lo[i].min(hi[i]) <= c[i] && c[i] <= lo[i].max(hi[i])))
        })
        .map(|((lo, hi), why)| {
            format!(
                "[{}, {}, {}]..[{}, {}, {}] (laid by {why})",
                lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]
            )
        })
        .collect()
}

/// The state of every runtime-written region on a walked leg arriving at the
/// objective at critical-path step `arrival` (DSL v0.6 `close-gate`, generalised in
/// v0.10 by spec-0031). A write counts only if its firing objective is a **causal
/// (DAG) ancestor** of the leg's objective — `ancestor(ev.fire_step, arrival)` —
/// i.e. it is guaranteed to have happened before this leg in *every* valid play
/// order. That excludes a write on a parallel quest branch that the lineariser
/// merely happens to interleave ahead of this leg (which would falsely seal it).
/// Among the causally-preceding writes on a region, the **latest** (max
/// `fire_step`, respecting the DAG linearisation) wins: the region is solid iff
/// that latest write is a fill, and cleared iff it is a clear.
///
/// A campaign that writes no region yields an empty state and routes byte-
/// identically to the base world.
///
/// **The world's own writes come first.** The list this reasons over is not the
/// campaign's alone: a gate the placed prefabs author shut is a `Fill` at step 0
/// ([`World::world_load_seals`]) and enters here exactly like a `close-gate`. It is
/// a method on [`World`] for that reason — the world is the only object that knows
/// what it was built holding, and routing it through here is what makes an
/// `open-gate` the thing that *opens* a door rather than the thing that *proves it
/// was a door*. Before that, a gate region was passable unless a `close-gate`
/// sealed it, so the one mistake an author actually makes — forgetting to open a
/// door — was the one mistake this model could not represent.
impl World {
    fn region_state_at(
        &self,
        region_events: &[RegionEvent],
        arrival: usize,
        ancestor: &dyn Fn(usize, usize) -> bool,
    ) -> RegionState {
        self.region_state_inner(region_events, arrival, ancestor, true)
    }

    /// [`World::region_state_at`] as it would be **if the world had been built with
    /// every gate already open** — the counterfactual [`DW_GATE_NEVER_OPENED`] is
    /// derived from, and the exact model this compiler shipped before the seals
    /// were measured.
    fn region_state_without_world_load(
        &self,
        region_events: &[RegionEvent],
        arrival: usize,
        ancestor: &dyn Fn(usize, usize) -> bool,
    ) -> RegionState {
        self.region_state_inner(region_events, arrival, ancestor, false)
    }

    fn region_state_inner(
        &self,
        region_events: &[RegionEvent],
        arrival: usize,
        ancestor: &dyn Fn(usize, usize) -> bool,
        world_load: bool,
    ) -> RegionState {
        // Per region, the causally-latest write that precedes this leg (ancestor of
        // the arrival objective); higher `fire_step` overrides. The winner carries
        // its own forcedness and blame, so a forced write landing after an unforced
        // one on the same box restores ordinary footing by winning, with no special
        // case: latest-write-wins already says which firing the party will find.
        let mut latest: BTreeMap<([i32; 3], [i32; 3]), LatestWrite> = BTreeMap::new();
        let world_load: Vec<RegionEvent> = if world_load {
            // FORCED: a gate the placed prefabs author shut is shut because the world
            // was built that way, not because anyone played a beat.
            self.modelled_seals()
                .map(|s| RegionEvent::forced(s.region, RegionWrite::Fill, 0))
                .collect()
        } else {
            Vec::new()
        };
        for ev in world_load.into_iter().chain(region_events.iter().cloned()) {
            if ancestor(ev.fire_step, arrival) {
                let key = (
                    ev.fire_step,
                    ev.write,
                    ev.is_forced(),
                    ev.blame().to_string(),
                );
                let e = latest.entry(ev.region).or_insert_with(|| key.clone());
                if ev.fire_step >= e.0 {
                    *e = key;
                }
            }
        }
        let mut st = RegionState::default();
        for (region, (_, write, forced, blame)) in latest {
            // An `Unseal` contributes to no set: it removes the gate's own block, and
            // the base world holds the gate cells empty. It matters above, in
            // latest-write-wins, where it is what cancels a fill — including the
            // world-load fill this gate was born with.
            //
            // A `Fill` splits on forcedness and nothing else does. `Flood` needs no
            // split (impassable and never floor is already the worst of both
            // futures); `Clear` and `Unseal` never reach here unforced, because
            // `plan::collect_region_events` drops them — an unforced firing may make
            // a region impassable and may never make one passable.
            let into = match write {
                RegionWrite::Fill if !forced => {
                    st.unforced_regions.push((region, blame));
                    &mut st.unforced
                }
                RegionWrite::Fill => &mut st.solid,
                RegionWrite::Clear => &mut st.cleared,
                RegionWrite::Flood => {
                    st.flood_regions.push(region);
                    &mut st.flooded
                }
                RegionWrite::Unseal => continue,
            };
            into.extend(crate::assembled::region_cells(region.0, region.1));
        }
        st
    }

    /// The runtime-region state for the walked leg `from_step → to_step` — the
    /// single definition of "which regions are filled and which are cleared while
    /// the player walks this leg", shared by the completability proof, the forced-
    /// cell set the trap proof reasons about, and the exported harness
    /// waypoints.
    ///
    /// Only a **causal** leg is written — one whose start objective is a DAG
    /// ancestor of the arrival objective, i.e. a step the player is genuinely
    /// forced to walk to reach the arrival. The lineariser concatenates parallel
    /// quest branches, producing artifact "legs" between objectives with no causal
    /// order (e.g. a `take-the-cheese` beat followed by a `nobody` beat on a
    /// sibling branch); the player never actually walks that pairing under the
    /// arrival's region state, so sealing it would falsely fail. A genuinely-forced
    /// re-crossing (start IS a causal ancestor) is still sealed, preserving the
    /// proof. Base DW0311 (open world) already checked every leg.
    fn leg_region_state(
        &self,
        region_events: &[RegionEvent],
        ancestor: &dyn Fn(usize, usize) -> bool,
        from_step: usize,
        to_step: usize,
    ) -> RegionState {
        if ancestor(from_step, to_step) {
            self.region_state_at(region_events, to_step, ancestor)
        } else {
            RegionState::default()
        }
    }

    /// The same leg with the world-load gate seals lifted — see
    /// [`World::region_state_without_world_load`].
    fn leg_region_state_without_world_load(
        &self,
        region_events: &[RegionEvent],
        ancestor: &dyn Fn(usize, usize) -> bool,
        from_step: usize,
        to_step: usize,
    ) -> RegionState {
        if ancestor(from_step, to_step) {
            self.region_state_without_world_load(region_events, to_step, ancestor)
        } else {
            RegionState::default()
        }
    }

    /// [`World::leg_region_state`] for a leg the player is asked to WALK, with the
    /// one exemption the world-load seal carries: a leg whose start sits inside a
    /// declared `teleport` source volume is judged with the seals lifted.
    ///
    /// The party may be carried off that cell before they ever face the door, and
    /// nothing in the critical path says whether they were — `transport_before`
    /// marks only the compiler's own inter-area rides. Lifting the seal restores
    /// exactly the pre-measurement verdict for such a leg, so DW0311's binding is
    /// unchanged and no campaign that compiled green over a teleport goes red.
    /// The single site that decides it, shared by the proof (`route_visited`) and
    /// the exported routes (`route_walked_legs`), so the route the harness walks is
    /// the route the proof passed.
    fn walked_leg_region_state(
        &self,
        region_events: &[RegionEvent],
        ancestor: &dyn Fn(usize, usize) -> bool,
        from_pos: [i32; 3],
        from_step: usize,
        to_step: usize,
    ) -> RegionState {
        if self.is_teleport_source(from_pos) {
            self.leg_region_state_without_world_load(region_events, ancestor, from_step, to_step)
        } else {
            self.leg_region_state(region_events, ancestor, from_step, to_step)
        }
    }
}

/// Render the [`DW_GATE_NEVER_OPENED`] blame clause for the gates a counterfactual
/// route crosses: each gate's anchor, its region, what the world puts in it, and —
/// the part that turns a symptom into a repair — **what the campaign does to it**.
///
/// Three answers, and they are three different bugs: nothing opens it anywhere
/// (a missing `open-gate`), something opens it but only from a bundle the party is
/// not forced to fire (a shop purchase, a sprung trap, a shortcut taken from the
/// far side — [`crate::plan::collect_region_events`]'s rule is that an optional
/// firing may seal a region and may never open one), or something opens it at a
/// step that is not a causal ancestor of this leg (opened, but too late, or on a
/// parallel branch).
fn gate_blame(
    gates: &[&crate::assembled::GateSeal],
    region_events: &[RegionEvent],
    ancestor: &dyn Fn(usize, usize) -> bool,
    arrival: usize,
) -> String {
    if gates.is_empty() {
        return "(none — the counterfactual route crosses no measured gate seal; this is a \
                compiler defect, escalate it)"
            .to_string();
    }
    gates
        .iter()
        .map(|g| {
            let openers: Vec<usize> = region_events
                .iter()
                .filter(|e| e.region == g.region && e.write != RegionWrite::Fill)
                .map(|e| e.fire_step)
                .collect();
            let fate = if openers.is_empty() {
                // Either the campaign declares no `open-gate` on this anchor at
                // all, or every one it declares hangs off an OPTIONAL bundle and
                // was never credited (`plan::collect_region_events`: an optional
                // firing may seal a region and may never open one). One sentence
                // for both, because the repair is the same — put the opening on a
                // beat the party cannot skip.
                "no firing the party is forced to make ever opens it (an `open-gate` in an \
                 optional bundle — a shop purchase, a sprung trap, a death beat, a shortcut \
                 taken from the far side — is not credited, by the same rule that keeps every \
                 shortcut gate sealed so the delve is finishable the long way)"
                    .to_string()
            } else if openers.iter().any(|&s| ancestor(s, arrival)) {
                // Unreachable while this is the blamed gate — an opener that is a
                // causal ancestor cancels the world-load fill — and stated anyway,
                // because a silent third branch is how a blame list starts lying.
                "an opener DOES precede this leg (compiler defect, escalate it)".to_string()
            } else {
                let steps: Vec<String> = openers.iter().map(|s| s.to_string()).collect();
                format!(
                    "it is opened only at critical-path step(s) {} — after this leg, or on a \
                     branch the party is not forced down",
                    steps.join(", ")
                )
            };
            let (lo, hi) = g.region;
            format!(
                "`{}` in area `{}` (region [{}, {}, {}]..[{}, {}, {}], {}/{} cells filled at \
                 world-load): {fate}",
                g.anchor, g.area, lo[0], lo[1], lo[2], hi[0], hi[1], hi[2], g.blocked, g.cells
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// **Where a player can walk from `seat`, under EVERY quest state that can hold
/// while that seat is the respawn point in force** (spec-0032).
///
/// Returns `(quest states examined, the intersected reachable set)`.
///
/// # Why an intersection, and why it lives here
///
/// spec-0032's placement rule says "under the quest state in force at that
/// moment". Nothing observable at runtime says WHICH point of a respawn point's
/// DAG span a death happened at — `#cp` names the seat, not the step — so the
/// table cannot key on quest state without inventing a runtime discriminator for
/// it. Intersecting instead makes the answer independent of that: a cell in this
/// set is reachable under every configuration the seat can be in force across, so
/// an anchor chosen from it is reachable whenever the player comes back.
///
/// **That is strictly stronger than the rule as written, not a simplification of
/// it** — the sentence a future reader needs, because it looks like a shortcut and
/// is the opposite. The rule permits an anchor reachable under the one quest state
/// that held at the moment of death; this permits only anchors reachable under all
/// of them.
///
/// It lives in this module, beside the model it reads, so `RegionState`,
/// [`World::region_state_at`] and [`World::with_region_state`] stay private: a second
/// passability model beside this one is exactly what spec-0031 refused when it
/// moved fill/clear out of the two verbs that held it privately.
///
/// A campaign with no runtime-written region has exactly one configuration and
/// pays one flood fill for it. Deterministic (ADR-0006): iteration is over a slice
/// in step order and over `BTreeSet` keys.
pub fn reachable_under_every_quest_state(
    plan: &Plan,
    world: &World,
    seat: [i32; 3],
    from_step: usize,
) -> (usize, BTreeSet<[i32; 3]>) {
    let ancestor = |g: usize, s: usize| plan.gate_fired_before(g, s);
    let mut seen: Vec<RegionState> = Vec::new();
    let mut acc: Option<BTreeSet<[i32; 3]>> = None;
    for arrival in from_step..=plan.critical_path.len() {
        let st = world.region_state_at(&plan.region_events, arrival, &ancestor);
        // Distinct configurations only. The number of them over a whole critical
        // path is small, and re-flooding an identical one would only cost time.
        // Compared WHOLE, never field by field. Two states that differ only in what
        // a runtime write flooded are two different worlds, and a hand-written
        // subset of the fields silently drops the newest one — which is exactly how
        // a third set gets added and the dedup keeps answering for two.
        if seen.contains(&st) {
            continue;
        }
        let w = if st.is_empty() {
            None
        } else {
            Some(world.with_region_state(&st))
        };
        let r = w.as_ref().unwrap_or(world).reachable_walkable(&[seat]);
        seen.push(st);
        acc = Some(match acc {
            None => r,
            Some(prev) => prev.intersection(&r).copied().collect(),
        });
    }
    // A seat whose span admits no configuration at all (its `from_step` is past the
    // last one) still gets the base world's answer rather than an empty set, which
    // would silently make every proof over it vacuous.
    (
        seen.len().max(1),
        acc.unwrap_or_else(|| world.reachable_walkable(&[seat])),
    )
}

/// Route every walked leg between consecutive visited positions over its
/// causally-sealed world ([`leg_seal`]), returning the proven cell routes. The
/// shared core of [`World::walked_legs`] and [`critical_path_routes`]; a leg whose
/// endpoints do not snap or that does not route is omitted (that is exactly the
/// [`route_visited`] failure, reported there as `DW0311`).
fn route_walked_legs(
    world: &World,
    positions: &[VisitedPos],
    region_events: &[RegionEvent],
    ancestor: &dyn Fn(usize, usize) -> bool,
) -> Vec<(LegRoute, BTreeSet<[i32; 3]>)> {
    let mut out = Vec::new();
    for pair in positions.windows(2) {
        if pair[1].transport_before {
            continue; // an inter-area teleport hop: the player is moved, not walking
        }
        let st = world.walked_leg_region_state(
            region_events,
            ancestor,
            pair[0].pos,
            pair[0].src_step,
            pair[1].src_step,
        );
        let leg_world_owned;
        let leg_world: &World = if st.is_empty() {
            world
        } else {
            leg_world_owned = world.with_region_state(&st);
            &leg_world_owned
        };
        let sealed = st.solid.clone();
        let (Some(start), Some(goal)) = (
            leg_world.snap_endpoint(pair[0].pos, false),
            leg_world.snap_endpoint(pair[1].pos, pair[1].talk_to),
        ) else {
            continue;
        };
        if let Some(cells) = leg_world.find_path(start, goal) {
            let use_gates = cells
                .iter()
                .copied()
                .filter(|&c| leg_world.is_use_gate(c))
                .collect();
            out.push((
                LegRoute {
                    from: pair[0].pos,
                    to: pair[1].pos,
                    to_step: pair[1].src_step,
                    cells,
                    use_gates,
                    // The leg carries the world it was PROVEN over, not just the
                    // route. Anything that re-judges these cells has to ask this
                    // value for the world to judge them in — see
                    // [`LegRoute::proven_world`].
                    region_state: st,
                },
                sealed,
            ));
        }
    }
    out
}

/// Route every walked leg between consecutive visited positions (the pure core of
/// [`check_critical_path`], split out so it is unit-testable without a full
/// [`Plan`]). A `transport_before` leg is a teleport ride and is skipped. Each leg
/// is routed over the world with any gate sealed by an earlier `close-gate`
/// ([`leg_seal`]) forced solid, so a forced path that must re-cross a sealed gate
/// fails [`DW_CRITICAL_UNROUTABLE`].
/// Render a blamed-volume list for a `DW0510` message: backticked ids joined by
/// `, `, or the honest `(none — the volume set is empty)` when the counterfactual
/// found a route that touches no declared volume, which would itself be a bug in
/// this proof rather than in the campaign.
fn names_of(ids: &[&str]) -> String {
    if ids.is_empty() {
        return "(none — the counterfactual route touches no declared volume; this is a \
                compiler defect, escalate it)"
            .to_string();
    }
    ids.iter()
        .map(|i| format!("`{i}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Render a blamed-region list for a `DW0544` message. A runtime region write has
/// no author-given id — `fill-region` names a box, not itself — so the box IS the
/// name, and it identifies the effect uniquely. The empty case is the same honest
/// admission [`names_of`] makes: the counterfactual found a route that touches no
/// fluid box, which would be a defect in this proof rather than in the campaign.
fn boxes_of(regions: &[([i32; 3], [i32; 3])]) -> String {
    if regions.is_empty() {
        return "(none — the counterfactual route touches no fluid-filled box; this is a \
                compiler defect, escalate it)"
            .to_string();
    }
    regions
        .iter()
        .map(|(lo, hi)| {
            format!(
                "[{}, {}, {}]..[{}, {}, {}]",
                lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn route_visited(
    world: &World,
    positions: &[VisitedPos],
    region_events: &[RegionEvent],
    ancestor: &dyn Fn(usize, usize) -> bool,
) -> Result<(), NavError> {
    for pair in positions.windows(2) {
        let from = pair[0].pos;
        let to = pair[1].pos;
        if pair[1].transport_before {
            continue; // an inter-area teleport hop: the player is moved, not walking
        }
        let st = world.walked_leg_region_state(
            region_events,
            ancestor,
            pair[0].pos,
            pair[0].src_step,
            pair[1].src_step,
        );
        let leg_world_owned;
        let leg_world: &World = if st.is_empty() {
            world
        } else {
            leg_world_owned = world.with_region_state(&st);
            &leg_world_owned
        };
        // The unforced counterfactual is built HERE, while `st` is still whole: the
        // world as it would be if every fill laid by a skippable beat were credited
        // as ordinary floor, which is precisely the model this compiler ran before
        // forcedness reached the geometry. Built only for a leg that has such a fill
        // at all — every other campaign routes over the identical single world and
        // pays nothing. Its blame ledger is taken by value for the same reason.
        let unforced_regions = st.unforced_regions.clone();
        let has_unforced = !st.unforced.is_empty();
        let credited_owned;
        let credited: Option<&World> = if st.unforced.is_empty() {
            None
        } else {
            credited_owned = world.with_region_state(&st.as_if_forced());
            Some(&credited_owned)
        };
        let sealed = st.solid;
        let flooded = st.flooded;
        // The lethal-free view of this same leg, built once and only when the
        // campaign declares a volume at all. Every failure below asks it first:
        // a leg that routes here and nowhere else failed *because of* lethality,
        // and saying which volume is a different fix from every other answer
        // this function gives.
        let open_owned;
        let open: Option<&World> = if leg_world.has_lethal() {
            open_owned = leg_world.without_lethal();
            Some(&open_owned)
        } else {
            None
        };
        // The same leg as this compiler saw it before the world-load gate seals
        // were measured: every gate assumed open. Built once, and only for a
        // campaign whose world actually authors a gate shut — everyone else routes
        // over the identical single world and pays nothing.
        let ungated_owned;
        let ungated: Option<&World> =
            if !world.has_world_load_seals() || world.is_teleport_source(from) {
                None
            } else {
                let st2 = world.leg_region_state_without_world_load(
                    region_events,
                    ancestor,
                    pair[0].src_step,
                    pair[1].src_step,
                );
                ungated_owned = if st2.is_empty() {
                    world.with_region_state(&RegionState::default())
                } else {
                    world.with_region_state(&st2)
                };
                Some(&ungated_owned)
            };
        let lethal_snap_err = |at: [i32; 3], talk_to: bool| -> Option<NavError> {
            let open = open?;
            let cell = open.snap_endpoint(at, talk_to)?;
            let names = names_of(&leg_world.lethal_volumes_over(&[cell]));
            Some(NavError {
                code: DW_LETHAL_ON_CRITICAL_PATH,
                message: format!(
                    "critical path: the only footing within {SNAP_RADIUS} blocks of visited \
                     anchor {at:?} lies INSIDE lethal volume(s) {names} — a player who reaches \
                     this objective is killed by standing where the objective is. Move the \
                     volume off the anchor, shrink its `extent`, or move the objective; do NOT \
                     delete the volume to silence the proof."
                ),
            })
        };
        // The gate counterfactual, in the same shape and for the same reason as the
        // lethal one: an anchor whose only footing is inside a gate region the world
        // authors shut needs to be told WHICH door, not sent to look for a wedged
        // prefab that is fine.
        let gate_snap_err = |at: [i32; 3], talk_to: bool| -> Option<NavError> {
            let ungated = ungated?;
            let cell = ungated.snap_endpoint(at, talk_to)?;
            let blamed = gate_blame(
                &world.gate_seals_over(&[cell]),
                region_events,
                ancestor,
                pair[1].src_step,
            );
            Some(NavError {
                code: DW_GATE_NEVER_OPENED,
                message: format!(
                    "critical path: the only footing within {SNAP_RADIUS} blocks of visited \
                     anchor {at:?} is inside a gate region the placed world authors SHUT at \
                     world-load — {blamed}. The party can never stand where this objective is. \
                     Fire `open-gate` on that anchor from an objective the party is forced to \
                     complete before this one, or move the objective off the gate; do NOT delete \
                     the gate, and do NOT remove the anchor's fill block from the prefab to \
                     silence the proof."
                ),
            })
        };

        // The same move for the other premise that closes a leg while the geometry
        // reads open: a runtime fill of water or lava. Built once, and only for a
        // campaign that fills a region with a fluid at all — every other campaign
        // pays nothing and routes byte-identically.
        let dry_owned;
        let dry: Option<&World> = if leg_world.has_runtime_flood() {
            dry_owned = leg_world.without_runtime_flood();
            Some(&dry_owned)
        } else {
            None
        };
        let fluid_snap_err = |at: [i32; 3], talk_to: bool| -> Option<NavError> {
            let dry = dry?;
            let cell = dry.snap_endpoint(at, talk_to)?;
            let boxes = boxes_of(&leg_world.flood_regions_over(&[cell]));
            Some(NavError {
                code: DW_FLUID_FILL_ON_CRITICAL_PATH,
                message: format!(
                    "critical path: the only footing within {SNAP_RADIUS} blocks of visited \
                     anchor {at:?} is a cell a runtime region write fills with FLUID {boxes} — \
                     a body does not stand on water or lava, so the party arrives at this \
                     objective and sinks. Fill the box with a block that is floor, put the \
                     floor one cell below the fluid, or move the objective; do NOT silence \
                     this by filling with a solid you do not want in the world."
                ),
            })
        };
        // And the same blame move for the premise this family was missing: a solid
        // laid by a beat nobody has to play.
        let unforced_snap_err = |at: [i32; 3], talk_to: bool| -> Option<NavError> {
            let credited = credited?;
            let cell = credited.snap_endpoint(at, talk_to)?;
            let boxes = unforced_blame_over(&unforced_regions, &[cell]).join("; ");
            Some(NavError {
                code: DW_UNFORCED_FOOTING,
                message: format!(
                    "critical path: the only footing within {SNAP_RADIUS} blocks of visited \
                     anchor {at:?} is laid at runtime by a beat the party is NOT forced to play \
                     — {boxes}. A party that never plays that beat arrives at this objective \
                     and finds no ground to stand on. Fire the fill from an objective the party \
                     is FORCED to complete before this one, put the floor in the prefab, or move \
                     the objective; do NOT keep the beat optional and hope."
                ),
            })
        };
        let start = match leg_world.snap_endpoint(from, false) {
            Some(c) => c,
            None => {
                if let Some(e) = lethal_snap_err(from, false) {
                    return Err(e);
                }
                if let Some(e) = fluid_snap_err(from, false) {
                    return Err(e);
                }
                if let Some(e) = unforced_snap_err(from, false) {
                    return Err(e);
                }
                if let Some(e) = gate_snap_err(from, false) {
                    return Err(e);
                }
                return Err(NavError {
                    code: DW_CRITICAL_UNROUTABLE,
                    message: format!(
                        "critical path: no standable floor within {SNAP_RADIUS} blocks of visited \
                         anchor {from:?} — a player-visited anchor sits walled in or over void. \
                         Fix the prefab so this anchor sits on/next to reachable floor; if the \
                         prefab looks correct, this is an assembly/toolchain defect — escalate \
                         rather than move the anchor into a wall"
                    ),
                });
            }
        };
        let goal = match leg_world.snap_endpoint(to, pair[1].talk_to) {
            Some(c) => c,
            None => {
                if let Some(e) = lethal_snap_err(to, pair[1].talk_to) {
                    return Err(e);
                }
                if let Some(e) = fluid_snap_err(to, pair[1].talk_to) {
                    return Err(e);
                }
                if let Some(e) = unforced_snap_err(to, pair[1].talk_to) {
                    return Err(e);
                }
                if let Some(e) = gate_snap_err(to, pair[1].talk_to) {
                    return Err(e);
                }
                return Err(NavError {
                    code: DW_CRITICAL_UNROUTABLE,
                    message: format!(
                        "critical path: no standable floor within {SNAP_RADIUS} blocks of visited \
                         anchor {to:?} — a player-visited anchor sits walled in or over void (a \
                         talk-to NPC needs a dry standable cell beside it, within interaction \
                         range and clear of water). Fix the prefab so this anchor sits on/next to \
                         reachable floor; if the prefab looks correct, this is an \
                         assembly/toolchain defect — escalate rather than move it into a wall"
                    ),
                });
            }
        };
        if leg_world.find_path(start, goal).is_none() {
            // Lethality first: it is the strictly more specific answer, and the
            // generic one below would send the author to fix open geometry.
            if let Some(open) = open
                && let (Some(s2), Some(g2)) = (
                    open.snap_endpoint(from, false),
                    open.snap_endpoint(to, pair[1].talk_to),
                )
                && let Some(cells) = open.find_path(s2, g2)
            {
                let names = names_of(&leg_world.lethal_volumes_over(&cells));
                return Err(NavError {
                    code: DW_LETHAL_ON_CRITICAL_PATH,
                    message: format!(
                        "critical path: the only route from {from:?} (floor {start:?}) to \
                         {to:?} (floor {goal:?}) runs THROUGH lethal volume(s) {names} — the \
                         party cannot reach this objective without dying on the way. The \
                         geometry is walkable; the volume is what closes it. Move or shrink the \
                         volume, or give the party a route around it; do NOT delete the volume \
                         to silence the proof."
                    ),
                });
            }
            // Then the fluid fill, for the same reason and with the same shape: the
            // route the author believes in exists, and what closed it is a box they
            // filled with water or lava. Asked after lethality only because a
            // campaign that has both wants the answer that kills the party told
            // first; the two are otherwise independent.
            if let Some(dry) = dry
                && let (Some(s2), Some(g2)) = (
                    dry.snap_endpoint(from, false),
                    dry.snap_endpoint(to, pair[1].talk_to),
                )
                && let Some(cells) = dry.find_path(s2, g2)
            {
                let boxes = boxes_of(&leg_world.flood_regions_over(&cells));
                return Err(NavError {
                    code: DW_FLUID_FILL_ON_CRITICAL_PATH,
                    message: format!(
                        "critical path: the only route from {from:?} (floor {start:?}) to \
                         {to:?} (floor {goal:?}) needs footing inside FLUID-filled region(s) \
                         {boxes} — a runtime write fills that box with water or lava, and a \
                         body walks on neither. The geometry would carry the party if the box \
                         held a block; the fluid is what closes it. Fill with a block that is \
                         floor, drop the walkable surface to the cell below the fluid, or route \
                         the forced path around the box; do NOT swap in a solid you do not want \
                         in the world just to get green."
                    ),
                });
            }
            // Then the unforced fill, for the same reason and in the same shape: the
            // route the author believes in exists, and what closed it is that its
            // floor is laid by a beat the party can walk past. Asked before the gate
            // counterfactual because it is the more specific answer — a gate the
            // campaign never opens is about a door, this is about who has to press
            // it — and because an unforced fill over a gate region would otherwise
            // be reported as a missing `open-gate` the author has already written.
            if let Some(credited) = credited
                && let (Some(s2), Some(g2)) = (
                    credited.snap_endpoint(from, false),
                    credited.snap_endpoint(to, pair[1].talk_to),
                )
                && let Some(cells) = credited.find_path(s2, g2)
            {
                let boxes = unforced_blame_over(&unforced_regions, &cells).join("; ");
                return Err(NavError {
                    code: DW_UNFORCED_FOOTING,
                    message: format!(
                        "critical path: the only route from {from:?} (floor {start:?}) to \
                         {to:?} (floor {goal:?}) needs footing laid at runtime by a beat the \
                         party is NOT forced to play — {boxes}. The geometry would carry the \
                         party if that beat always fired; it does not, so a party that skips it \
                         is stranded here. The fill still SEALS for the proof — an unforced \
                         write may make a region impassable — it just may not be stood on. \
                         Move the fill onto an objective the party is FORCED to complete before \
                         this leg, build the floor into the prefab, or route the forced path \
                         around the box; do NOT leave the path depending on a beat that can be \
                         skipped."
                    ),
                });
            }
            // Then the gate counterfactual: the identical world as it would be if
            // every gate the placed prefabs author shut had been born open — which
            // is precisely the model this compiler used to ship. A leg that routes
            // there and nowhere else failed *because of a door*, and the repair is
            // a missing `open-gate` on a name, not a hunt through the geometry.
            if let Some(ungated) = ungated
                && let (Some(s2), Some(g2)) = (
                    ungated.snap_endpoint(from, false),
                    ungated.snap_endpoint(to, pair[1].talk_to),
                )
                && let Some(cells) = ungated.find_path(s2, g2)
            {
                let blamed = gate_blame(
                    &world.gate_seals_over(&cells),
                    region_events,
                    ancestor,
                    pair[1].src_step,
                );
                return Err(NavError {
                    code: DW_GATE_NEVER_OPENED,
                    message: format!(
                        "critical path: the only route from {from:?} (floor {start:?}) to \
                         {to:?} (floor {goal:?}) runs THROUGH a gate the placed world authors \
                         SHUT at world-load — {blamed}. The geometry is walkable and the prefab \
                         is right; the door is what closes it, and the party has no way to open \
                         it before they must be on the far side. Fire `open-gate` on that anchor \
                         from an objective the party is FORCED to complete before this leg, or \
                         route the forced path so it does not cross the gate. Do NOT delete the \
                         gate, and do NOT strip the anchor's fill block out of the prefab to \
                         silence the proof."
                    ),
                });
            }
            // A leg the fluid did not *uniquely* close is still a leg a fluid may
            // have walled: `DW0544` fires only when the box supplied FOOTING, and a
            // flood laid across a corridor blocks it whether or not it is wet. That
            // is an unroutable leg the campaign built on purpose, so it may not be
            // reported as a wedged doorway — the "go and fix a prefab that was never
            // wrong" answer this whole family exists to avoid.
            // An UNFORCED fill still walls the leg, and the author still has to be
            // told which write did it. It is asked before the generic seam answer
            // for exactly the reason every code in this family exists: `sealed` is
            // empty here only because the seal came from a beat nobody has to play,
            // and "wedged doorway seam" would send someone to fix a prefab that is
            // perfectly correct. The blocking half of an unforced write is credited
            // in full — it is only the footing half that is withheld.
            let gate_hint = if sealed.is_empty() && has_unforced {
                "a runtime fill fired from a beat the party is NOT forced to play — a `close-gate` \
                 or `fill-region` in a trap payload, a shop offer, a death bundle or a shortcut's \
                 far side — has walled a region on/before this leg. An unforced write still SEALS \
                 for the proof (the delve must survive it) even though the footing it would lay is \
                 not credited (`DW0546`), so this wall is real. Move the fill off the forced path, \
                 fire it later, or clear it before this leg — do NOT delete the proof."
            } else if !flooded.is_empty() && sealed.is_empty() {
                "a runtime region write has filled a box with FLUID on/before this leg, and it \
                 blocks the forced path. Water and lava are impassable to a walker, so a filled \
                 box is a wall whatever its block. Move the box off the forced path, fire the \
                 fill later, or clear it before this leg — do NOT delete the proof."
            } else if sealed.is_empty() {
                "this is a wedged doorway seam, a void gap in the assembled layout, or an \
                 unbroken 1.5-tall barrier (fence/wall) ring — a walking player can neither pass \
                 through nor stand on top of a fence, so a pen needs a fence-gate opening (or, if \
                 the jump is intended, a missing inter-area transport)."
            } else {
                "a `close-gate` has sealed a gate region on/before this leg (a point of no \
                 return), and the forced path must re-cross it. Reopen it with `open-gate` \
                 before this leg, route the forced path so it does not re-cross the sealed gate, \
                 or fire the `close-gate` later — do NOT delete the proof."
            };
            return Err(NavError {
                code: DW_CRITICAL_UNROUTABLE,
                message: format!(
                    "critical path: the player cannot walk from {from:?} (floor {start:?}) to \
                     {to:?} (floor {goal:?}) over the assembled geometry — no collision-free \
                     path. A same-area leg must be walkable end to end; {gate_hint}"
                ),
            });
        }
    }
    Ok(())
}

/// Prove no `set-checkpoint` strands the party (DSL v0.6, spec-0012). Two
/// obligations, per checkpoint:
///
/// 1. **Placement** ([`DW_CHECKPOINT_UNSTANDABLE`], `DW0316`): the checkpoint
///    anchor must have a standable floor cell within [`SNAP_RADIUS`] on the final
///    assembled model, or the party respawns into void / a wall. (Because the
///    relight pass — which runs before nav — proves every reachable walkable cell
///    meets the area's `min_light`, a standable, reachable checkpoint cell
///    provably meets `min_light` too; no separate light probe is needed.)
/// 2. **No stranding** ([`DW_CHECKPOINT_STRANDED`], `DW0315`, the core proof):
///    the DW0311 reachability, re-rooted at the checkpoint cell, must still reach
///    the remaining critical path. Since consecutive walked legs are already
///    proven forward-walkable, it suffices to reach the FIRST walked critical
///    position that fires after the checkpoint — reconnecting the whole forward
///    path. The message names the checkpoint and that first unreachable anchor,
///    and prescribes moving the checkpoint or adding a return route (never
///    deleting the checkpoint to silence the proof).
pub fn check_checkpoints(plan: &Plan, world: &World) -> Result<(), NavError> {
    let cps: Vec<(String, [i32; 3], usize)> = plan
        .checkpoints
        .iter()
        .map(|c| (c.anchor.clone(), c.pos, c.fire_step))
        .collect();
    verify_checkpoints(
        world,
        &cps,
        &critical_positions(plan),
        &plan.region_events,
        &|g, s| plan.gate_fired_before(g, s),
    )
}

/// The pure core of [`check_checkpoints`] (split out so it is unit-testable
/// against a synthetic [`World`] without a full [`Plan`]). Each checkpoint is
/// `(anchor, cell, fire_step)`.
fn verify_checkpoints(
    world: &World,
    checkpoints: &[(String, [i32; 3], usize)],
    positions: &[VisitedPos],
    region_events: &[RegionEvent],
    ancestor: &dyn Fn(usize, usize) -> bool,
) -> Result<(), NavError> {
    for (anchor, pos, fire_step) in checkpoints {
        let Some(cell) = world.snap_standable(*pos, SNAP_RADIUS) else {
            return Err(NavError {
                code: DW_CHECKPOINT_UNSTANDABLE,
                message: format!(
                    "checkpoint anchor `{anchor}` at {pos:?} has no standable floor within \
                     {SNAP_RADIUS} blocks on the assembled model — the party would respawn into \
                     void or a wall. Move the checkpoint onto reachable floor (not a trap-trigger, \
                     hazard, or mid-air cell); if the prefab looks correct, this is an \
                     assembly/toolchain defect — escalate rather than hide it."
                ),
            });
        };
        // The first walked critical position strictly after the checkpoint fires.
        let Some(target) = positions
            .iter()
            .filter(|p| p.src_step > *fire_step && !p.transport_before)
            .min_by_key(|p| p.src_step)
        else {
            continue; // nothing left to walk to (checkpoint at/near the finale)
        };
        // Seal any gate closed by the time the party reaches the target (the same
        // per-leg gate state DW0311 routes under), so a checkpoint whose forward
        // path is walled off by a `close-gate` strands the party (DSL v0.6).
        let st = world.region_state_at(region_events, target.src_step, ancestor);
        let leg_world_owned;
        let leg_world: &World = if st.is_empty() {
            world
        } else {
            leg_world_owned = world.with_region_state(&st);
            &leg_world_owned
        };
        let Some(goal) = leg_world.snap_endpoint(target.pos, target.talk_to) else {
            continue; // the target itself is unsnappable → a DW0311 concern, not ours
        };
        if leg_world.find_path(cell, goal).is_none() {
            return Err(NavError {
                code: DW_CHECKPOINT_STRANDED,
                message: format!(
                    "checkpoint `{anchor}` (cell {cell:?}) strands the party: the next required \
                     anchor {:?} is not walkable from it over the assembled geometry (a checkpoint \
                     behind a one-way drop the forward path can't re-cross after respawn). Move the \
                     checkpoint to a cell that keeps the remaining path reachable, or add a return \
                     route back up — do NOT delete the checkpoint to silence this proof.",
                    target.pos
                ),
            });
        }
    }
    Ok(())
}

/// The minimum share of a `timed-gate` cycle that must admit a crossing
/// (spec-0016 §4). Below this the gate stops being a
/// timing read and becomes a coin flip. Expressed as a percentage so the
/// arithmetic below stays in integers — no float rounding in a proof (ADR-0006).
const TIMED_GATE_MIN_ADMIT_PERCENT: u32 = 20;

/// Prove every `timed-gate` is readable — [`DW_TIMED_GATE_COIN_FLIP`] (`DW0378`).
///
/// The requirement is deliberately **not** all-phase passability: spec-0016 §4 is
/// explicit that a gate which punishes bad timing is the entire point. What must
/// hold is that the gate can be *read*: over one full cycle, the entry phases from
/// which a walking player clears the span before it shuts must cover at least
/// [`TIMED_GATE_MIN_ADMIT_PERCENT`] of the cycle.
///
/// The crossing cost comes from the same nav model every other proof uses: the A*
/// step count from the footing on one side of the gate region to the footing on
/// the other with the gate open, charged at [`SPRINT_TICKS_PER_BLOCK`]. A player
/// who starts the crossing `p` ticks into the open window arrives in time iff
/// `p + cross <= open_ticks`, so the admitting window is
/// `max(0, open_ticks - cross + 1)` ticks out of `open_ticks + closed_ticks`.
///
/// A gate whose two sides have no walkable footing, or that no route connects even
/// while open, is left to the geometry proofs that own it (`DW0311`) rather than
/// double-reported here.
pub fn check_timed_gates(plan: &Plan, world: &World) -> Result<(), NavError> {
    verify_timed_gates(world, &plan.timed_gates)
}

/// Prove every `timed-gate` `disarm` affordance can be reached before the gate is
/// crossed — [`DW_TIMED_GATE_DISARM_UNREACHABLE`] (`DW0393`).
///
/// One clause, the same one `DW0373` puts on a shortcut's unlock: the `via` cell
/// must be walkable from the campaign entry over the world with the gate span
/// **SEALED**. Searching the open world would "prove" a lever whose only approach
/// is through the portcullis — precisely the fake third rung this exists to
/// refuse.
///
/// Vacuous where another proof owns the ground: no entry (`DW0345`), an
/// unstandable entry or `via` cell (the anchor checks), a gate with no `disarm`.
pub fn check_timed_gate_disarms(
    plan: &Plan,
    world: &World,
    entry: Option<[i32; 3]>,
) -> Result<(), NavError> {
    verify_timed_gate_disarms(world, &plan.timed_gates, entry)
}

/// The pure core of [`check_timed_gate_disarms`] (unit-testable against a
/// synthetic [`World`]).
fn verify_timed_gate_disarms(
    world: &World,
    gates: &[crate::plan::TimedGatePlan],
    entry: Option<[i32; 3]>,
) -> Result<(), NavError> {
    let Some(entry) = entry else {
        return Ok(());
    };
    for g in gates {
        let Some(dis) = &g.disarm else {
            continue;
        };
        let cells: BTreeSet<[i32; 3]> =
            crate::assembled::region_cells(g.gate_region.0, g.gate_region.1).collect();
        let sealed = world.with_sealed(&cells);
        let start = sealed.snap_standable(entry, SNAP_RADIUS);
        let goal = sealed.snap_standable(dis.via_cell, SNAP_RADIUS);
        let (Some(start), Some(goal)) = (start, goal) else {
            continue; // an unstandable entry or lever cell is another proof's concern
        };
        if sealed.find_path(start, goal).is_some() {
            continue;
        }
        return Err(NavError {
            code: DW_TIMED_GATE_DISARM_UNREACHABLE,
            message: format!(
                "timed gate `{}`: its disarm affordance at `{}` ({:?}) is not walkable from the \
                 campaign entry while gate `{}` is closed, so the only way to the jam lever is \
                 THROUGH the portcullis. A disarm the party can reach only by first surviving the \
                 hazard disables nothing — it is a trophy for having beaten it, not the third rung \
                 of the ladder (souls dossier §5.2: readable, avoidable, disable-able). \
                 Put the lever on ground the approach already touches — the stair head above the \
                 run, the alcove beside the doorway — or drop the `disarm` and let the clock \
                 stand. Do NOT leave the gate open at world-load to silence this.",
                g.id, dis.via_anchor, dis.via_cell, g.gate_anchor
            ),
        });
    }
    Ok(())
}

/// The pure core of [`check_timed_gates`] (unit-testable against a synthetic
/// [`World`]).
fn verify_timed_gates(world: &World, gates: &[crate::plan::TimedGatePlan]) -> Result<(), NavError> {
    for g in gates {
        let cells: BTreeSet<[i32; 3]> =
            crate::assembled::region_cells(g.gate_region.0, g.gate_region.1).collect();
        let Some((near, far)) = gate_crossing_footings(world, g.gate_region, &cells) else {
            continue; // no footing on both sides — a geometry concern, not a timing one
        };
        let Some(path) = world.find_path(near, far) else {
            continue; // the open gate connects nothing — DW0311's business
        };
        // `path` includes both endpoints; the crossing is the moves between them.
        let cross_ticks = (path.len().saturating_sub(1) as u32) * SPRINT_TICKS_PER_BLOCK;
        let cycle = g.open_ticks + g.closed_ticks;
        let admits =
            g.open_ticks.saturating_sub(cross_ticks) + u32::from(cross_ticks <= g.open_ticks);
        // Integer percentage, rounded DOWN — the proof never credits the gate with
        // a share it does not have.
        let percent = admits.saturating_mul(100) / cycle.max(1);
        if percent < TIMED_GATE_MIN_ADMIT_PERCENT {
            return Err(NavError {
                code: DW_TIMED_GATE_COIN_FLIP,
                message: format!(
                    "timed gate `{}` is a coin flip, not a timing read: crossing its span takes \
                     {cross_ticks} ticks ({} blocks at {SPRINT_TICKS_PER_BLOCK} t/block), so only \
                     {admits} of its {cycle}-tick cycle ({percent}%) admit a player who starts \
                     walking then — under the {TIMED_GATE_MIN_ADMIT_PERCENT}% floor \
                     (spec-0016 §4). Punishing bad timing is the point; punishing EVERY \
                     timing is a slot machine. Lengthen `open_ticks`, shorten `closed_ticks`, or \
                     narrow the span — never lower the floor.",
                    g.id,
                    path.len().saturating_sub(1)
                ),
            });
        }
    }
    Ok(())
}

/// The standable footings immediately on either side of a gate region, over the
/// world with the region SEALED so neither endpoint can land inside it or snap
/// through. The crossing axis is whichever horizontal axis actually has footing on
/// both sides — trying x then z rather than guessing from the region's extents is
/// both deterministic and correct for a square 1×1 gate column, where the extents
/// tie and a guess would pick the wall's own plane.
fn gate_crossing_footings(
    world: &World,
    region: ([i32; 3], [i32; 3]),
    cells: &BTreeSet<[i32; 3]>,
) -> Option<([i32; 3], [i32; 3])> {
    let sealed = world.with_sealed(cells);
    let (from, to) = region;
    for axis in [0usize, 2] {
        let mut near = None;
        let mut far = None;
        for cell in crate::assembled::region_cells(from, to) {
            for (slot, delta) in [(&mut near, -1), (&mut far, 1)] {
                if slot.is_some() {
                    continue;
                }
                let mut c = cell;
                c[axis] += delta;
                if !cells.contains(&c) && sealed.standable(c) {
                    *slot = Some(c);
                }
            }
            if near.is_some() && far.is_some() {
                break;
            }
        }
        if let (Some(n), Some(f)) = (near, far) {
            return Some((n, f));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Hazard observability (spec-0016 §4 addendum, souls dossier §5.3 / §2.2 axis 5)
// ---------------------------------------------------------------------------

/// A player's eye height above the floor of the cell they stand in, in blocks —
/// the vanilla 1.21.11 standing eye offset, the same figure the player-POV camera
/// derivation uses (`DW0724`). The observability sightline starts here because
/// the question the proof asks is literally "can a player standing there see it".
const EYE_HEIGHT: f64 = 1.62;

/// The minimum distance a watch cell must keep from every cell of a hazard's
/// lethal span, in blocks (Chebyshev, box distance).
///
/// Derived rather than invented: it is **one second of sprinting** at the nav
/// model's own speed (`20 / SPRINT_TICKS_PER_BLOCK`), so the proof demands a
/// sightline from ground the player reaches a full second before the hazard could
/// have them. Sight from the very lip of the span is not observation from safety —
/// it is already the commitment. (The bell remake's portcullis bay sits six blocks
/// out, comfortably clear of this floor.)
const HAZARD_STANDOFF: i32 = (20 / SPRINT_TICKS_PER_BLOCK) as i32;

/// How far from a hazard the proof will look for a watch cell, in blocks
/// (Chebyshev, box distance). Two chunks: a bay further out than this is not a
/// bay, and the bound keeps the search over a box-garden world small and its cost
/// independent of how large the reachable region happens to be.
const HAZARD_WATCH_RANGE: i32 = 32;

/// One hazard the observability proof judges: a region whose contents become
/// lethal on a **clock the player is expected to read**.
///
/// Exactly two verbs qualify today. A `timed-gate` cycles open/closed forever; a
/// `volley` rakes its kill zone for `salvos × interval` ticks. Both ask the player
/// to time an entry, and both are therefore owed a sightline first. `collapse` is
/// deliberately NOT here: it fires once, its region is a ceiling with no standable
/// cell, and there is no cycle to watch — its fairness obligation is the
/// post-collapse completability proof (`DW0445`), not observability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimedHazard {
    /// How the diagnostic names it (`timed-gate/portcullis`, `volley into
    /// `anchor/stair-run``).
    pub id: String,
    /// The lethal span's inclusive corners, in absolute world coordinates.
    pub region: ([i32; 3], [i32; 3]),
}

/// Every timed hazard in the campaign, in deterministic content order:
/// `timed_gates[]` in declared order, then each distinct `volley` kill zone in
/// effect-traversal order. A volley declared twice with the same gallery slot and
/// the same zone is one hazard (the emitter dedupes it the same way).
pub fn timed_hazards(plan: &Plan) -> Vec<TimedHazard> {
    let mut out: Vec<TimedHazard> = plan
        .timed_gates
        .iter()
        .map(|g| TimedHazard {
            id: g.id.clone(),
            region: g.gate_region,
        })
        .collect();
    let mut seen: BTreeSet<([i32; 3], [i32; 3])> = BTreeSet::new();
    for eff in all_effects(plan) {
        let Some((_, from_anchor, kill_zone, _, _)) = eff.volley() else {
            continue;
        };
        let Some(region) = plan.zone_box(kill_zone) else {
            continue; // an unresolvable anchor is the payload planner's error
        };
        if !seen.insert(region) {
            continue;
        }
        out.push(TimedHazard {
            id: format!(
                "volley from `{from_anchor}` into `{}`",
                kill_zone.anchor.as_str()
            ),
            region,
        });
    }
    out
}

/// Prove every timed hazard can be **watched before it is committed to** —
/// [`DW_HAZARD_UNOBSERVABLE`] (`DW0388`).
///
/// For each hazard the proof asks for one **watch cell** `w` with all three of:
///
/// 1. **Outside the lethal span, by a margin.** `w` is standable and at least
///    [`HAZARD_STANDOFF`] blocks (box distance) from every cell of the span — one
///    second of sprint at the nav model's own speed. Sight from the lip of the
///    span is not observation from safety.
/// 2. **Line of sight.** The segment from `w`'s eye ([`EYE_HEIGHT`] above its
///    floor) to the player-centre-mass point of some standable hazard cell
///    ([`volley_target`], 1.0 above that cell's floor — the exact point a volley
///    aims at, so "the dangerous point of a hazard cell" has one definition in the
///    compiler) crosses no sight-blocking geometry. The predicate is the cutscene
///    clip check's `blocks_camera` walked by the same Amanatides–Woo
///    [`walk_cells`] traversal, so glass and grates are transparent to an eye
///    exactly as they are to a camera — a bay behind a grate is a bay.
/// 3. **Reached without committing.** `w` is walkable from the campaign entry over
///    the world with the span **sealed**. This is the load-bearing clause: it is
///    what makes the cell a *watch* cell rather than a cell you can only reach by
///    first surviving the hazard.
///
/// Tiering (spec-0016 §4 addendum): **error** for a campaign that declares a
/// `bonfire` — a souls campaign, where observe-before-commit is the fairness
/// contract the whole loop rests on — and **warning** everywhere else, where the
/// same geometry is a design note rather than a broken promise.
///
/// Hazards whose region holds no standable cell, and campaigns with no resolvable
/// entry, are left to the proofs that own them (`DW0444`, `DW0311`, `DW0345`)
/// rather than double-reported here.
pub fn check_hazard_observability(
    plan: &Plan,
    world: &World,
    entry: Option<[i32; 3]>,
) -> Result<Vec<Diagnostic>, NavError> {
    let hazards = timed_hazards(plan);
    let findings = verify_hazard_observability(world, &hazards, entry);
    // A campaign that places a bonfire IS a souls campaign — the same test the
    // flask obligation (`DW0476`) uses, so one campaign never sits on two
    // different answers to "is this spec-0016 content".
    hazard_tier(plan.bonfires().next().is_some(), findings)
}

/// Apply the spec-0016 §4-addendum tiering to the observability findings: a souls
/// campaign fails the build on the first one, anything else carries all of them as
/// advisory warnings. Split out from [`check_hazard_observability`] so the tier
/// rule itself is unit-testable without standing up a whole [`Plan`].
fn hazard_tier(souls: bool, findings: Vec<Diagnostic>) -> Result<Vec<Diagnostic>, NavError> {
    if !souls {
        return Ok(findings);
    }
    match findings.into_iter().next() {
        Some(d) => Err(NavError {
            code: DW_HAZARD_UNOBSERVABLE,
            message: d.message,
        }),
        None => Ok(Vec::new()),
    }
}

/// The pure core of [`check_hazard_observability`] (unit-testable against a
/// synthetic [`World`]). Reports at the advisory tier; [`hazard_tier`] decides how
/// loud that is for the campaign at hand.
fn verify_hazard_observability(
    world: &World,
    hazards: &[TimedHazard],
    entry: Option<[i32; 3]>,
) -> Vec<Diagnostic> {
    let Some(entry) = entry else {
        return Vec::new(); // DW0345 owns a campaign with no entry anchor
    };
    let mut out = Vec::new();
    for h in hazards {
        let span: BTreeSet<[i32; 3]> =
            crate::assembled::region_cells(h.region.0, h.region.1).collect();
        // What the player would be standing on inside the hazard — the cells the
        // clock actually judges, and so the cells a watcher must be able to see.
        let samples: Vec<[i32; 3]> = span
            .iter()
            .copied()
            .filter(|c| world.standable(*c))
            .collect();
        if samples.is_empty() {
            continue; // an unusable region is DW0444 / DW0311's business
        }
        // Pre-commitment ground: everywhere the player can walk from the entry
        // WITHOUT entering the span.
        let sealed = world.with_sealed(&span);
        let pre_commit = sealed.reachable_walkable(&[entry]);
        // Nearest candidate first — a real watch bay is close, so the passing case
        // costs a handful of sightlines. Ties break on cell order (ADR-0006).
        let mut candidates: Vec<(i32, [i32; 3])> = pre_commit
            .into_iter()
            .map(|c| (box_distance(c, h.region), c))
            .filter(|(d, _)| (HAZARD_STANDOFF..=HAZARD_WATCH_RANGE).contains(d))
            .collect();
        candidates.sort_unstable();
        let watch = candidates
            .into_iter()
            .find(|(_, c)| samples.iter().any(|s| sees_hazard_cell(world, *c, *s)));
        if watch.is_some() {
            continue;
        }
        out.push(Diagnostic::warning(
            DW_HAZARD_UNOBSERVABLE,
            "quests",
            format!("/content/quests/hazard/{}", h.id),
            format!(
                "hazard `{}` cannot be watched before it is committed to: no standable cell \
                 within {HAZARD_WATCH_RANGE} blocks of its span [{}, {}, {}]..[{}, {}, {}] is \
                 both at least {HAZARD_STANDOFF} blocks clear of it (one second of sprint at \
                 {SPRINT_TICKS_PER_BLOCK} t/block) and walkable from the campaign entry without \
                 entering it, with line of sight to any cell the hazard judges. The strongest \
                 rule in the souls vocabulary is observe-from-safety-before-commit (spec-0016 §4 \
                 addendum): you can stand outside Sen's Fortress and watch a blade cycle, and you \
                 cannot see inside the Capra room — a timed hazard you meet blind is a coin flip \
                 no repetition teaches, whatever its duty cycle. Give it a watch bay: open the \
                 approach so the span is visible from a few blocks back, or move the hazard off \
                 the blind side of the corner. Do NOT shorten the standoff.",
                h.id,
                h.region.0[0],
                h.region.0[1],
                h.region.0[2],
                h.region.1[0],
                h.region.1[1],
                h.region.1[2],
            ),
        ));
    }
    out
}

/// Chebyshev distance from a cell to a box, in blocks — `0` inside the box.
/// Integer arithmetic end to end: a proof never rounds (ADR-0006).
fn box_distance(c: [i32; 3], region: ([i32; 3], [i32; 3])) -> i32 {
    let (lo, hi) = region;
    (0..3)
        .map(|i| {
            let (a, b) = (lo[i].min(hi[i]), lo[i].max(hi[i]));
            (a - c[i]).max(c[i] - b).max(0)
        })
        .max()
        .unwrap_or(0)
}

/// Whether a player standing on `watch` can see the space a player standing on
/// `hazard` would occupy: the segment from eye height over one to centre mass over
/// the other, walked cell by cell through the **sight** predicate. Both endpoint
/// cells are exempt — they are the observer's own head and the target volume, both
/// standable and so both passable by construction.
fn sees_hazard_cell(world: &World, watch: [i32; 3], hazard: [i32; 3]) -> bool {
    let eye = [
        watch[0] as f64 + 0.5,
        watch[1] as f64 + EYE_HEIGHT,
        watch[2] as f64 + 0.5,
    ];
    let target = volley_target(hazard);
    let eye_cell = [watch[0], watch[1] + 1, watch[2]];
    walk_cells(eye, target, |c| {
        c != eye_cell && c != hazard && world.blocks_camera(c)
    })
    .is_none()
}

/// Prove every `ambush` (spec-0016 §3) leaves the player a play —
/// [`DW_AMBUSH_NO_COUNTERPLAY`] (`DW0376`).
///
/// The obligation is *not* "warn the player". Spec-0016 is explicit that the
/// un-telegraphed ambush — 初见杀, the shove off the cliff you could not have
/// known about — is legitimate and essential: you die uninformed once, and the
/// SECOND attempt is where the design pays off. Determinism already guarantees
/// that second attempt meets the same ambushers in the same cells.
///
/// What the compiler adds is the half determinism cannot supply: that there is
/// something to *do* about them. Generalizing the trap-avoidability machinery
/// (`DW0342`'s "reachable with the hazard cell blocked"), this stands every
/// ambusher on the cell it will occupy and re-asks whether the trigger cell still
/// connects to any rest point — a checkpoint, a bonfire, or the campaign entry.
/// If it does, a retreat exists: luring ground, a positioning line, an exit. If it
/// does not, the player is sealed in a pocket with the ambush and the beat has no
/// second attempt to reward — that is a broken beat, not a hard one.
pub fn check_ambushes(plan: &Plan, world: &World, entry: Option<[i32; 3]>) -> Result<(), NavError> {
    let mut rests: Vec<[i32; 3]> = plan.checkpoints.iter().map(|c| c.pos).collect();
    rests.extend(entry);
    verify_ambushes(world, &plan.ambushes, &rests)
}

/// The pure core of [`check_ambushes`] (unit-testable against a synthetic
/// [`World`]). `rests` are the cells that count as safety — every checkpoint and
/// bonfire cell plus the campaign entry.
fn verify_ambushes(
    world: &World,
    ambushes: &[crate::plan::AmbushPlan],
    rests: &[[i32; 3]],
) -> Result<(), NavError> {
    if ambushes.is_empty() || rests.is_empty() {
        return Ok(());
    }
    for amb in ambushes {
        let blocked: BTreeSet<[i32; 3]> = amb
            .actor_cells
            .iter()
            .copied()
            .filter(|c| *c != amb.at)
            .collect();
        if blocked.is_empty() {
            continue; // nothing stands in the player's way
        }
        let occupied = world.with_sealed(&blocked);
        let Some(from) = occupied.snap_standable(amb.at, SNAP_RADIUS) else {
            continue; // an unstandable trigger cell is another proof's concern
        };
        let escapes = rests.iter().any(|r| {
            occupied
                .snap_standable(*r, SNAP_RADIUS)
                .is_some_and(|goal| occupied.find_path(from, goal).is_some())
        });
        if !escapes {
            return Err(NavError {
                code: DW_AMBUSH_NO_COUNTERPLAY,
                message: format!(
                    "ambush `{}` at {:?} leaves no counterplay: with its ambushers standing on \
                     {:?}, no checkpoint, bonfire or campaign entry is walkable from the trigger \
                     cell any more — the party is sealed in a pocket with the ambush and can only \
                     trade blows blind. An un-telegraphed ambush is fine (spec-0016 §3: dying \
                     uninformed once is how the level teaches); an ambush with no retreat, no \
                     luring ground and no exit is not, because the second attempt has nothing to \
                     reward. Widen the room, move an ambusher off the only way out, or add a rest \
                     point behind the player — do NOT delete the proof.",
                    amb.id, amb.at, amb.actor_cells
                ),
            });
        }
    }
    Ok(())
}

/// Prove every `shortcut` door (spec-0016 §2) is a real shortcut.
///
/// The base occupancy model treats every gate region as passable (the
/// "assume the gate the player needs is opened" stance), and `Plan::build`
/// registers each shortcut gate as sealed from step 0, so the critical path,
/// the checkpoints and the traps are all already proven **without** any shortcut
/// taken. What remains are the two obligations the pattern itself carries, both
/// measured against the same sealed world:
///
/// 1. [`DW_SHORTCUT_NO_LONG_ROUTE`] (`DW0373`) — with the gate SEALED, the
///    far-side `unlock` affordance must still be walkable from the campaign
///    entry. That walk IS the long route. Without it the mechanism that opens the
///    shortcut sits behind the shortcut, and the gate is dead scenery.
/// 2. [`DW_SHORTCUT_NO_GAIN`] (`DW0374`) — opening the gate must strictly shorten
///    that same walk. This is the anti-leak proof: it is what makes `unlock` a
///    FAR-side anchor rather than a label. An unlock on the near side of its own
///    gate measures identically sealed and open, and fails here.
///
/// Distances are A* step counts over the nav model — the same routing every other
/// completability proof uses, so the two numbers are directly comparable.
pub fn check_shortcuts(
    plan: &Plan,
    world: &World,
    entry: Option<[i32; 3]>,
) -> Result<(), NavError> {
    verify_shortcuts(world, &plan.shortcuts, entry)
}

/// The proven lane polyline of every wave that declares one (spec-0016 §6): wave
/// id → the snapped, walk-connected waypoint cells in march order. These cells —
/// not the raw anchor positions — are what the compiler writes into
/// `patrol_target`, so the squad is always sent somewhere it can actually stand.
pub type LaneRoutes = BTreeMap<String, Vec<[i32; 3]>>;

/// The minimum legal distance between consecutive lane waypoints, in blocks
/// (`DW0386`). Vanilla re-rolls a patrol target to a random point once the
/// patroller gets within 10 blocks of it, so a leg of 10 or less is a leg the
/// engine stops following; the spike's measured working default is 12.
const LANE_MIN_LEG: f64 = 10.0;

/// Resolve and prove every TD lane (spec-0016 §6), raising `DW0386` on the first
/// failure.
///
/// Four obligations per lane, in the order an author hits them:
/// 1. every waypoint anchor resolves in the wave's area;
/// 2. every waypoint has standable footing within [`SNAP_RADIUS`];
/// 3. every leg — including the one from the spawn anchor to the first waypoint —
///    is genuinely walkable by the squad;
/// 4. every leg is longer than [`LANE_MIN_LEG`].
///
/// Routed over the **no-gate-use** view, exactly like wave seating: lane mobs
/// cannot right-click a fence gate open, so a lane that "works" only by walking
/// through one does not work.
pub fn plan_lanes(plan: &Plan, world: &World) -> Result<LaneRoutes, NavError> {
    let entity_world_owned;
    let world: &World = if world.has_use_gates() {
        entity_world_owned = world.without_gate_use();
        &entity_world_owned
    } else {
        world
    };
    let c = plan.campaign;
    let mut out: LaneRoutes = BTreeMap::new();
    for w in &c.quests.content.waves {
        let Some(lane) = &w.lane else { continue };
        let area = crate::plan::wave_area(c, w.id.as_str());
        let anchor = area.and_then(|a| plan.point(a, w.anchor.as_str()));
        let (Some(area), Some(anchor)) = (area, anchor) else {
            // An unresolvable spawn anchor is DW0310's concern (the dangling
            // `spawn-wave`); do not double-report it here.
            continue;
        };
        let fail = |message: String| NavError {
            code: DW_LANE_GEOMETRY,
            message,
        };
        let mut cells: Vec<[i32; 3]> = Vec::new();
        let mut prev = world.snap_standable(anchor, SNAP_RADIUS).ok_or_else(|| {
            fail(format!(
                "lane wave `{}`: its spawn anchor `{}` ({anchor:?}) has no standable footing \
                 within {SNAP_RADIUS} blocks, so the squad has nowhere to form up before the \
                 march (spec-0016 §6)",
                w.id,
                w.anchor.as_str()
            ))
        })?;
        let mut prev_name = w.anchor.as_str().to_string();
        for wp in &lane.waypoints {
            let Some(pos) = plan.point(area, wp.as_str()) else {
                return Err(fail(format!(
                    "lane wave `{}`: waypoint anchor `{}` resolves to no position in area `{area}` \
                     (spec-0016 §6). A lane is a polyline of REAL places; use an anchor the area's \
                     assembled prefabs actually expose.",
                    w.id,
                    wp.as_str()
                )));
            };
            let cell = world.snap_standable(pos, SNAP_RADIUS).ok_or_else(|| {
                fail(format!(
                    "lane wave `{}`: waypoint `{}` ({pos:?}) has no standable footing within \
                     {SNAP_RADIUS} blocks (spec-0016 §6) — a patrol target the squad cannot stand \
                     on is a target it never arrives at, so the lane stalls there forever",
                    w.id,
                    wp.as_str()
                ))
            })?;
            if world.find_path(prev, cell).is_none() {
                return Err(fail(format!(
                    "lane wave `{}`: the leg `{prev_name}` ({prev:?}) → `{}` ({cell:?}) is not \
                     walkable (spec-0016 §6). The squad marches this polyline on foot with native \
                     pathfinding; a leg the compiler cannot walk is a leg the mobs cannot walk. \
                     Note lane mobs cannot open fence gates — the proof runs on the same \
                     no-gate-use view wave seating uses.",
                    w.id,
                    wp.as_str()
                )));
            }
            let leg = (0..3)
                .map(|i| f64::from(cell[i] - prev[i]).powi(2))
                .sum::<f64>()
                .sqrt();
            if leg <= LANE_MIN_LEG {
                return Err(fail(format!(
                    "lane wave `{}`: the leg `{prev_name}` ({prev:?}) → `{}` ({cell:?}) is {leg:.1} \
                     blocks, which is not more than {LANE_MIN_LEG:.0} (spec-0016 §6). Vanilla \
                     re-rolls a patrol target to a RANDOM point once the patroller is within 10 \
                     blocks of it, so a leg this short is one the engine quietly stops following \
                     and the squad wanders off-lane — it reads as working-but-drunk, not as a bug. \
                     Space lane waypoints at least 12 blocks apart (the spike's measured default), \
                     or drop this waypoint.",
                    w.id,
                    wp.as_str()
                )));
            }
            cells.push(cell);
            prev = cell;
            prev_name = wp.as_str().to_string();
        }
        out.insert(w.id.as_str().to_string(), cells);
    }
    Ok(out)
}

/// The measured off-lane drift of a marching TD squad, in blocks — the constraint
/// source is the td-routing-spike dossier (`docs/notes/td-routing-spike.md`,
/// "Lane fidelity": followers deviate mean ≤3.2, **max 7.9** blocks off the lane
/// polyline, 116 samples). A marching squad is a CORRIDOR around its polyline,
/// not a line: a placement can clear the centre-line by 2 blocks and still stand
/// inside the marching mobs' real aggro reach — run nine's live death at 17.7
/// blocks from a 16-`follow_range` lane. `DW0478`'s lane term is therefore
/// `follow_range + LANE_MARCH_DRIFT`; stationary
/// spawn/staging cells keep the plain `follow_range` term.
pub const LANE_MARCH_DRIFT: f64 = 7.9;

/// One hostile force as the bonfire safe-zone proof (`DW0478`) sees it: a
/// perception radius plus every cell the force provably occupies.
#[derive(Clone, Debug)]
pub struct AggroSource {
    /// What the message calls it (`wave/gate-assault`, `actor/barrow-warden`).
    pub id: String,
    /// The perception radius, in blocks — the declared `follow_range`, a lane's
    /// `aggro_radius` (which the compiler emits AS `follow_range`), or
    /// [`DEFAULT_FOLLOW_RANGE`].
    pub radius: f64,
    /// Why this radius is the number it is, for the message.
    pub radius_source: &'static str,
    /// Every occupied cell: what it is, where it is, and the extra reach margin
    /// added on top of `radius` — `0.0` for a stationary cell (seated spawn,
    /// staging anchor), [`LANE_MARCH_DRIFT`] for a lane path cell, because the
    /// squad marches a corridor around the polyline, not the polyline itself.
    pub cells: Vec<(&'static str, [i32; 3], f64)>,
}

/// `DW0478`: **no respawn point may sit inside any hostile's aggro range**
/// (spec-0016 §1).
///
/// A **respawn point** is every resolved [`crate::plan::CheckpointPlan`] — a
/// `bonfire` and a plain `set-checkpoint` alike. The proof is about the cell a
/// dead player materialises on, and vanilla returns them to both by the same
/// `spawnpoint` mechanism, so keying it to the `rest` flag examined one variant
/// of a sum type and silently skipped its sibling (see [`DW_RESPAWN_IN_AGGRO`]).
///
/// The rule, verbatim: for every wave / actor hostile, the distance from the
/// respawn cell to that hostile's spawn cell — or to any cell of its lane path —
/// must EXCEED that hostile's `follow_range` (the declared attribute; the
/// documented default when undeclared). For a **lane path cell** the term is
/// `follow_range + `[`LANE_MARCH_DRIFT`]: the squad
/// marches a measured corridor around the polyline, so the centre-line distance
/// understates its real aggro reach. Stationary cells keep the plain term.
///
/// What "occupies" means per force:
/// * a plain wave — its DW0312-proven seated spawn cells (where the datapack
///   actually summons it, not where its anchor is);
/// * an `aggro-edge` wave — the same, which for it is its perception ring;
/// * a **lane** wave — those cells PLUS the marched polyline: every cell of every
///   A*-proven leg from the form-up point through the waypoints, because a lane
///   wave's whole design is that it walks that corridor while the party is
///   elsewhere — and each of those cells carries the [`LANE_MARCH_DRIFT`]
///   margin, because the squad's measured march is a corridor around the
///   polyline, not the polyline. This is the shape that killed the drowned
///   bell's ladder bot: a re-seated gate squad marched its lane, and the lane
///   ended a couple of blocks outside a bonfire the party had just rested at;
/// * an **actor** the campaign declares as a fighter — `unleash-actor`ed
///   somewhere, or staged `vulnerable` — at its staging anchor. Fighter-ness is
///   read off the campaign's own declarations and never guessed from the species:
///   the pinned entity registry is a membership set with no mob-category data
///   (the same rule `DW0469` is built on), so the compiler cannot and does not
///   ask whether `minecraft:sheep` is a monster.
///
/// Radius per force: a lane's `aggro_radius` (emitted verbatim as each lane mob's
/// `follow_range`), else the largest declared `follow_range` among its mobs, else
/// [`DEFAULT_FOLLOW_RANGE`] — one documented number, never a per-species table the
/// compiler would have to invent (`DW0475`'s rule).
///
/// A campaign with no respawn point at all, or with no hostile force at all,
/// proves nothing here — and [`RespawnSafetyLedger`] says so out loud rather than
/// returning a silent `Ok`.
pub fn check_respawn_safe_zone(
    plan: &Plan,
    world: &World,
    placements: &BTreeMap<String, Vec<[i32; 3]>>,
    lanes: &LaneRoutes,
) -> Result<RespawnSafetyLedger, NavError> {
    let reign_ends = plan.respawn_reign_ends();
    let rest_points: Vec<RestPoint> = plan
        .checkpoints
        .iter()
        .zip(&reign_ends)
        .map(|(c, end)| RestPoint {
            anchor: c.anchor.clone(),
            kind: if c.rest { "bonfire" } else { "set-checkpoint" },
            pos: c.pos,
            reign_end: *end,
        })
        .collect();
    let onsets = plan.hostile_onsets();
    let sources = aggro_sources(plan, world, placements, lanes);
    let ledger = RespawnSafetyLedger::new(&rest_points, &sources, &onsets);
    if ledger.pairs == 0 {
        return Ok(ledger);
    }
    verify_respawn_safe_zone(&rest_points, &sources, &onsets)?;
    Ok(ledger)
}

/// Whether a hostile force can be in the world while a respawn point still
/// governs where a dead player lands.
///
/// Both halves are the campaign's own declarations: the force's onset is the
/// earliest beat that stages it ([`Plan::hostile_onsets`]), and the respawn
/// point's reign ends when a later `set-checkpoint` replaces it
/// ([`Plan::respawn_reign_ends`]). A bonfire never stops reigning, so every
/// bonfire is compared against every force exactly as before.
///
/// This is not a relaxation of the geometry — the clearance demanded of an
/// overlapping pair is unchanged, to the block. It is what makes the proof about
/// a *respawn point* rather than about a bonfire: a plain checkpoint is
/// superseded, so a body staged two quests after it was retired can no more meet
/// the party there than a body in another campaign can.
fn contemporaneous(rest: &RestPoint, hostile_onset: usize) -> bool {
    rest.reign_end.is_none_or(|end| hostile_onset < end)
}

/// One cell a dead player can materialise on, as the `DW0478` proof sees it.
#[derive(Clone, Debug)]
pub struct RestPoint {
    /// The checkpoint anchor name.
    pub anchor: String,
    /// `"bonfire"` or `"set-checkpoint"` — recorded so a reader can see WHICH
    /// respawn points were examined, and notice at a glance if one kind is
    /// missing from a campaign that has them.
    pub kind: &'static str,
    /// The resolved absolute respawn cell.
    pub pos: [i32; 3],
    /// The step at which this respawn point stops governing, `None` = never
    /// ([`crate::plan::Plan::respawn_reign_ends`]).
    pub reign_end: Option<usize>,
}

/// What the `DW0478` proof quantified over on this build.
///
/// Emitted as `validation/respawn-safety.json`. A proof that examined nothing is
/// not a pass, and the only way to tell the two apart is to publish the count
/// (CLAUDE.md: *every validation artifact states its binding count; a zero
/// binding is a finding*). This ledger exists because `DW0478` spent its whole
/// life returning `Ok(())` on `nobodys-cave-island` — three respawn points, five
/// unleashed hostiles, zero comparisons — and nothing anywhere said so.
#[derive(Clone, Debug)]
pub struct RespawnSafetyLedger {
    /// Every respawn point in content order, with the forces it was measured
    /// against and the forces it was not.
    pub rest_points: Vec<RestPoint>,
    /// Every hostile force's id, in content order.
    pub hostiles: Vec<String>,
    /// Per respawn point, in the same order: the ids it was compared against.
    pub compared: Vec<Vec<String>>,
    /// Per respawn point, in the same order: `(id, why it was not compared)`.
    pub skipped: Vec<Vec<(String, String)>>,
    /// The comparisons actually made — the proof's binding count.
    pub pairs: usize,
}

impl RespawnSafetyLedger {
    fn new(
        rest_points: &[RestPoint],
        sources: &[AggroSource],
        onsets: &BTreeMap<String, usize>,
    ) -> Self {
        let mut compared = Vec::new();
        let mut skipped = Vec::new();
        let mut pairs = 0;
        for r in rest_points {
            let mut yes = Vec::new();
            let mut no = Vec::new();
            for s in sources {
                let onset = onsets.get(&s.id).copied().unwrap_or(0);
                if contemporaneous(r, onset) {
                    yes.push(s.id.clone());
                    pairs += 1;
                } else {
                    no.push((
                        s.id.clone(),
                        format!(
                            "`{}` is first staged at critical-path step {onset}, and this \
                             `set-checkpoint` stops governing at step {} — a later \
                             `set-checkpoint` has replaced it before the body exists, so no \
                             death can ever deliver the party here while it is in the world",
                            s.id,
                            r.reign_end.unwrap_or(usize::MAX)
                        ),
                    ));
                }
            }
            compared.push(yes);
            skipped.push(no);
        }
        Self {
            rest_points: rest_points.to_vec(),
            hostiles: sources.iter().map(|s| s.id.clone()).collect(),
            compared,
            skipped,
            pairs,
        }
    }

    /// Whether the proof examined nothing at all.
    pub fn unbound(&self) -> bool {
        self.pairs == 0
    }

    /// Why it examined nothing, when it did — named, so a zero reads as a finding
    /// instead of as a green.
    pub fn reason(&self) -> Option<String> {
        if self.pairs > 0 {
            return None;
        }
        Some(
            match (self.rest_points.is_empty(), self.hostiles.is_empty()) {
                (true, true) => {
                    "this campaign declares no respawn point and no hostile force, so no \
                             cell a player comes back to life on can be inside anything's aggro \
                             range"
                        .to_string()
                }
                (true, false) => format!(
                    "this campaign declares {} hostile force(s) but no `set-checkpoint` and no \
                 `bonfire`: every death returns the party to world spawn, which this proof does \
                 not model",
                    self.hostiles.len()
                ),
                (false, true) => format!(
                    "this campaign declares {} respawn point(s) but no hostile force at all (no wave \
                 with a seated spawn, no actor the campaign unleashes or stages `vulnerable`), so \
                 there is no aggro range for one to be inside",
                    self.rest_points.len()
                ),
                (false, false) => format!(
                    "this campaign declares {} respawn point(s) and {} hostile force(s), and NO pair \
                 of them is ever in the world at the same time — every force is first staged \
                 after the respawn point that could have met it was already replaced",
                    self.rest_points.len(),
                    self.hostiles.len()
                ),
            },
        )
    }

    /// The ledger as the `validation/respawn-safety.json` artifact.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "code": DW_RESPAWN_IN_AGGRO,
            "rest_points": self
                .rest_points
                .iter()
                .zip(&self.compared)
                .zip(&self.skipped)
                .map(|((r, yes), no)| serde_json::json!({
                    "anchor": r.anchor,
                    "kind": r.kind,
                    "pos": r.pos,
                    "reign_end": r.reign_end,
                    "compared_against": yes,
                    "not_compared": no
                        .iter()
                        .map(|(id, why)| serde_json::json!({"id": id, "reason": why}))
                        .collect::<Vec<_>>(),
                }))
                .collect::<Vec<_>>(),
            "examined": self.rest_points.len(),
            "hostiles": self.hostiles,
            "pairs": self.pairs,
            "unbound": self.unbound(),
            "reason": self.reason(),
        })
    }
}

/// Every hostile force in the campaign, in deterministic content order (waves
/// then actors, each in declaration order).
fn aggro_sources(
    plan: &Plan,
    world: &World,
    placements: &BTreeMap<String, Vec<[i32; 3]>>,
    lanes: &LaneRoutes,
) -> Vec<AggroSource> {
    let c = plan.campaign;
    let mut out = Vec::new();
    for w in &c.quests.content.waves {
        let mut cells: Vec<(&'static str, [i32; 3], f64)> = placements
            .get(w.id.as_str())
            .into_iter()
            .flatten()
            .map(|p| ("seated spawn cell", *p, 0.0))
            .collect();
        let (radius, radius_source) = match &w.lane {
            Some(l) => (f64::from(l.aggro_radius), "the lane's `aggro_radius`"),
            None => match w
                .mobs
                .iter()
                .filter_map(|m| m.attributes.and_then(|a| a.follow_range))
                .fold(None::<f64>, |acc, r| Some(acc.map_or(r, |a| a.max(r))))
            {
                Some(r) => (r, "the wave's declared `follow_range`"),
                None => (
                    f64::from(DEFAULT_FOLLOW_RANGE),
                    "the default `follow_range` (none declared)",
                ),
            },
        };
        if let Some(wps) = lanes.get(w.id.as_str()) {
            cells.extend(
                lane_march_cells(plan, world, w, wps)
                    .into_iter()
                    .map(|p| ("lane path cell", p, LANE_MARCH_DRIFT)),
            );
        }
        if cells.is_empty() {
            continue;
        }
        out.push(AggroSource {
            id: w.id.as_str().to_string(),
            radius,
            radius_source,
            cells,
        });
    }
    for a in &c.quests.content.actors {
        if !actor_fights(c, a) {
            continue;
        }
        let Some(pos) = crate::plan::point_any(&plan.anchors, a.anchor.as_str()) else {
            continue;
        };
        let (radius, radius_source) = match a.attributes.and_then(|at| at.follow_range) {
            Some(r) => (r, "the actor's declared `follow_range`"),
            None => (
                f64::from(DEFAULT_FOLLOW_RANGE),
                "the default `follow_range` (none declared)",
            ),
        };
        out.push(AggroSource {
            id: a.id.as_str().to_string(),
            radius,
            radius_source,
            cells: vec![("staging anchor", pos, 0.0)],
        });
    }
    out
}

/// Whether the campaign declares this actor as something that FIGHTS — the same
/// declaration-based test `DW0469` uses: an `unleash-actor` beat (the author
/// asking for a real-AI twin) or `vulnerable: true` (a damageable target). Never
/// inferred from the species.
fn actor_fights(c: &delvewright_dsl::Campaign, a: &delvewright_dsl::Actor) -> bool {
    if a.vulnerable {
        return true;
    }
    let mut unleashed = false;
    delvewright_dsl::for_each_campaign_effect(c, &mut |_, _, eff| {
        if let QuestEffect::UnleashActor { actor, .. } = eff
            && actor.as_str() == a.id.as_str()
        {
            unleashed = true;
        }
    });
    unleashed
}

/// Every cell a lane squad provably walks: the A*-proven legs from the wave's
/// form-up footing through each waypoint, over the same **no-gate-use** view
/// [`plan_lanes`] proved them on. An unroutable leg is `DW0386`'s business and is
/// simply skipped here.
fn lane_march_cells(
    plan: &Plan,
    world: &World,
    w: &delvewright_dsl::Wave,
    wps: &[[i32; 3]],
) -> Vec<[i32; 3]> {
    let entity_world_owned;
    let world: &World = if world.has_use_gates() {
        entity_world_owned = world.without_gate_use();
        &entity_world_owned
    } else {
        world
    };
    let Some(area) = crate::plan::wave_area(plan.campaign, w.id.as_str()) else {
        return Vec::new();
    };
    let Some(anchor) = plan.point(area, w.anchor.as_str()) else {
        return Vec::new();
    };
    let Some(mut prev) = world.snap_standable(anchor, SNAP_RADIUS) else {
        return Vec::new();
    };
    let mut out = vec![prev];
    for wp in wps {
        if let Some(path) = world.find_path(prev, *wp) {
            out.extend(path);
        }
        prev = *wp;
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// The pure core of [`check_respawn_safe_zone`] (unit-testable without a
/// [`Plan`]). Reports the FIRST violation in content order, naming the closest
/// offending cell and the exact clearance the geometry is short by.
fn verify_respawn_safe_zone(
    rest_points: &[RestPoint],
    sources: &[AggroSource],
    onsets: &BTreeMap<String, usize>,
) -> Result<(), NavError> {
    for rest in rest_points {
        let (anchor, pos) = (&rest.anchor, &rest.pos);
        for src in sources {
            if !contemporaneous(rest, onsets.get(&src.id).copied().unwrap_or(0)) {
                continue;
            }
            let Some((what, cell, dist, drift)) = src
                .cells
                .iter()
                .map(|(what, cell, drift)| (*what, *cell, cell_distance(*pos, *cell), *drift))
                .filter(|(_, _, d, drift)| *d <= src.radius + drift)
                // Nearest first, then by cell, so the message is deterministic.
                .min_by(|a, b| {
                    a.2.partial_cmp(&b.2)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.1.cmp(&b.1))
                })
            else {
                continue;
            };
            let reach = if drift > 0.0 {
                format!(
                    "the {reach:.1}-block reach: the {radius:.1}-block perception radius \
                     ({radius_source}) plus the {drift:.1}-block measured marching drift — a \
                     lane squad marches a corridor around its polyline, not the line itself \
                     (td-routing-spike dossier)",
                    reach = src.radius + drift,
                    radius = src.radius,
                    radius_source = src.radius_source,
                )
            } else {
                format!(
                    "the {radius:.1}-block perception radius ({radius_source})",
                    radius = src.radius,
                    radius_source = src.radius_source,
                )
            };
            return Err(NavError {
                code: DW_RESPAWN_IN_AGGRO,
                message: format!(
                    "respawn point `{anchor}` ({pos:?}) sits INSIDE the aggro range of `{id}`: \
                     its {what} {cell:?} is {dist:.1} blocks away, within {reach}. A respawn \
                     point is where the party comes back after a death — and, for a bonfire, \
                     where every `respawns_on_rest` wave is put back on its feet. With a hostile \
                     already perceiving that cell, dying delivers the party into contact on the \
                     tick they arrive, and the retry loop becomes a soft-lock \
                     (spec-0016 §1). The rule is the same for a plain `set-checkpoint` \
                     and for a `bonfire`: vanilla returns a dead player to either by the \
                     identical `spawnpoint` mechanism, so the hazard is a property of the CELL, \
                     never of the verb that named it. Move the respawn point out of the danger — \
                     into a side room, behind the threshold, past the end of the lane — or move \
                     the force's anchor / lane. Do NOT shrink `follow_range` to buy the \
                     clearance: that retunes the fight to hide a placement bug.",
                    id = src.id,
                ),
            });
        }
    }
    Ok(())
}

/// Euclidean distance between two cells, in blocks.
fn cell_distance(a: [i32; 3], b: [i32; 3]) -> f64 {
    (0..3)
        .map(|i| f64::from(a[i] - b[i]).powi(2))
        .sum::<f64>()
        .sqrt()
}

/// The pure core of [`check_shortcuts`] (split out so it is unit-testable against
/// a synthetic [`World`] without a full [`Plan`]). With no resolvable entry cell
/// there is nothing to measure from and both proofs are vacuous — `DW0345`
/// already fails a campaign whose entry does not resolve.
fn verify_shortcuts(
    world: &World,
    shortcuts: &[crate::plan::ShortcutPlan],
    entry: Option<[i32; 3]>,
) -> Result<(), NavError> {
    let Some(entry) = entry else {
        return Ok(());
    };
    for sc in shortcuts {
        let cells: BTreeSet<[i32; 3]> =
            crate::assembled::region_cells(sc.gate_region.0, sc.gate_region.1).collect();
        let sealed = world.with_sealed(&cells);

        // Both walks are measured from the same footing and to the same goal; only
        // the gate differs. Snapping happens on the SEALED world so neither
        // endpoint can land inside the gate region itself.
        let start = sealed.snap_standable(entry, SNAP_RADIUS);
        let goal = sealed.snap_standable(sc.unlock, SNAP_RADIUS);
        let (Some(start), Some(goal)) = (start, goal) else {
            // An unstandable entry or unlock is another proof's concern
            // (`DW0345` / the anchor checks); do not double-report it here.
            continue;
        };

        // (1) the long route exists while the gate is sealed.
        let Some(long) = sealed.find_path(start, goal).map(|p| p.len()) else {
            return Err(NavError {
                code: DW_SHORTCUT_NO_LONG_ROUTE,
                message: format!(
                    "shortcut `{}`: no long route — its unlock affordance at `{}` ({:?}) is not \
                     walkable from the campaign entry while gate `{}` is sealed, so the mechanism \
                     that opens the shortcut sits behind the shortcut and can never be pulled. A \
                     shortcut is earned the hard way first (spec-0016 §2): connect the far side by \
                     a long route, or move the unlock onto one. Do NOT open the gate at world-load \
                     to silence this.",
                    sc.id, sc.unlock_anchor, sc.unlock, sc.gate_anchor
                ),
            });
        };

        // (2) opening the gate strictly shortens that same walk (anti-leak).
        let short = world
            .find_path(start, goal)
            .map(|p| p.len())
            .unwrap_or(long);
        if short >= long {
            return Err(NavError {
                code: DW_SHORTCUT_NO_GAIN,
                message: format!(
                    "shortcut `{}` leaks: opening gate `{}` does not shorten the walk from the \
                     campaign entry to its own unlock `{}` ({long} steps sealed, {short} open), so \
                     the unlock is not on the far side of anything and the loop-back the shortcut \
                     is FOR never happens. Put the unlock past the gate, on the end of the long \
                     route (spec-0016 §2) — never delete the proof.",
                    sc.id, sc.gate_anchor, sc.unlock_anchor
                ),
            });
        }
    }
    Ok(())
}

/// The retry-cost budget (spec-0016 §7): 60 s of traversal from a rest point to
/// the beat it respawns the party into, in ticks.
const RETRY_BUDGET_TICKS: u32 = 60 * 20;

/// Default aggro radius for a wave mob with no declared `follow_range` — vanilla's
/// `generic.follow_range` default for the common hostiles (zombie, skeleton,
/// husk, pillager). Used by the optional-elite bypass lint when the author has
/// not tuned the attribute.
pub const DEFAULT_FOLLOW_RANGE: u32 = 16;

/// The spec-0016 §7 pacing lints. **Warning tier** — every finding here is a
/// design judgement the compiler can measure but must not overrule, so these
/// return diagnostics rather than failing the build.
///
/// 1. [`DW_RETRY_COST`] (`DW0379`) — bonfire/checkpoint → the beat it respawns
///    into, over the proven path, must be under [`RETRY_BUDGET_TICKS`]. Dying
///    should be an investment, not a commute.
/// 2. [`DW_OPTIONAL_ELITE_UNAVOIDABLE`] (`DW0380`) — an enemy no critical-path
///    `kill` objective requires must have a route around it. The Tree Sentinel
///    pattern is legitimate; a "walk around it" you cannot walk around is not.
pub fn pacing_lints(plan: &Plan, world: &World) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    out.extend(retry_cost_lint(plan, world));
    out.extend(optional_elite_lint(plan, world));
    out
}

/// `DW0379`: the walk back from each rest point to the first beat that follows it.
fn retry_cost_lint(plan: &Plan, world: &World) -> Vec<Diagnostic> {
    let cps: Vec<(String, [i32; 3], usize, bool)> = plan
        .checkpoints
        .iter()
        .map(|c| (c.anchor.clone(), c.pos, c.fire_step, c.rest))
        .collect();
    verify_retry_cost(world, &cps, &critical_positions(plan))
}

/// A rest point as the retry-cost lint sees it.
struct RestRef<'a> {
    anchor: &'a str,
    pos: [i32; 3],
    fire_step: usize,
    rest: bool,
}

/// The pure core of [`retry_cost_lint`] (unit-testable against a synthetic
/// [`World`]). Each rest point is `(anchor, cell, fire_step, is_bonfire)`.
fn verify_retry_cost(
    world: &World,
    rests: &[(String, [i32; 3], usize, bool)],
    positions: &[VisitedPos],
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for (anchor, pos, fire_step, is_rest) in rests {
        let cp = RestRef {
            anchor,
            pos: *pos,
            fire_step: *fire_step,
            rest: *is_rest,
        };
        let Some(from) = world.snap_standable(cp.pos, SNAP_RADIUS) else {
            continue; // DW0316 owns an unstandable rest point
        };
        let Some(target) = positions
            .iter()
            .filter(|p| p.src_step > cp.fire_step && !p.transport_before)
            .min_by_key(|p| p.src_step)
        else {
            continue;
        };
        let Some(goal) = world.snap_endpoint(target.pos, target.talk_to) else {
            continue;
        };
        let Some(path) = world.find_path(from, goal) else {
            continue; // DW0315 owns an unreachable one
        };
        let blocks = path.len().saturating_sub(1) as u32;
        let ticks = blocks * SPRINT_TICKS_PER_BLOCK;
        if ticks > RETRY_BUDGET_TICKS {
            out.push(Diagnostic::warning(
                DW_RETRY_COST,
                "quests",
                format!("/content/quests/checkpoint/{}", cp.anchor),
                format!(
                    "retry cost: {} `{}` is {blocks} blocks ({} s at {SPRINT_TICKS_PER_BLOCK} \
                     t/block) from the next beat it respawns the party into — over the {} s \
                     budget (spec-0016 §7). Dying must be an investment, not a commute: past this \
                     the loop stops teaching and starts taxing. Move the rest point forward, or \
                     add one closer to the beat.",
                    if cp.rest { "bonfire" } else { "checkpoint" },
                    cp.anchor,
                    ticks / 20,
                    RETRY_BUDGET_TICKS / 20
                ),
            ));
        }
    }
    out
}

/// `DW0380`: every optional enemy must be walkable around.
///
/// A wave is **optional** when no `kill` objective on the critical path names it —
/// the party is never required to fight it. For each such wave, its mobs' aggro
/// spheres (the declared `follow_range`, else [`DEFAULT_FOLLOW_RANGE`]) are
/// forced solid around the wave anchor and the forced critical path is re-routed:
/// if a leg that routed before no longer does, every way forward runs through the
/// fight and "optional" is a lie.
fn optional_elite_lint(plan: &Plan, world: &World) -> Vec<Diagnostic> {
    use delvewright_dsl::Objective;
    let c = plan.campaign;
    let required: BTreeSet<&str> = c
        .quests
        .content
        .quests
        .iter()
        .flat_map(|q| q.objectives.iter())
        .filter_map(|o| match o {
            Objective::Kill { wave, .. } => Some(wave.as_str()),
            _ => None,
        })
        .collect();
    let elites: Vec<(String, [i32; 3], i32)> = c
        .quests
        .content
        .waves
        .iter()
        .filter(|w| !required.contains(w.id.as_str()))
        .filter_map(|w| {
            let centre = crate::plan::point_any(&plan.anchors, w.anchor.as_str())?;
            let radius = w
                .mobs
                .iter()
                .filter_map(|m| m.attributes.and_then(|a| a.follow_range))
                .map(|r| r.max(0.0) as i32)
                .max()
                .unwrap_or(DEFAULT_FOLLOW_RANGE as i32);
            Some((w.id.as_str().to_string(), centre, radius))
        })
        .collect();
    verify_optional_elites(world, &elites, &critical_positions(plan))
}

/// The pure core of [`optional_elite_lint`] (unit-testable against a synthetic
/// [`World`]). Each elite is `(wave id, anchor cell, aggro radius)`.
fn verify_optional_elites(
    world: &World,
    elites: &[(String, [i32; 3], i32)],
    positions: &[VisitedPos],
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for (id, centre, radius) in elites {
        let r = *radius;
        let mut sphere: BTreeSet<[i32; 3]> = BTreeSet::new();
        for dx in -r..=r {
            for dy in -r..=r {
                for dz in -r..=r {
                    if dx * dx + dy * dy + dz * dz <= r * r {
                        sphere.insert([centre[0] + dx, centre[1] + dy, centre[2] + dz]);
                    }
                }
            }
        }
        let aggroed = world.with_sealed(&sphere);
        let blocked = positions.windows(2).find(|pair| {
            if pair[1].transport_before {
                return false;
            }
            let (Some(a), Some(b)) = (
                world.snap_endpoint(pair[0].pos, pair[0].talk_to),
                world.snap_endpoint(pair[1].pos, pair[1].talk_to),
            ) else {
                return false;
            };
            // A leg that goes nowhere, or that never routed in the clean world,
            // is not this lint's business (`DW0311` owns the latter).
            if a == b || world.find_path(a, b).is_none() {
                return false;
            }
            // An endpoint INSIDE the aggro sphere is not a missing bypass — the
            // party is required to stand there, so the fight is contested ground
            // by design (a "live threat" wave seated on an objective anchor is a
            // legitimate, landed pattern). This lint is about the ROUTE being
            // swallowed, not the destination being dangerous.
            if sphere.contains(&a) || sphere.contains(&b) {
                return false;
            }
            let (Some(a2), Some(b2)) = (
                aggroed.snap_endpoint(pair[0].pos, pair[0].talk_to),
                aggroed.snap_endpoint(pair[1].pos, pair[1].talk_to),
            ) else {
                return true;
            };
            aggroed.find_path(a2, b2).is_none()
        });
        if let Some(pair) = blocked {
            out.push(Diagnostic::warning(
                DW_OPTIONAL_ELITE_UNAVOIDABLE,
                "quests",
                format!("/content/waves/{id}"),
                format!(
                    "optional enemy `{id}` at {centre:?} has no bypass: with its {r}-block aggro \
                     radius blocked, the forced walk from {:?} to {:?} no longer routes — every \
                     way forward runs through the fight, so \"optional\" is a lie (spec-0016 §7). \
                     A powerful OPTIONAL enemy near the start is legitimate — the Tree Sentinel \
                     pattern — and this is its one obligation: the walk-around has to exist. \
                     Widen the room, move the wave off the corridor, or make the kill a real \
                     objective.",
                    pair[0].pos, pair[1].pos
                ),
            ));
        }
    }
    out
}

/// Prove every `begin-stealth` zone is standable and reachable from the beat that
/// activates it (DSL v0.6, spec-0014) — [`DW_STEALTH_ZONE`] (`DW0327`). A zone the
/// player can never legally occupy (walled/void) or can never walk to from the
/// activating position is a guaranteed unwinnable stealth beat.
pub fn check_stealth_zones(plan: &Plan, world: &World) -> Result<(), NavError> {
    let beats: Vec<StealthProbe> = plan
        .stealth_beats
        .iter()
        .map(|b| (b.zones.clone(), b.fire_step))
        .collect();
    verify_stealth(world, &beats, &critical_positions(plan))
}

/// The pure core of [`check_stealth_zones`] (unit-testable against a synthetic
/// [`World`]). Each beat is `(zones, fire_step)`; each zone `(name, centre,
/// half-extents)`.
fn verify_stealth(
    world: &World,
    beats: &[StealthProbe],
    positions: &[VisitedPos],
) -> Result<(), NavError> {
    for (zones, fire_step) in beats {
        // The player's position at the activating beat: the visited position at the
        // firing step, else the nearest earlier one, else the first position.
        let player_pos = positions
            .iter()
            .filter(|p| p.src_step <= *fire_step)
            .max_by_key(|p| p.src_step)
            .or_else(|| positions.first())
            .map(|p| p.pos);
        for (name, pos, extent) in zones {
            let lo = [
                pos[0] - extent[0] as i32,
                pos[1] - extent[1] as i32,
                pos[2] - extent[2] as i32,
            ];
            let hi = [
                pos[0] + extent[0] as i32,
                pos[1] + extent[1] as i32,
                pos[2] + extent[2] as i32,
            ];
            // EVERY standable cell of the zone box, not just the one nearest its
            // centre. A zone whose centre snaps to a standable cell in a
            // walled-off pocket while the rest of the box is perfectly reachable
            // used to raise a spurious `DW0327`; the obligation is "the player can
            // reach *somewhere* in this zone", so the proof is reachable-any.
            let stands: Vec<[i32; 3]> = crate::assembled::region_cells(lo, hi)
                .filter(|c| world.is_standable(*c))
                .collect();
            if stands.is_empty() {
                return Err(NavError {
                    code: DW_STEALTH_ZONE,
                    message: format!(
                        "stealth zone `{name}` (box {lo:?}..{hi:?}) has no standable cell — a \
                         player can never legally hide there, so the beat is unwinnable. Place the \
                         zone over reachable floor, or widen its `extent` to include a standable \
                         cell."
                    ),
                });
            }
            // One reachability flood from the player's position (not one A* per zone
            // cell): the question is set membership, not a route.
            let start = player_pos.and_then(|p| world.snap_standable(p, SNAP_RADIUS));
            if let Some(start) = start
                && {
                    let reachable = world.reachable_walkable(&[start]);
                    !stands.iter().any(|s| reachable.contains(s))
                }
            {
                return Err(NavError {
                    code: DW_STEALTH_ZONE,
                    message: format!(
                        "stealth zone `{name}` (box {lo:?}..{hi:?}, {n} standable cell(s)) is not \
                         reachable from the player's position {:?} when the stealth beat begins — \
                         NO cell of the zone is walkable from there, so the player would be caught \
                         before ever reaching cover. Route the zone within walkable reach of the \
                         activating beat, or move where the beat starts.",
                        player_pos.unwrap(),
                        n = stands.len(),
                    ),
                });
            }
        }
    }
    Ok(())
}

// --- DW0355: stealth onset survivability ------------------------------------

/// Ticks a sprinting player needs to cross one block. Vanilla sprint is
/// 5.612 blocks/s = 0.2806 blocks/tick → 3.56 t/block; rounded **up** to 4 so the
/// model never credits the player with speed they do not have. (Sprint-jumping is
/// faster; the proof deliberately does not assume the player chains jumps.)
const SPRINT_TICKS_PER_BLOCK: u32 = 4;
/// Extra ticks charged for each one-block step **up** on the flee route — the jump
/// arc a player must complete to gain the block. Conservative: a vanilla jump apex
/// is ~6 ticks.
const CLIMB_TICKS: u32 = 6;
/// Ticks charged before the player is under way at all: the beat arms while they
/// are standing still, mid-interaction, reading the narration that tells them to
/// run. 10 ticks = 0.5 s of orientation. This is the "fair warning" allowance —
/// without it the proof would assume a player already sprinting toward cover at
/// the instant the session arms.
const ONSET_REACTION_TICKS: u32 = 10;

/// A start position the onset proof must clear, with the label its diagnostic uses.
type OnsetStart = (String, [i32; 3]);

/// Prove every **punishing** `begin-stealth` beat is escapable at onset (DSL v0.6,
/// spec-0014 + spec-0016) — [`DW_STEALTH_ONSET`] (`DW0355`).
///
/// DW0327 already proves each zone is standable and connected to the beat. That is
/// not enough: `begin-stealth` arms *instantly*, the judge starts counting on the
/// very next tick, and `on_caught` fires `grace_ticks` later wherever the player
/// happens to be. So the real obligation is a **timing** one — from every position
/// a player can legally occupy when the session arms, some zone must be reachable
/// within the grace window at sprint speed:
///
/// - the **activating position** — the anchor of the objective whose completion
///   fires the beat (where the player provably is, since completing it is what
///   armed the session), and
/// - every **respawn position** — each `set-checkpoint` reigning at some step in
///   the beat's active window `[fire_step, end_step]`. A caught player respawns
///   there with the session still running and the grace clock restarted; if that
///   cell cannot beat the window either, the beat is an infinite death loop rather
///   than a souls retry.
///
/// Routes are measured over the same per-leg geometry DW0311/DW0315 use (gates
/// causally sealed by the beat's firing step forced solid), costed at
/// [`SPRINT_TICKS_PER_BLOCK`] per block plus [`CLIMB_TICKS`] per block climbed, and
/// charged [`ONSET_REACTION_TICKS`] of standing-start reaction.
///
/// Scope: beats whose `on_caught` actually punishes ([`StealthBeat::is_punishing`]
/// — `damage-players` or `spawn-wave`, at any nesting depth). A beat that only
/// narrates when spotted has nothing to escape, so no timing obligation exists.
pub fn check_stealth_onset(plan: &Plan, world: &World) -> Result<(), NavError> {
    let positions = critical_positions(plan);
    for beat in &plan.stealth_beats {
        if !beat.is_punishing() {
            continue;
        }
        let mut starts: Vec<OnsetStart> = Vec::new();
        // 1. Where the player stands when the beat arms: the visited position at
        //    the firing step, else the nearest earlier one, else the first.
        if let Some(p) = positions
            .iter()
            .filter(|p| p.src_step <= beat.fire_step)
            .max_by_key(|p| p.src_step)
            .or_else(|| positions.first())
        {
            starts.push((
                format!("the activating objective's anchor {:?}", p.pos),
                p.pos,
            ));
        }
        // 2. Every checkpoint that can drop a player into the running session: the
        //    one reigning when the beat arms (latest fire_step ≤ fire_step, ties
        //    broken by content index — a `set-checkpoint` listed beside the
        //    `begin-stealth` in the same objective's effects is the reigning one),
        //    plus every checkpoint set later while the beat is still active.
        let end = beat.end_step.unwrap_or(usize::MAX);
        if let Some(reigning) = plan
            .checkpoints
            .iter()
            .filter(|c| c.fire_step <= beat.fire_step)
            .max_by_key(|c| (c.fire_step, c.index))
        {
            starts.push((
                format!(
                    "checkpoint `{}` respawn {:?}",
                    reigning.anchor, reigning.pos
                ),
                reigning.pos,
            ));
        }
        for c in plan
            .checkpoints
            .iter()
            .filter(|c| c.fire_step > beat.fire_step && c.fire_step <= end)
        {
            starts.push((
                format!("checkpoint `{}` respawn {:?}", c.anchor, c.pos),
                c.pos,
            ));
        }
        let st = world.region_state_at(&plan.region_events, beat.fire_step, &|g, s| {
            plan.gate_fired_before(g, s)
        });
        let leg_world_owned;
        let leg_world: &World = if st.is_empty() {
            world
        } else {
            leg_world_owned = world.with_region_state(&st);
            &leg_world_owned
        };
        verify_stealth_onset(
            leg_world,
            &beat.zones,
            beat.grace_ticks,
            &starts,
            beat.index,
        )?;
    }
    Ok(())
}

/// The pure core of [`check_stealth_onset`] (unit-testable against a synthetic
/// [`World`]): every start must reach some zone cell within `grace_ticks`.
fn verify_stealth_onset(
    world: &World,
    zones: &[ZoneCell],
    grace_ticks: u32,
    starts: &[OnsetStart],
    beat_index: usize,
) -> Result<(), NavError> {
    // The budget the flee route itself gets, after the standing-start allowance.
    let budget = grace_ticks.saturating_sub(ONSET_REACTION_TICKS);
    for (label, raw) in starts {
        let Some(start) = world.snap_standable(*raw, SNAP_RADIUS) else {
            continue; // unsnappable start — DW0311/DW0315/DW0316's concern, not ours
        };
        let Some((cost, cell, zone)) = nearest_zone_by_flee_time(world, zones, start, grace_ticks)
        else {
            continue; // no zone reachable at all within the search cap — DW0327's concern
        };
        if cost <= budget {
            continue;
        }
        let need = cost + ONSET_REACTION_TICKS;
        let deficit = need - grace_ticks;
        return Err(NavError {
            code: DW_STEALTH_ONSET,
            message: format!(
                "stealth beat #{beat_index}: a player cannot beat the grace window from {label}. \
                 The nearest zone cell is `{zone}` {cell:?} — {cost} ticks of sprinting away \
                 (model: {SPRINT_TICKS_PER_BLOCK} t/block, +{CLIMB_TICKS} t per block climbed) \
                 plus {ONSET_REACTION_TICKS} ticks of standing-start reaction = {need} ticks, \
                 against `grace_ticks` {grace_ticks} — short by {deficit} ticks. The beat's \
                 `on_caught` punishes, so EVERY player dies here a fixed moment after it arms, \
                 and if this start is a checkpoint the retry loop never terminates. Fix the \
                 BEAT, not the proof: raise `grace_ticks` to at least {need} (the measured \
                 sprint time plus reaction) and add a tension margin, put a zone within reach \
                 of where the beat actually starts, move the checkpoint into/beside a zone, or \
                 arm the beat from a less exposed objective. Note that merely DELAYING the arm \
                 (a `sequence` step) does not discharge this: the clock still starts with the \
                 player free to be standing right here, so the grace window itself must cover \
                 the flee. Do NOT delete the `on_caught` consequence to silence this."
            ),
        });
    }
    Ok(())
}

/// The cheapest zone cell by **flee time** from `start`: a deterministic
/// tick-weighted Dijkstra over standable cells (cardinal steps cost
/// [`SPRINT_TICKS_PER_BLOCK`], a step up additionally costs [`CLIMB_TICKS`]),
/// stopping at the first settled cell inside any zone box. Returns
/// `(ticks, cell, zone name)`.
///
/// The search is capped well past the grace window so a failing beat can still
/// report a real number; beyond the cap the beat is failing by so much that the
/// exact figure carries no extra information. Determinism (ADR-0006): the frontier
/// is ordered by `(cost, cell)` and neighbours expand in `neighbors`' fixed order.
fn nearest_zone_by_flee_time(
    world: &World,
    zones: &[ZoneCell],
    start: [i32; 3],
    grace_ticks: u32,
) -> Option<(u32, [i32; 3], String)> {
    let boxes: Vec<(&str, [i32; 3], [i32; 3])> = zones
        .iter()
        .map(|(name, pos, extent)| {
            let lo = [
                pos[0] - extent[0] as i32,
                pos[1] - extent[1] as i32,
                pos[2] - extent[2] as i32,
            ];
            let hi = [
                pos[0] + extent[0] as i32,
                pos[1] + extent[1] as i32,
                pos[2] + extent[2] as i32,
            ];
            (name.as_str(), lo, hi)
        })
        .collect();
    let zone_of = |c: [i32; 3]| {
        boxes
            .iter()
            .find(|(_, lo, hi)| (0..3).all(|k| lo[k] <= c[k] && c[k] <= hi[k]))
            .map(|(n, _, _)| *n)
    };
    let cap = grace_ticks.saturating_mul(4).saturating_add(400);
    let mut best: BTreeMap<[i32; 3], u32> = BTreeMap::new();
    let mut open: BinaryHeap<Reverse<(u32, [i32; 3])>> = BinaryHeap::new();
    best.insert(start, 0);
    open.push(Reverse((0, start)));
    while let Some(Reverse((cost, cur))) = open.pop() {
        if cost > *best.get(&cur).unwrap_or(&u32::MAX) {
            continue; // stale heap entry
        }
        if let Some(name) = zone_of(cur) {
            return Some((cost, cur, name.to_string()));
        }
        if cost >= cap {
            continue;
        }
        for n in world.neighbors(cur) {
            let climb = if n[1] > cur[1] { CLIMB_TICKS } else { 0 };
            let next = cost + SPRINT_TICKS_PER_BLOCK + climb;
            if next < *best.get(&n).unwrap_or(&u32::MAX) {
                best.insert(n, next);
                open.push(Reverse((next, n)));
            }
        }
    }
    None
}

/// Prove every **lethal** trap on the forced critical path is discharged (DSL
/// v0.6, spec-0011) — [`DW_TRAP_LETHAL_UNAVOIDABLE`] (`DW0342`). Death is
/// recoverable but costly (`keep_inventory true`, respawn at the entrance or last
/// checkpoint), so an unavoidable lethal trap deep in the delve can soft-loop the
/// party. For every trap whose lethality is `lethal` and whose trigger cell is a
/// required critical-path cell, exactly one discharge must hold:
///
/// - **Avoidable** — the trigger cell is not a forced-path cell (the exported
///   waypoints already steer clear). No obligation; the preferred outcome.
/// - **Survivable** — the trap is `reset: once`: it fires, is spent, and the
///   respawn walk-back never re-triggers it, so there is no soft-loop.
/// - **Disarmable** — a disarm affordance is reachable from the spawn **without
///   crossing the trap cell**, so the party can turn the trap off before being
///   forced onto it.
///
/// A forced lethal `rearm` trap with no reachable disarm provably soft-loops the
/// party → `DW0342`. Non-critical-path (branch/optional) lethal traps carry no
/// obligation here (existing `DW0306` gate-reachability covers not sealing off a
/// mandatory anchor).
pub fn check_traps(plan: &Plan, world: &World, moves: &[MovePlan]) -> Result<(), NavError> {
    if plan.traps.is_empty() {
        return Ok(());
    }
    let required = world.required_path_cells(plan, moves);
    let spawn_starts: Vec<[i32; 3]> = plan
        .anchors
        .iter()
        .filter(|((_, name), _)| crate::plan::is_entry_anchor_name(name))
        .filter_map(|(_, a)| match a {
            ResolvedAnchor::Point { pos, .. } => Some(*pos),
            ResolvedAnchor::Gate { .. } => None,
        })
        .collect();
    let legs = world.walked_legs_sealed(plan);
    verify_traps(world, &plan.traps, &required, &spawn_starts, &legs)
}

/// The pure core of [`check_traps`] (unit-testable against a synthetic [`World`]).
/// `required` is the forced critical-path cell set; `spawn_starts` are the spawn
/// cells the disarm-reachability search roots at; `legs` are the walked legs with
/// their gate seals, used to pick the gate state a disarm must be reachable under.
fn verify_traps(
    world: &World,
    traps: &[TrapPlan],
    required: &BTreeSet<[i32; 3]>,
    spawn_starts: &[[i32; 3]],
    legs: &[(LegRoute, BTreeSet<[i32; 3]>)],
) -> Result<(), NavError> {
    for t in traps {
        if !matches!(t.lethality, Lethality::Lethal) {
            continue; // only lethal traps carry the obligation
        }
        let tc = t.trigger_cell;
        // (a) Avoidable: the trigger cell is never a forced critical-path cell.
        // `required` is computed over the causally-sealed per-leg world,
        // so a detour the player only has to walk BECAUSE a `close-gate`
        // shut the direct route counts as forced, exactly as it is in play.
        if !required.contains(&tc) {
            continue;
        }
        // (b) Survivable: a single-shot trap fires once and is spent; the respawn
        // walk-back (keep_inventory) never re-triggers it → no soft-loop.
        if matches!(t.reset, TrapReset::Once) {
            continue;
        }
        // (c) Disarmable: a disarm affordance reachable before the trap is forced —
        // under the gate state in force on the earliest leg that crosses the trap
        // cell. Searching the fully-open world would "prove" a disarm the party
        // can no longer reach once a `close-gate` has fired.
        let seal = legs
            .iter()
            .find(|(leg, _)| leg.cells.contains(&tc))
            .map(|(_, s)| s)
            .filter(|s| !s.is_empty());
        let sealed_world;
        let disarm_world: &World = match seal {
            Some(s) => {
                sealed_world = world.with_sealed(s);
                &sealed_world
            }
            None => world,
        };
        if let Some(dis) = &t.disarm
            && disarm_reachable_before(disarm_world, spawn_starts, dis.via_cell, tc)
        {
            continue;
        }
        return Err(NavError {
            code: DW_TRAP_LETHAL_UNAVOIDABLE,
            message: format!(
                "lethal trap `{}` sits on the forced critical path at {tc:?} with no discharge — \
                 it is not avoidable (its trigger cell is a required path cell), not survivable (it \
                 `rearm`s, so a respawn walk-back re-triggers it → soft-loop), and not disarmable \
                 (no disarm affordance is reachable before it). Move the trap off the critical \
                 path, set `reset: once`, or add a `disarm` whose `via` anchor is reachable before \
                 the trap cell — do NOT weaken this check to get green.",
                t.id
            ),
        });
    }
    Ok(())
}

/// Whether the disarm affordance at `via` is reachable from any spawn start over
/// the walkable world **without ever stepping on the trap cell** — i.e. the party
/// can reach and use the disarm before being forced onto the trap. A BFS over
/// standable cells with `trap_cell` removed from the walkable set.
fn disarm_reachable_before(
    world: &World,
    starts: &[[i32; 3]],
    via: [i32; 3],
    trap_cell: [i32; 3],
) -> bool {
    let Some(goal) = world.snap_standable(via, SNAP_RADIUS) else {
        return false;
    };
    if goal == trap_cell {
        return false;
    }
    let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut queue: std::collections::VecDeque<[i32; 3]> = std::collections::VecDeque::new();
    for s in starts {
        if let Some(start) = world.snap_standable(*s, SNAP_RADIUS)
            && start != trap_cell
            && seen.insert(start)
        {
            queue.push_back(start);
        }
    }
    while let Some(cur) = queue.pop_front() {
        if cur == goal {
            return true;
        }
        for n in world.neighbors(cur) {
            if n != trap_cell && seen.insert(n) {
                queue.push_back(n);
            }
        }
    }
    seen.contains(&goal)
}

/// A walked critical-path leg with the full A* cell route the compiler proved
/// connects it — the export counterpart of [`check_critical_path`].
/// `from`/`to` are the raw visited anchor cells (identical to the harness
/// `critical-path.json` step positions, so the harness can key a leg by its
/// destination); `cells` is the standable-cell polyline between their snapped floor
/// endpoints, inclusive of both.
#[derive(Debug, Clone)]
pub struct LegRoute {
    /// The raw visited anchor the leg walks FROM (the previous critical position).
    pub from: [i32; 3],
    /// The raw visited anchor the leg walks TO (this critical position).
    pub to: [i32; 3],
    /// The `critical_path` step index of the destination position — the objective
    /// this leg walks toward. Lets the visual-tier POV planner
    /// (`crate::render_plan`) name the served objective without re-deriving the
    /// leg selection.
    pub to_step: usize,
    /// The standable-cell A* path from the snapped `from` floor to the snapped `to`
    /// floor, inclusive of both endpoints.
    pub cells: Vec<[i32; 3]>,
    /// The cells of `cells` that are closed fence gates the player must right-click
    /// open to pass ("use-gate" edges), in path order. Exported in the
    /// waypoint metadata so the harness bot knows the leg crosses a gate (its
    /// pathfinder's `canOpenDoors` performs the adventure-legal click); always
    /// kept as thinned waypoints.
    pub use_gates: Vec<[i32; 3]>,
    /// The runtime-region state in force while the player walks this leg — the
    /// world the A* above actually ran in ([`World::walked_leg_region_state`]).
    ///
    /// **Private, and it is the point.** A `LegRoute` is only ever produced by
    /// [`route_walked_legs`], which is the single site that decides this value; a
    /// consumer that wants to re-judge these cells cannot supply an opinion of its
    /// own, it can only ask [`LegRoute::proven_world`]. Before the leg carried it,
    /// the state was computed, used for the route, and dropped — so the
    /// standability self-check re-judged the cells against the base world and
    /// refused every route that crossed floor the campaign lays at runtime.
    region_state: RegionState,
}

impl LegRoute {
    /// The world this leg's route was proven over: `world` as this leg's runtime
    /// region writes leave it. `None` when the leg has no runtime writes in force,
    /// which is every leg of every campaign that writes no region — those judge
    /// `world` itself and clone nothing.
    ///
    /// The one way to obtain a leg's world. It exists so that "which world does
    /// this route mean" has exactly one answer, held by the value that carries the
    /// route, rather than one answer per caller.
    fn proven_world(&self, world: &World) -> Option<World> {
        (!self.region_state.is_empty()).then(|| world.with_region_state(&self.region_state))
    }
}

/// Compute the proven A* cell route for every WALKED critical-path leg (transport
/// hops skipped), for export as validation metadata (see the `waypoints` module).
/// Mirrors [`check_critical_path`]'s leg selection, endpoint snapping **and
/// per-leg gate seals** exactly, so an exported leg is the same route
/// the DW0311 guard proved routable — and an exported waypoint can never cross a
/// gate a `close-gate` has already shut by the time the bot walks that leg.
/// Intended to be called only after [`check_critical_path`] has succeeded; a leg
/// that fails to snap or route is silently omitted (cannot occur once the check
/// has passed).
pub fn critical_path_routes(plan: &Plan, world: &World) -> Vec<LegRoute> {
    world.walked_legs(plan)
}

/// Per-branch `DW0311` (spec-0025): prove every walked leg of ONE
/// branch's exported path is routable over the assembled geometry, under the
/// branch's own causal gate seals.
///
/// [`check_critical_path`] quantifies over the DEFAULT playthrough only; a
/// branch-divergent leg — one the fork adds or resequences — was walked by the
/// harness with no compile-time proof behind it. This is the same
/// [`route_visited`] core over the branch's own step list, with `region_events` /
/// `ancestor` in the **branch path's step space**
/// ([`Plan::branch_gate_model`]) — never the default path's indices, which
/// belong to a different sequence.
pub fn check_branch_path(
    world: &World,
    steps: &[Step],
    transports: &[Option<[i32; 3]>],
    region_events: &[RegionEvent],
    ancestor: &dyn Fn(usize, usize) -> bool,
) -> Result<(), NavError> {
    route_visited(
        world,
        &positions_of(steps, transports),
        region_events,
        ancestor,
    )
}

/// The proven A* cell routes of one branch's walked legs — the branch
/// counterpart of [`critical_path_routes`], for export as that branch's waypoint
/// artifact (`validation/branch-waypoints-<branch>.json`). Same leg
/// selection, endpoint snapping and per-leg gate seals as [`check_branch_path`];
/// call it only after that check has succeeded (a leg that fails to snap or
/// route is omitted, which cannot occur once the check has passed).
pub fn branch_path_routes(
    world: &World,
    steps: &[Step],
    transports: &[Option<[i32; 3]>],
    region_events: &[RegionEvent],
    ancestor: &dyn Fn(usize, usize) -> bool,
) -> Vec<LegRoute> {
    route_walked_legs(
        world,
        &positions_of(steps, transports),
        region_events,
        ancestor,
    )
    .into_iter()
    .map(|(leg, _)| leg)
    .collect()
}

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
pub const DW_WAYPOINT_NOT_STANDABLE: DwCode = DwCode::every_version("DW0314");

/// Assert every exported waypoint cell is standable in `world` — the final model the
/// routes were computed over (settled + flooded + fixtures). Returns
/// [`DW_WAYPOINT_NOT_STANDABLE`] (`DW0314`) naming the first offending cell/leg on
/// violation. This is the structural guard the water-flood model exists to make
/// enforceable: a waypoint in a flooded (or newly-walled) cell fails the build
/// loudly instead of stranding the bot at runtime.
pub fn verify_exported_routes(world: &World, routes: &[LegRoute]) -> Result<(), NavError> {
    for leg in routes {
        // The world this leg was PROVEN over, obtained from the leg rather than
        // decided here. `world` is the final assembled model (settled + flooded +
        // fixtures); the leg's own runtime region writes are laid over it, which is
        // exactly the model the A* ran in. Judging `world` bare instead is the
        // second opinion this field exists to remove: a leg the campaign lays floor
        // for is walkable when it is walked and not before, so the bare world calls
        // its cells "no floor" and refuses a route that is correct.
        let leg_world_owned = leg.proven_world(world);
        let leg_world: &World = leg_world_owned.as_ref().unwrap_or(world);
        for &cell in &leg.cells {
            if !leg_world.is_standable(cell) {
                return Err(NavError {
                    code: DW_WAYPOINT_NOT_STANDABLE,
                    message: format!(
                        "critical-path waypoint export: cell {cell:?} on the leg to {to:?} is not \
                         standable in the final assembled world as this leg's runtime region \
                         writes leave it (it is solid, water-flooded, or has no floor). A proven \
                         route must not cross a cell a later pass mutated — this is the \
                         water-flow / post-nav-mutation divergence class: fix the prefab/water or \
                         the assembly, do not move the waypoint. (leg from {from:?})",
                        to = leg.to,
                        from = leg.from,
                    ),
                });
            }
        }
    }
    Ok(())
}

/// `DW0724`: a visual-tier camera's eye cell is occupied (a solid block or water)
/// in the FINAL assembled world — the frame would render the inside of a block
/// instead of the scene, and a picture of the inside of a block is
/// indistinguishable from a picture of a featureless room.
///
/// The property belongs to **a camera**, not to one kind of camera. Every shot
/// `crate::render_plan` derives — spawn, per-piece interior, seam, NPC, interact
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
pub const DW_CAMERA_EYE_OCCLUDED: DwCode = DwCode::every_version("DW0724");

/// One derived camera's eye, as [`verify_camera_eyes`] needs it.
///
/// Built by the derivation ([`crate::render_plan`]) from the same eye position it
/// writes into the shot's `camera`, so a shot cannot carry one camera and offer
/// the proof another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CameraEye {
    /// The shot id the camera belongs to (`seam/keep/0`, `pov/leg0/wp3`, …).
    pub shot_id: String,
    /// The shot's `kind`, so the message can say what to repair.
    pub kind: &'static str,
    /// The integer block the eye sits in (`floor` of the eye position).
    pub cell: [i32; 3],
}

/// Assert every derived camera's eye cell is clear (unoccupied) in `world` — the
/// final assembled model the shots will be rendered from. Returns
/// [`DW_CAMERA_EYE_OCCLUDED`] (`DW0724`) naming the first offending shot on
/// violation. The structural guard behind the visual tier: it is impossible to
/// ship a render plan holding a camera that looks out from inside geometry.
pub fn verify_camera_eyes(world: &World, cameras: &[CameraEye]) -> Result<(), NavError> {
    for cam in cameras {
        if world.is_clear(cam.cell) {
            continue;
        }
        let CameraEye {
            shot_id,
            kind,
            cell,
        } = cam;
        // A `pov` eye is clear by construction (1.62 over a DW0314-proven
        // standable waypoint), so the two verdicts point at different repairs and
        // must not be blurred into one sentence.
        let repair = if *kind == "pov" {
            "The eye sits at 1.62 above a proven standable waypoint, so fix the POV camera \
             derivation (eye height / standing cell) — do NOT move the waypoint or the geometry."
        } else {
            "This camera is placed at a fixed offset from authored geometry, so the finding is \
             that geometry: move what occupies the cell (a hung lantern on the centre column is \
             the recorded case), or move the anchor/seal the camera is derived from. Never nudge \
             the camera to make the picture come out."
        };
        return Err(NavError {
            code: DW_CAMERA_EYE_OCCLUDED,
            message: format!(
                "{kind} shot `{shot_id}`: the camera eye cell {cell:?} is occupied (a solid block \
                 or water) in the assembled world — the frame would render the inside of a block, \
                 not the scene. {repair}"
            ),
        });
    }
    Ok(())
}

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
pub const DW_EDIT_BORDERS_VOID: DwCode = DwCode::every_version("DW0322");

/// How many individual violations a `DW0322` report names before summarising the
/// remainder as a count. A boundary failure is systemic by nature — one stripped
/// berm is hundreds of exposed columns — and hundreds of identical lines are
/// noise, not information (the `DW0354` aggregation precedent, `edit::check_support`).
/// Aborting at the first one instead hid the *scale*, which is the single most
/// useful fact about the failure: "one cell" and "the whole coastline" call for
/// completely different fixes.
const BOUNDARY_LIST_LIMIT: usize = 6;

/// How far past the placed geometry the ocean-stranding search window extends
/// before the sea counts as **open sea** (see [`verify_boundary_safety`]). Any
/// margin ≥ 1 works: the ring beyond the window is untouched ambient water in
/// every direction, so every body that reaches it is one and the same sea.
const OPEN_SEA_MARGIN: i32 = 2;

/// Assert the reachable walk region's boundary is safe (spec-0017 boundary
/// safety; [`DW_EDIT_BORDERS_VOID`]). `starts` are the reachability roots (the
/// plan's resolved anchors, each carrying the piece that declares it — see
/// [`AnchorRoot`], and the same roots the relight pass floods from). Run after
/// every edit batch, and once over the finished world for every campaign that
/// assembles one.
///
/// The premise is the world's [`Ambient`] — what a column the compiler modelled
/// nothing into actually contains — and the rule follows from it:
///
/// **`Ambient::Void`** (unchanged, byte-identical semantics). A neighbour column
/// is a void drop when the player could enter it — its feet and head cells are
/// clear (a closed fence gate counts as enterable: opening it is an
/// adventure-legal right-click) — and **nothing anywhere below** would arrest
/// the fall: no solid, no 1.5-tall barrier top, no gate top, no water. A deep
/// drop onto real geometry is legal (that is falling, not leaving the world);
/// only a bottomless column is an error.
///
/// **`Ambient::Ocean`** — the *stranding* invariant. The superflat's bedrock
/// floor makes every column fall-arresting, so the question is never "can the
/// player fall out" but "can the player get back". The model:
///
/// 1. **Entering.** A reachable walkable cell `c` puts the player in the sea if
///    some horizontally adjacent column is enterable at `c`'s level (feet + head
///    clear of solids and 1.5-tall barriers — water does *not* block walking in)
///    and that column is open, between `c`'s level and the sea surface, all the
///    way to ambient water. Whether the player walks in, wades in or falls from a
///    cliff, they end up afloat: vanilla buoyancy puts a swimmer at the surface
///    plane, `sea.level`.
/// 2. **The sea.** A surface cell (`y == sea.level`) is swimmable when it is not
///    solid/tall and is either ambient water or authored water (a lagoon at sea
///    level is physically the same plane). Surface cells are 4-connected into
///    **bodies**; a body that reaches the edge of the search window is the open
///    sea, and all such bodies are one (the ring beyond the window is untouched
///    ambient water). Connectivity is taken on the surface plane only — a diver
///    might swim under a land bridge into another body, which this model
///    deliberately does not count on.
/// 3. **Climbing out.** A body is escapable when some surface cell of it is
///    horizontally adjacent to a **proven reachable walkable** cell whose feet
///    are at `sea.level` (wade out of the shallows onto a rim one block below the
///    waterline) or at `sea.level + 1` (the canonical beach: land flush with the
///    sea surface). A ledge higher than that is a wall to a swimmer, and a
///    boat/blocks are not available in adventure mode.
///
/// A body the player can enter and cannot climb out of is the violation.
///
/// ## Why the climb-out band stays **cell-level** under partial floor heights
///
/// The step rule reasons in sixteenths ([`World::feet_16_fp`]), but this
/// band deliberately does not. A partial floor can only ever *lower* the standing
/// surface inside its own cell (`feet_16(c) ≤ c.y · 16`), so:
///
/// - inside the band, refining `level` / `level + 1` to a true feet height never
///   flips a verdict — a swimmer climbing onto a slab at `level + 0.5` has an
///   easier exit than onto full ground at `level + 1`, and the body is escapable
///   either way;
/// - the only refinement that *could* flip one is admitting a cell at
///   `level + 2` whose partial support drops its feet back into jump range. That
///   would mark **more** bodies escapable, i.e. weaken the stranding proof.
///
/// So the cell-level band is already the conservative reading of the sixteenth
/// model, and tightening it here could only ever lose a `DW0322` that should
/// fire. The two models compose without interacting: partial heights change
/// *which cells are reachable* (via [`World::neighbors_fp`], feeding `reachable`
/// above), never *what counts as a climb-out*.
/// **The fluid proof runs first, and it runs inside here.**
///
/// `boundary_void`'s per-column fall-arrest scan counts a flooded cell as
/// arrest, so a bottomless column with a waterfall running down it reads as
/// *supported* and this proof goes quiet on exactly the columns the water
/// escaped through. Escaping fluid is therefore a false premise of this proof,
/// the way an unsettled gravity block is of everything downstream of `DW0313`.
///
/// That constraint used to be held by the ORDER OF TWO STATEMENTS at each of the
/// two call sites, plus a comment saying why. Source order is not a mechanism:
/// it is a checklist item that survives only until somebody inserts a third gate
/// into the same function — which is precisely what happened when tiled-zone
/// placement landed, and nothing would have said so. So the sequence is a fact
/// about this proof rather than a fact about its callers, and a caller can no
/// longer get it wrong: there is no order to get wrong.
///
/// The masking is not a corner case that a cleverer fixture could dodge. Under a
/// void horizon the flood model spreads without a floor to stop it, so escaped
/// water reaches essentially every column and silences essentially every hit —
/// which is why no black-box test can tell the two orders apart, and why this is
/// structural instead of a test. [`boundary_only`] is the unsequenced proof,
/// kept so the masking can still be DEMONSTRATED rather than asserted.
pub fn verify_boundary_safety(world: &World, starts: &[AnchorRoot]) -> Result<(), NavError> {
    if let Some(e) = measure_fluid_escape(world).finding() {
        return Err(e);
    }
    boundary_only(world, starts)
}

/// Boundary safety alone, on a world whose fluid has already been accounted for.
///
/// Split out of [`verify_boundary_safety`] for one reason: the masking that
/// makes the sequence load-bearing has to be showable. A test that wants to see
/// this proof go quiet on a flooded world calls this; nothing else should.
fn boundary_only(world: &World, starts: &[AnchorRoot]) -> Result<(), NavError> {
    let reachable = world.reachable_walkable_rooted(starts);
    match &world.ambient {
        Ambient::Void => boundary_void(world, &reachable),
        Ambient::Ocean(sea) => boundary_ocean(world, &reachable, sea),
    }
}

/// Boundary safety under [`Ambient::Void`]: no reachable walkable cell may
/// border a bottomless column. Every violation is collected (see
/// [`BOUNDARY_LIST_LIMIT`]) so one report shows the scale of the breach.
fn boundary_void(world: &World, reachable: &BTreeSet<[i32; 3]>) -> Result<(), NavError> {
    // Per-column lowest fall-arresting cell: solid, tall barrier, use-gate, or
    // flooded — anything vanilla stops a falling player on (or in).
    let mut col_min: BTreeMap<(i32, i32), i32> = BTreeMap::new();
    for set in [&world.solid, &world.tall, &world.use_gates, &world.flooded] {
        for c in set.iter() {
            col_min
                .entry((c[0], c[2]))
                .and_modify(|m| *m = (*m).min(c[1]))
                .or_insert(c[1]);
        }
    }
    // (edge cell, void column entered) pairs, in deterministic BTreeSet order.
    let mut hits: Vec<([i32; 3], [i32; 3])> = Vec::new();
    let mut columns: BTreeSet<(i32, i32)> = BTreeSet::new();
    for &cell in reachable {
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = [cell[0] + dx, cell[1], cell[2] + dz];
            let head = [n[0], n[1] + 1, n[2]];
            // Enterable: feet + head clear of solids/talls/water. A use-gate
            // cell is deliberately enterable (the player can open it and walk
            // through — a gate onto a bottomless drop is exactly the hazard).
            let blocked = |c: [i32; 3]| {
                world.solid.contains(&c) || world.tall.contains(&c) || world.flooded.contains(&c)
            };
            if blocked(n) || blocked(head) {
                continue;
            }
            let has_support = col_min
                .get(&(n[0], n[2]))
                .is_some_and(|&lowest| lowest < n[1]);
            if !has_support {
                hits.push((cell, n));
                columns.insert((n[0], n[2]));
            }
        }
    }
    if hits.is_empty() {
        return Ok(());
    }
    let mut listing = String::new();
    for (cell, n) in hits.iter().take(BOUNDARY_LIST_LIMIT) {
        listing.push_str(&format!("\n  - {cell:?} → void drop at {n:?}"));
    }
    if hits.len() > BOUNDARY_LIST_LIMIT {
        listing.push_str(&format!(
            "\n  - … and {} more",
            hits.len() - BOUNDARY_LIST_LIMIT
        ));
    }
    let first = hits[0].1;
    Err(NavError {
        code: DW_EDIT_BORDERS_VOID,
        message: format!(
            "boundary safety (spec-0017): {} reachable walkable cell(s) border a void drop over \
             {} distinct column(s) — one step off the proven ground falls out of the world:{}\n\
             There is no physical boundary here — either an edit stripped one or the scene ground \
             never had an edge: extend the terrain under the exposed edge (fill/morph a slope or \
             outcrop below {first:?}) or reinstate a barrier shape; do NOT weaken this check or \
             reroute the path to sidestep it",
            hits.len(),
            columns.len(),
            listing,
        ),
    })
}

/// One 4-connected body of sea-surface cells, plus what the walk region does
/// with it (see [`verify_boundary_safety`]'s ocean model).
struct SeaBody {
    /// Reaches the search-window edge ⇒ it is the open sea, and every other
    /// open body is the same water.
    open: bool,
    /// Some surface cell of the body is adjacent to a reachable walkable cell at
    /// `sea.level` or `sea.level + 1`.
    escapable: bool,
    /// Reachable walkable cells from which the player enters this body, in
    /// deterministic order.
    entries: BTreeSet<[i32; 3]>,
    /// A representative surface cell (the smallest, for a stable message).
    sample: [i32; 3],
    /// Surface cells in the body.
    size: usize,
}

/// Boundary safety under [`Ambient::Ocean`]: the stranding invariant. See
/// [`verify_boundary_safety`] for the model this implements.
fn boundary_ocean(
    world: &World,
    reachable: &BTreeSet<[i32; 3]>,
    sea: &Sea,
) -> Result<(), NavError> {
    let level = sea.level;
    let Some(([min_x, min_z], [max_x, max_z])) = ocean_window(world) else {
        return Ok(()); // nothing placed: open sea everywhere, nothing to strand
    };
    let w = (max_x - min_x + 1) as usize;
    let d = (max_z - min_z + 1) as usize;
    let idx = |x: i32, z: i32| (x - min_x) as usize * d + (z - min_z) as usize;
    let inside = |x: i32, z: i32| (min_x..=max_x).contains(&x) && (min_z..=max_z).contains(&z);
    let blocked = |c: [i32; 3]| world.solid.contains(&c) || world.tall.contains(&c);
    let swimmable = |x: i32, z: i32| {
        let c = [x, level, z];
        !blocked(c) && (world.flooded.contains(&c) || world.ambient_water(c))
    };

    // --- label the sea-surface bodies (deterministic scan + BFS) -------------
    const NONE: u32 = u32::MAX;
    let mut label = vec![NONE; w * d];
    let mut bodies: Vec<SeaBody> = Vec::new();
    for x in min_x..=max_x {
        for z in min_z..=max_z {
            if label[idx(x, z)] != NONE || !swimmable(x, z) {
                continue;
            }
            let id = bodies.len() as u32;
            let mut body = SeaBody {
                open: false,
                escapable: false,
                entries: BTreeSet::new(),
                sample: [x, level, z],
                size: 0,
            };
            let mut queue = std::collections::VecDeque::from([(x, z)]);
            label[idx(x, z)] = id;
            while let Some((cx, cz)) = queue.pop_front() {
                body.size += 1;
                if cx == min_x || cx == max_x || cz == min_z || cz == max_z {
                    body.open = true;
                }
                for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, nz) = (cx + dx, cz + dz);
                    if !inside(nx, nz) || label[idx(nx, nz)] != NONE || !swimmable(nx, nz) {
                        continue;
                    }
                    label[idx(nx, nz)] = id;
                    queue.push_back((nx, nz));
                }
            }
            bodies.push(body);
        }
    }
    if bodies.is_empty() {
        return Ok(());
    }

    // --- where the walk region touches the water ----------------------------
    for &cell in reachable {
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = [cell[0] + dx, cell[1], cell[2] + dz];
            // Climb-out: standing at (or one above) the waterline beside water.
            if (cell[1] == level || cell[1] == level + 1) && inside(n[0], n[2]) {
                let id = label[idx(n[0], n[2])];
                if id != NONE {
                    bodies[id as usize].escapable = true;
                }
            }
            // Entry: an enterable neighbour column that is open all the way to
            // the sea surface. Water does not block walking in.
            if blocked(n) || blocked([n[0], n[1] + 1, n[2]]) || !inside(n[0], n[2]) {
                continue;
            }
            let id = label[idx(n[0], n[2])];
            if id == NONE {
                continue;
            }
            let (lo, hi) = if n[1] > level {
                (level + 1, n[1])
            } else {
                (n[1], level)
            };
            if (lo..=hi).any(|y| blocked([n[0], y, n[2]])) {
                continue;
            }
            bodies[id as usize].entries.insert(cell);
        }
    }

    // Every body that reaches the window edge is the same open sea: one climb-out
    // anywhere on the coast serves all of them.
    let open_escapable = bodies.iter().any(|b| b.open && b.escapable);
    let stranding: Vec<&SeaBody> = bodies
        .iter()
        .filter(|b| !b.entries.is_empty() && !b.escapable && !(b.open && open_escapable))
        .collect();
    if stranding.is_empty() {
        return Ok(());
    }

    let shores: usize = stranding.iter().map(|b| b.entries.len()).sum();
    let mut listing = String::new();
    let mut listed = 0usize;
    for b in &stranding {
        for cell in b.entries.iter() {
            if listed == BOUNDARY_LIST_LIMIT {
                break;
            }
            listing.push_str(&format!(
                "\n  - {cell:?} → the sea at {:?} ({})",
                b.sample,
                if b.open { "open sea" } else { "enclosed water" }
            ));
            listed += 1;
        }
    }
    if shores > listed {
        listing.push_str(&format!("\n  - … and {} more", shores - listed));
    }
    let first = stranding[0]
        .entries
        .iter()
        .next()
        .copied()
        .unwrap_or_default();
    Err(NavError {
        code: DW_EDIT_BORDERS_VOID,
        message: format!(
            "boundary safety (spec-0017, `horizon: ocean`): {shores} reachable walkable cell(s) \
             let the player into {} body/bodies of water ({} surface cell(s)) with NO way back \
             ashore — nothing in an ocean world falls out of the world, but a swimmer who cannot \
             climb out is stranded there for the rest of the delve:{}\n\
             A climb-out is a proven-walkable cell at y={level} (a rim one block under the \
             waterline: wade out) or y={} (land flush with the sea surface) beside the water. \
             Give the shoreline near {first:?} such a step — a beach, a bank, a ladder-free \
             landing — or wall the edge off so the player cannot enter the water there; do NOT \
             weaken this check",
            stranding.len(),
            stranding.iter().map(|b| b.size).sum::<usize>(),
            listing,
            level + 1,
        ),
    })
}

/// The x/z window the ocean stranding search runs over: every placed piece and
/// every modelled cell, inflated by [`OPEN_SEA_MARGIN`]. `None` when the world is
/// completely empty. Beyond the window the ambient sea is uniform in every
/// direction, so a body that reaches the edge is the open sea.
fn ocean_window(world: &World) -> Option<([i32; 2], [i32; 2])> {
    let mut lo = [i32::MAX; 2];
    let mut hi = [i32::MIN; 2];
    let mut note = |x: i32, z: i32| {
        lo[0] = lo[0].min(x);
        lo[1] = lo[1].min(z);
        hi[0] = hi[0].max(x);
        hi[1] = hi[1].max(z);
    };
    for (_, (bmin, bmax)) in &world.built {
        note(bmin[0], bmin[2]);
        note(bmax[0], bmax[2]);
    }
    for set in [&world.solid, &world.tall, &world.use_gates, &world.flooded] {
        for c in set.iter() {
            note(c[0], c[2]);
        }
    }
    if lo[0] > hi[0] {
        return None;
    }
    Some((
        [lo[0] - OPEN_SEA_MARGIN, lo[1] - OPEN_SEA_MARGIN],
        [hi[0] + OPEN_SEA_MARGIN, hi[1] + OPEN_SEA_MARGIN],
    ))
}

// ---------------------------------------------------------------------------
// Fluid that leaves the built world (DW0318)
// ---------------------------------------------------------------------------

/// `DW0318`: **a body of fluid runs out of the built world**, stated against the
/// world-generator [`Ambient`] the way [`DW_EDIT_BORDERS_VOID`] already is.
///
/// The piece-level containment rule (`DW0800`, `delve-grammar` /
/// `delve-admit`) proves that every fluid source in a piece has something in
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
/// It is the exact fluid analogue of [`crate::assembled::DW_GRAVITY_DESPAWN`]
/// (`DW0313`), which fails the build when a placed *gravity* block falls out of
/// a void world. The solid case was covered from the beginning; this is the
/// fluid case, and the asymmetry is all that made it a hole rather than a
/// policy.
///
/// Both branches **aggregate**, like `DW0322`: one report per run naming up to
/// [`BOUNDARY_LIST_LIMIT`] cells plus the totals, so a one-cell dribble and a
/// whole coastline pouring into nothing are distinguishable without re-probing.
pub const DW_FLUID_LEAVES_WORLD: DwCode = DwCode::every_version("DW0318");

/// **What the fluid-escape proof looked at**, so the verdict is readable as a
/// measurement rather than as a silence (CLAUDE.md: every validation artifact
/// states its binding count).
///
/// None of these three numbers is the length of the finding list:
/// `pieces` is counted off the **plan** (how much world there is), `fluid_cells`
/// off the **assembled occupancy model** (how much water there is), and only
/// `outside` is this check's own conclusion. A build in which `fluid_cells` is
/// zero examined nothing, and says so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FluidEscape {
    /// The horizon the verdict is stated against — `"void"` or `"ocean"`.
    pub horizon: &'static str,
    /// Placed pieces forming the built volume ([`World::built`]).
    pub pieces: usize,
    /// Every fluid cell in the assembled world: prefab-authored sources plus the
    /// reach the shared flood model gives them
    /// ([`crate::assembled::Occupancy::flooded`]). This is the binding count.
    pub fluid_cells: usize,
    /// The subset of those cells lying outside **every** placed piece's AABB,
    /// sorted (ADR-0006). Under `void` this is the violation set; under `ocean`
    /// it is water that reached the ambient sea, and is reported and not judged.
    pub outside: Vec<[i32; 3]>,
    /// The placed pieces whose AABB touches an escaped cell — where to fix it.
    /// Fluid only ever moves cell to cell, so every escaped cell set is
    /// 6-connected back into the volume it came from; sorted, deduplicated.
    pub from_pieces: Vec<String>,
}

/// Measure where the assembled world's fluid ended up relative to the built
/// volume. Pure: it re-derives nothing, reading `world.flooded` — the one
/// placement-time fluid model this repo has ([`crate::assembled::flood`]'s
/// product, via [`crate::assembled::occupancy_of`]) — and `world.built`.
pub fn measure_fluid_escape(world: &World) -> FluidEscape {
    let outside: Vec<[i32; 3]> = world
        .flooded
        .iter()
        .copied()
        .filter(|&c| !world.is_built(c))
        .collect();
    // Attribution: the piece an escaped cell is 6-adjacent to. Deterministic —
    // `outside` is sorted (it comes from a `BTreeSet`) and the neighbour offsets
    // are a fixed list, collected through a `BTreeSet`.
    const NEIGHBOURS: [[i32; 3]; 6] = [
        [-1, 0, 0],
        [1, 0, 0],
        [0, -1, 0],
        [0, 1, 0],
        [0, 0, -1],
        [0, 0, 1],
    ];
    let mut from: BTreeSet<String> = BTreeSet::new();
    for &c in &outside {
        for d in NEIGHBOURS {
            let n = [c[0] + d[0], c[1] + d[1], c[2] + d[2]];
            for (id, (lo, hi)) in &world.built {
                if (0..3).all(|a| lo[a] <= n[a] && n[a] <= hi[a]) {
                    from.insert(id.clone());
                }
            }
        }
    }
    FluidEscape {
        horizon: world.ambient.name(),
        pieces: world.built.len(),
        fluid_cells: world.flooded.len(),
        outside,
        from_pieces: from.into_iter().collect(),
    }
}

impl FluidEscape {
    /// The `DW0318` violation, or `None`. A finding **only** under
    /// [`Ambient::Void`]: under `ocean` the water met the sea, which is what a
    /// shoreline piece's water is for.
    pub fn finding(&self) -> Option<NavError> {
        if self.horizon != "void" || self.outside.is_empty() {
            return None;
        }
        let columns: BTreeSet<(i32, i32)> = self.outside.iter().map(|c| (c[0], c[2])).collect();
        let lowest = self.outside.iter().map(|c| c[1]).min().unwrap_or(0);
        let sample: Vec<String> = self
            .outside
            .iter()
            .take(BOUNDARY_LIST_LIMIT)
            .map(|c| format!("{c:?}"))
            .collect();
        let more = self.outside.len().saturating_sub(sample.len());
        let blame = if self.from_pieces.is_empty() {
            "no placed piece adjoins them".to_string()
        } else {
            format!("from placed piece(s) {}", self.from_pieces.join(", "))
        };
        Some(NavError {
            code: DW_FLUID_LEAVES_WORLD,
            message: format!(
                "fluid leaves the built world: {n} fluid cell(s) in {cols} column(s) lie outside \
                 every placed piece, {blame}. Under `horizon: void` a column the content did not \
                 build is bottomless, so this water is not a pond that overhangs an edge — it is \
                 a waterfall that runs down forever on the server's own clock, before any player \
                 arrives, and nothing that draws the delve draws it. Cells: {sample}{extra}; the \
                 model stops marking at y={lowest} (the lowest cell it holds), the game does not \
                 stop at all. Examined {cells} fluid cell(s) across {pieces} placed piece(s). \
                 WHERE to fix: the prefab or tileset generator that authored this water, or the \
                 placement that put an open face against nothing — the piece-level rule counts a \
                 run leaving a face and deliberately does not judge it, because only the \
                 placement knows what is on the other side. HOW: wall the face the water runs \
                 out of, pull the body back a cell, place a piece against that face, or declare \
                 `horizon: ocean` if this water is meant to be a sea. Do NOT delete the water to \
                 silence this: an authored pond is first-class content.",
                n = self.outside.len(),
                cols = columns.len(),
                sample = sample.join(", "),
                extra = if more > 0 {
                    format!(" (+{more} more)")
                } else {
                    String::new()
                },
                cells = self.fluid_cells,
                pieces = self.pieces,
            ),
        })
    }

    /// The binding ledger (`validation/fluid-escape.json`): what was examined,
    /// not only what was found. Emitted for every campaign that assembles a
    /// world, so a zero binding is a number a reader can act on rather than a
    /// check nobody notices did nothing.
    pub fn ledger(&self) -> serde_json::Value {
        serde_json::json!({
            "horizon": self.horizon,
            "pieces_examined": self.pieces,
            "fluid_cells_examined": self.fluid_cells,
            "cells_outside_built_volume": self.outside.len(),
            "from_pieces": self.from_pieces,
            "verdict": if self.finding().is_some() { "fail" } else { "pass" },
        })
    }
}

// ---------------------------------------------------------------------------
// spec-0022 — command-driven trap payloads: volley coverage + collapse burial
// ---------------------------------------------------------------------------

/// `DW0442`: a `volley`'s gallery slot has no clear line of fire to a standable
/// cell of its declared kill zone. The compile-time form of the owner's
/// saturation ruling — a volley must BLANKET its zone, so a cell
/// the slot cannot reach is a hole a player could stand in and be safe by
/// accident. Escaping a volley must be a decision (leave the zone), never a
/// lucky step.
pub const DW_VOLLEY_ZONE_UNCOVERED: DwCode = DwCode::every_version("DW0442");
/// `DW0444`: a trap-payload region is unusable — a `volley` kill zone with no
/// standable cell, or a `collapse` region with nothing to drop / nothing to
/// land on.
pub const DW_TRAP_REGION_EMPTY: DwCode = DwCode::every_version("DW0444");
/// `DW0445`: the critical path is not completable once a `collapse` has fired.
pub const DW_COLLAPSE_BURIES_PATH: DwCode = DwCode::every_version("DW0445");
/// `DW0446`: a `volley`'s `from_anchor` cell is not clear, so the projectile
/// would be summoned inside solid geometry and never leave it.
pub const DW_VOLLEY_SLOT_OCCLUDED: DwCode = DwCode::every_version("DW0446");

/// Height above a kill-zone cell's floor a volley aims at: centre mass of a
/// standing player (a 1.8-tall hitbox with feet on the floor). Aiming at the
/// centre rather than the feet means the shot passes through the hitbox for the
/// whole cell rather than grazing it.
const VOLLEY_AIM_HEIGHT: f64 = 1.0;

/// Speed of a summoned volley projectile, in blocks/tick.
///
/// Arrow impact damage in 1.21.11 is `ceil(|velocity| * damage)` with `damage`
/// defaulting to 2.0, so 2.5 b/t lands 5 half-hearts per arrow — a real
/// consequence that three salvos of saturating fire can kill, without any one
/// arrow being an instant death.
pub const VOLLEY_SPEED: f64 = 2.5;

/// One computed shot of a volley: the kill-zone cell it covers and the exact
/// `Motion` vector that carries a projectile from the gallery slot into it.
#[derive(Debug, Clone, PartialEq)]
pub struct VolleyShot {
    /// The standable kill-zone cell this shot covers.
    pub cell: [i32; 3],
    /// The `Motion` NBT vector, in blocks/tick.
    pub motion: [f64; 3],
}

/// A proven volley: every standable cell of the kill zone, each with the
/// velocity vector that reaches it.
#[derive(Debug, Clone)]
pub struct VolleyGeometry {
    /// The gallery slot cell the projectiles are summoned in.
    pub from: [i32; 3],
    /// One shot per standable kill-zone cell, in ascending cell order
    /// (deterministic — ADR-0006; no RNG anywhere in the pattern).
    pub shots: Vec<VolleyShot>,
}

/// The exact world-space point a volley projectile is summoned at.
pub fn volley_source(from: [i32; 3]) -> [f64; 3] {
    let c = cell_center(from);
    [c[0], c[1], c[2]]
}

/// The exact world-space point a volley shot aims at for kill-zone cell `c`.
pub fn volley_target(c: [i32; 3]) -> [f64; 3] {
    let p = cell_center(c);
    [p[0], p[1] - 0.5 + VOLLEY_AIM_HEIGHT, p[2]]
}

impl World {
    /// Whether a cell stops a projectile. Collision geometry stops it outright;
    /// water is included because it destroys a flat trajectory rather than
    /// merely slowing it — a shot that has to swim is not a shot that arrives.
    ///
    /// Deliberately NOT `blocks_camera`: glass is transparent to a camera and
    /// solid to an arrow, so reusing the sight predicate would prove coverage
    /// through a window the projectile cannot pass.
    fn blocks_projectile(&self, c: [i32; 3]) -> bool {
        self.is_occupied(c)
    }

    /// The first cell that stops a projectile flying `from` → `to`, or `None`
    /// when the line of fire is clear. The origin cell is exempt: that is where
    /// the projectile is summoned.
    ///
    /// Uses the same [`walk_cells`] traversal as the cutscene clip and the mob
    /// line-of-sight, so "can this be traversed" has one definition in the
    /// compiler. Critically, the ray checked here is *exactly* the segment the
    /// emitted `Motion` vector flies (projectiles are summoned `NoGravity`, and
    /// drag scales speed without turning the path), so the proof and the runtime
    /// cannot drift apart.
    fn first_projectile_block(&self, from: [f64; 3], to: [f64; 3]) -> Option<[i32; 3]> {
        let origin = [
            from[0].floor() as i32,
            from[1].floor() as i32,
            from[2].floor() as i32,
        ];
        walk_cells(from, to, |c| c != origin && self.blocks_projectile(c))
    }

    /// Whether this cell is clear enough to summon a projectile in.
    pub fn is_volley_slot_clear(&self, c: [i32; 3]) -> bool {
        !self.blocks_projectile(c)
    }
}

/// Plan a volley, proving saturation by construction (spec-0022).
///
/// The returned geometry contains one shot per standable kill-zone cell — so
/// emitting every shot IS the coverage — and the function errors rather than
/// returning partial cover. `label` names the volley in diagnostics.
pub fn plan_volley(
    world: &World,
    from: [i32; 3],
    region: ([i32; 3], [i32; 3]),
    label: &str,
) -> Result<VolleyGeometry, NavError> {
    if !world.is_volley_slot_clear(from) {
        return Err(NavError {
            code: DW_VOLLEY_SLOT_OCCLUDED,
            message: format!(
                "{label}: the `from_anchor` cell [{}, {}, {}] is solid or flooded, so a \
                 summoned projectile would never leave it. Move the gallery slot into the \
                 open air of the firing niche (the anchor marks where the projectile \
                 spawns, not the wall it comes out of)",
                from[0], from[1], from[2]
            ),
        });
    }
    let src = volley_source(from);
    let mut shots = Vec::new();
    // BTreeSet-ordered cells: the pattern is a pure function of the geometry,
    // with no RNG and no hash-order iteration (ADR-0006).
    let cells: Vec<[i32; 3]> = crate::assembled::region_cells(region.0, region.1)
        .filter(|c| world.is_standable(*c))
        .collect();
    if cells.is_empty() {
        return Err(NavError {
            code: DW_TRAP_REGION_EMPTY,
            message: format!(
                "{label}: the `kill_zone` region [{}, {}, {}]..[{}, {}, {}] contains no \
                 standable cell, so there is nothing for the volley to saturate — it would \
                 fire into geometry no player can occupy. Point `kill_zone` at the floor \
                 players actually cross (the stair treads, the corridor run), not at the \
                 wall or the air above it",
                region.0[0], region.0[1], region.0[2], region.1[0], region.1[1], region.1[2]
            ),
        });
    }
    for cell in cells {
        let dst = volley_target(cell);
        if let Some(block) = world.first_projectile_block(src, dst) {
            return Err(NavError {
                code: DW_VOLLEY_ZONE_UNCOVERED,
                message: format!(
                    "{label}: the gallery slot [{}, {}, {}] has no line of fire to \
                     kill-zone cell [{}, {}, {}] — the shot is stopped at [{}, {}, {}]. A \
                     volley must BLANKET its kill zone: an uncovered cell is a pocket a \
                     player is safe in by accident, which turns dodging from a decision \
                     into luck. Either clear the obstruction, move `from_anchor` where it \
                     sees the whole zone, or shrink `kill_zone` to the part it does cover",
                    from[0],
                    from[1],
                    from[2],
                    cell[0],
                    cell[1],
                    cell[2],
                    block[0],
                    block[1],
                    block[2]
                ),
            });
        }
        let d = [dst[0] - src[0], dst[1] - src[1], dst[2] - src[2]];
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        let motion = if len <= f64::EPSILON {
            [0.0, 0.0, 0.0]
        } else {
            [
                d[0] / len * VOLLEY_SPEED,
                d[1] / len * VOLLEY_SPEED,
                d[2] / len * VOLLEY_SPEED,
            ]
        };
        shots.push(VolleyShot { cell, motion });
    }
    Ok(VolleyGeometry { from, shots })
}

/// A proven collapse: what falls, and where it comes to rest.
#[derive(Debug, Clone)]
pub struct CollapseGeometry {
    /// The region whose blocks are deleted.
    pub region: ([i32; 3], [i32; 3]),
    /// Cells that currently hold a block — one `falling_block` summon each, in
    /// ascending cell order.
    pub drops: Vec<[i32; 3]>,
    /// Where the debris settles, in ascending cell order. This is the geometry
    /// the completability proof treats as solid.
    pub debris: Vec<[i32; 3]>,
    /// The tallest fall, in blocks — drives the `then_floor` paving delay.
    pub max_fall: i32,
}

/// Plan a collapse and settle its debris deterministically (spec-0022).
///
/// Settling reuses the assembled model's rule — a falling block comes to rest on
/// the first solid cell beneath it, stacking within its own column — so the
/// post-collapse world the proof reasons over is the world the server will
/// actually have.
pub fn plan_collapse(
    world: &World,
    blocks: &BTreeMap<[i32; 3], String>,
    region: ([i32; 3], [i32; 3]),
    label: &str,
) -> Result<CollapseGeometry, NavError> {
    let drops: Vec<[i32; 3]> = crate::assembled::region_cells(region.0, region.1)
        .filter(|c| blocks.contains_key(c))
        .collect();
    if drops.is_empty() {
        return Err(NavError {
            code: DW_TRAP_REGION_EMPTY,
            message: format!(
                "{label}: the `collapse` region [{}, {}, {}]..[{}, {}, {}] contains no \
                 blocks, so nothing would fall. Point `region_anchor` at the ceiling slab \
                 that caves in, not at the air below it",
                region.0[0], region.0[1], region.0[2], region.1[0], region.1[1], region.1[2]
            ),
        });
    }
    let lo_y = region.0[1].min(region.1[1]);
    // Group the drops by column so a stack settles as a stack.
    let mut by_col: BTreeMap<[i32; 2], usize> = BTreeMap::new();
    for c in &drops {
        *by_col.entry([c[0], c[2]]).or_insert(0) += 1;
    }
    let mut debris: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut max_fall = 0;
    let mut landed_any = false;
    for (col, n) in by_col {
        // Find the first solid cell below the region in this column: the debris
        // rests on top of it. Search stops at the world floor.
        let mut rest: Option<i32> = None;
        let mut y = lo_y - 1;
        while y > lo_y - MAX_COLLAPSE_FALL {
            if world.is_solid([col[0], y, col[1]]) {
                rest = Some(y + 1);
                break;
            }
            y -= 1;
        }
        let Some(base) = rest else { continue };
        landed_any = true;
        max_fall = max_fall.max(lo_y - base);
        for k in 0..n as i32 {
            debris.insert([col[0], base + k, col[1]]);
        }
    }
    if !landed_any {
        return Err(NavError {
            code: DW_TRAP_REGION_EMPTY,
            message: format!(
                "{label}: nothing beneath the `collapse` region [{}, {}, {}]..[{}, {}, {}] \
                 stops the debris within {MAX_COLLAPSE_FALL} blocks — the falling blocks \
                 would drop out of the box garden instead of burying anyone. Put the \
                 region over the floor the players walk on",
                region.0[0], region.0[1], region.0[2], region.1[0], region.1[1], region.1[2]
            ),
        });
    }
    Ok(CollapseGeometry {
        region,
        drops,
        debris: debris.into_iter().collect(),
        max_fall,
    })
}

/// How far debris is allowed to fall before the compiler calls the collapse
/// unmodellable. Well beyond any box-garden room height.
const MAX_COLLAPSE_FALL: i32 = 64;

/// Prove the critical path survives every collapse (spec-0022, `DW0445`).
///
/// A trap can always fire — the player WILL step on the plate — so the world the
/// completability proof must hold in is the world after the collapse, not
/// before. This is the same pessimism the `shortcut` seal applies (a shortcut is
/// proven never-taken; a trap is proven always-sprung), and it is deliberately
/// conservative in one direction: the debris is added as solid geometry while
/// the deleted region is left in place, so the proof can only ever be stricter
/// than the real post-collapse world, never laxer.
pub fn check_collapses(
    plan: &Plan,
    world: &World,
    collapses: &[(String, CollapseGeometry)],
) -> Result<(), NavError> {
    for (label, g) in collapses {
        let debris: BTreeSet<[i32; 3]> = g.debris.iter().copied().collect();
        let collapsed = world.with_sealed(&debris);
        if let Err(e) = check_critical_path(plan, &collapsed) {
            return Err(NavError {
                code: DW_COLLAPSE_BURIES_PATH,
                message: format!(
                    "{label}: the critical path is no longer completable once this collapse \
                     has fired — the debris buries the route ({}). A trap is proven in its \
                     SPRUNG state, because a player will step on the trigger: either leave a \
                     way through the rubble, drop fewer layers (a shallower `region_anchor`), \
                     or move the collapse off the forced path",
                    e.message
                ),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat solid floor at `y-1` over `[0,w) × [0,d)`, with the given interior
    /// cells at `y` set solid (obstacles). Cells at `y` not listed are open air.
    fn floored(w: i32, d: i32, y: i32, walls: &[[i32; 3]]) -> World {
        let mut solid = BTreeSet::new();
        for x in 0..w {
            for z in 0..d {
                solid.insert([x, y - 1, z]); // floor
                solid.insert([x, y + 2, z]); // ceiling (headroom = y, y+1)
            }
        }
        for &c in walls {
            solid.insert(c);
        }
        World::from_solid_cells(solid)
    }

    /// The linear "every earlier step is an ancestor" gate-ordering used by the
    /// synthetic gate tests (no parallel branches). Production routing uses the
    /// campaign's real DAG-causal predicate (`Plan::gate_fired_before`).
    fn linear(g: usize, s: usize) -> bool {
        g < s
    }

    /// One boundary-proof root whose declaring "piece" is the whole synthetic
    /// world. These fixtures build a single free-standing shape a few blocks
    /// across and test the void/ocean MODEL, so the piece AABB is that shape —
    /// stated explicitly rather than left open, because an unbounded root is the
    /// exact defect [`AnchorRoot`] exists to prevent and no production path may
    /// construct one.
    fn roots(at: [i32; 3]) -> [AnchorRoot; 1] {
        [AnchorRoot {
            at,
            within: ([-64, 0, -64], [64, 128, 64]),
        }]
    }

    // -----------------------------------------------------------------------
    // DW0318 — fluid that leaves the built world
    // -----------------------------------------------------------------------

    /// A 3x3 solid plate at y=63 with a water source standing on its `+x` edge
    /// column, and a built volume covering exactly the plate. Vanilla runs that
    /// source off the plate and down: the shape of every shoreline piece placed
    /// against nothing.
    fn plate_with_a_source_at_the_edge() -> World {
        let mut blocks: BTreeMap<[i32; 3], String> = BTreeMap::new();
        for x in 0..3 {
            for z in 0..3 {
                blocks.insert([x, 63, z], "minecraft:stone".to_string());
            }
        }
        blocks.insert([2, 64, 1], "minecraft:water".to_string());
        let occ = crate::assembled::occupancy_of(blocks, &BTreeSet::new());
        World::from_occupancy(occ)
    }

    /// The plate's own AABB — one "piece" covering the built cells and nothing
    /// beyond them.
    fn plate_built() -> Vec<(String, Bbox)> {
        vec![("prefab/plate".to_string(), ([0, 63, 0], [2, 64, 2]))]
    }

    /// **The whole rule, both horizons, one geometry.** The piece is identical;
    /// only the world-generator premise changes, exactly as `DW0322` is stated.
    #[test]
    fn fluid_off_the_built_world_is_a_finding_under_void_and_not_under_ocean_dw0318() {
        // void: the water runs off the plate into columns nothing built.
        let void = plate_with_a_source_at_the_edge().with_ambient(Ambient::Void, plate_built());
        let m = measure_fluid_escape(&void);
        assert_eq!(m.horizon, "void");
        assert_eq!(m.pieces, 1, "the built volume is the plate");
        assert!(
            m.fluid_cells > m.outside.len(),
            "the binding count is the world's water ({}), not the finding list ({})",
            m.fluid_cells,
            m.outside.len()
        );
        assert!(!m.outside.is_empty(), "water left the plate");
        assert_eq!(
            m.from_pieces,
            vec!["prefab/plate".to_string()],
            "the escape is attributed to the piece it came from"
        );
        let err = m.finding().expect("a void world does not hold this water");
        assert_eq!(err.code, DW_FLUID_LEAVES_WORLD);
        assert!(
            err.message.contains("DW0318") || err.code.id() == "DW0318",
            "the code is DW0318"
        );
        assert!(
            err.message.contains("fluid leaves the built world"),
            "names the hazard:\n{}",
            err.message
        );

        // ocean: the same water meets the sea it depicts. Same cells, no finding.
        let ocean = plate_with_a_source_at_the_edge().with_ambient(
            Ambient::Ocean(Sea {
                level: 62,
                floor_top: 54,
            }),
            plate_built(),
        );
        let o = measure_fluid_escape(&ocean);
        assert_eq!(o.horizon, "ocean");
        assert_eq!(
            o.outside, m.outside,
            "the geometry is identical — only the premise moved"
        );
        assert!(
            o.finding().is_none(),
            "an ocean horizon puts sea under every column the content did not build"
        );
    }

    /// Water that stays inside the pieces is not a finding, under either
    /// horizon: a walled pond is first-class content, and the piece's own
    /// `DW0800` already governs it.
    #[test]
    fn fluid_contained_within_the_built_volume_is_not_a_finding() {
        let mut blocks: BTreeMap<[i32; 3], String> = BTreeMap::new();
        // A 5x5 stone dish with a 3x3 rim: the source cannot get out.
        for x in 0..5 {
            for z in 0..5 {
                blocks.insert([x, 63, z], "minecraft:stone".to_string());
            }
        }
        for x in 0..5 {
            for z in 0..5 {
                if (1..4).contains(&x) && (1..4).contains(&z) {
                    continue;
                }
                blocks.insert([x, 64, z], "minecraft:stone".to_string());
            }
        }
        blocks.insert([2, 64, 2], "minecraft:water".to_string());
        let occ = crate::assembled::occupancy_of(blocks, &BTreeSet::new());
        let world = World::from_occupancy(occ).with_ambient(
            Ambient::Void,
            vec![("prefab/dish".to_string(), ([0, 63, 0], [4, 64, 4]))],
        );
        let m = measure_fluid_escape(&world);
        assert!(m.fluid_cells > 0, "the check BOUND to this world's water");
        assert!(m.outside.is_empty(), "the dish holds it: {:?}", m.outside);
        assert!(m.finding().is_none());
        assert_eq!(m.ledger()["verdict"], "pass");
        assert_eq!(m.ledger()["fluid_cells_examined"], m.fluid_cells);
    }

    /// Water that runs from one piece into the piece placed against it is the
    /// deferral's other answer, and it must be a pass: the neighbour's own bytes
    /// decide those cells, and the neighbour's own `DW0800` governs them.
    #[test]
    fn fluid_running_into_the_piece_next_door_is_that_pieces_business() {
        let mut blocks: BTreeMap<[i32; 3], String> = BTreeMap::new();
        for x in 0..6 {
            for z in 0..3 {
                blocks.insert([x, 63, z], "minecraft:stone".to_string());
            }
        }
        // A rim around the whole 6x3 slab so nothing leaves the pair.
        for x in -1..7 {
            for z in -1..4 {
                if (0..6).contains(&x) && (0..3).contains(&z) {
                    continue;
                }
                blocks.insert([x, 63, z], "minecraft:stone".to_string());
                blocks.insert([x, 64, z], "minecraft:stone".to_string());
                blocks.insert([x, 65, z], "minecraft:stone".to_string());
            }
        }
        blocks.insert([2, 64, 1], "minecraft:water".to_string());
        let occ = crate::assembled::occupancy_of(blocks, &BTreeSet::new());
        let world = World::from_occupancy(occ).with_ambient(
            Ambient::Void,
            vec![
                ("prefab/west".to_string(), ([-1, 63, -1], [2, 65, 3])),
                ("prefab/east".to_string(), ([3, 63, -1], [6, 65, 3])),
            ],
        );
        let m = measure_fluid_escape(&world);
        assert_eq!(m.pieces, 2);
        assert!(m.fluid_cells > 1, "the water spread across the seam");
        assert!(
            m.outside.is_empty(),
            "water in the piece next door is that piece's business: {:?}",
            m.outside
        );
    }

    /// **The escape hatch a synthetic world could have opened, closed by
    /// direction.** A `World` with no built volume knows of no content at all,
    /// so it cannot prove any water contained. It must therefore report EVERY
    /// fluid cell as outside — fail closed — rather than report none and pass.
    /// The opposite default is the one that would have shipped: an empty list
    /// read as "everything is inside".
    #[test]
    fn a_world_with_no_built_volume_proves_nothing_contained() {
        let world = plate_with_a_source_at_the_edge();
        let m = measure_fluid_escape(&world);
        assert_eq!(m.pieces, 0, "nothing declared where the content is");
        assert_eq!(
            m.outside.len(),
            m.fluid_cells,
            "with no built volume, no cell can be shown contained"
        );
        assert!(
            m.from_pieces.is_empty(),
            "and nothing can be blamed for it either"
        );
        assert!(m.finding().is_some(), "fails closed under void");
    }

    /// **Why `DW0318` runs before `DW0322`, demonstrated rather than asserted.**
    ///
    /// `boundary_void` counts a flooded cell as fall-arrest, so a bottomless
    /// column with a waterfall running down it reads as *supported*. The same
    /// plate is a `DW0322` void drop when it is dry and passes `DW0322` when its
    /// edge is leaking — the escaping water hides the hole it made. Only
    /// `DW0318` sees it, which is why it is asked first.
    #[test]
    fn escaping_water_masks_the_boundary_proof_which_is_why_it_runs_first() {
        // Dry: every rim cell borders a bottomless column → DW0322.
        let mut dry: BTreeMap<[i32; 3], String> = BTreeMap::new();
        for x in 0..3 {
            for z in 0..3 {
                dry.insert([x, 63, z], "minecraft:stone".to_string());
            }
        }
        let dry_world = World::from_occupancy(crate::assembled::occupancy_of(
            dry.clone(),
            &BTreeSet::new(),
        ))
        .with_ambient(Ambient::Void, plate_built());
        let err = verify_boundary_safety(&dry_world, &roots([1, 64, 1]))
            .expect_err("a dry plate edge is a void drop");
        assert_eq!(err.code, DW_EDIT_BORDERS_VOID);

        // Wet: one source on the plate floods every surrounding column, and
        // `col_min` now finds arrest in each of them.
        let mut wet = dry;
        wet.insert([1, 64, 1], "minecraft:water".to_string());
        let wet_world =
            World::from_occupancy(crate::assembled::occupancy_of(wet, &BTreeSet::new()))
                .with_ambient(Ambient::Void, plate_built());
        assert!(
            boundary_only(&wet_world, &roots([1, 64, 1])).is_ok(),
            "the leak silences the boundary proof — this is the masking, not a pass"
        );
        assert!(
            measure_fluid_escape(&wet_world).finding().is_some(),
            "and DW0318 is the only proof left that sees it"
        );
        // …which is why the sequence is inside `verify_boundary_safety` rather
        // than at its call sites: the same world, through the entry point every
        // caller uses, reports the leak instead of the silence. A caller cannot
        // put these two in the wrong order because there is no order left to
        // put them in.
        let err = verify_boundary_safety(&wet_world, &roots([1, 64, 1]))
            .expect_err("the sequenced entry point reports the leak");
        assert_eq!(err.code, DW_FLUID_LEAVES_WORLD);
    }

    /// Boundary safety (spec-0017 invariant 4): a walkable platform edge whose
    /// neighbour column has NOTHING below is a void drop → `DW0322`; ringing the
    /// platform with a 2-high (unjumpable) rim, or giving the neighbour column
    /// real geometry anywhere below (a deep drop onto land is falling, not
    /// leaving the world), passes.
    #[test]
    fn boundary_safety_flags_a_walkable_edge_over_void_dw0322() {
        // A bare 3×3 platform: every rim cell borders bottomless columns.
        let mut solid = BTreeSet::new();
        for x in 0..3 {
            for z in 0..3 {
                solid.insert([x, 63, z]);
            }
        }
        let world = World::from_solid_cells(solid);
        let err =
            verify_boundary_safety(&world, &roots([1, 64, 1])).expect_err("edge borders void");
        assert_eq!(err.code, DW_EDIT_BORDERS_VOID);
        assert!(err.message.contains("void drop"), "names the hazard");
    }

    #[test]
    fn boundary_safety_accepts_rimmed_platforms_and_deep_drops() {
        // (a) The same platform ringed by a 2-high rim (feet + head blocked, and
        // the rim top is +2 — unclimbable, so it never joins the walkable set).
        let mut solid = BTreeSet::new();
        for x in 0..3 {
            for z in 0..3 {
                solid.insert([x, 63, z]);
            }
        }
        for x in -1..4 {
            for z in -1..4 {
                if (0..3).contains(&x) && (0..3).contains(&z) {
                    continue;
                }
                solid.insert([x, 64, z]);
                solid.insert([x, 65, z]);
            }
        }
        let world = World::from_solid_cells(solid);
        verify_boundary_safety(&world, &roots([1, 64, 1])).expect("a 2-high rim holds the line");

        // (b) A single floor cell whose four neighbour columns all have geometry
        // far below: a deep drop is legal (falling, not leaving the world).
        let mut solid = BTreeSet::new();
        solid.insert([0, 63, 0]);
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            solid.insert([dx, 10, dz]);
        }
        let world = World::from_solid_cells(solid);
        verify_boundary_safety(&world, &roots([0, 64, 0])).expect("deep drops onto land are legal");
    }

    /// Anchor SEATING, not the boundary model ([`AnchorRoot`]). A ceiling anchor —
    /// which every spec-0022 `collapse` payload must declare — is two blocks from
    /// the cell on top of the roof and three from the floor of its own room, so an
    /// unconfined nearest-standable snap seats it on the ROOF, a component no
    /// player can walk to. Confining the snap to the declaring piece puts it back
    /// on the floor. Both halves are asserted here: the confined seating is the
    /// fix, the unconfined one is the defect it replaces.
    #[test]
    fn a_ceiling_anchor_seats_in_its_room_not_on_the_roof() {
        // A 7×7 room: floor y=63, ceiling y=68, walls between them.
        let mut solid = BTreeSet::new();
        for x in 0..7 {
            for z in 0..7 {
                solid.insert([x, 63, z]);
                solid.insert([x, 68, z]);
                for y in 64..68 {
                    if x == 0 || x == 6 || z == 0 || z == 6 {
                        solid.insert([x, y, z]);
                    }
                }
            }
        }
        let world = World::from_solid_cells(solid);
        let ceiling_anchor = [3, 67, 3];
        let (floor, roof) = ([3, 64, 3], [3, 69, 3]);

        // The defect: the nearest standable cell by squared distance is the roof
        // (Δy 2) rather than the room's own floor (Δy 3) — a solid ceiling in
        // between counts for nothing.
        let loose = world.reachable_walkable(&[ceiling_anchor]);
        assert!(
            loose.contains(&roof),
            "unconfined snap climbs onto the roof"
        );
        assert!(
            !loose.contains(&floor),
            "and never reaches the room it was declared in"
        );

        // The fix: the piece AABB (y 63..=68) excludes the roof cell entirely.
        let seated = world.reachable_walkable_rooted(&[AnchorRoot {
            at: ceiling_anchor,
            within: ([0, 63, 0], [6, 68, 6]),
        }]);
        assert!(seated.contains(&floor), "confined snap seats in the room");
        assert!(!seated.contains(&roof), "the roof is out of the piece");
    }

    // -----------------------------------------------------------------------
    // DW0322 aggregation + the ocean horizon's stranding invariant
    // -----------------------------------------------------------------------

    /// `DW0322` reports **every** violation of a run, not the first: a stripped
    /// boundary is systemic, and the scale of the breach is the most useful fact
    /// about it. The bare 3×3 platform exposes 12 edge/void pairs over 12 columns
    /// (4 corners × 2 + 4 edges × 1); the message counts all of them and lists
    /// [`BOUNDARY_LIST_LIMIT`] before summarising the rest.
    #[test]
    fn boundary_safety_aggregates_every_void_drop_dw0322() {
        let mut solid = BTreeSet::new();
        for x in 0..3 {
            for z in 0..3 {
                solid.insert([x, 63, z]);
            }
        }
        let world = World::from_solid_cells(solid);
        let err =
            verify_boundary_safety(&world, &roots([1, 64, 1])).expect_err("edge borders void");
        assert_eq!(err.code, DW_EDIT_BORDERS_VOID);
        assert!(
            err.message.contains("12 reachable walkable cell(s)"),
            "counts every violation, not just the first:\n{}",
            err.message
        );
        assert!(
            err.message.contains("12 distinct column(s)"),
            "counts the exposed columns:\n{}",
            err.message
        );
        assert_eq!(
            err.message.matches("void drop at").count(),
            BOUNDARY_LIST_LIMIT,
            "listing is bounded:\n{}",
            err.message
        );
        assert!(
            err.message.contains("and 6 more"),
            "summarises the tail:\n{}",
            err.message
        );
    }

    /// The `ocean` horizon's ambient: sea level 62, sea floor 54 (the pinned
    /// superflat `crate::plan::SEA_LEVEL` / `SEA_FLOOR_TOP_Y`), with `covered`
    /// standing in for the placed pieces' AABBs.
    fn ocean(solid: BTreeSet<[i32; 3]>, flooded: BTreeSet<[i32; 3]>, covered: Vec<Bbox>) -> World {
        World::from_solid_and_flooded(solid, flooded).with_ambient(
            Ambient::Ocean(Sea {
                level: 62,
                floor_top: 54,
            }),
            built(covered),
        )
    }

    /// A built volume from bare AABBs, naming each box after its index — a
    /// synthetic stand-in for the prefab ids [`built_volume`] reads off a plan.
    fn built(boxes: Vec<Bbox>) -> Vec<(String, Bbox)> {
        boxes
            .into_iter()
            .enumerate()
            .map(|(i, b)| (format!("piece-{i}"), b))
            .collect()
    }

    /// An inclusive world AABB, as [`World::built`] carries it.
    type Bbox = ([i32; 3], [i32; 3]);

    /// A `size`×`size` island of one solid plate whose top block is at `top`,
    /// inside a piece AABB spanning y 60..=`top`.
    fn island(size: i32, top: i32) -> (BTreeSet<[i32; 3]>, Vec<Bbox>) {
        let mut solid = BTreeSet::new();
        for x in 0..size {
            for z in 0..size {
                for y in 60..=top {
                    solid.insert([x, y, z]);
                }
            }
        }
        (solid, vec![([0, 60, 0], [size - 1, top, size - 1])])
    }

    /// Ocean horizon (spec-0013), the false-premise fix: the pinned superflat
    /// puts bedrock under *every* column, so a coastline is not a void drop —
    /// the identical geometry is `DW0322` under `horizon: void` and clean under
    /// `horizon: ocean`, because its shore is a canonical beach (land top flush
    /// with the sea surface, walk plane at `sea_level + 1`).
    #[test]
    fn boundary_safety_ocean_beach_is_not_a_void_drop_dw0322() {
        let (solid, covered) = island(8, 62);
        let voidish = World::from_solid_cells(solid.clone());
        let err = verify_boundary_safety(&voidish, &roots([3, 63, 3]))
            .expect_err("under `void` the same coast IS a void drop");
        assert_eq!(err.code, DW_EDIT_BORDERS_VOID);
        assert!(err.message.contains("void drop"));

        let sea = ocean(solid, BTreeSet::new(), covered);
        verify_boundary_safety(&sea, &roots([3, 63, 3]))
            .expect("an ocean beach is swimming, not falling out of the world");
    }

    /// The ocean horizon's replacement invariant: a sheer-cliff coast with no
    /// climb-out anywhere strands the player who steps off it, and that is
    /// `DW0322` — with every shore cell aggregated, not the first one.
    #[test]
    fn boundary_safety_ocean_sheer_cliff_strands_the_player_dw0322() {
        let (solid, covered) = island(8, 70);
        let world = ocean(solid, BTreeSet::new(), covered);
        let err = verify_boundary_safety(&world, &roots([3, 71, 3]))
            .expect_err("a sheer coast cannot be re-climbed");
        assert_eq!(err.code, DW_EDIT_BORDERS_VOID);
        assert!(
            err.message.contains("NO way back ashore"),
            "names the stranding hazard:\n{}",
            err.message
        );
        assert!(
            err.message.contains("open sea"),
            "names the water body:\n{}",
            err.message
        );
        // 8×8 plateau: 28 distinct rim cells touch the water (a corner counts
        // once — the report is per shore *cell*, not per cell/direction pair).
        assert!(
            err.message.contains("28 reachable walkable cell(s)"),
            "aggregates every shore cell:\n{}",
            err.message
        );
        assert!(
            err.message.contains("and 22 more"),
            "bounded listing + tail count:\n{}",
            err.message
        );
    }

    /// The other admitted shoreline profile: a rim one block **under** the
    /// waterline (walk plane at `sea_level`, wade out of the shallows). Both it
    /// and the flush beach pass; a lip two blocks above the surface does not.
    #[test]
    fn boundary_safety_ocean_admits_a_rim_below_the_waterline() {
        let (solid, covered) = island(8, 61);
        let world = ocean(solid, BTreeSet::new(), covered);
        verify_boundary_safety(&world, &roots([3, 62, 3])).expect("wade out of the shallows");

        // One block higher than the flush beach: the swimmer faces a wall.
        let (solid, covered) = island(8, 64);
        let world = ocean(solid, BTreeSet::new(), covered);
        let err = verify_boundary_safety(&world, &roots([3, 65, 3]))
            .expect_err("a lip 2 above the surface is not a climb-out");
        assert_eq!(err.code, DW_EDIT_BORDERS_VOID);
    }

    /// Stranding is proven **per body of water**, not globally: an island whose
    /// outer coast is a perfect beach still fails if it contains an inner pool
    /// the player can walk into and not climb out of. A global "is there a
    /// climb-out anywhere" test would pass this world.
    #[test]
    fn boundary_safety_ocean_enclosed_pool_is_checked_separately_dw0322() {
        // Outer plate: top 62 (flush beach, walk plane 63) over 0..=12.
        let mut solid = BTreeSet::new();
        for x in 0..=12 {
            for z in 0..=12 {
                for y in 60..=62 {
                    solid.insert([x, y, z]);
                }
            }
        }
        // Inner plateau one step up (top 63, walk plane 64) over 3..=9, with a
        // 3×3 shaft at 5..=7 down to a pool at sea level.
        let mut flooded = BTreeSet::new();
        for x in 3..=9 {
            for z in 3..=9 {
                if (5..=7).contains(&x) && (5..=7).contains(&z) {
                    solid.remove(&[x, 62, z]);
                    flooded.insert([x, 62, z]);
                } else {
                    solid.insert([x, 63, z]);
                }
            }
        }
        let world = ocean(solid, flooded, vec![([0, 60, 0], [12, 63, 12])]);
        let err = verify_boundary_safety(&world, &roots([1, 63, 1]))
            .expect_err("the inner pool has 2-high walls all round");
        assert_eq!(err.code, DW_EDIT_BORDERS_VOID);
        assert!(
            err.message.contains("enclosed water"),
            "names the enclosed body, not the (escapable) open sea:\n{}",
            err.message
        );
        assert!(
            !err.message.contains("open sea"),
            "the outer beach is proven fine and must not be reported:\n{}",
            err.message
        );
        assert!(
            err.message.contains("1 body/bodies of water"),
            "exactly one failing body:\n{}",
            err.message
        );

        // Lower the inner plateau to the outer datum and the pool is flush with
        // its bank: the player steps straight back out.
        let mut solid = BTreeSet::new();
        for x in 0..=12 {
            for z in 0..=12 {
                for y in 60..=62 {
                    solid.insert([x, y, z]);
                }
            }
        }
        let mut flooded = BTreeSet::new();
        for x in 5..=7 {
            for z in 5..=7 {
                solid.remove(&[x, 62, z]);
                flooded.insert([x, 62, z]);
            }
        }
        let world = ocean(solid, flooded, vec![([0, 60, 0], [12, 62, 12])]);
        verify_boundary_safety(&world, &roots([1, 63, 1]))
            .expect("a flush pool is a puddle, not a trap");
    }

    /// A non-talk-to visited position (test convenience for `route_visited`).
    /// A room `w × d` split by a wall at `z = zw` with a single doorway at
    /// `x = gx`, and (optionally) a second permanent opening at `x = bx` — the
    /// walk-around. Ceilinged, so a body standing in the doorway cannot be
    /// climbed over.
    fn two_room_world(w: i32, d: i32, y: i32, zw: i32, gx: i32, bypass: Option<i32>) -> World {
        let mut walls = Vec::new();
        for x in 0..w {
            if x == gx || Some(x) == bypass {
                continue;
            }
            walls.push([x, y, zw]);
            walls.push([x, y + 1, zw]);
        }
        floored(w, d, y, &walls)
    }

    /// A hostile force for the `DW0478` proof. A "lane path cell" carries the
    /// [`LANE_MARCH_DRIFT`] margin exactly as [`aggro_sources`] assigns it;
    /// every stationary cell carries none.
    fn src(id: &str, radius: f64, cells: &[(&'static str, [i32; 3])]) -> AggroSource {
        AggroSource {
            id: id.to_string(),
            radius,
            radius_source: "the wave's declared `follow_range`",
            cells: cells
                .iter()
                .map(|(what, cell)| {
                    let drift = if *what == "lane path cell" {
                        LANE_MARCH_DRIFT
                    } else {
                        0.0
                    };
                    (*what, *cell, drift)
                })
                .collect(),
        }
    }

    /// A permanently-reigning bonfire at `pos` — what every pre-existing `DW0478`
    /// case is, so those tests read exactly as they did before the proof learned
    /// about plain checkpoints.
    fn fire(anchor: &str, pos: [i32; 3]) -> RestPoint {
        RestPoint {
            anchor: anchor.to_string(),
            kind: "bonfire",
            pos,
            reign_end: None,
        }
    }

    /// A plain `set-checkpoint` that stops governing at `reign_end`.
    fn checkpoint(anchor: &str, pos: [i32; 3], reign_end: usize) -> RestPoint {
        RestPoint {
            anchor: anchor.to_string(),
            kind: "set-checkpoint",
            pos,
            reign_end: Some(reign_end),
        }
    }

    /// No force declares an onset, so every one is conservatively live from step 0.
    fn from_the_start() -> BTreeMap<String, usize> {
        BTreeMap::new()
    }

    /// `DW0478`: a bonfire inside a wave's perception radius. The party respawns
    /// into contact — a soft-lock, not a difficulty choice — so this is an error,
    /// and the message must name the clearance the geometry is short by.
    #[test]
    fn a_bonfire_inside_an_aggro_radius_is_dw0478() {
        let bonfires = vec![fire("anchor/chapel", [34, 71, -113])];
        let sources = vec![src(
            "wave/gate-assault",
            16.0,
            &[("seated spawn cell", [34, 71, -103])],
        )];
        let err = verify_respawn_safe_zone(&bonfires, &sources, &from_the_start())
            .expect_err("a fire 10 blocks inside a 16-block perception radius is a soft-lock");
        assert_eq!(err.code, DW_RESPAWN_IN_AGGRO); // DW0478
        assert!(
            err.message.contains("anchor/chapel") && err.message.contains("wave/gate-assault"),
            "the message names both sides of the violation: {}",
            err.message
        );
        assert!(
            err.message.contains("10.0 blocks"),
            "the message states the measured distance: {}",
            err.message
        );
        assert!(
            err.message.contains("Do NOT shrink `follow_range`"),
            "the prescription must not offer retuning the fight as a fix: {}",
            err.message
        );
    }

    /// The LANE half of the same rule: the squad's seated cells can be far away
    /// and the fire still be unsafe, because a lane wave walks its polyline while
    /// the party is elsewhere. This is the drowned-bell shape — a fire beside the
    /// end of a siege lane.
    #[test]
    fn a_bonfire_beside_a_lane_path_is_dw0478() {
        let bonfires = vec![fire("anchor/l2-bonfire", [34, 71, -113])];
        let sources = vec![src(
            "wave/gate-assault",
            16.0,
            &[
                ("seated spawn cell", [12, 71, -84]),
                ("lane path cell", [24, 71, -110]),
            ],
        )];
        let err = verify_respawn_safe_zone(&bonfires, &sources, &from_the_start())
            .expect_err("the lane reaches it");
        assert_eq!(err.code, DW_RESPAWN_IN_AGGRO);
        assert!(
            err.message.contains("lane path cell"),
            "the message must say it is the MARCH that reaches the fire, not the seating: {}",
            err.message
        );
    }

    /// The DRIFT half of the lane term: a fire that
    /// clears the centre-line polyline by less than the measured marching drift
    /// is still inside the squad's real aggro reach, because the squad marches a
    /// corridor around the polyline (td-routing-spike dossier: followers max 7.9
    /// blocks off-lane). This is the drowned bell's chapel fire — 18.0 blocks
    /// from a 16-`follow_range` lane, and run nine died to it live at 17.7.
    #[test]
    fn a_bonfire_clearing_the_centre_line_but_not_the_march_corridor_is_dw0478() {
        let bonfires = vec![fire("anchor/chapel", [18, 64, 0])];
        let sources = vec![src(
            "wave/bell-siege",
            16.0,
            &[("lane path cell", [0, 64, 0])],
        )];
        let err = verify_respawn_safe_zone(&bonfires, &sources, &from_the_start())
            .expect_err("18.0 blocks clears follow_range 16 but not 16 + 7.9 drift");
        assert_eq!(err.code, DW_RESPAWN_IN_AGGRO); // DW0478
        assert!(
            err.message.contains("marching drift") && err.message.contains("td-routing-spike"),
            "the message must name the drift term and its constraint source: {}",
            err.message
        );
        assert!(
            err.message.contains("23.9"),
            "the message states the full reach (16 + 7.9): {}",
            err.message
        );
    }

    /// The drift margin belongs to the MARCH alone: a stationary seated cell at
    /// the same 18.0 blocks from the same 16-block radius is legal, because a
    /// force that never walks has no corridor around a polyline it never marches.
    #[test]
    fn a_stationary_cell_at_the_same_distance_carries_no_drift_margin() {
        let bonfires = vec![fire("anchor/chapel", [18, 64, 0])];
        let sources = vec![src(
            "wave/bell-siege",
            16.0,
            &[("seated spawn cell", [0, 64, 0])],
        )];
        assert!(
            verify_respawn_safe_zone(&bonfires, &sources, &from_the_start()).is_ok(),
            "the drift term is specifically for lane-marching squads"
        );
    }

    /// Clearance strictly greater than the radius is legal — the rule is
    /// "must exceed", so the boundary itself is not a violation.
    #[test]
    fn a_bonfire_outside_every_aggro_radius_is_clean() {
        let bonfires = vec![fire("anchor/beach", [0, 64, 0])];
        let sources = vec![
            src("wave/near", 8.0, &[("seated spawn cell", [9, 64, 0])]),
            src("wave/far", 16.0, &[("lane path cell", [0, 64, 40])]),
        ];
        assert!(verify_respawn_safe_zone(&bonfires, &sources, &from_the_start()).is_ok());
    }

    /// A campaign with no rest point proves nothing here: the rule is about where
    /// the party is DELIVERED, and without a bonfire nothing delivers them.
    #[test]
    fn hostiles_without_a_bonfire_are_not_the_safe_zone_proof_s_business() {
        let sources = vec![src(
            "wave/anything",
            64.0,
            &[("seated spawn cell", [0, 64, 0])],
        )];
        assert!(verify_respawn_safe_zone(&[], &sources, &from_the_start()).is_ok());
    }

    /// **The sibling case, and the whole point of `bell-08`.** The identical
    /// geometry that is `DW0478` for a bonfire is `DW0478` for a plain
    /// `set-checkpoint`: the party is delivered onto that cell by the same
    /// vanilla `spawnpoint`, so the hazard belongs to the CELL. For nineteen-plus
    /// island rounds this proof examined zero objects on a campaign with three
    /// checkpoints, because it filtered on `rest == true`.
    #[test]
    fn a_plain_set_checkpoint_inside_an_aggro_radius_is_dw0478() {
        let rest = vec![checkpoint("anchor/checkpoint-3", [34, 71, -113], 99)];
        let sources = vec![src(
            "actor/polyphemus-blinded",
            16.0,
            &[("staging anchor", [34, 71, -103])],
        )];
        let err = verify_respawn_safe_zone(&rest, &sources, &from_the_start()).expect_err(
            "a set-checkpoint 10 blocks inside a 16-block radius is the same soft-lock",
        );
        assert_eq!(err.code, DW_RESPAWN_IN_AGGRO); // DW0478
        assert!(
            err.message.contains("anchor/checkpoint-3")
                && err
                    .message
                    .contains("the same for a plain `set-checkpoint`"),
            "the message must say the rule does not care which verb placed the cell: {}",
            err.message
        );
    }

    /// The reign model, in the direction that makes it honest: a force first
    /// staged AFTER a plain checkpoint has been replaced can never meet the party
    /// there, so it is not compared. This is not a relaxation of the geometry —
    /// the same pair at the same distance IS a violation while both are live.
    #[test]
    fn a_replaced_checkpoint_is_not_measured_against_a_body_staged_later() {
        let rest = vec![checkpoint("anchor/checkpoint-1", [34, 71, -113], 7)];
        let sources = vec![src(
            "wave/storm-shore",
            48.0,
            &[("seated spawn cell", [34, 71, -103])],
        )];
        let late: BTreeMap<String, usize> = [("wave/storm-shore".to_string(), 12)].into();
        assert!(
            verify_respawn_safe_zone(&rest, &sources, &late).is_ok(),
            "a checkpoint retired at step 7 cannot deliver anybody to a wave first seated at 12"
        );
        assert!(
            verify_respawn_safe_zone(&rest, &sources, &from_the_start()).is_err(),
            "the SAME geometry is a violation the moment the two are contemporaneous — the \
             window narrows what is compared, never what is demanded of a compared pair"
        );
    }

    /// A bonfire never stops reigning, so it is compared against a force staged
    /// at any step whatsoever — byte-for-byte the behaviour before the window
    /// existed.
    #[test]
    fn a_bonfire_is_compared_against_a_force_staged_at_any_later_step() {
        let rest = vec![fire("anchor/chapel", [34, 71, -113])];
        let sources = vec![src(
            "wave/gate-assault",
            16.0,
            &[("seated spawn cell", [34, 71, -103])],
        )];
        let late: BTreeMap<String, usize> = [("wave/gate-assault".to_string(), 999)].into();
        assert_eq!(
            verify_respawn_safe_zone(&rest, &sources, &late)
                .expect_err("a fire the party can return to forever meets everything")
                .code,
            DW_RESPAWN_IN_AGGRO
        );
    }

    /// The binding count is published, and a zero says WHY. A proof that examined
    /// nothing is the vacuity this whole ledger exists to break, so the artifact
    /// must never be able to look like a pass.
    #[test]
    fn the_ledger_states_its_binding_count_and_names_a_zero() {
        let sources = vec![src("wave/x", 8.0, &[("seated spawn cell", [0, 64, 0])])];
        let bound = RespawnSafetyLedger::new(
            &[
                fire("anchor/a", [99, 64, 0]),
                checkpoint("anchor/b", [98, 64, 0], 5),
            ],
            &sources,
            &from_the_start(),
        );
        assert_eq!(bound.pairs, 2, "two rest points x one force");
        assert!(!bound.unbound() && bound.reason().is_none());

        let no_rest = RespawnSafetyLedger::new(&[], &sources, &from_the_start());
        assert!(no_rest.unbound());
        assert!(
            no_rest.reason().unwrap().contains("no `set-checkpoint`"),
            "a zero must name which half of the proof was missing"
        );

        let no_hostiles =
            RespawnSafetyLedger::new(&[fire("anchor/a", [0, 64, 0])], &[], &from_the_start());
        assert!(no_hostiles.unbound());
        assert!(
            no_hostiles
                .reason()
                .unwrap()
                .contains("no hostile force at all")
        );

        // The third zero, and the one a reader would otherwise never suspect:
        // both halves exist and no pair is ever contemporaneous.
        let never_meet = RespawnSafetyLedger::new(
            &[checkpoint("anchor/b", [0, 64, 0], 3)],
            &sources,
            &[("wave/x".to_string(), 9)].into(),
        );
        assert!(never_meet.unbound());
        assert!(
            never_meet.reason().unwrap().contains("at the same time"),
            "a campaign whose respawn points and hostiles never coexist is a real zero, and it \
             is named rather than reported as a pass: {:?}",
            never_meet.reason()
        );
    }

    /// A sentinel parked in the only doorway between two beats: with its aggro
    /// radius blocked, the forced walk no longer routes. There is nothing to
    /// walk around it by, so "optional" is a lie — `DW0380`, at warning tier.
    #[test]
    fn an_optional_elite_in_the_only_doorway_is_dw0380() {
        let world = two_room_world(12, 9, 65, 4, 6, None);
        let elites = vec![("wave/sentinel".to_string(), [6, 65, 4], 2)];
        let diags = verify_optional_elites(
            &world,
            &elites,
            &[vp_at([1, 65, 1], 0), vp_at([1, 65, 7], 1)],
        );
        assert_eq!(diags.len(), 1, "expected one finding: {diags:#?}");
        assert_eq!(diags[0].code, DW_OPTIONAL_ELITE_UNAVOIDABLE); // DW0380
        assert_eq!(
            diags[0].severity,
            delvewright_dsl::Severity::Warning,
            "spec-0016 §7 is the design-contract section: this measures, it does not gate"
        );
        assert!(
            diags[0].message.contains("Tree Sentinel"),
            "the finding must not read as 'no optional enemies' — the pattern is legitimate, \
             only the missing walk-around is the problem: {}",
            diags[0].message
        );
    }

    /// The same sentinel with a second door far enough away to stay outside its
    /// aggro radius: the walk-around exists, so the Tree Sentinel pattern stands
    /// and nothing is reported.
    #[test]
    fn an_optional_elite_with_a_walk_around_is_legitimate() {
        let world = two_room_world(12, 9, 65, 4, 6, Some(0));
        let elites = vec![("wave/sentinel".to_string(), [6, 65, 4], 2)];
        let diags = verify_optional_elites(
            &world,
            &elites,
            &[vp_at([1, 65, 1], 0), vp_at([1, 65, 7], 1)],
        );
        assert!(
            diags.is_empty(),
            "a route around the sentinel is all the engine asks for: {diags:#?}"
        );
    }

    /// A beat the party is required to STAND on, inside the aggro radius, is
    /// contested ground by design — a landed "live threat" pattern, not a missing
    /// bypass. The lint is about the route, never the destination.
    #[test]
    fn an_elite_seated_on_a_beat_is_contested_ground_not_a_missing_bypass() {
        let world = two_room_world(12, 9, 65, 4, 6, None);
        let elites = vec![("wave/threat".to_string(), [1, 65, 1], 4)];
        let diags = verify_optional_elites(
            &world,
            &elites,
            &[vp_at([1, 65, 1], 0), vp_at([1, 65, 7], 1)],
        );
        assert!(
            diags.is_empty(),
            "an objective inside the fight is design, not a defect: {diags:#?}"
        );
    }

    /// A visited critical position at a given step (spec-0016 §7 lints select on
    /// `src_step`, which the plain `vp` helper always leaves at 0).
    fn vp_at(pos: [i32; 3], src_step: usize) -> VisitedPos {
        VisitedPos {
            pos,
            transport_before: false,
            talk_to: false,
            src_step,
        }
    }

    /// A rest point 4 blocks from the next beat is a real retry loop: 16 ticks
    /// back, well inside the 60 s budget. No warning.
    #[test]
    fn a_close_rest_point_is_within_the_retry_budget() {
        let world = corridor(400, 65);
        let rests = vec![("anchor/fire".to_string(), [0, 65, 1], 0usize, true)];
        let diags = verify_retry_cost(&world, &rests, &[vp_at([4, 65, 1], 1)]);
        assert!(diags.is_empty(), "4 blocks is not a commute: {diags:#?}");
    }

    /// A rest point 350 blocks from the beat it respawns into is 70 s of walking
    /// back on every death — `DW0379`, at warning tier.
    #[test]
    fn a_distant_rest_point_is_dw0379() {
        let world = corridor(400, 65);
        let rests = vec![("anchor/fire".to_string(), [0, 65, 1], 0usize, true)];
        let diags = verify_retry_cost(&world, &rests, &[vp_at([350, 65, 1], 1)]);
        assert_eq!(diags.len(), 1, "one finding expected: {diags:#?}");
        assert_eq!(diags[0].code, DW_RETRY_COST); // DW0379
        assert_eq!(
            diags[0].severity,
            delvewright_dsl::Severity::Warning,
            "retry cost is a judgement the compiler measures but must not overrule"
        );
        assert!(
            diags[0].message.contains("bonfire `anchor/fire`"),
            "the message names the rest point: {}",
            diags[0].message
        );
    }

    /// The budget is measured to the FIRST beat after the rest point, not the
    /// last — a rest point followed immediately by its beat is cheap even if the
    /// delve runs on for hundreds of blocks afterwards.
    #[test]
    fn retry_cost_measures_the_first_beat_after_the_rest_point() {
        let world = corridor(400, 65);
        let rests = vec![("anchor/fire".to_string(), [0, 65, 1], 0usize, false)];
        let diags = verify_retry_cost(
            &world,
            &rests,
            &[vp_at([2, 65, 1], 1), vp_at([390, 65, 1], 2)],
        );
        assert!(
            diags.is_empty(),
            "the far LATER beat must not be charged to this rest point: {diags:#?}"
        );
    }

    fn vp(pos: [i32; 3], transport_before: bool) -> VisitedPos {
        VisitedPos {
            pos,
            transport_before,
            talk_to: false,
            src_step: 0,
        }
    }

    /// Whether an entity of `width` standing (feet) at `p` has any part of its AABB
    /// inside a solid cell. Height 1.95 (the player/villager box).
    fn aabb_clips(world: &World, p: [f64; 3], width: f64) -> bool {
        let span = |c: f64| {
            (
                (c - width / 2.0).floor() as i32,
                (c + width / 2.0 - 1e-9).floor() as i32,
            )
        };
        let (x0, x1) = span(p[0]);
        let (z0, z1) = span(p[2]);
        let (y0, y1) = (p[1].floor() as i32, (p[1] + 1.95 - 1e-9).floor() as i32);
        (x0..=x1).any(|x| (z0..=z1).any(|z| (y0..=y1).any(|y| world.solid_at([x, y, z]))))
    }

    /// The full walked path for `cells`, as the emitter would teleport it.
    fn walked(cells: &[[i32; 3]]) -> Vec<[f64; 3]> {
        resample(cells, DEFAULT_SPEED)
    }

    /// **Regression (owner, island QA): "the NPC visibly passes through blocks".**
    ///
    /// A 1-wide corridor with solid walls on both sides. Every planned waypoint must
    /// keep the mover's whole AABB out of the walls. The bare integer cell
    /// coordinate — what the emitter used before `cell_center` — puts 70 % of a
    /// 0.6-wide body inside the neighbouring column, i.e. inside the wall, for the
    /// entire walk; the second half of this test asserts exactly that, so the defect
    /// cannot silently come back.
    #[test]
    fn walked_path_keeps_the_body_out_of_corridor_walls() {
        let y = 65;
        let mut walls = Vec::new();
        for z in 0..8 {
            for dy in 0..2 {
                walls.push([0, y + dy, z]); // west wall
                walls.push([2, y + dy, z]); // east wall
            }
        }
        let world = floored(3, 8, y, &walls);
        let cells: Vec<[i32; 3]> = (0..8).map(|z| [1, y, z]).collect();
        let path = world
            .find_path(cells[0], *cells.last().unwrap())
            .expect("the corridor is walkable");
        assert_eq!(path, cells, "a 1-wide corridor has exactly one route");

        for w in walked(&path) {
            assert!(
                !aabb_clips(&world, w, 0.6),
                "waypoint {w:?} puts the body inside a corridor wall"
            );
        }
        // The pre-fix emission (bare cell coordinates) DID clip — the defect this
        // test guards. Keep as the counter-example, never as the behaviour.
        assert!(
            aabb_clips(&world, [1.0, y as f64, 3.0], 0.6),
            "a body at the bare integer cell straddles the wall columns"
        );
    }

    /// An L-shaped corridor whose inside corner is solid. A* is strictly cardinal
    /// (`neighbors_fp` offers 4 horizontal moves, never a diagonal), so no path can
    /// cut the corner; this pins that property *and* proves the interpolated body
    /// never enters the corner block on the turn.
    #[test]
    fn corner_turn_routes_around_the_corner_block_not_through_it() {
        let y = 65;
        // Open cells: the column z=1..=4 at x=1, then x=1..=4 at z=4. Everything
        // else at head height is solid, including the inside corner [2, y, 1].
        let open: BTreeSet<[i32; 3]> = (1..=4)
            .map(|z| [1, y, z])
            .chain((1..=4).map(|x| [x, y, 4]))
            .collect();
        let mut walls = Vec::new();
        for x in 0..6 {
            for z in 0..6 {
                for dy in 0..2 {
                    if !open.contains(&[x, y, z]) {
                        walls.push([x, y + dy, z]);
                    }
                }
            }
        }
        let world = floored(6, 6, y, &walls);
        let path = world
            .find_path([1, y, 1], [4, y, 4])
            .expect("the L-corridor is walkable");
        assert!(
            path.iter().all(|c| open.contains(c)),
            "the route must stay in the open cells: {path:?}"
        );
        for w in walked(&path) {
            assert!(
                !aabb_clips(&world, w, 0.6),
                "waypoint {w:?} clips the corner block"
            );
        }
    }

    /// A one-block step up is interpolated as an **L** (rise over the source column,
    /// then cross), not a diagonal lerp: a straight line between the two cell centres
    /// drags the body through the corner of the step block. Both legs stay inside
    /// cells the neighbour rule already proved clear (`standable_fp` + the jump
    /// head-clearance check), so the AABB never enters the step.
    #[test]
    fn vertical_step_is_l_shaped_and_never_clips_the_step_block() {
        let y = 65;
        let mut solid = BTreeSet::new();
        for x in 0..4 {
            for z in 0..3 {
                solid.insert([x, y - 1, z]); // lower floor
                solid.insert([x, y + 4, z]); // ceiling, clear of both levels
            }
        }
        // A raised ledge at x∈{2,3}: its top face is the upper walking surface.
        for x in [2, 3] {
            for z in 0..3 {
                solid.insert([x, y, z]);
            }
        }
        let world = World::from_solid_cells(solid);
        let path = world
            .find_path([1, y, 1], [3, y + 1, 1])
            .expect("a one-block step up is walkable");
        assert_eq!(path, vec![[1, y, 1], [2, y + 1, 1], [3, y + 1, 1]]);

        let pts = walked(&path);
        // The rise happens over the SOURCE column: some waypoint sits at the source
        // cell's centre already at the upper height.
        let src = cell_center([1, y, 1]);
        assert!(
            pts.iter()
                .any(|w| w[0] == src[0] && w[2] == src[2] && w[1] > y as f64),
            "the step up must rise in place before crossing: {pts:?}"
        );
        for w in &pts {
            assert!(
                !aabb_clips(&world, *w, 0.6),
                "waypoint {w:?} clips the step block"
            );
        }
    }

    fn eye(shot_id: &str, kind: &'static str, cell: [i32; 3]) -> CameraEye {
        CameraEye {
            shot_id: shot_id.to_string(),
            kind,
            cell,
        }
    }

    #[test]
    fn pov_camera_in_open_air_passes_but_inside_a_block_is_dw0724() {
        // A flat floor at y=64 with headroom; the eye of a standing player is at
        // y=65..66 (clear). A camera eye in a clear cell passes; one placed inside
        // the floor block is DW0724.
        let world = floored(5, 5, 65, &[]);
        assert!(world.is_clear([2, 65, 2]), "standing eye cell is clear");
        // Clear eye → ok.
        verify_camera_eyes(&world, &[eye("pov/leg0/wp0", "pov", [2, 65, 2])])
            .expect("clear eye ok");
        // Eye buried in the solid floor → DW0724.
        let err = verify_camera_eyes(&world, &[eye("pov/leg0/wp1", "pov", [2, 64, 2])])
            .expect_err("occupied eye must fail");
        assert_eq!(err.code, DW_CAMERA_EYE_OCCLUDED);
        assert!(err.message.contains("pov/leg0/wp1"), "names the shot");
    }

    /// The widening this code exists for: the identical fact about a camera that
    /// is NOT the player's own eye. A seam camera stands one cell under the
    /// ceiling on the tile's centre column — where a hanging lantern is — and
    /// before this binding reached it the frame was one flat colour and no build
    /// in the repository said anything.
    #[test]
    fn a_seam_camera_inside_a_ceiling_block_is_dw0724_too() {
        let world = floored(5, 5, 65, &[[2, 67, 2]]);
        verify_camera_eyes(&world, &[eye("seam/keep/0", "seam", [2, 66, 2])])
            .expect("a clear seam eye passes");
        let err = verify_camera_eyes(&world, &[eye("seam/keep/0", "seam", [2, 67, 2])])
            .expect_err("a seam eye inside the hung block must fail");
        assert_eq!(err.code, DW_CAMERA_EYE_OCCLUDED);
        assert!(
            err.message.contains("seam shot `seam/keep/0`"),
            "{}",
            err.message
        );
        // The two kinds prescribe different repairs, and the message must not
        // send a seam author looking at a waypoint they do not have.
        assert!(
            !err.message.contains("waypoint"),
            "a non-pov verdict must not blame the waypoint derivation: {}",
            err.message
        );
    }

    #[test]
    fn path_routes_around_a_wall_corner() {
        // A wall spanning z=0..2 at x=2 forces a detour around its open end at z=2.
        let world = floored(5, 4, 65, &[[2, 65, 0], [2, 65, 1], [2, 65, 2]]);
        let path = world.find_path([0, 65, 0], [4, 65, 0]).expect("routable");
        assert_eq!(path.first(), Some(&[0, 65, 0]));
        assert_eq!(path.last(), Some(&[4, 65, 0]));
        // The detour must have turned a corner: it cannot be a straight x-line.
        assert!(
            path.iter().any(|c| c[2] >= 3),
            "path must round the wall's open end, got {path:?}"
        );
        // No waypoint sits inside the wall.
        for c in &path {
            assert!(!world.is_solid(*c), "path clips wall at {c:?}");
        }
    }

    #[test]
    fn disconnected_floors_are_unroutable() {
        // Two floor patches with a void gap (no floor at x=2) → DW0307 condition.
        let mut solid = BTreeSet::new();
        for x in [0, 1, 3, 4] {
            for z in 0..3 {
                solid.insert([x, 64, z]);
                solid.insert([x, 67, z]);
            }
        }
        let world = World::from_solid_cells(solid);
        assert!(world.standable([0, 65, 1]));
        assert!(world.standable([4, 65, 1]));
        assert!(world.find_path([0, 65, 1], [4, 65, 1]).is_none());
    }

    #[test]
    fn snap_finds_floor_in_front_of_a_solid_affordance() {
        // A solid altar block at the target; the nearest standable cell is beside
        // it (the NPC walks up to it, not into it).
        let world = floored(5, 5, 65, &[[2, 65, 2]]);
        assert!(world.snap_standable([2, 65, 2], 0).is_none());
        let snapped = world.snap_standable([2, 65, 2], 2).expect("floor nearby");
        assert!(world.standable(snapped));
        assert!((snapped[0] - 2).abs() + (snapped[2] - 2).abs() <= 1);
    }

    #[test]
    fn snap_none_when_fully_embedded() {
        // A solid cell walled in by solids within the radius → no floor to snap to.
        let mut solid = BTreeSet::new();
        for dx in -2..=2 {
            for dy in -2..=2 {
                for dz in -2..=2 {
                    solid.insert([10 + dx, 65 + dy, 10 + dz]);
                }
            }
        }
        let world = World::from_solid_cells(solid);
        assert!(world.snap_standable([10, 65, 10], 2).is_none());
    }

    #[test]
    fn cutscene_clip_detects_a_solid_on_the_dolly_and_passes_clean_air() {
        // A solid pillar at [2,66,1]; a dolly through it clips, one beside it does
        // not.
        let world = floored(5, 4, 65, &[[2, 66, 1]]);
        let through = [[0.5, 66.5, 1.5], [4.5, 66.5, 1.5]];
        assert_eq!(first_clip(&world, &through), Some((0, [2, 66, 1])));
        let clear = [[0.5, 66.5, 3.5], [4.5, 66.5, 3.5]];
        assert_eq!(first_clip(&world, &clear), None);
    }

    #[test]
    fn critical_path_unroutable_leg_is_dw0311() {
        // Two standable floor patches separated by a void gap (no floor at x=2):
        // a walked leg across them is DW0311; the same leg guarded by a transport
        // hop (transport_before = true) is skipped.
        let mut solid = BTreeSet::new();
        for x in [0, 1, 3, 4] {
            for z in 0..3 {
                solid.insert([x, 64, z]);
                solid.insert([x, 67, z]);
            }
        }
        let world = World::from_solid_cells(solid);
        let a = [0, 65, 1];
        let b = [4, 65, 1];
        assert!(world.standable(a) && world.standable(b));
        // Walked leg → unroutable → DW0311.
        let err = route_visited(&world, &[vp(a, false), vp(b, false)], &[], &linear).unwrap_err();
        assert_eq!(err.code, DW_CRITICAL_UNROUTABLE);
        // Same leg ridden by an inter-area transport → skipped, ok.
        assert!(route_visited(&world, &[vp(a, false), vp(b, true)], &[], &linear).is_ok());
    }

    #[test]
    fn talk_to_endpoint_excludes_the_npc_cell_and_flooded_cells() {
        // A flat floor; the NPC anchor cell is standable (a mannequin stands on the
        // floor), and one adjacent cell is flooded. The talk-to goal snap must NOT
        // return the NPC's own cell, and must skip the flooded neighbour — it lands
        // on a dry standable cell beside the NPC, within interaction range.
        let mut solid = BTreeSet::new();
        for x in 0..5 {
            for z in 0..5 {
                solid.insert([x, 64, z]); // floor at y=64, standable at y=65
            }
        }
        let npc = [2, 65, 2];
        let flooded: BTreeSet<[i32; 3]> = [[1, 65, 2]].into_iter().collect(); // west neighbour is water
        let world = World::from_solid_and_flooded(solid, flooded);
        assert!(
            world.standable(npc),
            "the NPC cell itself is standable in the model"
        );
        let goal = world
            .snap_endpoint(npc, true)
            .expect("a dry standable cell beside the NPC exists");
        assert_ne!(goal, npc, "must not stand on the NPC's own (occupied) cell");
        assert!(
            !world.flooded.contains(&goal),
            "must not stand in water: {goal:?}"
        );
        assert!(world.standable(goal));
        // …and it is within interaction range (adjacent) of the NPC.
        let d2 = (0..3).map(|i| (goal[i] - npc[i]).pow(2)).sum::<i32>();
        assert!(
            d2 <= SNAP_RADIUS * SNAP_RADIUS,
            "goal {goal:?} within range of NPC"
        );
    }

    #[test]
    fn verify_exported_routes_rejects_a_flooded_waypoint_dw0314() {
        // Synthetic negative for the DW0314 self-check: a hand-built leg
        // whose polyline crosses a flooded cell must fail the standability guard.
        let mut solid = BTreeSet::new();
        for x in 0..4 {
            solid.insert([x, 64, 0]); // floor
        }
        let flooded: BTreeSet<[i32; 3]> = [[2, 65, 0]].into_iter().collect(); // a water tongue on the route
        let world = World::from_solid_and_flooded(solid, flooded);
        let routes = vec![LegRoute {
            from: [0, 65, 0],
            to: [3, 65, 0],
            to_step: 1,
            cells: vec![[0, 65, 0], [1, 65, 0], [2, 65, 0], [3, 65, 0]],
            use_gates: Vec::new(),
            // No runtime write on this leg: the bare world is the world it was
            // proven over, so the flooded cell has nothing to explain it away.
            region_state: RegionState::default(),
        }];
        let err = verify_exported_routes(&world, &routes).unwrap_err();
        assert_eq!(err.code, DW_WAYPOINT_NOT_STANDABLE);
        assert!(
            err.message.contains("[2, 65, 0]"),
            "names the offending cell: {}",
            err.message
        );
        // A route entirely on dry standable floor passes.
        let dry = vec![LegRoute {
            from: [0, 65, 0],
            to: [1, 65, 0],
            to_step: 1,
            cells: vec![[0, 65, 0], [1, 65, 0]],
            use_gates: Vec::new(),
            region_state: RegionState::default(),
        }];
        assert!(verify_exported_routes(&world, &dry).is_ok());
    }

    #[test]
    fn critical_path_routable_leg_passes() {
        // A flat connected floor: consecutive visited cells are walkable → ok.
        let world = floored(6, 3, 65, &[]);
        assert!(
            route_visited(
                &world,
                &[vp([0, 65, 1], false), vp([5, 65, 1], false)],
                &[],
                &linear
            )
            .is_ok()
        );
    }

    #[test]
    fn confined_cells_are_standable_distinct_and_ordered_by_distance() {
        // A 5×5 floored room. Placement floods standable cells from the anchor.
        let world = floored(5, 5, 65, &[]);
        let bounds = ([0, 64, 0], [4, 66, 4]);
        let cells = world.confined_standable_cells([2, 65, 2], bounds);
        // Every returned cell is standable, and all are distinct.
        for c in &cells {
            assert!(world.standable(*c), "non-standable cell {c:?}");
        }
        let uniq: BTreeSet<_> = cells.iter().copied().collect();
        assert_eq!(uniq.len(), cells.len(), "duplicate spawn cell");
        // The anchor's own snapped start comes first (distance 0), then its
        // cardinal neighbours (distance 1) before any distance-2 cell.
        assert_eq!(cells[0], [2, 65, 2]);
        // Non-increasing BFS distance is enforced by construction; spot-check that a
        // near cell precedes a far corner.
        let idx = |t: [i32; 3]| cells.iter().position(|c| *c == t).unwrap();
        assert!(idx([2, 65, 3]) < idx([0, 65, 0]));
    }

    #[test]
    fn confined_cells_never_cross_a_socket_seam() {
        // Two 3-wide rooms sharing an open (air) seam at x=3 — as a mated jigsaw
        // socket would be. Confining to the left room's bounds must keep every
        // placement cell at x<=2, never flooding through the open seam into the
        // right room (the den↔mouth spill this fix prevents).
        let mut solid = BTreeSet::new();
        for x in 0..=6 {
            for z in 0..3 {
                solid.insert([x, 64, z]); // continuous floor across both rooms
                solid.insert([x, 67, z]); // ceiling
            }
        }
        let world = World::from_solid_cells(solid);
        let left_bounds = ([0, 64, 0], [2, 66, 2]);
        let cells = world.confined_standable_cells([1, 65, 1], left_bounds);
        assert!(!cells.is_empty());
        for c in &cells {
            assert!(
                c[0] <= 2,
                "placement {c:?} crossed the seam into the right room"
            );
        }
        // Sanity: the floor genuinely connects across the seam (an unconfined flood
        // would reach the right room), so confinement — not a wall — is what holds.
        assert!(world.find_path([1, 65, 1], [5, 65, 1]).is_some());
    }

    // --- spec-0016 §6: the aggro ring -----------------------------------

    #[test]
    fn aggro_ring_cells_sit_at_or_just_inside_the_radius() {
        // A 25×25 floored hall; the ring is drawn around its centre at radius 10.
        let world = floored(25, 25, 65, &[]);
        let bounds = ([0, 64, 0], [24, 66, 24]);
        let centre = [12, 65, 12];
        let ring = world.annulus_standable_cells(centre, bounds, 10.0, 1.0);
        assert!(!ring.is_empty(), "an open hall has a ring at radius 10");
        for c in &ring {
            assert!(world.standable(*c), "ring cell {c:?} is standable");
            let d = ((0..3)
                .map(|i| f64::from(c[i] - centre[i]).powi(2))
                .sum::<f64>())
            .sqrt();
            // One-sided on purpose: a cell OUTSIDE follow_range summons a mob
            // that perceives nobody and stands there.
            assert!(
                (9.0..=10.0).contains(&d),
                "ring cell {c:?} at distance {d} is outside [radius-1, radius]"
            );
        }
        // Ordered outermost-first — the edge of perception is where the fiction
        // (and the mechanic) puts them.
        let dist = |c: &[i32; 3]| {
            ((0..3)
                .map(|i| f64::from(c[i] - centre[i]).powi(2))
                .sum::<f64>())
            .sqrt()
        };
        assert!(
            dist(&ring[0]) >= dist(ring.last().unwrap()),
            "the ring is ordered outermost-first: {ring:?}"
        );
        // Deterministic (ADR-0006).
        assert_eq!(
            ring,
            world.annulus_standable_cells(centre, bounds, 10.0, 1.0)
        );
    }

    #[test]
    fn aggro_ring_excludes_cells_that_cannot_see_the_defended_point() {
        // A hall with a full-height wall at x=12 splitting it in two, pierced by
        // nothing: cells on the far side are at ring distance but blind, so a mob
        // summoned there would acquire no target — the mechanic's whole point.
        let mut solid = BTreeSet::new();
        for x in 0..25 {
            for z in 0..9 {
                solid.insert([x, 64, z]);
                solid.insert([x, 68, z]);
            }
        }
        for z in 0..9 {
            for y in 65..=67 {
                solid.insert([18, y, z]);
            }
        }
        // Leave a floor-level gap so the far side stays walk-REACHABLE (this is a
        // sight test, not a reachability test).
        solid.remove(&[18, 65, 4]);
        solid.remove(&[18, 66, 4]);
        let world = World::from_solid_cells(solid);
        let bounds = ([0, 64, 0], [24, 67, 8]);
        let centre = [8, 65, 4];
        let ring = world.annulus_standable_cells(centre, bounds, 10.0, 1.0);
        assert!(!ring.is_empty(), "the near side offers ring cells");
        for c in &ring {
            assert!(
                world.has_line_of_sight(*c, centre),
                "ring cell {c:?} must see the defended point"
            );
        }
        // A far-side cell at exactly ring distance is excluded despite standing
        // and being reachable through the gap.
        let blind = [18 + 1, 65, 0];
        if world.standable(blind) {
            let d = ((0..3)
                .map(|i| f64::from(blind[i] - centre[i]).powi(2))
                .sum::<f64>())
            .sqrt();
            if (9.0..=10.0).contains(&d) {
                assert!(
                    !ring.contains(&blind),
                    "a blind cell at ring distance must be excluded"
                );
            }
        }
    }

    #[test]
    fn aggro_ring_is_empty_when_the_room_is_smaller_than_the_radius() {
        // The DW0387 shape: a 5×5 room has no cell 20 blocks from its centre.
        let world = floored(5, 5, 65, &[]);
        let bounds = ([0, 64, 0], [4, 66, 4]);
        assert!(
            world
                .annulus_standable_cells([2, 65, 2], bounds, 20.0, 1.0)
                .is_empty()
        );
    }

    #[test]
    fn confined_cells_deterministic_across_runs() {
        let world = floored(6, 4, 65, &[[3, 65, 1]]);
        let bounds = ([0, 64, 0], [5, 66, 3]);
        let a = world.confined_standable_cells([1, 65, 1], bounds);
        let b = world.confined_standable_cells([1, 65, 1], bounds);
        assert_eq!(a, b);
    }

    #[test]
    fn step_up_needs_head_clearance_to_jump() {
        // Lower stand at x=0 (floor y64 → stand y65); raised stand at x=1,2 (floor
        // y65 → stand y66). Reaching the raised floor means jumping up one block at
        // x=0, whose head sweeps the cell two above the feet ([0,67,0]).
        let mk = |low_ceiling: bool| {
            let mut solid = BTreeSet::new();
            solid.insert([0, 64, 0]); // lower floor
            solid.insert([1, 65, 0]); // raised floor
            solid.insert([2, 65, 0]);
            if low_ceiling {
                solid.insert([0, 67, 0]); // ceiling two above the jumper's feet
            }
            World::from_solid_cells(solid)
        };
        // Open headroom: the jump-up is walkable.
        let open = mk(false);
        assert!(open.standable([0, 65, 0]) && open.standable([2, 66, 0]));
        assert!(open.find_path([0, 65, 0], [2, 66, 0]).is_some());
        // A ceiling two above the feet blocks the jump (the entity would head-bonk),
        // so no walkable path exists — the DW0311 case a runtime bot rejects with
        // "No path to the goal!".
        let low = mk(true);
        assert!(low.standable([0, 65, 0]) && low.standable([2, 66, 0]));
        assert!(low.find_path([0, 65, 0], [2, 66, 0]).is_none());
    }

    // --- collision-accurate standability: fences / walls / fence gates ---

    /// A world from explicit collision classes: a flat solid floor at
    /// `y-1` over `[0,w) × [0,d)` with the given `tall` (fence/wall) and
    /// `use_gates` (closed fence gate) cells at stand level.
    fn classified(w: i32, d: i32, y: i32, tall: &[[i32; 3]], use_gates: &[[i32; 3]]) -> World {
        let mut solid = BTreeSet::new();
        for x in 0..w {
            for z in 0..d {
                solid.insert([x, y - 1, z]);
            }
        }
        World::from_occupancy(crate::assembled::Occupancy {
            solid,
            tall: tall.iter().copied().collect(),
            use_gates: use_gates.iter().copied().collect(),
            flooded: BTreeSet::new(),
            partial: BTreeMap::new(),
        })
    }

    /// The gateless ram-pen shape: a closed fence ring at stand level around an
    /// interior anchor. `gate`, when set, replaces one ring cell with a closed
    /// fence gate (a use-gate cell).
    fn fence_ring_world(gate: Option<[i32; 3]>) -> World {
        let y = 65;
        let mut ring: Vec<[i32; 3]> = Vec::new();
        for i in 1..=5 {
            ring.push([i, y, 1]);
            ring.push([i, y, 5]);
            ring.push([1, y, i]);
            ring.push([5, y, i]);
        }
        let gates: Vec<[i32; 3]> = gate.into_iter().collect();
        ring.retain(|c| !gates.contains(c));
        classified(7, 7, y, &ring, &gates)
    }

    #[test]
    fn fence_top_is_not_standable_and_fence_is_not_passable() {
        // The owner-hit island bug, modelled: a 1.5-tall oak_fence is neither a
        // floor (no walking player can jump 1.5 onto its top) nor a passable cell.
        let world = classified(3, 1, 65, &[[1, 65, 0]], &[]);
        assert!(
            !world.standable([1, 66, 0]),
            "a fence-top cell must not be standable (the old full-solid model's bug)"
        );
        assert!(
            !world.standable([1, 65, 0]),
            "the fence cell itself must not be passable"
        );
        // The two floor cells beside it are fine but no longer connected.
        assert!(world.standable([0, 65, 0]) && world.standable([2, 65, 0]));
        assert!(
            world.find_path([0, 65, 0], [2, 65, 0]).is_none(),
            "no route through or over a fence line"
        );
    }

    #[test]
    fn gateless_fence_ring_is_dw0311() {
        // The soundness hole the full-solid model had: a pen fenced on every side
        // with NO gate "passed" the completability proof by standing the player on
        // the fence-top. It must now be a DW0311 build failure.
        let world = fence_ring_world(None);
        let inside = [3, 65, 3];
        let outside = [0, 65, 0]; // corner, outside the ring
        assert!(world.standable(inside) && world.standable(outside));
        let err = route_visited(
            &world,
            &[vp(outside, false), vp(inside, false)],
            &[],
            &linear,
        )
        .expect_err("a humanly impassable gateless fence ring must fail the proof");
        assert_eq!(err.code, DW_CRITICAL_UNROUTABLE); // DW0311
        assert!(
            err.message.contains("fence"),
            "the message should name the barrier class: {}",
            err.message
        );
    }

    #[test]
    fn fence_ring_with_closed_gate_routes_through_it_as_a_use_gate_edge() {
        // The island ram pen: the ring's only opening is a closed oak_fence_gate.
        // The player passes it with an adventure-legal right-click, so the proof
        // routes THROUGH the gate cell — a first-class use-gate edge, not a
        // fence-top hop and not a harness workaround.
        let gate = [3, 65, 1];
        let world = fence_ring_world(Some(gate));
        let inside = [3, 65, 3];
        let outside = [3, 65, 0];
        let path = world
            .find_path(outside, inside)
            .expect("the pen is enterable through its gate");
        assert!(
            path.contains(&gate),
            "the proven route must pass through the gate cell: {path:?}"
        );
        assert!(world.is_use_gate(gate), "the gate cell is tagged use-gate");
        // Every route cell is standable in the final model (the DW0314 guard).
        for &c in &path {
            assert!(world.is_standable(c), "route cell {c:?} standable");
        }
        assert!(
            route_visited(
                &world,
                &[vp(outside, false), vp(inside, false)],
                &[],
                &linear
            )
            .is_ok()
        );
        // The gate is still never a floor: its top is not standable.
        assert!(!world.standable([3, 66, 1]));
    }

    #[test]
    fn autonomous_walkers_treat_a_closed_gate_as_a_fence() {
        // A wave mob acting on its own cannot right-click: on the no-gate-use view
        // (wave seating) the pen is sealed again.
        let gate = [3, 65, 1];
        let world = fence_ring_world(Some(gate));
        let entity_world = world.without_gate_use();
        assert!(world.has_use_gates() && !entity_world.has_use_gates());
        assert!(
            entity_world.find_path([3, 65, 0], [3, 65, 3]).is_none(),
            "a non-player walker must not route through a closed gate"
        );
        // Wave seating never picks the gate threshold or anything past it.
        let cells = entity_world.confined_standable_cells([3, 65, 3], ([2, 64, 2], [4, 66, 4]));
        assert!(!cells.is_empty());
        assert!(!cells.contains(&gate), "no mob seated in the gate cell");
    }

    #[test]
    fn open_fence_gate_is_a_passable_threshold_with_no_use_tag() {
        // An authored-open gate (block state open=true) is just a passable cell:
        // no use-gate tag, and even gate-incapable walkers pass it.
        let world = classified(3, 1, 65, &[], &[]); // flat; the "gate" cell is plain air
        assert!(world.find_path([0, 65, 0], [2, 65, 0]).is_some());
        assert!(!world.is_use_gate([1, 65, 0]));
    }

    #[test]
    fn camera_dolly_clips_fences_and_closed_gates() {
        // A fence contains visible geometry: a cutscene camera flying through its
        // cell is a DW0308 clip, exactly like a full solid — and so is a closed
        // fence gate (the camera would fly through the gate leaves).
        let world = classified(5, 3, 65, &[[2, 65, 1]], &[[2, 65, 2]]);
        let through_fence = [[0.5, 65.5, 1.5], [4.5, 65.5, 1.5]];
        assert_eq!(first_clip(&world, &through_fence), Some((0, [2, 65, 1])));
        let through_gate = [[0.5, 65.5, 2.5], [4.5, 65.5, 2.5]];
        assert_eq!(first_clip(&world, &through_gate), Some((0, [2, 65, 2])));
    }

    #[test]
    fn critical_path_route_returns_the_proven_cell_polyline() {
        // A flat connected floor: the walked leg's exported route is the A* cell
        // path, inclusive of both snapped endpoints.
        let world = floored(8, 3, 65, &[]);
        let a = [0, 65, 1];
        let b = [6, 65, 1];
        let cells = world.find_path(a, b).expect("routable");
        assert_eq!(cells.first(), Some(&a));
        assert_eq!(cells.last(), Some(&b));
        // Every cell on an exported route is standable (a real floor cell).
        for c in &cells {
            assert!(world.standable(*c), "route cell {c:?} not standable");
        }
    }

    #[test]
    fn resample_honors_speed_and_lands_exactly_on_target() {
        let cells = [[0, 65, 0], [10, 65, 0]];
        let slow = resample(&cells, 0.15);
        let fast = resample(&cells, 1.0);
        // Slower speed → more per-tick waypoints for the same distance.
        assert!(slow.len() > fast.len());
        // Endpoints are the CENTRES of the start/goal cells, not their corners:
        // a body positioned on the integer cell coordinate straddles four columns.
        assert_eq!(*slow.last().unwrap(), cell_center([10, 65, 0]));
        assert_eq!(slow[0], cell_center([0, 65, 0]));
    }

    // --- v0.6 checkpoint / stealth proofs (spec-0012 / spec-0014) ---

    /// Two floor patches (x∈{0,1} and x∈{3,4}) with a void gap at x=2.
    fn split_world(y: i32) -> World {
        let mut solid = BTreeSet::new();
        for x in [0, 1, 3, 4] {
            for z in 0..3 {
                solid.insert([x, y - 1, z]); // floor
                solid.insert([x, y + 2, z]); // ceiling
            }
        }
        World::from_solid_cells(solid)
    }

    fn at_step(pos: [i32; 3], src_step: usize) -> VisitedPos {
        VisitedPos {
            pos,
            transport_before: false,
            talk_to: false,
            src_step,
        }
    }

    // --- close-gate completability (DSL v0.6) --------------------------------

    /// A `close-gate` firing before a forced walked leg seals the gate region, so a
    /// critical path that must re-cross it fails DW0311; a later `open-gate` before
    /// the same leg reopens it and the route passes again.
    #[test]
    fn close_gate_seals_a_forced_leg_is_dw0311() {
        // A 1-wide corridor along x, y=65; the pass-through cell [2,65,0] is the sole
        // connection between the two ends. Base world (gate open) routes end to end.
        let world = floored(5, 1, 65, &[]);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        assert!(
            route_visited(&world, &[a, b], &[], &linear).is_ok(),
            "the open corridor must route with no gate events"
        );
        // A close-gate seals the pass-through before the leg to `b` (fire_step 0 < 2).
        let close = RegionEvent::forced(([2, 65, 0], [2, 65, 0]), RegionWrite::Fill, 0);
        let err =
            route_visited(&world, &[a, b], std::slice::from_ref(&close), &linear).unwrap_err();
        assert_eq!(err.code, DW_CRITICAL_UNROUTABLE); // DW0311
        assert!(
            err.message.contains("close-gate"),
            "the message must name the sealed gate: {}",
            err.message
        );
        // Reopening the gate before the leg (open-gate at a later fire_step) restores it.
        let open = RegionEvent::forced(([2, 65, 0], [2, 65, 0]), RegionWrite::Unseal, 1);
        assert!(
            route_visited(&world, &[a, b], &[close, open], &linear).is_ok(),
            "a gate reopened by open-gate before the leg must route again"
        );
    }

    // --- the region write, generalised (DSL v0.10, spec-0031) ---------------

    /// A `fill-region` seals a forced leg exactly as a `close-gate` does, and a
    /// later `clear-region` over the same box reopens it.
    ///
    /// Same world, same predicate, same proof as the gate pair above — which is the
    /// claim: the completability rule belongs to the region, and a verb that names
    /// no gate inherits it rather than re-deriving it.
    #[test]
    fn fill_region_seals_a_forced_leg_and_clear_region_reopens_it() {
        let world = floored(5, 1, 65, &[]);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        let fill = RegionEvent::forced(([2, 65, 0], [2, 65, 0]), RegionWrite::Fill, 0);
        let err = route_visited(&world, &[a, b], std::slice::from_ref(&fill), &linear).unwrap_err();
        assert_eq!(err.code, DW_CRITICAL_UNROUTABLE); // DW0311
        let clear = RegionEvent::forced(([2, 65, 0], [2, 65, 0]), RegionWrite::Clear, 1);
        assert!(
            route_visited(&world, &[a, b], &[fill, clear], &linear).is_ok(),
            "a region cleared before the leg must route again"
        );
    }

    /// A floor at `y=64` with a three-cell gap at `x ∈ {1,2,3}` — the two ends are
    /// separated by open void, so nothing routes end to end until something lays
    /// floor in the gap.
    fn chasm() -> World {
        let mut solid = BTreeSet::new();
        for x in 0..5i32 {
            if !(1..=3).contains(&x) {
                solid.insert([x, 64, 0]); // floor, minus the gap
            }
            solid.insert([x, 67, 0]); // ceiling
        }
        World::from_solid_cells(solid)
    }

    /// A `fill-region` that LAYS floor — a repaired stair, a lowered bridge, a
    /// placed plank — carries the critical path across a gap, and the exported
    /// waypoints are judged in the same world the route was proven in.
    ///
    /// The two halves used to disagree about this world, and only one of them was
    /// wrong. The completability proof ([`route_visited`]) has read the leg's
    /// runtime region state since spec-0031; the waypoint self-check
    /// ([`verify_exported_routes`]) re-judged the very same cells against the BARE
    /// assembled world, where the plank does not exist. So a leg over runtime-laid
    /// floor routed and was then refused `DW0314` for having "no floor" — and a
    /// campaign whose critical path crosses a bridge it lowers could not ship.
    #[test]
    fn a_critical_path_over_runtime_laid_floor_routes_and_exports() {
        let world = chasm();
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        assert!(
            route_visited(&world, &[a, b], &[], &linear).is_err(),
            "the chasm must not route before anything lays floor in it"
        );
        // FORCED, and it has to be: this test asserts the leg routes and exports,
        // and only a fill the party cannot skip carries a forced leg's footing. The
        // unforced spelling of this very plank is the opposite verdict — see
        // `the_export_self_check_reads_a_legs_world_with_the_unforced_reading`.
        let plank = RegionEvent::forced(([1, 64, 0], [3, 64, 0]), RegionWrite::Fill, 0);
        assert!(
            route_visited(&world, &[a, b], std::slice::from_ref(&plank), &linear).is_ok(),
            "the proof must credit floor the campaign lays from a beat the party cannot skip, \
             before the leg is walked"
        );
        let legs: Vec<LegRoute> =
            route_walked_legs(&world, &[a, b], std::slice::from_ref(&plank), &linear)
                .into_iter()
                .map(|(leg, _)| leg)
                .collect();
        assert_eq!(legs.len(), 1, "the walked leg must be exported");
        assert!(
            legs[0].cells.contains(&[2, 65, 0]),
            "the exported route must cross the laid floor: {:?}",
            legs[0].cells
        );
        // The half that was wrong. Judged against the bare world these cells have
        // no floor; judged against the world the leg was proven over, they do.
        assert!(
            !world.is_standable([2, 65, 0]),
            "the bare assembled world really does lack the floor — otherwise this \
             test proves nothing"
        );
        verify_exported_routes(&world, &legs)
            .expect("a waypoint on floor the campaign lays must pass the export self-check");
    }

    /// The direction the self-check exists for is untouched: a cell a **later pass**
    /// mutated is still `DW0314`, because a terrain edit is not a runtime region
    /// write and no leg state restores it.
    ///
    /// Same leg, same laid plank, same exported route — but the final world has had
    /// one of the route's cells walled since the route was proven. The leg's own
    /// region state cannot explain that cell away, so the refusal stands.
    #[test]
    fn a_later_pass_that_walls_a_proven_cell_is_still_dw0314() {
        let world = chasm();
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        // FORCED for the same reason: the leg has to route at all before a later
        // pass can be shown to break it.
        let plank = RegionEvent::forced(([1, 64, 0], [3, 64, 0]), RegionWrite::Fill, 0);
        let legs: Vec<LegRoute> =
            route_walked_legs(&world, &[a, b], std::slice::from_ref(&plank), &linear)
                .into_iter()
                .map(|(leg, _)| leg)
                .collect();
        // A later pass drops a block into a cell the proven route walks through.
        let mutated = world.with_sealed(&[[2, 65, 0]].into_iter().collect());
        let err = verify_exported_routes(&mutated, &legs)
            .expect_err("a walled waypoint must still be refused");
        assert_eq!(err.code, DW_WAYPOINT_NOT_STANDABLE); // DW0314
        assert!(
            err.message.contains("[2, 65, 0]"),
            "the message must name the offending cell: {}",
            err.message
        );
    }

    /// **The junction of the two halves, at the one point where it is directly
    /// observable.** The export self-check judges a leg in the world the leg was
    /// proven over ([`LegRoute::proven_world`]), and that world is built by
    /// [`World::with_region_state`] — which also applies the unforced reading
    /// ([`World::with_unforced`]). So the self-check inherits forcedness for free,
    /// and neither half alone says so: one decided *which world* the check reads,
    /// the other decided *what an unforced fill does to a world*.
    ///
    /// Same world, same leg, same exported cells. The only thing that differs
    /// between the two verdicts below is whether the plank under `[2,65,0]` was
    /// laid by a beat the party cannot skip.
    ///
    /// **What would make this test vacuous**, stated so a later reader can check
    /// it rather than trust it:
    ///
    /// * If the chasm did not really lack the floor, the accept would pass for the
    ///   wrong reason — the bare world would already be standable and
    ///   `proven_world` would be doing nothing. Asserted below.
    /// * If the exported route did not really cross the laid cell, both verdicts
    ///   would be about a cell nobody stands on. Asserted below.
    /// * If `verify_exported_routes` returned `Err` for some unrelated cell, the
    ///   refusal would look right and mean nothing. The code is asserted, and the
    ///   message is required to name one of the cells standing on the plank.
    ///
    /// One thing this test deliberately does NOT claim: that a whole campaign can
    /// reach the refusing branch. It cannot — `route_visited` refuses an unforced
    /// footing as `DW0546` before any route is exported, so at campaign scale this
    /// reading is defence in depth rather than the live gate. That is why the
    /// unforced leg here is constructed rather than routed: `route_walked_legs`
    /// over an unforced plank correctly produces no leg at all, which is asserted
    /// too. The campaign-scale statement of the same junction is
    /// `crates/compiler/tests/laid_footing_root.rs`.
    #[test]
    fn the_export_self_check_reads_a_legs_world_with_the_unforced_reading() {
        let world = chasm();
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        let box_ = ([1, 64, 0], [3, 64, 0]);

        // Not assumed: the bare assembled world really has no floor here, so
        // anything that stands at [2,65,0] is standing on something a runtime
        // write laid.
        assert!(
            !world.is_standable([2, 65, 0]),
            "the chasm must really be a chasm, or neither verdict below means anything"
        );

        // --- forced: the leg routes, and the export self-check accepts it -------
        let forced = RegionEvent::forced(box_, RegionWrite::Fill, 0);
        let legs: Vec<LegRoute> =
            route_walked_legs(&world, &[a, b], std::slice::from_ref(&forced), &linear)
                .into_iter()
                .map(|(leg, _)| leg)
                .collect();
        assert_eq!(legs.len(), 1, "the forced plank must carry a walked leg");
        assert!(
            legs[0].cells.contains(&[2, 65, 0]),
            "the exported route must really cross the laid cell: {:?}",
            legs[0].cells
        );
        // Reverting the leg-carries-its-world half reds HERE: judged against the
        // bare `world`, [2,65,0] has no floor and this becomes `DW0314`.
        verify_exported_routes(&world, &legs)
            .expect("footing the party cannot skip must pass the export self-check");

        // --- unforced: the same cells, in a world that may not hold the plank ---
        // `route_walked_legs` will not produce this leg — an unforced plank is
        // impassable and not floor, so nothing routes across it. That is the
        // campaign-scale verdict, and it is asserted rather than assumed.
        assert!(
            route_walked_legs(
                &world,
                &[a, b],
                &[RegionEvent::unforced(
                    box_,
                    RegionWrite::Fill,
                    0,
                    "a trap nobody must spring"
                )],
                &linear,
            )
            .is_empty(),
            "an unforced plank must not carry a walked leg at all"
        );
        // So the leg is carried over from the forced run with only its region
        // state re-marked: identical cells, identical world, one bit different.
        let mut unforced_state = RegionState::default();
        unforced_state
            .unforced
            .extend(crate::assembled::region_cells(box_.0, box_.1));
        let mut leg = legs[0].clone();
        leg.region_state = unforced_state;
        // Reverting the footing half reds HERE: with an unforced fill folded back
        // into `solid`, [2,65,0] is floored and the self-check accepts a waypoint
        // standing on a plank the party may never have laid.
        let err = verify_exported_routes(&world, std::slice::from_ref(&leg))
            .expect_err("a waypoint standing on unforced footing must not be exported");
        assert_eq!(err.code, DW_WAYPOINT_NOT_STANDABLE); // DW0314
        // The refusal must be ABOUT the plank, not about some other cell that
        // happens to be unstandable — that is the vacuity this assertion removes.
        // Which of the three cells standing on the box is reported is the loop's
        // order and not a claim, so any of them satisfies it.
        assert!(
            [[1, 65, 0], [2, 65, 0], [3, 65, 0]]
                .iter()
                .any(|c| err.message.contains(&format!("{c:?}"))),
            "the refusal must name a cell whose footing is the uncertain plank: {}",
            err.message
        );
    }

    /// A leg that writes no region judges the bare world, clones nothing, and is
    /// the answer it always was — the fast path every campaign without a runtime
    /// region takes.
    #[test]
    fn a_leg_with_no_runtime_write_judges_the_bare_world() {
        let world = floored(5, 1, 65, &[]);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        let legs: Vec<LegRoute> = route_walked_legs(&world, &[a, b], &[], &linear)
            .into_iter()
            .map(|(leg, _)| leg)
            .collect();
        assert_eq!(legs.len(), 1);
        assert!(
            legs[0].proven_world(&world).is_none(),
            "a leg with no runtime write must not clone a world"
        );
        verify_exported_routes(&world, &legs).expect("an ordinary leg still passes");
    }

    // --- what a write LEAVES: fluid is not floor ----------------------------

    /// A runtime fill of a **fluid** takes the floor away, and the model says so.
    ///
    /// The corridor's floor at `[2,64,0]` is the only footing between the two ends.
    /// A `Fill` there is a no-op (the cell was already solid) and the leg routes. A
    /// `Flood` there — the same box, the same fire step, the same verb, a different
    /// block — replaces the floor with water, and a body does not stand on water.
    ///
    /// This is the whole defect in four lines: with `Flood` folded into `Fill`, the
    /// second case routed too, and the compiler proved a party walking across a
    /// pond in mid-air.
    #[test]
    fn a_fluid_fill_takes_the_floor_away_and_a_solid_fill_does_not() {
        let world = floored(5, 1, 65, &[]);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        let floor_box = ([2, 64, 0], [2, 64, 0]);
        let solid_fill = RegionEvent::forced(floor_box, RegionWrite::Fill, 0);
        assert!(
            route_visited(&world, &[a, b], std::slice::from_ref(&solid_fill), &linear).is_ok(),
            "filling a floor cell with a block leaves it floor"
        );
        let fluid_fill = RegionEvent::forced(floor_box, RegionWrite::Flood, 0);
        let err =
            route_visited(&world, &[a, b], std::slice::from_ref(&fluid_fill), &linear).unwrap_err();
        assert_eq!(err.code, DW_FLUID_FILL_ON_CRITICAL_PATH); // DW0544
        assert!(
            err.message.contains("[2, 64, 0]..[2, 64, 0]"),
            "the message must name the box that took the footing: {}",
            err.message
        );
    }

    /// A flooded cell blocks passage as hard as a wall does, on top of not being
    /// floor — so a fluid fill laid **across** the corridor closes it exactly as a
    /// `close-gate` would.
    ///
    /// The code here is `DW0311`, not `DW0544`, and that is the counterfactual being
    /// honest rather than a gap. `DW0544` answers one question — *would this route
    /// exist if the box held a block?* — and for a box laid across the path the
    /// answer is no: the campaign built a wall, and calling the fluid the culprit
    /// would send the author to change a block that changes nothing. What the fluid
    /// must still buy them is the right HINT: not "your prefab has a wedged
    /// doorway", which is the geometry-is-innocent misattribution this family exists
    /// to prevent.
    #[test]
    fn a_fluid_fill_across_the_corridor_is_a_wall_and_says_which() {
        let world = floored(5, 1, 65, &[]);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        let flood = RegionEvent::forced(([2, 65, 0], [2, 66, 0]), RegionWrite::Flood, 0);
        let err =
            route_visited(&world, &[a, b], std::slice::from_ref(&flood), &linear).unwrap_err();
        assert_eq!(err.code, DW_CRITICAL_UNROUTABLE);
        assert!(
            err.message.contains("FLUID"),
            "an unroutable leg the campaign flooded must not be reported as a wedged \
             doorway: {}",
            err.message
        );
    }

    /// Where a fluid fill and a solid fill overlap, the **fluid** wins.
    ///
    /// Not a tie-break picked for convenience: a flooded cell is everything a walled
    /// cell is (impassable) and one thing more (not floor), so taking it is the
    /// conservative answer in the same sense that "a fill beats a clear" is. It also
    /// makes the result independent of declaration order, which ADR-0006 requires.
    #[test]
    fn where_a_fluid_fill_overlaps_a_solid_fill_the_fluid_wins() {
        let world = floored(5, 1, 65, &[]);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        let over_floor = RegionEvent::forced(([2, 64, 0], [2, 64, 0]), RegionWrite::Flood, 0);
        // A solid fill over a box that covers the same cell, declared either side of
        // the flood. Both orders must refuse.
        let wider_solid = RegionEvent::forced(([1, 64, 0], [3, 64, 0]), RegionWrite::Fill, 0);
        for events in [
            vec![over_floor.clone(), wider_solid.clone()],
            vec![wider_solid, over_floor],
        ] {
            let err = route_visited(&world, &[a, b], &events, &linear).unwrap_err();
            assert_eq!(
                err.code, DW_FLUID_FILL_ON_CRITICAL_PATH,
                "a solid fill over the same cells must not dry the fluid out"
            );
        }
    }

    /// A runtime **clear** declared over a box a different write floods does not dry
    /// it: within one quest state the order is clear → fill → flood, so the wettest
    /// answer is the one that survives.
    ///
    /// This is [`World::with_cleared`]'s stated rule reaching a case it could not
    /// reach before — a `fill … air` against a wet cell lets the water back in
    /// rather than removing it, and the water may now be water a runtime write put
    /// there rather than only water a prefab did. What the ordering guards is a
    /// reorder of [`World::with_region_state`]: run the clear last and this campaign
    /// silently proves a dry floor again.
    #[test]
    fn a_clear_over_a_flooded_box_does_not_dry_it() {
        let world = floored(5, 1, 65, &[]);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        let flood = RegionEvent::forced(([2, 64, 0], [2, 64, 0]), RegionWrite::Flood, 0);
        // A different box (so it is a different region, with its own latest write)
        // covering the flooded floor cell and the air above it.
        let clear = RegionEvent::forced(([2, 64, 0], [2, 65, 0]), RegionWrite::Clear, 1);
        let err = route_visited(&world, &[a, b], &[flood, clear], &linear).unwrap_err();
        assert_eq!(err.code, DW_FLUID_FILL_ON_CRITICAL_PATH);
    }

    /// A campaign that writes no fluid pays nothing: the counterfactual is never
    /// built, and every existing verdict is the verdict it always was.
    #[test]
    fn a_world_with_no_runtime_flood_reports_none() {
        let world = floored(5, 1, 65, &[]);
        assert!(!world.has_runtime_flood());
        let sealed = world.with_sealed(&[[2, 65, 0]].into_iter().collect());
        assert!(
            !sealed.has_runtime_flood(),
            "a seal is not a flood, however many cells it forces"
        );
    }

    /// The half no gate could ever exercise: a `clear-region` credits a route
    /// through geometry the **prefab** put there, not merely through a wall an
    /// earlier effect built. The assembled model holds every gate cell open
    /// unconditionally, so `open-gate` never had to prove this and never did.
    #[test]
    fn clear_region_opens_prefab_geometry() {
        // A wall cell across the corridor: no route at all in the base world.
        let world = floored(5, 1, 65, &[[2, 65, 0], [2, 66, 0]]);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        assert!(
            route_visited(&world, &[a, b], &[], &linear).is_err(),
            "the walled corridor must not route before the clear"
        );
        let clear = RegionEvent::forced(([2, 65, 0], [2, 66, 0]), RegionWrite::Clear, 0);
        assert!(
            route_visited(&world, &[a, b], std::slice::from_ref(&clear), &linear).is_ok(),
            "the cleared wall must be passable from the DAG point the clear fires at"
        );
    }

    /// An `open-gate` is **not** an unfiltered clear: it removes only the gate's own
    /// block. So it cannot delete geometry another proof has forced solid — a
    /// `collapse`'s debris resting in the doorway stays exactly where it fell.
    ///
    /// The guard is [`World::pinned`], and it holds for an authored `clear-region`
    /// too: clearing a region says "the blocks the campaign put here are gone", not
    /// "the hazard another proof is reasoning about never happened".
    #[test]
    fn a_runtime_clear_does_not_undo_another_proofs_premise() {
        let world = floored(5, 1, 65, &[]);
        let debris: BTreeSet<[i32; 3]> = [[2, 65, 0], [2, 66, 0]].into_iter().collect();
        let buried = world.with_sealed(&debris);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        assert!(
            route_visited(&buried, &[a, b], &[], &linear).is_err(),
            "the debris blocks the corridor"
        );
        for write in [RegionWrite::Unseal, RegionWrite::Clear] {
            let ev = RegionEvent::forced(([2, 65, 0], [2, 66, 0]), write, 0);
            assert!(
                route_visited(&buried, &[a, b], std::slice::from_ref(&ev), &linear).is_err(),
                "{write:?} must not delete another proof's forced-solid cells"
            );
        }
    }

    /// The seal is **DAG-causal**, not linear: a `close-gate` fired on a parallel
    /// quest branch (not a causal ancestor of the leg) must NOT seal it, even though
    /// its `fire_step` is numerically earlier — the fix for the lineariser
    /// interleaving a sibling branch ahead of a sealed leg (island `take-the-cheese`
    /// vs `hide`). A genuinely-forced causal re-crossing is still sealed.
    #[test]
    fn close_gate_seal_is_dag_causal_not_linear() {
        let world = floored(5, 1, 65, &[]);
        let close = RegionEvent::forced(([2, 65, 0], [2, 65, 0]), RegionWrite::Fill, 8);
        let a = at_step([0, 65, 0], 9);
        let b = at_step([4, 65, 0], 10);
        // Parallel: neither the close (step 8) nor the prior position (step 9) is a
        // causal ancestor of the arrival (step 10) — a cross-branch artifact leg.
        let parallel = |g: usize, s: usize| !((g == 8 || g == 9) && s == 10) && g < s;
        assert!(
            route_visited(&world, &[a, b], std::slice::from_ref(&close), &parallel).is_ok(),
            "a close on a parallel branch must not seal a non-causal leg"
        );
        // Causal: step 8 (close) and step 9 are ancestors of step 10 (a forced
        // re-crossing with no reopen) → sealed → DW0311 (proof preserved).
        let err = route_visited(&world, &[a, b], std::slice::from_ref(&close), &linear)
            .expect_err("a forced causal re-crossing of a sealed gate must fail");
        assert_eq!(err.code, DW_CRITICAL_UNROUTABLE);
    }

    /// A `close-gate` that walls off the forward path from a checkpoint strands the
    /// party (DW0315) — the checkpoint gate proof routes under the same per-leg seal.
    #[test]
    fn close_gate_walls_off_checkpoint_forward_path_is_dw0315() {
        let world = floored(5, 1, 65, &[]);
        // Checkpoint at the near end (fire_step 0); the next required anchor is past
        // the gate cell [2,65,0].
        let cps = vec![("cp/rest".to_string(), [0, 65, 0], 0usize)];
        let positions = vec![at_step([4, 65, 0], 1)];
        // Open gate → reachable.
        assert!(verify_checkpoints(&world, &cps, &positions, &[], &linear).is_ok());
        // Sealed before the party reaches the target (fire_step 0 < 1) → stranded.
        let close = RegionEvent::forced(([2, 65, 0], [2, 65, 0]), RegionWrite::Fill, 0);
        let err = verify_checkpoints(
            &world,
            &cps,
            &positions,
            std::slice::from_ref(&close),
            &linear,
        )
        .unwrap_err();
        assert_eq!(err.code, DW_CHECKPOINT_STRANDED); // DW0315
    }

    #[test]
    fn checkpoint_behind_a_one_way_drop_is_dw0315() {
        // Checkpoint on the near patch; the next required anchor is on the far,
        // disconnected patch → not walkable from the checkpoint → DW0315.
        let world = split_world(65);
        let cps = vec![("cp/rest".to_string(), [0, 65, 1], 0usize)];
        let positions = vec![at_step([4, 65, 1], 1)];
        let err = verify_checkpoints(&world, &cps, &positions, &[], &linear).unwrap_err();
        assert_eq!(err.code, DW_CHECKPOINT_STRANDED); // DW0315
    }

    #[test]
    fn checkpoint_with_reachable_remaining_path_passes() {
        // Both the checkpoint and the next anchor sit on the same connected floor.
        let world = floored(5, 3, 65, &[]);
        let cps = vec![("cp/rest".to_string(), [0, 65, 1], 0usize)];
        let positions = vec![at_step([4, 65, 1], 1)];
        assert!(verify_checkpoints(&world, &cps, &positions, &[], &linear).is_ok());
    }

    #[test]
    fn checkpoint_over_void_is_dw0316() {
        // The checkpoint cell has no standable floor within snap radius.
        let world = floored(5, 3, 65, &[]);
        let cps = vec![("cp/rest".to_string(), [20, 65, 20], 0usize)];
        let err = verify_checkpoints(&world, &cps, &[], &[], &linear).unwrap_err();
        assert_eq!(err.code, DW_CHECKPOINT_UNSTANDABLE); // DW0316
    }

    #[test]
    fn stealth_zone_over_void_is_dw0327() {
        // A zero-extent zone centred on a void cell → no standable cell → DW0327.
        let world = floored(5, 3, 65, &[]);
        let beats = vec![(
            vec![("zone/shadow".to_string(), [20, 65, 20], [0, 0, 0])],
            0usize,
        )];
        let err = verify_stealth(&world, &beats, &[at_step([2, 65, 1], 0)]).unwrap_err();
        assert_eq!(err.code, DW_STEALTH_ZONE); // DW0327
    }

    #[test]
    fn stealth_zone_unreachable_from_beat_is_dw0327() {
        // The zone is standable on the far patch, but the activating beat sits on
        // the near patch across a void gap → unreachable → DW0327.
        let world = split_world(65);
        let beats = vec![(
            vec![("zone/shadow".to_string(), [4, 65, 1], [1, 1, 1])],
            0usize,
        )];
        let err = verify_stealth(&world, &beats, &[at_step([0, 65, 1], 0)]).unwrap_err();
        assert_eq!(err.code, DW_STEALTH_ZONE); // DW0327
    }

    #[test]
    fn stealth_zone_standable_and_reachable_passes() {
        let world = floored(6, 3, 65, &[]);
        let beats = vec![(
            vec![("zone/shadow".to_string(), [4, 65, 1], [1, 1, 1])],
            0usize,
        )];
        assert!(verify_stealth(&world, &beats, &[at_step([0, 65, 1], 0)]).is_ok());
    }

    // --- DW0355: stealth onset survivability --------------------------------

    /// The island defect in miniature: cover EXISTS and is reachable (DW0327 is
    /// happy) but is too far to reach inside the grace window, so the beat kills
    /// every player a fixed moment after it arms.
    #[test]
    fn stealth_zone_out_of_sprint_range_at_onset_is_dw0352() {
        // A 40-long corridor; the beat arms at x=0, the only zone sits at x=39.
        let world = floored(40, 3, 65, &[]);
        let zones = vec![("zone/alcove".to_string(), [39, 65, 1], [1, 1, 1])];
        // Reachability alone passes — this is exactly the gap DW0355 closes.
        assert!(
            verify_stealth(&world, &[(zones.clone(), 0)], &[at_step([0, 65, 1], 0)]).is_ok(),
            "DW0327 must be satisfied, so the failure below is purely a timing one"
        );
        let starts = vec![("the activating objective's anchor".to_string(), [0, 65, 1])];
        let err = verify_stealth_onset(&world, &zones, 50, &starts, 1)
            .expect_err("cover 38 blocks away cannot be reached in 50 ticks");
        assert_eq!(err.code, DW_STEALTH_ONSET); // DW0355
        assert!(
            err.message.contains("zone/alcove") && err.message.contains("short by"),
            "the diagnostic names the nearest zone and the tick deficit: {}",
            err.message
        );
        // The deficit is measured, not guessed: 38 blocks × 4 t + 10 t reaction.
        assert!(
            err.message.contains("152 ticks of sprinting"),
            "the sprint cost is the nav-model measurement: {}",
            err.message
        );
    }

    /// A checkpoint that respawns the party into a running punishing beat is a
    /// start position too — if IT cannot beat the window, the retry loop never
    /// terminates (a broken beat, not a souls retry).
    #[test]
    fn checkpoint_respawning_into_a_running_beat_must_beat_the_window_dw0352() {
        let world = floored(40, 3, 65, &[]);
        let zones = vec![("zone/alcove".to_string(), [2, 65, 1], [1, 1, 1])];
        // The activating anchor is next to cover; the respawn point is not.
        let starts = vec![
            ("the activating objective's anchor".to_string(), [0, 65, 1]),
            ("checkpoint `cp/below` respawn".to_string(), [39, 65, 1]),
        ];
        let err = verify_stealth_onset(&world, &zones, 50, &starts, 1)
            .expect_err("a respawn point outside sprint range of cover is a death loop");
        assert_eq!(err.code, DW_STEALTH_ONSET);
        assert!(
            err.message.contains("cp/below"),
            "the diagnostic names the offending checkpoint: {}",
            err.message
        );
    }

    /// A climb on the flee route is charged its jump arc: the same horizontal
    /// distance costs more when cover is up a step, which is what made the
    /// island's ramp-top zone unreachable in time.
    #[test]
    fn stealth_onset_charges_the_climb() {
        // Floor at y=65 with a step up to y=66 at x=5..7 (a 2-block rise).
        let mut solid = BTreeSet::new();
        for x in 0..8 {
            for z in 0..3 {
                solid.insert([x, 64, z]);
            }
        }
        for x in 5..8 {
            for z in 0..3 {
                solid.insert([x, 65, z]);
            }
        }
        let world = World::from_solid_cells(solid);
        let zones = vec![("zone/ledge".to_string(), [7, 66, 1], [0, 0, 0])];
        let starts = vec![("the activating objective's anchor".to_string(), [0, 65, 1])];
        // 7 horizontal steps (28 t) + one +1 climb (6 t) = 34 t, +10 reaction = 44.
        let err = verify_stealth_onset(&world, &zones, 40, &starts, 1)
            .expect_err("34 t of flee + 10 t reaction exceeds a 40-tick window");
        assert_eq!(err.code, DW_STEALTH_ONSET);
        assert!(
            err.message.contains("34 ticks of sprinting"),
            "the climb is charged its jump arc: {}",
            err.message
        );
        // Widening the window to the measured need discharges the obligation.
        assert!(
            verify_stealth_onset(&world, &zones, 44, &starts, 1).is_ok(),
            "grace sized to the measured need must pass"
        );
    }

    /// Cover inside the window passes — the green half of the story.
    #[test]
    fn stealth_onset_within_the_grace_window_passes() {
        let world = floored(40, 3, 65, &[]);
        let zones = vec![("zone/alcove".to_string(), [5, 65, 1], [1, 1, 1])];
        let starts = vec![
            ("the activating objective's anchor".to_string(), [0, 65, 1]),
            ("checkpoint `cp/below` respawn".to_string(), [8, 65, 1]),
        ];
        // 4 blocks to the zone edge = 16 t (+10) from the anchor; 4 t (+10) from cp.
        assert!(verify_stealth_onset(&world, &zones, 30, &starts, 1).is_ok());
    }

    /// The obligation is scoped to beats that actually punish: a `begin-stealth`
    /// whose `on_caught` only narrates has nothing to escape, so unreachable-in-time
    /// cover is atmosphere, not a broken beat.
    #[test]
    fn a_stealth_beat_that_only_narrates_is_not_punishing() {
        use delvewright_dsl::QuestEffect;
        let beat = |on_caught: Vec<QuestEffect>| crate::plan::StealthBeat {
            index: 1,
            zones: vec![("zone/alcove".to_string(), [0, 65, 0], [1, 1, 1])],
            on_caught,
            grace_ticks: 20,
            fire_step: 0,
            end_step: None,
        };
        assert!(
            !beat(vec![QuestEffect::Narrate {
                text: "Spotted!".to_string(),
                style: None,
                sound: None,
                requires_flags: Vec::new(),
                forbids_flags: Vec::new(),
                requires_state: Vec::new(),
            }])
            .is_punishing(),
            "a narrate-only on_caught carries no timing obligation"
        );
        assert!(
            beat(vec![QuestEffect::DamagePlayers {
                amount: 40,
                within: None,
                damage_type: None,
                requires_flags: Vec::new(),
                forbids_flags: Vec::new(),
                requires_state: Vec::new(),
            }])
            .is_punishing(),
            "damage-players makes the beat punishing"
        );
    }

    #[test]
    fn player_footprint_matches_pre_0_6_walkability() {
        // find_path (the delegating wrapper) must equal find_path_fp(player) — the
        // byte-identity guarantee for move-npc / critical-path.
        let world = floored(6, 3, 65, &[[3, 65, 1]]);
        let fp = Footprint::player();
        let a = [0, 65, 1];
        let b = [5, 65, 1];
        assert_eq!(world.find_path(a, b), world.find_path_fp(a, b, &fp));
        // Player footprint is one column, two cells tall.
        assert_eq!(fp.cols, vec![[0, 0]]);
        assert_eq!(fp.height, 2);
    }

    #[test]
    fn tall_footprint_cannot_walk_a_two_high_gap_a_player_fits() {
        // `floored` gives a floor at y-1 and a ceiling at y+2 → two clear cells (y,
        // y+1): a player (2 tall) fits; a warden (2.9 → 3 tall) head-bonks the
        // ceiling, so its footprint has no walkable path (the DW0325 condition).
        let world = floored(6, 3, 65, &[]);
        let a = [0, 65, 1];
        let b = [5, 65, 1];
        let player = Footprint::player();
        let warden = entity_footprint("minecraft:warden");
        assert_eq!(warden.height, 3, "warden is 2.9 tall → 3 cells");
        assert!(
            world.find_path_fp(a, b, &player).is_some(),
            "a player fits the 2-high corridor"
        );
        assert!(
            !world.standable_fp(a, &warden),
            "a warden cannot stand under a 2-high ceiling"
        );
        assert!(
            world.find_path_fp(a, b, &warden).is_none(),
            "a warden cannot walk the 2-high corridor → unroutable"
        );
        // The best-effort blocked-cell reporter names a non-standable cell on the leg.
        let blocked = first_blocked_fp(&world, a, b, &warden);
        assert!(!world.standable_fp(blocked, &warden));
    }

    #[test]
    fn dims_table_and_default_fallback() {
        // Sub-block-wide mobs are single-column; the default fallback is humanoid.
        assert_eq!(entity_footprint("minecraft:sheep").cols, vec![[0, 0]]);
        assert_eq!(entity_footprint("minecraft:sheep").height, 2); // 1.3 → 2
        assert_eq!(entity_footprint("minecraft:iron_golem").height, 3); // 2.7 → 3
        let unknown = entity_footprint("minecraft:some_new_mob");
        assert_eq!(unknown.cols, vec![[0, 0]]);
        assert_eq!(unknown.height, 2);
    }

    #[test]
    fn yaw_follows_the_movement_tangent() {
        // MC yaw: 0 = +z (south), 90 = -x (west), 180 = -z (north), 270 = +x (east).
        assert_eq!(yaw_of(0.0, 1.0), Some(0));
        assert_eq!(yaw_of(-1.0, 0.0), Some(90));
        assert_eq!(yaw_of(0.0, -1.0), Some(180));
        assert_eq!(yaw_of(1.0, 0.0), Some(270));
        assert_eq!(yaw_of(0.0, 0.0), None);
        // A straight +x path yaws every waypoint east (270), including the last.
        let wps = vec![[0.0, 65.0, 0.0], [1.0, 65.0, 0.0], [2.0, 65.0, 0.0]];
        assert_eq!(yaws_along(&wps, 0), vec![270, 270, 270]);
    }

    /// The corner turns on the tick it is taken: each waypoint carries the exact
    /// bearing of the segment it is about to walk, with no smoothing between the
    /// two legs, and the arrival waypoint keeps the last leg's facing.
    #[test]
    fn yaw_turns_at_a_direction_change() {
        // +x for two steps (east, 270), then +z for two (south, 0).
        let wps = vec![
            [0.0, 65.0, 0.0],
            [1.0, 65.0, 0.0],
            [2.0, 65.0, 0.0],
            [2.0, 65.0, 1.0],
            [2.0, 65.0, 2.0],
        ];
        assert_eq!(yaws_along(&wps, 180), vec![270, 270, 0, 0, 0]);
    }

    /// A leading segment with no horizontal motion (`resample`'s vertical step-up
    /// leg) keeps the seed — the facing the body already has — instead of
    /// fabricating a snap to south.
    #[test]
    fn yaw_keeps_the_seed_until_the_first_horizontal_step() {
        // Rise in place, then walk -z (north, 180).
        let wps = vec![
            [0.5, 65.0, 0.5],
            [0.5, 66.0, 0.5],
            [0.5, 66.0, -0.5],
            [0.5, 66.0, -1.5],
        ];
        assert_eq!(yaws_along(&wps, 90), vec![90, 180, 180, 180]);
        // A degenerate zero-length move never overrides the established facing.
        assert_eq!(yaws_along(&[[0.0, 65.0, 0.0]], 90), vec![90]);
    }

    // --- v0.6 trap completability proof (spec-0011, DW0342) ---

    use crate::plan::TrapDisarmPlan;
    use delvewright_dsl::{Lethality, TrapReset, TrapTrigger};

    /// A 1-wide walkable corridor along z=1: a floor strip at `[0..len, y-1, 1]`.
    /// `[x, y, 1]` are the only standable cells, so the corridor has no bypass — a
    /// cell on it is a genuine chokepoint the player cannot walk around.
    fn corridor(len: i32, y: i32) -> World {
        let mut solid = BTreeSet::new();
        for x in 0..len {
            solid.insert([x, y - 1, 1]);
        }
        World::from_solid_cells(solid)
    }

    /// A 1-wide, ceilinged corridor along x at z=1 (walls at z=0 and z=2, floor
    /// at y=64, ceiling at y=67). A body standing in it cannot be climbed over —
    /// the headroom above a blocked cell is the ceiling.
    fn walled_corridor() -> World {
        let mut walls = Vec::new();
        for x in 0..9 {
            for y in [65, 66] {
                walls.push([x, y, 0]);
                walls.push([x, y, 2]);
            }
        }
        floored(9, 3, 65, &walls)
    }

    fn timed_gate(
        region: ([i32; 3], [i32; 3]),
        open_ticks: u32,
        closed_ticks: u32,
    ) -> crate::plan::TimedGatePlan {
        crate::plan::TimedGatePlan {
            id: "timed-gate/piston-hall".to_string(),
            safe: "piston_hall".to_string(),
            gate_anchor: "anchor/gate".to_string(),
            gate_region: region,
            gate_block: "minecraft:iron_bars".to_string(),
            open_ticks,
            closed_ticks,
            phase: 0,
            // The DW0378 window proof is about geometry and timing, not the
            // penalty for mistiming it — crush changes neither.
            crush: false,
            // …nor does a disarm: `DW0393` is its own proof below.
            disarm: None,
        }
    }

    /// The same gate with a jam lever at `via`.
    fn timed_gate_with_disarm(
        region: ([i32; 3], [i32; 3]),
        via: [i32; 3],
    ) -> crate::plan::TimedGatePlan {
        let mut g = timed_gate(region, 60, 40);
        g.disarm = Some(crate::plan::TimedGateDisarmPlan {
            via_anchor: "anchor/jam-lever".to_string(),
            via_cell: via,
            sets_flag: "flag/gate-jammed".to_string(),
        });
        g
    }

    // --- timed-gate disarm reachability (DW0393) ---

    /// The jam lever on the ENTRY side of the barred doorway: the party walks up
    /// to the portcullis, sees the clock, and can pull the lever without ever
    /// stepping into the span. The third rung, working.
    #[test]
    fn timed_gate_disarm_on_the_approach_side_passes() {
        let world = shortcut_world(12, 9, 65, 4, 1, None);
        let g = timed_gate_with_disarm(([1, 65, 4], [1, 66, 4]), [3, 65, 2]);
        verify_timed_gate_disarms(&world, &[g], Some([1, 65, 1]))
            .expect("a lever on the near side of the gate is reachable before the crossing");
    }

    /// The same lever moved past the doorway, with the gate the only hole in the
    /// wall: the only route to it is through the portcullis, so the "disarm"
    /// rewards a crossing the party already survived. `DW0393`.
    #[test]
    fn timed_gate_disarm_behind_its_own_gate_is_dw0393() {
        let world = shortcut_world(12, 9, 65, 4, 1, None);
        let g = timed_gate_with_disarm(([1, 65, 4], [1, 66, 4]), [3, 65, 7]);
        let err = verify_timed_gate_disarms(&world, &[g], Some([1, 65, 1]))
            .expect_err("a lever only reachable through the gate must fail");
        assert_eq!(err.code, DW_TIMED_GATE_DISARM_UNREACHABLE); // DW0393
        assert!(
            err.message.contains("timed-gate/piston-hall"),
            "{}",
            err.message
        );
        assert!(err.message.contains("anchor/jam-lever"), "{}", err.message);
    }

    /// …and with a bypass hole in the same wall, that far-side lever is reachable
    /// the long way round while the gate is shut, so the same geometry passes. The
    /// proof is about pre-commitment, not about which side of a wall a cell is on.
    #[test]
    fn timed_gate_disarm_behind_the_gate_but_reachable_the_long_way_passes() {
        let world = shortcut_world(12, 9, 65, 4, 1, Some(10));
        let g = timed_gate_with_disarm(([1, 65, 4], [1, 66, 4]), [3, 65, 7]);
        verify_timed_gate_disarms(&world, &[g], Some([1, 65, 1]))
            .expect("a detour around the gate makes the lever pre-commitment ground");
    }

    /// A gate with no `disarm` is not judged at all — the whole proof is vacuous
    /// for every campaign authored before the field existed.
    #[test]
    fn timed_gate_without_a_disarm_is_not_judged() {
        let world = shortcut_world(12, 9, 65, 4, 1, None);
        let g = timed_gate(([1, 65, 4], [1, 66, 4]), 60, 40);
        verify_timed_gate_disarms(&world, &[g], Some([1, 65, 1]))
            .expect("no disarm, nothing to prove");
    }

    /// A generous window: crossing a 1-cell doorway costs a handful of ticks and
    /// the gate stands open for 60 of every 100, so most of the cycle is a legal
    /// entry. A readable gate.
    #[test]
    fn timed_gate_with_a_generous_window_is_readable() {
        let world = shortcut_world(12, 9, 65, 4, 1, None);
        let g = timed_gate(([1, 65, 4], [1, 66, 4]), 60, 40);
        verify_timed_gates(&world, &[g]).expect("60 open of a 100-tick cycle is a timing read");
    }

    /// The same span with an open window barely longer than the crossing itself:
    /// almost every entry phase is a death, so the gate is a coin flip. `DW0378`.
    #[test]
    fn timed_gate_whose_window_barely_admits_a_crossing_is_dw0378() {
        let world = shortcut_world(12, 9, 65, 4, 1, None);
        // Crossing the doorway is 2 moves = 8 ticks; a 10-tick open window inside
        // a 200-tick cycle admits 3 of 200 phases = 1%.
        let g = timed_gate(([1, 65, 4], [1, 66, 4]), 10, 190);
        let err = verify_timed_gates(&world, &[g])
            .expect_err("a window that admits ~1% of the cycle is a slot machine");
        assert_eq!(err.code, DW_TIMED_GATE_COIN_FLIP); // DW0378
        assert!(
            err.message.contains("coin flip"),
            "the message must name the failure: {}",
            err.message
        );
    }

    /// An open window SHORTER than the crossing admits nothing at all — the
    /// degenerate end of the same rule, and the one a player can never learn.
    #[test]
    fn timed_gate_no_one_can_ever_cross_is_dw0378() {
        let world = shortcut_world(12, 9, 65, 4, 1, None);
        let g = timed_gate(([1, 65, 4], [1, 66, 4]), 2, 20);
        let err = verify_timed_gates(&world, &[g])
            .expect_err("a window shorter than the crossing admits no phase at all");
        assert_eq!(err.code, DW_TIMED_GATE_COIN_FLIP); // DW0378
    }

    // --- hazard observability (spec-0016 §4 addendum, DW0388) ---

    /// The y every observability fixture walks on. Feet at `WY`, head at `WY + 1`.
    const WY: i32 = 65;

    /// A synthetic world stated in terms of what is **open**: `open` lists the
    /// `(x, z)` columns a player can stand in at [`WY`]. Everything else inside the
    /// padded bounding box is solid rock at feet and head height, with a floor
    /// below and a lid above. Sightlines are the whole subject here, so describing
    /// the carved space directly is what makes each fixture's geometry readable.
    fn carved(open: &[[i32; 2]]) -> World {
        let air: BTreeSet<[i32; 2]> = open.iter().copied().collect();
        let xs: Vec<i32> = open.iter().map(|c| c[0]).collect();
        let zs: Vec<i32> = open.iter().map(|c| c[1]).collect();
        let (x0, x1) = (xs.iter().min().unwrap() - 3, xs.iter().max().unwrap() + 3);
        let (z0, z1) = (zs.iter().min().unwrap() - 3, zs.iter().max().unwrap() + 3);
        let mut solid = BTreeSet::new();
        for x in x0..=x1 {
            for z in z0..=z1 {
                solid.insert([x, WY - 1, z]); // floor
                solid.insert([x, WY + 2, z]); // lid
                if !air.contains(&[x, z]) {
                    solid.insert([x, WY, z]);
                    solid.insert([x, WY + 1, z]);
                }
            }
        }
        World::from_solid_cells(solid)
    }

    /// A one-wide run of open columns along z at a fixed x.
    fn run_z(x: i32, z0: i32, z1: i32) -> Vec<[i32; 2]> {
        (z0..=z1).map(|z| [x, z]).collect()
    }

    /// A one-wide run of open columns along x at a fixed z.
    fn run_x(z: i32, x0: i32, x1: i32) -> Vec<[i32; 2]> {
        (x0..=x1).map(|x| [x, z]).collect()
    }

    /// A gate-shaped hazard: the full-height column at `(x, z)`.
    fn hazard(x: i32, z: i32) -> TimedHazard {
        TimedHazard {
            id: "timed-gate/portcullis".to_string(),
            region: ([x, WY, z], [x, WY + 1, z]),
        }
    }

    /// A straight hall with the portcullis at the far end: every cell of the
    /// approach looks right down the barrel of it, so the player can stand a good
    /// way back and watch a whole cycle before stepping in. This is the shape the
    /// dossier calls fair (§5.3 rule 1) and the proof must let it through.
    #[test]
    fn hazard_at_the_end_of_a_straight_hall_is_observable() {
        let world = carved(&run_z(0, 0, 12));
        let found = verify_hazard_observability(&world, &[hazard(0, 12)], Some([0, WY, 0]));
        assert!(
            found.is_empty(),
            "a hall you can see down is the observable case: {found:#?}"
        );
    }

    /// The seeded violation: the same portcullis put four blocks around a blind
    /// corner. Every cell far enough back to be safety is in the other leg of the
    /// L and sees rock; every cell that sees the gate is already inside the
    /// commitment radius. The Capra door — you meet the hazard for the first time
    /// with no read available. `DW0388`.
    #[test]
    fn hazard_around_a_blind_corner_is_dw0388() {
        let mut open = run_z(0, 0, 8);
        open.extend(run_x(8, 0, 4));
        let world = carved(&open);
        let found = verify_hazard_observability(&world, &[hazard(4, 8)], Some([0, WY, 0]));
        assert_eq!(found.len(), 1, "the blind corner is reported: {found:#?}");
        assert_eq!(found[0].code, DW_HAZARD_UNOBSERVABLE); // DW0388
        assert!(
            found[0].message.contains("cannot be watched"),
            "the message must name the failure: {}",
            found[0].message
        );
    }

    /// The same blind corner with a watch bay: the corner leg is continued PAST
    /// the junction, so the approach opens onto ground that stands off the gate and
    /// looks straight down the hall at it. This is the bell remake's "roofed bay
    /// six blocks out" (REMAKE §7.4 entry O), and it is the fix the diagnostic
    /// prescribes — the geometry changes, never the floor.
    #[test]
    fn a_watch_bay_off_the_approach_restores_observability() {
        let mut open = run_z(0, 0, 8);
        open.extend(run_x(8, -6, 4));
        let world = carved(&open);
        let found = verify_hazard_observability(&world, &[hazard(4, 8)], Some([0, WY, 0]));
        assert!(
            found.is_empty(),
            "a bay with a sightline is exactly what the proof asks for: {found:#?}"
        );
    }

    /// The load-bearing clause: the sightline must be reachable WITHOUT entering
    /// the hazard. One hall, one gate, one long clear view of it — but the view is
    /// all on the far side, and the near approach is too short to stand off in. A
    /// bay you can only reach by first surviving the gate is not a watch bay; the
    /// identical world entered from the far end passes, which is the whole
    /// difference.
    #[test]
    fn a_sightline_only_reachable_through_the_hazard_does_not_count() {
        let world = carved(&run_z(0, 0, 20));
        let h = [hazard(0, 4)];
        let blind = verify_hazard_observability(&world, &h, Some([0, WY, 0]));
        assert_eq!(blind.len(), 1, "entered from the short side: {blind:#?}");
        assert_eq!(blind[0].code, DW_HAZARD_UNOBSERVABLE); // DW0388
        let seen = verify_hazard_observability(&world, &h, Some([0, WY, 20]));
        assert!(
            seen.is_empty(),
            "the same geometry entered from the long side is observable: {seen:#?}"
        );
    }

    /// Tiering (spec-0016 §4 addendum): the same finding fails the build for a
    /// campaign that declares a `bonfire` — souls content, where
    /// observe-before-commit is the contract the retry loop rests on — and is
    /// advisory everywhere else.
    #[test]
    fn hazard_observability_is_error_tier_only_for_souls_campaigns() {
        let mut open = run_z(0, 0, 8);
        open.extend(run_x(8, 0, 4));
        let world = carved(&open);
        let found = verify_hazard_observability(&world, &[hazard(4, 8)], Some([0, WY, 0]));
        let warned = hazard_tier(false, found.clone()).expect("non-souls stays advisory");
        assert_eq!(warned.len(), 1);
        assert_eq!(warned[0].code, DW_HAZARD_UNOBSERVABLE); // DW0388
        assert_eq!(warned[0].severity, delvewright_dsl::Severity::Warning);
        let err = hazard_tier(true, found).expect_err("a souls campaign fails the build");
        assert_eq!(err.code, DW_HAZARD_UNOBSERVABLE); // DW0388
    }

    /// A campaign with no resolvable entry anchor raises nothing here: `DW0345`
    /// owns that failure, and a proof that piles a second diagnostic on the same
    /// root cause sends the author chasing the wrong fix.
    #[test]
    fn no_campaign_entry_leaves_the_hazard_to_dw0345() {
        let mut open = run_z(0, 0, 8);
        open.extend(run_x(8, 0, 4));
        let world = carved(&open);
        assert!(verify_hazard_observability(&world, &[hazard(4, 8)], None).is_empty());
    }

    /// Box distance is a Chebyshev reach to the span, zero inside it, and is what
    /// both the standoff floor and the search bound are measured in.
    #[test]
    fn box_distance_is_chebyshev_to_the_span() {
        let region = ([0, 65, 0], [2, 67, 2]);
        assert_eq!(box_distance([1, 66, 1], region), 0);
        assert_eq!(box_distance([-5, 66, 1], region), 5);
        assert_eq!(box_distance([7, 66, 4], region), 5);
        assert_eq!(box_distance([0, 72, 0], region), 5);
    }

    fn ambush(at: [i32; 3], actor_cells: Vec<[i32; 3]>) -> crate::plan::AmbushPlan {
        crate::plan::AmbushPlan {
            id: "ambush/stair-turn".to_string(),
            at,
            actor_cells,
        }
    }

    /// An ambush in an open room: the ambusher stands beside the player, so a
    /// retreat to the entry is still walkable. Un-telegraphed and lethal is fine
    /// — there is a play on the retry, which is all the engine owes.
    #[test]
    fn ambush_in_open_ground_has_counterplay() {
        let world = floored(9, 9, 65, &[]);
        let amb = ambush([4, 65, 4], vec![[5, 65, 4]]);
        verify_ambushes(&world, &[amb], &[[0, 65, 0]])
            .expect("an ambusher in open ground never seals the room");
    }

    /// A 1-wide corridor with the ambusher between the player and everything
    /// behind them: no retreat, no luring ground, no exit. `DW0376`.
    #[test]
    fn ambush_that_seals_the_only_way_out_is_dw0376() {
        let world = walled_corridor();
        // Player at the dead end (x=8); the ambusher spawns at x=7, behind them.
        let amb = ambush([8, 65, 1], vec![[7, 65, 1]]);
        let err = verify_ambushes(&world, &[amb], &[[0, 65, 1]])
            .expect_err("an ambush that corks the only corridor has no counterplay");
        assert_eq!(err.code, DW_AMBUSH_NO_COUNTERPLAY); // DW0376
        assert!(
            err.message.contains("no counterplay"),
            "the message must name the missing play: {}",
            err.message
        );
    }

    /// The same corridor, but a bonfire sits at the dead end WITH the player:
    /// dying is now cheap and the retry is a real second attempt, so the beat
    /// carries no obligation.
    #[test]
    fn ambush_with_a_rest_point_on_the_players_side_has_counterplay() {
        let world = walled_corridor();
        let amb = ambush([8, 65, 1], vec![[7, 65, 1]]);
        verify_ambushes(&world, &[amb], &[[0, 65, 1], [8, 65, 1]])
            .expect("a rest point on the player's own side is a play");
    }

    /// A synthetic shortcut-door world (spec-0016 §2): a room `w × d` split by a
    /// solid wall at `z = zw`, with a 1-cell **gate** doorway at `x = gx` and an
    /// optional **bypass** hole at `x = bx` (the long way round). The gate cells
    /// are open in the base world — the assembled model always clears a gate
    /// region — and the proof re-seals them itself.
    fn shortcut_world(w: i32, d: i32, y: i32, zw: i32, gx: i32, bypass: Option<i32>) -> World {
        let mut walls = Vec::new();
        for x in 0..w {
            if x == gx || Some(x) == bypass {
                continue;
            }
            walls.push([x, y, zw]);
            walls.push([x, y + 1, zw]);
        }
        floored(w, d, y, &walls)
    }

    /// A shortcut plan over a 1-cell gate column at `(gx, y..y+1, zw)`.
    fn shortcut(gx: i32, y: i32, zw: i32, unlock: [i32; 3]) -> crate::plan::ShortcutPlan {
        crate::plan::ShortcutPlan {
            id: "shortcut/lift".to_string(),
            safe: "lift".to_string(),
            gate_anchor: "anchor/gate".to_string(),
            gate_region: ([gx, y, zw], [gx, y + 1, zw]),
            gate_block: "minecraft:iron_bars".to_string(),
            unlock_anchor: "anchor/lift-lever".to_string(),
            unlock,
            on_unlock: Vec::new(),
            sealed_side: crate::wrongside::derive(([gx, y, zw], [gx, y + 1, zw]), unlock),
        }
    }

    /// The happy path: a wall with a barred doorway AND a far bypass hole. The
    /// unlock is reachable the long way while the gate is sealed, and opening the
    /// gate genuinely shortens the crossing — a real shortcut.
    #[test]
    fn shortcut_with_a_long_way_round_passes_both_proofs() {
        let world = shortcut_world(12, 9, 65, 4, 1, Some(10));
        let sc = shortcut(1, 65, 4, [1, 65, 7]);
        verify_shortcuts(&world, &[sc], Some([1, 65, 1]))
            .expect("a gate with a genuine detour around it is a real shortcut");
    }

    /// No bypass: sealing the gate cuts the room in two, so the unlock on the far
    /// side can never be reached the hard way — the mechanism that opens the
    /// shortcut is behind the shortcut. `DW0373`.
    #[test]
    fn shortcut_whose_unlock_is_only_behind_its_own_gate_is_dw0373() {
        let world = shortcut_world(12, 9, 65, 4, 1, None);
        let sc = shortcut(1, 65, 4, [1, 65, 7]);
        let err = verify_shortcuts(&world, &[sc], Some([1, 65, 1]))
            .expect_err("a shortcut with no long route must fail");
        assert_eq!(err.code, DW_SHORTCUT_NO_LONG_ROUTE); // DW0373
        assert!(
            err.message.contains("no long route"),
            "the message must name the missing long route: {}",
            err.message
        );
    }

    /// The classic leak: the `unlock` sits on the NEAR side of its own gate, so
    /// the player can pull it without ever earning the far side — and opening the
    /// gate measurably changes nothing about reaching it. `DW0374`.
    #[test]
    fn shortcut_whose_unlock_is_on_the_near_side_is_dw0374() {
        let world = shortcut_world(12, 9, 65, 4, 1, Some(10));
        // Entry z=1, wall z=4: an unlock at z=2 is on the entry's own side.
        let sc = shortcut(1, 65, 4, [5, 65, 2]);
        let err = verify_shortcuts(&world, &[sc], Some([1, 65, 1]))
            .expect_err("an unlock the gate does not stand in front of is a leak");
        assert_eq!(err.code, DW_SHORTCUT_NO_GAIN); // DW0374
        assert!(
            err.message.contains("leaks"),
            "the message must name the leak: {}",
            err.message
        );
    }

    /// A minimal lethal trap for the proof tests.
    fn lethal_trap(cell: [i32; 3], reset: TrapReset, disarm: Option<TrapDisarmPlan>) -> TrapPlan {
        TrapPlan {
            id: "trap/darts".to_string(),
            safe: "darts".to_string(),
            trigger: TrapTrigger::PressurePlate,
            at_anchor: "anchor/trap".to_string(),
            trigger_cell: cell,
            dispenser: None,
            payload: None,
            payload_effects: Vec::new(),
            lethality: Lethality::Lethal,
            reset,
            disarm,
            requires_flags: Vec::new(),
            forbids_flags: Vec::new(),
            requires_state: Vec::new(),
        }
    }

    #[test]
    fn forced_lethal_rearm_trap_with_no_discharge_is_dw0342() {
        // A rearming lethal trap on a required chokepoint, no disarm → soft-loop.
        let world = corridor(6, 65);
        let tc = [3, 65, 1];
        let required: BTreeSet<[i32; 3]> = (0..6).map(|x| [x, 65, 1]).collect();
        let traps = [lethal_trap(tc, TrapReset::Rearm, None)];
        let err = verify_traps(&world, &traps, &required, &[[0, 65, 1]], &[]).unwrap_err();
        assert_eq!(err.code, DW_TRAP_LETHAL_UNAVOIDABLE);
    }

    #[test]
    fn forced_lethal_once_trap_is_survivable() {
        // The same forced trap set to `once` fires and is spent — no soft-loop.
        let world = corridor(6, 65);
        let tc = [3, 65, 1];
        let required: BTreeSet<[i32; 3]> = (0..6).map(|x| [x, 65, 1]).collect();
        let traps = [lethal_trap(tc, TrapReset::Once, None)];
        assert!(verify_traps(&world, &traps, &required, &[[0, 65, 1]], &[]).is_ok());
    }

    #[test]
    fn off_path_lethal_trap_is_avoidable() {
        // A rearming lethal trap whose trigger cell is NOT a required path cell.
        let world = corridor(6, 65);
        let tc = [3, 65, 1];
        let required: BTreeSet<[i32; 3]> = BTreeSet::new(); // path avoids the trap
        let traps = [lethal_trap(tc, TrapReset::Rearm, None)];
        assert!(verify_traps(&world, &traps, &required, &[[0, 65, 1]], &[]).is_ok());
    }

    #[test]
    fn forced_lethal_trap_with_reachable_disarm_is_discharged() {
        // Disarm affordance BEFORE the trap on the corridor (reachable from spawn
        // without crossing the trap cell) → disarmable.
        let world = corridor(6, 65);
        let tc = [4, 65, 1];
        let required: BTreeSet<[i32; 3]> = (0..6).map(|x| [x, 65, 1]).collect();
        let disarm = TrapDisarmPlan {
            via_anchor: "anchor/lever".to_string(),
            via_cell: [1, 65, 1],
            sets_flag: "flag/darts-off".to_string(),
        };
        let traps = [lethal_trap(tc, TrapReset::Rearm, Some(disarm))];
        assert!(verify_traps(&world, &traps, &required, &[[0, 65, 1]], &[]).is_ok());
    }

    #[test]
    fn forced_lethal_trap_with_disarm_behind_the_trap_is_dw0342() {
        // The only route to the disarm crosses the trap chokepoint, so the disarm
        // cannot be reached first → still a soft-loop → DW0342.
        let world = corridor(6, 65);
        let tc = [3, 65, 1];
        let required: BTreeSet<[i32; 3]> = (0..6).map(|x| [x, 65, 1]).collect();
        let disarm = TrapDisarmPlan {
            via_anchor: "anchor/lever".to_string(),
            via_cell: [5, 65, 1],
            sets_flag: "flag/darts-off".to_string(),
        };
        let traps = [lethal_trap(tc, TrapReset::Rearm, Some(disarm))];
        let err = verify_traps(&world, &traps, &required, &[[0, 65, 1]], &[]).unwrap_err();
        assert_eq!(err.code, DW_TRAP_LETHAL_UNAVOIDABLE);
    }

    #[test]
    fn non_lethal_forced_trap_carries_no_obligation() {
        // A harmful (non-lethal) trap on the forced path is fine — no DW0342.
        let world = corridor(6, 65);
        let mut t = lethal_trap([3, 65, 1], TrapReset::Rearm, None);
        t.lethality = Lethality::Harmful;
        let required: BTreeSet<[i32; 3]> = (0..6).map(|x| [x, 65, 1]).collect();
        assert!(verify_traps(&world, &[t], &required, &[[0, 65, 1]], &[]).is_ok());
    }

    // --- partial floor heights: a physical step rule ---------------

    /// A world from an explicit cell→block map, through the real classifier — the
    /// only way to exercise the partial-height model end to end.
    fn blocks_world(cells: &[([i32; 3], &str)]) -> World {
        let map: BTreeMap<[i32; 3], String> =
            cells.iter().map(|(c, n)| (*c, (*n).to_string())).collect();
        World::from_occupancy(crate::assembled::occupancy_of(map, &BTreeSet::new()))
    }

    #[test]
    fn step_up_from_a_bottom_slab_onto_a_full_block_is_impossible() {
        // THE regression the full-cube model proved wrong. Standing on a bottom
        // slab puts the feet at y=65.5; the neighbouring ledge's top face is at
        // y=67.0 — a **1.5-block** rise, past the ~1.25-block jump apex. The old
        // model saw an ordinary "+1 cell" step (feet cell 66 → 67) and proved a
        // route no player and no mineflayer bot can walk.
        let world = blocks_world(&[
            ([0, 64, 0], "minecraft:stone"), // support under the slab
            ([0, 65, 0], "minecraft:oak_slab[type=bottom]"), // stand at y=65.5
            ([1, 64, 0], "minecraft:stone"),
            ([1, 65, 0], "minecraft:stone"),
            ([1, 66, 0], "minecraft:stone"), // ledge top at y=67.0
        ]);
        // Both standing cells are standable in isolation…
        assert!(world.is_standable([0, 66, 0]), "the slab top is standable");
        assert!(world.is_standable([1, 67, 0]), "the ledge top is standable");
        // …but no step connects them: 1.5 blocks is not jumpable.
        assert!(
            !world.neighbors([0, 66, 0]).contains(&[1, 67, 0]),
            "a 1.5-block rise must not be a legal step: {:?}",
            world.neighbors([0, 66, 0])
        );
        assert!(
            world.find_path([0, 66, 0], [1, 67, 0]).is_none(),
            "no route may cross an unjumpable rise"
        );
        // The same ledge one cell lower IS reachable — a 0.5-block auto-step off
        // the slab. The rule rejects the impossible rise, not the block kind.
        let ok = blocks_world(&[
            ([0, 64, 0], "minecraft:stone"),
            ([0, 65, 0], "minecraft:oak_slab[type=bottom]"),
            ([1, 64, 0], "minecraft:stone"),
            ([1, 65, 0], "minecraft:stone"),
        ]);
        assert!(
            ok.neighbors([0, 66, 0]).contains(&[1, 66, 0]),
            "slab top → full block top is a 0.5 auto-step: {:?}",
            ok.neighbors([0, 66, 0])
        );
    }

    #[test]
    fn step_up_onto_a_bottom_slab_needs_no_jump_headroom() {
        // The other direction — a step vanilla ADMITS that the full-cube model
        // refused. From a full floor onto a bottom slab is a 0.5-block rise: an
        // auto-step (vanilla `maxUpStep` 0.6), not a jump, so a ceiling directly
        // over the walker's jump arc is irrelevant. The old rule treated it as a
        // "+1 cell" jump and demanded head clearance that a real player never
        // needs.
        let world = blocks_world(&[
            ([0, 64, 0], "minecraft:stone"), // stand at y=65
            ([1, 64, 0], "minecraft:stone"),
            ([1, 65, 0], "minecraft:oak_slab[type=bottom]"), // slab top y=65.5
            ([0, 67, 0], "minecraft:stone"),                 // ceiling over the source
        ]);
        assert!(world.is_standable([0, 65, 0]));
        assert!(world.is_standable([1, 66, 0]));
        assert!(
            world.neighbors([0, 65, 0]).contains(&[1, 66, 0]),
            "a 0.5-block auto-step must be legal even under a low ceiling: {:?}",
            world.neighbors([0, 65, 0])
        );
    }

    #[test]
    fn top_slab_and_double_slab_are_full_height_steps() {
        // A `type=top` slab's walkable face IS the cell top, so stepping onto it
        // from a full floor one cell down is an ordinary 1.0-block jump — legal
        // with headroom. The half-step rule must key on the slab HALF, not on the
        // word "slab".
        let world = blocks_world(&[
            ([0, 64, 0], "minecraft:stone"),
            ([1, 64, 0], "minecraft:stone"),
            ([1, 65, 0], "minecraft:oak_slab[type=top]"),
        ]);
        assert!(
            world.neighbors([0, 65, 0]).contains(&[1, 66, 0]),
            "a top slab is a full-height step up: {:?}",
            world.neighbors([0, 65, 0])
        );
        // And from a top slab, the next full block one cell up is a normal 1.0
        // rise — unlike the bottom-slab case above, this stays legal.
        let world = blocks_world(&[
            ([0, 64, 0], "minecraft:stone"),
            ([0, 65, 0], "minecraft:oak_slab[type=top]"),
            ([1, 64, 0], "minecraft:stone"),
            ([1, 65, 0], "minecraft:stone"),
        ]);
        assert!(
            world.neighbors([0, 66, 0]).contains(&[1, 66, 0]),
            "top slab → full block at the same standing cell is level"
        );
    }

    #[test]
    fn snow_layers_step_by_layer_count_and_thin_snow_is_walked_over() {
        // `snow` collision is `(layers-1)*2/16`: one layer has NO collision box at
        // all (walked straight over — the floor is what is under it), five layers
        // is a half-block auto-step, and a deep drift plus a full block above is
        // past the jump apex.
        let thin = blocks_world(&[
            ([0, 64, 0], "minecraft:stone"),
            ([0, 65, 0], "minecraft:snow[layers=1]"),
        ]);
        assert!(
            thin.is_standable([0, 65, 0]),
            "a single snow layer is walked through, not stood on top of"
        );
        let drift = blocks_world(&[
            ([0, 64, 0], "minecraft:stone"),
            ([1, 64, 0], "minecraft:stone"),
            ([1, 65, 0], "minecraft:snow[layers=5]"), // top at +0.5
        ]);
        assert!(
            drift.neighbors([0, 65, 0]).contains(&[1, 66, 0]),
            "a 5-layer drift is a 0.5-block auto-step: {:?}",
            drift.neighbors([0, 65, 0])
        );
        let over = blocks_world(&[
            ([0, 64, 0], "minecraft:stone"),
            ([0, 65, 0], "minecraft:snow[layers=5]"), // stand at y=65.5
            ([1, 64, 0], "minecraft:stone"),
            ([1, 65, 0], "minecraft:stone"),
            ([1, 66, 0], "minecraft:stone"), // ledge top at y=67.0
        ]);
        assert!(
            !over.neighbors([0, 66, 0]).contains(&[1, 67, 0]),
            "1.5 blocks up off a drift is not jumpable: {:?}",
            over.neighbors([0, 66, 0])
        );
    }

    // --- exact camera clip test -----------------------------------

    #[test]
    fn first_clip_catches_a_corner_graze_the_old_sampler_missed() {
        // A single solid cell the dolly cuts diagonally through the corner of. The
        // old 0.25-sampler stepped over it (the segment is inside the cell for far
        // less than one sample); the DDA walk visits every cell the segment
        // touches, so the clip is caught.
        let world = World::from_solid_cells([[1, 0, 1]].into_iter().collect());
        let pts = [[0.9, 0.5, 0.9], [1.2, 0.5, 1.2]];
        assert_eq!(
            first_clip(&world, &pts).map(|(_, c)| c),
            Some([1, 0, 1]),
            "the grazed cell must be reported"
        );
        // A parallel path that never enters the cell stays clean.
        let clear = [[0.9, 0.5, 0.9], [0.9, 0.5, 2.5]];
        assert_eq!(first_clip(&world, &clear), None);
    }

    #[test]
    fn walk_cells_visits_every_cell_in_order() {
        // A long axis-aligned run visits each cell once, in order; the first
        // matching cell wins.
        let seen = std::cell::RefCell::new(Vec::new());
        let out = walk_cells([0.5, 0.5, 0.5], [4.5, 0.5, 0.5], |c| {
            seen.borrow_mut().push(c);
            c[0] == 3
        });
        assert_eq!(out, Some([3, 0, 0]));
        assert_eq!(
            *seen.borrow(),
            vec![[0, 0, 0], [1, 0, 0], [2, 0, 0], [3, 0, 0]]
        );
    }

    // --- stealth zones: reachable-ANY, not reachable-centre --------

    #[test]
    fn stealth_zone_passes_when_any_zone_cell_is_reachable() {
        // The zone box straddles a wall: its CENTRE snaps to a standable cell in a
        // walled-off pocket, while the rest of the box is plainly reachable. The
        // obligation is "the player can reach cover somewhere in this zone", so
        // this must pass — testing only the snapped centre raised a spurious
        // DW0327.
        let mut cells: Vec<([i32; 3], &str)> = Vec::new();
        for x in 0..7 {
            for z in 0..3 {
                cells.push(([x, 64, z], "minecraft:stone")); // floor
            }
        }
        // A full-height wall at x=4 sealing off the x=5 pocket from the walkway,
        // with a 2-high stack so nothing can be jumped over.
        for z in 0..3 {
            for y in 65..=66 {
                cells.push(([4, y, z], "minecraft:stone"));
                cells.push(([5, y, z], "minecraft:stone"));
            }
        }
        // Reopen the pocket floor at [5,65,1] by removing the wall cell there.
        cells.retain(|(c, _)| *c != [5, 65, 1] && *c != [5, 66, 1]);
        let world = blocks_world(&cells);
        // Zone box x 3..=5 at y=65: [5,65,1] (the pocket) and [3,65,z] (the open
        // walkway) are both standable, but only the walkway is reachable.
        assert!(world.is_standable([5, 65, 1]), "the pocket is standable");
        assert!(world.is_standable([3, 65, 1]), "the walkway is standable");
        let beats = vec![(
            vec![("zone/shadow".to_string(), [4, 65, 1], [1, 0, 1])],
            0usize,
        )];
        assert!(
            verify_stealth(&world, &beats, &[at_step([0, 65, 1], 0)]).is_ok(),
            "a zone with ANY reachable standable cell must pass"
        );
    }

    // --- causally-sealed waypoint export + trap forcing ------------

    /// A 5-long, 3-wide room at y=65 whose only two lanes (z=0 and z=2) run from
    /// x=0 to x=4; the middle lane z=1 is walled. Sealing one lane's chokepoint
    /// forces the route onto the other.
    fn two_lane_room(y: i32) -> World {
        let mut cells: Vec<([i32; 3], &str)> = Vec::new();
        for x in 0..5 {
            for z in 0..3 {
                cells.push(([x, y - 1, z], "minecraft:stone"));
            }
            // The middle lane is a solid wall at stand + head height.
            cells.push(([x, y, 1], "minecraft:stone"));
            cells.push(([x, y + 1, 1], "minecraft:stone"));
        }
        blocks_world(&cells)
    }

    #[test]
    fn exported_waypoints_never_route_through_a_sealed_gate() {
        // The z=0 lane is the short way; a `close-gate` seals its chokepoint
        // [2,65,0] before the leg is walked. The completability proof already
        // routed the detour — the EXPORT must agree, or the harness bot is handed
        // a route through a boulder that has already dropped.
        let mut world = two_lane_room(65);
        // Join the two lanes at both ends so a detour exists.
        world.solid.remove(&[0, 65, 1]);
        world.solid.remove(&[0, 66, 1]);
        world.solid.remove(&[4, 65, 1]);
        world.solid.remove(&[4, 66, 1]);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        let close = RegionEvent::forced(([2, 65, 0], [2, 66, 0]), RegionWrite::Fill, 0);
        let open_legs = route_walked_legs(&world, &[a, b], &[], &linear);
        assert!(
            open_legs[0].0.cells.contains(&[2, 65, 0]),
            "with the gate open the export takes the short lane"
        );
        let sealed_legs = route_walked_legs(&world, &[a, b], std::slice::from_ref(&close), &linear);
        assert_eq!(sealed_legs.len(), 1, "the leg is still routable via z=2");
        assert!(
            !sealed_legs[0].0.cells.contains(&[2, 65, 0]),
            "an exported waypoint must never cross a sealed gate cell: {:?}",
            sealed_legs[0].0.cells
        );
        assert!(
            sealed_legs[0].0.cells.contains(&[2, 65, 2]),
            "the export takes the detour lane the proof routed"
        );
    }

    #[test]
    fn a_lethal_plate_on_a_close_gate_detour_is_forced_and_dw0342() {
        // Same room. A rearming lethal plate sits on the DETOUR lane at [2,65,2].
        // With the gate open the player walks the short lane and the trap is
        // genuinely avoidable. Once the `close-gate` seals the short lane, the
        // detour is forced and the plate becomes a provable soft-loop — which the
        // old unsealed forced-cell set could not see.
        let mut world = two_lane_room(65);
        world.solid.remove(&[0, 65, 1]);
        world.solid.remove(&[0, 66, 1]);
        world.solid.remove(&[4, 65, 1]);
        world.solid.remove(&[4, 66, 1]);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        let close = RegionEvent::forced(([2, 65, 0], [2, 66, 0]), RegionWrite::Fill, 0);
        let tc = [2, 65, 2];
        let traps = [lethal_trap(tc, TrapReset::Rearm, None)];
        let spawn = [[0, 65, 0]];

        let open_legs = route_walked_legs(&world, &[a, b], &[], &linear);
        let open_required: BTreeSet<[i32; 3]> = open_legs
            .iter()
            .flat_map(|(l, _)| l.cells.clone())
            .collect();
        assert!(
            verify_traps(&world, &traps, &open_required, &spawn, &open_legs).is_ok(),
            "with the gate open the plate is genuinely avoidable"
        );

        let sealed_legs = route_walked_legs(&world, &[a, b], std::slice::from_ref(&close), &linear);
        let sealed_required: BTreeSet<[i32; 3]> = sealed_legs
            .iter()
            .flat_map(|(l, _)| l.cells.clone())
            .collect();
        let err = verify_traps(&world, &traps, &sealed_required, &spawn, &sealed_legs)
            .expect_err("the sealed detour forces the party across the plate");
        assert_eq!(err.code, DW_TRAP_LETHAL_UNAVOIDABLE); // DW0342
    }

    // --- partial heights x ocean horizon compose ----------------

    /// The two world models are independent axes and must both stay live in one
    /// `World`: `partial` decides *which cells are reachable*, and
    /// `ambient` (spec-0013) decides *what the unmodelled columns contain*.
    ///
    /// Geometry: an ocean island whose top plate is a course of BOTTOM SLABS, so
    /// the walk plane sits at `sea_level + 0.5` rather than `sea_level + 1`. The
    /// slab cells are still `solid` (feet cell `sea_level + 1`), so the climb-out
    /// band sees the canonical beach and the coast is not a stranding — while the
    /// step rule is simultaneously reasoning in sixteenths over the same cells.
    #[test]
    fn partial_floor_heights_and_the_ocean_horizon_compose() {
        let mut cells: Vec<([i32; 3], &str)> = Vec::new();
        for x in 0..8 {
            for z in 0..8 {
                for y in 60..=61 {
                    cells.push(([x, y, z], "minecraft:stone"));
                }
                // The shore course is a bottom slab: top face at 62.5.
                cells.push(([x, 62, z], "minecraft:oak_slab[type=bottom]"));
            }
        }
        let occ = crate::assembled::occupancy_of(
            cells.iter().map(|(c, n)| (*c, (*n).to_string())).collect(),
            &BTreeSet::new(),
        );
        // The partial map is populated…
        assert_eq!(
            occ.partial.get(&[3, 62, 3]),
            Some(&8),
            "the slab course must be modelled as a half-height floor"
        );
        let world = World::from_occupancy(occ).with_ambient(
            Ambient::Ocean(Sea {
                level: 62,
                floor_top: 54,
            }),
            vec![("island".to_string(), ([0, 60, 0], [7, 62, 7]))],
        );

        // …and BOTH axes are live on the same World: the step rule sees the true
        // feet height (62 + 0.5 → 62·16 + 8 sixteenths) …
        let fp = Footprint::player();
        assert_eq!(
            world.feet_16_fp([3, 63, 3], &fp),
            62 * 16 + 8,
            "feet rest on the slab face, not the cell floor"
        );
        // … while the ocean premise still governs the boundary verdict.
        verify_boundary_safety(&world, &roots([3, 63, 3]))
            .expect("a slab-course beach is a climb-out, not a stranding");

        // The control: the identical geometry under the void premise is still a
        // void-drop error — partial heights do not disturb that verdict either.
        let voidish = World::from_occupancy(crate::assembled::occupancy_of(
            cells.iter().map(|(c, n)| (*c, (*n).to_string())).collect(),
            &BTreeSet::new(),
        ));
        let err = verify_boundary_safety(&voidish, &roots([3, 63, 3]))
            .expect_err("under `void` the same slab coast is a void drop");
        assert_eq!(err.code, DW_EDIT_BORDERS_VOID);
    }

    // --- terrain-shaped step cost (round-8 owner playtest) -------------------

    /// An open plateau: solid floor at `y=63` over `[0,w) × [0,d)`, so the walk
    /// plane is `y=64`. Each cell in `bumps` gets a block laid ON the floor, which
    /// raises the standing cell there by one — the "bumpy 1-step terrain" the herd
    /// and the giant pogo'd over on the island.
    fn plateau(w: i32, d: i32, bumps: &[[i32; 2]]) -> World {
        let mut solid = BTreeSet::new();
        for x in 0..w {
            for z in 0..d {
                solid.insert([x, 63, z]);
            }
        }
        for &[x, z] in bumps {
            solid.insert([x, 64, z]);
        }
        World::from_solid_cells(solid)
    }

    /// The cost currency: a flat step is one block of level walking, and every
    /// sixteenth of height change (either direction) costs [`ELEV_WEIGHT`] more.
    #[test]
    fn step_cost_prices_elevation_change_in_walking_distance() {
        let flat = step_cost_16(64 * 16, 64 * 16);
        assert_eq!(
            flat, STEP_COST_16,
            "a level step costs one block of walking"
        );
        // A full block up and the same block down cost alike — bobbing is bobbing.
        let up = step_cost_16(64 * 16, 65 * 16);
        let down = step_cost_16(65 * 16, 64 * 16);
        assert_eq!(up, down);
        assert_eq!(
            up,
            STEP_COST_16 * (1 + ELEV_WEIGHT),
            "a 1-block rise costs one flat step plus ELEV_WEIGHT blocks of detour"
        );
        // A half-block (slab / path lip) costs proportionally less, so intentional
        // slab stairs are not treated like lumpy ground.
        assert!(step_cost_16(64 * 16, 64 * 16 + 8) < up);
        assert!(step_cost_16(64 * 16, 64 * 16 + 8) > flat);
    }

    /// The weight's own derivation, executable at last.
    ///
    /// `ELEV_WEIGHT`'s doc comment argued from a jump arc of ≈12 airborne ticks
    /// against ≈4.6 ticks of flat walking per block, and both of those numbers
    /// lived in prose, so the arithmetic could not go red if either moved. They
    /// are entries of the metrics table now, and this asserts the relationship
    /// rather than executing it: the weight is a TUNED number an owner playtest
    /// settled (round 8), so deriving it at run time would let an edit to a
    /// physics fact silently move every route in every campaign. Asserting it
    /// makes the same edit a red that says which decision has to be re-taken.
    #[test]
    fn the_elevation_weight_is_the_integer_under_its_jump_arc() {
        use delvewright_dsl::metrics::{JUMP_AIRBORNE_TICKS, walk_ticks_per_block};
        let blocks_of_walking_per_block_of_climb = JUMP_AIRBORNE_TICKS / walk_ticks_per_block();
        assert!(
            (blocks_of_walking_per_block_of_climb - 2.59).abs() < 0.01,
            "a block of climb costs about 2.5 blocks of walking time; got {blocks_of_walking_per_block_of_climb}"
        );
        assert_eq!(
            ELEV_WEIGHT,
            blocks_of_walking_per_block_of_climb.floor() as u32,
            "the weight is deliberately the integer UNDER the physical figure — \
             overpaying for flatness is what would distort routes on legitimately \
             sloped terrain"
        );
    }

    /// The island defect in miniature: a straight lane with one 1-block bump, and
    /// a flat lane one column over. The flat road is two steps longer and must
    /// still win — this is precisely what a distance-only cost could not do.
    #[test]
    fn a_walk_takes_a_slightly_longer_flat_lane_over_a_bump() {
        // Lane x=0 carries a bump at z=5; lane x=1 is clear.
        let world = plateau(4, 11, &[[0, 5]]);
        let path = world
            .find_path([0, 64, 0], [0, 64, 10])
            .expect("both lanes connect the endpoints");
        assert!(
            !path.contains(&[0, 65, 5]),
            "the planner must route around the bump, not over it: {path:?}"
        );
        assert!(
            path.iter().any(|c| c[0] == 1),
            "the detour uses the flat lane one column over: {path:?}"
        );
        // Every cell of the chosen route is level — the whole point.
        assert!(
            path.iter().all(|c| c[1] == 64),
            "the flat route stays on one plane: {path:?}"
        );
    }

    /// The other side of the constant: flatness is preferred, not bought at any
    /// price. With the three nearest lanes all bumped, the flat lane is three
    /// columns away (six extra steps) — more than one bump is worth — so the walk
    /// correctly steps over the bump instead of touring the map to avoid it.
    #[test]
    fn a_walk_does_not_take_an_absurd_detour_to_avoid_one_bump() {
        let world = plateau(5, 11, &[[0, 5], [1, 5], [2, 5]]);
        let path = world
            .find_path([0, 64, 0], [0, 64, 10])
            .expect("the bumped lanes are still walkable");
        assert!(
            path.iter().any(|c| c[1] == 65),
            "a 6-step detour costs more than one 1-block step up: {path:?}"
        );
        assert!(
            !path.iter().any(|c| c[0] == 3),
            "the far flat lane is not worth the detour: {path:?}"
        );
    }

    /// Cost shaping changes which of several *valid* routes is chosen — never
    /// which routes exist. A bump is still walkable when it is the only way, and
    /// a genuinely disconnected goal is still unreachable (DW0307/DW0311 semantics
    /// unchanged).
    #[test]
    fn cost_shaping_does_not_change_reachability() {
        // A single lane, bumped: the only route climbs, and it is still found.
        let world = plateau(1, 11, &[[0, 5]]);
        let path = world
            .find_path([0, 64, 0], [0, 64, 10])
            .expect("a bump is a cost, never a wall");
        assert!(path.contains(&[0, 65, 5]));
        // A wall two blocks tall is still impassable.
        let mut solid = BTreeSet::new();
        for x in 0..1 {
            for z in 0..11 {
                solid.insert([x, 63, z]);
            }
        }
        solid.insert([0, 64, 5]);
        solid.insert([0, 65, 5]);
        solid.insert([0, 66, 5]);
        let walled = World::from_solid_cells(solid);
        assert!(walled.find_path([0, 64, 0], [0, 64, 10]).is_none());
    }

    /// Determinism (ADR-0006): the same world and endpoints yield the identical
    /// path every time — the frontier is ordered by `(f, g, cell)` and the costs
    /// are integers, so no float comparison or map ordering can wobble the result.
    #[test]
    fn shaped_paths_are_deterministic() {
        let world = plateau(5, 11, &[[0, 5], [2, 3], [3, 7]]);
        let first = world.find_path([0, 64, 0], [4, 64, 10]).unwrap();
        for _ in 0..8 {
            assert_eq!(world.find_path([0, 64, 0], [4, 64, 10]).unwrap(), first);
        }
    }

    // --- unforced footing (DW0546) -------------------------------------------

    /// A corridor whose floor is missing at `x = 2`: the two ends are separated by a
    /// one-cell void gap, so nothing routes end to end until something floors it.
    fn gapped_floor() -> World {
        let mut solid = BTreeSet::new();
        for x in 0..5i32 {
            if x != 2 {
                solid.insert([x, 64, 0]); // floor, minus the gap
            }
            solid.insert([x, 67, 0]); // ceiling
        }
        World::from_solid_cells(solid)
    }

    /// **The rule, at the layer it lives on.** The identical fill over the identical
    /// box carries the forced leg when the party cannot avoid causing it, and does
    /// not when they can. Only the root differs.
    ///
    /// Red before forcedness reached the geometry: both cases routed, because a fill
    /// was a fill and the model had no way to say who had to fire it.
    #[test]
    fn only_a_forced_fill_lays_footing_the_critical_path_may_use() {
        let world = gapped_floor();
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        let gap = ([2, 64, 0], [2, 64, 0]);

        let forced = RegionEvent::forced(gap, RegionWrite::Fill, 0);
        assert!(
            route_visited(&world, &[a, b], std::slice::from_ref(&forced), &linear).is_ok(),
            "a plank the party cannot avoid laying is floor they certainly have"
        );

        let unforced = RegionEvent::unforced(gap, RegionWrite::Fill, 0, "the payload of trap `t`");
        let err = route_visited(&world, &[a, b], std::slice::from_ref(&unforced), &linear)
            .expect_err("a plank laid by a skippable beat may not carry the forced path");
        assert_eq!(err.code, DW_UNFORCED_FOOTING); // DW0546
        assert!(
            err.message.contains("[2, 64, 0]..[2, 64, 0]"),
            "the message must name the box: {}",
            err.message
        );
        assert!(
            err.message.contains("the payload of trap `t`"),
            "the message must name the beat: {}",
            err.message
        );
    }

    /// The blocking half of an unforced write is credited in FULL — only the footing
    /// half is withheld. A fill laid across the corridor closes the leg whoever fires
    /// it, and the author is told which write walled them rather than sent to hunt a
    /// wedged doorway.
    ///
    /// This is the half that keeps the fix from being a quiet weakening: an unforced
    /// write is strictly *more* restrictive than a forced one, never less.
    #[test]
    fn an_unforced_fill_still_seals_and_says_which() {
        let world = floored(5, 1, 65, &[]);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        let wall = RegionEvent::unforced(
            ([2, 65, 0], [2, 66, 0]),
            RegionWrite::Fill,
            0,
            "the payload of trap `t`",
        );
        let err = route_visited(&world, &[a, b], std::slice::from_ref(&wall), &linear)
            .expect_err("an unforced fill across the corridor is still a wall");
        assert_eq!(err.code, DW_CRITICAL_UNROUTABLE); // DW0311
        assert!(
            err.message.contains("close-gate") && err.message.contains("NOT forced"),
            "the message must name the unforced write, never blame the prefab: {}",
            err.message
        );
    }

    /// **The case that keeps this from refusing correct campaigns.** A fill over a
    /// cell the world already holds solid changes nothing about footing: the box is
    /// floor whether or not the beat fires, both futures agree, and there is no
    /// uncertainty to model. Re-surfacing an existing floor is decoration, and the
    /// rule binds to laying NEW floor.
    #[test]
    fn an_unforced_fill_over_existing_floor_is_not_a_finding() {
        let world = floored(5, 1, 65, &[]);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        let repave = RegionEvent::unforced(
            ([2, 64, 0], [2, 64, 0]),
            RegionWrite::Fill,
            0,
            "the payload of trap `t`",
        );
        assert!(
            route_visited(&world, &[a, b], std::slice::from_ref(&repave), &linear).is_ok(),
            "a fill over a cell that was already floor takes nothing away"
        );
    }

    /// A **forced** write landing later on the same box restores ordinary footing,
    /// with no special case: latest-write-wins already says which firing the party
    /// will find, and the winner carries its own forcedness.
    #[test]
    fn a_later_forced_fill_wins_over_an_earlier_unforced_one() {
        let world = gapped_floor();
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 3);
        let gap = ([2, 64, 0], [2, 64, 0]);
        let events = [
            RegionEvent::unforced(gap, RegionWrite::Fill, 0, "the payload of trap `t`"),
            RegionEvent::forced(gap, RegionWrite::Fill, 2),
        ];
        assert!(
            route_visited(&world, &[a, b], &events, &linear).is_ok(),
            "a beat the party must complete re-lays the plank for certain"
        );
    }

    /// An unforced **flood** needs no split and gets none: impassable and never floor
    /// is already the pointwise-worst of "the water is there" and "it is not", so it
    /// is judged exactly as a forced flood is — `DW0544`, not `DW0546`.
    #[test]
    fn an_unforced_flood_is_judged_as_a_flood() {
        let world = floored(5, 1, 65, &[]);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        let flood = RegionEvent::unforced(
            ([2, 64, 0], [2, 64, 0]),
            RegionWrite::Flood,
            0,
            "the payload of trap `t`",
        );
        let err = route_visited(&world, &[a, b], std::slice::from_ref(&flood), &linear)
            .expect_err("a fluid fill takes the floor away whoever fires it");
        assert_eq!(err.code, DW_FLUID_FILL_ON_CRITICAL_PATH); // DW0544
    }
}
