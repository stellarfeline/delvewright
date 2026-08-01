//! serde types for the six stage `content` payloads (spec-0001 v0.2).
//!
//! Every struct is `deny_unknown_fields`. Reserved enum values (npc `role`,
//! objective/effect types) parse successfully but are rejected by validation
//! ([`crate::validate`]) with code `DW0141`.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{
    ActorId, AnchorId, AreaId, ClassId, DialogueId, FlagId, NpcId, ObjectiveId, PoolId, PrefabId,
    QuestId, TrapId, TriggerId, WaveId,
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
    /// Declared initial world time (DSL v0.5, spec-0010). Dimension-global; frozen
    /// by environment sealing (`advance_time false`) so the set state persists.
    /// Absent = `noon` (the v0 default). Affects sky attenuation in the compiler's
    /// assembled-light model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<WorldTime>,
    /// Declared initial weather (DSL v0.5, spec-0010). Dimension-global; frozen by
    /// environment sealing (`advance_weather false`). Absent = `clear`. Rain and
    /// thunder attenuate effective sky brightness in the assembled-light model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weather: Option<WorldWeather>,
    /// Scenic horizon (DSL v0.6, spec-0013). Absent or `void` = the void world
    /// (byte-identical to v0.5). `ocean` swaps the world generator for a
    /// deterministic superflat sea (bedrock/stone/water, sea level y=62) so areas
    /// sitting at y=64+ read as islands. No structures or mobs either way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizon: Option<Horizon>,
    /// Playable-region boundary (DSL v0.6, spec-0013). When present, the compiler
    /// derives a region from the placed geometry and a per-second clock returns any
    /// player who leaves it to the last checkpoint. Required when `horizon` is
    /// `ocean` (an infinite swimmable sea with no return rule is `DW0320`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boundary: Option<Boundary>,
}

/// A declared world time state (DSL v0.5, spec-0010). Values are the vanilla
/// `/time set` keywords; the sole difference from vanilla is that the daylight
/// cycle is frozen (`advance_time false`), so a set state persists for the whole
/// delve until a `set-time` effect cuts to another.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum WorldTime {
    /// Morning daylight (`/time set day`, 1000 ticks).
    Day,
    /// Midday, brightest (`/time set noon`, 6000 ticks) — the default.
    #[default]
    Noon,
    /// Dusk/night (`/time set night`, 13000 ticks).
    Night,
    /// Deep night, darkest (`/time set midnight`, 18000 ticks).
    Midnight,
}

impl WorldTime {
    /// The vanilla `/time set` keyword.
    pub fn token(self) -> &'static str {
        match self {
            WorldTime::Day => "day",
            WorldTime::Noon => "noon",
            WorldTime::Night => "night",
            WorldTime::Midnight => "midnight",
        }
    }

    /// The `daytime` tick value the keyword sets (the `time query daytime`
    /// read-back). Vanilla constants: day=1000, noon=6000, night=13000,
    /// midnight=18000.
    pub fn daytime_ticks(self) -> i64 {
        match self {
            WorldTime::Day => 1000,
            WorldTime::Noon => 6000,
            WorldTime::Night => 13000,
            WorldTime::Midnight => 18000,
        }
    }
}

/// A declared weather state (DSL v0.5, spec-0010). Values are the vanilla
/// `/weather` keywords; frozen (`advance_weather false`), so a set state persists.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum WorldWeather {
    /// Clear sky (`/weather clear`) — the default.
    #[default]
    Clear,
    /// Rain (`/weather rain`).
    Rain,
    /// Thunderstorm (`/weather thunder`).
    Thunder,
}

impl WorldWeather {
    /// The vanilla `/weather` keyword.
    pub fn token(self) -> &'static str {
        match self {
            WorldWeather::Clear => "clear",
            WorldWeather::Rain => "rain",
            WorldWeather::Thunder => "thunder",
        }
    }
}

/// A scenic horizon (DSL v0.6, spec-0013). `void` is the default and is
/// byte-identical to v0.5 (empty-layer superflat, `minecraft:the_void` biome);
/// `ocean` selects a pinned bedrock/stone/water superflat with sea level y=62,
/// pure backdrop with no structures or mobs. The compiler owns the exact
/// generator-settings; this enum only picks which one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Horizon {
    /// The void world (default) — no sky-filling geometry.
    #[default]
    Void,
    /// A superflat sea backdrop; areas at y=64+ read as islands.
    Ocean,
}

/// The default boundary `margin` (blocks of horizontal breathing room added
/// around the derived region). Separate function so `serde(default = …)` and the
/// documented literal cannot drift.
fn default_margin() -> u16 {
    16
}

/// A playable-region boundary declaration (DSL v0.6, spec-0013). The region
/// itself is **derived** by the compiler (union of the final placed-piece AABBs,
/// inflated horizontally by `margin`, unbounded upward, floored at the lowest
/// placed block − 8) — never authored — so "every anchor is inside" is structural.
/// Enforcement is a per-second clock that returns any player outside the region to
/// the last checkpoint (`dw:cp`) with an actionbar message and a soft sound; no
/// damage, no items lost.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Boundary {
    /// Horizontal breathing room in blocks added around the derived region on
    /// every side (default 16). Range-checked to `0..=64` (`DW0321`).
    #[serde(default = "default_margin")]
    pub margin: u16,
    /// Actionbar message shown on return. Absent = the compiler's English default.
    /// When set, it is inventoried under l10n key `world.boundary.message` and is
    /// translated like every other player-facing string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// A supplemental-lighting fixture the relight pass may place (DSL v0.5,
/// spec-0010 fixture registry v1). The theme choice stays in the DSL layer; the
/// compiler owns the placement rule and block-light emission.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Fixture {
    /// Floor torch (block light 14); `wall_torch` on a wall face as fallback.
    Torch,
    /// Ceiling-hung lantern (block light 15); floor-sitting as fallback.
    Lantern,
    /// Floor campfire (block light 15); never on or adjacent to a required path
    /// cell (it is a damage source).
    Campfire,
    /// Embedded shroomlight (block light 15); replaces a solid wall/ceiling block.
    Shroomlight,
}

impl Fixture {
    /// The kebab id (`torch` / `lantern` / `campfire` / `shroomlight`).
    pub fn token(self) -> &'static str {
        match self {
            Fixture::Torch => "torch",
            Fixture::Lantern => "lantern",
            Fixture::Campfire => "campfire",
            Fixture::Shroomlight => "shroomlight",
        }
    }
}

/// A per-area supplemental-lighting declaration (DSL v0.5, spec-0010). Its
/// presence puts the area on the relight path: the compiler guarantees every
/// reachable walkable cell reaches `min_light` by placing `fixture`s, or fails
/// with `DW0211` if it cannot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AreaLighting {
    /// The fixture the relight pass places.
    pub fixture: Fixture,
    /// The minimum block+sky light guaranteed on reachable walkable cells
    /// (1..=14, default 7). Validated by `DW0141`-adjacent bounds checking.
    #[serde(default = "default_min_light")]
    pub min_light: u8,
}

/// Default `min_light` for an [`AreaLighting`] declaration (spec-0010).
fn default_min_light() -> u8 {
    7
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
    /// Optional supplemental-lighting declaration (DSL v0.5, spec-0010). When
    /// present, the compiler's relight pass guarantees `min_light` on every
    /// reachable walkable cell of this area by placing the declared fixture, or
    /// fails with `DW0211`. Absent = no relight (the area is judged as-assembled,
    /// with `DW0210` if a reachable walkable cell is dark and unmitigated).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lighting: Option<AreaLighting>,
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
    /// Cuts the world time (DSL v0.5), mirroring [`QuestEffect::SetTime`].
    SetTime {
        /// The time state to cut to.
        time: WorldTime,
    },
    /// Cuts the weather (DSL v0.5), mirroring [`QuestEffect::SetWeather`].
    SetWeather {
        /// The weather state to cut to.
        weather: WorldWeather,
    },
    /// Sets the party-wide respawn checkpoint (DSL v0.6, spec-0012), mirroring
    /// [`QuestEffect::SetCheckpoint`] — usable from a dialogue outcome.
    SetCheckpoint {
        /// The prefab checkpoint anchor the party respawns at.
        anchor: AnchorId,
        /// Per-player effects re-run on respawn while this checkpoint is active
        /// (scene reset). Empty = no hook.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        on_respawn: Vec<QuestEffect>,
    },
}

impl DialogueEffect {
    /// The v0.5 effect name if this dialogue effect is one introduced in DSL v0.5
    /// (`set-time`/`set-weather`, spec-0010).
    pub fn v05_effect(&self) -> Option<&'static str> {
        match self {
            DialogueEffect::SetTime { .. } => Some("set-time"),
            DialogueEffect::SetWeather { .. } => Some("set-weather"),
            _ => None,
        }
    }

    /// The v0.6 effect name if this dialogue effect is one introduced in DSL v0.6
    /// (`set-checkpoint`, spec-0012). Reserved (`DW0141`) in an earlier campaign.
    pub fn v06_effect(&self) -> Option<&'static str> {
        match self {
            DialogueEffect::SetCheckpoint { .. } => Some("set-checkpoint"),
            _ => None,
        }
    }

    /// `(anchor, on_respawn)` if this is a v0.6 `set-checkpoint` dialogue effect.
    pub fn set_checkpoint(&self) -> Option<(&AnchorId, &[QuestEffect])> {
        match self {
            DialogueEffect::SetCheckpoint { anchor, on_respawn } => {
                Some((anchor, on_respawn.as_slice()))
            }
            _ => None,
        }
    }
}

impl DialogueEffect {
    /// The `set-flag` flag id if this is a v0.4 `set-flag` dialogue effect.
    pub fn set_flag(&self) -> Option<&FlagId> {
        match self {
            DialogueEffect::SetFlag { flag } => Some(flag),
            _ => None,
        }
    }

    /// The v0.4 effect name if this dialogue effect is one introduced in DSL v0.4
    /// (`set-flag`). Reserved (`DW0141`) in an earlier campaign.
    pub fn v04_effect(&self) -> Option<&'static str> {
        match self {
            DialogueEffect::SetFlag { .. } => Some("set-flag"),
            _ => None,
        }
    }

    /// The target time if this is a v0.5 `set-time` dialogue effect.
    pub fn set_time(&self) -> Option<WorldTime> {
        match self {
            DialogueEffect::SetTime { time } => Some(*time),
            _ => None,
        }
    }

    /// The target weather if this is a v0.5 `set-weather` dialogue effect.
    pub fn set_weather(&self) -> Option<WorldWeather> {
        match self {
            DialogueEffect::SetWeather { weather } => Some(*weather),
            _ => None,
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
    /// Scripted actors (DSL v0.6, spec-0014): NoAI/Silent/no-loot puppets moved by
    /// compiler-emitted per-tick teleport (task #46). Distinct from stage-2 NPCs
    /// (no dialogue, any mob type). Summoned/removed/moved/unleashed by the actor
    /// staging effects. Empty/absent before v0.6.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actors: Vec<Actor>,
    /// Traps (DSL v0.6, spec-0011): redstone-native environmental hazards bound to
    /// `anchor/trap` prefab markers. Each gives mute trap hardware meaning — its
    /// trigger, dispenser payload, lethality, disarm, and reset. Empty/absent in
    /// pre-0.6 campaigns (reserved `DW0141`), so a v0.5-or-earlier campaign that
    /// declares none stays byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub traps: Vec<Trap>,
}

/// A stage-5 trap (DSL v0.6, spec-0011): a redstone-native environmental hazard.
/// The trap *mechanism* — the trigger (pressure plate / tripwire / trapped chest)
/// wired to a pre-placed empty dispenser — lives in the `.nbt` prefab at an
/// `anchor/trap` marker; this declaration gives the mute hardware meaning. The
/// compiler fills the dispenser payload, models the trigger cell as a hazard for
/// the completability proofs (`DW0342`), and — for a disarmable trap — emits the
/// disarm affordance. No detection is emitted for the harm itself: the redstone
/// fires it ("harm is redstone-native", spec-0011). Player-vs-mob distinguishing
/// matters in a sealed box-garden with controlled mobs, so `trapped-chest` (opened
/// by a player) is called out as the only player-distinct trigger.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Trap {
    /// Unique trap id (`trap/<kebab>`).
    pub id: TrapId,
    /// The `anchor/trap` marker this trap binds to. Its cell is the trigger/hazard
    /// cell the compiler models; the anchor also carries the dispenser socket the
    /// payload fills.
    pub at: AnchorId,
    /// What springs the trap (all redstone-native).
    pub trigger: TrapTrigger,
    /// What the trap does when sprung.
    pub effect: TrapEffect,
    /// How dangerous the trap is. A `lethal` trap on the forced critical path
    /// carries the completability obligation (`DW0342`); `harmful`/`nonlethal`
    /// carry none. Defaults to `harmful`.
    #[serde(default)]
    pub lethality: Lethality,
    /// Optional disarm affordance (quest-coupling): an anchor the player acts on to
    /// turn the trap off — setting a flag and emptying the dispenser — before the
    /// trap cell is forced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disarm: Option<TrapDisarm>,
    /// Whether the trap re-arms after firing. `once` = single-shot (fires, then
    /// spent — the survivability path); `rearm` = re-fires each trigger (default).
    #[serde(default)]
    pub reset: TrapReset,
    /// Flags that must be set before the trap is considered active (mirrors
    /// [`EnvTrigger::requires_flags`]). Default empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_flags: Vec<FlagId>,
}

impl Trap {
    /// `(item, count)` if this trap's effect is a `dispense` payload.
    pub fn dispense(&self) -> Option<(&str, u32)> {
        match &self.effect {
            TrapEffect::Dispense { item, count } => Some((item.as_str(), *count)),
        }
    }

    /// Whether this trap is `lethal` (carries the `DW0342` obligation on the
    /// forced critical path).
    pub fn is_lethal(&self) -> bool {
        matches!(self.lethality, Lethality::Lethal)
    }
}

/// The mechanism that springs a [`Trap`] (DSL v0.6, spec-0011). All three are
/// redstone-native — the hardware fires without any command — so the compiler
/// emits no detection for them; it only models the trigger cell as a hazard and
/// fills the dispenser payload. (`approach`, the compiler-detected v0.4 primitive,
/// is deliberately *not* a trap trigger: it is already fully expressible as an
/// [`EnvTrigger`], so admitting it here would only duplicate that surface.)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TrapTrigger {
    /// A pressure plate: any entity stepping on the cell. Auto-rearms on step-off.
    PressurePlate,
    /// A tripwire line: any entity crossing it. Auto-rearms.
    Tripwire,
    /// A trapped chest: a *player opening it* (comparator pulse). The only
    /// player-distinct trigger — a controlled mob cannot spring it.
    TrappedChest,
}

impl TrapTrigger {
    /// The kebab tag (`pressure-plate` / `tripwire` / `trapped-chest`).
    pub fn kind(&self) -> &'static str {
        match self {
            TrapTrigger::PressurePlate => "pressure-plate",
            TrapTrigger::Tripwire => "tripwire",
            TrapTrigger::TrappedChest => "trapped-chest",
        }
    }
}

/// What a [`Trap`] does when sprung (DSL v0.6, spec-0011). Externally tagged so a
/// future effect adds a variant; a non-`dispense` key (e.g. `tnt`,
/// `release-falling-block`, `crusher`) is an unknown variant → `DW0100`, keeping
/// block-destroying and unmodeled effects out of the schema by construction
/// (spec-0011 non-goals — no hardware the compiler cannot model reaches a world).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum TrapEffect {
    /// Load the prefab's pre-wired dispenser with `count` of `item` (arrows, tipped
    /// arrows, splash potions). The redstone fires it; terrain is untouched
    /// (spec-0011 "primary lethal"). The item is round-tripped into the dispenser
    /// `Items` NBT — a deterministic, static payload.
    Dispense {
        /// Vanilla item id (validated against the pinned 1.21.11 registry, `DW0341`).
        item: String,
        /// How many to load into the dispenser stack.
        count: u32,
    },
}

/// How dangerous a [`Trap`] is (DSL v0.6, spec-0011). Only `lethal` carries the
/// forced-critical-path completability obligation (`DW0342`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Lethality {
    /// Can kill a full-health player — carries the `DW0342` obligation on the path.
    Lethal,
    /// Hurts but is not designed to kill (the default).
    #[default]
    Harmful,
    /// Cosmetic / trivial (a stumble, a scare).
    Nonlethal,
}

/// Whether a [`Trap`] re-arms after firing (DSL v0.6, spec-0011).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TrapReset {
    /// Fires once, then is spent — the survivability path for a forced lethal trap
    /// (respawn-safe with `keep_inventory`, non-re-triggering on the walk back; no
    /// soft-loop).
    Once,
    /// Re-fires every time the trigger is met (default). A forced lethal `rearm`
    /// trap must be avoidable or disarmable, else `DW0342`.
    #[default]
    Rearm,
}

/// A [`Trap`]'s disarm affordance (DSL v0.6, spec-0011): the player acts on the
/// `via` anchor (an interaction the compiler emits, reusing the v0.4 interaction
/// entity) to turn the trap off — setting `sets_flag` and emptying the dispenser —
/// before the trap cell is forced. Discharges the `DW0342` obligation when the
/// affordance is reachable ahead of the trap without crossing it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrapDisarm {
    /// The anchor the player interacts with to disarm.
    pub via: AnchorId,
    /// The flag set when the trap is disarmed (a new flag this trap produces; other
    /// objectives/triggers may read it via `requires_flags`).
    pub sets_flag: FlagId,
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

// ---------------------------------------------------------------------------
// Stage 5 — scripted actors (DSL v0.6, spec-0014)
// ---------------------------------------------------------------------------

/// A scripted stage actor (DSL v0.6, spec-0014): a NoAI/Silent/no-loot puppet,
/// distinct from a stage-2 [`Npc`] (no dialogue, any mob type). Emitted with tag
/// `dw_actor_<id>`, `Invulnerable` unless `vulnerable` (a damageable puppet stays
/// knockback-immune — the tower-defense creep). `skin` re-dresses it as a
/// `minecraft:mannequin`, exactly as a stage-2 NPC skin. The puppet is summoned by
/// a `spawn-actor` effect (not at load), moved by `move-actor`, and can be replaced
/// by a real-AI twin with `unleash-actor`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Actor {
    /// Unique actor id (`actor/<kebab>`).
    pub id: ActorId,
    /// The vanilla entity to puppet, e.g. `minecraft:warden`. Validated against the
    /// pinned 1.21.11 entity registry (`DW0173`).
    pub entity: String,
    /// Optional custom name shown above the puppet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Optional player-model skin (mannequin), as a stage-2 NPC (`DW0190`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skin: Option<NpcSkin>,
    /// The anchor the puppet is summoned on (resolved across areas, like an
    /// `open-gate` / `move-npc` destination).
    pub anchor: AnchorId,
    /// Initial facing (default `south`). The puppet spawns yawed this way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facing: Option<Facing>,
    /// If `true`, the puppet is damageable (a tower-defense creep) but stays
    /// knockback-immune; default `false` (fully `Invulnerable`).
    #[serde(default, skip_serializing_if = "is_false")]
    pub vulnerable: bool,
}

/// A cardinal facing keyword (DSL v0.6). Emitted as the puppet's spawn yaw
/// (MC: yaw 0 = +z/south).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Facing {
    /// Facing +z (yaw 0) — the default.
    South,
    /// Facing -z (yaw 180).
    North,
    /// Facing -x (yaw 90).
    West,
    /// Facing +x (yaw 270).
    East,
}

impl Facing {
    /// The kebab token (`south` / `north` / `west` / `east`).
    pub fn token(self) -> &'static str {
        match self {
            Facing::South => "south",
            Facing::North => "north",
            Facing::West => "west",
            Facing::East => "east",
        }
    }
}

/// How a `despawn-actor` removes its puppet (DSL v0.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DespawnStyle {
    /// Silent removal (`kill @e` with no death animation is not possible; the
    /// compiler removes the entity via `/kill` on an `Invulnerable` puppet, or a
    /// data-driven removal — see the emitter — so no death particles/sound show).
    Vanish,
    /// Plays the vanilla death animation (a cutscene death).
    Kill,
}

impl DespawnStyle {
    /// The kebab token (`vanish` / `kill`).
    pub fn token(self) -> &'static str {
        match self {
            DespawnStyle::Vanish => "vanish",
            DespawnStyle::Kill => "kill",
        }
    }
}

/// One step of a [`QuestEffect::Sequence`] (DSL v0.6): a group of effects fired at
/// an exact tick offset from the sequence's start.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SequenceStep {
    /// Tick offset from the sequence start at which `effects` fire.
    pub at_ticks: u32,
    /// The effects fired at `at_ticks`. Any stage-5 effect except a nested
    /// `sequence` (rejected with `DW0329`).
    pub effects: Vec<QuestEffect>,
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
        /// Per-effect flag gate (DSL v0.6); see [`QuestEffect::requires_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
    },
    /// Seals a prefab-declared gate — the physical dual of `open-gate` (DSL v0.6):
    /// fills the gate anchor's region with the block the anchor declares (e.g. the
    /// boulder's `minecraft:basalt`), turning an opened threshold back into a wall.
    /// The declared fill block is prefab metadata; a gate anchor with no `block` is
    /// rejected (`DW0343`). The completability model treats the region as **solid**
    /// from the point in the quest DAG where this fires (mirroring how `open-gate`'s
    /// clearing is modelled) — a critical path that must cross a gate after it seals
    /// fails the DW0311 reachability proof.
    CloseGate {
        /// The gate anchor to seal.
        anchor: AnchorId,
        /// Per-effect flag gate (DSL v0.6); see [`QuestEffect::requires_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
    },
    /// Marks the campaign complete (final advancement + credits). Terminal — not
    /// flag-gatable (gating the campaign's own completion is a deadlock footgun),
    /// so this variant carries no `requires_flags`.
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
        /// Per-effect flag gate (DSL v0.6); see [`QuestEffect::requires_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
    },
    /// Sets a campaign flag, enabling flag-gated objectives (v0.3).
    SetFlag {
        /// The flag to set.
        flag: FlagId,
        /// Per-effect flag gate (DSL v0.6); see [`QuestEffect::requires_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
    },
    /// Spawns a stage-5 wave's mobs at its anchor (v0.3).
    SpawnWave {
        /// The wave (stage-5 `waves` ref) to spawn.
        wave: WaveId,
        /// Per-effect flag gate (DSL v0.6); see [`QuestEffect::requires_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
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
        /// Per-effect flag gate (DSL v0.6); see [`QuestEffect::requires_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
    },
    /// Sets a block at an anchor (DSL v0.4, spec-0008 §2). General form of a prop
    /// placement. Block id validated against the pinned 1.21.11 block registry;
    /// a vanilla blockstate suffix (`minecraft:grindstone[face=floor]`) is
    /// accepted and passed through verbatim (DSL v0.6).
    SetBlock {
        /// The anchor to place the block at.
        anchor: AnchorId,
        /// Vanilla block id to place.
        block: String,
        /// Per-effect flag gate (DSL v0.6); see [`QuestEffect::requires_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
    },
    /// Despawns an NPC and its interaction hitbox (DSL v0.4, spec-0008 §5).
    DespawnNpc {
        /// The NPC (stage-2 ref) to remove.
        npc: NpcId,
        /// Per-effect flag gate (DSL v0.6); see [`QuestEffect::requires_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
    },
    /// Moves an NPC (and its interaction hitbox in lockstep) to an anchor (DSL
    /// v0.4, spec-0008 §5 + addendum). The compiler plans a **collision-safe walked
    /// path** by A* over the solved voxel grid and emits per-tick teleport
    /// waypoints along it, so the NPC never clips a wall and walks up to (not into)
    /// a solid affordance. An unroutable move is a compile error (`DW0307`).
    MoveNpc {
        /// The NPC (stage-2 ref) to move.
        npc: NpcId,
        /// The destination anchor.
        to_anchor: AnchorId,
        /// Optional travel speed in blocks/tick (defaults to ~0.15).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        speed: Option<f64>,
        /// Per-effect flag gate (DSL v0.6); see [`QuestEffect::requires_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
    },
    /// Plays a scripted camera cutscene (DSL v0.4 addendum). Per player: save
    /// gamemode+position, spectator, then dolly two co-located cameras along a
    /// straight-line lerp between waypoints and alternate `spectate` between them
    /// each tick (the two-camera bounce; the same-entity re-`spectate` is a server
    /// no-op and is never emitted), and restore on completion. The compiler
    /// validates the dolly path passes only through non-solid blocks — cameras
    /// fly but must not clip a solid (`DW0308`).
    Cutscene {
        /// Ordered camera waypoints (straight-line lerp between them).
        path: Vec<CameraWaypoint>,
        /// Cutscene duration in seconds.
        seconds: u32,
        /// Per-effect flag gate (DSL v0.6); see [`QuestEffect::requires_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
    },
    /// Cuts the dimension-global world time to a new state (DSL v0.5, spec-0010).
    /// Instantaneous (vanilla has no gradual transition); the state persists
    /// because the daylight cycle is frozen by sealing.
    SetTime {
        /// The time state to cut to.
        time: WorldTime,
        /// Per-effect flag gate (DSL v0.6); see [`QuestEffect::requires_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
    },
    /// Cuts the dimension-global weather to a new state (DSL v0.5, spec-0010).
    /// Instantaneous; persists because the weather cycle is frozen by sealing.
    SetWeather {
        /// The weather state to cut to.
        weather: WorldWeather,
        /// Per-effect flag gate (DSL v0.6); see [`QuestEffect::requires_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
    },
    /// Plays a vanilla sound event, positionally or per-player (DSL v0.6,
    /// spec-0014). `sound` is validated against the vendored pinned-1.21.11
    /// sound-event registry (`DW0326` unknown). `at` selects where the sound
    /// originates (default: each player's own position); `volume`/`pitch` map to
    /// the `playsound` command's trailing args (pitch clamps to 0.0..=2.0 in
    /// vanilla).
    PlaySound {
        /// The vanilla sound-event id (`minecraft:` prefix optional).
        sound: String,
        /// Where the sound plays from (default: `players`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<SoundAt>,
        /// Playback volume (vanilla default 1.0; > 1.0 only extends audible range).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        volume: Option<f64>,
        /// Playback pitch (vanilla 0.0..=2.0; default 1.0).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pitch: Option<f64>,
        /// Per-effect flag gate (DSL v0.6); see [`QuestEffect::requires_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
    },
    /// Sets the party-wide respawn checkpoint (DSL v0.6, spec-0012). Emits
    /// `spawnpoint @a` at the anchor cell and mirrors the coords into
    /// `storage dw:cp pos`. Party-wide and monotonic by quest order (a later
    /// `set-checkpoint` always replaces an earlier one). The compiler proves the
    /// cell is standable (`DW0316`) and that the remaining critical path stays
    /// reachable from it (`DW0315`).
    SetCheckpoint {
        /// The prefab checkpoint anchor the party respawns at.
        anchor: AnchorId,
        /// Per-player effects re-run each time a player respawns while this
        /// checkpoint is the active one — scene reset (e.g. re-caging an
        /// unleashed actor). Emitted idempotently in declared order; empty = no
        /// hook. Respawn is detected via the vanilla `deathCount` criterion.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        on_respawn: Vec<QuestEffect>,
    },
    /// Begins a stealth beat (DSL v0.6, spec-0014). While active, every player
    /// must be inside some `zone` **and** sneaking each tick; a player failing
    /// both for `grace_ticks` fires `on_caught` (typically a kill → checkpoint
    /// respawn). Sneaking is read from the vanilla `sneak_time` custom stat; zone
    /// membership from the player's position. The compiler proves each zone is
    /// standable and reachable from the activating beat (`DW0327`).
    BeginStealth {
        /// The "shadow" regions, each an anchor-centred box (see [`StealthZone`]).
        zones: Vec<StealthZone>,
        /// Per-player effects fired when a player is caught (out of every zone or
        /// standing, for `grace_ticks`). Empty = no consequence.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        on_caught: Vec<QuestEffect>,
        /// Ticks a player may be exposed before `on_caught` fires (default 20).
        #[serde(default = "default_grace_ticks")]
        grace_ticks: u32,
    },
    /// Ends the active stealth beat (DSL v0.6, spec-0014). No-op if none active.
    EndStealth,
    // --- DSL v0.6 actor staging effects (spec-0014) ---
    /// Summons a stage-5 actor's puppet at its anchor (DSL v0.6). Idempotent: a
    /// spawn of an already-present actor is a no-op (re-caging after `unleash`).
    SpawnActor {
        /// The actor (stage-5 `actors` ref) to summon.
        actor: ActorId,
    },
    /// Removes an actor's puppet (DSL v0.6). `kill` plays the vanilla death
    /// animation (cutscene deaths); `vanish` is silent removal.
    DespawnActor {
        /// The actor (stage-5 `actors` ref) to remove.
        actor: ActorId,
        /// How the puppet is removed.
        style: DespawnStyle,
    },
    /// Walks an actor's puppet to an anchor by A*-planned per-tick teleport over
    /// the assembled model, using the actor's hitbox footprint, yawed along the
    /// path tangent (DSL v0.6, task #46). Concurrent movers are allowed (a herded
    /// flock is N synchronized `move-actor`s). Unroutable → `DW0325`. `on_arrive`
    /// effects fire once the puppet reaches the destination cell.
    MoveActor {
        /// The actor (stage-5 `actors` ref) to move.
        actor: ActorId,
        /// The destination anchor.
        to_anchor: AnchorId,
        /// Optional travel speed in blocks/tick (defaults to ~0.15).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        speed: Option<f64>,
        /// Effects fired once the puppet arrives at the destination cell.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        on_arrive: Vec<QuestEffect>,
    },
    /// Replaces an actor's puppet with a real-AI twin of the same type / position /
    /// name / attributes / tag (DSL v0.6) — the "attack the idle giant → real
    /// fight" beat. Re-caging is `despawn-actor` + `spawn-actor` (idempotent), not a
    /// special verb.
    UnleashActor {
        /// The actor (stage-5 `actors` ref) to unleash.
        actor: ActorId,
    },
    /// A deterministic timeline (DSL v0.6): one schedule chain firing effect groups
    /// at exact tick offsets. Effects are any in the stage-5 set except a nested
    /// `sequence` (rejected with `DW0329`).
    Sequence {
        /// Timeline steps; each fires its `effects` at `at_ticks` from the start.
        steps: Vec<SequenceStep>,
    },
}

/// Default `grace_ticks` for [`QuestEffect::BeginStealth`] (spec-0014).
fn default_grace_ticks() -> u32 {
    20
}

/// A stealth "shadow" region (DSL v0.6, spec-0014): an axis-aligned box centred
/// on `anchor`, extending `extent` blocks along each axis (so the box spans
/// `anchor ± extent`). Presented in-world via dark cells but judged purely by
/// region membership, so the check is deterministic and provable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StealthZone {
    /// The prefab anchor at the centre of the zone box.
    pub anchor: AnchorId,
    /// Half-extents `[x, y, z]` in blocks from the anchor (each component ≥ 0);
    /// the zone AABB is `[anchor - extent, anchor + extent]`.
    pub extent: [u32; 3],
}

/// Where a [`QuestEffect::PlaySound`] originates (DSL v0.6, spec-0014). The
/// `actor` variant is accepted by the schema but not yet wired — it is rejected
/// with `DW0335` until the actors surface (spec-0014 `actors[]`) lands, at which
/// point it resolves to the actor's position like `move-actor` does.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "at", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SoundAt {
    /// Play the sound positioned at a resolved anchor, audible to all players.
    Anchor {
        /// The anchor the sound plays from.
        anchor: AnchorId,
    },
    /// Play the sound at each player's own position (the default).
    Players,
    /// Play the sound at a scripted actor's position (deferred — `DW0335` until
    /// the actors surface lands).
    Actor {
        /// The actor id (stage-5 `actors[]`; not yet resolvable).
        actor: String,
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
    /// A large-glyph "art" title rendered through the delve's custom resource-pack
    /// font (`delve:art`) so endings can flash big art text (DSL v0.6, spec-0014).
    /// Text is checked at compile time against the font's glyph inventory
    /// (`DW0328`); characters outside it (e.g. non-Latin script) are rejected.
    Art,
}

impl NarrateStyle {
    /// The kebab tag (`chat` / `title` / `subtitle` / `art`).
    pub fn token(self) -> &'static str {
        match self {
            NarrateStyle::Chat => "chat",
            NarrateStyle::Title => "title",
            NarrateStyle::Subtitle => "subtitle",
            NarrateStyle::Art => "art",
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
            QuestEffect::OpenGate { anchor, .. } => Some(anchor),
            _ => None,
        }
    }

    /// The gate anchor if this is `close-gate` (DSL v0.6).
    pub fn close_gate_anchor(&self) -> Option<&AnchorId> {
        match self {
            QuestEffect::CloseGate { anchor, .. } => Some(anchor),
            _ => None,
        }
    }

    /// The wave id if this is `spawn-wave` (v0.3).
    pub fn spawn_wave(&self) -> Option<&WaveId> {
        match self {
            QuestEffect::SpawnWave { wave, .. } => Some(wave),
            _ => None,
        }
    }

    /// The flag id if this is `set-flag` (v0.3).
    pub fn set_flag(&self) -> Option<&FlagId> {
        match self {
            QuestEffect::SetFlag { flag, .. } => Some(flag),
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
            QuestEffect::SetBlock { anchor, block, .. } => Some((anchor, block.as_str())),
            _ => None,
        }
    }

    /// The NPC id if this is a v0.4 `despawn-npc` effect.
    pub fn despawn_npc(&self) -> Option<&NpcId> {
        match self {
            QuestEffect::DespawnNpc { npc, .. } => Some(npc),
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
            QuestEffect::OpenGate { .. }
            | QuestEffect::CloseGate { .. }
            | QuestEffect::CampaignComplete => None,
            // v0.4 effects report via `v04_effect`; v0.5 via `v05_effect`; they
            // are not v0.3 verbs.
            QuestEffect::Narrate { .. }
            | QuestEffect::SetBlock { .. }
            | QuestEffect::DespawnNpc { .. }
            | QuestEffect::MoveNpc { .. }
            | QuestEffect::Cutscene { .. }
            | QuestEffect::SetTime { .. }
            | QuestEffect::SetWeather { .. }
            | QuestEffect::PlaySound { .. }
            | QuestEffect::SetCheckpoint { .. }
            | QuestEffect::BeginStealth { .. }
            | QuestEffect::EndStealth
            | QuestEffect::SpawnActor { .. }
            | QuestEffect::DespawnActor { .. }
            | QuestEffect::MoveActor { .. }
            | QuestEffect::UnleashActor { .. }
            | QuestEffect::Sequence { .. } => None,
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

    /// The v0.5 effect name if this effect is one introduced in DSL v0.5
    /// (`set-time`/`set-weather`, spec-0010). These validate in v0.5 campaigns and
    /// are reserved (`DW0141`) earlier.
    pub fn v05_effect(&self) -> Option<&'static str> {
        match self {
            QuestEffect::SetTime { .. } => Some("set-time"),
            QuestEffect::SetWeather { .. } => Some("set-weather"),
            _ => None,
        }
    }

    /// The v0.6 effect name if this effect is one introduced in DSL v0.6
    /// (`set-checkpoint`, spec-0012; `begin-stealth`/`end-stealth`, spec-0014;
    /// `play-sound`, spec-0014; the scripted-actor staging verbs
    /// `spawn-actor`/`despawn-actor`/`move-actor`/`unleash-actor`/`sequence`,
    /// spec-0014). These validate in v0.6 campaigns and are reserved (`DW0141`)
    /// earlier. (The `narrate` `art` style is a v0.6 addition to an existing verb
    /// — see [`QuestEffect::narrate_art`] — not a new effect.)
    pub fn v06_effect(&self) -> Option<&'static str> {
        match self {
            QuestEffect::CloseGate { .. } => Some("close-gate"),
            QuestEffect::SetCheckpoint { .. } => Some("set-checkpoint"),
            QuestEffect::BeginStealth { .. } => Some("begin-stealth"),
            QuestEffect::EndStealth => Some("end-stealth"),
            QuestEffect::PlaySound { .. } => Some("play-sound"),
            QuestEffect::SpawnActor { .. } => Some("spawn-actor"),
            QuestEffect::DespawnActor { .. } => Some("despawn-actor"),
            QuestEffect::MoveActor { .. } => Some("move-actor"),
            QuestEffect::UnleashActor { .. } => Some("unleash-actor"),
            QuestEffect::Sequence { .. } => Some("sequence"),
            _ => None,
        }
    }

    /// `(anchor, on_respawn)` if this is a v0.6 `set-checkpoint` effect.
    pub fn set_checkpoint(&self) -> Option<(&AnchorId, &[QuestEffect])> {
        match self {
            QuestEffect::SetCheckpoint { anchor, on_respawn } => {
                Some((anchor, on_respawn.as_slice()))
            }
            _ => None,
        }
    }

    /// `(zones, on_caught, grace_ticks)` if this is a v0.6 `begin-stealth` effect.
    pub fn begin_stealth(&self) -> Option<(&[StealthZone], &[QuestEffect], u32)> {
        match self {
            QuestEffect::BeginStealth {
                zones,
                on_caught,
                grace_ticks,
            } => Some((zones.as_slice(), on_caught.as_slice(), *grace_ticks)),
            _ => None,
        }
    }

    /// The target time if this is a v0.5 `set-time` effect.
    pub fn set_time(&self) -> Option<WorldTime> {
        match self {
            QuestEffect::SetTime { time, .. } => Some(*time),
            _ => None,
        }
    }

    /// The target weather if this is a v0.5 `set-weather` effect.
    pub fn set_weather(&self) -> Option<WorldWeather> {
        match self {
            QuestEffect::SetWeather { weather, .. } => Some(*weather),
            _ => None,
        }
    }

    /// The effect lists nested one level inside this effect (DSL v0.6): a
    /// `sequence`'s per-step effects (in step order), a `set-checkpoint`'s
    /// `on_respawn`, a `begin-stealth`'s `on_caught`, and a `move-actor`'s
    /// `on_arrive`. Empty for a leaf effect.
    ///
    /// This is the **single authority** on effect nesting. Every deep traversal —
    /// the flag/wave producer scans, the checkpoint/stealth collector, the l10n
    /// string inventory, and emission — walks the tree through it (see
    /// [`Self::visit_deep`]), so a new nesting site is picked up everywhere at
    /// once and no walker can silently miss a list (the class of bug where a
    /// `set-flag`/`set-checkpoint` nested in a `sequence` was skipped).
    pub fn nested_effect_lists(&self) -> Vec<&[QuestEffect]> {
        match self {
            QuestEffect::Sequence { steps } => steps.iter().map(|s| s.effects.as_slice()).collect(),
            QuestEffect::SetCheckpoint { on_respawn, .. } => vec![on_respawn.as_slice()],
            QuestEffect::BeginStealth { on_caught, .. } => vec![on_caught.as_slice()],
            QuestEffect::MoveActor { on_arrive, .. } => vec![on_arrive.as_slice()],
            _ => Vec::new(),
        }
    }

    /// Visit `self` and every transitively nested effect (depth-first, pre-order),
    /// descending through [`Self::nested_effect_lists`].
    pub fn visit_deep<'a>(&'a self, f: &mut dyn FnMut(&'a QuestEffect)) {
        f(self);
        for list in self.nested_effect_lists() {
            for e in list {
                e.visit_deep(f);
            }
        }
    }

    /// Each nested effect list ([`Self::nested_effect_lists`]) paired with the
    /// **stable key segment** used to derive child l10n keys / diagnostic paths, and
    /// exposed mutably so the localization pass can rewrite nested player-visible
    /// strings in place. Segments: `seq.<step>` for each sequence step (step index),
    /// `respawn` for `set-checkpoint.on_respawn`, `caught` for
    /// `begin-stealth.on_caught`, `arrive` for `move-actor.on_arrive`. Kept in
    /// lockstep with `nested_effect_lists` (same lists, same order) — the position-
    /// derived segments make every derived key deterministic and stable across
    /// builds (ADR-0006 byte-identity).
    pub fn nested_effect_lists_keyed_mut(&mut self) -> Vec<(String, &mut [QuestEffect])> {
        match self {
            QuestEffect::Sequence { steps } => steps
                .iter_mut()
                .enumerate()
                .map(|(s, st)| (format!("seq.{s}"), st.effects.as_mut_slice()))
                .collect(),
            QuestEffect::SetCheckpoint { on_respawn, .. } => {
                vec![("respawn".to_string(), on_respawn.as_mut_slice())]
            }
            QuestEffect::BeginStealth { on_caught, .. } => {
                vec![("caught".to_string(), on_caught.as_mut_slice())]
            }
            QuestEffect::MoveActor { on_arrive, .. } => {
                vec![("arrive".to_string(), on_arrive.as_mut_slice())]
            }
            _ => Vec::new(),
        }
    }

    /// `true` if this is a `narrate` carrying the v0.6 `art` style (reserved
    /// `DW0141` under a pre-0.6 campaign; glyph-checked `DW0328`).
    pub fn narrate_art(&self) -> bool {
        matches!(
            self,
            QuestEffect::Narrate {
                style: Some(NarrateStyle::Art),
                ..
            }
        )
    }

    /// The `narrate` line's text if this is a `narrate` with the `art` style.
    pub fn narrate_art_text(&self) -> Option<&str> {
        match self {
            QuestEffect::Narrate {
                text,
                style: Some(NarrateStyle::Art),
                ..
            } => Some(text.as_str()),
            _ => None,
        }
    }

    /// Every vanilla sound-event id this effect references, for registry
    /// validation (`DW0326`): a `play-sound`'s `sound`, and a `narrate`'s optional
    /// `sound`. Returns `(subpath, id)` pairs where `subpath` locates the field
    /// within the effect (e.g. `sound`).
    pub fn sound_refs(&self) -> Vec<(&'static str, &str)> {
        match self {
            QuestEffect::PlaySound { sound, .. } => vec![("sound", sound.as_str())],
            QuestEffect::Narrate { sound: Some(s), .. } => vec![("sound", s.as_str())],
            _ => Vec::new(),
        }
    }

    /// The deferred `play-sound` `at: actor` id, if this effect is a `play-sound`
    /// targeting an actor (rejected `DW0335` until the actors surface lands).
    pub fn play_sound_actor(&self) -> Option<&str> {
        match self {
            QuestEffect::PlaySound {
                at: Some(SoundAt::Actor { actor }),
                ..
            } => Some(actor.as_str()),
            _ => None,
        }
    }

    /// The per-effect flag gate (DSL v0.6, task #55): flags that must ALL be set
    /// (per player) for this effect to fire. Empty for an ungated effect and for
    /// the verbs that are not per-effect gatable — terminal `campaign-complete`
    /// and the party/session-global `set-checkpoint` / `begin-stealth` /
    /// `end-stealth`. Emission wraps a gated effect's commands in a per-player
    /// `execute if score @s dw.f_<flag> matches 1` guard; a pre-0.6 campaign that
    /// gates any effect is rejected (`DW0141`).
    pub fn requires_flags(&self) -> &[FlagId] {
        match self {
            QuestEffect::OpenGate { requires_flags, .. }
            | QuestEffect::CloseGate { requires_flags, .. }
            | QuestEffect::GiveItem { requires_flags, .. }
            | QuestEffect::SetFlag { requires_flags, .. }
            | QuestEffect::SpawnWave { requires_flags, .. }
            | QuestEffect::Narrate { requires_flags, .. }
            | QuestEffect::SetBlock { requires_flags, .. }
            | QuestEffect::DespawnNpc { requires_flags, .. }
            | QuestEffect::MoveNpc { requires_flags, .. }
            | QuestEffect::Cutscene { requires_flags, .. }
            | QuestEffect::SetTime { requires_flags, .. }
            | QuestEffect::SetWeather { requires_flags, .. }
            | QuestEffect::PlaySound { requires_flags, .. } => requires_flags,
            // Terminal / party- or session-global verbs are not per-effect
            // gatable: `campaign-complete` is terminal; `set-checkpoint`
            // (`spawnpoint @a`) / `begin-stealth` / `end-stealth` are party-wide
            // session state; and the actor staging verbs (`spawn-actor` /
            // `despawn-actor` / `move-actor` / `unleash-actor` / `sequence`) are
            // world-global staging — none are per-player `@s` effects. Gate these
            // at the objective / dialogue-option level instead.
            QuestEffect::CampaignComplete
            | QuestEffect::SetCheckpoint { .. }
            | QuestEffect::BeginStealth { .. }
            | QuestEffect::EndStealth
            | QuestEffect::SpawnActor { .. }
            | QuestEffect::DespawnActor { .. }
            | QuestEffect::MoveActor { .. }
            | QuestEffect::UnleashActor { .. }
            | QuestEffect::Sequence { .. } => &[],
        }
    }

    /// The actor id this effect targets, if it is one of the actor staging effects
    /// (`spawn-actor`/`despawn-actor`/`move-actor`/`unleash-actor`). `sequence` has
    /// no single actor (its nested effects each carry their own).
    pub fn actor_ref(&self) -> Option<&ActorId> {
        match self {
            QuestEffect::SpawnActor { actor }
            | QuestEffect::DespawnActor { actor, .. }
            | QuestEffect::MoveActor { actor, .. }
            | QuestEffect::UnleashActor { actor } => Some(actor),
            _ => None,
        }
    }
}
