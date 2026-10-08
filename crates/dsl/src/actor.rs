//! Stage 5 — scripted actors (DSL v0.6, spec-0014): bodies the story moves.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::serde_fields::{is_false, is_zero3};
use crate::{
    ActorId, AnchorId, BodyTraversal, EncounterTier, MobAttributes, MobDrop, MobEquipment, NpcSkin,
    OnKill, QuestEffect,
};

#[cfg(doc)]
use crate::{Mark, Npc, Wave};

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

// ---------------------------------------------------------------------------
// Validation — the checks `dsl::validate` runs over this object (ADR-0031)
// ---------------------------------------------------------------------------

use crate::Verb;
use crate::diagnostic::{Diagnostic, codes};
use crate::envelope::Campaign;
use crate::ids::is_kebab;
use crate::quest::check::check_no_nested_sequence;
use crate::registry::{AnchorRegistry, EntityRegistry, ItemRegistry};
use crate::validate::{AnchorProviders, station_kind_diag};
use crate::wave::check_equipment;
use std::collections::BTreeSet;

/// Recursively visit every effect in `effs`, descending into every nested effect
/// list ([`QuestEffect::nested_effect_lists`]: `sequence` steps, `set-checkpoint`
/// `on_respawn`, `begin-stealth` `on_caught`, `move-actor` / `move-npc`
/// `on_arrive`).
fn walk_effects_deep(effs: &[QuestEffect], f: &mut dyn FnMut(&QuestEffect)) {
    for e in effs {
        e.visit_deep(f);
    }
}

/// Actor declarations (spec-0014): a known entity id, a well-formed and distinct
/// skin, a spawn anchor some area provides — and every actor staging effect's
/// reference (`DW0112`), every `move-actor` destination anchor (`DW0142`) and the
/// no-nested-`sequence` rule (`DW0329`) at every depth of a quest's or a
/// trigger's bundles.
pub(crate) fn actor_checks(
    c: &Campaign,
    anchors: &dyn AnchorRegistry,
    entities: &dyn EntityRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let quests = &c.quests.content;
    let declared: BTreeSet<&str> = quests.actors.iter().map(|a| a.id.as_str()).collect();

    // Anchor names provided by single-prefab areas (pool areas resolve anchors in
    // the compiler, so their presence defers the check — mirroring `DW0142`'s
    // single-prefab-only scope; never a false positive).
    let providers = AnchorProviders::build(c, anchors);

    // Actor declarations: entity id, skin, spawn anchor.
    let mut seen_skins: BTreeSet<&str> = BTreeSet::new();
    for (i, a) in quests.actors.iter().enumerate() {
        if !entities.contains(&a.entity) {
            d.push(Diagnostic::error(
                codes::ENTITY_UNKNOWN,
                "quests",
                format!("/content/actors/{i}/entity"),
                format!(
                    "actor entity `{}` is not a known 1.21.11 entity id — use a valid namespaced \
                     entity id (e.g. `minecraft:warden`)",
                    a.entity
                ),
            ));
        }
        if let Some(skin) = &a.skin {
            if !is_kebab(&skin.texture_id) {
                d.push(Diagnostic::error(
                    codes::SKIN_INVALID,
                    "quests",
                    format!("/content/actors/{i}/skin/texture_id"),
                    format!(
                        "actor skin `texture_id` `{}` is malformed — it must be a bare kebab token \
                         (e.g. `giant-idle`), matching the `skins/<texture_id>.png` filename",
                        skin.texture_id
                    ),
                ));
            } else if !seen_skins.insert(skin.texture_id.as_str()) {
                d.push(Diagnostic::error(
                    codes::SKIN_INVALID,
                    "quests",
                    format!("/content/actors/{i}/skin/texture_id"),
                    format!(
                        "duplicate actor skin `texture_id` `{}` — each mannequin needs a distinct \
                         texture; rename one (and its `skins/<id>.png`)",
                        skin.texture_id
                    ),
                ));
            }
        }
        if let Some(f) = station_kind_diag(
            &providers,
            a.anchor.as_str(),
            crate::layout::StationKind::Point,
            "an actor's station",
            "quests",
            format!("/content/actors/{i}/anchor"),
        ) {
            d.push(f);
        }
        if !providers.resolvable(a.anchor.as_str()) {
            d.push(Diagnostic::error(
                codes::ANCHOR_UNRESOLVED,
                "quests",
                format!("/content/actors/{i}/anchor"),
                format!(
                    "actor anchor `{}` is not provided by any area's prefab — {}",
                    a.anchor,
                    providers.anchor_remedy(
                        "use an anchor a prefab exposes, or bind a prefab/pool that carries it"
                    ),
                ),
            ));
        }
    }

    // Effect-level: actor references (DW0112), move-actor destination anchors
    // (DW0142), and the no-nested-sequence rule (DW0329). Deep-walk so effects
    // nested in a `sequence` / `move-actor` `on_arrive` are covered.
    let mut groups: Vec<(String, &[QuestEffect])> = Vec::new();
    for (i, q) in quests.quests.iter().enumerate() {
        for (key, effs) in &q.on_objective_complete {
            groups.push((
                format!("/content/quests/{i}/on_objective_complete/{key}"),
                effs.as_slice(),
            ));
        }
        groups.push((
            format!("/content/quests/{i}/on_complete"),
            q.on_complete.as_slice(),
        ));
    }
    for (i, t) in quests.triggers.iter().enumerate() {
        groups.push((
            format!("/content/triggers/{i}/effects"),
            t.effects.as_slice(),
        ));
    }
    for (path, effs) in &groups {
        let mut visit = |e: &QuestEffect| {
            if let Some(actor) = e.actor_ref()
                && !declared.contains(actor.as_str())
            {
                d.push(Diagnostic::error(
                    codes::DANGLING_REF,
                    "quests",
                    path.clone(),
                    format!(
                        "actor staging effect references unknown actor `{actor}` — declare it in \
                         the stage-5 `actors` list, or fix the reference"
                    ),
                ));
            }
            if let Verb::MoveActor { to, .. } = &e.verb
                && let Some(f) = station_kind_diag(
                    &providers,
                    to.anchor.as_str(),
                    crate::layout::StationKind::Point,
                    "a `move-actor` destination",
                    "quests",
                    path.clone(),
                )
            {
                d.push(f);
            } else if let Verb::MoveActor { to, .. } = &e.verb
                && !providers.resolvable(to.anchor.as_str())
            {
                d.push(Diagnostic::error(
                    codes::ANCHOR_UNRESOLVED,
                    "quests",
                    path.clone(),
                    format!(
                        "move-actor destination anchor `{}` is not provided by any \
                         area's prefab — {}",
                        to.anchor,
                        providers.anchor_remedy("use an anchor a prefab exposes"),
                    ),
                ));
            }
        };
        walk_effects_deep(effs, &mut visit);
        check_no_nested_sequence(effs, path, d);
    }
}

/// Actor `equipment` (spec-0021): item ids and enchantments, by the wave mob's
/// rule ([`crate::wave::check_equipment`]).
pub(crate) fn actor_equipment_checks(
    c: &Campaign,
    items: &dyn ItemRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let quests = &c.quests.content;
    // Actor `equipment` (spec-0021): the same shape, the same registries, the
    // same diagnostics as a wave mob's — one surface, one rule set.
    for (i, a) in quests.actors.iter().enumerate() {
        let Some(eq) = &a.equipment else { continue };
        check_equipment(
            eq,
            "actor",
            &format!("/content/actors/{i}/equipment"),
            items,
            d,
        );
    }
}
