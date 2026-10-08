//! Stage 2 — NPCs: who they are, what they look like and where they stand.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::serde_fields::{is_false, is_zero3};
use crate::{AnchorId, AreaId, BodyTraversal, NpcId};

#[cfg(doc)]
use crate::{EncounterTier, Mark, Verb};

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
    /// Integer `[x, y, z]` block offset from `anchor` (spec-0066, default
    /// `[0, 0, 0]`): the NPC stands at the [`Mark`] the two fields spell.
    #[serde(default, skip_serializing_if = "is_zero3")]
    pub offset: [i32; 3],
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
    /// Deferred entrance (DSL v0.6): when `true` the NPC is **not** summoned at
    /// world init — its body and interaction hitbox only appear when a
    /// [`Verb::SpawnNpc`] fires, at this same `anchor`. The dual of
    /// `despawn-npc`: a character with a scripted entrance must not stand at its
    /// mark as a statue from minute one. A deferred NPC that no `spawn-npc` ever
    /// spawns is unreachable content (`DW0197`). Default `false` = summoned at
    /// init, byte-identical to pre-0.6.
    #[serde(default, skip_serializing_if = "is_false")]
    pub deferred: bool,
    /// What this body can do when it moves (DSL v0.11, spec-0034). Absent = the
    /// class the compiler derives from `base_entity` (or from `minecraft:mannequin`
    /// when `skin` is set — the body that actually ships). See [`BodyTraversal`]:
    /// the declaration must change a verdict or it is `DW0454`, and it can never
    /// reach the error tier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub traversal: Option<BodyTraversal>,
}

/// A mannequin NPC's player-model skin (DSL v0.4). The skin PNG is sourced from
/// the campaign dir's `skins/<texture_id>.png` and ships in the per-delve resource
/// pack at `assets/delvewright/textures/npc/<campaign_id>/<texture_id>.png`, which
/// is what the mannequin's `profile.texture` resolves to. The delve's own
/// directory is stamped on at emission ([`crate::l10n::namespace_skin_textures`])
/// — a client merges every applied pack's textures into ONE space, so two delves
/// that both cast a `keeper` would otherwise wear each other's faces. Nothing a
/// creator writes or names on disk carries it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NpcSkin {
    /// Skin id: the PNG basename under `skins/`, and the last segment of the
    /// resource-pack texture path (a bare kebab token; validated by `DW0190`).
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

/// A structured casting sheet. Structure lives in the
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

/// What a speaking part does. A schema enum offers what the engine accepts,
/// so there are two of them: how hard a fight is billed is [`EncounterTier`] on
/// the body that fights (a `waves[]` entry or a stage-5 actor), not a role here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    /// Gives and advances quests.
    QuestGiver,
    /// Flavor only.
    Flavor,
}
