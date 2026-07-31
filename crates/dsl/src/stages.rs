//! serde types for the six stage `content` payloads (spec-0001 v0.2).
//!
//! Every struct is `deny_unknown_fields`. Reserved enum values (npc `role`,
//! objective/effect types) parse successfully but are rejected by validation
//! ([`crate::validate`]) with code `DW0141`.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{
    AnchorId, AreaId, ClassId, DialogueId, FlagId, NpcId, ObjectiveId, PoolId, PrefabId, QuestId,
    TriggerId, WaveId,
};

/// serde default helper: `true` (used by DSL v0.4 `trigger.once`).
fn default_true() -> bool {
    true
}

/// serde `skip_serializing_if` helper: skip a `false` bool (DSL v0.4
/// `objective.stealth`), keeping older campaigns byte-identical.
fn is_false(b: &bool) -> bool {
    !*b
}

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
    /// 1..N areas; each binds exactly one of `prefab` / `prefab_pool`.
    pub areas: Vec<Area>,
    /// Additional author-declared translation languages (BCP-47-style codes, e.g.
    /// `["zh-cn"]`). English (`en`) is implicit, always canonical, and is **never**
    /// listed here (spec-0001 i18n addendum). Absent or empty = English-only. Every
    /// declared language must ship a fully-covering `l10n/<code>.json` sidecar
    /// (`DW0180`/`DW0181`). Stage docs themselves stay pure English.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<String>,
}

/// One area of the world, bound to a single prefab or a jigsaw prefab pool.
///
/// An area binds **exactly one of** `prefab` (single piece) or `prefab_pool`
/// (+ `pieces`, jigsaw multi-piece assembly, ADR-0004). The exclusivity and
/// pool-existence rules are enforced by validation (`DW0160` / `DW0161`); the
/// full jigsaw layout semantics land in M2 task #9 (spec-0002).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Area {
    /// Unique area id.
    pub id: AreaId,
    /// Player-facing area name.
    pub name: String,
    /// The single prefab bound to this area (mutually exclusive with
    /// `prefab_pool`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefab: Option<PrefabId>,
    /// The jigsaw prefab pool bound to this area (mutually exclusive with
    /// `prefab`); requires `pieces`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefab_pool: Option<PoolId>,
    /// Jigsaw piece-count bounds (only with `prefab_pool`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pieces: Option<Pieces>,
}

/// Inclusive piece-count bounds for a jigsaw `prefab_pool` area.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Pieces {
    /// Minimum number of pieces to assemble.
    pub min: u32,
    /// Maximum number of pieces to assemble.
    pub max: u32,
}

// ---------------------------------------------------------------------------
// Stage 2 — npcs
// ---------------------------------------------------------------------------

/// Stage 2 payload: the campaign's NPCs (casting sheets).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NpcsContent {
    /// All NPCs in the campaign.
    pub npcs: Vec<Npc>,
}

/// A stationary NPC bound to an area anchor (a casting sheet, spec-0001 v0.2).
///
/// Stage 2 carries **no dialogue** — the structured [`Persona`] is the character
/// contract the stage-6 `dialogue` tree must honor.
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
    /// The structured persona (character contract for stage 6).
    pub persona: Persona,
    /// Optional player-model skin (DSL v0.4, spec-0008 §6 / spec-0009). When set,
    /// the compiler emits a `minecraft:mannequin` body carrying this skin profile
    /// instead of re-dressing `base_entity`; the interaction hitbox is unchanged.
    /// Non-skinned NPCs are byte-identical to v0.3.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skin: Option<NpcSkin>,
}

/// A mannequin NPC's player-model skin (DSL v0.4). The skin PNG ships in the
/// per-delve resource pack at `assets/delvewright/textures/npc/<texture_id>.png`
/// (sourced from the campaign dir's `skins/<texture_id>.png`); the mannequin's
/// `profile.texture` resolves to `delvewright:npc/<texture_id>`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NpcSkin {
    /// Skin id: the PNG basename under `skins/` and the resource-pack texture
    /// path segment (a bare kebab token; validated by `DW0190`).
    pub texture_id: String,
    /// Player model. **Required** (spec-0009): an omitted model renders slim, so
    /// a wide skin on a slim model is distorted — the compiler always emits it.
    pub model: SkinModel,
}

/// Player-model shape for a mannequin skin (`wide` = classic/Steve, `slim` =
/// Alex). Emitted verbatim into the mannequin `profile.model`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SkinModel {
    /// Classic 4-pixel arms (Steve).
    Wide,
    /// Slim 3-pixel arms (Alex).
    Slim,
}

impl SkinModel {
    /// The vanilla `profile.model` token.
    pub fn token(self) -> &'static str {
        match self {
            SkinModel::Wide => "wide",
            SkinModel::Slim => "slim",
        }
    }
}

/// A structured casting sheet (owner decision 2026-07-30). Structure lives in the
/// keys; every value is free text. `archetype`, `speech_style` and `motivation`
/// are required; the rest are optional.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Persona {
    /// One-line character archetype (required).
    pub archetype: String,
    /// How the NPC speaks — register, tics, formality (required).
    pub speech_style: String,
    /// Emotional bearing toward the player (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub demeanor: Option<String>,
    /// What the NPC wants (required).
    pub motivation: String,
    /// Something the NPC hides (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
    /// Backstory colour (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backstory: Option<String>,
    /// Attitudes toward other same-stage NPCs (optional; refs validated).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relationships: Vec<Relationship>,
}

/// One persona relationship: an attitude toward another same-stage NPC.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Relationship {
    /// The other NPC (stage-2 ref, validated within stage 2).
    pub npc: NpcId,
    /// Free-text attitude toward that NPC.
    pub attitude: String,
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

// ---------------------------------------------------------------------------
// Stage 6 — dialogue
// ---------------------------------------------------------------------------

/// Stage 6 payload: one dialogue tree per stage-2 NPC (spec-0001 v0.2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DialogueContent {
    /// One tree per NPC (1:1 with stage 2, both directions).
    pub dialogues: Vec<NpcDialogue>,
}

impl DialogueContent {
    /// The dialogue tree for an NPC id, if present.
    pub fn tree_for(&self, npc: &str) -> Option<&NpcDialogue> {
        self.dialogues.iter().find(|t| t.npc.as_str() == npc)
    }
}

/// One NPC's dialogue tree: a root node plus a set of nodes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NpcDialogue {
    /// The NPC this tree belongs to (stage-2 ref).
    pub npc: NpcId,
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
    /// Flags that must be set for this option to be shown (DSL v0.4). Mirrors an
    /// objective's `requires_flags`: an ungated option (empty) is unchanged; a
    /// gated option is hidden until every referenced flag has been set by a
    /// `set-flag` effect (quest or dialogue). Validation guarantees a flag-gated
    /// option cannot make a critical-path node unreachable (`DW0191`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_flags: Vec<FlagId>,
    /// Effects fired when this option is chosen.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<DialogueEffect>,
}

/// Effect fired by a dialogue option. `complete-objective` (v0.2) and, from DSL
/// v0.4, `set-flag` (mirrors the quest effect — sets a campaign flag from a
/// dialogue choice, enabling flag-gated options/objectives/triggers).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum DialogueEffect {
    /// Marks a stage-5 objective complete.
    CompleteObjective {
        /// The objective to complete (resolved at the stage-5 boundary).
        objective: ObjectiveId,
    },
    /// Sets a campaign flag (DSL v0.4), mirroring [`QuestEffect::SetFlag`].
    SetFlag {
        /// The flag to set.
        flag: FlagId,
    },
}

impl DialogueEffect {
    /// The `set-flag` flag id if this is a v0.4 `set-flag` dialogue effect.
    pub fn set_flag(&self) -> Option<&FlagId> {
        match self {
            DialogueEffect::SetFlag { flag } => Some(flag),
            DialogueEffect::CompleteObjective { .. } => None,
        }
    }

    /// The v0.4 effect name if this dialogue effect is one introduced in DSL v0.4
    /// (`set-flag`). Reserved (`DW0141`) in an earlier campaign.
    pub fn v04_effect(&self) -> Option<&'static str> {
        match self {
            DialogueEffect::SetFlag { .. } => Some("set-flag"),
            DialogueEffect::CompleteObjective { .. } => None,
        }
    }
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
    /// Combat waves (DSL v0.3). Each wave is spawned by a `spawn-wave` effect and
    /// slain to complete a `kill` objective. Empty/absent in v0.2 campaigns.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub waves: Vec<Wave>,
    /// Environment triggers (DSL v0.4, spec-0008 §7): "the world answers". Each
    /// watches an anchor for a strike / use / approach event and fires a bundle
    /// of [`QuestEffect`]s. Empty/absent in v0.2/v0.3 campaigns.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub triggers: Vec<EnvTrigger>,
}

/// A stage-5 environment trigger (DSL v0.4). Emission uses vanilla-intended
/// primitives only (spec-0008 §7): `strike`/`use` read a `minecraft:interaction`
/// entity's attack/interaction records; `approach` is a `distance` selector on
/// the tick. Look-at / break-attempt detection is excluded on principle (no
/// vanilla primitive).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EnvTrigger {
    /// Unique trigger id (`trigger/<kebab>`).
    pub id: TriggerId,
    /// The anchor this trigger watches.
    pub at: AnchorId,
    /// The event that fires it.
    pub on: TriggerOn,
    /// Flags that must be set before the trigger can fire (DSL v0.4).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_flags: Vec<FlagId>,
    /// Fire at most once (default `true`, mirroring objective completion). Set
    /// `false` to allow re-firing every time the condition is met.
    #[serde(default = "default_true")]
    pub once: bool,
    /// Effects fired when the trigger matches.
    pub effects: Vec<QuestEffect>,
}

/// The event an [`EnvTrigger`] watches (DSL v0.4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "on", rename_all = "kebab-case", deny_unknown_fields)]
pub enum TriggerOn {
    /// The player attacks (left-clicks) the interaction entity at the anchor.
    Strike,
    /// The player uses (right-clicks) the interaction entity at the anchor.
    Use,
    /// The player comes within `range` blocks of the anchor.
    Approach {
        /// Approach radius (blocks).
        range: u32,
    },
}

impl TriggerOn {
    /// The kebab tag (`strike` / `use` / `approach`).
    pub fn kind(&self) -> &'static str {
        match self {
            TriggerOn::Strike => "strike",
            TriggerOn::Use => "use",
            TriggerOn::Approach { .. } => "approach",
        }
    }
}

/// A combat wave (DSL v0.3): a bundle of mobs spawned at an anchor and slain to
/// complete a `kill` objective. Emission (spec-0002): a `spawn-wave` effect
/// summons the mobs tagged `dw_wave_<id>` (AI enabled — they fight); a
/// `player_killed_entity` advancement per tag decrements a scoreboard countdown,
/// and the `kill` objective completes when the count reaches zero.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Wave {
    /// Unique wave id.
    pub id: WaveId,
    /// The anchor the wave's mobs spawn at.
    pub anchor: AnchorId,
    /// The mobs that make up the wave (1..N).
    pub mobs: Vec<WaveMob>,
}

/// One mob stack in a [`Wave`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WaveMob {
    /// Vanilla entity id, validated against the pinned 1.21.11 registry.
    pub entity: String,
    /// How many to spawn.
    pub count: u32,
    /// Optional custom name (shown above the mob).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Optional attribute overrides (DSL v0.4), emitted as 1.21.11 attribute
    /// components. Enables e.g. a weakened live warden as a survivable stealth
    /// threat. Omitted = vanilla defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attributes: Option<MobAttributes>,
    /// Optional permanent, ambient status effects (DSL v0.4).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<MobEffect>,
}

/// Attribute overrides for a wave mob (DSL v0.4). Each field maps to a 1.21.11
/// `minecraft:` attribute component; an unset field keeps the vanilla base value.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MobAttributes {
    /// `minecraft:max_health` base value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_health: Option<f64>,
    /// `minecraft:attack_damage` base value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attack_damage: Option<f64>,
    /// `minecraft:movement_speed` base value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub movement_speed: Option<f64>,
    /// `minecraft:follow_range` base value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follow_range: Option<f64>,
}

/// One permanent status effect on a wave mob (DSL v0.4), emitted as an ambient,
/// non-expiring `effect give`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MobEffect {
    /// Vanilla effect id (e.g. `minecraft:slowness`), validated against the
    /// pinned registry (`DW0192`).
    pub effect: String,
    /// Amplifier (0 = level I).
    pub amplifier: u32,
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
/// `kill`, `collect` and `interact` are **implemented in DSL v0.3**; in a v0.2
/// campaign they are still reserved and rejected with `DW0141` (see
/// [`Objective::v03_verb`]). Every variant may carry `requires_flags` (v0.3):
/// flag-gated activation, satisfied only once each referenced flag has been set
/// by a `set-flag` effect.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Objective {
    /// Completed by a dialogue option's `complete-objective` effect.
    TalkTo {
        /// Objective id.
        id: ObjectiveId,
        /// Short player-facing objective name (v0.3, optional).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        /// One-line location/direction hint (v0.3, optional).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hint: Option<String>,
        /// The NPC to talk to.
        npc: NpcId,
        /// Prerequisite objectives (intra-quest ordering).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        after: Vec<ObjectiveId>,
        /// Flags that must be set before this objective activates (v0.3).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
        /// Bot stealth hint (DSL v0.4): mark this leg as one the critical-path
        /// bot should traverse sneaking (sprint disabled). Emitted into
        /// `critical-path.json` as `sneak: true` on the step. Purely a harness
        /// hint; no datapack effect.
        #[serde(default, skip_serializing_if = "is_false")]
        stealth: bool,
    },
    /// Completed by reaching an anchor once prerequisites are met.
    ReachAnchor {
        /// Objective id.
        id: ObjectiveId,
        /// Short player-facing objective name (v0.3, optional).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        /// One-line location/direction hint (v0.3, optional).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hint: Option<String>,
        /// The anchor to reach.
        anchor: AnchorId,
        /// Completion radius (blocks).
        radius: u32,
        /// Prerequisite objectives (intra-quest ordering).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        after: Vec<ObjectiveId>,
        /// Flags that must be set before this objective activates (v0.3).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
        /// Bot stealth hint (DSL v0.4): mark this leg as one the critical-path
        /// bot should traverse sneaking (sprint disabled). Emitted into
        /// `critical-path.json` as `sneak: true` on the step. Purely a harness
        /// hint; no datapack effect.
        #[serde(default, skip_serializing_if = "is_false")]
        stealth: bool,
    },
    /// Completed when the referenced wave is fully slain (v0.3).
    Kill {
        /// Objective id.
        id: ObjectiveId,
        /// Short player-facing objective name (v0.3, optional).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        /// One-line location/direction hint (v0.3, optional).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hint: Option<String>,
        /// The wave (stage-5 `waves` ref) whose mobs must be slain.
        wave: WaveId,
        /// Prerequisite objectives.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        after: Vec<ObjectiveId>,
        /// Flags that must be set before this objective activates (v0.3).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
        /// Bot stealth hint (DSL v0.4): mark this leg as one the critical-path
        /// bot should traverse sneaking (sprint disabled). Emitted into
        /// `critical-path.json` as `sneak: true` on the step. Purely a harness
        /// hint; no datapack effect.
        #[serde(default, skip_serializing_if = "is_false")]
        stealth: bool,
    },
    /// Completed when `count` of `item` have been collected from `anchor` (v0.3).
    Collect {
        /// Objective id.
        id: ObjectiveId,
        /// Short player-facing objective name (v0.3, optional).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        /// One-line location/direction hint (v0.3, optional).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hint: Option<String>,
        /// Vanilla item id to collect (validated against the registry).
        item: String,
        /// How many are required.
        count: u32,
        /// The anchor items are provided at (chest / pickup).
        anchor: AnchorId,
        /// Prerequisite objectives.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        after: Vec<ObjectiveId>,
        /// Flags that must be set before this objective activates (v0.3).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
        /// Bot stealth hint (DSL v0.4): mark this leg as one the critical-path
        /// bot should traverse sneaking (sprint disabled). Emitted into
        /// `critical-path.json` as `sneak: true` on the step. Purely a harness
        /// hint; no datapack effect.
        #[serde(default, skip_serializing_if = "is_false")]
        stealth: bool,
    },
    /// Completed by interacting with an entity at `anchor`; if `requires_item` is
    /// set, the item must be in the player's inventory (v0.3).
    Interact {
        /// Objective id.
        id: ObjectiveId,
        /// Short player-facing objective name (v0.3, optional).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        /// One-line location/direction hint (v0.3, optional).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hint: Option<String>,
        /// The anchor the interaction entity stands at.
        anchor: AnchorId,
        /// Item required in inventory to complete the interaction (optional).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        requires_item: Option<String>,
        /// Prop block that IS the interaction affordance (DSL v0.4, spec-0008
        /// §2): the compiler `setblock`s it at the anchor on activation (exactly
        /// as `collect` uses a real chest). Omitted = the glowing-lantern
        /// hologram marker (the v0.3 fallback).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prop: Option<Prop>,
        /// Prerequisite objectives.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        after: Vec<ObjectiveId>,
        /// Flags that must be set before this objective activates (v0.3).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
        /// Bot stealth hint (DSL v0.4): mark this leg as one the critical-path
        /// bot should traverse sneaking (sprint disabled). Emitted into
        /// `critical-path.json` as `sneak: true` on the step. Purely a harness
        /// hint; no datapack effect.
        #[serde(default, skip_serializing_if = "is_false")]
        stealth: bool,
    },
}

/// A prop block for an `interact` objective (DSL v0.4). The block is the
/// affordance the player interacts with; its id is validated against the pinned
/// 1.21.11 block registry (`DW0193`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Prop {
    /// Vanilla block id (e.g. `minecraft:lever`).
    pub block: String,
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

    /// The short player-facing objective title (v0.3, optional).
    pub fn title(&self) -> Option<&str> {
        match self {
            Objective::TalkTo { title, .. }
            | Objective::ReachAnchor { title, .. }
            | Objective::Kill { title, .. }
            | Objective::Collect { title, .. }
            | Objective::Interact { title, .. } => title.as_deref(),
        }
    }

    /// The one-line location/direction hint (v0.3, optional).
    pub fn hint(&self) -> Option<&str> {
        match self {
            Objective::TalkTo { hint, .. }
            | Objective::ReachAnchor { hint, .. }
            | Objective::Kill { hint, .. }
            | Objective::Collect { hint, .. }
            | Objective::Interact { hint, .. } => hint.as_deref(),
        }
    }

    /// Mutable access to the optional player-facing title (i18n localization).
    pub fn title_mut(&mut self) -> &mut Option<String> {
        match self {
            Objective::TalkTo { title, .. }
            | Objective::ReachAnchor { title, .. }
            | Objective::Kill { title, .. }
            | Objective::Collect { title, .. }
            | Objective::Interact { title, .. } => title,
        }
    }

    /// Mutable access to the optional one-line hint (i18n localization).
    pub fn hint_mut(&mut self) -> &mut Option<String> {
        match self {
            Objective::TalkTo { hint, .. }
            | Objective::ReachAnchor { hint, .. }
            | Objective::Kill { hint, .. }
            | Objective::Collect { hint, .. }
            | Objective::Interact { hint, .. } => hint,
        }
    }

    /// The flags that must be set before this objective activates (v0.3).
    pub fn requires_flags(&self) -> &[FlagId] {
        match self {
            Objective::TalkTo { requires_flags, .. }
            | Objective::ReachAnchor { requires_flags, .. }
            | Objective::Kill { requires_flags, .. }
            | Objective::Collect { requires_flags, .. }
            | Objective::Interact { requires_flags, .. } => requires_flags,
        }
    }

    /// The bot stealth hint (DSL v0.4): traverse this leg sneaking.
    pub fn stealth(&self) -> bool {
        match self {
            Objective::TalkTo { stealth, .. }
            | Objective::ReachAnchor { stealth, .. }
            | Objective::Kill { stealth, .. }
            | Objective::Collect { stealth, .. }
            | Objective::Interact { stealth, .. } => *stealth,
        }
    }

    /// The `interact` prop block (DSL v0.4), if this is an `interact` with a prop.
    pub fn prop(&self) -> Option<&Prop> {
        match self {
            Objective::Interact { prop, .. } => prop.as_ref(),
            _ => None,
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

    /// The v0.3 verb name if this objective is one of the verbs introduced in
    /// DSL v0.3 (`kill`/`collect`/`interact`). These validate in v0.3 campaigns
    /// and are reserved (`DW0141`) in v0.2 campaigns.
    pub fn v03_verb(&self) -> Option<&'static str> {
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
/// `give-item`, `set-flag` and `spawn-wave` are **implemented in DSL v0.3**; in a
/// v0.2 campaign they are still reserved and rejected with `DW0141` (see
/// [`QuestEffect::v03_effect`]).
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
    /// Gives the player an item (v0.3).
    GiveItem {
        /// Vanilla item id to give (validated against the registry).
        item: String,
        /// How many to give.
        count: u32,
        /// Optional display name (DSL v0.4), matching [`KitItem::name`].
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    /// Sets a campaign flag, enabling flag-gated objectives (v0.3).
    SetFlag {
        /// The flag to set.
        flag: FlagId,
    },
    /// Spawns a stage-5 wave's mobs at its anchor (v0.3).
    SpawnWave {
        /// The wave (stage-5 `waves` ref) to spawn.
        wave: WaveId,
    },
    /// Narrates a player-visible line (DSL v0.4, spec-0008 §3). `text` enters the
    /// l10n key inventory like any player-visible string.
    Narrate {
        /// The line shown to the player.
        text: String,
        /// Presentation channel (default `chat`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        style: Option<NarrateStyle>,
        /// Optional sound id played alongside the line.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sound: Option<String>,
    },
    /// Sets a block at an anchor (DSL v0.4, spec-0008 §2). General form of a prop
    /// placement. Block id validated against the pinned 1.21.11 block registry.
    SetBlock {
        /// The anchor to place the block at.
        anchor: AnchorId,
        /// Vanilla block id to place.
        block: String,
    },
    /// Despawns an NPC and its interaction hitbox (DSL v0.4, spec-0008 §5).
    DespawnNpc {
        /// The NPC (stage-2 ref) to remove.
        npc: NpcId,
    },
    /// Moves an NPC (and its interaction hitbox in lockstep) to an anchor (DSL
    /// v0.4, spec-0008 §5). The compiler emits a collision-safe teleport to the
    /// resolved anchor cell — a valid standable location — so the move never
    /// lands inside a wall. (`speed` is reserved for smooth per-tick pathfinding.)
    MoveNpc {
        /// The NPC (stage-2 ref) to move.
        npc: NpcId,
        /// The destination anchor.
        to_anchor: AnchorId,
        /// Optional travel speed in blocks/tick (reserved).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        speed: Option<f64>,
    },
    /// Plays a scripted camera cutscene (DSL v0.4 addendum). Per player: save
    /// gamemode+position, spectator, then dolly two co-located cameras along a
    /// straight-line lerp between waypoints and alternate `spectate` between them
    /// each tick (the two-camera bounce; the same-entity re-`spectate` is a server
    /// no-op and is never emitted), and restore on completion.
    Cutscene {
        /// Ordered camera waypoints (straight-line lerp between them).
        path: Vec<CameraWaypoint>,
        /// Cutscene duration in seconds.
        seconds: u32,
    },
}

/// The presentation channel for a [`QuestEffect::Narrate`] (DSL v0.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum NarrateStyle {
    /// A chat line (default).
    Chat,
    /// A large on-screen title.
    Title,
    /// An on-screen subtitle.
    Subtitle,
}

impl NarrateStyle {
    /// The kebab tag (`chat` / `title` / `subtitle`).
    pub fn token(self) -> &'static str {
        match self {
            NarrateStyle::Chat => "chat",
            NarrateStyle::Title => "title",
            NarrateStyle::Subtitle => "subtitle",
        }
    }
}

/// One camera waypoint of a [`QuestEffect::Cutscene`] (DSL v0.4): an anchor plus
/// an integer block offset from it, giving the camera's world position.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CameraWaypoint {
    /// The anchor the waypoint is relative to.
    pub anchor: AnchorId,
    /// Integer `[x, y, z]` block offset from the anchor (default `[0, 0, 0]`).
    #[serde(default, skip_serializing_if = "is_zero3")]
    pub offset: [i32; 3],
}

/// serde `skip_serializing_if` helper: skip a `[0, 0, 0]` offset.
fn is_zero3(v: &[i32; 3]) -> bool {
    *v == [0, 0, 0]
}

impl QuestEffect {
    /// The gate anchor if this is `open-gate`.
    pub fn open_gate_anchor(&self) -> Option<&AnchorId> {
        match self {
            QuestEffect::OpenGate { anchor } => Some(anchor),
            _ => None,
        }
    }

    /// The wave id if this is `spawn-wave` (v0.3).
    pub fn spawn_wave(&self) -> Option<&WaveId> {
        match self {
            QuestEffect::SpawnWave { wave } => Some(wave),
            _ => None,
        }
    }

    /// The flag id if this is `set-flag` (v0.3).
    pub fn set_flag(&self) -> Option<&FlagId> {
        match self {
            QuestEffect::SetFlag { flag } => Some(flag),
            _ => None,
        }
    }

    /// The item id if this is `give-item` (v0.3).
    pub fn give_item(&self) -> Option<&str> {
        match self {
            QuestEffect::GiveItem { item, .. } => Some(item),
            _ => None,
        }
    }

    /// `true` if this is a `give-item` carrying a v0.4 display `name`.
    pub fn give_item_named(&self) -> bool {
        matches!(self, QuestEffect::GiveItem { name: Some(_), .. })
    }

    /// The `set-block` block id if this is a v0.4 `set-block` effect.
    pub fn set_block(&self) -> Option<(&AnchorId, &str)> {
        match self {
            QuestEffect::SetBlock { anchor, block } => Some((anchor, block.as_str())),
            _ => None,
        }
    }

    /// The NPC id if this is a v0.4 `despawn-npc` effect.
    pub fn despawn_npc(&self) -> Option<&NpcId> {
        match self {
            QuestEffect::DespawnNpc { npc } => Some(npc),
            _ => None,
        }
    }

    /// `(npc, to_anchor)` if this is a v0.4 `move-npc` effect.
    pub fn move_npc(&self) -> Option<(&NpcId, &AnchorId)> {
        match self {
            QuestEffect::MoveNpc { npc, to_anchor, .. } => Some((npc, to_anchor)),
            _ => None,
        }
    }

    /// The v0.3 effect name if this effect is one introduced in DSL v0.3
    /// (`give-item`/`set-flag`/`spawn-wave`). These validate in v0.3 campaigns
    /// and are reserved (`DW0141`) in v0.2 campaigns.
    pub fn v03_effect(&self) -> Option<&'static str> {
        match self {
            QuestEffect::GiveItem { .. } => Some("give-item"),
            QuestEffect::SetFlag { .. } => Some("set-flag"),
            QuestEffect::SpawnWave { .. } => Some("spawn-wave"),
            QuestEffect::OpenGate { .. } | QuestEffect::CampaignComplete => None,
            // v0.4 effects report via `v04_effect`; they are not v0.3 verbs.
            QuestEffect::Narrate { .. }
            | QuestEffect::SetBlock { .. }
            | QuestEffect::DespawnNpc { .. }
            | QuestEffect::MoveNpc { .. }
            | QuestEffect::Cutscene { .. } => None,
        }
    }

    /// The v0.4 effect name if this effect is one introduced in DSL v0.4
    /// (`narrate`/`set-block`/`despawn-npc`/`move-npc`/`cutscene`). These validate
    /// in v0.4 campaigns and are reserved (`DW0141`) earlier.
    pub fn v04_effect(&self) -> Option<&'static str> {
        match self {
            QuestEffect::Narrate { .. } => Some("narrate"),
            QuestEffect::SetBlock { .. } => Some("set-block"),
            QuestEffect::DespawnNpc { .. } => Some("despawn-npc"),
            QuestEffect::MoveNpc { .. } => Some("move-npc"),
            QuestEffect::Cutscene { .. } => Some("cutscene"),
            _ => None,
        }
    }
}
