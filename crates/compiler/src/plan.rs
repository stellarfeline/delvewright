//! Resolve a validated [`Campaign`] into a placement + naming model that
//! emission and `critical-path.json` both consume.
//!
//! ## Coordinate scheme (deterministic)
//!
//! Each stage-1 area is placed at origin `[index * AREA_SPACING, BASE_Y, 0]`
//! (M1 has one area → `[0, 64, 0]`). A prefab's local anchor position resolves to
//! `origin + local`. All coordinates are integers; no randomness is used in v0.
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
    Campaign, DialogueEffect, DialogueId, Npc, NpcDialogue, Objective, Quest, QuestEffect, Trigger,
};

use crate::registry::{AnchorMeta, PrefabRegistry};
use crate::solver::{self, Facing, Rotation, SealFill, Splitmix64};

/// World-space distance between successive area origins.
pub const AREA_SPACING: i32 = 256;
/// The Y of every area origin (structures carry their own floor at local y=0).
pub const BASE_Y: i32 = 64;

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
}

/// A critical-path step (mirrors the amended `critical-path.json` shape).
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
        /// NPC id.
        npc_id: String,
        /// Absolute NPC position.
        pos: [i32; 3],
        /// The chat command the bot sends.
        command: String,
    },
    /// Walk to within `radius` of `pos`.
    Reach {
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
        for (i, area) in campaign.world.content.areas.iter().enumerate() {
            let area_id = area.id.as_str().to_string();
            let origin = [i as i32 * AREA_SPACING, BASE_Y, 0];

            let placement = if let Some(prefab) = &area.prefab {
                // Single-prefab area (the M1 degenerate assembly): one piece at
                // the origin, rotation none, no sockets to seal.
                let prefab_id = prefab.as_str().to_string();
                let meta = prefabs.get(&prefab_id).ok_or_else(|| {
                    PlanError::new(
                        DW_BUILD,
                        format!(
                            "area `{area_id}` binds prefab `{prefab_id}` but no matching prefab \
                             metadata was found in the prefabs dir"
                        ),
                    )
                })?;
                for (name, am) in &meta.anchors {
                    anchors.insert((area_id.clone(), name.clone()), resolve_anchor(origin, am));
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
                let layout = solver::solve_area(
                    prefabs,
                    &pool_id,
                    &required,
                    pmin,
                    pmax,
                    origin,
                    &mut stream,
                )
                .map_err(|e| PlanError::new(e.code, e.message))?;

                let mut pieces = Vec::new();
                for placed in &layout.pieces {
                    let meta = prefabs.get(&placed.prefab_id).ok_or_else(|| {
                        PlanError::new(
                            DW_BUILD,
                            format!("solver placed unknown prefab `{}`", placed.prefab_id),
                        )
                    })?;
                    // Transform this piece's anchors to world space. Each required
                    // anchor is carried by exactly one placed piece (fillers are
                    // anchorless connectors), so names do not collide.
                    for (name, am) in &meta.anchors {
                        anchors
                            .entry((area_id.clone(), name.clone()))
                            .or_insert_with(|| resolve_piece_anchor(placed, am));
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
                    format!("area `{area_id}` binds neither `prefab` nor `prefab_pool`"),
                ));
            };
            areas.push(placement);
        }

        // ---- gate-aware reachability (M2 fix 7, DW0306) ----
        // With the layout solved, verify no objective's anchor is sealed behind a
        // gate that only a later objective opens (an unwinnable deadlock the anchor
        // resolver alone cannot see).
        for area in &areas {
            check_gate_reachability(campaign, &area.area_id, &area.pieces, prefabs)?;
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

        // ---- critical path + inter-area transport ----
        let cp = build_critical_path(campaign, &anchors, &npcs)?;

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
        })
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
    if let Some((a, _)) = e.set_block() {
        set.insert(a.as_str().to_string());
    }
    if let Some((_, a)) = e.move_npc() {
        set.insert(a.as_str().to_string());
    }
    if let QuestEffect::Cutscene { path, .. } = e {
        for w in path {
            set.insert(w.anchor.as_str().to_string());
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
            for e in &opt.effects {
                match e {
                    DialogueEffect::CompleteObjective { objective } => {
                        completes.push(objective.as_str().to_string());
                    }
                    DialogueEffect::SetFlag { flag } => {
                        sets_flags.push(flag.as_str().to_string());
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
}

/// Build the critical path: select first class, then each critical objective in
/// topological order (quests by `depends_on`, objectives by `after`), then assert
/// campaign completion. Also returns the inter-area transport map (when
/// consecutive objectives sit in different areas) and, per step, the DSL v0.4
/// harness hints: `sneak` (a `stealth` objective) and `cutscene_seconds` (a step
/// whose completion triggers a `QuestEffect::Cutscene`).
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

    // Quest order: finale's depends_on closure, topologically sorted.
    let quest_order = finale_quest_order(campaign)?;
    let stage5: BTreeMap<&str, &_> = campaign
        .quests
        .content
        .quests
        .iter()
        .map(|q| (q.id.as_str(), q))
        .collect();

    for qid in &quest_order {
        let Some(quest) = stage5.get(qid.as_str()) else {
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
        for obj in objectives_in_order(&quest.objectives) {
            match obj {
                Objective::TalkTo { id, npc, .. } => {
                    let npc_plan =
                        npcs.iter()
                            .find(|n| n.npc_id == npc.as_str())
                            .ok_or_else(|| {
                                PlanError::new(
                                    DW_BUILD,
                                    format!("talk-to references unknown npc `{npc}`"),
                                )
                            })?;
                    let opt = npc_plan
                        .options
                        .iter()
                        .find(|o| o.completes.iter().any(|c| c == id.as_str()))
                        .ok_or_else(|| {
                            PlanError::new(DW_BUILD, format!(
                                "objective `{id}` has no dialogue option completing it (analyze should have caught this)"
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
                            format!("kill objective references unknown wave `{wave}`"),
                        )
                    })?;
                    let pos = point_of(anchors, area, w.anchor.as_str())?;
                    steps.push(Step::Kill {
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

    Ok(CriticalPath {
        steps,
        transport,
        transport_by_step,
        sneak_by_step,
        cutscene_by_step,
    })
}

/// The `seconds` of the first `Cutscene` effect in `effects`, if any.
fn cutscene_seconds_in<'a>(effects: impl Iterator<Item = &'a QuestEffect>) -> Option<u32> {
    for e in effects {
        if let QuestEffect::Cutscene { seconds, .. } = e {
            return Some(*seconds);
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
            format!("anchor `{anchor}` in area `{area}` did not resolve"),
        )),
    }
}

/// Topologically order the finale quest and its transitive `depends_on`.
fn finale_quest_order(campaign: &Campaign) -> Result<Vec<String>, PlanError> {
    let plan = &campaign.quest_plan.content;
    let deps: BTreeMap<&str, &Vec<_>> = plan
        .quests
        .iter()
        .map(|q| (q.id.as_str(), &q.depends_on))
        .collect();

    // Closure over finale's dependencies.
    let mut needed: BTreeSet<&str> = BTreeSet::new();
    let mut stack = vec![plan.finale.as_str()];
    while let Some(q) = stack.pop() {
        if needed.insert(q)
            && let Some(ds) = deps.get(q)
        {
            for d in ds.iter() {
                stack.push(d.as_str());
            }
        }
    }

    // Kahn topological sort restricted to `needed`.
    let mut indeg: BTreeMap<&str, usize> = needed.iter().map(|q| (*q, 0)).collect();
    for q in &needed {
        if let Some(ds) = deps.get(q) {
            for d in ds.iter() {
                if needed.contains(d.as_str()) {
                    *indeg.get_mut(q).unwrap() += 1;
                }
            }
        }
    }
    let mut queue: VecDeque<&str> = indeg
        .iter()
        .filter(|&(_, &d)| d == 0)
        .map(|(q, _)| *q)
        .collect();
    let mut order = Vec::new();
    while let Some(q) = queue.pop_front() {
        order.push(q.to_string());
        // Decrement dependents.
        for r in &needed {
            if let Some(ds) = deps.get(r)
                && ds.iter().any(|d| d.as_str() == q)
            {
                let e = indeg.get_mut(*r).unwrap();
                *e -= 1;
                if *e == 0 {
                    queue.push_back(r);
                }
            }
        }
    }
    if order.len() != needed.len() {
        return Err(PlanError::new(
            DW_BUILD,
            "quest dependency cycle in critical path",
        ));
    }
    Ok(order)
}

/// Order a quest's objectives by their intra-quest `after` DAG (Kahn).
fn objectives_in_order(objectives: &[Objective]) -> Vec<&Objective> {
    let ids: Vec<&str> = objectives.iter().map(|o| o.id().as_str()).collect();
    let mut indeg: BTreeMap<&str, usize> = ids.iter().map(|i| (*i, 0)).collect();
    for o in objectives {
        for a in o.after() {
            if indeg.contains_key(a.as_str()) {
                *indeg.get_mut(o.id().as_str()).unwrap() += 1;
            }
        }
    }
    let mut queue: VecDeque<&str> = ids.iter().filter(|i| indeg[**i] == 0).copied().collect();
    let by_id: BTreeMap<&str, &Objective> =
        objectives.iter().map(|o| (o.id().as_str(), o)).collect();
    let mut order = Vec::new();
    while let Some(id) = queue.pop_front() {
        order.push(by_id[id]);
        for o in objectives {
            if o.after().iter().any(|a| a.as_str() == id) {
                let e = indeg.get_mut(o.id().as_str()).unwrap();
                *e -= 1;
                if *e == 0 {
                    queue.push_back(o.id().as_str());
                }
            }
        }
    }
    // Fallback: if a cycle slipped through (validation should prevent it), keep
    // declared order for the remainder.
    if order.len() != objectives.len() {
        return objectives.iter().collect();
    }
    order
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
    let adj = build_adjacency(&sockets, pieces, registry, &gates);

    let Some(spawn) = anchor_node(pieces, registry, "spawn", &gates) else {
        return Ok(()); // no spawn anchor resolved → cannot reason; leave to other checks
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
            return Err(PlanError::new(
                DW_GATE_DEADLOCK,
                format!(
                    "objective `{}` (anchor `{tname}` in area `{area_id}`) is only reachable \
                     through a gate that no earlier objective opens (sealed gate(s): {culprit}); \
                     the delve deadlocks — an anchor cannot sit beyond the gate that a later \
                     objective opens",
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

/// Every objective in critical-path order (finale-dependency topo order, then each
/// quest's objectives by their `after` DAG) — the order the player completes them.
fn critical_objective_order(campaign: &Campaign) -> Vec<OrderedObj<'_>> {
    let quest_order = finale_quest_order(campaign).unwrap_or_default();
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
    for qid in &quest_order {
        let Some(q) = stage5.get(qid.as_str()) else {
            continue;
        };
        let area = quest_area.get(qid.as_str()).copied().unwrap_or("");
        for obj in objectives_in_order(&q.objectives) {
            out.push(OrderedObj {
                obj,
                quest: q.id.as_str(),
                area,
            });
        }
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
