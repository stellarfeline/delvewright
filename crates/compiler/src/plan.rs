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
    Campaign, DialogueEffect, DialogueId, Npc, NpcDialogue, Objective, QuestEffect, Trigger,
};

use crate::registry::{AnchorMeta, PrefabRegistry};
use crate::solver::{self, Rotation, SealFill, Splitmix64};

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
        let (critical_path, transport) = build_critical_path(campaign, &anchors, &npcs)?;

        Ok(Self {
            campaign,
            namespace,
            seed,
            areas,
            anchors,
            classes,
            npcs,
            critical_path,
            transport,
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
            if let Objective::ReachAnchor { anchor, .. } = o {
                set.insert(anchor.as_str().to_string());
            }
        }
        for effs in q.on_objective_complete.values() {
            for e in effs {
                if let Some(a) = e.open_gate_anchor() {
                    set.insert(a.as_str().to_string());
                }
            }
        }
        for e in &q.on_complete {
            if let Some(a) = e.open_gate_anchor() {
                set.insert(a.as_str().to_string());
            }
        }
    }
    set.into_iter().collect()
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
            let completes = opt
                .effects
                .iter()
                .map(|e| {
                    let DialogueEffect::CompleteObjective { objective } = e;
                    objective.as_str().to_string()
                })
                .collect();
            options.push(OptionPlan {
                n,
                node_id: node.id.as_str().to_string(),
                label: opt.label.clone(),
                next: opt
                    .next
                    .as_ref()
                    .map(|d: &DialogueId| d.as_str().to_string()),
                completes,
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

/// Build the critical path: select first class, then each critical objective in
/// topological order (quests by `depends_on`, objectives by `after`), then assert
/// campaign completion. Also returns the inter-area transport map: when
/// consecutive objectives sit in different areas, completing the earlier one
/// teleports the player to the later area's entry spawn.
fn build_critical_path(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    npcs: &[NpcPlan],
) -> Result<(Vec<Step>, TransportMap), PlanError> {
    let mut steps = Vec::new();
    // (objective id, physical area) in critical-path order, for transport.
    let mut obj_areas: Vec<(String, String)> = Vec::new();

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
                    obj_areas.push((id.as_str().to_string(), npc_area.to_string()));
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
                    obj_areas.push((id.as_str().to_string(), area.to_string()));
                }
                _ => {}
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
    for pair in obj_areas.windows(2) {
        let (prev_id, prev_area) = &pair[0];
        let (_, next_area) = &pair[1];
        if prev_area != next_area
            && let Some(ResolvedAnchor::Point { pos, .. }) =
                anchors.get(&(next_area.clone(), "spawn".to_string()))
        {
            transport.insert(prev_id.clone(), *pos);
        }
    }

    Ok((steps, transport))
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
