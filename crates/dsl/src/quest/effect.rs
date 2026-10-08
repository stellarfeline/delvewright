//! An effect: a verb with its guards, audience and happening, and the bonfire
//! dialog's canonical labels.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    ActorId, AnchorId, CameraShot, CameraSubject, Carrier, CutsceneParty,
    DEFAULT_COLLAPSE_FALLING_BLOCK, DEFAULT_VOLLEY_INTERVAL, DEFAULT_VOLLEY_PROJECTILE,
    DEFAULT_VOLLEY_SALVOS, FlagId, Happening, HappeningSubject, Mark, NarrateStyle, NpcId,
    ParticleAt, PrefabId, SoundAt, StateCompare, StateId, StateWrite, StationKind, StealthZone,
    Verb, WaveId, WorldTime, WorldWeather,
};

#[cfg(doc)]
use crate::BodyRef;

/// The canonical English title of the bonfire rest dialog. Baked at emit time
/// when the campaign authors no `prompt`, in the
/// `world.boundary.message` tradition: a compiler default is not inventoried, an
/// authored line is — so a delve that wants this sentence in `zh-cn` authors it.
pub const BONFIRE_PROMPT_EN: &str = "Bonfire";
/// The canonical English label of the **rest and save** option.
pub const BONFIRE_REST_LABEL_EN: &str = "Rest and save";
/// The canonical English label of the **save only** option.
pub const BONFIRE_SAVE_LABEL_EN: &str = "Save only";

/// A `bonfire`'s authored rest-dialog strings, each `None` when the campaign
/// leaves the compiler's canonical English in place
/// ([`QuestEffect::bonfire_labels`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BonfireLabels<'a> {
    /// Dialog title; `None` → [`BONFIRE_PROMPT_EN`].
    pub prompt: Option<&'a str>,
    /// **Rest and save** button label; `None` → [`BONFIRE_REST_LABEL_EN`].
    pub rest_label: Option<&'a str>,
    /// **Save only** button label; `None` → [`BONFIRE_SAVE_LABEL_EN`].
    pub save_label: Option<&'a str>,
    /// **Rest and save** button hover tooltip (spec-0078); `None` → none emitted.
    pub rest_tooltip: Option<&'a str>,
    /// **Save only** button hover tooltip (spec-0078); `None` → none emitted.
    pub save_tooltip: Option<&'a str>,
}

impl BonfireLabels<'_> {
    /// The dialog title actually emitted.
    pub fn prompt_or_default(&self) -> &str {
        self.prompt.unwrap_or(BONFIRE_PROMPT_EN)
    }
    /// The **rest and save** label actually emitted.
    pub fn rest_or_default(&self) -> &str {
        self.rest_label.unwrap_or(BONFIRE_REST_LABEL_EN)
    }
    /// The **save only** label actually emitted.
    pub fn save_or_default(&self) -> &str {
        self.save_label.unwrap_or(BONFIRE_SAVE_LABEL_EN)
    }
}

/// The condition under which an effect fires, as one object.
///
/// The three axes of [`crate::gate::Gate`] in their declared form: the flags that
/// must be set, the flags that must not be, and the numeric comparisons that must
/// hold. Declared once and carried by [`QuestEffect::when`], so every verb is
/// gatable on exactly the same terms and a fourth axis is one field here.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Guard {
    /// Flags that must ALL be set (per party) for the effect to fire. Emission
    /// wraps the effect's commands in `execute if score #party dw.f_<flag> matches 1`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_flags: Vec<FlagId>,
    /// Flags whose being set SUPPRESSES the effect — the dual of `requires_flags`,
    /// emitted as `execute unless score #party dw.f_<flag> matches 1`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forbids_flags: Vec<FlagId>,
    /// Numeric comparisons that must ALL hold (spec-0031), emitted as
    /// `execute if score <holder> dw.s_<state> matches <range>`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_state: Vec<StateCompare>,
}

/// An effect fired by quest progress: **one guard, one story note, one verb.**
///
/// The guard is a property of an effect, not of the verb that first wanted one, so
/// it is declared once in [`Guard`] and every verb carries it — including the
/// staging and souls vocabulary (`spawn-actor`, `move-actor`, `set-checkpoint`,
/// `bonfire`, `begin-stealth`, `sequence`, …) that could not be branch-gated while
/// the fields lived on the variants. A gate that can never open on a
/// `campaign-complete` is caught where it belongs, by the completability proof.
///
/// `verb` is `#[serde(flatten)]`, so the JSON is unchanged in shape apart from the
/// guard moving under `when`: `{"type": "open-gate", "anchor": "…", "when":
/// {"requires_flags": ["…"]}}`. [`Verb`] keeps `deny_unknown_fields`, which is what
/// the flattened deserializer applies to everything the outer struct did not claim
/// — an author's typo is still `DW0100`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct QuestEffect {
    /// When this effect fires. `None` is the always-open gate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Guard>,
    /// What this beat does to the story (spec-0025) — validation metadata with no
    /// emission of its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub happening: Option<Happening>,
    /// **Who a player-facing effect addresses** (spec-0085 §3.2): `party` or
    /// `actor`, the one player whose act fired the root. Absent = the root's own
    /// answer — a quest completion addresses the party, a death, a respawn, a
    /// purchase, a credited kill and a `presser` trigger address their actor, a
    /// polled trigger, a trap and a shortcut address the party — so a campaign
    /// that never writes the field emits exactly what it emitted before it
    /// existed.
    ///
    /// A property of the envelope rather than of any verb: it reaches every
    /// verb the emitter addresses to players ([`Verb::addresses_players`]), and
    /// on any other verb it is refused (`DW0942`). `actor` where emission has no
    /// acting player is `DW0503`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audience: Option<EffectAudience>,
    /// **Narrows the audience to players inside an anchor-centred box**
    /// (`anchor ± extent`, spec-0085 §3.2) at the moment the effect fires — the
    /// same [`StealthZone`] a `begin-stealth` zone and a `lethal_volumes[]`
    /// region take, resolved through the one `Plan::zone_box`. Composes with
    /// [`Self::audience`]: `actor` + `in` is the actor, if they stand in the box.
    ///
    /// One field on the envelope, reaching every player-facing verb; refused on
    /// any other (`DW0942`) — a box narrows an audience, and a world fact has
    /// none.
    #[serde(default, rename = "in", skip_serializing_if = "Option::is_none")]
    pub within: Option<StealthZone>,
    /// What the effect does.
    #[serde(flatten)]
    pub verb: Verb,
}

/// **Who a player-facing effect addresses** (spec-0085 §3.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum EffectAudience {
    /// Every player — `@a`.
    Party,
    /// The one player whose act fired the root — the completing player, the
    /// presser, the dying or respawning player, the buyer, the credited killer.
    /// Emitted `@s`, so it exists only where emission has an acting player
    /// (`DW0503`).
    Actor,
}

impl EffectAudience {
    /// The token the document spells.
    pub fn token(self) -> &'static str {
        match self {
            EffectAudience::Party => "party",
            EffectAudience::Actor => "actor",
        }
    }
}

impl From<Verb> for QuestEffect {
    /// An unguarded effect with no story note and the root's own audience — the
    /// shape a compiler-synthesized beat and most tests want.
    fn from(verb: Verb) -> Self {
        QuestEffect {
            when: None,
            happening: None,
            audience: None,
            within: None,
            verb,
        }
    }
}

/// **How a nested effect list is dispatched, relative to the bundle it sits in**
/// (spec-0085 §3.2) — the one statement of which command source each nesting
/// site runs under, read by `DW0357`/`DW0503` and by the emitter's timeline
/// keying alike.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NestedDispatch {
    /// Under the parent's own source: a `sequence` step. A timeline started
    /// where there is an acting player carries that player across its
    /// `schedule`s by a compiler-owned tag, and a timeline started from the
    /// server source has nobody to carry.
    Inherit,
    /// As one player, whatever the parent was: a `set-checkpoint`'s
    /// `on_respawn` (the respawning player) and a `begin-stealth`'s `on_caught`
    /// (the spotted player).
    Player,
    /// From the server command source, whatever the parent was: a
    /// `move-npc`/`move-actor` `on_arrive` (the driver's scheduled tick) and a
    /// `bonfire`'s `on_rest` (the party-wide rest, dispatched from the tick).
    Server,
}

impl NestedDispatch {
    /// Whether a list dispatched this way, inside a bundle that does (or does
    /// not) have an acting player, has one.
    pub fn has_actor(self, parent_has_actor: bool) -> bool {
        match self {
            NestedDispatch::Inherit => parent_has_actor,
            NestedDispatch::Player => true,
            NestedDispatch::Server => false,
        }
    }
}

impl QuestEffect {
    /// Each nested effect list with how it is dispatched ([`NestedDispatch`]) —
    /// the same lists, in the same order, as [`Self::nested_effect_lists`].
    pub fn nested_effect_dispatch(&self) -> Vec<(&[QuestEffect], NestedDispatch)> {
        match &self.verb {
            Verb::Sequence { steps } => steps
                .iter()
                .map(|s| (s.effects.as_slice(), NestedDispatch::Inherit))
                .collect(),
            Verb::SetCheckpoint { on_respawn, .. } => {
                vec![(on_respawn.as_slice(), NestedDispatch::Player)]
            }
            Verb::Bonfire { on_rest, .. } => vec![(on_rest.as_slice(), NestedDispatch::Server)],
            Verb::BeginStealth { on_caught, .. } => {
                vec![(on_caught.as_slice(), NestedDispatch::Player)]
            }
            Verb::MoveActor { on_arrive, .. } | Verb::MoveNpc { on_arrive, .. } => {
                vec![(on_arrive.as_slice(), NestedDispatch::Server)]
            }
            _ => Vec::new(),
        }
    }

    /// Whether this effect addresses players at all — [`Verb::addresses_players`].
    pub fn addresses_players(&self) -> bool {
        self.verb.addresses_players()
    }
    /// The gate anchor if this is `open-gate`.
    pub fn open_gate_anchor(&self) -> Option<&AnchorId> {
        match &self.verb {
            Verb::OpenGate { anchor, .. } => Some(anchor),
            _ => None,
        }
    }

    /// The gate anchor if this is `close-gate` (DSL v0.6).
    pub fn close_gate_anchor(&self) -> Option<&AnchorId> {
        match &self.verb {
            Verb::CloseGate { anchor, .. } => Some(anchor),
            _ => None,
        }
    }

    /// The authored `sealed_hint` if this is a `close-gate` that declares one (DSL
    /// v0.8). `None` for every other effect **and** for a `close-gate` that takes
    /// the compiler's canonical English seal line.
    pub fn close_gate_sealed_hint(&self) -> Option<&str> {
        match &self.verb {
            Verb::CloseGate { sealed_hint, .. } => sealed_hint.as_deref(),
            _ => None,
        }
    }

    /// The wave id if this is `spawn-wave` (v0.3).
    pub fn spawn_wave(&self) -> Option<&WaveId> {
        match &self.verb {
            Verb::SpawnWave { wave, .. } => Some(wave),
            _ => None,
        }
    }

    /// The flag id if this is `set-flag` (v0.3).
    pub fn set_flag(&self) -> Option<&FlagId> {
        match &self.verb {
            Verb::SetFlag { flag, .. } => Some(flag),
            _ => None,
        }
    }

    /// The item id if this is `give-item` (v0.3).
    pub fn give_item(&self) -> Option<&str> {
        match &self.verb {
            Verb::GiveItem { item, .. } => Some(item),
            _ => None,
        }
    }

    /// `true` if this is a `give-item` carrying a v0.4 display `name`.
    pub fn give_item_named(&self) -> bool {
        matches!(self.verb, Verb::GiveItem { name: Some(_), .. })
    }

    /// The declared `carrier` if this is a `give-item` that states one (v0.6,
    /// spec-0018). `None` for any other effect **and** for a `give-item` that
    /// leaves it absent — absent reads as [`Carrier::All`], and the distinction
    /// matters only for the pre-0.6 reserved-field gate.
    pub fn give_carrier(&self) -> Option<Carrier> {
        match &self.verb {
            Verb::GiveItem { carrier, .. } => *carrier,
            _ => None,
        }
    }

    /// Does this `give-item` hand a single copy to the acting player (v0.6,
    /// spec-0018)? `false` for every other effect and for the party-wide default.
    pub fn gives_to_one(&self) -> bool {
        matches!(self.give_carrier(), Some(Carrier::One))
    }

    /// The `set-block` block id if this is a v0.4 `set-block` effect.
    pub fn set_block(&self) -> Option<(&AnchorId, &str)> {
        match &self.verb {
            Verb::SetBlock { anchor, block, .. } => Some((anchor, block.as_str())),
            _ => None,
        }
    }

    /// The NPC id if this is a v0.4 `despawn-npc` effect.
    pub fn despawn_npc(&self) -> Option<&NpcId> {
        match &self.verb {
            Verb::DespawnNpc { npc, .. } => Some(npc),
            _ => None,
        }
    }

    /// `(npc, to)` if this is a v0.4 `move-npc` effect.
    pub fn move_npc(&self) -> Option<(&NpcId, &Mark)> {
        match &self.verb {
            Verb::MoveNpc { npc, to, .. } => Some((npc, to)),
            _ => None,
        }
    }

    /// The v0.3 effect name if this effect is one introduced in DSL v0.3
    /// (`give-item`/`set-flag`/`spawn-wave`). These validate in v0.3 campaigns
    ///.
    pub fn v03_effect(&self) -> Option<&'static str> {
        match &self.verb {
            Verb::GiveItem { .. } => Some("give-item"),
            Verb::SetFlag { .. } => Some("set-flag"),
            Verb::SpawnWave { .. } => Some("spawn-wave"),
            Verb::OpenGate { .. }
            | Verb::CloseGate { .. }
            | Verb::CampaignComplete { .. } => None,
            // v0.4 effects report via `v04_effect`; v0.5 via `v05_effect`; they
            // are not v0.3 verbs.
            Verb::Narrate { .. }
            | Verb::SetBlock { .. }
            | Verb::DespawnNpc { .. }
            | Verb::MoveNpc { .. }
            | Verb::Cutscene { .. }
            | Verb::SetTime { .. }
            | Verb::SetWeather { .. }
            | Verb::PlaySound { .. }
            | Verb::DamagePlayers { .. }
            | Verb::SetCheckpoint { .. }
            | Verb::Bonfire { .. }
            | Verb::BeginStealth { .. }
            | Verb::EndStealth
            | Verb::SpawnActor { .. }
            | Verb::DespawnActor { .. }
            | Verb::MoveActor { .. }
            | Verb::UnleashActor { .. }
            | Verb::SpawnNpc { .. }
            | Verb::Sequence { .. }
            // spec-0022 trap-payload verbs are v0.6 — they report via `v06_effect`.
            | Verb::Volley { .. }
            | Verb::Collapse { .. }
            // spec-0031's state verbs, region writes, status effects and teleport,
            // and spec-0032's `drop-stake`, are all v0.10 — they report via
            // `v10_effect`.
            | Verb::SetState { .. }
            | Verb::AddState { .. }
            | Verb::ClearState { .. }
            | Verb::FillRegion { .. }
            | Verb::ClearRegion { .. }
            | Verb::SetAtmosphere { .. }
            // spec-0042's `open-way` is v0.12 — it reports via `v12_effect`.
            | Verb::OpenWay { .. }
            | Verb::GiveEffect { .. }
            | Verb::ClearEffect { .. }
            | Verb::Teleport { .. }
            // spec-0068's `firework` is v0.29.
            | Verb::Firework { .. }
            // spec-0082's assembly verbs.
            | Verb::SpawnAssembly { .. }
            | Verb::DespawnAssembly { .. }
            | Verb::PlayClip { .. }
            // spec-0094's `arm-strikes`.
            | Verb::ArmStrikes { .. }
            // spec-0085's `particle`.
            | Verb::Particle { .. }
            // spec-0092's `lightning`.
            | Verb::Lightning { .. }
            | Verb::DropStake { .. } => None,
        }
    }

    /// The v0.4 effect name if this effect is one introduced in DSL v0.4
    /// (`narrate`/`set-block`/`despawn-npc`/`move-npc`/`cutscene`). These validate
    /// in v0.4 campaigns.
    pub fn v04_effect(&self) -> Option<&'static str> {
        match &self.verb {
            Verb::Narrate { .. } => Some("narrate"),
            Verb::SetBlock { .. } => Some("set-block"),
            Verb::DespawnNpc { .. } => Some("despawn-npc"),
            Verb::MoveNpc { .. } => Some("move-npc"),
            Verb::Cutscene { .. } => Some("cutscene"),
            _ => None,
        }
    }

    /// The v0.5 effect name if this effect is one introduced in DSL v0.5
    /// (`set-time`/`set-weather`, spec-0010).
    pub fn v05_effect(&self) -> Option<&'static str> {
        match &self.verb {
            Verb::SetTime { .. } => Some("set-time"),
            Verb::SetWeather { .. } => Some("set-weather"),
            _ => None,
        }
    }

    /// The v0.6 effect name if this effect is one introduced in DSL v0.6
    /// (`set-checkpoint`, spec-0012; `begin-stealth`/`end-stealth`, spec-0014;
    /// `play-sound`, spec-0014; the scripted-actor staging verbs
    /// `spawn-actor`/`despawn-actor`/`move-actor`/`unleash-actor`/`sequence`,
    /// spec-0014). These validate in v0.6 campaigns
    /// earlier. (The `narrate` `art` style is a v0.6 addition to an existing verb
    /// — see [`QuestEffect::narrate_art`] — not a new effect.)
    pub fn v06_effect(&self) -> Option<&'static str> {
        match &self.verb {
            Verb::CloseGate { .. } => Some("close-gate"),
            Verb::SetCheckpoint { .. } => Some("set-checkpoint"),
            Verb::Bonfire { .. } => Some("bonfire"),
            Verb::BeginStealth { .. } => Some("begin-stealth"),
            Verb::EndStealth => Some("end-stealth"),
            Verb::PlaySound { .. } => Some("play-sound"),
            Verb::DamagePlayers { .. } => Some("damage-players"),
            Verb::SpawnActor { .. } => Some("spawn-actor"),
            Verb::DespawnActor { .. } => Some("despawn-actor"),
            Verb::MoveActor { .. } => Some("move-actor"),
            Verb::UnleashActor { .. } => Some("unleash-actor"),
            Verb::Sequence { .. } => Some("sequence"),
            Verb::SpawnNpc { .. } => Some("spawn-npc"),
            // spec-0022 trap-payload verbs — v0.6 surface, reserved earlier.
            Verb::Volley { .. } => Some("volley"),
            Verb::Collapse { .. } => Some("collapse"),
            _ => None,
        }
    }

    /// The v0.10 effect name if this effect is one introduced in DSL v0.10
    /// (`set-state`/`add-state`/`clear-state` and the region writes
    /// `fill-region`/`clear-region`, spec-0031). These validate in v0.10
    /// campaigns.
    pub fn v10_effect(&self) -> Option<&'static str> {
        match &self.verb {
            Verb::SetState { .. } => Some("set-state"),
            Verb::AddState { .. } => Some("add-state"),
            Verb::ClearState { .. } => Some("clear-state"),
            Verb::FillRegion { .. } => Some("fill-region"),
            Verb::ClearRegion { .. } => Some("clear-region"),
            Verb::GiveEffect { .. } => Some("give-effect"),
            Verb::ClearEffect { .. } => Some("clear-effect"),
            Verb::Teleport { .. } => Some("teleport"),
            Verb::DropStake { .. } => Some("drop-stake"),
            _ => None,
        }
    }

    /// The v0.12 effect name if this effect is one introduced in DSL v0.12
    /// (`open-way`, spec-0042).
    pub fn v12_effect(&self) -> Option<&'static str> {
        match &self.verb {
            Verb::OpenWay { .. } => Some("open-way"),
            _ => None,
        }
    }

    /// **The way this effect opens** (DSL v0.12, spec-0042): the placed piece and
    /// the name of one way that piece's spatial contract exports.
    ///
    /// The third spelling of the one region write, beside
    /// [`QuestEffect::region_write`] (the author's own box) and
    /// [`QuestEffect::gate_region_write`] (a gate anchor's box). It answers with a
    /// *reference* and never with geometry, because the geometry is not the
    /// campaign's to state: the cells, the block and the direction all live in the
    /// piece's metadata, and the compiler resolves them there
    /// (`compiler::ways`). A region-shaped accessor here would be the second
    /// authority this surface exists to avoid.
    pub fn way_write(&self) -> Option<(&PrefabId, &str)> {
        match &self.verb {
            Verb::OpenWay { piece, way, .. } => Some((piece, way.as_str())),
            _ => None,
        }
    }

    /// **The one region write**, whichever verb spelled it (DSL v0.10,
    /// spec-0031): the box to write and the block to write it with — `None` for
    /// the block meaning *clear to air*.
    ///
    /// This is the accessor the capability belongs to. It answers for
    /// `fill-region` / `clear-region`, which name their own box; `open-gate` /
    /// `close-gate` are the same operation over a box a prefab gate anchor
    /// declares, so they answer through
    /// [`QuestEffect::gate_region_write`](Self::gate_region_write) — the anchor
    /// is theirs, the *operation* is shared.
    pub fn region_write(&self) -> Option<(&StealthZone, Option<&str>)> {
        match &self.verb {
            Verb::FillRegion { region, block, .. } => Some((region, Some(block.as_str()))),
            Verb::ClearRegion { region, .. } => Some((region, None)),
            _ => None,
        }
    }

    /// The gate anchor this effect writes, and whether the write **fills** it:
    /// `Some((anchor, true))` for `close-gate`, `Some((anchor, false))` for
    /// `open-gate`, `None` for everything else.
    ///
    /// The gate half of [`QuestEffect::region_write`]: same operation, but the box
    /// and the fill block come from the prefab's gate anchor rather than from the
    /// author. Everything that reasons about runtime region writes reads both
    /// accessors and nothing else.
    pub fn gate_region_write(&self) -> Option<(&AnchorId, bool)> {
        match &self.verb {
            Verb::CloseGate { anchor, .. } => Some((anchor, true)),
            Verb::OpenGate { anchor, .. } => Some((anchor, false)),
            _ => None,
        }
    }

    /// `(projectile, from_anchor, kill_zone, salvos, interval)` if this is a
    /// `volley` (spec-0022), with the documented defaults already applied.
    pub fn volley(&self) -> Option<(&str, &AnchorId, &StealthZone, u32, u32)> {
        match &self.verb {
            Verb::Volley {
                projectile,
                from_anchor,
                kill_zone,
                salvos,
                interval,
                ..
            } => Some((
                projectile.as_deref().unwrap_or(DEFAULT_VOLLEY_PROJECTILE),
                from_anchor,
                kill_zone,
                salvos.unwrap_or(DEFAULT_VOLLEY_SALVOS),
                interval.unwrap_or(DEFAULT_VOLLEY_INTERVAL),
            )),
            _ => None,
        }
    }

    /// `(region_anchor, falling_block, then_floor)` if this is a `collapse`
    /// (spec-0022), with the documented default already applied.
    pub fn collapse(&self) -> Option<(&StealthZone, &str, Option<&str>)> {
        match &self.verb {
            Verb::Collapse {
                region_anchor,
                falling_block,
                then_floor,
                ..
            } => Some((
                region_anchor,
                falling_block
                    .as_deref()
                    .unwrap_or(DEFAULT_COLLAPSE_FALLING_BLOCK),
                then_floor.as_deref(),
            )),
            _ => None,
        }
    }

    /// The NPC id if this is a v0.6 `spawn-npc` effect (the dual of
    /// [`QuestEffect::despawn_npc`]).
    pub fn spawn_npc(&self) -> Option<&NpcId> {
        match &self.verb {
            Verb::SpawnNpc { npc, .. } => Some(npc),
            _ => None,
        }
    }

    /// **The body this effect puts into the world**, by id, for every body class
    /// alike ([`BodyRef`]).
    ///
    /// `spawn-npc` and `spawn-actor` are the two, and they are answered in one
    /// place so a rule about a body's lifetime quantifies over bodies rather
    /// than over the verb that first needed it. A body's OTHER entry — standing
    /// on its mark from world init — is not an effect at all and is
    /// [`BodyRef::at_world_init`].
    ///
    /// `unleash-actor` is deliberately not an entry: it puts no new body on the
    /// mark, it replaces the one already standing there (see [`Self::body_exit`]).
    pub fn body_entry(&self) -> Option<&str> {
        match &self.verb {
            Verb::SpawnNpc { npc, .. } => Some(npc.as_str()),
            Verb::SpawnActor { actor, .. } => Some(actor.as_str()),
            _ => None,
        }
    }

    /// **The body this effect takes out of the world**, by id, for every body
    /// class alike.
    ///
    /// `despawn-npc` and `despawn-actor` remove the body outright.
    /// `unleash-actor` is the third: it kills the staged puppet and stands a
    /// real-AI twin in its place, and from that moment the compiler makes no
    /// claim about where that body is or whether it is still alive — the twin
    /// walks, fights and dies under vanilla AI. Answering all three here is what
    /// keeps "can this body still be standing?" from being decided one verb at a
    /// time.
    ///
    /// Deliberately NOT an exit: `move-npc` / `move-actor`. A walked body is
    /// still in the world, and its declared mark is still the cell the engine
    /// summoned it onto — a mark two live bodies share is shared whether or not
    /// one of them has since walked off it.
    pub fn body_exit(&self) -> Option<&str> {
        match &self.verb {
            Verb::DespawnNpc { npc, .. } => Some(npc.as_str()),
            Verb::DespawnActor { actor, .. } | Verb::UnleashActor { actor, .. } => {
                Some(actor.as_str())
            }
            _ => None,
        }
    }

    /// `(anchor, on_respawn)` if this is a v0.6 `set-checkpoint` effect.
    pub fn set_checkpoint(&self) -> Option<(&AnchorId, &[QuestEffect])> {
        match &self.verb {
            Verb::SetCheckpoint { anchor, on_respawn } => Some((anchor, on_respawn.as_slice())),
            _ => None,
        }
    }

    /// `(anchor, on_rest)` if this is a `bonfire` effect (spec-0016 §1).
    pub fn bonfire(&self) -> Option<(&AnchorId, &[QuestEffect])> {
        match &self.verb {
            Verb::Bonfire {
                anchor, on_rest, ..
            } => Some((anchor, on_rest.as_slice())),
            _ => None,
        }
    }

    /// The bonfire's authored rest-dialog strings, each `None` when unauthored
    /// (the compiler then bakes its canonical English). `None` for every other
    /// effect (spec-0016 §1).
    pub fn bonfire_labels(&self) -> Option<BonfireLabels<'_>> {
        match &self.verb {
            Verb::Bonfire {
                prompt,
                rest_label,
                save_label,
                rest_tooltip,
                save_tooltip,
                ..
            } => Some(BonfireLabels {
                prompt: prompt.as_deref(),
                rest_label: rest_label.as_deref(),
                save_label: save_label.as_deref(),
                rest_tooltip: rest_tooltip.as_deref(),
                save_tooltip: save_tooltip.as_deref(),
            }),
            _ => None,
        }
    }

    /// The `within` filter zone if this is a v0.6 `damage-players` effect that
    /// declares one (the `in` spatial scope). `None` for an unscoped
    /// `damage-players` and for every other effect.
    pub fn damage_within(&self) -> Option<&StealthZone> {
        match &self.verb {
            Verb::DamagePlayers { .. } => self.within.as_ref(),
            _ => None,
        }
    }

    /// `(zones, on_caught, grace_ticks)` if this is a v0.6 `begin-stealth` effect.
    pub fn begin_stealth(&self) -> Option<(&[StealthZone], &[QuestEffect], u32)> {
        match &self.verb {
            Verb::BeginStealth {
                zones,
                on_caught,
                grace_ticks,
            } => Some((zones.as_slice(), on_caught.as_slice(), *grace_ticks)),
            _ => None,
        }
    }

    /// The target time if this is a v0.5 `set-time` effect.
    pub fn set_time(&self) -> Option<WorldTime> {
        match &self.verb {
            Verb::SetTime { time, .. } => Some(*time),
            _ => None,
        }
    }

    /// The target weather if this is a v0.5 `set-weather` effect.
    pub fn set_weather(&self) -> Option<WorldWeather> {
        match &self.verb {
            Verb::SetWeather { weather, .. } => Some(*weather),
            _ => None,
        }
    }

    /// The effect lists nested one level inside this effect (DSL v0.6): a
    /// `sequence`'s per-step effects (in step order), a `set-checkpoint`'s
    /// `on_respawn`, a `begin-stealth`'s `on_caught`, and a `move-actor`'s /
    /// `move-npc`'s `on_arrive`. Empty for a leaf effect.
    ///
    /// This is the **single authority** on effect nesting. Every deep traversal —
    /// the flag/wave producer scans, the checkpoint/stealth collector, the l10n
    /// string inventory, and emission — walks the tree through it (see
    /// [`Self::visit_deep`]), so a new nesting site is picked up everywhere at
    /// once and no walker can silently miss a list (the class of bug where a
    /// `set-flag`/`set-checkpoint` nested in a `sequence` was skipped).
    pub fn nested_effect_lists(&self) -> Vec<&[QuestEffect]> {
        match &self.verb {
            Verb::Sequence { steps } => steps.iter().map(|s| s.effects.as_slice()).collect(),
            Verb::SetCheckpoint { on_respawn, .. } => vec![on_respawn.as_slice()],
            Verb::Bonfire { on_rest, .. } => vec![on_rest.as_slice()],
            Verb::BeginStealth { on_caught, .. } => vec![on_caught.as_slice()],
            Verb::MoveActor { on_arrive, .. } | Verb::MoveNpc { on_arrive, .. } => {
                vec![on_arrive.as_slice()]
            }
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
        match &mut self.verb {
            Verb::Sequence { steps } => steps
                .iter_mut()
                .enumerate()
                .map(|(s, st)| (format!("seq.{s}"), st.effects.as_mut_slice()))
                .collect(),
            Verb::SetCheckpoint { on_respawn, .. } => {
                vec![("respawn".to_string(), on_respawn.as_mut_slice())]
            }
            Verb::Bonfire { on_rest, .. } => {
                vec![("rest".to_string(), on_rest.as_mut_slice())]
            }
            Verb::BeginStealth { on_caught, .. } => {
                vec![("caught".to_string(), on_caught.as_mut_slice())]
            }
            Verb::MoveActor { on_arrive, .. } | Verb::MoveNpc { on_arrive, .. } => {
                vec![("arrive".to_string(), on_arrive.as_mut_slice())]
            }
            _ => Vec::new(),
        }
    }

    /// Immutable sibling of [`Self::nested_effect_lists_keyed_mut`] that additionally
    /// exposes the **JSON-pointer path segment** for each nested list, so a deep
    /// consumer scan (sound/art/give/wave refs) can report a precise diagnostic path
    /// *and* the matching l10n key. Each entry is `(path_seg, key_seg, list)`; the
    /// caller appends the per-effect index — `/{j}` to the path, `.{j}` to the key.
    /// Kept in lockstep with `nested_effect_lists` / `nested_effect_lists_keyed_mut`
    /// (same lists, same order): the l10n key segments match
    /// `nested_effect_lists_keyed_mut` exactly (`seq.<step>`/`respawn`/`caught`/
    /// `arrive`), and the path segments name the real fields
    /// (`steps/<step>/effects`, `on_respawn`, `on_caught`, `on_arrive`).
    pub fn nested_effect_lists_labeled(&self) -> Vec<(String, String, &[QuestEffect])> {
        match &self.verb {
            Verb::Sequence { steps } => steps
                .iter()
                .enumerate()
                .map(|(s, st)| {
                    (
                        format!("steps/{s}/effects"),
                        format!("seq.{s}"),
                        st.effects.as_slice(),
                    )
                })
                .collect(),
            Verb::SetCheckpoint { on_respawn, .. } => vec![(
                "on_respawn".to_string(),
                "respawn".to_string(),
                on_respawn.as_slice(),
            )],
            Verb::Bonfire { on_rest, .. } => vec![(
                "on_rest".to_string(),
                "rest".to_string(),
                on_rest.as_slice(),
            )],
            Verb::BeginStealth { on_caught, .. } => vec![(
                "on_caught".to_string(),
                "caught".to_string(),
                on_caught.as_slice(),
            )],
            Verb::MoveActor { on_arrive, .. } | Verb::MoveNpc { on_arrive, .. } => {
                vec![(
                    "on_arrive".to_string(),
                    "arrive".to_string(),
                    on_arrive.as_slice(),
                )]
            }
            _ => Vec::new(),
        }
    }

    /// Every world anchor this effect names **at this node** — the single
    /// authority on the anchor-bearing effect surface, the referential sibling of
    /// [`Self::nested_effect_lists`]. Each entry is `(json_path_suffix, anchor)`,
    /// where the suffix is appended to the effect's own JSON pointer
    /// (`anchor`, `to/anchor`, `in/anchor`, `zones/<i>/anchor`, `at/anchor`,
    /// `shots/<i>/path/<j>/anchor`, …).
    ///
    /// Not recursive: pair it with [`Self::visit_deep`] to sweep a whole effect
    /// tree. Every consumer that must resolve an anchor — the DSL's `DW0142`
    /// reference scan, the compiler's build-time resolution seal (`DW0360`) —
    /// goes through here, so a new anchor-bearing variant (or a new anchor field
    /// on an existing one) is picked up by all of them at once. That closes the
    /// silent-drop class of bug: an anchor-bearing effect whose anchor is typo'd
    /// used to emit *nothing* (the emitter's `for … if name == anchor` loop simply
    /// found no match) while every shallow validator looked straight past it.
    ///
    /// # The third element is the SHAPE the site demands
    ///
    /// A reference is not only a name: `close-gate` fills and clears a region and
    /// `bonfire` seats a body, and until spec-0052 those two sat in one match arm
    /// with nothing recording the difference. The demand rides with the reference
    /// so that a new anchor-bearing variant cannot be added without stating what
    /// it does with the anchor — the same reason this is one authority and not
    /// three walks.
    ///
    /// **A demand is stated only where the shape is structurally required**, and
    /// the line is drawn where this engine's own `DW0845` already draws it — *a
    /// region is not a place to stand*:
    ///
    /// * [`StationKind::Gate`] where the verb cannot function without a region
    ///   that seals and clears: the two gate verbs. A point is not a gate, and
    ///   `gate_region_block_any` finds nothing for one.
    /// * [`StationKind::Point`] where a **body is put**: a checkpoint seat, a
    ///   bonfire, a `move-npc`/`move-actor` destination, a `teleport`
    ///   destination. A body cannot stand inside bars.
    /// * `None` wherever a reference merely names a **location** — every
    ///   anchor-centred volume's centre, every camera field, a `set-block`, a
    ///   `play-sound`. A gate region's own corner is a perfectly good answer
    ///   there, and refusing it would refuse correct content.
    ///
    /// That last case is not a gap left for later. An earlier draft of this rule
    /// demanded a point at every non-gate site, and the gallery refused to
    /// compile: `trigger/east-door-wrong-side` is a `use` trigger sitting on the
    /// very `anchor/seam-…` of a barred door so that it can say "the east door
    /// does not open from this side". A check that resolves against a smaller
    /// world than the campaign has refuses CONTENT, which is the lesson `DW0343`
    /// carries three files away.
    pub fn anchor_refs(&self) -> Vec<(String, &AnchorId, Option<StationKind>)> {
        // The envelope's `in` box (spec-0085 §3.2) is one capability of every
        // player-facing verb, so it registers once, here, before the verb's own.
        let mut out: Vec<(String, &AnchorId, Option<StationKind>)> = Vec::new();
        if let Some(zone) = &self.within {
            out.push(("in/anchor".to_string(), &zone.anchor, None));
        }
        out.extend(self.verb_anchor_refs());
        out
    }

    /// The anchors the VERB names at this node — [`Self::anchor_refs`] without the
    /// envelope.
    fn verb_anchor_refs(&self) -> Vec<(String, &AnchorId, Option<StationKind>)> {
        // Every camera field is a point: a shot flies through cells and looks at
        // one.
        /// `(suffix, anchor, kind)` for a shot's own anchor-bearing fields, under `base`.
        fn shot_refs<'a>(
            base: &str,
            shot: &'a CameraShot,
        ) -> Vec<(String, &'a AnchorId, Option<StationKind>)> {
            let mut out: Vec<(String, &AnchorId, Option<StationKind>)> = shot
                .path
                .iter()
                .enumerate()
                .map(|(j, w)| (format!("{base}path/{j}/anchor"), &w.anchor, None))
                .collect();
            if let Some(t) = &shot.look_at {
                out.push((format!("{base}look_at/anchor"), &t.anchor, None));
            }
            for (field, subject) in [("subject", &shot.subject), ("subject_b", &shot.subject_b)] {
                if let Some(CameraSubject::Anchor(s)) = subject {
                    out.push((format!("{base}{field}/anchor"), &s.anchor, None));
                }
            }
            out
        }
        match &self.verb {
            // The two gate verbs address a REGION that seals and clears; the
            // three below them seat or write at a cell. They shared an arm until
            // the demand had somewhere to be written down.
            Verb::OpenGate { anchor, .. } | Verb::CloseGate { anchor, .. } => {
                vec![("anchor".to_string(), anchor, Some(StationKind::Gate))]
            }
            // A checkpoint and a bonfire are **respawn seats** — a body is put
            // there, so a region is not one. `set-block` writes a block at a
            // cell, which names a location and seats nothing.
            Verb::SetCheckpoint { anchor, .. } | Verb::Bonfire { anchor, .. } => {
                vec![("anchor".to_string(), anchor, Some(StationKind::Point))]
            }
            Verb::SetBlock { anchor, .. } => {
                vec![("anchor".to_string(), anchor, None)]
            }
            Verb::MoveNpc { to, .. } | Verb::MoveActor { to, .. } => {
                vec![(
                    "to/anchor".to_string(),
                    &to.anchor,
                    Some(StationKind::Point),
                )]
            }
            // Both of a `teleport`'s anchors are load-bearing — the source volume
            // decides WHAT moves and the destination decides WHERE — so a typo in
            // either is a dangling reference (`DW0142`), never a silently
            // zero-cell volume or a dropped command.
            Verb::Teleport { from, to, .. } => vec![
                ("from/anchor".to_string(), &from.anchor, None),
                (
                    "to/anchor".to_string(),
                    &to.anchor,
                    Some(StationKind::Point),
                ),
            ],
            Verb::BeginStealth { zones, .. } => zones
                .iter()
                .enumerate()
                .map(|(i, z)| (format!("zones/{i}/anchor"), &z.anchor, None))
                .collect(),
            Verb::PlaySound {
                at: Some(SoundAt::Anchor { anchor, .. }),
                ..
            } => vec![("at/anchor".to_string(), anchor, None)],
            // A firework is launched from a point and seats nothing, so it names
            // a location in the same shape `play-sound` does.
            Verb::Firework { at, .. } => vec![("at/anchor".to_string(), &at.anchor, None)],
            // A bolt strikes a point and seats nothing, the same shape.
            Verb::Lightning { at } => vec![("at/anchor".to_string(), &at.anchor, None)],
            // A particle at a mark names a location the same way; at `players`
            // it names none.
            Verb::Particle {
                at: ParticleAt::Mark(m),
                ..
            } => vec![("at/anchor".to_string(), &m.anchor, None)],
            // spec-0022 trap-payload verbs. Both anchors of a `volley` are
            // load-bearing for the coverage proof, so both register here — a
            // typo'd `kill_zone` must be a dangling-reference error, never a
            // silently zero-cell (and therefore vacuously "covered") volley.
            Verb::Volley {
                from_anchor,
                kill_zone,
                ..
            } => vec![
                ("from_anchor".to_string(), from_anchor, None),
                ("kill_zone/anchor".to_string(), &kill_zone.anchor, None),
            ],
            Verb::Collapse { region_anchor, .. } => {
                vec![(
                    "region_anchor/anchor".to_string(),
                    &region_anchor.anchor,
                    None,
                )]
            }
            // The v0.10 region writes: the box's anchor is load-bearing for both
            // the emission and the completability model, so a typo'd one must be a
            // dangling-reference error (`DW0142`/`DW0360`), never a silently
            // unwritten — and therefore vacuously proven — region.
            Verb::FillRegion { region, .. } | Verb::ClearRegion { region, .. } => {
                vec![("region/anchor".to_string(), &region.anchor, None)]
            }
            // A repaint's box centre names a location, exactly as a region
            // write's does.
            Verb::SetAtmosphere {
                region: Some(region),
                ..
            } => vec![("region/anchor".to_string(), &region.anchor, None)],
            // Both cutscene spellings (`DW0199` polices mixing them): the v0.6
            // multi-shot list, or the v0.4 single-shot fields flattened at the
            // effect's own level.
            Verb::Cutscene {
                shots,
                path,
                look_at,
                ..
            } => {
                let mut out: Vec<(String, &AnchorId, Option<StationKind>)> = shots
                    .iter()
                    .enumerate()
                    .flat_map(|(i, s)| shot_refs(&format!("shots/{i}/"), s))
                    .collect();
                out.extend(
                    path.iter()
                        .enumerate()
                        .map(|(j, w)| (format!("path/{j}/anchor"), &w.anchor, None)),
                );
                if let Some(t) = look_at {
                    out.push(("look_at/anchor".to_string(), &t.anchor, None));
                }
                out
            }
            _ => Vec::new(),
        }
    }

    /// **Every object of a subject kind this effect names at this node** — the
    /// `npc/`, `actor/`, `wave/` and `anchor/` ids [`Happening::subject`] may
    /// name, in a fixed order, deduplicated (spec-0071 §3).
    ///
    /// The bodies come from [`Self::body_entry`] / [`Self::body_exit`] and the
    /// move verbs; the anchors come from [`Self::anchor_refs`], the single
    /// authority on the anchor-bearing surface, so a verb that gains an anchor
    /// gains it here too. Not recursive: a `sequence`'s steps each answer for
    /// themselves.
    ///
    /// This is a question about the **object class** a beat can be about, not
    /// about a list of verbs somebody maintains — which is why `unleash-actor`
    /// (one actor), `set-block` (one anchor) and `fill-region` (one anchor)
    /// answer it without being named anywhere, and why `move-actor` (an actor
    /// AND a destination anchor) and `teleport` (two anchors) answer with two.
    pub fn subject_objects(&self) -> Vec<&str> {
        fn add<'a>(id: &'a str, out: &mut Vec<&'a str>) {
            if !out.contains(&id) {
                out.push(id);
            }
        }
        let mut out: Vec<&str> = Vec::new();
        match &self.verb {
            Verb::SpawnNpc { npc, .. }
            | Verb::DespawnNpc { npc, .. }
            | Verb::MoveNpc { npc, .. } => add(npc.as_str(), &mut out),
            Verb::SpawnWave { wave, .. } => add(wave.as_str(), &mut out),
            _ => {}
        }
        if let Some(actor) = self.actor_ref() {
            add(actor.as_str(), &mut out);
        }
        for (_, anchor, _) in self.anchor_refs() {
            add(anchor.as_str(), &mut out);
        }
        out
    }

    /// **What this effect's `happening` is about**: the subject the branch
    /// chronicle records and the contradiction proof (`DW0485`) reasons over
    /// (spec-0071 §3).
    ///
    /// A stated [`Happening::subject`] always wins — the caller knows more. An
    /// absent one resolves to the effect's own object when it has exactly one
    /// ([`Self::subject_objects`]), because the beat that opens a gate is about
    /// that gate and the id is otherwise typed twice, two keys apart. An effect
    /// with several objects, or none, resolves nothing: naming one of them would
    /// be the compiler guessing which.
    ///
    /// **The one derivation.** The namespace check (`dsl::validate`) and the
    /// chronicle writer (`delvec::compiler::branch`) both read the subject
    /// through here, so a beat cannot be about one thing for the proof and
    /// another for the account a reader is handed.
    pub fn happening_subject(&self) -> Option<HappeningSubject<'_>> {
        let h = self.happening.as_ref()?;
        if let Some(stated) = h.subject.as_deref() {
            return Some(HappeningSubject {
                id: stated,
                derived: false,
            });
        }
        let objects = self.subject_objects();
        match objects.as_slice() {
            [only] => Some(HappeningSubject {
                id: only,
                derived: true,
            }),
            _ => None,
        }
    }

    /// The `cutscene` camera subject if this is a single-shot `cutscene` carrying
    /// the v0.6 `look_at` field.
    pub fn cutscene_look_at(&self) -> Option<&Mark> {
        match &self.verb {
            Verb::Cutscene { look_at, .. } => look_at.as_ref(),
            _ => None,
        }
    }

    /// Whether this `cutscene` shows the party's bodies (spec-0095): the stated
    /// `party`, or `present` when none is stated. `None` for any other effect.
    pub fn cutscene_party(&self) -> Option<CutsceneParty> {
        match &self.verb {
            Verb::Cutscene { party, .. } => Some(party.unwrap_or_default()),
            _ => None,
        }
    }

    /// `true` if this is a `cutscene` written in the v0.6 multi-shot form
    ///.
    pub fn cutscene_multi_shot(&self) -> bool {
        matches!(&self.verb, Verb::Cutscene { shots, .. } if !shots.is_empty())
    }

    /// The normalized shot list of a `cutscene`, whichever spelling was used: the
    /// v0.6 `shots` list as-is, or the v0.4 `path`/`seconds`/`look_at` fields as a
    /// single shot. `None` for a non-cutscene effect; an empty list for a cutscene
    /// whose shape is invalid (`DW0199` reports that).
    pub fn cutscene_shots(&self) -> Option<Vec<CameraShot>> {
        match &self.verb {
            Verb::Cutscene {
                shots,
                path,
                seconds,
                look_at,
                ..
            } => {
                if !shots.is_empty() {
                    return Some(shots.clone());
                }
                match seconds {
                    Some(secs) => Some(vec![CameraShot {
                        path: path.clone(),
                        seconds: Some(*secs),
                        look_at: look_at.clone(),
                        shot_style: None,
                        subject: None,
                        subject_b: None,
                        dist: None,
                        degrees: None,
                        bearing: None,
                    }]),
                    None => Some(Vec::new()),
                }
            }
            _ => None,
        }
    }

    /// `true` if this is a `narrate` carrying the `art` style (glyph-checked
    /// `DW0328`).
    pub fn narrate_art(&self) -> bool {
        matches!(
            &self.verb,
            Verb::Narrate {
                style: Some(NarrateStyle::Art),
                ..
            }
        )
    }

    /// The `narrate` line's text if this is a `narrate` with the `art` style.
    pub fn narrate_art_text(&self) -> Option<&str> {
        match &self.verb {
            Verb::Narrate {
                text,
                style: Some(NarrateStyle::Art),
                ..
            } => Some(text.as_str()),
            _ => None,
        }
    }

    /// The `narrate` line's style and text if this is a `narrate` rendered **on
    /// screen** — `title`, `subtitle` or `art` — rather than in chat. These are the
    /// styles vanilla draws centred and unwrapped, so their rendered width is
    /// length-checked against the screen (`DW0330`); `chat` scrolls and wraps, so it
    /// is exempt.
    pub fn narrate_on_screen(&self) -> Option<(NarrateStyle, &str)> {
        match &self.verb {
            Verb::Narrate {
                text,
                style: Some(s),
                ..
            } if matches!(
                s,
                NarrateStyle::Title | NarrateStyle::Subtitle | NarrateStyle::Art
            ) =>
            {
                Some((*s, text.as_str()))
            }
            _ => None,
        }
    }

    /// Every vanilla sound-event id this effect references, for registry
    /// validation (`DW0326`): a `play-sound`'s `sound`, and a `narrate`'s optional
    /// `sound`. Returns `(subpath, id)` pairs where `subpath` locates the field
    /// within the effect (e.g. `sound`).
    pub fn sound_refs(&self) -> Vec<(&'static str, &str)> {
        match &self.verb {
            Verb::PlaySound { sound, .. } => vec![("sound", sound.as_str())],
            Verb::Narrate { sound: Some(s), .. } => vec![("sound", s.as_str())],
            _ => Vec::new(),
        }
    }

    /// The `play-sound` `at: actor` id, if this effect is a `play-sound`
    /// targeting an actor (rejected `DW0335`).
    pub fn play_sound_actor(&self) -> Option<&str> {
        match &self.verb {
            Verb::PlaySound {
                at: Some(SoundAt::Actor { actor }),
                ..
            } => Some(actor.as_str()),
            _ => None,
        }
    }

    /// The per-effect flag gate: flags that must ALL be set (per party) for this
    /// effect to fire. Empty for an ungated effect. Read off the one [`Guard`], so
    /// **every** verb answers it — the staging and souls vocabulary included.
    pub fn requires_flags(&self) -> &[FlagId] {
        self.when.as_ref().map_or(&[][..], |g| &g.requires_flags)
    }

    /// The per-effect **negative** flag gate: flags whose being set suppresses this
    /// effect — the dual of [`QuestEffect::requires_flags`], on the same guard.
    pub fn forbids_flags(&self) -> &[FlagId] {
        self.when.as_ref().map_or(&[][..], |g| &g.forbids_flags)
    }

    /// The numeric gate terms (spec-0031) — the third axis of the same guard, so
    /// "which verbs are gatable" has exactly one answer.
    pub fn requires_state(&self) -> &[StateCompare] {
        self.when.as_ref().map_or(&[][..], |g| &g.requires_state)
    }

    /// The datum this effect writes and how, if it is one of the DSL v0.10 state
    /// verbs (`set-state` / `add-state` / `clear-state`).
    pub fn writes_state(&self) -> Option<(&StateId, StateWrite)> {
        match &self.verb {
            Verb::SetState { state, value, .. } => Some((state, StateWrite::Set(*value))),
            Verb::AddState { state, amount, .. } => Some((state, StateWrite::Add(*amount))),
            Verb::ClearState { state, .. } => Some((state, StateWrite::Clear)),
            _ => None,
        }
    }

    /// The `on_arrive` bundle if this is a `move-npc` carrying one (DSL v0.6;
    /// parity with `move-actor`). `None` for a bare `move-npc` and every other
    /// effect.
    pub fn move_npc_on_arrive(&self) -> Option<&[QuestEffect]> {
        match &self.verb {
            Verb::MoveNpc { on_arrive, .. } if !on_arrive.is_empty() => Some(on_arrive.as_slice()),
            _ => None,
        }
    }

    /// The actor id this effect targets, if it is one of the actor staging effects
    /// (`spawn-actor`/`despawn-actor`/`move-actor`/`unleash-actor`). `sequence` has
    /// no single actor (its nested effects each carry their own).
    pub fn actor_ref(&self) -> Option<&ActorId> {
        match &self.verb {
            Verb::SpawnActor { actor, .. }
            | Verb::DespawnActor { actor, .. }
            | Verb::MoveActor { actor, .. }
            | Verb::UnleashActor { actor, .. } => Some(actor),
            _ => None,
        }
    }

    /// Every vanilla **status-effect** id this effect names, for registry
    /// validation (`DW0192`) — the sibling of [`Self::sound_refs`] and the single
    /// authority on the status-effect-bearing verb surface. `(subpath, id)`
    /// pairs; empty for a `clear-effect` that names none (which clears all).
    pub fn status_effect_refs(&self) -> Vec<(&'static str, &str)> {
        match &self.verb {
            Verb::GiveEffect { effect, .. } => vec![("effect", effect.as_str())],
            Verb::ClearEffect {
                effect: Some(e), ..
            } => vec![("effect", e.as_str())],
            _ => Vec::new(),
        }
    }

    /// `(effect, seconds, amplifier, hide_particles, in)` if this is a
    /// `give-effect` (DSL v0.10), with the documented defaults already applied.
    pub fn give_effect(&self) -> Option<(&str, u32, u32, bool, Option<&StealthZone>)> {
        match &self.verb {
            Verb::GiveEffect {
                effect,
                seconds,
                amplifier,
                hide_particles,
                ..
            } => Some((
                effect.as_str(),
                *seconds,
                amplifier.unwrap_or(0),
                hide_particles.unwrap_or(false),
                self.within.as_ref(),
            )),
            _ => None,
        }
    }

    /// `(effect, in)` if this is a `clear-effect` (DSL v0.10). The effect is
    /// `None` for the clear-everything form, exactly as vanilla spells it.
    pub fn clear_effect(&self) -> Option<(Option<&str>, Option<&StealthZone>)> {
        match &self.verb {
            Verb::ClearEffect { effect, .. } => Some((effect.as_deref(), self.within.as_ref())),
            _ => None,
        }
    }

    /// `(from, to)` if this is a `teleport` (DSL v0.10): the source volume and
    /// the destination mark.
    pub fn teleport(&self) -> Option<(&StealthZone, &Mark)> {
        match &self.verb {
            Verb::Teleport { from, to, .. } => Some((from, to)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod happening_subject_tests;
