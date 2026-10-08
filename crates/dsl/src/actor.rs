//! Stage 5 — scripted actors (DSL v0.6, spec-0014): bodies the story moves.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::serde_fields::{is_false, is_zero3};
use crate::{
    ActorId, AnchorId, BodyTraversal, EncounterTier, MobAttributes, MobDrop, MobEquipment, NpcSkin,
    OnKill, QuestEffect,
};

#[cfg(doc)]
use crate::{Mark, Npc, Verb, Wave};

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
    /// Integer `[x, y, z]` block offset from `anchor` (spec-0066, default
    /// `[0, 0, 0]`): the puppet stands at the [`Mark`] the two fields spell, so
    /// a rank of bodies is one anchor and an offset apiece.
    #[serde(default, skip_serializing_if = "is_zero3")]
    pub offset: [i32; 3],
    /// Initial facing (default `south`). The puppet spawns yawed this way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facing: Option<Facing>,
    /// If `true`, the puppet is damageable (a tower-defense creep) but stays
    /// knockback-immune; default `false` (fully `Invulnerable`).
    #[serde(default, skip_serializing_if = "is_false")]
    pub vulnerable: bool,
    /// Gear the actor wears and holds, in the same shape a wave mob uses
    /// ([`MobEquipment`]). Emitted into BOTH the staged puppet and the
    /// unleashed twin, so the dormant elite the player has been circling is
    /// visibly the same armoured thing that stands up. Drop chances are zero —
    /// wave gear and actor gear are never farmable (no-grind constitution).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equipment: Option<MobEquipment>,
    /// Attribute overrides, in the same shape a wave mob uses ([`MobAttributes`],
    /// the v0.4 surface — one type, one rule set, so the two surfaces cannot
    /// drift). Emitted into BOTH the staged puppet and the unleashed twin, so the
    /// elite the party fights is the elite the author tuned; without it an actor
    /// was stuck at vanilla base values while every wave mob could be tuned,
    /// which is what blocked elite authoring. A `vulnerable` actor's
    /// knockback-immunity is emitted first and is not authorable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attributes: Option<MobAttributes>,
    /// How hard this actor's fight is *meant* to be (DSL v0.8, spec-0023) — the
    /// same [`EncounterTier`] vocabulary a [`Wave`] declares. Absent =
    /// [`EncounterTier::Ordinary`], byte-identical to every pre-0.8 campaign.
    ///
    /// A wave is not the only shape an elite takes. The set-piece souls fight —
    /// the armoured thing kneeling among the graves that stands up when you hit
    /// it — is an **actor**: staged by `spawn-actor`, given AI by
    /// `unleash-actor`, killed by hand rather than by a `kill` objective. Before
    /// this field nothing anywhere stated what such a fight was billed as.
    ///
    /// Like the wave field this is a **declaration, not a knob**: the compiler
    /// never scales an actor from it, and emission is unchanged whichever tier is
    /// declared. Its readers are the health-bar advisory (`DW0912`) and the drop
    /// rule — only a billed fight leaves anything behind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<EncounterTier>,
    /// A health bar over this actor's fight (DSL v0.31, spec-0073) — the same
    /// [`HealthBar`](crate::healthbar::HealthBar) a [`Wave`] declares. It reads the
    /// bodies whose health can move: the unleashed twin, or the puppet itself when
    /// the actor is `vulnerable`. A bar on an actor that is neither is `DW0909`.
    /// Absent = no bar, byte-identical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health_bar: Option<crate::healthbar::HealthBar>,
    /// What this actor leaves behind when a player kills it. Only an
    /// `elite`/`boss` actor may
    /// declare it (`DW0491`). Emitted into BOTH the staged puppet and the
    /// unleashed twin, exactly as `equipment` is — the drop belongs to the body,
    /// not to one of its two lifecycles. A `despawn-actor` strips the
    /// declaration off the body before removing it, so re-caging an elite (a
    /// souls re-seat) never scatters its axe.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drops: Vec<MobDrop>,
    /// What this body can do when it moves (DSL v0.11, spec-0034) — the same
    /// [`BodyTraversal`] a stage-2 [`Npc`] carries, because traversal belongs to
    /// the body and not to the stage that declares it. Absent = the class the
    /// compiler derives from `entity` (or from `minecraft:mannequin` when `skin`
    /// is set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub traversal: Option<BodyTraversal>,
    /// What happens each time a player is credited with killing this actor's
    /// body (spec-0074) — effect root R9, the same [`OnKill`] a
    /// wave declares. Absent = no bundle, and the actor's emission is
    /// byte-identical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_kill: Option<OnKill>,
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
    /// The body leaves unseen: no death animation, red flash or death particles
    /// where it stood. It is moved under the world and removed there.
    Vanish,
    /// The body dies where it stands, with the vanilla death animation (a death
    /// the player is meant to watch).
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

/// One step of a [`Verb::Sequence`] (DSL v0.6): a group of effects fired at
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
