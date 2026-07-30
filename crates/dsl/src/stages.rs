//! serde types for the five stage `content` payloads (spec-0001).
//!
//! Every struct is `deny_unknown_fields`. Reserved enum values and the reserved
//! `prefab_pool` field parse successfully but are rejected by validation
//! ([`crate::validate`]) with code `DW0141`.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{AnchorId, AreaId, ClassId, DialogueId, NpcId, ObjectiveId, PrefabId, QuestId};

// ---------------------------------------------------------------------------
// Stage 1 — world
// ---------------------------------------------------------------------------

/// Stage 1 payload: setting, seed and the areas that make up the delve.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorldContent {
    /// Player-facing delve title.
    pub title: String,
    /// One-line thematic description.
    pub theme: String,
    /// Short narrative premise.
    pub premise: String,
    /// The single downstream randomness source (ADR-0006).
    pub seed: u64,
    /// Informational pacing target in minutes (v0: not enforced).
    pub target_minutes: u32,
    /// 1..N areas; each binds exactly one prefab in v0.
    pub areas: Vec<Area>,
}

/// One area of the world, bound to a prefab.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Area {
    /// Unique area id.
    pub id: AreaId,
    /// Player-facing area name.
    pub name: String,
    /// The single prefab bound to this area (v0).
    pub prefab: PrefabId,
    /// Reserved (M2): jigsaw prefab pool. Rejected in v0 (`DW0141`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefab_pool: Option<PrefabPool>,
}

/// Reserved M2 jigsaw prefab pool (rejected in v0).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PrefabPool {
    /// Candidate prefabs for jigsaw assembly.
    pub pools: Vec<PrefabId>,
}

// ---------------------------------------------------------------------------
// Stage 2 — npcs
// ---------------------------------------------------------------------------

/// Stage 2 payload: the campaign's NPCs and their dialogue graphs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NpcsContent {
    /// All NPCs in the campaign.
    pub npcs: Vec<Npc>,
}

/// A stationary NPC bound to an area anchor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Npc {
    /// Unique NPC id.
    pub id: NpcId,
    /// Player-facing name.
    pub name: String,
    /// NPC role.
    pub role: Role,
    /// The area this NPC stands in (stage-1 ref).
    pub area: AreaId,
    /// The prefab anchor this NPC stands on.
    pub anchor: AnchorId,
    /// The vanilla entity to re-dress, e.g. `minecraft:villager`.
    pub base_entity: String,
    /// The NPC's dialogue graph.
    pub dialogue: Dialogue,
}

/// NPC role. `vendor` and `boss` are reserved (rejected in v0, `DW0141`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    /// Gives and advances quests.
    QuestGiver,
    /// Flavor only.
    Flavor,
    /// Reserved (M2).
    Vendor,
    /// Reserved (M2).
    Boss,
}

impl Role {
    /// The reserved value name if this role is not implemented in v0.
    pub fn reserved(self) -> Option<&'static str> {
        match self {
            Role::Vendor => Some("vendor"),
            Role::Boss => Some("boss"),
            Role::QuestGiver | Role::Flavor => None,
        }
    }
}

/// A dialogue graph: a root node plus a set of nodes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Dialogue {
    /// The entry node id; every node must be reachable from it.
    pub root: DialogueId,
    /// The dialogue nodes.
    pub nodes: Vec<DialogueNode>,
}

/// One dialogue node: text plus branching options.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DialogueNode {
    /// Node id (unique within this NPC's dialogue).
    pub id: DialogueId,
    /// The line the NPC speaks.
    pub text: String,
    /// Branching options; empty closes the dialog.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<DialogueOption>,
}

/// One selectable dialogue option.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DialogueOption {
    /// Button label.
    pub label: String,
    /// Next node; omitted closes the dialog.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<DialogueId>,
    /// Effects fired when this option is chosen.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<DialogueEffect>,
}

/// Effect fired by a dialogue option. v0: `complete-objective` only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum DialogueEffect {
    /// Marks a stage-5 objective complete.
    CompleteObjective {
        /// The objective to complete (resolved at the stage-5 boundary).
        objective: ObjectiveId,
    },
}

// ---------------------------------------------------------------------------
// Stage 3 — classes
// ---------------------------------------------------------------------------

/// Stage 3 payload: 1..4 selectable classes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClassesContent {
    /// The selectable classes.
    pub classes: Vec<Class>,
}

/// A player class with a starting kit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Class {
    /// Unique class id.
    pub id: ClassId,
    /// Player-facing name.
    pub name: String,
    /// Selection-screen blurb.
    pub blurb: String,
    /// Granted items.
    pub kit: Vec<KitItem>,
}

/// One item in a class kit.
///
/// Note: `lore`, `enchantments` and `attributes` are reserved for M2/M3
/// (spec-0001). They are intentionally *not* defined as fields in v0, so a
/// document using them is rejected as an unknown field (`DW0100`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KitItem {
    /// Vanilla item id, validated against the pinned 1.21.11 registry.
    pub item: String,
    /// Stack count.
    pub count: u32,
    /// Optional display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

// ---------------------------------------------------------------------------
// Stage 4 — quest-plan
// ---------------------------------------------------------------------------

/// Stage 4 payload: the quest dependency plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QuestPlanContent {
    /// Planned quests (expanded in stage 5).
    pub quests: Vec<PlannedQuest>,
    /// The quest whose completion ends the campaign.
    pub finale: QuestId,
}

/// One planned quest (dependency-graph node).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlannedQuest {
    /// Unique quest id.
    pub id: QuestId,
    /// Human-readable goal.
    pub goal: String,
    /// Area this quest takes place in (stage-1 ref).
    pub area: AreaId,
    /// NPCs involved (stage-2 refs).
    pub npcs: Vec<NpcId>,
    /// Prerequisite quests; edges must form a DAG.
    pub depends_on: Vec<QuestId>,
    /// v0 requires `true`; optional quests are reserved (`DW0133`).
    pub mandatory: bool,
    /// Act number (informational).
    pub act: u32,
}

// ---------------------------------------------------------------------------
// Stage 5 — quests
// ---------------------------------------------------------------------------

/// Stage 5 payload: quest expansions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QuestsContent {
    /// The expanded quests (1:1 with stage 4).
    pub quests: Vec<Quest>,
}

/// One expanded quest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Quest {
    /// Quest id (matches a stage-4 planned quest).
    pub id: QuestId,
    /// What starts the quest.
    pub trigger: Trigger,
    /// Ordered objectives (intra-quest DAG via `after`).
    pub objectives: Vec<Objective>,
    /// Effects fired when a given objective completes.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub on_objective_complete: BTreeMap<ObjectiveId, Vec<QuestEffect>>,
    /// Effects fired when the whole quest completes.
    pub on_complete: Vec<QuestEffect>,
}

/// What triggers a quest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Trigger {
    /// Fires when the campaign starts.
    CampaignStart,
    /// Fires when another quest completes.
    QuestComplete {
        /// The prerequisite quest.
        quest: QuestId,
    },
}

/// A quest objective.
///
/// `kill`, `collect` and `interact` are reserved (rejected in v0, `DW0141`);
/// they carry only the common fields so they still parse.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Objective {
    /// Completed by a dialogue option's `complete-objective` effect.
    TalkTo {
        /// Objective id.
        id: ObjectiveId,
        /// The NPC to talk to.
        npc: NpcId,
        /// Prerequisite objectives (intra-quest ordering).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        after: Vec<ObjectiveId>,
    },
    /// Completed by reaching an anchor once prerequisites are met.
    ReachAnchor {
        /// Objective id.
        id: ObjectiveId,
        /// The anchor to reach.
        anchor: AnchorId,
        /// Completion radius (blocks).
        radius: u32,
        /// Prerequisite objectives (intra-quest ordering).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        after: Vec<ObjectiveId>,
    },
    /// Reserved (M2).
    Kill {
        /// Objective id.
        id: ObjectiveId,
        /// Prerequisite objectives.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        after: Vec<ObjectiveId>,
    },
    /// Reserved (M2).
    Collect {
        /// Objective id.
        id: ObjectiveId,
        /// Prerequisite objectives.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        after: Vec<ObjectiveId>,
    },
    /// Reserved (M2).
    Interact {
        /// Objective id.
        id: ObjectiveId,
        /// Prerequisite objectives.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        after: Vec<ObjectiveId>,
    },
}

impl Objective {
    /// This objective's id.
    pub fn id(&self) -> &ObjectiveId {
        match self {
            Objective::TalkTo { id, .. }
            | Objective::ReachAnchor { id, .. }
            | Objective::Kill { id, .. }
            | Objective::Collect { id, .. }
            | Objective::Interact { id, .. } => id,
        }
    }

    /// This objective's prerequisites.
    pub fn after(&self) -> &[ObjectiveId] {
        match self {
            Objective::TalkTo { after, .. }
            | Objective::ReachAnchor { after, .. }
            | Objective::Kill { after, .. }
            | Objective::Collect { after, .. }
            | Objective::Interact { after, .. } => after,
        }
    }

    /// The kebab type tag.
    pub fn kind(&self) -> &'static str {
        match self {
            Objective::TalkTo { .. } => "talk-to",
            Objective::ReachAnchor { .. } => "reach-anchor",
            Objective::Kill { .. } => "kill",
            Objective::Collect { .. } => "collect",
            Objective::Interact { .. } => "interact",
        }
    }

    /// The reserved type name if this objective type is not implemented in v0.
    pub fn reserved(&self) -> Option<&'static str> {
        match self {
            Objective::Kill { .. } => Some("kill"),
            Objective::Collect { .. } => Some("collect"),
            Objective::Interact { .. } => Some("interact"),
            Objective::TalkTo { .. } | Objective::ReachAnchor { .. } => None,
        }
    }
}

/// An effect fired by quest progress.
///
/// `give-item`, `set-flag` and `spawn-wave` are reserved (rejected in v0,
/// `DW0141`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum QuestEffect {
    /// Opens a prefab-declared gate (one-way).
    OpenGate {
        /// The gate anchor to open.
        anchor: AnchorId,
    },
    /// Marks the campaign complete (final advancement + credits).
    CampaignComplete,
    /// Reserved (M2).
    GiveItem,
    /// Reserved (M2).
    SetFlag,
    /// Reserved (M2).
    SpawnWave,
}

impl QuestEffect {
    /// The gate anchor if this is `open-gate`.
    pub fn open_gate_anchor(&self) -> Option<&AnchorId> {
        match self {
            QuestEffect::OpenGate { anchor } => Some(anchor),
            _ => None,
        }
    }

    /// The reserved effect name if this effect is not implemented in v0.
    pub fn reserved(&self) -> Option<&'static str> {
        match self {
            QuestEffect::GiveItem => Some("give-item"),
            QuestEffect::SetFlag => Some("set-flag"),
            QuestEffect::SpawnWave => Some("spawn-wave"),
            QuestEffect::OpenGate { .. } | QuestEffect::CampaignComplete => None,
        }
    }
}
