//! Resolve a validated [`Campaign`] into a placement + naming model that
//! emission and `critical-path.json` both consume.
//!
//! ## Coordinate scheme (deterministic)
//!
//! Each stage-1 area is placed at origin `[index * AREA_SPACING, base_y, 0]`
//! (M1 has one area → `[0, 64, 0]`). The origin Y comes from the **horizon**,
//! and how it comes from it depends on what kind of datum the base has
//! (spec-0060 §3):
//!
//! - `void` and `valley` state a datum for the ORIGIN, [`BASE_Y`] (64), and
//!   every area of such a world stands on it.
//! - `ocean` states a datum for the WALK PLANE
//!   ([`crate::compiler::horizon::OCEAN_WALK_REF_Y`], 63 — one block above the
//!   sea, the vanilla-normal beach relationship), and each area's origin is
//!   DERIVED from the piece set it seats: `walk_ref_y - walk_y`. A keep
//!   interior (`walk_y` 1) is seated at 62 and stands dry at 63; an island
//!   piece (`walk_y` 3) is seated at 60.
//!
//! The retired global ocean constant was one tileset's authoring convention
//! promoted to a world constant by being the only convention that existed when
//! the ocean was built, and it is why every other piece in every library landed
//! with its box bottom under the sea.
//!
//! A prefab's local anchor position resolves to `origin + local`. All coordinates
//! are integers; no randomness is used in v0.
//!
//! ## Naming scheme (scoreboard/function-safe)
//!
//! DSL ids are type-prefixed kebab (`obj/talk`); scoreboard objectives, function
//! names and tags need `[a-z0-9_.-]`. Each id's local part (after its `/`) is
//! lowered to `_` for `-`, giving stable, collision-free names (DSL ids are
//! unique within their namespace):
//! `dw.o_<obj>`, `dw.q_<quest>`, `dw.qa_<quest>` (quest active), `dw.dlg_<npc>`,
//! tag `dw_npc_<npc>`, function `class_apply_<class>`, dialog `<npc>_<node>`.

use crate::compiler::blockout::Perturb;
use crate::compiler::continuity::NpcWhere;
use crate::compiler::failure::Failure;
use delvewright_dsl::Verb;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use delvewright_dsl::{
    Campaign, Diagnostic, DialogueEffect, DialogueId, EnvTrigger, Lethality, Npc, NpcDialogue,
    Objective, Quest, QuestEffect, Trap, TrapReset, TrapTrigger, Trigger,
};

use crate::compiler::reach::{ReachCompletion, reach_completion};
/// The anchor-role vocabulary, re-exported where the resolution lives: a
/// consumer asking this module for an entry point should not have to know that
/// the term is declared in the prefab-metadata document type.
pub use crate::compiler::registry::AnchorRole;
use crate::compiler::registry::{AnchorMeta, PrefabRegistry};
use crate::compiler::solver::{self, Facing, Rotation, SealFill, Splitmix64};
use delvewright_dsl::prefab::{GateAnchor, PrefabMeta};
use delvewright_dsl::{DwCode, ExitTier};

mod actor;
mod ambush;
mod anchors;
mod area;
mod body;
mod checkpoint;
mod class;
mod effects;
mod gate_reach;
mod lethal;
mod loot;
mod naming;
mod npc;
mod objective;
mod onkill;
mod path;
mod quest;
mod sea;
mod seal;
mod shortcut;
mod state;
mod stealth;
mod surround;
mod timed_gate;
mod trap;
mod trigger;
mod wave;

pub use ambush::*;
pub use anchors::*;
pub use area::*;
pub use body::*;
pub use checkpoint::*;
pub use class::*;
pub(crate) use effects::*;
use gate_reach::*;
pub use lethal::*;
pub use loot::*;
pub use naming::*;
pub use npc::*;
pub use objective::*;
pub use path::*;
pub use quest::*;
pub use sea::*;
pub use seal::*;
pub use shortcut::*;
pub use state::*;
pub use stealth::*;
pub use surround::*;
pub use timed_gate::*;
pub use trap::*;
pub use trigger::*;
pub use wave::*;

/// The compiled model.
pub struct Plan<'a> {
    /// The source campaign.
    pub campaign: &'a Campaign,
    /// Datapack namespace = campaign id.
    pub namespace: String,
    /// Stage-1 seed (level seed / future PRNG source).
    pub seed: u64,
    /// Area placements, in stage-1 order.
    pub areas: Vec<AreaPlacement>,
    /// Advisory findings the placement stage raised, in area order. Currently
    /// `DW0498` ([`crate::compiler::pool`]): a pool draw that seats the same anchor-bearing
    /// prefab twice, so every anchor that prefab declares has more than one
    /// carrier. Reported by [`crate::compiler::emit::build_with_warnings`], which prepends
    /// them to the build's own advisories; never fatal.
    pub warnings: Vec<Diagnostic>,
    /// Resolved absolute anchors, keyed by `(area_id, anchor_name)`, plus the
    /// roles their pieces declared ([`AnchorTable`]). Derefs to the map, so a
    /// consumer that knows the name it wants reads it as one.
    pub anchors: AnchorTable,
    /// Class selection plan (n starts at 1).
    pub classes: Vec<ClassPlan>,
    /// Per-NPC dialogue plan.
    pub npcs: Vec<NpcPlan>,
    /// The bot critical path.
    pub critical_path: Vec<Step>,
    /// Inter-area transport: objective id → absolute teleport target. When
    /// completing an objective moves the player into a different area on the
    /// critical path, the compiler teleports them to that area's entry spawn
    /// (areas sit `AREA_SPACING` apart across void; the pathfinder-free bot cannot
    /// walk between them). Emitted in that objective's completion function.
    pub transport: BTreeMap<String, [i32; 3]>,
    /// Per-step transport marker, aligned 1:1 with `critical_path`: `Some(dest)` if
    /// completing that step's objective teleports the player to `dest` (a different
    /// area), else `None`. Emitted into `critical-path.json` as the step's
    /// `transport` field so the harness can wait for the position discontinuity
    /// before starting the next step (gap 8). `None` for `select-class` /
    /// `assert-complete` and any step that does not change area.
    pub critical_path_transport: Vec<Option<[i32; 3]>>,
    /// Resolved lethal volumes (DSL v0.10, spec-0031), declaration-ordered. Empty
    /// for every campaign that declares none — which is what keeps the navigation
    /// world, the emitted tick and the build outputs byte-identical.
    pub lethal_volumes: Vec<LethalVolumePlan>,
    /// Resolved loops (spec-0086), declaration-ordered. Empty for every campaign
    /// that declares none, which keeps the region model, the emitted tick and
    /// every output byte-identical.
    pub loops: Vec<crate::compiler::r#loop::LoopPlan>,
    /// Every exercise step on the default critical path (spec-0086 §5.2), in
    /// path order.
    pub loop_exercises: Vec<crate::compiler::r#loop::ExerciseRecord>,
    /// **Every placed furniture region** (spec-0065): `(anchor name, inclusive
    /// world box)` for each anchor with `role: furniture` on each placed piece, in
    /// area order, then placed-piece order, then anchor-name order — never hash
    /// order (ADR-0006). A piece seated twice contributes its tables twice,
    /// because both copies of the blocks are in the world. Empty for every
    /// campaign whose pieces declare none, which keeps every walk proof and
    /// output byte-identical.
    pub furniture: Vec<FurnitureRegion>,
    /// Per-step stealth hint (DSL v0.4), aligned 1:1 with `critical_path`: `true`
    /// when the step's objective is `stealth`-marked → emitted as `sneak: true`.
    pub critical_path_sneak: Vec<bool>,
    /// Per-step cutscene hold, aligned 1:1 with `critical_path`: seconds from
    /// the step's completion to the end of the last cutscene the bundles it
    /// fires schedule, at any depth ([`crate::compiler::hold::after_holds`]) →
    /// emitted as `cutscene_seconds`.
    pub critical_path_cutscene: Vec<Option<u32>>,
    /// **The approved reference images the campaign directory holds**
    /// (spec-0061), so emission can write `validation/design-record.json` — the
    /// ledger the staging gate reads — on every build.
    ///
    /// Default is *no images*, and that is the honest reading rather than a
    /// placeholder: a plan built from documents alone has no directory behind
    /// it, so there are no approved image files to find. A run that has one
    /// attaches it with [`Plan::with_design_files`].
    pub design_files: crate::compiler::design::DesignFiles,
    /// What the ocean-datum invariant (`DW0344`) examined in this build, printed
    /// as its own line by [`crate::compiler::emit::build`]. `NOT_AN_OCEAN` for
    /// every world that declares another horizon.
    pub waterline: WaterlineBinding,
    /// Resolved `set-checkpoint` effects (DSL v0.6, spec-0012), content-ordered.
    pub checkpoints: Vec<CheckpointPlan>,
    /// Resolved `begin-stealth` beats (DSL v0.6, spec-0014), content-ordered.
    pub stealth_beats: Vec<StealthBeat>,
    /// Objective id → its `critical_path` step index. The inverse of a step's
    /// serving objective — used by the visual-tier POV shot planner
    /// (`crate::compiler::render_plan`) to name the objective each player-POV leg walks
    /// toward, and by the v0.6 checkpoint / stealth proofs to root a beat.
    pub objective_steps: BTreeMap<String, usize>,
    /// What the exported path fires, and where — the one input every reader of
    /// forcedness on that path takes ([`firing_of`]).
    pub(crate) path_firing: PathFiring,
    /// Resolved traps (DSL v0.6, spec-0011), content-ordered.
    pub traps: Vec<TrapPlan>,
    /// The library rigs the campaign's assemblies name (spec-0082), by
    /// `rig/<name>`, copied out of the prefab registry so every consumer of the
    /// plan reads the rig the build validated. A rig the library does not hold
    /// is absent here — validation has already refused it (`DW0935`).
    pub rigs: BTreeMap<String, delvewright_dsl::rig::Rig>,
    /// Resolved shortcut doors (spec-0016 §2), content-ordered.
    pub shortcuts: Vec<ShortcutPlan>,
    /// Resolved container fills (spec-0021), declaration-ordered.
    pub loot: Vec<LootPlan>,
    /// Resolved `collect` container adoptions (DSL v0.8), campaign-
    /// ordered. Empty for a campaign whose collects keep the compiler's chest.
    pub collect_fills: Vec<CollectFillPlan>,
    /// Resolved ambushes (spec-0016 §3), declaration-ordered.
    pub ambushes: Vec<AmbushPlan>,
    /// Resolved timed gates (spec-0016 §4), declaration-ordered.
    pub timed_gates: Vec<TimedGatePlan>,
    /// One entry per gate anchor some `close-gate` seals (DSL v0.8),
    /// in first-firing order — the seal the party can press for an answer. Empty
    /// for a campaign that never seals a gate.
    pub seal_hints: Vec<SealHintPlan>,
    /// **What every compiler-owned sealed body answers a press with** (DSL v0.11).
    /// One entry per pressable body the campaign does not answer itself — a
    /// `close-gate` seal, a sealed `shortcut` door — in that order. Empty for a
    /// campaign with neither.
    pub press_answers: Vec<PressAnswer>,
    /// Resolved gate open/close firings (DSL v0.6), content-ordered — drives the
    /// `close-gate` completability model in `crate::compiler::nav`. Empty when the campaign
    /// uses no gate effects (byte-identical routing to pre-close-gate behavior).
    pub region_events: RegionEvents,
    /// **Every contingent way the placed world stages** (spec-0042 §2.4), in
    /// area → placement → declaration order, with its world cells, its block and
    /// its direction read from the carrying piece's metadata. Empty for every
    /// world whose pieces declare none — which is every world built before this
    /// surface — so nothing about such a build moves.
    ///
    /// It is on the plan rather than recomputed per consumer because three
    /// readers need the same answer: emission (what an `open-way` fills),
    /// the completability model ([`collect_region_events`]) and the disposition
    /// gate (`crate::compiler::ways`). Two of the three deriving it independently is how a
    /// verb and its proof come to disagree about what a way is.
    pub ways: crate::compiler::ways::WayStaging,
    /// The way gate's binding ledger (`crate::compiler::ways`, spec-0042 AC11) — what the
    /// disposition enumeration examined and what it found. `None` for a world
    /// that stages no way, which emits no artifact at all: a file reading zero is
    /// a finding, and an absent file is the honest statement that there was
    /// nothing to enumerate.
    pub way_gate: Option<crate::compiler::ways::WayGate>,
    /// **Every link** (spec-0083 §3.1): a `teleport` hosted in a `triggers[]`
    /// entry declared `once: false`, in [`crate::compiler::link::collect`] order.
    /// Empty for every campaign that declares none.
    pub links: Vec<crate::compiler::link::LinkPlan>,
    /// **Every gather** — every other `teleport`. Read by the put-at population
    /// and by `DW0311`'s message; never leaned on by a route.
    pub gathers: Vec<crate::compiler::link::GatherPlan>,
    /// Per critical-path step, the links live there (indices into
    /// [`Plan::links`]): the trigger's flag gate and every `when` on the way to
    /// the teleport hold under the flags the path holds walking up to the step,
    /// and every numeric term compares true against the writes the path has
    /// performed by then (spec-0083 §3.3). Aligned 1:1 with `critical_path`.
    pub critical_path_live_links: Vec<Vec<usize>>,
    /// The links this plan's path takes ([`LinkTakes`]) — empty for a plan built
    /// without a world, and for every campaign whose walks route unaided.
    pub link_takes: LinkTakes,
    /// The blockout perturbation this plan was built under, kept so a plan
    /// rebuilt with the route proof's link decisions ([`Plan::relinked`]) is
    /// the same plan with those steps added and nothing else changed.
    pub perturb: Perturb,
    /// Per-batch affected world AABBs from the stage-7 L2 massing verbs
    /// (spec-0017), keyed by batch id — the editor's per-batch snapshot
    /// framing for massing batches. Empty for a campaign without massing.
    pub massing_bounds: BTreeMap<String, ([i32; 3], [i32; 3])>,
    /// For each `critical_path` **arrival** step, the set of steps of its **strict
    /// DAG ancestors** — objectives guaranteed to complete before it in *every* valid
    /// play order (transitive `after` within its quest ∪ every objective of a
    /// transitive `depends_on`-ancestor quest). The `close-gate` seal model
    /// (`crate::compiler::nav`) uses this so a gate only seals a leg whose objective is a true
    /// causal descendant of the gate's firing objective — not a parallel branch the
    /// lineariser merely interleaved ahead of it.
    ///
    /// **Every arrival is keyed, not every objective.** The path is
    /// `[select-class, objective…, assert-complete]` and a sweep runs to
    /// `critical_path.len()` inclusive, so the last two arrivals are not
    /// objectives; they carry every objective on the path, because the path holds
    /// exactly the quests campaign completion depends on and the campaign is
    /// complete by then. `DW0525` reads those two arrivals, and keying objectives
    /// alone loses both directions there: a door the party is forced to open reads
    /// shut again at the end of the delve, and a door the last beat bars goes
    /// missing. Whether a firing counts at all is forcedness, and
    /// `collect_region_events` decides that before this relation sees it.
    pub strict_ancestor_steps: BTreeMap<usize, BTreeSet<usize>>,
    /// **The derived blockout** (spec-0049 §5), for a campaign whose placement
    /// authority is a site plan. `None` for every campaign that places pieces
    /// with `areas[]` — which is why nothing about such a build moves.
    ///
    /// It is on the plan rather than recomputed per consumer for the reason
    /// [`Plan::ways`] is: three readers need the same answer — emission (which
    /// writes the mass), the stage-5 battery (which judges the built bytes
    /// against the plan it was derived from) and the relight pass (which needs
    /// the site plan's one lighting setting) — and two of them deriving it
    /// independently is how a builder and its observer come to agree about a
    /// world neither describes.
    pub blockout: Option<crate::compiler::blockout::Blockout>,
    /// **The horizon's surround** (spec-0026): compiler-generated terrain
    /// standing around the map, for a base that builds ground rather than
    /// declaring a world generator. `None` for `void` and `ocean`, which is
    /// why nothing about such a build moves.
    ///
    /// Deliberately NOT an [`AreaPlacement`]. `plan.areas` is what the boundary
    /// region derives from, what the relight pass lights, what analysis counts
    /// and what anchors resolve against — and a surround belongs to none of
    /// those. It is scenery the map stands in: a body may walk its gap floor,
    /// and every proof that reads blocks reads it, but it is not a place the
    /// campaign has content in, and the playable region must not grow to
    /// enclose a mountain range.
    ///
    /// The sites that DO need it opt in by name, and there are exactly three:
    /// [`Plan::placed_pieces`] (emission and the voxel model),
    /// [`crate::compiler::assembled::placed_blocks`] through that iterator, and the
    /// biome paint in [`crate::compiler::emit`].
    pub surround: Option<SurroundPlan>,
    /// **What the piece-mating check examined** (`DW0780`/`DW0781`), emitted as
    /// `validation/piece-mating.json`.
    ///
    /// Carried out of `build` rather than recomputed at emission, because it is
    /// a verdict about the placement this `Plan` IS — recomputing it would be a
    /// second walk that could answer differently from the one that refused.
    pub face_binding: crate::compiler::faces::FaceBinding,
}

/// Errors that stop planning (map to build failure, exit 3). Carries a stable
/// `DW03xx` build/solver diagnostic code (catalogued in
/// `docs/reference/compiler.md` §5).
#[derive(Debug)]
pub struct PlanError {
    /// The rule that refused and what the author does about it — the same
    /// [`Failure`] every other pass raises, so the code and the message are
    /// declared in one place and this type adds only what is its own.
    pub failure: Failure,
    /// Advisory findings that were raised before this error stopped planning,
    /// and that explain it. Printed alongside the failure — a `DW0305` ambiguous
    /// anchor is usually the use-site symptom of a pool `DW0498` already
    /// describes at the declaration, and dropping the explanation because the
    /// build failed is exactly the silence `DW0498` exists to remove.
    pub warnings: Vec<Diagnostic>,
}

impl PlanError {
    /// Build a plan error with an explicit code.
    pub fn new(code: DwCode, message: impl Into<String>) -> Self {
        PlanError {
            failure: Failure::new(code, message),
            warnings: Vec::new(),
        }
    }

    /// The same error, carrying the advisories that explain it.
    pub fn with_warnings(mut self, warnings: Vec<Diagnostic>) -> Self {
        self.warnings = warnings;
        self
    }
}

delvewright_dsl::dw_code! {
    /// `DW0300`: generic build/resolution failure (missing prefab metadata, unknown
    /// anchor, dependency cycle in the critical path).
    pub const DW_BUILD: DwCode = DwCode::new("DW0300", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0985`: an objective's numeric gate reads a datum only presses write (`path::drive`),
    /// and no sequence of presses within the plan's bound drives it to a value
    /// the gate accepts. The bot would walk up to the objective and wait for
    /// ever; the delve cannot be finished.
    pub const DW_GATE_UNDRIVABLE: DwCode = DwCode::new("DW0985", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0306`: gate-aware reachability deadlock (M2 fix 7). After the solver produces
    /// a layout, sealed gates are modelled as cut edges in the piece-connectivity
    /// graph; an objective whose anchor is only reachable through a gate that no
    /// earlier objective (in the quest/objective DAG order) has opened is a deadlock —
    /// the delve is unwinnable even though every anchor resolves. The canonical case:
    /// a key chest sealed behind the very gate its key opens.
    pub const DW_GATE_DEADLOCK: DwCode = DwCode::new("DW0306", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0344`: an ocean-horizon world places a piece whose declared waterline does not
    /// land at sea level — the piece floats above the sea or is drowned by it.
    ///
    /// It is also the code this invariant's **zero binding** refuses under: a gate
    /// that examined nothing has proved nothing, and the gate that examined nothing
    /// is this one, so it answers under its own name rather than under a second
    /// code. See [`WaterlineBinding::seal`].
    pub const DW_OCEAN_WATERLINE: DwCode = DwCode::new("DW0344", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0345`: the assembled world resolves **no entry anchor** — the compiler has
    /// no cell to call the campaign's start, so it cannot `setworldspawn`, cannot place
    /// a first-joining player, and cannot teleport a player who picks a class. The
    /// world then falls back to the vanilla spawn search, which a dedicated server
    /// resolves to the surface but the integrated (singleplayer) server resolves to
    /// the build floor — inside solid stone. Silent before; a hard build error now.
    pub const DW_NO_ENTRY_ANCHOR: DwCode = DwCode::new("DW0345", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0804`: two anchors in one area declare [`AnchorRole::Entry`].
    ///
    /// An area has **one** place the party arrives at. Two claims to it is a
    /// question the compiler cannot answer and must not answer quietly: picking
    /// first-wins (by piece order, or by the `BTreeMap` order of two anchor names
    /// nobody chose for their sort) is how a spawn that moved becomes a mystery
    /// nothing in the build output mentions.
    ///
    /// Only reachable through a declared role, which is the only way an area has an
    /// entry point at all. The remedy is to take the role off one of the two, which
    /// every producer can do where it wrote it: `delvec prefab anchor --no-role` on a
    /// hand-built or ingested piece, and dropping `role` from the `mark` on a
    /// grammar program.
    pub const DW_TWO_ENTRY_ANCHORS: DwCode = DwCode::new("DW0804", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0872`: **a crossing into an area with nowhere to arrive.** A leg of the
    /// party's forced route changes area, and the destination declares no entry
    /// point — so there is no cell to put the party down on and the crossing cannot
    /// be made.
    ///
    /// Areas stand [`AREA_SPACING`] blocks apart across the void, so a leg that
    /// changes area is never a walk. Before this code the crossing was simply not
    /// emitted and nothing was said: the leg then fell through to the walkability
    /// proof, which reported `DW0311` — *the player cannot walk from … to …, a
    /// wedged doorway seam, a void gap, a fence ring* — a true sentence about a
    /// route nobody was ever going to walk, and an author who does what it says
    /// goes and widens a doorway.
    ///
    /// [`DW_NO_ENTRY_ANCHOR`] is the same rule over the whole world (*no area at
    /// all declares one*); this is the same rule over the one area a body must be
    /// put down in. The quantifiers differ and so do the remedies, which is why
    /// they are two codes.
    pub const DW_CROSSING_NO_ENTRY: DwCode = DwCode::new("DW0872", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0873`: **the party's first leg is a crossing, and nothing can carry it.**
    /// The campaign spawn and the first critical objective stand in different
    /// areas.
    ///
    /// A crossing rides on the completion of the objective the party leaves from
    /// (see [`Plan::transport`]), and at the spawn the party has completed nothing.
    /// So the first leg can be neither ridden nor walked, and the delve cannot be
    /// started.
    ///
    /// This is the member the old leg enumeration missed. It paired *consecutive
    /// objectives*, so the spawn — a leg's origin that is not an objective — was in
    /// no pair: such a campaign built clean, passed every game test, and stranded
    /// the party at the spawn with the harness reporting `No path to the goal!` and
    /// no diagnostic code at all.
    pub const DW_SPAWN_LEG_CROSSES: DwCode = DwCode::new("DW0873", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0932` (spec-0083 §3.2, §3.6): a **link** whose geometry does not hold —
    /// four faults under one code, because they are one claim, *a body in this
    /// volume is carried onto a route cell*: no standable cell inside the volume
    /// performs the trigger; `to` inside `from`; `to` not standable at the
    /// teleport's tick; `from` and `to` in different areas.
    pub const DW_TELEPORT_LINK: DwCode = DwCode::new("DW0932", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0933` (spec-0083 §3.5): a `teleport` fires at or before the tick its
    /// root's `cutscene` ends, and `cs_end` undoes it. Validation tier.
    pub const DW_TELEPORT_UNDER_CUTSCENE: DwCode = DwCode::new("DW0933", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0934` (spec-0083 §7): the layout graph and the quests disagree about
    /// carries. Validation tier.
    pub const DW_TELEPORT_CARRY_UNREALISED: DwCode = DwCode::new("DW0934", ExitTier::Build);
}

/// **Which links a path takes, and where** (spec-0083 §3.4) — the route
/// proof's decision, handed back to the path builder so the steps it splices
/// are in the ONE path every consumer reads.
///
/// Keyed by step indices of the path as built WITHOUT any link; the builder is
/// deterministic, so the same campaign rebuilds the same unlinked path and the
/// keys mean the same steps. Empty for every campaign the walk proof routes
/// without a link — which is what keeps those builds byte-identical.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LinkTakes {
    /// Leg end (unlinked step index) → the links taken on that leg, in the order
    /// taken, each `(index into Plan::links, stand cell)`. Each becomes a
    /// `trigger` step spliced directly in front of the leg's end.
    pub spliced: BTreeMap<usize, Vec<(usize, [i32; 3])>>,
    /// A `trigger` step the path already performs (unlinked step index) whose
    /// trigger is a link the party stands in: `(link index, stand cell)`. A
    /// performed trigger that carries, carries.
    pub performed: BTreeMap<usize, (usize, [i32; 3])>,
}

impl LinkTakes {
    /// Whether the path takes no link at all.
    pub fn is_empty(&self) -> bool {
        self.spliced.is_empty() && self.performed.is_empty()
    }
}

/// Inter-area transport map: objective id → absolute teleport target (see
/// [`Plan::transport`]).
pub type TransportMap = BTreeMap<String, [i32; 3]>;

impl<'a> Plan<'a> {
    /// Build the plan. Requires a validated campaign and loaded prefab metadata.
    pub fn build(campaign: &'a Campaign, prefabs: &PrefabRegistry) -> Result<Self, PlanError> {
        Self::build_with(campaign, prefabs, Perturb::none())
    }

    /// [`Plan::build`] with a deliberate defect built into the blockout
    /// derivation — see [`crate::compiler::blockout::Perturb`] for why this exists.
    ///
    /// The only caller outside a test is [`Plan::build`] itself, passing
    /// [`Perturb::none`](crate::compiler::blockout::Perturb::none) as a literal; a test
    /// asserts that, so this cannot quietly acquire another.
    pub fn build_with(
        campaign: &'a Campaign,
        prefabs: &PrefabRegistry,
        perturb: Perturb,
    ) -> Result<Self, PlanError> {
        Self::build_linked(campaign, prefabs, perturb, LinkTakes::default(), true)
    }

    /// **This plan with the route proof's link decisions in its path**
    /// (spec-0083 §3.4): the same campaign, prefabs and perturbation, rebuilt
    /// with each taken link spliced in front of the leg it carries and each
    /// performed link marked. Every field derived from the path — the step
    /// indices, the region-write firings, the ancestor relation, the
    /// checkpoints, the transport markers — is derived again from the one path,
    /// so no consumer can read a step index that means something else.
    ///
    /// Called by the build once the world exists, because whether a walk fails
    /// is a question about blocks; `takes` is what [`crate::compiler::nav::take_links`]
    /// decided over them.
    pub fn relinked(&self, prefabs: &PrefabRegistry, takes: LinkTakes) -> Result<Self, PlanError> {
        Ok(
            Self::build_linked(self.campaign, prefabs, self.perturb.clone(), takes, false)?
                .with_design_files(self.design_files.clone()),
        )
    }

    fn build_linked(
        campaign: &'a Campaign,
        prefabs: &PrefabRegistry,
        perturb: Perturb,
        link_takes: LinkTakes,
        announce: bool,
    ) -> Result<Self, PlanError> {
        let namespace = campaign.world.campaign_id.as_str().to_string();
        let seed = campaign.world.content.seed;

        // ---- placements + anchors ----
        let mut areas = Vec::new();
        // Advisory placement findings, in area order (`DW0498`, `crate::compiler::pool`).
        let mut warnings: Vec<Diagnostic> = Vec::new();
        let mut anchors = AnchorTable::default();
        // v0.6 (spec-0011): the absolute dispenser socket cell for each `anchor/trap`
        // marker that declares one, keyed like `anchors`. Empty for a campaign with no
        // trap hardware.
        let mut dispenser_cells: BTreeMap<(String, String), [i32; 3]> = BTreeMap::new();
        // Per-batch affected AABBs from L2 massing (spec-0017), for the
        // editor's per-batch snapshots. Empty without massing verbs.
        let mut massing_bounds: BTreeMap<String, ([i32; 3], [i32; 3])> = BTreeMap::new();
        // Socket doorways severed by `rewire-socket sealed`, per area — the
        // DW0306 connectivity graph must not count those edges.
        let mut severed: BTreeMap<String, BTreeSet<[i32; 3]>> = BTreeMap::new();
        for (i, area) in campaign.world.content.areas.iter().enumerate() {
            let area_id = area.id.as_str().to_string();
            // Origin Y is the horizon's datum, and on a base whose datum is a
            // walk plane it is DERIVED from this area's own piece set
            // (spec-0060 §3.2) — so it is computed per area rather than once
            // per world.
            let origin = [
                i as i32 * AREA_SPACING,
                area_base_y(campaign, area, prefabs)?,
                0,
            ];

            let placement = if let Some(prefab) = &area.prefab {
                // Single-prefab area (the M1 degenerate assembly): one piece at
                // the origin, rotation none — and every connector it declares is
                // unmated by construction, because there is no second piece for
                // one to mate with.
                let prefab_id = prefab.as_str().to_string();
                let meta = prefabs.get(&prefab_id).ok_or_else(|| {
                    PlanError::new(
                        DW_BUILD,
                        format!(
                            "area `{area_id}` binds prefab `{prefab_id}` but no matching prefab \
                             metadata was found in the prefabs dir — bind a prefab that exists in \
                             the prefab library, or add `{prefab_id}` (`.nbt` + metadata) to it. \
                             This is a prefab-library/naming issue, not a quest-logic one"
                        ),
                    )
                })?;
                if crate::compiler::massing::targets_area(campaign, &area_id) {
                    return Err(PlanError::new(
                        crate::compiler::massing::DW_MASSING,
                        format!(
                            "world-edits massing verbs target area `{area_id}`, which binds a \
                             single `prefab` — there is no jigsaw layout to mass. Massing \
                             applies only to `prefab_pool` areas; use the L3 detailing verbs \
                             (carve/fill/fragment/…) on a single-prefab area instead"
                        ),
                    ));
                }
                for (name, am) in &meta.anchors {
                    anchors.declare(&area_id, name, am, || {
                        resolve_anchor(origin, meta, name, am)
                    })?;
                    if let Some(dp) = am.dispenser {
                        dispenser_cells.insert(
                            (area_id.clone(), name.clone()),
                            [origin[0] + dp[0], origin[1] + dp[1], origin[2] + dp[2]],
                        );
                    }
                }
                // Seal this piece's sockets through the SAME mechanism the pool
                // path uses (`solver::seal_layout`), rather than skipping it.
                //
                // A connector is a hole the prefab deliberately leaves in its own
                // wall — a 3×3 doorway plus the `minecraft:jigsaw` marker at its
                // sill — and the solver's invariant is "every unmated socket is
                // sealed with wall material; every mated socket's jigsaw block is
                // cleared to air". That invariant is a property of a PLACED PIECE,
                // not of having run the layout solver, and binding it to the
                // solver left the one-piece case with no surface: a single-prefab
                // area shipped the doorway wide open onto whatever the horizon
                // says lies outside the piece, with the authoring marker still
                // standing in it as a real block a player can stand on.
                //
                // Nothing caught it, because both symptoms hide behind content.
                // `DW0322` reads the exposed doorway as a walkable cell one step
                // from a void drop — but only once the doorway is REACHABLE, and
                // a flooded cell is impassable, so any standing water between the
                // piece's anchors and its own sill deletes the finding while
                // leaving the hole. Correcting such a flood is what exposes it,
                // which is the worst possible time to first meet it. And the
                // stray jigsaw never surfaces in the datapack at all: `snapshot`
                // colours it the magenta fallback precisely as an alarm, on the
                // stated premise that "the solver strips them" — true of every
                // piece a solver had placed, and of no other.
                //
                // Byte impact is confined to what was broken: a prefab with no
                // connectors yields no seals, so every campaign and fixture that
                // binds one is byte-identical.
                let (bbox_min, bbox_max) = Rotation::None.bbox(origin, meta.size());
                let seals = solver::seal_layout(
                    prefabs,
                    &[solver::PlacedPiece {
                        prefab_id: prefab_id.clone(),
                        pos: origin,
                        rotation: Rotation::None,
                        bbox_min,
                        bbox_max,
                        mated: vec![false; meta.connectors.len()],
                    }],
                );
                AreaPlacement {
                    area_id: area_id.clone(),
                    pieces: vec![PiecePlacement {
                        prefab_id,
                        templates: placed_templates(meta, origin, Rotation::None),
                        pos: origin,
                        size: meta.size(),
                        rotation: Rotation::None,
                        // A lone piece mates with nothing: every socket it has is
                        // sealed, which is what `seals` above was just built from.
                        mated: vec![false; meta.connectors.len()],
                    }],
                    seals,
                    mass: Vec::new(),
                }
            } else if let Some(pool) = &area.prefab_pool {
                // Pool area (ADR-0004 jigsaw assembly): the solver grows a layout
                // from the campaign seed and we transform each piece's anchors to
                // world space. `pieces` bounds are guaranteed present by validation
                // (a pool binds `pieces`); default defensively.
                let pool_id = pool.as_str().to_string();
                let (pmin, pmax) = area.pieces.map(|p| (p.min, p.max)).unwrap_or((1, 1));
                let required = required_anchors_for_area(campaign, &area_id);
                let mut stream = Splitmix64::new(solver::stream_seed(seed, &area_id));
                let mut layout = solver::solve_area(
                    prefabs,
                    &pool_id,
                    &required,
                    pmin,
                    pmax,
                    origin,
                    &mut stream,
                )
                .map_err(|e| {
                    // A solver failure raised after growth (`DW0305`) carries the
                    // draw that produced it: attach the pool-level `DW0498` so the
                    // author reads the cause at the declaration, not just the
                    // symptom at the use site.
                    let mut w = warnings.clone();
                    w.extend(crate::compiler::pool::check(
                        prefabs,
                        &crate::compiler::pool::PoolArea {
                            area_id: &area_id,
                            area_index: i,
                            pool_id: &pool_id,
                            pieces_min: pmin,
                            pieces_max: pmax,
                        },
                        e.placed.iter().map(String::as_str),
                    ));
                    PlanError::new(e.failure.code, e.failure.message).with_warnings(w)
                })?;
                // Stage-7 L2 massing (spec-0017): apply the edit script's
                // massing batches for this area over the solved layout, so
                // everything downstream — anchor resolution just below, the
                // gate/waterline checks, assembly, relight, nav, the L3
                // detailing replay — sees the massaged layout. No-op (layout
                // and seals byte-identical) for a campaign without massing
                // verbs targeting this area.
                let massing_out =
                    crate::compiler::massing::apply(campaign, &area_id, &mut layout, prefabs, seed)
                        .map_err(|e| PlanError::new(e.code, e.message))?;
                massing_bounds.extend(massing_out.bounds);
                if !massing_out.severed.is_empty() {
                    severed.insert(area_id.clone(), massing_out.severed);
                }

                // `DW0498`: the draw is settled — read it back and say
                // so ONCE, here at the declaration, if it seats the same
                // anchor-bearing prefab more than once. Every anchor that prefab
                // declares now has more than one carrier; the `or_insert_with`
                // resolution just below silently keeps the first, and the solver's
                // `DW0305` will fail the build at whichever campaign-referenced
                // anchor happens to be the first use site. Advisory: a repeat with
                // no such use is legal, and shipping campaigns rely on it. Read
                // AFTER massing so the reported draw is the one the player gets.
                warnings.extend(crate::compiler::pool::check(
                    prefabs,
                    &crate::compiler::pool::PoolArea {
                        area_id: &area_id,
                        area_index: i,
                        pool_id: &pool_id,
                        pieces_min: pmin,
                        pieces_max: pmax,
                    },
                    layout.pieces.iter().map(|p| p.prefab_id.as_str()),
                ));

                let mut pieces = Vec::new();
                for placed in &layout.pieces {
                    let meta = prefabs.get(&placed.prefab_id).ok_or_else(|| {
                        PlanError::new(
                            DW_BUILD,
                            format!(
                                "internal invariant violation: the solver placed prefab `{}`, \
                                 which has no metadata entry — the solver and metadata registry \
                                 disagree. This is a compiler bug, not a campaign error; stop and \
                                 escalate",
                                placed.prefab_id
                            ),
                        )
                    })?;
                    // Transform this piece's anchors to world space. Each required
                    // anchor is carried by exactly one placed piece (fillers are
                    // anchorless connectors), so names do not collide.
                    for (name, am) in &meta.anchors {
                        anchors.declare(&area_id, name, am, || {
                            resolve_piece_anchor(placed, meta, name, am)
                        })?;
                        if let Some(dp) = am.dispenser {
                            dispenser_cells
                                .entry((area_id.clone(), name.clone()))
                                .or_insert_with(|| solver::transform_point(placed, dp));
                        }
                    }
                    pieces.push(PiecePlacement {
                        prefab_id: placed.prefab_id.clone(),
                        templates: placed_templates(meta, placed.pos, placed.rotation),
                        pos: placed.pos,
                        size: meta.size(),
                        rotation: placed.rotation,
                        mated: placed.mated.clone(),
                    });
                }
                AreaPlacement {
                    area_id: area_id.clone(),
                    pieces,
                    seals: layout.seals,
                    mass: Vec::new(),
                }
            } else {
                // Validation (DW0160) guarantees exactly one binding.
                return Err(PlanError::new(
                    DW_BUILD,
                    format!(
                        "internal invariant violation: area `{area_id}` binds neither `prefab` \
                         nor `prefab_pool` at build time — `DW0160` should have rejected this \
                         during validation. This is a compiler bug; stop and escalate"
                    ),
                ));
            };
            areas.push(placement);
        }

        // ---- the derived blockout (spec-0049 §5) ----
        //
        // **This is the ordering tooth, and it is here because here is the only
        // door.** The blockout has no authored form — no document, no schema, no
        // file — so the single path from a site plan to blockout bytes runs
        // through `crate::compiler::blockout::derive`, and the single path to that runs
        // through this function. Every `delvec` verb that reaches a world (build,
        // analyze, snapshot, viewer, blocking-chart, edit) reaches it through a
        // `Plan`, and there is no other constructor. Someone doing the guarded
        // thing without calling this would have to have built a `Plan` some other
        // way, and there is none.
        //
        // A campaign with no site plan gets `None` and nothing below runs, so its
        // output does not move by a byte.
        let mut blockout_reads = delvewright_dsl::metrics::Reads::new();
        let blockout = match crate::compiler::blockout::derive_with(
            campaign,
            &mut blockout_reads,
            perturb.clone(),
        ) {
            None => None,
            Some((mut placement, derived)) => {
                // The derivation is a producer of anchors exactly as a prefab
                // is, so it says what each one is FOR (spec-0046): the entry
                // node's anchor arrives carrying `AnchorRole::Entry`, and the
                // spelling `siteplan::ENTRY_ANCHOR` gives it stops being what
                // resolves it.
                for (name, resolved, role) in derived.anchors() {
                    anchors.place(&placement.area_id, name, resolved, role)?;
                }
                // ---- the detail plan's pieces (spec-0050 §1) ----
                //
                // **The second tooth, and it is in the same door as the first.**
                // A `details[]` row carries no coordinate, no extent and no
                // offset; where its piece goes is `Frame::of` over the plan's
                // own resolved box, computed here. There is no flag and no
                // second entry point, so a part that wanted a different box
                // would have to have built a `Plan` some other way, and there is
                // none.
                //
                // The anchors go in AFTER the derived ones on purpose: the
                // derivation names `anchor/node-…` at the massing's own footing,
                // and where a piece stands there the piece's anchor is the
                // truth. Overwriting is what keeps the campaign's stage-3
                // vocabulary working without a quest edit.
                let detailing = crate::compiler::detail::place(campaign, prefabs);
                placement.pieces.extend(detailing.pieces);
                for (name, pos, facing) in detailing.anchors {
                    anchors.place(
                        &placement.area_id,
                        &name,
                        ResolvedAnchor::Point { pos, facing },
                        None,
                    )?;
                }
                for (name, from, to, block) in detailing.gates {
                    anchors.place(
                        &placement.area_id,
                        &name,
                        ResolvedAnchor::Gate { from, to, block },
                        None,
                    )?;
                }
                areas.push(placement);
                Some(derived)
            }
        };

        // ---- gate-aware reachability (M2 fix 7, DW0306) ----
        // With the layout solved, verify no objective's anchor is sealed behind a
        // gate that only a later objective opens (an unwinnable deadlock the anchor
        // resolver alone cannot see).
        for area in &areas {
            check_gate_reachability(
                campaign,
                &area.area_id,
                anchors.entry_anchor_name(&area.area_id).as_deref(),
                &area.pieces,
                prefabs,
                severed.get(&area.area_id),
            )?;
        }

        // ---- ocean waterline invariant (DW0344) ----
        //
        // Bound here and nowhere else, on the same reasoning as the mating check
        // below: every campaign build goes through `Plan::build`. The binding
        // count comes back with the verdict and is PRINTED by the build, because
        // a check keyed off an optional metadata field goes quiet rather than red
        // when the field disappears. What stops that quiet from reading as a pass
        // is no longer a report about the emptiness: it is that a piece standing
        // in the sea without the declaration is refused outright, so the only
        // world that can reach a zero binding is one whose pieces all stand clear
        // of the water — a discharge taken from geometry, which the missing
        // declaration cannot fake.
        let waterline = check_ocean_waterline(campaign, &areas, prefabs);

        // ---- the pieces fit together (DW0780/DW0781, ADR-0020) ----
        //
        // Bound here and nowhere else: every campaign build goes through
        // `Plan::build`, so a world whose pieces contradict each other at the
        // faces they share cannot be compiled, packaged or shipped. There is no
        // flag and no separate command to remember.
        let binding = crate::compiler::faces::check(&areas, prefabs).map_err(|e| {
            let mut w = warnings.clone();
            w.extend(e.warnings.clone());
            PlanError::new(e.failure.code, e.failure.message).with_warnings(w)
        })?;
        let placed = crate::compiler::faces::placed_pieces(&areas);
        // Printed on every plan, found anything or not: the count this check
        // owes its reader is a fraction of the PLACEMENT, and stating it only
        // when it was zero is what let a build read `0 with a spatial contract`
        // as an unremarkable advisory line rather than as a check that examined
        // nothing.
        // A relinked plan is the same placement measured a second time; its
        // binding line was printed by the first build and is not printed twice.
        if announce {
            eprintln!("{}", binding.line(placed));
        }
        if let Some(finding) = binding.finding(placed, campaign.site_plan.is_some()) {
            warnings.push(finding);
        }

        // ---- classes ----
        let classes = campaign
            .classes
            .content
            .classes
            .iter()
            .enumerate()
            .map(|(i, c)| ClassPlan {
                n: i as i32 + 1,
                class_id: c.id.as_str().to_string(),
                safe: safe_local(c.id.as_str()),
            })
            .collect();

        // ---- npc dialogue numbering ----
        // Dialogue lives in stage 6 (1:1 with stage-2 NPCs, guaranteed by
        // validation, which `build` implies). An NPC without a tree is skipped
        // defensively.
        let npcs = campaign
            .npcs
            .content
            .npcs
            .iter()
            .filter_map(|npc| {
                campaign
                    .dialogue
                    .content
                    .tree_for(npc.id.as_str())
                    .map(|tree| plan_npc(npc, tree))
            })
            .collect::<Vec<_>>();

        // ---- links and gathers (spec-0083) ----
        //
        // Before the path, because the path records which links are live at
        // each of its steps and splices the ones the route proof took. The
        // static half of `DW0932` is judged here, where every link is known
        // and no block is needed.
        let (links, gathers) = crate::compiler::link::collect(campaign, &anchors);
        for l in &links {
            if let Some(message) = crate::compiler::link::static_fault(l) {
                return Err(PlanError::new(DW_TELEPORT_LINK, message).with_warnings(warnings));
            }
        }

        // ---- critical path + inter-area transport ----
        let flow = crate::compiler::flow::Flow::new(campaign);
        let start = resolve_campaign_start(&areas, &anchors);
        let cp = build_critical_path(
            campaign,
            &anchors,
            &npcs,
            &flow,
            &flow.playthrough(),
            start.as_ref(),
            PathLinks {
                links: &links,
                takes: &link_takes,
            },
        )?;

        // ---- v0.6 checkpoints + stealth beats (spec-0012 / spec-0014) ----
        let (checkpoints, stealth_beats) =
            collect_v06_effects(campaign, &anchors, &cp.firing.obj_step);
        let path_firing = cp.firing;
        let objective_steps = path_firing.obj_step.clone();
        let trigger_steps = path_firing.trigger_step.clone();
        let loop_spliced = cp.loops;

        // ---- v0.6 traps (spec-0011) ----
        let traps = collect_traps(campaign, &anchors, &dispenser_cells);

        // ---- shortcut doors (spec-0016 §2) ----
        let shortcuts = collect_shortcuts(campaign, &anchors);

        // ---- container fills (spec-0021) ----
        let loot = collect_loot(campaign, &anchors);

        // ---- lethal volumes (spec-0031) ----
        let lethal_volumes = collect_lethal_volumes(campaign, &anchors);

        // ---- furniture (spec-0065) ----
        let furniture = collect_furniture(&areas, prefabs);

        // ---- `collect` container adoption (DSL v0.8) ----
        let collect_fills = collect_collect_fills(campaign, &anchors);

        // ---- ambushes (spec-0016 §3) ----
        let ambushes = collect_ambushes(campaign, &anchors);

        // ---- timed gates (spec-0016 §4) ----
        let timed_gates: Vec<TimedGatePlan> = campaign
            .quests
            .content
            .timed_gates
            .iter()
            .filter_map(|g| {
                let (from, to, block) = gate_region_block_any(&anchors, g.gate.as_str())?;
                Some(TimedGatePlan {
                    id: g.id.as_str().to_string(),
                    safe: safe_local(g.id.as_str()),
                    gate_anchor: g.gate.as_str().to_string(),
                    gate_region: (from, to),
                    gate_block: block,
                    open_ticks: g.open_ticks,
                    closed_ticks: g.closed_ticks,
                    phase: g.phase,
                    crush: g.crush,
                    disarm: g.disarm.as_ref().and_then(|dis| {
                        point_any(&anchors, dis.via.as_str()).map(|via_cell| TimedGateDisarmPlan {
                            via_anchor: dis.via.as_str().to_string(),
                            via_cell,
                            sets_flag: dis.sets_flag.as_str().to_string(),
                        })
                    }),
                })
            })
            .collect();

        // ---- v0.8 seal hints: what a sealed gate answers ----
        let seal_hints = collect_seal_hints(campaign, &anchors);

        // ---- the press answers (DSL v0.11): what every sealed body answers ----
        // Collected AFTER both `shortcuts` and `seal_hints`, because it is derived
        // from the union of the two — one rule over the pressable class, not one
        // rule per verb.
        let press_answers = collect_press_answers(campaign, &seal_hints, &shortcuts);

        // ---- the contingent ways the placed world stages (spec-0042 §2.4) ----
        //
        // Before the region-write model, because an `open-way`'s geometry IS a
        // staged way: the campaign names a piece and a way, and everything else
        // about the write comes from the piece. Sealed here and nowhere else —
        // every campaign build goes through `Plan::build`, so a way that reaches
        // the world with no cells to open cannot be compiled, packaged or
        // shipped, and there is no flag and no separate command to remember.
        let ways = crate::compiler::ways::stage(&areas, prefabs);
        ways.seal().map_err(|e| e.with_warnings(warnings.clone()))?;

        // ---- v0.6 gate open/close firings (drives the close-gate nav proof) ----
        let mut region_events = collect_region_events(campaign, &anchors, &path_firing, &ways);
        // A shortcut gate is sealed from world-load and is opened only by an
        // OPTIONAL far-side interaction no proof can order (spec-0016 §2). Seal it
        // for the whole completability model — `fire_step: 0` precedes every leg —
        // so the critical path, the checkpoints and the traps are all proven over
        // a world where no shortcut has been taken. The delve must be finishable
        // the long way; the shortcut is a reward, never a requirement.
        // FORCED, and the word is exact: what is unforced about a shortcut is the
        // player OPENING it, and that firing is registered separately (and dropped,
        // being an unseal from an optional root). The door standing shut is a fact
        // about the world at load — nobody has to do anything for it to be true — so
        // its footing is footing the party really has.
        region_events.extend(shortcuts.iter().map(|sc| {
            RegionEvent::forced(sc.gate_region, RegionWrite::of_block(&sc.gate_block), 0)
        }));
        // spec-0086 §5.1–§5.2: every loop's slab as a gated seal, and the writes
        // each exercise step's crossings perform, credited as forced at it.
        region_events.extend(loop_spliced.events.iter().cloned());
        let strict_ancestor_steps = compute_strict_ancestor_steps(
            campaign,
            &objective_steps,
            &trigger_steps,
            cp.steps.len(),
        );
        // ---- loops (spec-0086) ----
        let loops = crate::compiler::r#loop::resolve(campaign, &anchors);
        // `DW0950`: a loop the forced route never meets while it holds.
        for (li, l) in loops.iter().enumerate() {
            if loop_spliced.exercises.iter().any(|e| e.r#loop == l.id) {
                continue;
            }
            let shut = loop_spliced
                .holds
                .get(li)
                .and_then(|h| h.iter().position(|v| *v == Some(false)));
            let index = campaign
                .quests
                .content
                .loops
                .iter()
                .position(|d| d.id.as_str() == l.id)
                .unwrap_or(0);
            warnings.push(Diagnostic::warning(
                crate::compiler::r#loop::DW_LOOP_UNMET,
                "quests",
                format!("/content/loops/{index}"),
                match shut {
                    Some(k) => format!(
                        "loop `{}` is never met by the forced route while it holds — its gate is \
                         first read shut at critical-path step {k}, and no leg before that crosses \
                         its slab. Nobody is made to walk it; that is a design when the corridor \
                         is endless only until a thing is found elsewhere, and a mechanism nobody \
                         experiences when it is not",
                        l.id
                    ),
                    None => format!(
                        "loop `{}` is never met by the forced route: no leg of the critical path \
                         crosses its slab, and its gate is never read shut on it. Nobody is made \
                         to walk it",
                        l.id
                    ),
                },
            ));
        }
        let region_events = with_loop_exercises(
            region_events_of(campaign, region_events, &path_firing, &npcs),
            &loop_spliced,
        );

        // ---- what became of every staged way (spec-0042 §2.5, DW0548) ----
        //
        // Last, because it needs everything above it: the staged ways, the
        // openings' DAG points, the resolved anchors and the strict-ancestor
        // relation the region-write model orders the world by. The ancestry
        // predicate is `Plan::gate_fired_before`'s body, handed over rather than
        // re-derived — a second reading of "has this fired yet" is a second
        // instrument, and the whole point of the verdict is that it agrees with
        // the route proof.
        //
        // **Run on either side of the pair, never on the intersection.** A world
        // that stages a way owes a disposition for it; a campaign that writes an
        // `open-way` owes a resolvable reference — and the case where a campaign
        // opens a way NO placed piece stages is exactly the one a guard on the
        // staged ways alone would skip in silence, which is how an effect comes to
        // emit nothing and be reported by nobody (the class `DW0360` exists for).
        let mut way_gate = None;
        let openings = collect_way_openings(campaign, &path_firing);
        if !ways.ways.is_empty() || !openings.is_empty() {
            let elements = collect_required_elements(campaign, &anchors, &objective_steps);
            let precedes = |g: usize, s: usize| {
                g == 0
                    || strict_ancestor_steps
                        .get(&s)
                        .is_some_and(|anc| anc.contains(&g))
            };
            let gate = crate::compiler::ways::judge(
                &ways, &openings, &elements, &areas, prefabs, &precedes,
            )
            .map_err(|e| e.with_warnings(warnings.clone()))?;
            if let Some(finding) = crate::compiler::ways::unbound_finding(&gate) {
                warnings.push(finding);
            }
            // The artifact belongs to a world that stages a way. A ledger of zero
            // ways is not a measurement of anything — and a campaign that reaches
            // here with none has already been refused above.
            if !ways.ways.is_empty() {
                way_gate = Some(gate);
            }
        }

        // ---- the horizon's surround (spec-0026) ----
        //
        // After the blockout, and that order is the whole point: the surround
        // rings the map the derivation just produced, not a footprint that
        // predates it. `surround_rect` states which authority fixed the
        // rectangle, and for a site-plan campaign that authority is the plan's
        // own region — so the landform is fixed the moment the whole is stated
        // and cannot be moved later by detailing a part.
        //
        // A base with no surround gets `None` and nothing below runs, so a
        // `void` or `ocean` build does not move by a byte.
        let surround = build_surround(campaign, seed, &areas)?;

        // ---- the assemblies' rigs (spec-0082) ----
        let mut rigs: BTreeMap<String, delvewright_dsl::rig::Rig> = BTreeMap::new();
        for a in &campaign.quests.content.assemblies {
            if let delvewright_dsl::rig::RigLookup::Found(r) =
                delvewright_dsl::AnchorRegistry::rig(prefabs, &a.rig)
            {
                rigs.insert(a.rig.as_str().to_string(), r.clone());
            }
        }

        Ok(Self {
            campaign,
            namespace,
            seed,
            areas,
            warnings,
            anchors,
            classes,
            npcs,
            critical_path: cp.steps,
            transport: cp.transport,
            critical_path_transport: cp.transport_by_step,
            loops,
            loop_exercises: loop_spliced.exercises,
            critical_path_sneak: cp.sneak_by_step,
            critical_path_cutscene: cp.cutscene_by_step,
            critical_path_live_links: cp.live_links_by_step,
            links,
            gathers,
            link_takes,
            perturb,
            checkpoints,
            lethal_volumes,
            furniture,
            stealth_beats,
            objective_steps,
            path_firing,
            traps,
            rigs,
            shortcuts,
            loot,
            collect_fills,
            ambushes,
            timed_gates,
            seal_hints,
            press_answers,
            region_events,
            ways,
            way_gate,
            strict_ancestor_steps,
            massing_bounds,
            blockout,
            surround,
            design_files: crate::compiler::design::DesignFiles::default(),
            waterline,
            face_binding: binding,
        })
    }

    /// Attach the approved reference images the campaign directory holds
    /// (spec-0061), which the plan cannot read for itself: it is built from
    /// parsed documents and never knows where they came from.
    ///
    /// Every `delvec` verb that reads a campaign directory calls this, because
    /// the loader is the one thing that knows the path and it hands the list on
    /// beside the stage documents.
    #[must_use]
    pub fn with_design_files(mut self, files: crate::compiler::design::DesignFiles) -> Self {
        self.design_files = files;
        self
    }
}
