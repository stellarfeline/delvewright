//! Resolve a validated [`Campaign`] into a placement + naming model that
//! emission and `critical-path.json` both consume.
//!
//! ## Coordinate scheme (deterministic)
//!
//! Each stage-1 area is placed at origin `[index * AREA_SPACING, base_y, 0]`
//! (M1 has one area → `[0, 64, 0]`). The origin Y is fixed per **horizon**
//! (spec-0013): `void` → [`BASE_Y`] (64), `ocean` → [`OCEAN_BASE_Y`] (60), which is
//! `sea_level - island waterline` so authored island water meets the world ocean.
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

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use delvewright_dsl::{
    Campaign, DialogueEffect, DialogueId, Lethality, Npc, NpcDialogue, Objective, Quest,
    QuestEffect, TrapReset, TrapTrigger, Trigger,
};

use crate::flow::objectives_in_order;
use crate::registry::{AnchorMeta, PrefabRegistry};
use crate::solver::{self, Facing, Rotation, SealFill, Splitmix64};

/// World-space distance between successive area origins.
pub const AREA_SPACING: i32 = 256;
/// The Y of every area origin under `horizon: void` (structures carry their own
/// floor at local y=0). Also the fallback Y for an unresolvable position.
pub const BASE_Y: i32 = 64;
/// Sea level of the `ocean` horizon superflat (spec-0013): the pinned
/// bedrock/stone/water layer stack (1 + 118 + 8 from the -64 build floor) tops the
/// water at y=62. Emission pins the same stack in `generator-settings`.
pub const SEA_LEVEL: i32 = 62;
/// The island tileset's authored waterline (`prefabs/island-tileset.md`): every
/// island piece puts its top water block at **local y=2**, with the walkable land
/// plane one block above it at local y=3.
///
/// Assumption (documented in `docs/reference/compiler.md`): the tileset convention
/// is a *library* constant, not a per-piece one — prefab metadata may *declare* its
/// waterline (`waterline_y`), which [`check_ocean_waterline`] then verifies against
/// sea level, but placement itself uses this single convention height so that every
/// area of an ocean world sits on one deterministic datum.
pub const ISLAND_WATERLINE_Y: i32 = 2;
/// The Y of every area origin under `horizon: ocean`: the piece base sits at
/// `SEA_LEVEL - ISLAND_WATERLINE_Y` (= 60) so the authored waterline (local y=2)
/// meets the world ocean (y=62) and the walk plane (local y=3) is the vanilla-normal
/// one block above the sea. Placing ocean areas at [`BASE_Y`] instead floats the
/// island ~4 blocks above the sea: a player who falls into open water cannot climb
/// ashore.
pub const OCEAN_BASE_Y: i32 = SEA_LEVEL - ISLAND_WATERLINE_Y;

/// The area-origin Y for a campaign's horizon (spec-0013). `void` (default/absent)
/// keeps [`BASE_Y`], so every pre-0.6 / void campaign stays byte-identical; `ocean`
/// uses [`OCEAN_BASE_Y`] so the island waterline convention holds.
pub fn base_y(campaign: &Campaign) -> i32 {
    match campaign.world.content.horizon {
        Some(delvewright_dsl::Horizon::Ocean) => OCEAN_BASE_Y,
        _ => BASE_Y,
    }
}

/// A resolved `set-checkpoint` effect (DSL v0.6, spec-0012), collected in
/// deterministic content order so its `index` is a stable, byte-identical id used
/// both for the active-checkpoint marker (`#cp dw.sys`) and its `on_respawn`
/// dispatch function.
#[derive(Clone, Debug)]
pub struct CheckpointPlan {
    /// Stable content-ordered id (0-based).
    pub index: usize,
    /// The checkpoint anchor name.
    pub anchor: String,
    /// The resolved absolute anchor cell.
    pub pos: [i32; 3],
    /// Per-player `on_respawn` effects (may be empty).
    pub on_respawn: Vec<QuestEffect>,
    /// `critical_path` step index at which this checkpoint fires (roots DW0315).
    pub fire_step: usize,
}

/// A resolved `begin-stealth` beat (DSL v0.6, spec-0014), collected in
/// deterministic content order; its `index` (1-based) is the active-session id
/// written to `#stealth dw.sys` (0 = inactive).
#[derive(Clone, Debug)]
pub struct StealthBeat {
    /// Stable content-ordered session id (1-based).
    pub index: usize,
    /// Zones: `(anchor name, resolved centre cell, half-extents)`.
    pub zones: Vec<(String, [i32; 3], [u32; 3])>,
    /// Per-player `on_caught` effects (may be empty).
    pub on_caught: Vec<QuestEffect>,
    /// Ticks of exposure tolerated before `on_caught` fires.
    pub grace_ticks: u32,
    /// `critical_path` step index that activates the beat (roots DW0327).
    pub fire_step: usize,
}

/// A resolved trap (DSL v0.6, spec-0011), collected in deterministic content
/// order. Carries everything the nav proof (`DW0342`), the payload/disarm
/// emission, and the PackTest need.
#[derive(Clone, Debug)]
pub struct TrapPlan {
    /// The raw trap id (`trap/<name>`).
    pub id: String,
    /// Sanitized local name (`dart_hall`) for emitted function/tag names.
    pub safe: String,
    /// The declared trigger kind (informs the hazard model + PackTest).
    pub trigger: TrapTrigger,
    /// The resolved absolute trigger/hazard cell (the trap's `at` anchor cell).
    pub trigger_cell: [i32; 3],
    /// The resolved absolute dispenser socket cell (from the `at` anchor's
    /// `dispenser` metadata), or `None` if the prefab exposes none.
    pub dispenser: Option<[i32; 3]>,
    /// The dispense payload `(item, count)` this trap loads, if any.
    pub payload: Option<(String, u32)>,
    /// How dangerous the trap is.
    pub lethality: Lethality,
    /// Whether the trap re-arms after firing.
    pub reset: TrapReset,
    /// The resolved disarm affordance, if declared.
    pub disarm: Option<TrapDisarmPlan>,
    /// Flags that gate the trap being active.
    pub requires_flags: Vec<String>,
    /// Flags whose being set deactivates the trap (DSL v0.6 negative gate).
    pub forbids_flags: Vec<String>,
}

/// A gate open/close firing (DSL v0.6), collected in deterministic content order.
/// `close-gate` seals the gate region (fills it with the anchor's declared block)
/// from its firing beat; `open-gate` clears it back to air. The occupancy model
/// (`crate::assembled`) otherwise treats every gate cell as always passable — the
/// conservative "assume the gate the player needs is opened" stance DW0306 checks.
/// `close-gate` is the physical dual: the critical-path / checkpoint reachability
/// proofs treat the region as **solid** on any walked leg reached *after* the
/// latest firing at or before it is a close (and not reopened by a later
/// `open-gate`), so a path that must cross a sealed gate fails `DW0311`/`DW0315`.
#[derive(Clone, Debug)]
pub struct GateEvent {
    /// The gate region's inclusive corners (absolute world coords).
    pub region: ([i32; 3], [i32; 3]),
    /// `true` for `close-gate` (seals the region), `false` for `open-gate` (clears).
    pub closes: bool,
    /// The `critical_path` step index at which this firing happens.
    pub fire_step: usize,
}

/// A resolved trap disarm affordance (DSL v0.6, spec-0011).
#[derive(Clone, Debug)]
pub struct TrapDisarmPlan {
    /// The disarm anchor name (`anchor/…`).
    pub via_anchor: String,
    /// The resolved absolute cell of the disarm affordance.
    pub via_cell: [i32; 3],
    /// The flag the disarm sets.
    pub sets_flag: String,
}

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
    /// Resolved absolute anchors, keyed by `(area_id, anchor_name)`.
    pub anchors: BTreeMap<(String, String), ResolvedAnchor>,
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
    /// Per-step stealth hint (DSL v0.4), aligned 1:1 with `critical_path`: `true`
    /// when the step's objective is `stealth`-marked → emitted as `sneak: true`.
    pub critical_path_sneak: Vec<bool>,
    /// Per-step cutscene duration (DSL v0.4), aligned 1:1 with `critical_path`:
    /// `Some(seconds)` when completing that step's objective triggers a
    /// `QuestEffect::Cutscene` → emitted as `cutscene_seconds`.
    pub critical_path_cutscene: Vec<Option<u32>>,
    /// Resolved `set-checkpoint` effects (DSL v0.6, spec-0012), content-ordered.
    pub checkpoints: Vec<CheckpointPlan>,
    /// Resolved `begin-stealth` beats (DSL v0.6, spec-0014), content-ordered.
    pub stealth_beats: Vec<StealthBeat>,
    /// Objective id → its `critical_path` step index. The inverse of a step's
    /// serving objective — used by the visual-tier POV shot planner
    /// (`crate::render_plan`) to name the objective each player-POV leg walks
    /// toward, and by the v0.6 checkpoint / stealth proofs to root a beat.
    pub objective_steps: BTreeMap<String, usize>,
    /// Resolved traps (DSL v0.6, spec-0011), content-ordered.
    pub traps: Vec<TrapPlan>,
    /// Resolved gate open/close firings (DSL v0.6), content-ordered — drives the
    /// `close-gate` completability model in `crate::nav`. Empty when the campaign
    /// uses no gate effects (byte-identical routing to pre-close-gate behavior).
    pub gate_events: Vec<GateEvent>,
    /// Per-batch affected world AABBs from the stage-7 L2 massing verbs
    /// (spec-0017 PR 3), keyed by batch id — the editor's per-batch snapshot
    /// framing for massing batches. Empty for a campaign without massing.
    pub massing_bounds: BTreeMap<String, ([i32; 3], [i32; 3])>,
    /// For each objective's `critical_path` step, the set of steps of its **strict
    /// DAG ancestors** — objectives guaranteed to complete before it in *every* valid
    /// play order (transitive `after` within its quest ∪ every objective of a
    /// transitive `depends_on`-ancestor quest). The `close-gate` seal model
    /// (`crate::nav`) uses this so a gate only seals a leg whose objective is a true
    /// causal descendant of the gate's firing objective — not a parallel branch the
    /// lineariser merely interleaved ahead of it.
    pub strict_ancestor_steps: BTreeMap<usize, BTreeSet<usize>>,
}

/// A placed area: one or more pieces plus their socket seals.
pub struct AreaPlacement {
    /// Area id (`area/…`).
    pub area_id: String,
    /// The placed pieces (single-prefab areas have exactly one; pool areas have
    /// the solver's assembly, entry first).
    pub pieces: Vec<PiecePlacement>,
    /// Socket seal/clear fills for this area (empty for single-prefab areas).
    pub seals: Vec<SealFill>,
}

impl AreaPlacement {
    /// The union world AABB `(min, max)` covering every placed piece. For a
    /// single-prefab area this is exactly `origin .. origin+size-1`.
    pub fn bounds(&self) -> ([i32; 3], [i32; 3]) {
        let mut min = [i32::MAX; 3];
        let mut max = [i32::MIN; 3];
        for piece in &self.pieces {
            let (pmin, pmax) = piece.bbox();
            for a in 0..3 {
                min[a] = min[a].min(pmin[a]);
                max[a] = max[a].max(pmax[a]);
            }
        }
        (min, max)
    }
}

/// One placed structure piece.
pub struct PiecePlacement {
    /// Bound prefab id (`prefab/…`).
    pub prefab_id: String,
    /// Datapack structure id path segment (e.g. `hello-room`).
    pub structure_id: String,
    /// Structure `.nbt` filename (relative to `prefabs/`).
    pub structure_file: String,
    /// World-space `/place template` position `[x, y, z]` (where local `(0,0,0)`
    /// lands).
    pub pos: [i32; 3],
    /// Unrotated prefab size `[sx, sy, sz]` (from prefab metadata).
    pub size: [i32; 3],
    /// Placement rotation (identity for single-prefab areas).
    pub rotation: Rotation,
}

impl PiecePlacement {
    /// The world AABB `(min, max)` of this placed piece, for chunk `forceload`.
    pub fn bbox(&self) -> ([i32; 3], [i32; 3]) {
        self.rotation.bbox(self.pos, self.size)
    }
}

/// A resolved anchor (absolute world coords).
pub enum ResolvedAnchor {
    /// A point with optional facing.
    Point {
        /// Absolute position.
        pos: [i32; 3],
        /// Facing keyword, if any.
        facing: Option<String>,
    },
    /// A gate region of `block`.
    Gate {
        /// Absolute min/max corners.
        from: [i32; 3],
        /// The opposite corner.
        to: [i32; 3],
        /// Filling block id.
        block: String,
    },
}

/// One selectable class.
pub struct ClassPlan {
    /// Trigger value (`/trigger dw.class set <n>`), 1-based.
    pub n: i32,
    /// Class id.
    pub class_id: String,
    /// Sanitized name for the apply function.
    pub safe: String,
}

/// A dialogue plan for one NPC.
pub struct NpcPlan {
    /// NPC id.
    pub npc_id: String,
    /// Sanitized local name (`keeper`).
    pub safe: String,
    /// The trigger objective (`dw.dlg_<npc>`).
    pub trigger_objective: String,
    /// Entity tag on the interaction entity (`dw_npc_<npc>`).
    pub tag: String,
    /// Root dialogue node id.
    pub root: String,
    /// Options in a stable order, each with its trigger value.
    pub options: Vec<OptionPlan>,
}

/// One dialogue option and the `/trigger` value it fires.
pub struct OptionPlan {
    /// Trigger value (`/trigger dw.dlg_<npc> set <n>`), 1-based across the NPC.
    pub n: i32,
    /// The node this option belongs to.
    pub node_id: String,
    /// Button label.
    pub label: String,
    /// Navigation target node, if any.
    pub next: Option<String>,
    /// Objectives this option completes.
    pub completes: Vec<String>,
    /// Flags this option sets when chosen (DSL v0.4 dialogue `set-flag`).
    pub sets_flags: Vec<String>,
    /// Flags that must be set for this option to be shown (DSL v0.4).
    pub requires_flags: Vec<String>,
    /// Flags whose being set HIDES this option (DSL v0.6 negative gate).
    pub forbids_flags: Vec<String>,
    /// World-time cuts this option fires (DSL v0.5 dialogue `set-time`), in order.
    pub sets_time: Vec<delvewright_dsl::WorldTime>,
    /// Weather cuts this option fires (DSL v0.5 dialogue `set-weather`), in order.
    pub sets_weather: Vec<delvewright_dsl::WorldWeather>,
    /// Checkpoints this option sets (DSL v0.6 dialogue `set-checkpoint`), each
    /// `(anchor, on_respawn)`, in order.
    pub sets_checkpoints: Vec<(String, Vec<QuestEffect>)>,
    /// Deferred NPCs this option summons (DSL v0.6 dialogue `spawn-npc`), in order.
    pub spawns_npcs: Vec<String>,
}

/// A critical-path step (mirrors the amended `critical-path.json` shape).
///
/// Every step that stands for a DSL objective carries that objective's id
/// (`objective_id`, exported as the step's `objective` field). It is the step's
/// **proof obligation**: the harness passes the step only when the anchored
/// completion marker for exactly this objective arrives ([`marker_line`]). Without
/// it a step could only be checked positionally — arriving somewhere is not
/// completing anything — which is how a run once passed 22/22 on a path whose
/// campaign had in fact completed at step 12.
pub enum Step {
    /// Select a class by chatting `command`.
    SelectClass {
        /// Class id.
        class_id: String,
        /// The chat command the bot sends.
        command: String,
    },
    /// Talk to an NPC; `command` fires the objective-completing option.
    TalkTo {
        /// The `obj/<id>` this step proves complete.
        objective_id: String,
        /// NPC id.
        npc_id: String,
        /// Absolute NPC position.
        pos: [i32; 3],
        /// The chat command the bot sends.
        command: String,
    },
    /// Walk to within `radius` of `pos`.
    Reach {
        /// The `obj/<id>` this step proves complete.
        objective_id: String,
        /// Anchor id.
        anchor_id: String,
        /// Absolute anchor position.
        pos: [i32; 3],
        /// Completion radius.
        radius: u32,
    },
    /// Slay a wave: goto `pos` (the wave anchor), attack entities tagged `tag`
    /// until the marker channel reports completion (v0.3).
    Kill {
        /// The `obj/<id>` this step proves complete.
        objective_id: String,
        /// Wave id (`wave/…`).
        wave_id: String,
        /// Absolute wave-anchor position.
        pos: [i32; 3],
        /// Entity tag on the wave's mobs (`dw_wave_<wave>`).
        tag: String,
        /// Total mob count.
        count: i32,
    },
    /// Collect `count` of `item` from a chest at `pos` (v0.3).
    Collect {
        /// The `obj/<id>` this step proves complete.
        objective_id: String,
        /// Vanilla item id.
        item: String,
        /// Required count.
        count: i32,
        /// Absolute chest-anchor position.
        pos: [i32; 3],
    },
    /// Interact at `pos`: goto, then chat `command` (the same `/trigger` the
    /// interaction advancement fires). `requires_item` gates completion (v0.3).
    Interact {
        /// The `obj/<id>` this step proves complete.
        objective_id: String,
        /// Interact anchor id.
        anchor_id: String,
        /// Absolute interact-anchor position.
        pos: [i32; 3],
        /// The chat command the bot sends.
        command: String,
        /// Item required in inventory, if any.
        requires_item: Option<String>,
    },
    /// Assert a scoreboard objective value.
    AssertComplete {
        /// The objective (`dw.campaign`).
        objective: String,
        /// Expected value.
        value: i32,
    },
}

/// Sanitize an id's local part (after its `/`) to `[a-z0-9_]`.
pub fn safe_local(id: &str) -> String {
    let local = id.split_once('/').map(|(_, r)| r).unwrap_or(id);
    local.replace(['-', '/', '.'], "_")
}

/// Version of the `critical-path.json` **contract** (its `format_version` field),
/// independent of the campaign's DSL version: the DSL describes the delve, this
/// describes what the harness is told about proving it.
///
/// * `1` — the pre-oracle shape (never written; a file with no `format_version`).
///   Steps carried no objective id, so the harness could only check position and a
///   single unanchored campaign-completion substring — a step could pass without
///   its objective completing.
/// * `2` — every objective-bearing step carries `objective`, and completion is
///   proved by the anchored per-objective marker channel ([`marker_line`]).
///
/// The harness **requires** the current version: an older `critical-path.json`
/// (which it cannot verify) is rejected rather than run hollow.
pub const CRITICAL_PATH_FORMAT_VERSION: u32 = 2;

/// The machine completion-marker token for campaign completion. An objective's
/// token is simply its own id (`obj/<kebab>`).
pub const MARKER_TOKEN_CAMPAIGN: &str = "campaign";

/// One line of the machine completion-marker channel:
/// `[dw:complete <campaign_id> <token>]`.
///
/// The format is **anchored and exact**: the harness matches the whole chat line
/// against this grammar (campaign id = the running campaign's, token = `campaign`
/// or an `obj/<kebab>` id), never a substring anywhere in chat. Three properties
/// make it a real oracle:
/// * player chat reaches the client as `<name> …`, so no player can utter a line
///   that starts with the sigil;
/// * the campaign id is part of the match, so a marker from other content cannot
///   satisfy this campaign's step;
/// * the sigil is reserved in every player-visible string by `DW0182`
///   ([`delvewright_dsl::validate_marker_channel`]), so authored — or
///   LLM-translated — text cannot forge one.
pub fn marker_line(campaign_id: &str, token: &str) -> String {
    format!("[dw:complete {campaign_id} {token}]")
}

/// Scoreboard objective for a DSL objective id.
pub fn obj_score(objective_id: &str) -> String {
    format!("dw.o_{}", safe_local(objective_id))
}
/// Scoreboard objective marking a quest complete.
pub fn quest_score(quest_id: &str) -> String {
    format!("dw.q_{}", safe_local(quest_id))
}
/// Scoreboard objective marking a quest active (its trigger fired).
pub fn quest_active_score(quest_id: &str) -> String {
    format!("dw.qa_{}", safe_local(quest_id))
}
/// Trigger objective for an NPC's dialogue.
pub fn dlg_trigger(npc_id: &str) -> String {
    format!("dw.dlg_{}", safe_local(npc_id))
}
/// Per-player scoreboard for a campaign flag (`set-flag` / `requires_flags`, v0.3).
pub fn flag_score(flag_id: &str) -> String {
    format!("dw.f_{}", safe_local(flag_id))
}
/// Trigger objective the bot chats / an interaction advancement sets to drive an
/// `interact` objective (v0.3).
pub fn interact_trigger(obj_id: &str) -> String {
    format!("dw.i_{}", safe_local(obj_id))
}
/// The shared scoreboard objective holding every wave's remaining-mob countdown
/// (fake players `#<wave>`, v0.3).
pub const WAVE_OBJECTIVE: &str = "dw.wave";
/// The fake-player key holding a wave's remaining-mob count.
pub fn wave_counter(wave_id: &str) -> String {
    format!("#{}", safe_local(wave_id))
}
/// The entity tag stamped on a wave's spawned mobs (v0.3).
pub fn wave_tag(wave_id: &str) -> String {
    format!("dw_wave_{}", safe_local(wave_id))
}

/// A stage-5 wave by id (v0.3).
pub fn wave_of<'a>(campaign: &'a Campaign, wave_id: &str) -> Option<&'a delvewright_dsl::Wave> {
    campaign
        .quests
        .content
        .waves
        .iter()
        .find(|w| w.id.as_str() == wave_id)
}
/// A wave's total mob count.
pub fn wave_total(wave: &delvewright_dsl::Wave) -> i32 {
    wave.mobs.iter().map(|m| m.count as i32).sum()
}

/// The area a stage-4 quest belongs to (free-function form of [`Plan::quest_area`],
/// usable before a [`Plan`] exists — e.g. from anchor collection).
fn quest_area_of<'a>(campaign: &'a Campaign, quest_id: &str) -> Option<&'a str> {
    campaign
        .quest_plan
        .content
        .quests
        .iter()
        .find(|q| q.id.as_str() == quest_id)
        .map(|q| q.area.as_str())
}

/// The area a wave's mobs spawn in — resolved from the wave's **spawn site**, not
/// from any `kill` objective. A `spawn-wave` effect (on a quest step, on a quest's
/// completion, or on an environment trigger) is what makes a wave appear; its
/// mobs materialize at `Wave.anchor` resolved in that spawning quest's area. This
/// is deliberately independent of objective type so a kill-less "live threat" wave
/// (spec-0008 §4 — e.g. a weakened warden the player sneaks past, an ambient mob
/// flock) resolves a spawn position exactly like a wave that is later slain.
///
/// Resolution order: the quest that fires the `spawn-wave` (`on_objective_complete`
/// or `on_complete`); else, in a single-area campaign, an environment trigger that
/// fires it (triggers are global — their sole possible area is the one area); else
/// a quest whose `kill` objective references the wave (defensive fallback for a
/// wave declared with a kill but no explicit spawn). `None` if nothing spawns it.
pub fn wave_area<'a>(campaign: &'a Campaign, wave_id: &str) -> Option<&'a str> {
    let spawns_wave = |e: &QuestEffect| matches!(e.spawn_wave(), Some(w) if w.as_str() == wave_id);
    // 1. A quest whose effects fire `spawn-wave` for this wave — the true spawn site.
    for q in &campaign.quests.content.quests {
        if q.on_objective_complete
            .values()
            .flatten()
            .chain(&q.on_complete)
            .any(&spawns_wave)
        {
            return quest_area_of(campaign, q.id.as_str());
        }
    }
    // 2. An environment trigger that fires it. Triggers are global; in a
    //    single-area campaign the sole area is unambiguous. (Multi-area
    //    trigger-only waves are not resolvable here and surface as a build
    //    diagnostic rather than a silent dangling spawn.)
    if campaign.world.content.areas.len() == 1
        && campaign
            .quests
            .content
            .triggers
            .iter()
            .any(|t| t.effects.iter().any(&spawns_wave))
    {
        return campaign.world.content.areas.first().map(|a| a.id.as_str());
    }
    // 3. Defensive fallback: a `kill` objective's quest.
    for q in &campaign.quests.content.quests {
        if q.objectives
            .iter()
            .any(|o| matches!(o, Objective::Kill { wave, .. } if wave.as_str() == wave_id))
        {
            return quest_area_of(campaign, q.id.as_str());
        }
    }
    None
}

/// Errors that stop planning (map to build failure, exit 3). Carries a stable
/// `DW03xx` build/solver diagnostic code (see `crates/compiler/README.md`).
#[derive(Debug)]
pub struct PlanError {
    /// The stable `DW03xx` code.
    pub code: &'static str,
    /// Human-readable explanation.
    pub message: String,
}

impl PlanError {
    /// Build a plan error with an explicit code.
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        PlanError {
            code,
            message: message.into(),
        }
    }
}

/// `DW0300`: generic build/resolution failure (missing prefab metadata, unknown
/// anchor, dependency cycle in the critical path).
pub const DW_BUILD: &str = "DW0300";

/// `DW0306`: gate-aware reachability deadlock (M2 fix 7). After the solver produces
/// a layout, sealed gates are modelled as cut edges in the piece-connectivity
/// graph; an objective whose anchor is only reachable through a gate that no
/// earlier objective (in the quest/objective DAG order) has opened is a deadlock —
/// the delve is unwinnable even though every anchor resolves. The canonical case:
/// a key chest sealed behind the very gate its key opens.
pub const DW_GATE_DEADLOCK: &str = "DW0306";

/// `DW0344`: an ocean-horizon world places a piece whose declared waterline does not
/// land at sea level — the piece floats above the sea or is drowned by it.
pub const DW_OCEAN_WATERLINE: &str = "DW0344";

/// `DW0345`: the assembled world resolves **no entry anchor** — the compiler has
/// no cell to call the campaign's start, so it cannot `setworldspawn`, cannot place
/// a first-joining player, and cannot teleport a player who picks a class. The
/// world then falls back to the vanilla spawn search, which a dedicated server
/// resolves to the surface but the integrated (singleplayer) server resolves to
/// the build floor — inside solid stone. Silent before; a hard build error now.
pub const DW_NO_ENTRY_ANCHOR: &str = "DW0345";

/// The prefab-metadata anchor names that mark a campaign's **entry point**, in
/// resolution order. One concept, two spellings in the shipped tileset library:
/// the keep/cave/test tilesets name it `spawn`, the island tileset names it
/// `entry`. The compiler owns the resolution (CLAUDE.md: never leave a layer
/// boundary to downstream folklore) — every consumer goes through
/// [`Plan::entry_point`] / `emit::campaign_spawn`, and a campaign that resolves
/// none of these names fails the build with [`DW_NO_ENTRY_ANCHOR`].
pub const ENTRY_ANCHOR_NAMES: [&str; 2] = ["spawn", "entry"];

/// Ocean-horizon waterline invariant (DW0344). In a `horizon: ocean` world every
/// placed piece that **declares** a waterline (`waterline_y` in its prefab metadata,
/// local y of its top authored water block) must land with that waterline at world
/// [`SEA_LEVEL`] — `piece.pos.y + waterline_y == 62`.
///
/// Why this is a hard invariant rather than a style rule: the island convention
/// (`prefabs/island-tileset.md`) puts the walkable land plane one block above the
/// waterline, which is the vanilla-normal beach relationship — a player swimming in
/// open sea can climb ashore, and the authored water reads as one body with the
/// world ocean. Off by a few blocks and the whole island floats above the sea: the
/// shore becomes an unclimbable cliff and the authored water pocket hangs in the
/// air. Nothing downstream (nav, boundary, POV, PackTest) can see this, because
/// every one of them derives from the very placement that is wrong.
///
/// Pieces that declare no `waterline_y` (interior keep/cave pieces, `hello-room`)
/// are not island pieces and are not checked.
fn check_ocean_waterline(
    campaign: &Campaign,
    areas: &[AreaPlacement],
    prefabs: &PrefabRegistry,
) -> Result<(), PlanError> {
    if !matches!(
        campaign.world.content.horizon,
        Some(delvewright_dsl::Horizon::Ocean)
    ) {
        return Ok(());
    }
    for area in areas {
        for piece in &area.pieces {
            let Some(meta) = prefabs.get(&piece.prefab_id) else {
                continue; // missing metadata is already DW0300 upstream
            };
            let Some(w) = meta.waterline_y else {
                continue;
            };
            let placed = piece.pos[1] + w;
            if placed != SEA_LEVEL {
                let delta = placed - SEA_LEVEL;
                let (dir, verb) = if delta > 0 {
                    (
                        "above",
                        "floats above the sea — its shore is an unclimbable cliff",
                    )
                } else {
                    ("below", "is drowned — the walk plane sits under the sea")
                };
                return Err(PlanError::new(
                    DW_OCEAN_WATERLINE,
                    format!(
                        "area `{}` places prefab `{}` at y={} with a declared waterline of local \
                         y={w}, putting its waterline at world y={placed} — {} blocks {dir} the \
                         ocean sea level (y={SEA_LEVEL}). The piece {verb}. Prefab metadata and \
                         placement disagree about the island datum: either declare the waterline \
                         the piece really authors (`waterline_y` in `{}.json`, the local y of its \
                         top water block — the island tileset convention is {ISLAND_WATERLINE_Y}), \
                         or rebuild the piece against that convention. Ocean areas are placed at \
                         y={OCEAN_BASE_Y} (= sea level - {ISLAND_WATERLINE_Y}); a piece with a \
                         different waterline cannot share that datum",
                        area.area_id,
                        piece.prefab_id,
                        piece.pos[1],
                        delta.abs(),
                        meta.structure.id,
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// Inter-area transport map: objective id → absolute teleport target (see
/// [`Plan::transport`]).
pub type TransportMap = BTreeMap<String, [i32; 3]>;

impl<'a> Plan<'a> {
    /// Build the plan. Requires a validated campaign and loaded prefab metadata.
    pub fn build(campaign: &'a Campaign, prefabs: &PrefabRegistry) -> Result<Self, PlanError> {
        let namespace = campaign.world.campaign_id.as_str().to_string();
        let seed = campaign.world.content.seed;

        // ---- placements + anchors ----
        let mut areas = Vec::new();
        let mut anchors: BTreeMap<(String, String), ResolvedAnchor> = BTreeMap::new();
        // v0.6 (spec-0011): the absolute dispenser socket cell for each `anchor/trap`
        // marker that declares one, keyed like `anchors`. Empty for a campaign with no
        // trap hardware.
        let mut dispenser_cells: BTreeMap<(String, String), [i32; 3]> = BTreeMap::new();
        // Per-batch affected AABBs from L2 massing (spec-0017 PR 3), for the
        // editor's per-batch snapshots. Empty without massing verbs.
        let mut massing_bounds: BTreeMap<String, ([i32; 3], [i32; 3])> = BTreeMap::new();
        // Socket doorways severed by `rewire-socket sealed`, per area — the
        // DW0306 connectivity graph must not count those edges.
        let mut severed: BTreeMap<String, BTreeSet<[i32; 3]>> = BTreeMap::new();
        // Origin Y is a per-horizon datum (spec-0013): void keeps 64, ocean drops to
        // sea_level-2 so the island waterline convention holds.
        let base_y = base_y(campaign);
        for (i, area) in campaign.world.content.areas.iter().enumerate() {
            let area_id = area.id.as_str().to_string();
            let origin = [i as i32 * AREA_SPACING, base_y, 0];

            let placement = if let Some(prefab) = &area.prefab {
                // Single-prefab area (the M1 degenerate assembly): one piece at
                // the origin, rotation none, no sockets to seal.
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
                if crate::massing::targets_area(campaign, &area_id) {
                    return Err(PlanError::new(
                        crate::massing::DW_MASSING,
                        format!(
                            "world-edits massing verbs target area `{area_id}`, which binds a \
                             single `prefab` — there is no jigsaw layout to mass. Massing \
                             applies only to `prefab_pool` areas; use the L3 detailing verbs \
                             (carve/fill/fragment/…) on a single-prefab area instead"
                        ),
                    ));
                }
                for (name, am) in &meta.anchors {
                    anchors.insert((area_id.clone(), name.clone()), resolve_anchor(origin, am));
                    if let Some(dp) = am.dispenser {
                        dispenser_cells.insert(
                            (area_id.clone(), name.clone()),
                            [origin[0] + dp[0], origin[1] + dp[1], origin[2] + dp[2]],
                        );
                    }
                }
                AreaPlacement {
                    area_id: area_id.clone(),
                    pieces: vec![PiecePlacement {
                        prefab_id,
                        structure_id: meta.structure.id.clone(),
                        structure_file: meta.structure.file.clone(),
                        pos: origin,
                        size: meta.structure.size,
                        rotation: Rotation::None,
                    }],
                    seals: Vec::new(),
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
                .map_err(|e| PlanError::new(e.code, e.message))?;
                // Stage-7 L2 massing (spec-0017 PR 3): apply the edit script's
                // massing batches for this area over the solved layout, so
                // everything downstream — anchor resolution just below, the
                // gate/waterline checks, assembly, relight, nav, the L3
                // detailing replay — sees the massaged layout. No-op (layout
                // and seals byte-identical) for a campaign without massing
                // verbs targeting this area.
                let massing_out =
                    crate::massing::apply(campaign, &area_id, &mut layout, prefabs, seed)
                        .map_err(|e| PlanError::new(e.code, e.message))?;
                massing_bounds.extend(massing_out.bounds);
                if !massing_out.severed.is_empty() {
                    severed.insert(area_id.clone(), massing_out.severed);
                }

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
                        anchors
                            .entry((area_id.clone(), name.clone()))
                            .or_insert_with(|| resolve_piece_anchor(placed, am));
                        if let Some(dp) = am.dispenser {
                            dispenser_cells
                                .entry((area_id.clone(), name.clone()))
                                .or_insert_with(|| solver::transform_point(placed, dp));
                        }
                    }
                    pieces.push(PiecePlacement {
                        prefab_id: placed.prefab_id.clone(),
                        structure_id: meta.structure.id.clone(),
                        structure_file: meta.structure.file.clone(),
                        pos: placed.pos,
                        size: meta.structure.size,
                        rotation: placed.rotation,
                    });
                }
                AreaPlacement {
                    area_id: area_id.clone(),
                    pieces,
                    seals: layout.seals,
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

        // ---- gate-aware reachability (M2 fix 7, DW0306) ----
        // With the layout solved, verify no objective's anchor is sealed behind a
        // gate that only a later objective opens (an unwinnable deadlock the anchor
        // resolver alone cannot see).
        for area in &areas {
            check_gate_reachability(
                campaign,
                &area.area_id,
                &area.pieces,
                prefabs,
                severed.get(&area.area_id),
            )?;
        }

        // ---- ocean waterline invariant (DW0344) ----
        check_ocean_waterline(campaign, &areas, prefabs)?;

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

        // ---- critical path + inter-area transport ----
        let cp = build_critical_path(campaign, &anchors, &npcs)?;

        // ---- v0.6 checkpoints + stealth beats (spec-0012 / spec-0014) ----
        let (checkpoints, stealth_beats) = collect_v06_effects(campaign, &anchors, &cp.obj_step);
        let objective_steps = cp.obj_step;

        // ---- v0.6 traps (spec-0011) ----
        let traps = collect_traps(campaign, &anchors, &dispenser_cells);

        // ---- v0.6 gate open/close firings (drives the close-gate nav proof) ----
        let gate_events = collect_gate_events(campaign, &anchors, &objective_steps);
        let strict_ancestor_steps = compute_strict_ancestor_steps(campaign, &objective_steps);

        Ok(Self {
            campaign,
            namespace,
            seed,
            areas,
            anchors,
            classes,
            npcs,
            critical_path: cp.steps,
            transport: cp.transport,
            critical_path_transport: cp.transport_by_step,
            critical_path_sneak: cp.sneak_by_step,
            critical_path_cutscene: cp.cutscene_by_step,
            checkpoints,
            stealth_beats,
            objective_steps,
            traps,
            gate_events,
            strict_ancestor_steps,
            massing_bounds,
        })
    }

    /// Whether a gate firing at critical-path step `g` is guaranteed to have fired
    /// before a walked leg arriving at step `s` — i.e. `g`'s objective is a strict
    /// DAG ancestor of `s`'s objective (see [`Self::strict_ancestor_steps`]). Step
    /// `0` (class-select / an environment trigger's conservative fire step) is
    /// treated as always-preceding. Drives the `close-gate` seal model in
    /// `crate::nav`.
    pub fn gate_fired_before(&self, g: usize, s: usize) -> bool {
        g == 0
            || self
                .strict_ancestor_steps
                .get(&s)
                .is_some_and(|anc| anc.contains(&g))
    }

    /// The area an NPC or quest belongs to.
    pub fn npc_area(&self, npc_id: &str) -> Option<&str> {
        self.campaign
            .npcs
            .content
            .npcs
            .iter()
            .find(|n| n.id.as_str() == npc_id)
            .map(|n| n.area.as_str())
    }

    /// The area a stage-4 quest belongs to.
    pub fn quest_area(&self, quest_id: &str) -> Option<&str> {
        self.campaign
            .quest_plan
            .content
            .quests
            .iter()
            .find(|q| q.id.as_str() == quest_id)
            .map(|q| q.area.as_str())
    }

    /// Resolve `(area, anchor)` to a point position, if it is a point anchor.
    pub fn point(&self, area_id: &str, anchor: &str) -> Option<[i32; 3]> {
        match self.anchors.get(&(area_id.to_string(), anchor.to_string())) {
            Some(ResolvedAnchor::Point { pos, .. }) => Some(*pos),
            _ => None,
        }
    }

    /// Whether any collected checkpoint carries an `on_respawn` hook — gates the
    /// vanilla respawn-detection machinery so checkpoint-free / hook-free campaigns
    /// stay byte-identical (DSL v0.6, spec-0012).
    pub fn any_checkpoint_on_respawn(&self) -> bool {
        self.checkpoints.iter().any(|c| !c.on_respawn.is_empty())
    }

    /// The collected checkpoint matching a `set-checkpoint` effect (by anchor +
    /// `on_respawn` list), giving the emitter its stable content-ordered index.
    pub fn checkpoint_for(
        &self,
        anchor: &str,
        on_respawn: &[QuestEffect],
    ) -> Option<&CheckpointPlan> {
        self.checkpoints
            .iter()
            .find(|c| c.anchor == anchor && c.on_respawn.as_slice() == on_respawn)
    }

    /// The collected stealth beat matching a `begin-stealth` effect (by zone
    /// anchors + `grace_ticks`), giving the emitter its 1-based session id.
    pub fn stealth_for(
        &self,
        zones: &[delvewright_dsl::StealthZone],
        grace: u32,
    ) -> Option<&StealthBeat> {
        self.stealth_beats.iter().find(|b| {
            b.grace_ticks == grace
                && b.zones.len() == zones.len()
                && b.zones
                    .iter()
                    .zip(zones)
                    .all(|((a, _, e), z)| a.as_str() == z.anchor.as_str() && *e == z.extent)
        })
    }
}

/// The set of anchor names the campaign references inside `area_id`: NPC stands
/// (NPCs in this area), `reach-anchor` targets and `open-gate` anchors (quests
/// planned in this area). Sorted + deduped for deterministic solver input. These
/// are the anchors the solver must guarantee exist in the assembled layout.
fn required_anchors_for_area(campaign: &Campaign, area_id: &str) -> Vec<String> {
    let mut set: BTreeSet<String> = BTreeSet::new();
    for npc in &campaign.npcs.content.npcs {
        if npc.area.as_str() == area_id {
            set.insert(npc.anchor.as_str().to_string());
        }
    }
    // Which planned quests belong to this area.
    let quest_area: BTreeMap<&str, &str> = campaign
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| (q.id.as_str(), q.area.as_str()))
        .collect();
    for q in &campaign.quests.content.quests {
        if quest_area.get(q.id.as_str()).copied() != Some(area_id) {
            continue;
        }
        for o in &q.objectives {
            match o {
                Objective::ReachAnchor { anchor, .. } | Objective::Collect { anchor, .. } => {
                    set.insert(anchor.as_str().to_string());
                }
                Objective::Interact { anchor, .. } => {
                    set.insert(anchor.as_str().to_string());
                }
                // Wave spawn anchors are registered below via `wave_area`, driven
                // by the `spawn-wave` effect (the true spawn site) rather than the
                // `kill` objective — so a kill-less live-threat wave is placed too.
                Objective::Kill { .. } | Objective::TalkTo { .. } => {}
            }
        }
        for e in q
            .on_objective_complete
            .values()
            .flatten()
            .chain(&q.on_complete)
        {
            collect_effect_anchors(e, &mut set);
        }
    }
    // Wave spawn anchors: a `spawn-wave` effect materializes its mobs at the wave's
    // `anchor` in the area of the quest (or single-area trigger) that fires it —
    // independent of any `kill` objective. Register the anchor for that area so the
    // solver guarantees a piece providing it; a kill-less live-threat wave would
    // otherwise resolve no spawn position and its `spawn_<wave>` call would dangle.
    for w in &campaign.quests.content.waves {
        if wave_area(campaign, w.id.as_str()) == Some(area_id) {
            set.insert(w.anchor.as_str().to_string());
        }
    }
    // Environment triggers (v0.4) are global. When the campaign has a single area,
    // their `at` and effect anchors must be provided by that area's assembly. For
    // a multi-area campaign, a trigger anchor is expected to coincide with an
    // objective anchor (already required above); over-provisioning every area is
    // avoided so the solver is not asked for an anchor an area's pool cannot fit.
    if campaign.world.content.areas.len() == 1 {
        for t in &campaign.quests.content.triggers {
            set.insert(t.at.as_str().to_string());
            for e in &t.effects {
                collect_effect_anchors(e, &mut set);
            }
        }
    }
    set.into_iter().collect()
}

/// Collect the anchors a v0.4 quest effect references (`open-gate`, `set-block`,
/// `move-npc` target, `cutscene` waypoints) into `set`, so the layout solver
/// guarantees they exist in the assembled area.
fn collect_effect_anchors(e: &QuestEffect, set: &mut BTreeSet<String>) {
    if let Some(a) = e.open_gate_anchor() {
        set.insert(a.as_str().to_string());
    }
    if let Some(a) = e.close_gate_anchor() {
        set.insert(a.as_str().to_string());
    }
    if let Some((a, _)) = e.set_block() {
        set.insert(a.as_str().to_string());
    }
    if let Some((_, a)) = e.move_npc() {
        set.insert(a.as_str().to_string());
    }
    // Every shot's waypoints, plus each shot's `look_at` subject — the camera is
    // aimed at that world point, so the area's assembly must provide its anchor.
    if let Some(shots) = e.cutscene_shots() {
        for shot in &shots {
            for w in &shot.path {
                set.insert(w.anchor.as_str().to_string());
            }
            if let Some(t) = &shot.look_at {
                set.insert(t.anchor.as_str().to_string());
            }
        }
    }
}

/// Resolve a placed-piece anchor to absolute world coords (transforming through
/// the piece's pos + rotation).
fn resolve_piece_anchor(placed: &solver::PlacedPiece, am: &AnchorMeta) -> ResolvedAnchor {
    if let Some(region) = &am.region {
        ResolvedAnchor::Gate {
            from: solver::transform_point(placed, region.from),
            to: solver::transform_point(placed, region.to),
            block: am
                .block
                .clone()
                .unwrap_or_else(|| "minecraft:air".to_string()),
        }
    } else {
        ResolvedAnchor::Point {
            pos: solver::transform_point(placed, am.pos.unwrap_or([0, 0, 0])),
            facing: solver::transform_facing(placed, am.facing.as_deref()),
        }
    }
}

fn resolve_anchor(origin: [i32; 3], am: &AnchorMeta) -> ResolvedAnchor {
    let add = |p: [i32; 3]| [origin[0] + p[0], origin[1] + p[1], origin[2] + p[2]];
    if let Some(region) = &am.region {
        ResolvedAnchor::Gate {
            from: add(region.from),
            to: add(region.to),
            block: am
                .block
                .clone()
                .unwrap_or_else(|| "minecraft:air".to_string()),
        }
    } else {
        ResolvedAnchor::Point {
            pos: add(am.pos.unwrap_or([0, 0, 0])),
            facing: am.facing.clone(),
        }
    }
}

fn plan_npc(npc: &Npc, tree: &NpcDialogue) -> NpcPlan {
    let safe = safe_local(npc.id.as_str());
    let mut options = Vec::new();
    let mut n = 0;
    for node in &tree.nodes {
        for opt in &node.options {
            n += 1;
            let mut completes = Vec::new();
            let mut sets_flags = Vec::new();
            let mut sets_time = Vec::new();
            let mut sets_weather = Vec::new();
            let mut sets_checkpoints = Vec::new();
            let mut spawns_npcs = Vec::new();
            for e in &opt.effects {
                match e {
                    DialogueEffect::CompleteObjective { objective } => {
                        completes.push(objective.as_str().to_string());
                    }
                    DialogueEffect::SetFlag { flag } => {
                        sets_flags.push(flag.as_str().to_string());
                    }
                    DialogueEffect::SetTime { time } => sets_time.push(*time),
                    DialogueEffect::SetWeather { weather } => sets_weather.push(*weather),
                    DialogueEffect::SetCheckpoint { anchor, on_respawn } => {
                        sets_checkpoints.push((anchor.as_str().to_string(), on_respawn.clone()));
                    }
                    DialogueEffect::SpawnNpc { npc } => {
                        spawns_npcs.push(npc.as_str().to_string());
                    }
                }
            }
            options.push(OptionPlan {
                n,
                node_id: node.id.as_str().to_string(),
                label: opt.label.clone(),
                next: opt
                    .next
                    .as_ref()
                    .map(|d: &DialogueId| d.as_str().to_string()),
                completes,
                sets_flags,
                requires_flags: opt
                    .requires_flags
                    .iter()
                    .map(|f| f.as_str().to_string())
                    .collect(),
                forbids_flags: opt
                    .forbids_flags
                    .iter()
                    .map(|f| f.as_str().to_string())
                    .collect(),
                sets_time,
                sets_weather,
                sets_checkpoints,
                spawns_npcs,
            });
        }
    }
    NpcPlan {
        npc_id: npc.id.as_str().to_string(),
        trigger_objective: dlg_trigger(npc.id.as_str()),
        tag: format!("dw_npc_{safe}"),
        root: tree.root.as_str().to_string(),
        safe,
        options,
    }
}

/// The computed critical path and its per-step metadata.
struct CriticalPath {
    steps: Vec<Step>,
    transport: TransportMap,
    transport_by_step: Vec<Option<[i32; 3]>>,
    sneak_by_step: Vec<bool>,
    cutscene_by_step: Vec<Option<u32>>,
    /// Objective id → its `critical_path` step index (v0.6): roots the checkpoint
    /// no-stranding proof (DW0315) and the stealth-zone reachability proof
    /// (DW0327) at the beat that fires the effect.
    obj_step: BTreeMap<String, usize>,
}

/// Build the critical path: select first class, then each objective of the
/// **flow-proven single-branch playthrough** ([`crate::flow::Flow::playthrough`])
/// in topological order (quests by `depends_on`, objectives by `after`), then
/// assert campaign completion. Quests that belong to a mutually exclusive branch
/// the chosen playthrough does not take are excluded, and each `talk-to` takes
/// the completing dialogue option that belongs to that branch — so the exported
/// path is a sequence one player can actually walk (proven by
/// `crate::flow::Flow::replay`, `DW0204`, before the build reaches here).
///
/// Also returns the inter-area transport map (when consecutive objectives sit in
/// different areas) and, per step, the DSL v0.4 harness hints: `sneak` (a
/// `stealth` objective) and `cutscene_seconds` (a step whose completion triggers
/// a `QuestEffect::Cutscene`).
fn build_critical_path(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    npcs: &[NpcPlan],
) -> Result<CriticalPath, PlanError> {
    let mut steps = Vec::new();
    // (objective id, physical area, step index) in critical-path order, for the
    // transport map and the per-step transport marker.
    let mut obj_areas: Vec<(String, String, usize)> = Vec::new();

    // select-class: first declared class.
    if let Some(first) = campaign.classes.content.classes.first() {
        steps.push(Step::SelectClass {
            class_id: first.id.as_str().to_string(),
            command: "/trigger dw.class set 1".to_string(),
        });
    }

    // The branch-coherent playthrough: one world's completing quests in
    // `depends_on` order, their objectives in `after` order, and the dialogue
    // option each `talk-to` takes on that branch.
    let flow = crate::flow::Flow::new(campaign);
    let path = flow.playthrough();
    if path.cyclic {
        return Err(PlanError::new(
            DW_BUILD,
            "internal invariant violation: a quest dependency cycle survived into critical-path \
             ordering — `DW0130` should have rejected it in validation. This is a compiler bug; \
             stop and escalate",
        ));
    }
    let stage5: BTreeMap<&str, &_> = campaign
        .quests
        .content
        .quests
        .iter()
        .map(|q| (q.id.as_str(), q))
        .collect();

    for st in &path.steps {
        let qid = st.quest.as_str();
        let Some(quest) = stage5.get(qid) else {
            continue;
        };
        let area = campaign
            .quest_plan
            .content
            .quests
            .iter()
            .find(|q| q.id.as_str() == qid)
            .map(|q| q.area.as_str())
            .unwrap_or("");
        let Some(obj) = quest
            .objectives
            .iter()
            .find(|o| o.id().as_str() == st.objective)
        else {
            continue;
        };
        {
            match obj {
                Objective::TalkTo { id, npc, .. } => {
                    let npc_plan =
                        npcs.iter()
                            .find(|n| n.npc_id == npc.as_str())
                            .ok_or_else(|| {
                                PlanError::new(
                                    DW_BUILD,
                                    format!(
                                        "internal invariant violation: `talk-to` references npc \
                                         `{npc}` with no build-time plan — `DW0112`/`DW0152` \
                                         should have caught this in validation. This is a compiler \
                                         bug; stop and escalate"
                                    ),
                                )
                            })?;
                    // The branch-consistent completing option (the flow model
                    // picked it); fall back to the first completing option only
                    // for a campaign with no branch at all.
                    let opt = st
                        .talk_option
                        .and_then(|n| npc_plan.options.iter().find(|o| o.n as usize == n))
                        .or_else(|| {
                            npc_plan
                                .options
                                .iter()
                                .find(|o| o.completes.iter().any(|c| c == id.as_str()))
                        })
                        .ok_or_else(|| {
                            PlanError::new(DW_BUILD, format!(
                                "internal invariant violation: objective `{id}` has no dialogue \
                                 option completing it at build time — `DW0123`/`DW0203` should \
                                 have caught this in validation/analysis. This is a compiler bug; \
                                 stop and escalate"
                            ))
                        })?;
                    // NPC position: its declared anchor within its area.
                    let npc_anchor = campaign
                        .npcs
                        .content
                        .npcs
                        .iter()
                        .find(|nn| nn.id.as_str() == npc.as_str())
                        .map(|nn| nn.anchor.as_str())
                        .unwrap_or("");
                    let npc_area = campaign
                        .npcs
                        .content
                        .npcs
                        .iter()
                        .find(|nn| nn.id.as_str() == npc.as_str())
                        .map(|nn| nn.area.as_str())
                        .unwrap_or(area);
                    let pos = point_of(anchors, npc_area, npc_anchor)?;
                    steps.push(Step::TalkTo {
                        objective_id: id.as_str().to_string(),
                        npc_id: npc.as_str().to_string(),
                        pos,
                        command: format!("/trigger {} set {}", npc_plan.trigger_objective, opt.n),
                    });
                    obj_areas.push((
                        id.as_str().to_string(),
                        npc_area.to_string(),
                        steps.len() - 1,
                    ));
                }
                Objective::ReachAnchor {
                    id, anchor, radius, ..
                } => {
                    let pos = point_of(anchors, area, anchor.as_str())?;
                    steps.push(Step::Reach {
                        objective_id: id.as_str().to_string(),
                        anchor_id: anchor.as_str().to_string(),
                        pos,
                        radius: *radius,
                    });
                    obj_areas.push((id.as_str().to_string(), area.to_string(), steps.len() - 1));
                }
                Objective::Kill { id, wave, .. } => {
                    let w = wave_of(campaign, wave.as_str()).ok_or_else(|| {
                        PlanError::new(
                            DW_BUILD,
                            format!(
                                "internal invariant violation: `kill` objective references wave \
                                 `{wave}` with no declaration at build time — `DW0170` should have \
                                 caught this in validation. This is a compiler bug; stop and \
                                 escalate"
                            ),
                        )
                    })?;
                    let pos = point_of(anchors, area, w.anchor.as_str())?;
                    steps.push(Step::Kill {
                        objective_id: id.as_str().to_string(),
                        wave_id: wave.as_str().to_string(),
                        pos,
                        tag: wave_tag(wave.as_str()),
                        count: wave_total(w),
                    });
                    obj_areas.push((id.as_str().to_string(), area.to_string(), steps.len() - 1));
                }
                Objective::Collect {
                    id,
                    item,
                    count,
                    anchor,
                    ..
                } => {
                    let pos = point_of(anchors, area, anchor.as_str())?;
                    steps.push(Step::Collect {
                        objective_id: id.as_str().to_string(),
                        item: item.clone(),
                        count: *count as i32,
                        pos,
                    });
                    obj_areas.push((id.as_str().to_string(), area.to_string(), steps.len() - 1));
                }
                Objective::Interact {
                    id,
                    anchor,
                    requires_item,
                    ..
                } => {
                    let pos = point_of(anchors, area, anchor.as_str())?;
                    steps.push(Step::Interact {
                        objective_id: id.as_str().to_string(),
                        anchor_id: anchor.as_str().to_string(),
                        pos,
                        command: format!("/trigger {} set 1", interact_trigger(id.as_str())),
                        requires_item: requires_item.clone(),
                    });
                    obj_areas.push((id.as_str().to_string(), area.to_string(), steps.len() - 1));
                }
            }
        }
    }

    steps.push(Step::AssertComplete {
        objective: "dw.campaign".to_string(),
        value: 1,
    });

    // Transport: when consecutive critical objectives change area, completing the
    // earlier objective teleports the player to the later area's entry spawn.
    let mut transport: BTreeMap<String, [i32; 3]> = BTreeMap::new();
    // Per-step transport marker, aligned with `steps`. Filled from `transport` via
    // each objective's recorded step index (gap 8).
    let mut transport_by_step: Vec<Option<[i32; 3]>> = vec![None; steps.len()];
    for pair in obj_areas.windows(2) {
        let (prev_id, prev_area, prev_idx) = &pair[0];
        let (_, next_area, _) = &pair[1];
        if prev_area != next_area
            && let Some(ResolvedAnchor::Point { pos, .. }) =
                anchors.get(&(next_area.clone(), "spawn".to_string()))
        {
            transport.insert(prev_id.clone(), *pos);
            transport_by_step[*prev_idx] = Some(*pos);
        }
    }

    // DSL v0.4 per-step harness hints: `sneak` (a stealth objective) and
    // `cutscene_seconds` (a step whose completion fires a `Cutscene` effect).
    let mut sneak_by_step = vec![false; steps.len()];
    let mut cutscene_by_step: Vec<Option<u32>> = vec![None; steps.len()];
    for (obj_id, _, step_idx) in &obj_areas {
        if let Some((qid, obj)) = objective_quest(campaign, obj_id) {
            sneak_by_step[*step_idx] = obj.stealth();
            let mut secs = cutscene_seconds_in(objective_effects(campaign, obj_id).into_iter());
            if secs.is_none()
                && is_last_objective_of_quest(campaign, qid, obj_id)
                && let Some(q) = campaign
                    .quests
                    .content
                    .quests
                    .iter()
                    .find(|q| q.id.as_str() == qid)
            {
                secs = cutscene_seconds_in(q.on_complete.iter());
            }
            cutscene_by_step[*step_idx] = secs;
        }
    }

    let obj_step: BTreeMap<String, usize> = obj_areas
        .iter()
        .map(|(id, _, idx)| (id.clone(), *idx))
        .collect();

    Ok(CriticalPath {
        steps,
        transport,
        transport_by_step,
        sneak_by_step,
        cutscene_by_step,
        obj_step,
    })
}

/// Resolve an anchor name to a point cell by scanning every area's resolved
/// anchors (first match), mirroring the emitter's `anchor_point_any`.
fn point_any(anchors: &BTreeMap<(String, String), ResolvedAnchor>, name: &str) -> Option<[i32; 3]> {
    for ((_, n), resolved) in anchors {
        if n == name {
            return match resolved {
                ResolvedAnchor::Point { pos, .. } => Some(*pos),
                ResolvedAnchor::Gate { from, .. } => Some(*from),
            };
        }
    }
    None
}

/// The `critical_path` step index at which a quest's `on_complete` fires: its
/// last objective's step (max over the quest's objectives). `0` if the quest has
/// no positioned objective (degenerate; conservative — proves the whole path).
fn quest_complete_step(quest: &Quest, obj_step: &BTreeMap<String, usize>) -> usize {
    quest
        .objectives
        .iter()
        .filter_map(|o| obj_step.get(o.id().as_str()).copied())
        .max()
        .unwrap_or(0)
}

/// The `critical_path` step index of the `talk-to` objective that a dialogue tree
/// belongs to (its NPC's completing beat), rooting a dialogue-hosted
/// `set-checkpoint`. `0` if none is found (degenerate).
fn dialogue_fire_step(
    campaign: &Campaign,
    npc_id: &str,
    obj_step: &BTreeMap<String, usize>,
) -> usize {
    campaign
        .quests
        .content
        .quests
        .iter()
        .flat_map(|q| q.objectives.iter())
        .filter_map(|o| match o {
            Objective::TalkTo { id, npc, .. } if npc.as_str() == npc_id => {
                obj_step.get(id.as_str()).copied()
            }
            _ => None,
        })
        .min()
        .unwrap_or(0)
}

/// Collect every `set-checkpoint` and `begin-stealth` effect (DSL v0.6) in a
/// deterministic content order, resolving each anchor to a cell and rooting it at
/// its firing step. An effect whose anchor does not resolve to a point is skipped
/// here (validation guarantees the anchor exists; a pool anchor that fails to
/// resolve at plan time simply carries no proof/emission).
fn collect_v06_effects(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    obj_step: &BTreeMap<String, usize>,
) -> (Vec<CheckpointPlan>, Vec<StealthBeat>) {
    let mut c = V06Collector {
        anchors,
        checkpoints: Vec::new(),
        stealth: Vec::new(),
    };

    // Stage 5 — quest effects (on_objective_complete, then on_complete).
    for q in &campaign.quests.content.quests {
        for (obj_id, effs) in &q.on_objective_complete {
            let step = obj_step.get(obj_id.as_str()).copied().unwrap_or(0);
            for eff in effs {
                c.handle(eff, step);
            }
        }
        let done_step = quest_complete_step(q, obj_step);
        for eff in &q.on_complete {
            c.handle(eff, done_step);
        }
    }

    // Stage 5 — environment triggers (conservative fire step 0: a trigger fires on
    // an environmental condition, not a critical beat, so require the checkpoint to
    // re-reach the whole remaining path).
    for t in &campaign.quests.content.triggers {
        for eff in &t.effects {
            c.handle(eff, 0);
        }
    }

    // Stage 6 — dialogue `set-checkpoint` (rooted at the NPC's talk-to beat).
    for tree in &campaign.dialogue.content.dialogues {
        let step = dialogue_fire_step(campaign, tree.npc.as_str(), obj_step);
        for node in &tree.nodes {
            for opt in &node.options {
                for eff in &opt.effects {
                    if let Some((anchor, on_respawn)) = eff.set_checkpoint() {
                        c.push_checkpoint(anchor.as_str(), on_respawn, step);
                    }
                }
            }
        }
    }

    (c.checkpoints, c.stealth)
}

/// The absolute gate region `(from, to)` a gate anchor resolves to (globally, like
/// `open-gate`/`close-gate` resolution). `None` if the anchor is not a gate.
fn gate_region_any(
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    name: &str,
) -> Option<([i32; 3], [i32; 3])> {
    for ((_, n), resolved) in anchors {
        if n == name
            && let ResolvedAnchor::Gate { from, to, .. } = resolved
        {
            return Some((*from, *to));
        }
    }
    None
}

/// Collect every `open-gate` / `close-gate` firing (DSL v0.6) in deterministic
/// content order — quest `on_objective_complete`, then `on_complete`, then
/// environment triggers (conservative fire step 0, like `collect_v06_effects`),
/// then dialogue options — resolving each anchor to its gate region and rooting it
/// at its firing step. Descends every nested effect list so a gate effect inside a
/// `sequence` step / lifecycle bundle is registered at the same firing step. An
/// effect whose anchor is not a resolvable gate is skipped (a point anchor / bad
/// close-gate is a validation concern, `DW0142`/`DW0343`). Feeds the `close-gate`
/// completability model in `crate::nav`. Gates are a quest/trigger-effect surface
/// only (the `DialogueEffect` enum carries no gate verb), so dialogue is not
/// scanned.
fn collect_gate_events(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    obj_step: &BTreeMap<String, usize>,
) -> Vec<GateEvent> {
    let mut out = Vec::new();
    let handle = |eff: &QuestEffect, fire_step: usize, out: &mut Vec<GateEvent>| {
        eff.visit_deep(&mut |e| {
            let gate = e
                .open_gate_anchor()
                .map(|a| (a, false))
                .or_else(|| e.close_gate_anchor().map(|a| (a, true)));
            if let Some((anchor, closes)) = gate
                && let Some(region) = gate_region_any(anchors, anchor.as_str())
            {
                out.push(GateEvent {
                    region,
                    closes,
                    fire_step,
                });
            }
        });
    };
    for q in &campaign.quests.content.quests {
        for (obj_id, effs) in &q.on_objective_complete {
            let step = obj_step.get(obj_id.as_str()).copied().unwrap_or(0);
            for eff in effs {
                handle(eff, step, &mut out);
            }
        }
        let done_step = quest_complete_step(q, obj_step);
        for eff in &q.on_complete {
            handle(eff, done_step, &mut out);
        }
    }
    for t in &campaign.quests.content.triggers {
        for eff in &t.effects {
            handle(eff, 0, &mut out);
        }
    }
    out
}

/// Compute, for each objective's `critical_path` step, the set of steps of its
/// **strict DAG ancestors** (see [`Plan::strict_ancestor_steps`]): the transitive
/// `after`-closure within its own quest, plus every objective of every transitive
/// `depends_on`-ancestor quest (a quest completes — all its objectives — before any
/// dependent quest starts). Pure DAG structure, so it is deterministic and
/// independent of the lineariser's choice among valid orders.
/// Transitive-reachability closure over a `node → direct successors` adjacency,
/// seeded by `start` (exclusive of the seeds' own membership only insofar as they
/// re-enter via the graph). Shared by the quest-`depends_on` and objective-`after`
/// ancestor computations.
fn transitive_closure<'a>(
    start: &[&'a str],
    next: &BTreeMap<&'a str, Vec<&'a str>>,
) -> BTreeSet<&'a str> {
    let mut seen: BTreeSet<&'a str> = BTreeSet::new();
    let mut stack: Vec<&'a str> = start.to_vec();
    while let Some(x) = stack.pop() {
        if seen.insert(x)
            && let Some(nx) = next.get(x)
        {
            stack.extend(nx.iter().copied());
        }
    }
    seen
}

fn compute_strict_ancestor_steps(
    campaign: &Campaign,
    obj_step: &BTreeMap<String, usize>,
) -> BTreeMap<usize, BTreeSet<usize>> {
    // Quest direct `depends_on`, then its transitive-ancestor closure.
    let quest_deps: BTreeMap<&str, Vec<&str>> = campaign
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| {
            (
                q.id.as_str(),
                q.depends_on.iter().map(|d| d.as_str()).collect(),
            )
        })
        .collect();
    let quest_anc: BTreeMap<&str, BTreeSet<&str>> = quest_deps
        .iter()
        .map(|(q, deps)| (*q, transitive_closure(deps, &quest_deps)))
        .collect();

    // Objective structure from stage 5: quest→objectives, objective→quest, `after`.
    let mut quest_objs: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut obj_quest: BTreeMap<&str, &str> = BTreeMap::new();
    let mut obj_after: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for q in &campaign.quests.content.quests {
        let qid = q.id.as_str();
        for o in &q.objectives {
            let oid = o.id().as_str();
            quest_objs.entry(qid).or_default().push(oid);
            obj_quest.insert(oid, qid);
            obj_after.insert(oid, o.after().iter().map(|a| a.as_str()).collect());
        }
    }
    let after_closure: BTreeMap<&str, BTreeSet<&str>> = obj_after
        .iter()
        .map(|(o, a)| (*o, transitive_closure(a, &obj_after)))
        .collect();

    // Assemble the step-level ancestor sets.
    let mut out: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for (&oid, &qid) in &obj_quest {
        let Some(&s) = obj_step.get(oid) else {
            continue;
        };
        let mut anc: BTreeSet<usize> = BTreeSet::new();
        let add = |name: &str, anc: &mut BTreeSet<usize>| {
            if let Some(&st) = obj_step.get(name) {
                anc.insert(st);
            }
        };
        if let Some(cl) = after_closure.get(oid) {
            for a in cl {
                add(a, &mut anc);
            }
        }
        if let Some(aq) = quest_anc.get(qid) {
            for q2 in aq {
                if let Some(objs) = quest_objs.get(*q2) {
                    for a in objs {
                        add(a, &mut anc);
                    }
                }
            }
        }
        out.insert(s, anc);
    }
    out
}

/// Resolve every stage-5 trap (DSL v0.6, spec-0011) in content order into a
/// [`TrapPlan`]: the trigger/hazard cell (the trap's `at` anchor), the dispenser
/// socket cell (from the `at` anchor's metadata), the dispense payload, and the
/// disarm affordance. A trap whose `at` anchor does not resolve to a point is
/// skipped (validation guarantees the anchor exists; an unresolved pool anchor
/// simply carries no proof/emission — the same policy as `collect_v06_effects`).
fn collect_traps(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    dispenser_cells: &BTreeMap<(String, String), [i32; 3]>,
) -> Vec<TrapPlan> {
    let mut out = Vec::new();
    for t in &campaign.quests.content.traps {
        let Some(trigger_cell) = point_any(anchors, t.at.as_str()) else {
            continue;
        };
        let dispenser = dispenser_cells
            .iter()
            .find(|((_, name), _)| name == t.at.as_str())
            .map(|(_, cell)| *cell);
        let payload = t.dispense().map(|(item, count)| (item.to_string(), count));
        let disarm = t.disarm.as_ref().and_then(|dis| {
            point_any(anchors, dis.via.as_str()).map(|via_cell| TrapDisarmPlan {
                via_anchor: dis.via.as_str().to_string(),
                via_cell,
                sets_flag: dis.sets_flag.as_str().to_string(),
            })
        });
        out.push(TrapPlan {
            id: t.id.as_str().to_string(),
            safe: safe_local(t.id.as_str()),
            trigger: t.trigger,
            trigger_cell,
            dispenser,
            payload,
            lethality: t.lethality,
            reset: t.reset,
            disarm,
            requires_flags: t
                .requires_flags
                .iter()
                .map(|f| f.as_str().to_string())
                .collect(),
            forbids_flags: t
                .forbids_flags
                .iter()
                .map(|f| f.as_str().to_string())
                .collect(),
        });
    }
    out
}

/// Accumulates v0.6 checkpoints / stealth beats in content order while resolving
/// their anchors (a struct so the collection borrows stay simple).
struct V06Collector<'a> {
    anchors: &'a BTreeMap<(String, String), ResolvedAnchor>,
    checkpoints: Vec<CheckpointPlan>,
    stealth: Vec<StealthBeat>,
}

impl V06Collector<'_> {
    fn push_checkpoint(&mut self, anchor: &str, on_respawn: &[QuestEffect], fire_step: usize) {
        if let Some(pos) = point_any(self.anchors, anchor) {
            self.checkpoints.push(CheckpointPlan {
                index: self.checkpoints.len(),
                anchor: anchor.to_string(),
                pos,
                on_respawn: on_respawn.to_vec(),
                fire_step,
            });
        }
    }

    fn push_stealth(
        &mut self,
        zones: &[delvewright_dsl::StealthZone],
        on_caught: &[QuestEffect],
        grace_ticks: u32,
        fire_step: usize,
    ) {
        let resolved: Vec<(String, [i32; 3], [u32; 3])> = zones
            .iter()
            .filter_map(|z| {
                point_any(self.anchors, z.anchor.as_str())
                    .map(|p| (z.anchor.as_str().to_string(), p, z.extent))
            })
            .collect();
        if resolved.len() == zones.len() {
            self.stealth.push(StealthBeat {
                index: self.stealth.len() + 1,
                zones: resolved,
                on_caught: on_caught.to_vec(),
                grace_ticks,
                fire_step,
            });
        }
    }

    fn handle(&mut self, eff: &QuestEffect, fire_step: usize) {
        if let Some((anchor, on_respawn)) = eff.set_checkpoint() {
            self.push_checkpoint(anchor.as_str(), on_respawn, fire_step);
        } else if let Some((zones, on_caught, grace)) = eff.begin_stealth() {
            self.push_stealth(zones, on_caught, grace, fire_step);
        }
        // Descend into every nested effect list (`sequence` steps, `on_respawn`,
        // `on_caught`, `on_arrive`): a `set-checkpoint`/`begin-stealth` nested in a
        // `sequence` step is a real checkpoint/beat, fired at the same critical-path
        // step, and must be collected — else its content-ordered index is never
        // registered and `emit_set_checkpoint` silently mis-binds `#cp` to 0.
        for list in eff.nested_effect_lists() {
            for inner in list {
                self.handle(inner, fire_step);
            }
        }
    }
}

/// The total duration of the first `Cutscene` effect in `effects`, if any — the
/// sum over its shots, which is how long the harness must wait out the whole
/// cinematic (a multi-shot cutscene plays back-to-back in one bracket).
fn cutscene_seconds_in<'a>(effects: impl Iterator<Item = &'a QuestEffect>) -> Option<u32> {
    for e in effects {
        if let Some(shots) = e.cutscene_shots() {
            return Some(shots.iter().map(|s| s.resolved_seconds()).sum());
        }
    }
    None
}

/// Whether `obj_id` is the last objective (in `after`-DAG order) of `quest_id`,
/// i.e. its completion is what fires the quest's `on_complete` effects.
fn is_last_objective_of_quest(campaign: &Campaign, quest_id: &str, obj_id: &str) -> bool {
    campaign
        .quests
        .content
        .quests
        .iter()
        .find(|q| q.id.as_str() == quest_id)
        .and_then(|q| objectives_in_order(&q.objectives).last().copied())
        .is_some_and(|last| last.id().as_str() == obj_id)
}

fn point_of(
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    area: &str,
    anchor: &str,
) -> Result<[i32; 3], PlanError> {
    match anchors.get(&(area.to_string(), anchor.to_string())) {
        Some(ResolvedAnchor::Point { pos, .. }) => Ok(*pos),
        Some(ResolvedAnchor::Gate { from, .. }) => Ok(*from),
        None => Err(PlanError::new(
            DW_BUILD,
            format!(
                "anchor `{anchor}` in area `{area}` did not resolve to a world position at build \
                 time — if the campaign references an anchor no bound prefab/pool provides, \
                 `DW0142`/`DW0302` should have named it; reaching here means the resolver and \
                 validator disagree, a compiler bug — stop and escalate"
            ),
        )),
    }
}

/// The stage-5 quest containing an objective id, and that objective.
pub fn objective_quest<'a>(
    campaign: &'a Campaign,
    obj_id: &str,
) -> Option<(&'a str, &'a Objective)> {
    for q in &campaign.quests.content.quests {
        for o in &q.objectives {
            if o.id().as_str() == obj_id {
                return Some((q.id.as_str(), o));
            }
        }
    }
    None
}

/// The `on_objective_complete` effects for an objective, across the campaign.
pub fn objective_effects<'a>(campaign: &'a Campaign, obj_id: &str) -> Vec<&'a QuestEffect> {
    let mut out = Vec::new();
    for q in &campaign.quests.content.quests {
        if let Some(effects) = q
            .on_objective_complete
            .get(&delvewright_dsl::ObjectiveId(obj_id.to_string()))
        {
            out.extend(effects.iter());
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Gate-aware reachability (M2 fix 7, DW0306)
// ---------------------------------------------------------------------------

/// A gate inside a placed piece: the carrying piece index and the local plane
/// (`axis`, `plane`) the barred row sits on. A sealed gate splits its piece into
/// the `local[axis] < plane` half (side 0) and the `>= plane` half (side 1); the
/// only path between them is the gate cut-edge, present once the gate is opened.
struct GateInfo {
    piece: usize,
    axis: usize,
    plane: i32,
}

/// A placed connector socket in world space, for reconstructing which pieces mate.
struct WorldSocket {
    piece: usize,
    connector: usize,
    world: [i32; 3],
    facing: Facing,
}

/// A piece-connectivity graph node: `(piece index, gate side)`. Non-gate pieces
/// are always side 0.
type Node = (usize, u8);

/// Verify every objective anchored in `area_id` is reachable from the area's
/// `spawn` using only gates already opened by earlier objectives in the DAG order
/// ([`DW_GATE_DEADLOCK`]). No-op for areas without gates.
fn check_gate_reachability(
    campaign: &Campaign,
    area_id: &str,
    pieces: &[PiecePlacement],
    registry: &PrefabRegistry,
    severed: Option<&BTreeSet<[i32; 3]>>,
) -> Result<(), PlanError> {
    // Gates carried by a piece in this area.
    let mut gates: BTreeMap<String, GateInfo> = BTreeMap::new();
    for name in collect_open_gate_anchors(campaign) {
        if let Some((pi, meta)) = anchor_piece(pieces, registry, &name)
            && let Some(region) = &meta.region
            && let Some((axis, plane)) = gate_plane(region)
        {
            gates.insert(
                name,
                GateInfo {
                    piece: pi,
                    axis,
                    plane,
                },
            );
        }
    }
    if gates.is_empty() {
        return Ok(());
    }

    // Global critical objective order (quests topo-sorted, objectives by `after`).
    let order = critical_objective_order(campaign);
    // When each gate opens: the earliest order index whose objective/quest opens it.
    let gate_open_at = gate_open_indices(campaign, &order, &gates);

    // Static (gate-independent) adjacency: mated sockets between pieces.
    let sockets = world_sockets(pieces, registry);
    let adj = build_adjacency(&sockets, pieces, registry, &gates, severed);

    // The entry piece, resolved through the same alias list every other consumer
    // uses (`spawn`, then `entry`) — the gate-deadlock proof must start where the
    // player actually starts, and the island tileset spells that anchor `entry`.
    let Some(spawn) = ENTRY_ANCHOR_NAMES
        .iter()
        .find_map(|name| anchor_node(pieces, registry, name, &gates))
    else {
        return Ok(()); // no entry anchor in this area → DW0345 reports it at build
    };

    for (i, step) in order.iter().enumerate() {
        let Some((tarea, tname)) = objective_target(campaign, step.obj, step.area) else {
            continue;
        };
        if tarea != area_id {
            continue;
        }
        let Some(target) = anchor_node(pieces, registry, &tname, &gates) else {
            continue;
        };
        // Gates already open when the player must stand at this objective's anchor:
        // those an earlier objective (index < i) has opened.
        let open: BTreeSet<usize> = gates
            .values()
            .zip(gates.keys())
            .filter(|(_, name)| gate_open_at.get(*name).is_some_and(|&j| j < i))
            .map(|(g, _)| g.piece)
            .collect();
        if !reachable(spawn, target, &adj, &gates, &open) {
            let culprit = gates
                .iter()
                .filter(|(name, _)| gate_open_at.get(*name).is_none_or(|&j| j >= i))
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            // A `rewire-socket sealed` (spec-0017 PR 3) cuts doorway edges out
            // of this graph, so the blockage may be a massed-away passage, not
            // a quest-order mistake — say so.
            let severed_note = if severed.is_some_and(|s| !s.is_empty()) {
                " NOTE: this area's world-edits script seals doorway socket(s) via \
                 `rewire-socket` — those passages are cut from this proof; if the blockage \
                 is one of them, reopen it or leave another route"
            } else {
                ""
            };
            return Err(PlanError::new(
                DW_GATE_DEADLOCK,
                format!(
                    "objective `{}` (anchor `{tname}` in area `{area_id}`) is only reachable \
                     through a gate that no earlier objective opens (sealed gate(s): {culprit}), \
                     so the delve deadlocks. Fix the quest order: add an earlier objective whose \
                     `open-gate` effect opens {culprit} before this objective, or move `{tname}` \
                     to the near side of the gate. Do NOT delete the gate to dodge the check — \
                     that removes intended progression.{severed_note}",
                    step.obj.id()
                ),
            ));
        }
    }
    Ok(())
}

/// One objective in critical order, with its owning quest area.
struct OrderedObj<'a> {
    obj: &'a Objective,
    quest: &'a str,
    area: &'a str,
}

/// Every objective in critical-path order — the branch-coherent playthrough
/// ([`crate::flow::Flow::playthrough`]), i.e. exactly the sequence
/// [`build_critical_path`] exports, so the gate-aware reachability proof
/// (`DW0306`) judges the same walk the bot will.
fn critical_objective_order(campaign: &Campaign) -> Vec<OrderedObj<'_>> {
    let path = crate::flow::Flow::new(campaign).playthrough();
    let stage5: BTreeMap<&str, &Quest> = campaign
        .quests
        .content
        .quests
        .iter()
        .map(|q| (q.id.as_str(), q))
        .collect();
    let quest_area: BTreeMap<&str, &str> = campaign
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| (q.id.as_str(), q.area.as_str()))
        .collect();
    let mut out = Vec::new();
    for st in &path.steps {
        let Some(q) = stage5.get(st.quest.as_str()) else {
            continue;
        };
        let Some(obj) = q
            .objectives
            .iter()
            .find(|o| o.id().as_str() == st.objective)
        else {
            continue;
        };
        out.push(OrderedObj {
            obj,
            quest: q.id.as_str(),
            area: quest_area.get(st.quest.as_str()).copied().unwrap_or(""),
        });
    }
    out
}

/// The `(area, anchor)` a player must stand at to complete `obj`.
fn objective_target(
    campaign: &Campaign,
    obj: &Objective,
    quest_area: &str,
) -> Option<(String, String)> {
    match obj {
        Objective::TalkTo { npc, .. } => {
            let n = campaign
                .npcs
                .content
                .npcs
                .iter()
                .find(|n| n.id.as_str() == npc.as_str())?;
            Some((n.area.as_str().to_string(), n.anchor.as_str().to_string()))
        }
        Objective::ReachAnchor { anchor, .. }
        | Objective::Collect { anchor, .. }
        | Objective::Interact { anchor, .. } => {
            Some((quest_area.to_string(), anchor.as_str().to_string()))
        }
        Objective::Kill { wave, .. } => {
            let w = wave_of(campaign, wave.as_str())?;
            Some((quest_area.to_string(), w.anchor.as_str().to_string()))
        }
    }
}

/// Every anchor named by an `open-gate` effect anywhere in the campaign.
fn collect_open_gate_anchors(campaign: &Campaign) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for q in &campaign.quests.content.quests {
        for effs in q.on_objective_complete.values() {
            for e in effs {
                if let Some(a) = e.open_gate_anchor() {
                    out.insert(a.as_str().to_string());
                }
            }
        }
        for e in &q.on_complete {
            if let Some(a) = e.open_gate_anchor() {
                out.insert(a.as_str().to_string());
            }
        }
    }
    out
}

/// The earliest critical-order index at which each gate opens: the index of the
/// objective whose `on_objective_complete` opens it, or the last objective of a
/// quest whose `on_complete` opens it (min over all openers).
fn gate_open_indices(
    campaign: &Campaign,
    order: &[OrderedObj<'_>],
    gates: &BTreeMap<String, GateInfo>,
) -> BTreeMap<String, usize> {
    let index_of: BTreeMap<&str, usize> = order
        .iter()
        .enumerate()
        .map(|(i, s)| (s.obj.id().as_str(), i))
        .collect();
    let last_obj_index: BTreeMap<&str, usize> = {
        let mut m: BTreeMap<&str, usize> = BTreeMap::new();
        for (i, s) in order.iter().enumerate() {
            m.entry(s.quest)
                .and_modify(|e| *e = (*e).max(i))
                .or_insert(i);
        }
        m
    };
    let mut out: BTreeMap<String, usize> = BTreeMap::new();
    let note = |gate: &str, idx: usize, out: &mut BTreeMap<String, usize>| {
        if gates.contains_key(gate) {
            out.entry(gate.to_string())
                .and_modify(|e| *e = (*e).min(idx))
                .or_insert(idx);
        }
    };
    for q in &campaign.quests.content.quests {
        for (oid, effs) in &q.on_objective_complete {
            for e in effs {
                if let Some(a) = e.open_gate_anchor()
                    && let Some(&idx) = index_of.get(oid.as_str())
                {
                    note(a.as_str(), idx, &mut out);
                }
            }
        }
        for e in &q.on_complete {
            if let Some(a) = e.open_gate_anchor()
                && let Some(&idx) = last_obj_index.get(q.id.as_str())
            {
                note(a.as_str(), idx, &mut out);
            }
        }
    }
    out
}

/// The gate's local dividing plane: the horizontal axis (x or z) the barred row is
/// thin along, plus the coordinate of that plane. `None` if neither horizontal
/// axis is a single cell thick (not a wall-like gate).
fn gate_plane(region: &crate::registry::Region) -> Option<(usize, i32)> {
    let span = |a: usize| (region.from[a] - region.to[a]).abs();
    // Prefer the thinner of the two horizontal axes (a vertical gate wall).
    let (zx, xx) = (span(2), span(0));
    if zx <= xx && zx == 0 {
        Some((2, region.from[2]))
    } else if xx == 0 {
        Some((0, region.from[0]))
    } else {
        None
    }
}

/// The placed piece carrying `anchor_name`, with that anchor's local metadata.
fn anchor_piece<'a>(
    pieces: &[PiecePlacement],
    registry: &'a PrefabRegistry,
    anchor_name: &str,
) -> Option<(usize, &'a AnchorMeta)> {
    pieces.iter().enumerate().find_map(|(i, p)| {
        registry
            .get(&p.prefab_id)
            .and_then(|m| m.anchors.get(anchor_name))
            .map(|am| (i, am))
    })
}

/// The gate side a local point falls on within piece `pi` (0 if the piece has no
/// gate).
fn side_of(pi: usize, local: [i32; 3], gates: &BTreeMap<String, GateInfo>) -> u8 {
    for g in gates.values() {
        if g.piece == pi {
            return u8::from(local[g.axis] >= g.plane);
        }
    }
    0
}

/// The graph node an anchor resolves to: its carrying piece + gate side.
fn anchor_node(
    pieces: &[PiecePlacement],
    registry: &PrefabRegistry,
    anchor_name: &str,
    gates: &BTreeMap<String, GateInfo>,
) -> Option<Node> {
    let (pi, am) = anchor_piece(pieces, registry, anchor_name)?;
    let local = am
        .pos
        .or_else(|| am.region.as_ref().map(|r| r.from))
        .unwrap_or([0, 0, 0]);
    Some((pi, side_of(pi, local, gates)))
}

/// Every connector socket of every placed piece, in world space.
fn world_sockets(pieces: &[PiecePlacement], registry: &PrefabRegistry) -> Vec<WorldSocket> {
    let mut out = Vec::new();
    for (pi, p) in pieces.iter().enumerate() {
        let Some(meta) = registry.get(&p.prefab_id) else {
            continue;
        };
        for (ci, conn) in meta.connectors.iter().enumerate() {
            let Some(f) = Facing::parse(&conn.facing) else {
                continue;
            };
            let t = p.rotation.transform(conn.local_pos);
            out.push(WorldSocket {
                piece: pi,
                connector: ci,
                world: [p.pos[0] + t[0], p.pos[1] + t[1], p.pos[2] + t[2]],
                facing: f.rotate(p.rotation),
            });
        }
    }
    out
}

/// Static adjacency over `(piece, side)` nodes: two mated sockets (child socket one
/// block beyond the parent, facing opposite) link their pieces' sub-nodes.
fn build_adjacency(
    sockets: &[WorldSocket],
    pieces: &[PiecePlacement],
    registry: &PrefabRegistry,
    gates: &BTreeMap<String, GateInfo>,
    severed: Option<&BTreeSet<[i32; 3]>>,
) -> BTreeMap<Node, BTreeSet<Node>> {
    // Local pos of a socket (for gate-side classification).
    let local_pos = |s: &WorldSocket| -> [i32; 3] {
        registry
            .get(&pieces[s.piece].prefab_id)
            .and_then(|m| m.connectors.get(s.connector))
            .map(|c| c.local_pos)
            .unwrap_or([0, 0, 0])
    };
    let mut adj: BTreeMap<Node, BTreeSet<Node>> = BTreeMap::new();
    for a in sockets {
        let a_next = [
            a.world[0] + a.facing.unit()[0],
            a.world[1] + a.facing.unit()[1],
            a.world[2] + a.facing.unit()[2],
        ];
        for b in sockets {
            if a.piece == b.piece {
                continue;
            }
            if b.world == a_next && b.facing == a.facing.opposite() {
                // A doorway severed by `rewire-socket sealed` (spec-0017 PR 3)
                // is walled on both planes — no edge.
                if severed.is_some_and(|s| s.contains(&a.world) || s.contains(&b.world)) {
                    continue;
                }
                let na = (a.piece, side_of(a.piece, local_pos(a), gates));
                let nb = (b.piece, side_of(b.piece, local_pos(b), gates));
                adj.entry(na).or_default().insert(nb);
                adj.entry(nb).or_default().insert(na);
            }
        }
    }
    adj
}

/// BFS reachability from `spawn` to `target` over static edges plus the cut-edge of
/// every gate whose piece is in `open` (its two sides become connected).
fn reachable(
    spawn: Node,
    target: Node,
    adj: &BTreeMap<Node, BTreeSet<Node>>,
    gates: &BTreeMap<String, GateInfo>,
    open: &BTreeSet<usize>,
) -> bool {
    let mut seen: BTreeSet<Node> = BTreeSet::new();
    let mut queue: VecDeque<Node> = VecDeque::new();
    seen.insert(spawn);
    queue.push_back(spawn);
    while let Some(n) = queue.pop_front() {
        if n == target {
            return true;
        }
        if let Some(neis) = adj.get(&n) {
            for &m in neis {
                if seen.insert(m) {
                    queue.push_back(m);
                }
            }
        }
        // Open gate cut-edge on this piece connects its two sides.
        if open.contains(&n.0) && gates.values().any(|g| g.piece == n.0) {
            let other = (n.0, 1 - n.1);
            if seen.insert(other) {
                queue.push_back(other);
            }
        }
    }
    seen.contains(&target)
}

/// Which quests are triggered by `campaign-start`.
pub fn campaign_start_quests(campaign: &Campaign) -> Vec<&str> {
    campaign
        .quests
        .content
        .quests
        .iter()
        .filter(|q| matches!(q.trigger, Trigger::CampaignStart))
        .map(|q| q.id.as_str())
        .collect()
}
