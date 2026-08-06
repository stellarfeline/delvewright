//! serde types for the six stage `content` payloads (spec-0001 v0.2).
//!
//! Every struct is `deny_unknown_fields`. Reserved enum values (npc `role`,
//! objective/effect types) parse successfully but are rejected by validation
//! ([`crate::validate`]) with code `DW0141`.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{
    ActorId, AmbushId, AnchorId, AreaId, BranchId, BranchPointId, ClassId, DialogueId, EditBatchId,
    EndingId, FlagId, LootId, NpcId, ObjectiveId, PoolId, PrefabId, QuestId, RegionId, ShortcutId,
    TimedGateId, TrapId, TriggerId, WaveId,
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
    /// Declared combat difficulty (DSL v0.6, owner ruling 2026-08-03). Absent =
    /// the compiler's historical derivation — `easy` when the campaign fields any
    /// wave, `peaceful` when it fields none — which is what keeps every campaign
    /// written before this field byte-identical. Declaring it overrides the
    /// derivation for **both** the shipped `server.properties` and a `/difficulty`
    /// in the sealing baseline, so the declaration also holds when the datapack is
    /// dropped into somebody else's world.
    ///
    /// `peaceful` is rejected (`DW0468`). Raising difficulty changes the damage
    /// players take — easy halves it — so combat arithmetic tuned under the old
    /// implicit `easy` must be redone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub difficulty: Option<WorldDifficulty>,
    /// Scenic horizon (DSL v0.6, spec-0013). Absent or `void` = the void world
    /// (byte-identical to v0.5). `ocean` swaps the world generator for a
    /// deterministic superflat sea (bedrock/stone/water, sea level y=62) and drops
    /// the area datum to y=60 (`sea_level-2`) so island pieces meet the sea at their
    /// authored waterline. No structures or mobs either way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizon: Option<Horizon>,
    /// Playable-region boundary (DSL v0.6, spec-0013). When present, the compiler
    /// derives a region from the placed geometry and a per-second clock returns any
    /// player who leaves it to the last checkpoint. Required when `horizon` is
    /// `ocean` (an infinite swimmable sea with no return rule is `DW0320`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boundary: Option<Boundary>,
    /// The closing line on the campaign-completion advancement — the last
    /// player-visible sentence of the delve (DSL v0.6). Player-visible, so it is
    /// l10n-inventoried as `world.outro` and sidecars translate it. Absent = the
    /// finale quest's `goal`, which is already both campaign-derived and
    /// inventoried; the description was previously the hardcoded English
    /// "You left the keep." on *every* delve, whatever its theme or language.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outro: Option<String>,
    /// The party size this delve **requires** (DSL v0.6, spec-0018). Absent = 1: a
    /// party of one is always legal and every pre-0.6 campaign keeps that reading.
    /// A design whose beats genuinely need `n` players — two rooms whose switches
    /// are two arms of one AND-join — declares `min_players: n` (max 4), and the
    /// lobby then refuses to start below it: the class-selection dialog stays shut
    /// and the waiting players get a party-count actionbar instead.
    ///
    /// Progression is party state either way (spec-0018), so this is a *declaration
    /// of intent*, not a mechanism: it makes a mandatory-n design first-class
    /// (owner amendment 2026-08-02) and turns on the analyzer's n-agent division
    /// proof. Out of `1..=4` is `DW0370`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_players: Option<u8>,
}

/// Who a granted item goes to (DSL v0.6, spec-0018).
///
/// Progression is a fact about the party, so the default for every `give-item` and
/// every class-kit entry is **all**: a quest beat that arms the party arms all of
/// it. `one` is the deliberate exception for a single quest prop (the wine-skin,
/// the stake): exactly one copy enters the party, handed to the player whose action
/// earned it, and the party passes it around physically.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Carrier {
    /// Every party member receives the item (the default).
    #[default]
    All,
    /// Exactly one copy, to the player whose action fired the effect.
    One,
}

/// A declared world time state (DSL v0.5, spec-0010). The sole difference from
/// vanilla is that the daylight cycle is frozen (`advance_time false`), so a set
/// state persists for the whole delve until a `set-time` effect cuts to another.
///
/// Vanilla's `/time set` primitive takes **either** one of four keywords or a raw
/// tick count, and the tick form is the general one — so the states worth naming
/// for a delve's pacing are not limited to the four keywords. `dusk` and `dawn`
/// (owner ruling, 2026-08-03) are the tick form exposed first-class, per the
/// no-hack rule: the DSL names the beat, the compiler emits `/time set <ticks>`.
/// Every keyword-to-tick mapping lives in exactly one table ([`WorldTime::spec`]),
/// and the four vanilla keywords still emit their keyword verbatim, so existing
/// campaigns are byte-identical.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum WorldTime {
    /// Morning daylight (`/time set day`, 1000 ticks).
    Day,
    /// Midday, brightest (`/time set noon`, 6000 ticks) — the default.
    #[default]
    Noon,
    /// Sunset — the sky visibly going orange and the day ending
    /// (`/time set 12000`). Deliberately NOT 13000: that is the instant the sun
    /// has finished setting, which is what the `night` keyword already sets, so
    /// 13000 would make `dusk` a synonym rather than its own beat.
    Dusk,
    /// Night, sun fully down (`/time set night`, 13000 ticks).
    Night,
    /// Deep night, darkest (`/time set midnight`, 18000 ticks).
    Midnight,
    /// First light, just before sunrise (`/time set 23000`). Spelled `dawn`;
    /// `sunrise` is accepted as a synonym on input.
    #[serde(alias = "sunrise")]
    Dawn,
}

impl WorldTime {
    /// The single keyword/tick table: `(the /time set argument, daytime ticks)`.
    ///
    /// A state vanilla names keeps its keyword — the argument the compiler has
    /// always emitted — so no shipped campaign's bytes move. A state vanilla does
    /// not name emits the equivalent tick count, which is the same primitive.
    const fn spec(self) -> (&'static str, i64) {
        match self {
            WorldTime::Day => ("day", 1000),
            WorldTime::Noon => ("noon", 6000),
            WorldTime::Dusk => ("12000", 12000),
            WorldTime::Night => ("night", 13000),
            WorldTime::Midnight => ("midnight", 18000),
            WorldTime::Dawn => ("23000", 23000),
        }
    }

    /// The vanilla `/time set` argument — a keyword for the four states vanilla
    /// names, a tick count otherwise.
    pub fn token(self) -> &'static str {
        self.spec().0
    }

    /// The `daytime` tick value this state sets (the `time query daytime`
    /// read-back). Vanilla constants: day=1000, noon=6000, dusk=12000 (sunset
    /// onset), night=13000, midnight=18000, dawn=23000.
    pub fn daytime_ticks(self) -> i64 {
        self.spec().1
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

/// The declared combat difficulty of the delve (DSL v0.6, owner ruling
/// 2026-08-03). Values are the vanilla `/difficulty` keywords.
///
/// Difficulty is the single largest lever on how hard a delve *feels*, and until
/// this field existed the compiler chose it: `easy` for any campaign with a wave,
/// `peaceful` for one without. Easy **halves incoming player damage** —
/// `min(dmg / 2 + 1, dmg)` — so every combat number in a pre-0.6 campaign was
/// tuned against a halved world without anyone declaring it. A campaign that
/// raises this must redo that arithmetic.
///
/// [`WorldDifficulty::Peaceful`] parses but is **rejected** by validation
/// (`DW0468`): peaceful makes the engine discard every hostile-category mob on
/// the tick it is ticked, summoned or not, so every wave, actor and ambush in the
/// campaign would silently vanish. It is a variant only so the compiler can say
/// that in a diagnostic instead of a serde "unknown variant" parse error.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum WorldDifficulty {
    /// `/difficulty easy` — the compiler's historical choice for a wave
    /// campaign, and the default reading of an absent field. Incoming player
    /// damage is halved (`min(dmg / 2 + 1, dmg)`).
    #[default]
    Easy,
    /// `/difficulty normal` — vanilla-baseline damage. The souls-style baseline.
    Normal,
    /// `/difficulty hard` — amplified damage, and zombies reinforce.
    Hard,
    /// `/difficulty peaceful` — **always rejected** (`DW0468`). Present only so
    /// the rejection can be a diagnostic with a rationale.
    Peaceful,
}

impl WorldDifficulty {
    /// The vanilla `/difficulty` keyword.
    pub fn token(self) -> &'static str {
        match self {
            WorldDifficulty::Peaceful => "peaceful",
            WorldDifficulty::Easy => "easy",
            WorldDifficulty::Normal => "normal",
            WorldDifficulty::Hard => "hard",
        }
    }

    /// The vanilla `Difficulty#getId()` ordinal, which is also what the bare
    /// `/difficulty` query command returns — the only vanilla read-back path for
    /// the setting, and so what the generated PackTest asserts on.
    pub fn id(self) -> i32 {
        match self {
            WorldDifficulty::Peaceful => 0,
            WorldDifficulty::Easy => 1,
            WorldDifficulty::Normal => 2,
            WorldDifficulty::Hard => 3,
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
    /// A superflat sea backdrop; areas are placed on the sea-level datum (y=60) so
    /// island pieces read as land ringed by the ocean.
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

/// A per-area **darkness mitigation** declaration (DSL v0.6).
///
/// The first-class answer to "this area is meant to be dark, and the players are
/// equipped for it". Declaring it is what makes the compiler *emit* the mitigation
/// (a clocked `effect give … night_vision` scoped to the area's placed bounds) and
/// what satisfies the `DW0210` darkness gate — one declaration, one mechanism, no
/// gap between the check and the feature.
///
/// It replaces the pre-0.6 heuristic that read a class kit item's display *name*
/// for `night vision`: that accepted a renamed water bottle, so the gate passed
/// while nothing in the world granted night vision (owner, island QA).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum AreaMitigation {
    /// Every player inside the area's placed bounds is kept under
    /// `minecraft:night_vision` by a compiler-emitted 1 s clock.
    NightVision,
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
    /// Optional darkness-mitigation declaration (DSL v0.6). `night-vision` makes
    /// the compiler emit a clocked `effect give` over this area's placed bounds and
    /// is the (only) declaration that satisfies `DW0210` without `lighting`.
    /// Independent of `lighting`: an area may declare both (fixtures *and* the
    /// effect), either, or neither.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mitigation: Option<AreaMitigation>,
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
    /// Deferred entrance (DSL v0.6): when `true` the NPC is **not** summoned at
    /// world init — its body and interaction hitbox only appear when a
    /// [`QuestEffect::SpawnNpc`] fires, at this same `anchor`. The dual of
    /// `despawn-npc`: a character with a scripted entrance must not stand at its
    /// mark as a statue from minute one. A deferred NPC that no `spawn-npc` ever
    /// spawns is unreachable content (`DW0197`). Default `false` = summoned at
    /// init, byte-identical to pre-0.6.
    #[serde(default, skip_serializing_if = "is_false")]
    pub deferred: bool,
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
    /// The full line this button is the caption of (DSL v0.8, owner design
    /// 2026-08-04; reserved `DW0141` earlier). Vanilla's dialog button codec
    /// (`CommonButtonData`) carries an optional `tooltip` component beside
    /// `label`, and the client hangs it on the button as a real hover tooltip —
    /// so a caption on the button and the sentence the character actually says
    /// can both exist. **Not** subject to `DW0331`: a tooltip is not drawn on the
    /// 150 px button, it is wrapped at 170 px into its own hover box, so it never
    /// scrolls. Player-visible, so it translates like the label
    /// (`dlg.<npc>.<node>.opt.<i>.tooltip`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<String>,
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
    /// Negative flag gate (DSL v0.6, reserved `DW0141` earlier): the option is
    /// **hidden** (and its `/trigger` handler inert) while ANY listed flag is set
    /// for the player — the dual of `requires_flags`. A `forbids_flags`-gated
    /// option counts as *gated* for the `DW0191` deadlock guard: it can be
    /// suppressed at any point, so it cannot be the only completing path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forbids_flags: Vec<FlagId>,
    /// Effects fired when this option is chosen.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<DialogueEffect>,
    /// What choosing this option does to the story (DSL v0.8, spec-0025;
    /// reserved `DW0141` earlier). Required at 0.8.0 for a **story-weight** beat
    /// — an option carrying a `set-flag` effect, which is how a player's choice
    /// forks the world (`DW0481`). An option that only walks the tree or
    /// completes an objective needs none: the objective already declares one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub happening: Option<Happening>,
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
    /// Summons a `deferred` stage-2 NPC (DSL v0.6), mirroring
    /// [`QuestEffect::SpawnNpc`] — a character who walks in mid-conversation.
    SpawnNpc {
        /// The NPC (stage-2 ref) to summon.
        npc: NpcId,
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
    /// (`set-checkpoint`, spec-0012; `spawn-npc`). Reserved (`DW0141`) in an
    /// earlier campaign.
    pub fn v06_effect(&self) -> Option<&'static str> {
        match self {
            DialogueEffect::SetCheckpoint { .. } => Some("set-checkpoint"),
            DialogueEffect::SpawnNpc { .. } => Some("spawn-npc"),
            _ => None,
        }
    }

    /// The NPC id if this is a v0.6 `spawn-npc` dialogue effect.
    pub fn spawn_npc(&self) -> Option<&NpcId> {
        match self {
            DialogueEffect::SpawnNpc { npc } => Some(npc),
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

    /// The `on_respawn` bundle of a v0.6 `set-checkpoint` dialogue effect —
    /// **effect root 5** ([`crate::effects`]). Named separately from
    /// [`Self::set_checkpoint`] so the root walk and its mutable mirror name the
    /// same accessor modulo mutability, which is what lets one macro body generate
    /// both.
    pub fn set_checkpoint_on_respawn(&self) -> Option<&[QuestEffect]> {
        match self {
            DialogueEffect::SetCheckpoint { on_respawn, .. } => Some(on_respawn.as_slice()),
            _ => None,
        }
    }

    /// The `on_respawn` bundle of a v0.6 `set-checkpoint` dialogue effect, exposed
    /// mutably so the localization pass can rewrite the player-visible strings
    /// nested inside it. Lockstep sibling of [`Self::set_checkpoint`] — the bundle
    /// is a plain `Vec<QuestEffect>` that emission really lowers, so every scan
    /// that reaches it read-only needs a way to reach it writable too.
    pub fn set_checkpoint_on_respawn_mut(&mut self) -> Option<&mut [QuestEffect]> {
        match self {
            DialogueEffect::SetCheckpoint { on_respawn, .. } => Some(on_respawn.as_mut_slice()),
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
    /// Who gets it (DSL v0.6, spec-0018). Absent = [`Carrier::All`]. A class kit is
    /// per-player gear by construction — every player who picks the class gets the
    /// kit — so `carrier` here marks a **party-unique** kit item: exactly one copy
    /// enters the party, given to the first player to pick this class.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub carrier: Option<Carrier>,
    /// **The flask** (DSL v0.8, spec-0016 §1, owner ruling 2026-08-03): this kit
    /// entry is the class's recovery item, and resting at a bonfire replenishes
    /// it to exactly `count`. A campaign that places a `bonfire` and declares no
    /// flask anywhere in its kits is `DW0476` — the estus loop is what makes
    /// dying an investment, so a souls campaign without one is a build error, not
    /// a design choice. Absent on every pre-0.8 kit → emission byte-identical.
    #[serde(default, skip_serializing_if = "is_false")]
    pub flask: bool,
    /// **What is in the bottle** (DSL v0.8, spec-0016 §1, owner directive
    /// 2026-08-03): the vanilla `minecraft:potion_contents` component of a
    /// potion-bearing item ([`POTION_BEARING_ITEMS`]). Without it a
    /// `minecraft:potion` is the *Uncraftable Potion* — a bottle that heals
    /// nothing — which is exactly the placeholder flask this field exists to
    /// abolish, so at `dsl_version` 0.8.0 a potion-bearing kit item that declares
    /// no `contents` is `DW0487`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contents: Option<PotionContents>,
}

/// The items whose vanilla item definition carries a `minecraft:potion_contents`
/// component — the only items a kit `contents` may be declared on (`DW0486`).
///
/// Read off the pinned 1.21.11 `item_components` summary (SHA-256
/// `51b191e13f86813ca02f1498942e5bc235947edb71eb8105a78401670b3665c4`, the same
/// misode/mcmeta ref `crates/compiler/data/PROVENANCE.md` pins): exactly these
/// four items declare the component, and on any other item the game drops the
/// data on the floor.
pub const POTION_BEARING_ITEMS: &[&str] = &[
    "minecraft:lingering_potion",
    "minecraft:potion",
    "minecraft:splash_potion",
    "minecraft:tipped_arrow",
];

/// True if `item_id` (optionally un-namespaced) is one of the four
/// [`POTION_BEARING_ITEMS`].
pub fn is_potion_bearing_item(item_id: &str) -> bool {
    let norm = if item_id.contains(':') {
        item_id.to_string()
    } else {
        format!("minecraft:{item_id}")
    };
    POTION_BEARING_ITEMS.contains(&norm.as_str())
}

/// The two **instantaneous** status effects: they are applied once, on the tick
/// the potion is drunk (`PotionContents.applyToLivingEntity` branches on
/// `isInstantenous` before any effect instance is ever added), so a `duration` on
/// one is a statement the game never reads — `DW0486` says so rather than letting
/// an author believe they wrote a thirty-second heal.
pub const INSTANT_EFFECTS: &[&str] = &["minecraft:instant_health", "minecraft:instant_damage"];

/// A potion-bearing kit item's `minecraft:potion_contents` component (DSL v0.8),
/// modelled field for field on vanilla rather than invented: a **named** potion,
/// a list of **custom effects**, or both, plus the bottle-colour override.
///
/// Vanilla resolves a drink as the named potion's effects followed by the custom
/// ones, and derives the bottle colour from those effects unless `color`
/// overrides it — so `{"potion": "minecraft:strong_healing"}` is literally the
/// Potion of Healing II a player would brew, not an approximation of one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PotionContents {
    /// A named vanilla potion id (`minecraft:strong_healing`,
    /// `minecraft:long_night_vision`, …), validated against the pinned 1.21.11
    /// `potion` registry (`DW0486`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub potion: Option<String>,
    /// Custom effects applied on top of (or instead of) the named potion — the
    /// escape hatch for a recovery item vanilla has no brew for.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<PotionEffect>,
    /// Bottle-colour override, `#rrggbb` (vanilla `custom_color`). Absent → the
    /// colour vanilla derives from the effects themselves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// One entry of a potion's `custom_effects` list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PotionEffect {
    /// Vanilla status-effect id (e.g. `minecraft:regeneration`), validated
    /// against the pinned registry (`DW0486`).
    pub effect: String,
    /// How long it lasts, in **ticks** (20 = one second). Required for every
    /// effect except the two [`INSTANT_EFFECTS`], which must NOT declare one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<u32>,
    /// Amplifier, 0 = level I (vanilla's unsigned byte, so 0–255). Absent = 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amplifier: Option<u32>,
}

impl PotionEffect {
    /// True if this effect is applied once on drinking rather than over time.
    pub fn is_instant(&self) -> bool {
        let norm = if self.effect.contains(':') {
            self.effect.clone()
        } else {
            format!("minecraft:{}", self.effect)
        };
        INSTANT_EFFECTS.contains(&norm.as_str())
    }
}

/// The largest `duration` a potion effect may declare, in ticks: 1 000 000 ticks
/// ≈ 13.9 hours, past the 10-hour delve ceiling, so nothing a delve can legally
/// need is refused — while a duration typed in *milliseconds*, or one that would
/// overflow vanilla's int, is caught (`DW0486`).
pub const MAX_POTION_DURATION_TICKS: u32 = 1_000_000;

/// The largest `amplifier` a potion effect may declare: vanilla stores it in an
/// unsigned byte, so 255 is not a policy but the end of the field.
pub const MAX_POTION_AMPLIFIER: u32 = 255;

/// The canonical English title of the bonfire rest dialog (owner ruling
/// 2026-08-03). Baked at emit time when the campaign authors no `prompt`, in the
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
    /// The campaign's declared **story forks** (DSL v0.8, spec-0025; reserved
    /// `DW0141` earlier). Empty/absent = a campaign that claims to have no
    /// branch — which the compiler then *verifies* rather than assumes: any flag
    /// that gates casts, staging or structure and is set on some playthroughs and
    /// not others belongs to no declared point and is `DW0480`.
    ///
    /// Enumerated branches are the **product of the declared points**, so the
    /// branch set is authored and small — never a combinatorial sweep of every
    /// flag in the campaign.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub branch_points: Vec<BranchPoint>,
}

/// One declared story fork (DSL v0.8, spec-0025).
///
/// A branch point names the flag set the story forks on, the quest at which the
/// fork opens, and every branch it offers. Each branch pins the point's whole
/// flag set: the flags it lists are **set**, and every other flag of `forks_on`
/// is **not** — which is what makes exclusive-content leakage (`DW0484`) and
/// per-branch cast resolution (`DW0483`) decidable instead of hopeful.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BranchPoint {
    /// Unique branch-point id.
    pub id: BranchPointId,
    /// The quest at which the fork opens — every branch's divergent content is at
    /// or after it in the stage-4 DAG.
    pub opens_at: QuestId,
    /// The flag set the story forks on. Every branch's `flags` is a subset of
    /// this, and the flags it does not list are pinned **unset** on that branch.
    pub forks_on: Vec<FlagId>,
    /// The alternatives (≥ 2).
    pub branches: Vec<BranchDecl>,
}

/// One alternative of a [`BranchPoint`] (DSL v0.8, spec-0025).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BranchDecl {
    /// Unique branch id (campaign-wide — it names the emitted chronicle file).
    pub id: BranchId,
    /// The subset of the point's `forks_on` that is SET on this branch. The rest
    /// of `forks_on` is pinned unset. An empty list is legal — the "took neither
    /// option" branch — as long as it is genuinely reachable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<FlagId>,
    /// Where this branch goes: either the `quest/<kebab>` the branches converge
    /// at, or the `ending/<kebab>` this branch runs to.
    ///
    /// One field, not two mutually exclusive ones, because the **id prefix
    /// already says which it is** — the same convention every cross-stage
    /// reference in the DSL uses. That is what makes "exactly one of them" an
    /// unrepresentable state rather than a rule some diagnostic has to police: a
    /// value that is neither a syntactically valid `quest/…` nor `ending/…` is
    /// the ordinary malformed-id `DW0110`, and one that names nothing is the
    /// ordinary dangling-reference `DW0112`.
    pub leads_to: String,
}

impl BranchDecl {
    /// The convergence quest, if [`Self::leads_to`] names one.
    pub fn converges_at(&self) -> Option<QuestId> {
        let q = QuestId(self.leads_to.clone());
        q.is_valid_syntax().then_some(q)
    }

    /// The ending, if [`Self::leads_to`] names one.
    pub fn ending(&self) -> Option<EndingId> {
        let e = EndingId(self.leads_to.clone());
        e.is_valid_syntax().then_some(e)
    }
}

/// What a story node does to the story (DSL v0.8, spec-0025).
///
/// The generalization of spec-0020's `doing` from NPC presence to event flow: a
/// design that never got written down node by node cannot compile. It is
/// **node-local on purpose** — there is no parallel per-branch script document
/// that could itself drift from the graph.
///
/// `text` is authoring/validation metadata, never shown to a player, so it is
/// deliberately **excluded from the l10n inventory** exactly like `doing`. The
/// compiler reads only `verb` and `subject`; `text` is the flesh the per-branch
/// chronicle is assembled from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Happening {
    /// The structured event verb — the machine-decidable part.
    pub verb: HappeningVerb,
    /// One line of prose stating what this node does to the story.
    pub text: String,
    /// What the event happens TO: an `npc/`, `actor/`, `wave/` or `anchor/` id
    /// (validated — a dangling one is `DW0112`), or an `item/<kebab>` label for a
    /// story token the campaign tracks by hand. Optional, because not every beat
    /// is about somebody; the hard-contradiction proof (`DW0485`) reasons only
    /// over the beats that name one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
}

/// The structured event vocabulary (DSL v0.8, spec-0025).
///
/// Deliberately small and closed. These ten verbs are what make a subset of
/// narrative errors machine-decidable per branch (`DW0485`); everything else a
/// beat means lives in [`Happening::text`], which the compiler never interprets.
/// Extend only when a real campaign cannot state its beat with what is here.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum HappeningVerb {
    /// The subject is killed / destroyed. Terminal: nothing the subject does may
    /// follow it on the same branch.
    Dies,
    /// The subject comes through alive — the explicit counterpart of `dies`,
    /// which is what lets a branch state that somebody *did not* die.
    Survives,
    /// The subject leaves the stage (offstage, not dead). Cleared by `arrives`.
    Departs,
    /// The subject enters the stage.
    Arrives,
    /// Somebody learns a fact — the true-information beat.
    Learns,
    /// Somebody comes to believe something (whether or not it is true) — the beat
    /// that carries a wrong belief forward, which is where branch drift shows.
    Believes,
    /// The party (or the subject) gains a thing.
    Gains,
    /// The party (or the subject) loses a thing. A second `loses` with no
    /// intervening `gains` is spending what is already spent (`DW0485`).
    Loses,
    /// A way is opened.
    Opens,
    /// A way is sealed. Cleared by `opens`.
    Seals,
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
    /// Shortcut doors (spec-0016 §2): a gate that is sealed from world-load and
    /// is opened — permanently — from the FAR side. Empty/absent in pre-0.6
    /// campaigns (reserved `DW0141`), so a campaign that declares none stays
    /// byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shortcuts: Vec<Shortcut>,
    /// Ambushes (spec-0016 §3): sugar over "deferred actors + a trigger that
    /// springs them". Empty/absent in pre-0.6 campaigns (reserved `DW0141`).
    ///
    /// **Never serialized.** [`parse_campaign`](crate::parse_campaign) expands
    /// each ambush into `triggers`, so the canonical form of a campaign is its
    /// **desugared** form. That is what keeps the canonical round-trip idempotent
    /// (re-parsing canonical output finds no `ambushes` and so cannot expand a
    /// second time and duplicate the trigger ids), and it means the sugar exists
    /// at exactly one layer boundary — the authored `.json` — with nothing
    /// downstream needing to know it was ever there. The list itself is kept in
    /// memory so diagnostics can name the ambush the author wrote.
    /// Timed gates (spec-0016 §4): gates on a deterministic open/close clock.
    /// Empty/absent in pre-0.6 campaigns (reserved `DW0141`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub timed_gates: Vec<TimedGate>,
    /// Container fills (spec-0021): pre-placed chests/barrels in the prefabs
    /// given contents at world init. Empty/absent in pre-0.6 campaigns
    /// (reserved `DW0141`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loot: Vec<Loot>,
    #[serde(default, skip_serializing)]
    pub ambushes: Vec<Ambush>,
    /// Whether [`Self::expand_ambushes`] has already run (never serialized). The
    /// authored `ambushes` are deliberately KEPT after expansion so validation
    /// and the counterplay proof can attribute diagnostics to the ambush the
    /// author actually wrote; this flag is what makes a second expansion a no-op.
    #[serde(skip)]
    pub ambushes_expanded: bool,
}

impl QuestsContent {
    /// Every environment trigger this stage produces: the authored `triggers`
    /// followed by the ones each `ambush` desugars to (spec-0016 §3), in
    /// declared order.
    ///
    /// **This is the single trigger authority.** Validation, the l10n
    /// inventory, the flag/wave producer scans, the nav proofs and emission all
    /// read triggers through it, so an ambush behaves exactly like the trigger
    /// an author would otherwise hand-write — there is no second code path for
    /// the sugar to drift down. Deterministic (declaration order, no hashing).
    pub fn all_triggers(&self) -> Vec<EnvTrigger> {
        let mut out = self.triggers.clone();
        out.extend(self.ambushes.iter().map(Ambush::to_trigger));
        out
    }

    /// Desugar every `ambush` into a real environment trigger and clear the
    /// ambush list (spec-0016 §3). Called once, by
    /// [`parse_campaign`](crate::parse_campaign); idempotent by construction
    /// (a second call sees no ambushes left to expand).
    pub fn expand_ambushes(&mut self) {
        if self.ambushes_expanded || self.ambushes.is_empty() {
            return;
        }
        self.triggers = self.all_triggers();
        self.ambushes_expanded = true;
    }
}

/// A stage-5 container fill (DSL v0.6, spec-0021): contents for a chest or
/// barrel the prefab already placed.
///
/// The container is **hardware the prefab authored**, exactly like a trap's
/// dispenser: this declaration gives an already-placed, already-lit, already
/// composed piece of furniture its contents. The compiler never places the
/// container itself — if the anchor's cell does not already hold one, that is a
/// content defect and a build error (`DW0431`), not something to paper over by
/// setblock-ing a chest into a wall.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Loot {
    /// Unique loot id (`loot/<kebab>`).
    pub id: LootId,
    /// The anchor whose cell holds the container to fill.
    pub anchor: AnchorId,
    /// Contents, in declaration order. Slot assignment is positional and
    /// deterministic — the first entry lands in `container.0`, the second in
    /// `container.1`, and so on (ADR-0006: no RNG, no loot tables).
    pub items: Vec<LootItem>,
}

/// One stack inside a [`Loot`] container.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LootItem {
    /// Item id (e.g. `minecraft:cooked_cod`). Validated against the pinned
    /// 1.21.11 item registry (`DW0143`).
    pub item: String,
    /// Stack size. Defaults to 1.
    #[serde(default = "one_u32")]
    pub count: u32,
    /// Optional custom item name. Enters the l10n string inventory exactly like
    /// a class kit item's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Enchantments on this stack (`{"minecraft:sharpness": 3}`), emitted as the
    /// 1.21 `minecraft:enchantments` item component.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub enchantments: BTreeMap<String, u32>,
}

fn one_u32() -> u32 {
    1
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
    /// The **legacy** redstone consequence (spec-0011): a static dispenser
    /// payload the prefab's own wiring fires. Superseded by [`Trap::payload`]
    /// (spec-0022) — redstone now keeps only the trigger — but kept meaningful
    /// so existing campaigns build unchanged. Optional since spec-0022; a trap
    /// must declare `effect`, `payload`, or both (`DW0440`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect: Option<TrapEffect>,
    /// The **command payload** (spec-0022): an ordered effect list in the same
    /// vocabulary quests use, run when the trigger fires. This is where a trap's
    /// consequence lives now — the compiler owns the detection tick and the
    /// effect vocabulary, so a trap's payload is authored like any other effect
    /// bundle rather than built out of dust and repeaters. Expressiveness moves
    /// from "what dust can carry" to "what the effect vocabulary can say":
    /// `volley` and `collapse` (spec-0022's trap verbs) join `damage-players`,
    /// `play-sound`, `narrate`, `set-flag` and `spawn-wave`.
    ///
    /// Empty = a pure spec-0011 redstone trap, which emits exactly what it
    /// emitted before (byte-identical).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub payload: Vec<QuestEffect>,
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
    /// Negative flag gate (DSL v0.6): the trap is considered inactive while ANY
    /// listed flag is set (mirrors [`EnvTrigger::forbids_flags`]). Default empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forbids_flags: Vec<FlagId>,
}

impl Trap {
    /// `(item, count)` if this trap declares a legacy `dispense` effect.
    pub fn dispense(&self) -> Option<(&str, u32)> {
        match &self.effect {
            Some(TrapEffect::Dispense { item, count }) => Some((item.as_str(), *count)),
            None => None,
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

/// A stage-5 **timed gate** (spec-0016 §4): a gate region driven by a
/// deterministic open/close clock, so passage is a timing read rather than a
/// permanent state.
///
/// **The proof is deliberately NOT all-phase passability** (owner ruling
/// 2026-08-02) — a gate that punishes bad timing is the entire point. What the
/// compiler requires is that the gate is *readable*: the set of entry phases from
/// which a walking player clears the span before it shuts must cover **≥ 20% of
/// the cycle** (`DW0378`). Below that it is a coin flip, not a skill, and no
/// amount of learning the level makes it fair.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimedGate {
    /// Unique timed-gate id (`timed-gate/<kebab>`).
    pub id: TimedGateId,
    /// The gate anchor the clock drives. Its prefab metadata must declare a fill
    /// `block` (`DW0343`), the same requirement `close-gate` and `shortcut` have.
    pub gate: AnchorId,
    /// Ticks the gate stays OPEN each cycle (> 0).
    pub open_ticks: u32,
    /// Ticks the gate stays CLOSED each cycle (> 0).
    pub closed_ticks: u32,
    /// Ticks after world init before the first open window begins (default 0) —
    /// how two gates in the same room are put out of step with each other. Must
    /// be less than the full cycle.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub phase: u32,
    /// Whether the gate **kills** a player caught inside its region when it
    /// shuts (owner directive 2026-08-03, spec-0016 §4 addendum). A portcullis
    /// that merely shoves you aside teaches nothing; mistiming the crossing is
    /// supposed to be a death you learn from, which is the whole point of §4's
    /// ≥20%-of-cycle window proof — the window is fair, so the penalty can be
    /// absolute.
    ///
    /// This is a real judgement issued by command on the closing tick, not
    /// suffocation: vanilla's in-wall damage is slow, gear-dependent and
    /// escapable, so it would make the portcullis a suggestion. `damage` at the
    /// closing edge is exact and unarguable.
    ///
    /// **Defaults to `false`**, so every campaign authored before this field
    /// existed compiles byte-identically; a delve opts its portcullis in.
    #[serde(default, skip_serializing_if = "is_false")]
    pub crush: bool,
    /// Optional **disarm** affordance (task #184, souls dossier §5.2): the third
    /// rung of the hazard ladder — readable, avoidable, and finally *disable-able*.
    /// The real games' best timed hazards can be removed for good (Smouldering
    /// Lake's ballista, the Fringefolk chariot); a clock the party can only ever
    /// dance with is one rung short of the vocabulary.
    ///
    /// Interacting with the affordance suppresses the clock **permanently, with
    /// the gate resting OPEN** — a jammed portcullis stays up. Permanence is
    /// structural exactly as a `shortcut`'s is: no emitted function ever re-arms
    /// the clock, and `DW0389` refuses a campaign that spells a re-seal.
    ///
    /// **Defaults to absent**, so every campaign authored before this field
    /// existed compiles byte-identically; a delve opts its portcullis in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disarm: Option<TimedGateDisarm>,
}

/// A [`TimedGate`]'s disarm affordance (task #184, souls dossier §5.2) — the
/// exact shape a trap's [`TrapDisarm`] takes, and deliberately so: one affordance
/// grammar for every mechanism the party can switch off.
///
/// The player acts on the `via` anchor (a compiler-emitted interaction entity
/// plus its visible hardware, `DW0420`) to jam the gate. The clock stops with the
/// span cleared, `sets_flag` is raised party-wide, and nothing in the delve can
/// put the gate back.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimedGateDisarm {
    /// The anchor the player interacts with to jam the gate. Must be an anchor
    /// some area's prefab provides, and never the gate anchor itself — the
    /// mechanism belongs beside the doorway, not inside the span that crushes.
    pub via: AnchorId,
    /// The flag set when the gate is disarmed (a new flag this gate produces;
    /// other objectives/triggers may read it via `requires_flags`).
    pub sets_flag: FlagId,
}

/// serde `skip_serializing_if` helper: skip a `0`.
fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// A stage-5 **ambush** (spec-0016 §3) — one declaration for a beat that
/// otherwise takes a deferred actor set plus a hand-wired trigger.
///
/// **`telegraph` is optional, and that is a design ruling, not an oversight**
/// (owner, 2026-08-02). The un-telegraphed ambush — the shove off the cliff you
/// could not have known about — is core souls vocabulary: 初见杀 is how the level
/// teaches. The engine does not sand that edge off.
///
/// What the engine *does* owe the player is **counterplay on the retry**: having
/// died once, an informed player must have something to do about it. Determinism
/// guarantees the second attempt meets the same ambushers in the same cells; the
/// compiler adds the missing half — `DW0376` proves the trigger cell is not a
/// sealed pocket, i.e. that with every ambusher standing where it will stand,
/// a route out still exists (a retreat, luring ground, a positioning line).
/// Dying uninformed is a lesson; dying with no play available is a broken beat.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Ambush {
    /// Unique ambush id (`ambush/<kebab>`).
    pub id: AmbushId,
    /// The anchor the trigger watches — the corner, doorway or ledge the player
    /// walks into.
    pub at: AnchorId,
    /// The stage-5 actors that spring (1..N). Each is summoned at its own
    /// declared anchor and immediately unleashed to real AI.
    pub actors: Vec<ActorId>,
    /// What springs it (`approach{range}` / `strike` / `use`), exactly the v0.4
    /// environment-trigger vocabulary.
    pub trigger: TriggerOn,
    /// The **optional** tell, fired at the trigger before the ambushers exist:
    /// a sound, a shadow, a line of narration. Empty = un-telegraphed, which is
    /// fully legal (owner ruling 2026-08-02).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub telegraph: Vec<QuestEffect>,
}

impl Ambush {
    /// The environment trigger this ambush desugars to (spec-0016 §3): a
    /// one-shot trigger at `at` whose effects are the telegraph, then a
    /// `spawn-actor` + `unleash-actor` per listed actor, in declared order.
    ///
    /// This is the **single** expansion authority. Every consumer — validation,
    /// the l10n inventory, the flag/wave producer scans, nav, emission — reads
    /// triggers through [`QuestsContent::all_triggers`], so the sugar cannot
    /// diverge from a hand-written equivalent, and an ambush is exactly as
    /// debuggable as the trigger an author would otherwise type.
    pub fn to_trigger(&self) -> EnvTrigger {
        let mut effects = self.telegraph.clone();
        for a in &self.actors {
            effects.push(QuestEffect::SpawnActor {
                actor: a.clone(),
                happening: None,
            });
        }
        for a in &self.actors {
            effects.push(QuestEffect::UnleashActor {
                actor: a.clone(),
                happening: None,
            });
        }
        EnvTrigger {
            id: TriggerId(format!(
                "trigger/{}",
                crate::l10n::local_id(self.id.as_str())
            )),
            at: Some(self.at.clone()),
            on: self.trigger.clone(),
            requires_flags: Vec::new(),
            forbids_flags: Vec::new(),
            once: true,
            effects,
        }
    }
}

/// A stage-5 **shortcut door** (spec-0016 §2) — the souls loop-back.
///
/// The owner's definition of the pattern: between two rest points there are two
/// routes. The **short** one starts sealed and holds nothing; the **long** one is
/// full of enemies and mechanisms. You earn the far side the hard way, pull one
/// mechanism, and the short route opens **forever**. That moment is the design.
///
/// The compiler owns three obligations, none of them optional:
/// 1. the `unlock` affordance is reachable while the gate is still sealed — the
///    long route genuinely exists (`DW0373`);
/// 2. opening the gate genuinely shortens the trip across it — a shortcut that
///    pays nothing is a leak, not a shortcut (`DW0360`);
/// 3. permanence is **structural**: no `close-gate` may target a shortcut gate
///    (`DW0372`). There is no re-sealing verb to reach for.
///
/// `close-gate` on a NON-shortcut gate (the point-of-no-return staging beat) is
/// untouched by this — the two verbs are deliberately disjoint.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Shortcut {
    /// Unique shortcut id (`shortcut/<kebab>`).
    pub id: ShortcutId,
    /// The gate anchor this shortcut opens. Sealed from world-load (the prefab
    /// carries the physical fill), and its metadata must declare the fill `block`
    /// the compiler clears — the same requirement `close-gate` has.
    pub gate: AnchorId,
    /// The FAR-side anchor whose interaction fires the permanent open. The
    /// compiler summons the affordance there and polls it, reusing the v0.4
    /// interaction-entity `use` primitive.
    pub unlock: AnchorId,
    /// Effects fired once, when the shortcut opens — the bar lifting, the
    /// elevator descending, the sound of a door you will never have to earn
    /// again. Emitted server-source-safe (the poll lives on the tick).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_unlock: Vec<QuestEffect>,
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
    /// The anchor this trigger watches. Required for `strike` / `use` /
    /// `approach`, which watch a *place*; **absent** for `strike-npc` (DSL
    /// v0.6), which watches a *character* and names it in `on.npc` instead —
    /// there is no cell for the author to supply and no cell the compiler
    /// would use. Either mismatch is `DW0194`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<AnchorId>,
    /// The event that fires it.
    pub on: TriggerOn,
    /// Flags that must be set before the trigger can fire (DSL v0.4).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_flags: Vec<FlagId>,
    /// Negative flag gate (DSL v0.6, reserved `DW0141` earlier): the trigger is
    /// **suppressed** while ANY listed flag is set (by any player — flags are
    /// campaign state). The dual of `requires_flags`, so an "armed between two
    /// story beats" trigger needs no re-arm plumbing: e.g. a strike-the-giant
    /// retaliation trigger with `requires_flags: [flag/sealed]` and
    /// `forbids_flags: [flag/asleep]` arms when the cave seals and stands down
    /// the moment the wake beat takes over.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forbids_flags: Vec<FlagId>,
    /// Fire at most once (default `true`, mirroring objective completion). Set
    /// `false` to allow re-firing every time the condition is met.
    #[serde(default = "default_true")]
    pub once: bool,
    /// Effects fired when the trigger matches.
    pub effects: Vec<QuestEffect>,
}

impl EnvTrigger {
    /// The anchor this trigger watches, if it watches a place at all. `None`
    /// for `strike-npc`, whose target is a character.
    pub fn at_anchor(&self) -> Option<&str> {
        self.at.as_ref().map(|a| a.as_str())
    }
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
    /// The player attacks (left-clicks) an **NPC's body** (DSL v0.6, reserved
    /// `DW0141` earlier).
    ///
    /// The place-based [`TriggerOn::Strike`] cannot express "hit the giant": it
    /// summons its own `minecraft:interaction` at a *cell*, and a large NPC's
    /// body eclipses that cell (`DW0359`), so the click never reaches the
    /// trigger — the owner's island round-7 finding. This form has no cell. It
    /// rides the interaction entity the NPC already owns, which is the entity a
    /// click on that NPC reaches by construction.
    ///
    /// Right-click and left-click stay separate all the way down: a
    /// `minecraft:interaction` records them in two distinct NBT fields
    /// (`interaction` and `attack`), so the NPC's dialogue keeps the right-click
    /// and this trigger takes the left-click, on one shared hitbox.
    StrikeNpc {
        /// The NPC (stage-2 ref) whose body is the target.
        npc: NpcId,
    },
}

impl TriggerOn {
    /// The kebab tag (`strike` / `use` / `approach` / `strike-npc`).
    pub fn kind(&self) -> &'static str {
        match self {
            TriggerOn::Strike => "strike",
            TriggerOn::Use => "use",
            TriggerOn::Approach { .. } => "approach",
            TriggerOn::StrikeNpc { .. } => "strike-npc",
        }
    }

    /// Whether this event needs an `at` anchor — true for everything that
    /// watches a place, false for `strike-npc`, which watches a character.
    pub fn needs_anchor(&self) -> bool {
        !matches!(self, TriggerOn::StrikeNpc { .. })
    }

    /// The NPC whose body this event watches (`strike-npc` only).
    pub fn npc_target(&self) -> Option<&NpcId> {
        match self {
            TriggerOn::StrikeNpc { npc } => Some(npc),
            _ => None,
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
    /// Re-seat this wave every time the party rests at (or respawns from) a
    /// bonfire (spec-0016 §1) — the souls contract: progress is kept, the
    /// enemies come back. The compiler kills any survivor carrying the wave tag
    /// and re-runs the wave's own spawn function, so the room is restored to its
    /// authored composition and spawn cells.
    ///
    /// Inert without a `bonfire` in the campaign, which is a compile error
    /// (`DW0370`) rather than a silent no-op.
    #[serde(default, skip_serializing_if = "is_false")]
    pub respawns_on_rest: bool,
    /// Tower-defense lane routing (spec-0016 §6): march this wave along a
    /// waypoint polyline while distant, hand it to native AI the instant a
    /// player is inside `aggro_radius`. Absent = today's behaviour (spawn and
    /// stand), byte-identical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane: Option<WaveLane>,
    /// Where the wave's mobs materialize (spec-0016 §6). Absent =
    /// [`WaveSummon::Anchor`], the pre-0.6 behaviour: standable cells around the
    /// wave `anchor`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summon: Option<WaveSummon>,
    /// How hard this encounter is *meant* to be (DSL v0.7, spec-0023). Absent =
    /// [`EncounterTier::Ordinary`], byte-identical to every pre-0.7 campaign.
    ///
    /// This is a **declaration, not a knob**: the compiler never scales content
    /// from it (spec-0023 "Out of scope"). It exists because the validation
    /// ladder's inverted floor gate needs to know which fights the content
    /// *claims* are hard — an `elite`/`boss` encounter the unassisted bot beats
    /// on its first attempt is reported as too easy for its billing. Marking it
    /// is how the author opts into that scrutiny; the alternative — inferring
    /// "elite" from how tuned a stack looks — is exactly the downstream folklore
    /// CLAUDE.md's no-hack rule forbids.
    ///
    /// **It does reach emission in exactly one place** (spec-0016 §1, owner
    /// ruling 2026-08-05): in a campaign with a `bonfire`, a billed `elite`/
    /// `boss` wave that does not declare `respawns_on_rest` is refreshed by a
    /// rest *while it is still standing* — deleted and re-seated at full count
    /// and full health, so chipping it down one life at a time is never a path.
    /// Beat it and it stays beaten. `DW0499` forbids billing a wave `boss` and
    /// `respawns_on_rest` at once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<EncounterTier>,
}

/// What a wave is billed as (DSL v0.7, spec-0023). Consumed by the validation
/// ladder (the run's combat plan) and — since spec-0016 §1's undefeated re-seat
/// — by one emission site: a bonfire refreshes a billed wave that is still
/// standing. Nothing about the encounter itself is scaled from it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum EncounterTier {
    /// Trash / pressure. No floor expectation: a bot that wins it cold proves
    /// nothing either way. The default.
    #[default]
    Ordinary,
    /// A set-piece the content bills as a hard fight (spec-0016's optional-elite
    /// and spawn-and-unleash vocabulary).
    Elite,
    /// A campaign's named fight. Same floor rule as `elite`; the distinction is
    /// for the run report a human reads.
    Boss,
}

impl EncounterTier {
    /// The kebab tag, as it appears in the DSL and in the emitted combat plan.
    pub fn token(self) -> &'static str {
        match self {
            EncounterTier::Ordinary => "ordinary",
            EncounterTier::Elite => "elite",
            EncounterTier::Boss => "boss",
        }
    }

    /// Does the inverted floor gate (spec-0023) apply to this tier? A fight the
    /// content bills as hard carries an expectation the bot can measure; an
    /// ordinary one does not.
    pub fn has_floor_expectation(self) -> bool {
        matches!(self, EncounterTier::Elite | EncounterTier::Boss)
    }
}

/// Where a wave's mobs materialize (spec-0016 §6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum WaveSummon {
    /// Standable cells around the wave `anchor`, nearest first — the pre-0.6
    /// behaviour and the default.
    Anchor,
    /// **Spirit-summoned at the edge of perception** (owner design 2026-08-02):
    /// each mob appears on the ring at its own `attributes.follow_range` from
    /// the wave `anchor`, so it acquires a target the instant it exists and
    /// closes under pure native AI. Species without patrol AI never march a
    /// lane; this is what they do instead — never a spawn on top of the party,
    /// never a mob that brushes past.
    ///
    /// With this mode the wave `anchor` is the **defended point** (what the ring
    /// is drawn around), not the spawn point. Each mob stack must declare its
    /// own `attributes.follow_range` (`DW0385`) — the ring radius is authored,
    /// never guessed from a vanilla defaults table the compiler cannot verify.
    AggroEdge,
}

/// Tower-defense lane routing for a wave (spec-0016 §6), built on vanilla's
/// **Raider patrol system** — the intended primitive, live-verified on 1.21.11
/// (`docs/notes/td-routing-spike.md`).
///
/// The squad spawns `Patrolling:1b` with one `PatrolLeader:1b` and a snake_case
/// `patrol_target` int-array; a compiler-emitted clock walks the shared waypoint
/// index forward, and per mob a player-proximity check releases `Patrolling:0b`.
/// From that instant the mob is a plain native hostile. "Combat preempts
/// routing" is engine semantics — vanilla's patrol goal is hard-gated on having
/// no target — so the owner's rule (march while distant, fight with NATIVE AI
/// once aggroed, never brush past) falls out of the primitive unforced.
///
/// Lanes are raider-family only (`DW0382`), squad ≥ 2 (`DW0383`), and a lane
/// pillager must keep its crossbow (`DW0384`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WaveLane {
    /// The lane polyline, in march order: anchors some area's prefab provides.
    /// At least one; consecutive legs (counting the wave `anchor` as the origin)
    /// must be more than 10 blocks apart — vanilla re-rolls a patrol target
    /// within 10 blocks of arrival, so a tighter lane is a lane the engine
    /// quietly stops following (`DW0386`).
    pub waypoints: Vec<AnchorId>,
    /// The release radius, in blocks: the compiler sets every lane mob's
    /// `follow_range` attribute to exactly this and releases `Patrolling:0b`
    /// at the same distance. The two MUST be equal — a patrolling raider that
    /// targets a player it cannot engage holds ground instead of marching, so a
    /// per-mob `attributes.follow_range` that disagrees is `DW0381`.
    pub aggro_radius: u32,
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
    /// Optional worn/held equipment (DSL v0.6, task #65). A helmet is the
    /// sanctioned fix for daylight-burning undead (owner ruling) — never
    /// `set-time`. Item ids validate against the pinned 1.21.11 item registry
    /// (`DW0143`, the give-item family); every emitted slot carries drop
    /// chance 0 so players can never farm wave gear (no-grind constitution).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equipment: Option<MobEquipment>,
    /// What this mob leaves behind when it dies (DSL v0.9, task #179; reserved
    /// `DW0141` earlier). Only an `elite`/`boss` wave may declare it (`DW0491`)
    /// — an ordinary mob's kit is never farmable. Empty = today's behaviour,
    /// drop chance 0 on every slot.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drops: Vec<MobDrop>,
}

/// Worn/held equipment for a wave mob (DSL v0.6). Each field is a vanilla item
/// id for the matching vanilla equipment slot; an unset slot stays empty —
/// except `main_hand`, where the compiler's armed-mob default (skeleton bow,
/// wither-skeleton sword) still applies unless overridden. Emitted as the
/// component-era `equipment`/`drop_chances` summon NBT (1.21.11 silently
/// ignores legacy `ArmorItems`/`HandItems` on `/summon`), all drop chances 0.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MobEquipment {
    /// Head slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<EquipItem>,
    /// Chest slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chest: Option<EquipItem>,
    /// Legs slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legs: Option<EquipItem>,
    /// Feet slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feet: Option<EquipItem>,
    /// Main-hand slot. Overrides the compiler's armed-mob default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_hand: Option<EquipItem>,
    /// Off-hand slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub off_hand: Option<EquipItem>,
}

impl MobEquipment {
    /// Every slot as `(dsl_field_name, piece)`, in the fixed schema order —
    /// the single iteration source for validation paths and emission.
    pub fn slots(&self) -> [(&'static str, Option<&EquipItem>); 6] {
        [
            ("head", self.head.as_ref()),
            ("chest", self.chest.as_ref()),
            ("legs", self.legs.as_ref()),
            ("feet", self.feet.as_ref()),
            ("main_hand", self.main_hand.as_ref()),
            ("off_hand", self.off_hand.as_ref()),
        ]
    }

    /// The piece this equipment declaration puts in `slot`, if any. The single
    /// question a `drops[]` `slot` entry asks: a mob can only drop a piece it
    /// actually wears (`DW0490`).
    pub fn filled(&self, slot: EquipSlot) -> Option<&EquipItem> {
        match slot {
            EquipSlot::Head => self.head.as_ref(),
            EquipSlot::Chest => self.chest.as_ref(),
            EquipSlot::Legs => self.legs.as_ref(),
            EquipSlot::Feet => self.feet.as_ref(),
            EquipSlot::MainHand => self.main_hand.as_ref(),
            EquipSlot::OffHand => self.off_hand.as_ref(),
        }
    }
}

/// One vanilla equipment slot, named exactly as the [`MobEquipment`] field that
/// fills it (DSL v0.9, task #179). The DSL name and the summon-NBT key differ
/// (`main_hand` vs `mainhand`), so both live here and nowhere else.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EquipSlot {
    /// Head slot.
    Head,
    /// Chest slot.
    Chest,
    /// Legs slot.
    Legs,
    /// Feet slot.
    Feet,
    /// Main-hand slot.
    MainHand,
    /// Off-hand slot.
    OffHand,
}

impl EquipSlot {
    /// The DSL field name (`main_hand`), for diagnostics and JSON pointers.
    pub fn field(self) -> &'static str {
        match self {
            EquipSlot::Head => "head",
            EquipSlot::Chest => "chest",
            EquipSlot::Legs => "legs",
            EquipSlot::Feet => "feet",
            EquipSlot::MainHand => "main_hand",
            EquipSlot::OffHand => "off_hand",
        }
    }

    /// The 1.21.11 `equipment` / `drop_chances` NBT key (`mainhand`).
    pub fn nbt(self) -> &'static str {
        match self {
            EquipSlot::Head => "head",
            EquipSlot::Chest => "chest",
            EquipSlot::Legs => "legs",
            EquipSlot::Feet => "feet",
            EquipSlot::MainHand => "mainhand",
            EquipSlot::OffHand => "offhand",
        }
    }
}

/// One declared drop of an elite or boss (DSL v0.9, task #179; owner ruling
/// 2026-08-04).
///
/// A mob may wear many pieces; what it *leaves behind* is a **declared subset**,
/// usually one piece and never automatically everything. Two forms, told apart
/// by which field is present:
///
/// ```json
/// { "slot": "main_hand" }                                  // its axe
/// { "item": "minecraft:tripwire_hook", "name": "Gate Key" } // a quest item
/// ```
///
/// A `slot` entry must name a slot the same entity's `equipment` really fills
/// (`DW0490`) — the drop is the piece the player has been *looking at* through
/// the whole fight, so the two declarations cannot disagree. An `item` entry is
/// a token the fight *yields* rather than wears, and rides the entity's own
/// death loot table.
///
/// Undeclared slots keep drop chance `0.0` — byte-for-byte today's behaviour, and
/// the no-grind constitution's guarantee that an ordinary kit is never farmable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum MobDrop {
    /// A worn/held piece, named by its equipment slot.
    Slot(SlotDrop),
    /// A quest token the fight yields (not worn).
    Item(ItemDrop),
}

/// The worn-piece form of a [`MobDrop`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SlotDrop {
    /// The equipment slot whose piece drops. Must be filled by the entity's own
    /// `equipment` declaration (`DW0490`).
    pub slot: EquipSlot,
}

/// The quest-token form of a [`MobDrop`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ItemDrop {
    /// Vanilla item id, validated against the pinned 1.21.11 registry
    /// (`DW0143`, the give-item family).
    pub item: String,
    /// Display name the dropped stack carries as a `custom_name` component.
    /// Player-visible, so it enters the l10n string inventory and translates
    /// like any other line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl MobDrop {
    /// The equipment slot this entry names, or `None` for a quest-item drop.
    pub fn slot(&self) -> Option<EquipSlot> {
        match self {
            MobDrop::Slot(s) => Some(s.slot),
            MobDrop::Item(_) => None,
        }
    }

    /// The item id this entry names, or `None` for a worn-piece drop.
    pub fn item(&self) -> Option<&str> {
        match self {
            MobDrop::Slot(_) => None,
            MobDrop::Item(i) => Some(&i.item),
        }
    }

    /// The declared display name of a quest-item drop, when set.
    pub fn name(&self) -> Option<&String> {
        match self {
            MobDrop::Slot(_) => None,
            MobDrop::Item(i) => i.name.as_ref(),
        }
    }

    /// Mutable access to the display name — the l10n traversal's hook.
    pub fn name_mut(&mut self) -> Option<&mut String> {
        match self {
            MobDrop::Slot(_) => None,
            MobDrop::Item(i) => i.name.as_mut(),
        }
    }
}

/// One equipped item: either a bare item id, or an id carrying enchantments.
///
/// The plain form is the common case and stays a plain JSON string, which is
/// what keeps every campaign written before enchantments existed byte-identical
/// on re-serialisation:
///
/// ```json
/// "main_hand": "minecraft:netherite_sword"
/// "head": { "item": "minecraft:netherite_helmet",
///           "enchantments": { "minecraft:protection": 4 } }
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum EquipItem {
    /// A bare item id — no enchantments.
    Plain(String),
    /// An item id plus its enchantments.
    Enchanted(EnchantedItem),
}

impl EquipItem {
    /// The item id, whichever form was authored.
    pub fn item(&self) -> &str {
        match self {
            EquipItem::Plain(s) => s,
            EquipItem::Enchanted(e) => &e.item,
        }
    }

    /// The enchantments on this piece — empty for the plain form. `BTreeMap`
    /// ordered, so emission order is the id order and never hash order
    /// (ADR-0006).
    pub fn enchantments(&self) -> &BTreeMap<String, u32> {
        static EMPTY: std::sync::LazyLock<BTreeMap<String, u32>> =
            std::sync::LazyLock::new(BTreeMap::new);
        match self {
            EquipItem::Plain(_) => &EMPTY,
            EquipItem::Enchanted(e) => &e.enchantments,
        }
    }
}

/// The enchanted form of [`EquipItem`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EnchantedItem {
    /// Item id (e.g. `minecraft:netherite_chestplate`).
    pub item: String,
    /// Enchantment id → level (e.g. `{"minecraft:protection": 4}`). Emitted as
    /// the 1.21 `minecraft:enchantments` item component.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub enchantments: BTreeMap<String, u32>,
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
    /// The **cast ledger** (DSL v0.7, spec-0020): for every stage-2 NPC that is
    /// live during this quest — spawned and not explicitly removed — where it
    /// stands, what it is doing, and what its right-click offers *for this
    /// quest's duration*.
    ///
    /// The ledger exists because an NPC's dialogue used to be one tree for the
    /// whole campaign: after the climactic escape a crew member still offered
    /// "Tell me what he is." — a premise question absurd once the story moved on.
    /// Declaring the scene per quest makes the compiler able to check it
    /// (`DW0460`–`DW0467`) and makes the declaration itself the gate: the
    /// emitted right-click shows the root this quest declares, so a stale root
    /// retires *because the ledger says so*, not because an author remembered a
    /// flag.
    ///
    /// Empty/absent before v0.7 (`DW0465` deprecation warning for one version
    /// window), so a campaign that declares none compiles byte-identically.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub cast: BTreeMap<NpcId, CastEntry>,
    /// What this quest does to the story (DSL v0.8, spec-0025; required at 0.8.0,
    /// `DW0481`; reserved `DW0141` earlier).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub happening: Option<Happening>,
}

/// One NPC's entry in a quest's [`cast`](Quest::cast) ledger.
///
/// Three authored shapes, tried in order (untagged): the bare keyword `"dead"` /
/// `"offstage"`, a single flat [`CastPlacement`], or a **list** of placements —
/// per-branch casts, each gated by the flags that select its branch (spec-0020
/// proof 4: where the effect history is branch-dependent, a single flat
/// declaration cannot hold on every reachable branch, and `DW0462` says so).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum CastEntry {
    /// The bare keyword form: `"npc/antiphos": "dead"`. Shorthand for a
    /// placement whose `at` is that keyword and which carries no `doing` or
    /// `dialogue` — a character who is not in the world has no business and
    /// answers no right-click.
    Absent(CastAbsence),
    /// A single placement that must hold on every reachable branch.
    Placement(CastPlacement),
    /// Per-branch placements (spec-0020 proof 4). Each carries the
    /// `requires_flags`/`forbids_flags` that select its branch.
    Branches(Vec<CastPlacement>),
}

impl CastEntry {
    /// Every placement this entry declares, in declared order. The bare-keyword
    /// form yields none — there is no scene to check.
    pub fn placements(&self) -> Vec<&CastPlacement> {
        match self {
            CastEntry::Absent(_) => Vec::new(),
            CastEntry::Placement(p) => vec![p],
            CastEntry::Branches(ps) => ps.iter().collect(),
        }
    }

    /// Every placement this entry declares, mutably (the l10n traversal).
    pub fn placements_mut(&mut self) -> Vec<&mut CastPlacement> {
        match self {
            CastEntry::Absent(_) => Vec::new(),
            CastEntry::Placement(p) => vec![p],
            CastEntry::Branches(ps) => ps.iter_mut().collect(),
        }
    }

    /// The bare-keyword absence this entry declares, if it is that form.
    pub fn absence(&self) -> Option<CastAbsence> {
        match self {
            CastEntry::Absent(a) => Some(*a),
            _ => None,
        }
    }

    /// True if this entry declares the NPC out of the world entirely — the bare
    /// keyword, or every placement's `at` being a keyword.
    pub fn is_absent(&self) -> bool {
        match self {
            CastEntry::Absent(_) => true,
            CastEntry::Placement(p) => p.at.absence().is_some(),
            CastEntry::Branches(ps) => {
                !ps.is_empty() && ps.iter().all(|p| p.at.absence().is_some())
            }
        }
    }
}

/// A declared absence: the NPC is deliberately not in the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CastAbsence {
    /// Removed from the story for good (must match a `despawn-npc` with no later
    /// `spawn-npc`).
    Dead,
    /// Not in the world for this quest, but may return (must match a
    /// `despawn-npc`).
    Offstage,
}

impl CastAbsence {
    /// The authored keyword (`dead` / `offstage`).
    pub fn token(self) -> &'static str {
        match self {
            CastAbsence::Dead => "dead",
            CastAbsence::Offstage => "offstage",
        }
    }
}

/// Where a cast entry puts an NPC: a prefab anchor, or a declared absence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum CastPlace {
    /// `"dead"` or `"offstage"` — explicitly not in the world.
    Absent(CastAbsence),
    /// The anchor the NPC stands on for this quest's duration. Must equal the
    /// position the effect history actually produces (`DW0461`): declaring an
    /// anchor does not teleport anybody.
    Anchor(AnchorId),
}

impl CastPlace {
    /// The anchor this place names, if it is an anchor.
    pub fn anchor(&self) -> Option<&AnchorId> {
        match self {
            CastPlace::Anchor(a) => Some(a),
            CastPlace::Absent(_) => None,
        }
    }

    /// The declared absence, if this place is one.
    pub fn absence(&self) -> Option<CastAbsence> {
        match self {
            CastPlace::Absent(a) => Some(*a),
            CastPlace::Anchor(_) => None,
        }
    }

    /// The authored token, for diagnostics.
    pub fn token(&self) -> &str {
        match self {
            CastPlace::Absent(a) => a.token(),
            CastPlace::Anchor(a) => a.as_str(),
        }
    }
}

/// One declared scene for one NPC in one quest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CastPlacement {
    /// Where the NPC is: an anchor, or `"offstage"` / `"dead"`.
    pub at: CastPlace,
    /// What the character is *doing* — free prose, never machine-checked.
    ///
    /// This field is the forcing function, which is the whole reason it is
    /// required (`DW0463`) despite being unverifiable: you cannot fill it in
    /// without deciding the character's business in this beat, and stage 6
    /// receives it as the context the NPC's lines are written against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doing: Option<String>,
    /// What right-click offers during this quest. Required for an on-stage
    /// placement (`DW0463`) — including the explicit `"none"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dialogue: Option<CastDialogue>,
    /// Branch gate (per-branch casts): this placement describes the world only
    /// once every listed flag is set. Mirrors an option's `requires_flags`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_flags: Vec<FlagId>,
    /// Negative branch gate: this placement describes the world only while no
    /// listed flag is set. Mirrors an option's `forbids_flags`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forbids_flags: Vec<FlagId>,
}

/// What an NPC's right-click offers for a quest's duration.
///
/// Untagged, tried in order: the keywords `"none"` / `"unchanged"`, a
/// `{"barks": […]}` pool, then a dialogue root id.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum CastDialogue {
    /// A keyword: `"none"` or `"unchanged"`.
    Keyword(CastDialogueKeyword),
    /// A bark pool: right-click yields one inconsequential in-character line —
    /// no tree, no options, no consequences.
    Barks(CastBarks),
    /// A dialogue root id: right-click opens this node of the NPC's stage-6
    /// tree. Must be a node of *that* NPC's tree (`DW0464`).
    Root(DialogueId),
}

/// The keyword forms of [`CastDialogue`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CastDialogueKeyword {
    /// Genuinely no reaction: the right-click is recorded and consumed, and
    /// nothing opens. Legal, but a last resort — if a body is clickable, the
    /// world should answer; prefer a bark.
    None,
    /// Carry forward whatever this NPC's dialogue was at its **previous**
    /// appearance in the quest-DAG ordering (owner amendment, 2026-08-03).
    ///
    /// The point is that carrying dialogue forward is a *conscious, declared
    /// act*. It is never an implicit default — an omitted `dialogue` is
    /// `DW0463`, not a carry-forward — and writing `"unchanged"` states the
    /// intent without re-spelling a root id that then drifts out of sync. It
    /// resolves transitively (`unchanged` → `unchanged` → a root), and using it
    /// at an NPC's first appearance is `DW0466`: there is nothing to carry.
    ///
    /// Emission is a **no-op** — no root swap is emitted for that NPC at that
    /// quest — which is what makes the sugar cheap and byte-stable.
    Unchanged,
}

/// A bark pool: inconsequential in-character lines, cycled deterministically.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CastBarks {
    /// The lines, in cycle order. At least one (`DW0464`). Player-visible, so
    /// they enter the l10n inventory like any narrate text.
    pub barks: Vec<String>,
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
        /// Negative flag gate (DSL v0.6, reserved `DW0141` earlier): the
        /// objective is suppressed (cannot activate or complete) while ANY listed
        /// flag is set for the player — the dual of `requires_flags`.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        forbids_flags: Vec<FlagId>,
        /// Bot stealth hint (DSL v0.4): mark this leg as one the critical-path
        /// bot should traverse sneaking (sprint disabled). Emitted into
        /// `critical-path.json` as `sneak: true` on the step. Purely a harness
        /// hint; no datapack effect.
        #[serde(default, skip_serializing_if = "is_false")]
        stealth: bool,
        /// What this objective does to the story (DSL v0.8, spec-0025; required
        /// at 0.8.0, `DW0481`; reserved `DW0141` earlier).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        happening: Option<Happening>,
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
        /// Negative flag gate (DSL v0.6, reserved `DW0141` earlier): the
        /// objective is suppressed (cannot activate or complete) while ANY listed
        /// flag is set for the player — the dual of `requires_flags`.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        forbids_flags: Vec<FlagId>,
        /// Bot stealth hint (DSL v0.4): mark this leg as one the critical-path
        /// bot should traverse sneaking (sprint disabled). Emitted into
        /// `critical-path.json` as `sneak: true` on the step. Purely a harness
        /// hint; no datapack effect.
        #[serde(default, skip_serializing_if = "is_false")]
        stealth: bool,
        /// What this objective does to the story (DSL v0.8, spec-0025; required
        /// at 0.8.0, `DW0481`; reserved `DW0141` earlier).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        happening: Option<Happening>,
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
        /// Negative flag gate (DSL v0.6, reserved `DW0141` earlier): the
        /// objective is suppressed (cannot activate or complete) while ANY listed
        /// flag is set for the player — the dual of `requires_flags`.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        forbids_flags: Vec<FlagId>,
        /// Bot stealth hint (DSL v0.4): mark this leg as one the critical-path
        /// bot should traverse sneaking (sprint disabled). Emitted into
        /// `critical-path.json` as `sneak: true` on the step. Purely a harness
        /// hint; no datapack effect.
        #[serde(default, skip_serializing_if = "is_false")]
        stealth: bool,
        /// What this objective does to the story (DSL v0.8, spec-0025; required
        /// at 0.8.0, `DW0481`; reserved `DW0141` earlier).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        happening: Option<Happening>,
    },
    /// Completed when `count` of `item` have been collected (v0.3).
    ///
    /// The items are provided in a container: the compiler's own chest at
    /// `anchor` by default, or — since DSL v0.8 — the prefab's existing
    /// chest/barrel at [`Objective::Collect::container`], optionally carrying an
    /// [`Objective::Collect::item_name`] and padded to read full with
    /// [`Objective::Collect::fill_count`].
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
        /// **Adopt the container the prefab already placed** (DSL v0.8, task #95;
        /// reserved `DW0141` earlier): the anchor whose assembled-world cell holds
        /// a `chest` / `trapped_chest` / `barrel` this collect fills instead of
        /// conjuring its own chest at [`Objective::Collect::anchor`].
        ///
        /// Same division of labour a `loot` entry and a trap's dispenser already
        /// keep with the prefab: furniture belongs in the piece. A beach camp's
        /// barrel is scenery the player has been walking past since minute one —
        /// having the compiler `setblock` a *second*, floating chest beside it to
        /// hold the quest item is exactly the downstream workaround the no-hack
        /// rule forbids. A `container` whose cell holds no container is a build
        /// error (`DW0438`), never a silent fill into a wall.
        ///
        /// The critical-path step's position follows the container (the bot opens
        /// *this* block), and no chest is placed at `anchor` when it is set.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        container: Option<AnchorId>,
        /// **The item comes off a body, not out of a box** (DSL v0.9, task #179;
        /// reserved `DW0141` earlier): the wave whose declared `drops[]` yield
        /// this objective's item. No container is placed — not the compiler's own
        /// chest at `anchor`, not a prefab one — and `container` is therefore
        /// mutually exclusive with it (`DW0100`-adjacent; `DW0492`).
        ///
        /// This is what makes "kill the boss → pick up its key → open the door"
        /// a *proved* chain rather than an authoring intention. The compiler
        /// requires (a) that the named wave really declares an `{item}` drop of
        /// this item (`DW0492`), and (b) that a `kill` objective for that wave
        /// precedes this collect in the objective graph (`DW0493`). The existing
        /// flow machinery then carries the ordering the rest of the way: the
        /// door's `requires_flags` hangs off this collect exactly as it would off
        /// a chest one.
        ///
        /// **Waves only.** An actor's death is not observable by any objective —
        /// there is no vanilla-side signal the flow machinery could consume — so
        /// an actor-gated collect would be an unprovable claim, and per the
        /// no-hack doctrine it is excluded rather than approximated. An actor may
        /// still declare `drops[]`; those drops just cannot gate a quest.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dropped_by: Option<WaveId>,
        /// Display name for the collected item (DSL v0.8, task #95; reserved
        /// `DW0141` earlier), emitted as the vanilla `custom_name` item component.
        ///
        /// A quest item is a *named thing* in the story ("Cheese", "Tide
        /// Ledger"), and a player who opens the barrel must read that name — an
        /// unnamed `minecraft:pumpkin_pie` says nothing about what the quest asked
        /// for. Player-visible, so it enters the l10n string inventory
        /// (`obj.<quest>.<obj>.item_name`) and translates like any other line.
        ///
        /// Naming changes nothing about adjudication: the completion advancement
        /// and the per-tick held check both match on the ITEM ID, which a named
        /// stack still carries.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        item_name: Option<String>,
        /// Padding stacks that make the container **read full** (DSL v0.8, task
        /// #95; reserved `DW0141` earlier). Default `0` = the single required
        /// stack, exactly as every pre-0.8 campaign emits.
        ///
        /// A barrel of cheese that opens on one lonely wheel reads as a bug, and
        /// vanilla's notion of "full" is *occupied slots*, not stack size — so
        /// this counts SLOTS: the objective's own stack lands in `container.0` and
        /// each padding stack repeats it in `container.1`, `container.2`, … Slot
        /// assignment is positional and total, the same determinism story `loot`
        /// tells (ADR-0006): no RNG, no loot tables, nothing to reseed.
        ///
        /// The padding is the same item, so taking the whole barrel still
        /// completes the objective and never over- or under-counts it.
        #[serde(default, skip_serializing_if = "is_zero")]
        fill_count: u32,
        /// Prerequisite objectives.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        after: Vec<ObjectiveId>,
        /// Flags that must be set before this objective activates (v0.3).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
        /// Negative flag gate (DSL v0.6, reserved `DW0141` earlier): the
        /// objective is suppressed (cannot activate or complete) while ANY listed
        /// flag is set for the player — the dual of `requires_flags`.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        forbids_flags: Vec<FlagId>,
        /// Bot stealth hint (DSL v0.4): mark this leg as one the critical-path
        /// bot should traverse sneaking (sprint disabled). Emitted into
        /// `critical-path.json` as `sneak: true` on the step. Purely a harness
        /// hint; no datapack effect.
        #[serde(default, skip_serializing_if = "is_false")]
        stealth: bool,
        /// What this objective does to the story (DSL v0.8, spec-0025; required
        /// at 0.8.0, `DW0481`; reserved `DW0141` earlier).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        happening: Option<Happening>,
    },
    /// Completed by interacting with an entity at `anchor`; if `requires_item` is
    /// set, the item must be **held in the main hand** (v0.3; held semantics since
    /// DSL v0.7 — see [`Objective::Interact::requires_item`]).
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
        /// Item the player must be **holding in the main hand** for the
        /// interaction to complete (optional).
        ///
        /// Held, not merely possessed (owner ruling, 2026-08-03): presenting the
        /// item IS the action — a player who right-clicks a sleeping giant with a
        /// sharpened stake buried in their backpack has not stabbed anything.
        /// Before this ruling the gate read the whole inventory, which made every
        /// `requires_item` interaction fire the moment the item was picked up
        /// anywhere, whatever the player was actually doing with their hands.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        requires_item: Option<String>,
        /// Diegetic feedback for a click that arrives without the required item in
        /// hand (DSL v0.7, reserved `DW0141` earlier): narrated to that player in
        /// chat instead of the silence the gate used to answer with. Requires
        /// `requires_item` (`DW0437`).
        ///
        /// Only fires while the objective is genuinely open — same activation gate
        /// as the affordance itself — so a finished or not-yet-active interaction
        /// stays quiet.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        missing_item_hint: Option<String>,
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
        /// Negative flag gate (DSL v0.6, reserved `DW0141` earlier): the
        /// objective is suppressed (cannot activate or complete) while ANY listed
        /// flag is set for the player — the dual of `requires_flags`.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        forbids_flags: Vec<FlagId>,
        /// Bot stealth hint (DSL v0.4): mark this leg as one the critical-path
        /// bot should traverse sneaking (sprint disabled). Emitted into
        /// `critical-path.json` as `sneak: true` on the step. Purely a harness
        /// hint; no datapack effect.
        #[serde(default, skip_serializing_if = "is_false")]
        stealth: bool,
        /// What this objective does to the story (DSL v0.8, spec-0025; required
        /// at 0.8.0, `DW0481`; reserved `DW0141` earlier).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        happening: Option<Happening>,
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
    /// this field the validation ladder's inverted floor gate could only see
    /// `waves[].tier`, so such a boss was *structurally invisible* to it and an
    /// empty finding list read as a pass while covering nothing.
    ///
    /// Like the wave field this is a **declaration, not a knob**: the compiler
    /// never scales an actor from it, and emission is unchanged whichever tier is
    /// declared. What it buys is scrutiny — the actor enters
    /// `validation/combat-plan.json`, and the compiler states, per tiered actor,
    /// whether the floor gate can measure it and why not when it cannot
    /// (`DW0477`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<EncounterTier>,
    /// What this actor leaves behind when a player kills it (DSL v0.9, task
    /// #179; reserved `DW0141` earlier). Only an `elite`/`boss` actor may
    /// declare it (`DW0491`). Emitted into BOTH the staged puppet and the
    /// unleashed twin, exactly as `equipment` is — the drop belongs to the body,
    /// not to one of its two lifecycles. A `despawn-actor` strips the
    /// declaration off the body before removing it, so re-caging an elite (a
    /// souls re-seat) never scatters its axe.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drops: Vec<MobDrop>,
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

    /// The negative flag gate (DSL v0.6): flags whose being set **suppresses**
    /// this objective. The dual of [`Objective::requires_flags`].
    pub fn forbids_flags(&self) -> &[FlagId] {
        match self {
            Objective::TalkTo { forbids_flags, .. }
            | Objective::ReachAnchor { forbids_flags, .. }
            | Objective::Kill { forbids_flags, .. }
            | Objective::Collect { forbids_flags, .. }
            | Objective::Interact { forbids_flags, .. } => forbids_flags,
        }
    }

    /// What this objective does to the story (DSL v0.8, spec-0025).
    pub fn happening(&self) -> Option<&Happening> {
        match self {
            Objective::TalkTo { happening, .. }
            | Objective::ReachAnchor { happening, .. }
            | Objective::Kill { happening, .. }
            | Objective::Collect { happening, .. }
            | Objective::Interact { happening, .. } => happening.as_ref(),
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

    /// The container this objective ADOPTS (DSL v0.8), if it is a `collect` that
    /// declares one: the anchor whose prefab-placed chest/barrel it fills instead
    /// of conjuring its own chest. `None` on every other objective and on a
    /// `collect` that keeps the compiler-placed chest.
    pub fn collect_container(&self) -> Option<&AnchorId> {
        match self {
            Objective::Collect { container, .. } => container.as_ref(),
            _ => None,
        }
    }

    /// The wave whose declared drops provide this objective's item (DSL v0.9),
    /// if it is a `collect` that declares one. `None` on every other objective
    /// and on a `collect` fed by a container.
    pub fn collect_dropped_by(&self) -> Option<&WaveId> {
        match self {
            Objective::Collect { dropped_by, .. } => dropped_by.as_ref(),
            _ => None,
        }
    }

    /// The padding-stack count of a `collect` (DSL v0.8); `0` for every other
    /// objective and for a `collect` that fills the single required stack only.
    pub fn collect_fill_count(&self) -> u32 {
        match self {
            Objective::Collect { fill_count, .. } => *fill_count,
            _ => 0,
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
///
/// `Debug` is hand-written (see the impl below the enum) because it is a
/// **stable content-key rendering** — the compiler's `sequence_key` hashes
/// `{steps:?}` to name `seq_<hash>` functions, so an effect that uses none of
/// the v0.6 `forbids_flags` / `move-npc on_arrive` fields must render
/// byte-identically to the pre-addition enum (the [`CameraShot`] rule).
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum QuestEffect {
    /// Opens a prefab-declared gate (one-way).
    OpenGate {
        /// The gate anchor to open.
        anchor: AnchorId,
        /// Per-effect flag gate (DSL v0.6); see [`QuestEffect::requires_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
        /// Per-effect negative flag gate (DSL v0.6); see
        /// [`QuestEffect::forbids_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        forbids_flags: Vec<FlagId>,
        /// What this beat does to the story (DSL v0.8, spec-0025; required at
        /// 0.8.0, `DW0481`; reserved `DW0141` earlier). Deliberately absent from
        /// the hand-written `Debug` rendering below: the declaration is
        /// validation metadata with no emission of its own, so a content key can
        /// never move because a beat gained a line of prose.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        happening: Option<Happening>,
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
        /// Per-effect negative flag gate (DSL v0.6); see
        /// [`QuestEffect::forbids_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        forbids_flags: Vec<FlagId>,
        /// What this beat does to the story (DSL v0.8, spec-0025; required at
        /// 0.8.0, `DW0481`; reserved `DW0141` earlier). Deliberately absent from
        /// the hand-written `Debug` rendering below: the declaration is
        /// validation metadata with no emission of its own, so a content key can
        /// never move because a beat gained a line of prose.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        happening: Option<Happening>,
        /// What the seal *says* when a player right-clicks it (DSL v0.8; reserved
        /// `DW0141` earlier). A sealed gate is a wall the party will walk back to
        /// and press: the compiler answers that press on the actionbar. Absent, the
        /// compiler's canonical English is baked in (`The way is sealed.`) exactly
        /// as `world.boundary.message` does; authored, the line is l10n-inventoried
        /// under `<effect-key>.sealed_hint` and translates like every other
        /// player-visible string.
        ///
        /// Unlike `happening`, this **does** print in the hand-written `Debug`
        /// below when present — it changes emission, so two otherwise-identical
        /// sequences that differ only in their seal's answer are different content.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sealed_hint: Option<String>,
    },
    /// Marks the campaign complete (final advancement + credits). Terminal — not
    /// flag-gatable (gating the campaign's own completion is a deadlock footgun),
    /// so this variant carries no `requires_flags`.
    CampaignComplete {
        /// Which ENDING this is (DSL v0.8, spec-0025; reserved `DW0141` earlier).
        /// A campaign with more than one `campaign-complete` has more than one
        /// ending, and a branch that runs to an ending names it here — so the
        /// terminality proof (`DW0482`) can state *which* ending a branch reached
        /// instead of merely that something ended. There is no separate `endings`
        /// section: the set of endings is exactly the set named here, the same
        /// rule flags follow.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ending: Option<EndingId>,
        /// What this beat does to the story (DSL v0.8, spec-0025; required at
        /// 0.8.0, `DW0481`; reserved `DW0141` earlier). Deliberately absent from
        /// the hand-written `Debug` rendering below: the declaration is
        /// validation metadata with no emission of its own, so a content key can
        /// never move because a beat gained a line of prose.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        happening: Option<Happening>,
    },
    /// Gives the party an item (v0.3; party-wide since v0.6/spec-0018).
    GiveItem {
        /// Vanilla item id to give (validated against the registry).
        item: String,
        /// How many to give.
        count: u32,
        /// Optional display name (DSL v0.4), matching [`KitItem::name`].
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        /// Who receives it (DSL v0.6, spec-0018). Absent = [`Carrier::All`] — every
        /// party member. `one` hands a single copy to the player whose action fired
        /// the effect; it is rejected in a scheduler-only bundle (`DW0371`), which
        /// has no acting player.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        carrier: Option<Carrier>,
        /// Per-effect flag gate (DSL v0.6); see [`QuestEffect::requires_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
        /// Per-effect negative flag gate (DSL v0.6); see
        /// [`QuestEffect::forbids_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        forbids_flags: Vec<FlagId>,
    },
    /// Sets a campaign flag, enabling flag-gated objectives (v0.3).
    SetFlag {
        /// The flag to set.
        flag: FlagId,
        /// Per-effect flag gate (DSL v0.6); see [`QuestEffect::requires_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
        /// Per-effect negative flag gate (DSL v0.6); see
        /// [`QuestEffect::forbids_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        forbids_flags: Vec<FlagId>,
    },
    /// Spawns a stage-5 wave's mobs at its anchor (v0.3).
    SpawnWave {
        /// The wave (stage-5 `waves` ref) to spawn.
        wave: WaveId,
        /// Per-effect flag gate (DSL v0.6); see [`QuestEffect::requires_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
        /// Per-effect negative flag gate (DSL v0.6); see
        /// [`QuestEffect::forbids_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        forbids_flags: Vec<FlagId>,
        /// What this beat does to the story (DSL v0.8, spec-0025; required at
        /// 0.8.0, `DW0481`; reserved `DW0141` earlier). Deliberately absent from
        /// the hand-written `Debug` rendering below: the declaration is
        /// validation metadata with no emission of its own, so a content key can
        /// never move because a beat gained a line of prose.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        happening: Option<Happening>,
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
        /// Per-effect negative flag gate (DSL v0.6); see
        /// [`QuestEffect::forbids_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        forbids_flags: Vec<FlagId>,
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
        /// Per-effect negative flag gate (DSL v0.6); see
        /// [`QuestEffect::forbids_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        forbids_flags: Vec<FlagId>,
    },
    /// Despawns an NPC and its interaction hitbox (DSL v0.4, spec-0008 §5).
    DespawnNpc {
        /// The NPC (stage-2 ref) to remove.
        npc: NpcId,
        /// Per-effect flag gate (DSL v0.6); see [`QuestEffect::requires_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
        /// Per-effect negative flag gate (DSL v0.6); see
        /// [`QuestEffect::forbids_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        forbids_flags: Vec<FlagId>,
        /// What this beat does to the story (DSL v0.8, spec-0025; required at
        /// 0.8.0, `DW0481`; reserved `DW0141` earlier). Deliberately absent from
        /// the hand-written `Debug` rendering below: the declaration is
        /// validation metadata with no emission of its own, so a content key can
        /// never move because a beat gained a line of prose.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        happening: Option<Happening>,
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
        /// Effects fired once the NPC arrives at the destination cell (DSL v0.6,
        /// reserved `DW0141` earlier) — exact parity with [`QuestEffect::MoveActor`]
        /// `on_arrive`: same arrival detection (the walk driver's final tick), same
        /// execution context, and every deep effect walker recurses into it via
        /// [`QuestEffect::nested_effect_lists`]. This is what lets content gate a
        /// beat on walk *completion* instead of fire-and-forgetting the walk (e.g.
        /// `on_arrive: [set-flag]` so a cutscene waits for the NPC to reach its
        /// mark).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        on_arrive: Vec<QuestEffect>,
        /// Per-effect flag gate (DSL v0.6); see [`QuestEffect::requires_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
        /// Per-effect negative flag gate (DSL v0.6); see
        /// [`QuestEffect::forbids_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        forbids_flags: Vec<FlagId>,
        /// What this beat does to the story (DSL v0.8, spec-0025; required at
        /// 0.8.0, `DW0481`; reserved `DW0141` earlier). Deliberately absent from
        /// the hand-written `Debug` rendering below: the declaration is
        /// validation metadata with no emission of its own, so a content key can
        /// never move because a beat gained a line of prose.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        happening: Option<Happening>,
    },
    /// Plays a scripted camera cutscene (DSL v0.4 addendum). Per player: save
    /// gamemode+position, spectator, then dolly two co-located cameras along a
    /// straight-line lerp between waypoints and alternate `spectate` between them
    /// each tick (the two-camera bounce; the same-entity re-`spectate` is a server
    /// no-op and is never emitted), and restore on completion. The compiler
    /// validates the dolly path passes only through non-solid blocks — cameras
    /// fly but must not clip a solid (`DW0308`).
    ///
    /// Camera **aim** (DSL v0.6): with `look_at`, every dolly camera is rotated at
    /// emission to face that world point from its own position, so the shot keeps
    /// its subject framed for the whole move; without it, the camera faces along
    /// the direction of travel (the segment it is currently traversing).
    ///
    /// **Shape** — a cutscene is a list of [`CameraShot`]s played back-to-back
    /// inside one save/restore bracket (hard cut between shots). Two accepted,
    /// mutually exclusive spellings, both normalized by
    /// [`QuestEffect::cutscene_shots`]:
    /// - multi-shot (DSL v0.6): `{"shots": [{path, seconds, look_at?}, …]}`;
    /// - single-shot (DSL v0.4): `{"path": […], "seconds": n, "look_at"?: …}` —
    ///   exactly equivalent to a one-entry `shots` list.
    ///
    /// Mixing or omitting both is `DW0199`.
    Cutscene {
        /// Multi-shot form (DSL v0.6): the ordered shot list. Mutually exclusive
        /// with the single-shot `path`/`seconds` fields (`DW0199`).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        shots: Vec<CameraShot>,
        /// Single-shot form (DSL v0.4): ordered camera waypoints (straight-line
        /// lerp between them).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        path: Vec<CameraWaypoint>,
        /// Single-shot form (DSL v0.4): shot duration in seconds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        seconds: Option<u32>,
        /// Single-shot form (DSL v0.6): the subject the camera keeps framed.
        /// Absent = face along the direction of travel.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_at: Option<CameraTarget>,
        /// Per-effect flag gate (DSL v0.6); see [`QuestEffect::requires_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
        /// Per-effect negative flag gate (DSL v0.6); see
        /// [`QuestEffect::forbids_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        forbids_flags: Vec<FlagId>,
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
        /// Per-effect negative flag gate (DSL v0.6); see
        /// [`QuestEffect::forbids_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        forbids_flags: Vec<FlagId>,
    },
    /// Cuts the dimension-global weather to a new state (DSL v0.5, spec-0010).
    /// Instantaneous; persists because the weather cycle is frozen by sealing.
    SetWeather {
        /// The weather state to cut to.
        weather: WorldWeather,
        /// Per-effect flag gate (DSL v0.6); see [`QuestEffect::requires_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
        /// Per-effect negative flag gate (DSL v0.6); see
        /// [`QuestEffect::forbids_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        forbids_flags: Vec<FlagId>,
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
        /// Per-effect negative flag gate (DSL v0.6); see
        /// [`QuestEffect::forbids_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        forbids_flags: Vec<FlagId>,
    },
    /// Deals damage to the acting player(s) (DSL v0.6): the real consequence a
    /// stealth `on_caught` or a souls-style beat needs — vanilla's `/damage`
    /// primitive. Runs in the effect's `as @a` / `as @s` context, so `@s` is each
    /// acting player: at top level it damages every player once; inside a stealth
    /// `on_caught` it damages the caught player (the "caught → death → respawn at
    /// checkpoint" beat). `amount` is in **half-hearts** (1 HP each); an amount ≥ 40
    /// is lethal through golden apples / absorption. `within` (JSON `in`) narrows to
    /// acting players standing inside an anchor-centred box (the same box model as a
    /// stealth zone), keeping the per-`@s` semantics. `damage_type` is the damage
    /// type — a curated set of vanilla types that all respect `keepInventory` and do
    /// **not** bypass totems (no `out_of_world`/`generic_kill`); default `generic`.
    /// (The field is `damage_type`, not `type`, because the effect enum is
    /// internally tagged on `type`.)
    DamagePlayers {
        /// Damage dealt, in half-hearts (1 = 1 HP; ≥ 40 is effectively lethal).
        amount: u32,
        /// Optional spatial filter: only damage an acting player inside this
        /// anchor-centred box (`anchor ± extent`). Absent = every acting player.
        #[serde(default, rename = "in", skip_serializing_if = "Option::is_none")]
        within: Option<StealthZone>,
        /// The damage type (default [`DamageKind::Generic`]).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        damage_type: Option<DamageKind>,
        /// Per-effect flag gate (DSL v0.6); see [`QuestEffect::requires_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
        /// Per-effect negative flag gate (DSL v0.6); see
        /// [`QuestEffect::forbids_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        forbids_flags: Vec<FlagId>,
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
    /// Places a **bonfire** rest point (DSL v0.6, spec-0016 §1) — the sibling of
    /// [`QuestEffect::SetCheckpoint`] for souls-mode pacing. The effect *arms*
    /// the rest affordance (a `minecraft:interaction` the player right-clicks at
    /// the anchor, the campfire prop being prefab dressing); the checkpoint moves
    /// only **when the party actually rests**. Resting fires `on_rest` — the
    /// scene reset that makes retry cheap: re-arming traps, re-seating waves
    /// declared `respawns_on_rest`, restoring actor postures. Death respawns the
    /// party at the last-rested bonfire and runs the **same** `on_rest` bundle,
    /// so the world's answer to a death and to a rest is identical (spec-0016:
    /// death is an investment, never a tax).
    ///
    /// Proofs are inherited from the checkpoint machinery: the anchor must be
    /// standable (`DW0316`) and must not strand the party (`DW0315`), rooted at
    /// the beat that arms the bonfire (the earliest moment a rest can happen).
    Bonfire {
        /// The prefab anchor the rest affordance stands at, and the cell the
        /// party respawns at once rested.
        anchor: AnchorId,
        /// Effects re-run on every rest **and** on every respawn at this bonfire
        /// — the scene reset. Emitted in declared order and expected to be
        /// idempotent (the same contract as `set-checkpoint`'s `on_respawn`).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        on_rest: Vec<QuestEffect>,
        /// Title of the two-option rest dialog (DSL v0.8, owner ruling
        /// 2026-08-03). Absent = the compiler's canonical English `Bonfire`,
        /// baked at emit time (the `world.boundary.message` precedent): an
        /// authored line is inventoried and translates like every other
        /// player-visible string.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prompt: Option<String>,
        /// Label of the **rest and save** button. Absent = `Rest and save`.
        /// A dialog button is a fixed-width caption, so keep it to ~20 Latin /
        /// ~12 Han characters (skill *Writing craft* §C) — a wider label scrolls.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rest_label: Option<String>,
        /// Label of the **save only** button. Absent = `Save only`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        save_label: Option<String>,
    },
    /// Begins a stealth beat (DSL v0.6, spec-0014; owner ruling 2026-08-01:
    /// zone presence alone = hidden — no sneak requirement, which collided with
    /// the spectator cutscene camera). While active, every player must be
    /// inside some `zone` each tick; a player outside every zone for
    /// `grace_ticks` fires `on_caught` (typically a kill → checkpoint respawn).
    /// Zone membership is read from the player's position. The compiler proves
    /// each zone is standable and reachable from the activating beat (`DW0327`).
    BeginStealth {
        /// The "shadow" regions, each an anchor-centred box (see [`StealthZone`]).
        zones: Vec<StealthZone>,
        /// Per-player effects fired when a player is caught (out of every zone
        /// for `grace_ticks`). Empty = no consequence.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        on_caught: Vec<QuestEffect>,
        /// Ticks a player may be exposed before `on_caught` fires (default 20).
        #[serde(default = "default_grace_ticks")]
        grace_ticks: u32,
    },
    /// Ends the active stealth beat (DSL v0.6, spec-0014). No-op if none active.
    EndStealth,
    /// Summons a `deferred` stage-2 NPC (body + interaction hitbox + name display)
    /// at its declared anchor (DSL v0.6) — the dual of `despawn-npc`, and the
    /// scripted entrance a staged character needs. Idempotent: spawning an NPC
    /// already in the world is a no-op. Only meaningful for an NPC declared
    /// `deferred: true`; a non-deferred NPC is already in the world from init.
    SpawnNpc {
        /// The NPC (stage-2 ref) to summon.
        npc: NpcId,
        /// What this beat does to the story (DSL v0.8, spec-0025; required at
        /// 0.8.0, `DW0481`; reserved `DW0141` earlier). Deliberately absent from
        /// the hand-written `Debug` rendering below: the declaration is
        /// validation metadata with no emission of its own, so a content key can
        /// never move because a beat gained a line of prose.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        happening: Option<Happening>,
    },
    // --- DSL v0.6 actor staging effects (spec-0014) ---
    /// Summons a stage-5 actor's puppet at its anchor (DSL v0.6). Idempotent: a
    /// spawn of an already-present actor is a no-op (re-caging after `unleash`).
    SpawnActor {
        /// The actor (stage-5 `actors` ref) to summon.
        actor: ActorId,
        /// What this beat does to the story (DSL v0.8, spec-0025; required at
        /// 0.8.0, `DW0481`; reserved `DW0141` earlier). Deliberately absent from
        /// the hand-written `Debug` rendering below: the declaration is
        /// validation metadata with no emission of its own, so a content key can
        /// never move because a beat gained a line of prose.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        happening: Option<Happening>,
    },
    /// Removes an actor's puppet (DSL v0.6). `kill` plays the vanilla death
    /// animation (cutscene deaths); `vanish` is silent removal.
    DespawnActor {
        /// The actor (stage-5 `actors` ref) to remove.
        actor: ActorId,
        /// How the puppet is removed.
        style: DespawnStyle,
        /// What this beat does to the story (DSL v0.8, spec-0025; required at
        /// 0.8.0, `DW0481`; reserved `DW0141` earlier). Deliberately absent from
        /// the hand-written `Debug` rendering below: the declaration is
        /// validation metadata with no emission of its own, so a content key can
        /// never move because a beat gained a line of prose.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        happening: Option<Happening>,
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
        /// What this beat does to the story (DSL v0.8, spec-0025; required at
        /// 0.8.0, `DW0481`; reserved `DW0141` earlier). Deliberately absent from
        /// the hand-written `Debug` rendering below: the declaration is
        /// validation metadata with no emission of its own, so a content key can
        /// never move because a beat gained a line of prose.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        happening: Option<Happening>,
    },
    /// Replaces an actor's puppet with a real-AI twin of the same type / position /
    /// name / attributes / tag (DSL v0.6) — the "attack the idle giant → real
    /// fight" beat. Re-caging is `despawn-actor` + `spawn-actor` (idempotent), not a
    /// special verb.
    UnleashActor {
        /// The actor (stage-5 `actors` ref) to unleash.
        actor: ActorId,
        /// What this beat does to the story (DSL v0.8, spec-0025; required at
        /// 0.8.0, `DW0481`; reserved `DW0141` earlier). Deliberately absent from
        /// the hand-written `Debug` rendering below: the declaration is
        /// validation metadata with no emission of its own, so a content key can
        /// never move because a beat gained a line of prose.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        happening: Option<Happening>,
    },
    /// A deterministic timeline (DSL v0.6): one schedule chain firing effect groups
    /// at exact tick offsets. Effects are any in the stage-5 set except a nested
    /// `sequence` (rejected with `DW0329`).
    Sequence {
        /// Timeline steps; each fires its `effects` at `at_ticks` from the start.
        steps: Vec<SequenceStep>,
    },
    /// **Saturating projectile volley** (DSL v0.6, spec-0022): command-summoned
    /// projectiles with real velocity vectors, fired from a gallery slot into a
    /// declared kill zone.
    ///
    /// The contract is **saturation, not sniping** (owner ruling 2026-08-03).
    /// Every salvo puts one projectile on the trajectory to **every standable
    /// cell of `kill_zone`**, plus one aimed at the triggering player's
    /// fire-time position (which punishes standing still). A player therefore
    /// cannot dodge a volley by strafing — escaping means *leaving the zone*, a
    /// decision rather than a lucky step. Coverage is proven at compile time:
    /// `from_anchor` must have clear line of fire to every standable kill-zone
    /// cell (`DW0442`), so "the gallery slot can actually hit a player anywhere
    /// on the stairs" is a build-time fact, not a hope.
    ///
    /// Projectiles are summoned `NoGravity` so the flown path is exactly the
    /// straight segment the coverage proof checks — proof and runtime share one
    /// geometry. Drag scales speed but not direction, so the line is preserved.
    Volley {
        /// Projectile entity id (default `minecraft:arrow`; validated against
        /// the pinned 1.21.11 registry, `DW0441`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        projectile: Option<String>,
        /// The gallery slot the volley is fired FROM — a point anchor. Its cell
        /// must be clear (a projectile spawned inside a wall never leaves it).
        from_anchor: AnchorId,
        /// The zone the volley must blanket, as an anchor-centred box
        /// (`anchor ± extent`) — the same shape `damage-players`'s `in` and
        /// `begin-stealth`'s `zones` use. Every standable cell in it receives
        /// fire each salvo.
        ///
        /// Deliberately NOT a bare prefab `region` anchor: the assembled model
        /// clears every gate-region anchor's cells unconditionally, so
        /// describing a zone that way would delete the geometry it names.
        kill_zone: StealthZone,
        /// How many rounds the pattern repeats (default 3, `DW0443` bounds it).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        salvos: Option<u32>,
        /// Ticks between salvos (default 10, `DW0443` bounds it).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        interval: Option<u32>,
        /// Per-effect flag gate (DSL v0.6); see [`QuestEffect::requires_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
        /// Per-effect negative flag gate (DSL v0.6); see
        /// [`QuestEffect::forbids_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        forbids_flags: Vec<FlagId>,
    },
    /// **Ceiling collapse** (DSL v0.6, spec-0022): delete a region's blocks and
    /// drop them as `falling_block` entities — the buried-alive trap redstone
    /// cannot express at all.
    ///
    /// The post-collapse world is **modeled, not guessed**: the compiler clears
    /// the region, settles every dropped column through the existing gravity
    /// model (spec-0010), optionally paves the landing surface with
    /// `then_floor`, and re-runs the critical-path completability proof against
    /// that mutated world (`DW0445`). A collapse that buries the only route is a
    /// build error, exactly as a `shortcut` seal that strands the party is.
    Collapse {
        /// The volume whose blocks fall, as an anchor-centred box
        /// (`anchor ± extent`). Must hold blocks and sit above standable footing
        /// (`DW0444`) — a collapse over nothing is scenery, not a trap.
        ///
        /// An anchor-centred box rather than a prefab `region` anchor for a
        /// load-bearing reason: the assembled model deletes every gate-region
        /// anchor's cells, so a ceiling slab declared that way would already be
        /// gone before the trap ever fired.
        region_anchor: StealthZone,
        /// The block the falling entities are made of (default
        /// `minecraft:gravel`; validated against the pinned registry, `DW0441`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        falling_block: Option<String>,
        /// Optional block the landing surface is paved with once the debris
        /// settles — the authored post-collapse floor the completability proof
        /// reasons over.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        then_floor: Option<String>,
        /// Per-effect flag gate (DSL v0.6); see [`QuestEffect::requires_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
        /// Per-effect negative flag gate (DSL v0.6); see
        /// [`QuestEffect::forbids_flags`].
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        forbids_flags: Vec<FlagId>,
    },
}

/// `Debug` is hand-written because it is a **stable content-key rendering**: the
/// compiler's `sequence_key` (FNV over `{steps:?}`, where a step's `effects` are
/// `QuestEffect`s) names generated `seq_<hash>` functions from it, so an effect
/// that uses none of the v0.6 additions (`forbids_flags` anywhere, `on_arrive`
/// on `move-npc`) must render byte-identically to the pre-addition derive —
/// otherwise every existing sequence would silently churn its function names on
/// a purely additive schema change (the [`CameraShot`] precedent). Rules:
/// every pre-existing field prints exactly as `#[derive(Debug)]` printed it (in
/// declaration order); `forbids_flags` prints only when non-empty; `move-npc`'s
/// `on_arrive` prints only when non-empty.
impl std::fmt::Debug for QuestEffect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        /// Append `forbids_flags` only when non-empty (see the impl doc).
        fn ff<'c, 'a, 'b: 'a>(
            d: &'c mut std::fmt::DebugStruct<'a, 'b>,
            forbids_flags: &[FlagId],
        ) -> &'c mut std::fmt::DebugStruct<'a, 'b> {
            if forbids_flags.is_empty() {
                d
            } else {
                d.field("forbids_flags", &forbids_flags)
            }
        }
        match self {
            QuestEffect::OpenGate {
                anchor,
                requires_flags,
                forbids_flags,
                ..
            } => ff(
                f.debug_struct("OpenGate")
                    .field("anchor", anchor)
                    .field("requires_flags", requires_flags),
                forbids_flags,
            )
            .finish(),
            QuestEffect::CloseGate {
                anchor,
                requires_flags,
                forbids_flags,
                sealed_hint,
                ..
            } => {
                let mut s = f.debug_struct("CloseGate");
                let d = ff(
                    s.field("anchor", anchor)
                        .field("requires_flags", requires_flags),
                    forbids_flags,
                );
                // Prints only when authored (the additive-field rule): a campaign
                // that takes the compiler's canonical seal line renders exactly as
                // the pre-0.8 derive did, so no existing `seq_<hash>` moves.
                match sealed_hint {
                    Some(h) => d.field("sealed_hint", h).finish(),
                    None => d.finish(),
                }
            }
            QuestEffect::CampaignComplete { .. } => f.write_str("CampaignComplete"),
            QuestEffect::GiveItem {
                item,
                count,
                name,
                carrier,
                requires_flags,
                forbids_flags,
            } => ff(
                f.debug_struct("GiveItem")
                    .field("item", item)
                    .field("count", count)
                    .field("name", name)
                    .field("carrier", carrier)
                    .field("requires_flags", requires_flags),
                forbids_flags,
            )
            .finish(),
            QuestEffect::SetFlag {
                flag,
                requires_flags,
                forbids_flags,
            } => ff(
                f.debug_struct("SetFlag")
                    .field("flag", flag)
                    .field("requires_flags", requires_flags),
                forbids_flags,
            )
            .finish(),
            QuestEffect::SpawnWave {
                wave,
                requires_flags,
                forbids_flags,
                ..
            } => ff(
                f.debug_struct("SpawnWave")
                    .field("wave", wave)
                    .field("requires_flags", requires_flags),
                forbids_flags,
            )
            .finish(),
            QuestEffect::Narrate {
                text,
                style,
                sound,
                requires_flags,
                forbids_flags,
            } => ff(
                f.debug_struct("Narrate")
                    .field("text", text)
                    .field("style", style)
                    .field("sound", sound)
                    .field("requires_flags", requires_flags),
                forbids_flags,
            )
            .finish(),
            QuestEffect::SetBlock {
                anchor,
                block,
                requires_flags,
                forbids_flags,
            } => ff(
                f.debug_struct("SetBlock")
                    .field("anchor", anchor)
                    .field("block", block)
                    .field("requires_flags", requires_flags),
                forbids_flags,
            )
            .finish(),
            QuestEffect::DespawnNpc {
                npc,
                requires_flags,
                forbids_flags,
                ..
            } => ff(
                f.debug_struct("DespawnNpc")
                    .field("npc", npc)
                    .field("requires_flags", requires_flags),
                forbids_flags,
            )
            .finish(),
            QuestEffect::MoveNpc {
                npc,
                to_anchor,
                speed,
                on_arrive,
                requires_flags,
                forbids_flags,
                ..
            } => {
                let mut d = f.debug_struct("MoveNpc");
                d.field("npc", npc)
                    .field("to_anchor", to_anchor)
                    .field("speed", speed);
                if !on_arrive.is_empty() {
                    d.field("on_arrive", on_arrive);
                }
                d.field("requires_flags", requires_flags);
                ff(&mut d, forbids_flags).finish()
            }
            QuestEffect::Cutscene {
                shots,
                path,
                seconds,
                look_at,
                requires_flags,
                forbids_flags,
            } => ff(
                f.debug_struct("Cutscene")
                    .field("shots", shots)
                    .field("path", path)
                    .field("seconds", seconds)
                    .field("look_at", look_at)
                    .field("requires_flags", requires_flags),
                forbids_flags,
            )
            .finish(),
            QuestEffect::SetTime {
                time,
                requires_flags,
                forbids_flags,
            } => ff(
                f.debug_struct("SetTime")
                    .field("time", time)
                    .field("requires_flags", requires_flags),
                forbids_flags,
            )
            .finish(),
            QuestEffect::SetWeather {
                weather,
                requires_flags,
                forbids_flags,
            } => ff(
                f.debug_struct("SetWeather")
                    .field("weather", weather)
                    .field("requires_flags", requires_flags),
                forbids_flags,
            )
            .finish(),
            QuestEffect::PlaySound {
                sound,
                at,
                volume,
                pitch,
                requires_flags,
                forbids_flags,
            } => ff(
                f.debug_struct("PlaySound")
                    .field("sound", sound)
                    .field("at", at)
                    .field("volume", volume)
                    .field("pitch", pitch)
                    .field("requires_flags", requires_flags),
                forbids_flags,
            )
            .finish(),
            QuestEffect::DamagePlayers {
                amount,
                within,
                damage_type,
                requires_flags,
                forbids_flags,
            } => ff(
                f.debug_struct("DamagePlayers")
                    .field("amount", amount)
                    .field("within", within)
                    .field("damage_type", damage_type)
                    .field("requires_flags", requires_flags),
                forbids_flags,
            )
            .finish(),
            QuestEffect::SetCheckpoint { anchor, on_respawn } => f
                .debug_struct("SetCheckpoint")
                .field("anchor", anchor)
                .field("on_respawn", on_respawn)
                .finish(),
            QuestEffect::Bonfire {
                anchor,
                on_rest,
                prompt,
                rest_label,
                save_label,
            } => f
                .debug_struct("Bonfire")
                .field("anchor", anchor)
                .field("on_rest", on_rest)
                .field("prompt", prompt)
                .field("rest_label", rest_label)
                .field("save_label", save_label)
                .finish(),
            QuestEffect::BeginStealth {
                zones,
                on_caught,
                grace_ticks,
            } => f
                .debug_struct("BeginStealth")
                .field("zones", zones)
                .field("on_caught", on_caught)
                .field("grace_ticks", grace_ticks)
                .finish(),
            QuestEffect::EndStealth => f.write_str("EndStealth"),
            QuestEffect::SpawnNpc { npc, .. } => {
                f.debug_struct("SpawnNpc").field("npc", npc).finish()
            }
            QuestEffect::SpawnActor { actor, .. } => {
                f.debug_struct("SpawnActor").field("actor", actor).finish()
            }
            QuestEffect::DespawnActor { actor, style, .. } => f
                .debug_struct("DespawnActor")
                .field("actor", actor)
                .field("style", style)
                .finish(),
            QuestEffect::MoveActor {
                actor,
                to_anchor,
                speed,
                on_arrive,
                ..
            } => f
                .debug_struct("MoveActor")
                .field("actor", actor)
                .field("to_anchor", to_anchor)
                .field("speed", speed)
                .field("on_arrive", on_arrive)
                .finish(),
            QuestEffect::UnleashActor { actor, .. } => f
                .debug_struct("UnleashActor")
                .field("actor", actor)
                .finish(),
            QuestEffect::Sequence { steps } => {
                f.debug_struct("Sequence").field("steps", steps).finish()
            }
            // spec-0022 additions. These are NEW variants, so there is no
            // pre-addition rendering to preserve — every field prints
            // unconditionally, which is the stable choice going forward.
            QuestEffect::Volley {
                projectile,
                from_anchor,
                kill_zone,
                salvos,
                interval,
                requires_flags,
                forbids_flags,
            } => f
                .debug_struct("Volley")
                .field("projectile", projectile)
                .field("from_anchor", from_anchor)
                .field("kill_zone", kill_zone)
                .field("salvos", salvos)
                .field("interval", interval)
                .field("requires_flags", requires_flags)
                .field("forbids_flags", forbids_flags)
                .finish(),
            QuestEffect::Collapse {
                region_anchor,
                falling_block,
                then_floor,
                requires_flags,
                forbids_flags,
            } => f
                .debug_struct("Collapse")
                .field("region_anchor", region_anchor)
                .field("falling_block", falling_block)
                .field("then_floor", then_floor)
                .field("requires_flags", requires_flags)
                .field("forbids_flags", forbids_flags)
                .finish(),
        }
    }
}

/// Default `grace_ticks` for [`QuestEffect::BeginStealth`] (spec-0014).
fn default_grace_ticks() -> u32 {
    20
}

/// Default projectile for [`QuestEffect::Volley`] (spec-0022).
pub const DEFAULT_VOLLEY_PROJECTILE: &str = "minecraft:arrow";
/// Default salvo count for [`QuestEffect::Volley`] (spec-0022).
pub const DEFAULT_VOLLEY_SALVOS: u32 = 3;
/// Default ticks between salvos for [`QuestEffect::Volley`] (spec-0022).
pub const DEFAULT_VOLLEY_INTERVAL: u32 = 10;
/// Largest admissible `salvos` — beyond this a volley is an entity-count
/// hazard rather than a trap (`DW0443`).
pub const MAX_VOLLEY_SALVOS: u32 = 16;
/// Largest admissible `interval` in ticks (`DW0443`): 10 seconds. A volley
/// slower than this is no longer one event the player reads as a trap.
pub const MAX_VOLLEY_INTERVAL: u32 = 200;
/// Default falling block for [`QuestEffect::Collapse`] (spec-0022).
pub const DEFAULT_COLLAPSE_FALLING_BLOCK: &str = "minecraft:gravel";

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

/// The damage type of a [`QuestEffect::DamagePlayers`] effect (DSL v0.6). A
/// **curated** subset of the vanilla 1.21.11 damage-type registry: every variant
/// respects the `keepInventory` death flow (a gamerule, so all deaths do) and does
/// **not** bypass a totem of undying — the totem-bypassing `out_of_world` /
/// `generic_kill` types are deliberately excluded, so a scripted consequence can
/// never silently void a player's held totem. Modelled as an enum (not a free
/// string) so an unknown type is a schema rejection (`DW0100`) and needs no separate
/// registry / diagnostic. `generic` is the default: command damage that respects
/// totems + absorption but ignores armor, so a scripted hit lands regardless of gear.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DamageKind {
    /// `minecraft:generic` — armor-ignoring command damage (default).
    Generic,
    /// `minecraft:magic` — magical damage.
    Magic,
    /// `minecraft:wither` — the wither/withering effect's damage type.
    Wither,
    /// `minecraft:on_fire` — burning damage.
    Fire,
    /// `minecraft:drown` — drowning damage.
    Drown,
    /// `minecraft:freeze` — powder-snow freezing damage.
    Freeze,
    /// `minecraft:fall` — fall damage.
    Fall,
    /// `minecraft:lightning_bolt` — a lightning strike's damage type.
    LightningBolt,
    /// `minecraft:explosion` — a (non-player) explosion's damage type.
    Explosion,
}

impl DamageKind {
    /// The vanilla `minecraft:` damage-type id emitted to `/damage`.
    pub fn id(self) -> &'static str {
        match self {
            DamageKind::Generic => "minecraft:generic",
            DamageKind::Magic => "minecraft:magic",
            DamageKind::Wither => "minecraft:wither",
            DamageKind::Fire => "minecraft:on_fire",
            DamageKind::Drown => "minecraft:drown",
            DamageKind::Freeze => "minecraft:freeze",
            DamageKind::Fall => "minecraft:fall",
            DamageKind::LightningBolt => "minecraft:lightning_bolt",
            DamageKind::Explosion => "minecraft:explosion",
        }
    }
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
    /// An "art" title rendered through the delve's custom resource-pack pixel-banner
    /// font (`delve:art`) so endings can flash blocky all-caps text (DSL v0.6,
    /// spec-0014). Text is checked at compile time against the font's glyph inventory
    /// (`DW0328`); characters outside it (e.g. non-Latin script) are rejected. It
    /// renders in the vanilla title slot, so it is width-checked like any title
    /// (`DW0330`) — roughly 15 glyphs fit on screen.
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

/// One shot of a [`QuestEffect::Cutscene`] (DSL v0.6): a camera dolly with its
/// own duration and optional subject. A cutscene plays its shots back-to-back —
/// a hard cut between them — inside a single gamemode/position save-restore
/// bracket, so a wide establishing move can be followed by an interior close-up
/// without the players ever leaving the cinematic.
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CameraShot {
    /// Ordered camera waypoints (straight-line lerp between them). A one-waypoint
    /// path is a static shot. Required without `shot_style`; with one, optional —
    /// an explicit `path` always overrides the style's expanded dolly.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<CameraWaypoint>,
    /// This shot's duration in seconds. Required without `shot_style`; with one,
    /// optional — the style's default duration applies (see
    /// [`ShotStyle::default_seconds`]), and an explicit value always overrides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seconds: Option<u32>,
    /// Optional subject the camera keeps framed for this shot. Absent = face
    /// along the direction of travel (or, under a `shot_style`, the style's own
    /// aim at its `subject`). An explicit `look_at` always overrides a style's
    /// aim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look_at: Option<CameraTarget>,
    /// Shot-style preset (DSL v0.6, spec-0015 shot-grammar library): the
    /// compiler expands the style deterministically into a camera dolly +
    /// per-keyframe aim from the `subject`'s resolved geometry. Requires
    /// `subject`; `path`/`look_at`/`seconds` remain legal and always override
    /// the corresponding expanded part (`DW0348` polices the combinations).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shot_style: Option<ShotStyle>,
    /// The styled shot's subject — what the shot is *about*. Required with
    /// `shot_style`, rejected without one (`DW0348`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<CameraSubject>,
    /// `two-shot` only: the second framed subject (`DW0348` elsewhere).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_b: Option<CameraSubject>,
    /// Styled shots only: the style's characteristic camera distance in blocks
    /// (its *start* distance for the dolly styles). Default per style — see the
    /// `shot_style` table in `docs/reference/compiler.md`. Clamped range 1..=48
    /// (`DW0348`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dist: Option<f64>,
    /// `orbit-arc` only: the sweep in degrees, 45..=120 (dossier range),
    /// default 90 (`DW0348` elsewhere or out of range).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degrees: Option<f64>,
    /// Styled shots only: placement bearing in degrees — where the camera sits
    /// (or starts) relative to the subject, measured like a Minecraft yaw
    /// *from* the subject: `0` puts the camera south of the subject (+Z), `90`
    /// west (−X), `-90` east (+X), `180` north (−Z). Default `0`. For
    /// `side-track` the bearing picks which side of the subject's travel the
    /// camera runs abeam (`0` = right of travel, `180` = left).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bearing: Option<f64>,
}

/// A `shot_style` preset (DSL v0.6): the dossier's 9-template library
/// (`docs/notes/camera-dossier.md` §2), each expanded deterministically by the
/// compiler into a dolly + aim from the subject's geometry. Camera "lens feel"
/// is **distance only** — vanilla has no in-game FOV control.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ShotStyle {
    /// Fully static close framing of a prop/detail. The beat that always looks
    /// right.
    Insert,
    /// Static position; only the aim turns as the subject (ideally moving)
    /// passes. Rockstar's roadside "ground view".
    LockedOff,
    /// Straight dolly toward the subject along the view axis; medium → close.
    PushIn,
    /// Reverse of `push-in`: close → wide, revealing context.
    PullBackReveal,
    /// High and far, descending and closing on the subject. First sight of an
    /// area.
    EstablishingCrane,
    /// Constant-radius, constant-height arc around the subject.
    OrbitArc,
    /// Parallel dolly abeam a **moving** subject at constant offset — needs a
    /// compiler-known subject path (`DW0349`).
    SideTrack,
    /// Static placement solved so **both** subjects land on opposite thirds
    /// (Toric-space construction). Needs `subject_b`.
    TwoShot,
    /// Low, close, trailing a **moving** subject near ground level — needs a
    /// compiler-known subject path (`DW0349`).
    LowFollow,
}

impl ShotStyle {
    /// The kebab token (diagnostics, digests).
    pub fn token(self) -> &'static str {
        match self {
            ShotStyle::Insert => "insert",
            ShotStyle::LockedOff => "locked-off",
            ShotStyle::PushIn => "push-in",
            ShotStyle::PullBackReveal => "pull-back-reveal",
            ShotStyle::EstablishingCrane => "establishing-crane",
            ShotStyle::OrbitArc => "orbit-arc",
            ShotStyle::SideTrack => "side-track",
            ShotStyle::TwoShot => "two-shot",
            ShotStyle::LowFollow => "low-follow",
        }
    }

    /// Default shot duration in seconds, from the dossier's per-style duration
    /// ranges (§2, anchored on the film-editing ASL literature) — the value an
    /// omitted `seconds` resolves to.
    pub fn default_seconds(self) -> u32 {
        match self {
            ShotStyle::Insert => 2,
            ShotStyle::LockedOff => 6,
            ShotStyle::PushIn => 4,
            ShotStyle::PullBackReveal => 6,
            ShotStyle::EstablishingCrane => 8,
            ShotStyle::OrbitArc => 8,
            ShotStyle::SideTrack => 8,
            ShotStyle::TwoShot => 5,
            ShotStyle::LowFollow => 5,
        }
    }

    /// `true` for the styles whose subject must be *moving* on a compiler-known
    /// path (`move-npc` / `move-actor` in the same effect group or sequence) —
    /// `side-track` and `low-follow` (`DW0349` otherwise).
    pub fn needs_moving_subject(self) -> bool {
        matches!(self, ShotStyle::SideTrack | ShotStyle::LowFollow)
    }
}

/// A styled shot's subject (DSL v0.6): the world thing the shot frames — a
/// prefab anchor point, a stage-2 NPC, or a stage-5 actor — plus an integer
/// block offset. For `npc`/`actor` subjects the aim point is the entity's cell
/// **plus one block up** (torso height, so a close shot does not frame feet)
/// before `offset` is applied; an `anchor` subject aims at the block centre
/// exactly like a [`CameraTarget`].
/// Each variant's payload is a **named struct** carrying `deny_unknown_fields`
/// (task #78). Serde has no variant-level `deny_unknown_fields`, so an untagged
/// enum with inline struct variants silently *ignores* any key it does not
/// recognise: `{"npc": …, "ofset": [0,1,0]}` deserialized happily with the offset
/// dropped, and `{"anchor": …, "npc": …}` quietly matched `Anchor` and discarded
/// the NPC. Lifting each variant into its own type restores the repo-wide
/// deny-unknown rule for both serde and the published JSON Schema
/// (`additionalProperties: false`): a mistyped shot subject now fails the schema
/// instead of rendering a shot pointed somewhere the author never asked for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum CameraSubject {
    /// A fixed world point: prefab anchor + offset.
    Anchor(AnchorSubject),
    /// A stage-2 NPC — moving if a `move-npc` for it runs in the same effect
    /// group / sequence, else static at its declared (or spawn) anchor.
    Npc(NpcSubject),
    /// A stage-5 actor — moving if a `move-actor` for it runs in the same
    /// effect group / sequence, else static at its declared anchor.
    Actor(ActorSubject),
}

/// A [`CameraSubject::Anchor`] payload: a fixed world point.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnchorSubject {
    /// The anchor the subject sits at.
    pub anchor: AnchorId,
    /// Integer `[x, y, z]` block offset (default `[0, 0, 0]`).
    #[serde(default, skip_serializing_if = "is_zero3")]
    pub offset: [i32; 3],
}

/// A [`CameraSubject::Npc`] payload: a stage-2 NPC.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NpcSubject {
    /// The NPC (stage-2 ref).
    pub npc: NpcId,
    /// Integer `[x, y, z]` block offset (default `[0, 0, 0]`).
    #[serde(default, skip_serializing_if = "is_zero3")]
    pub offset: [i32; 3],
}

/// A [`CameraSubject::Actor`] payload: a stage-5 actor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActorSubject {
    /// The actor (stage-5 `actors` ref).
    pub actor: ActorId,
    /// Integer `[x, y, z]` block offset (default `[0, 0, 0]`).
    #[serde(default, skip_serializing_if = "is_zero3")]
    pub offset: [i32; 3],
}

impl CameraSubject {
    /// The subject's integer offset, whichever variant.
    pub fn offset(&self) -> [i32; 3] {
        match self {
            CameraSubject::Anchor(s) => s.offset,
            CameraSubject::Npc(s) => s.offset,
            CameraSubject::Actor(s) => s.offset,
        }
    }

    /// A short canonical rendering for digests/diagnostics.
    pub fn canon(&self) -> String {
        let (kind, id, o) = match self {
            CameraSubject::Anchor(s) => ("a", s.anchor.as_str(), &s.offset),
            CameraSubject::Npc(s) => ("n", s.npc.as_str(), &s.offset),
            CameraSubject::Actor(s) => ("c", s.actor.as_str(), &s.offset),
        };
        format!("{kind}:{id}@{},{},{}", o[0], o[1], o[2])
    }
}

/// `Debug` is hand-written because it is a **stable content-key rendering**:
/// the compiler's `sequence_key` (FNV over `{steps:?}`) names generated
/// `seq_<hash>` functions from it, so a shot that uses none of the v0.6 style
/// fields must render byte-identically to the pre-style struct (`seconds`
/// prints its inner value; absent style fields print nothing) — otherwise
/// every existing sequence containing a cutscene would silently churn its
/// function names on a purely additive schema change.
impl std::fmt::Debug for CameraShot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("CameraShot");
        d.field("path", &self.path);
        match self.seconds {
            Some(v) => d.field("seconds", &v),
            None => d.field("seconds", &self.seconds),
        };
        d.field("look_at", &self.look_at);
        if self.shot_style.is_some() {
            d.field("shot_style", &self.shot_style);
        }
        if self.subject.is_some() {
            d.field("subject", &self.subject);
        }
        if self.subject_b.is_some() {
            d.field("subject_b", &self.subject_b);
        }
        if self.dist.is_some() {
            d.field("dist", &self.dist);
        }
        if self.degrees.is_some() {
            d.field("degrees", &self.degrees);
        }
        if self.bearing.is_some() {
            d.field("bearing", &self.bearing);
        }
        d.finish()
    }
}

impl CameraShot {
    /// The shot's resolved duration in seconds: explicit `seconds`, else the
    /// style default, else `1` (a shape-invalid shot — `DW0199` reports it; the
    /// fallback only keeps downstream passes total).
    pub fn resolved_seconds(&self) -> u32 {
        self.seconds
            .or(self.shot_style.map(ShotStyle::default_seconds))
            .unwrap_or(1)
    }
}

/// The subject a [`QuestEffect::Cutscene`] camera keeps framed (DSL v0.6): an
/// anchor plus an integer block offset from it, giving the world point every
/// dolly camera is aimed at. Same shape as a [`CameraWaypoint`] — a waypoint says
/// where the camera *is*, a target says what it *looks at*.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CameraTarget {
    /// The anchor the look target is relative to.
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
    /// The kebab-case `type` tag this effect serializes as — the verb a
    /// diagnostic should name so the author can find it in the JSON.
    pub fn verb(&self) -> &'static str {
        match self {
            QuestEffect::OpenGate { .. } => "open-gate",
            QuestEffect::CloseGate { .. } => "close-gate",
            QuestEffect::CampaignComplete { .. } => "campaign-complete",
            QuestEffect::GiveItem { .. } => "give-item",
            QuestEffect::SetFlag { .. } => "set-flag",
            QuestEffect::SpawnWave { .. } => "spawn-wave",
            QuestEffect::Narrate { .. } => "narrate",
            QuestEffect::SetBlock { .. } => "set-block",
            QuestEffect::DespawnNpc { .. } => "despawn-npc",
            QuestEffect::MoveNpc { .. } => "move-npc",
            QuestEffect::Cutscene { .. } => "cutscene",
            QuestEffect::SetTime { .. } => "set-time",
            QuestEffect::SetWeather { .. } => "set-weather",
            QuestEffect::PlaySound { .. } => "play-sound",
            QuestEffect::DamagePlayers { .. } => "damage-players",
            QuestEffect::SetCheckpoint { .. } => "set-checkpoint",
            QuestEffect::Bonfire { .. } => "bonfire",
            QuestEffect::BeginStealth { .. } => "begin-stealth",
            QuestEffect::EndStealth => "end-stealth",
            QuestEffect::SpawnNpc { .. } => "spawn-npc",
            QuestEffect::SpawnActor { .. } => "spawn-actor",
            QuestEffect::DespawnActor { .. } => "despawn-actor",
            QuestEffect::MoveActor { .. } => "move-actor",
            QuestEffect::UnleashActor { .. } => "unleash-actor",
            QuestEffect::Sequence { .. } => "sequence",
            QuestEffect::Volley { .. } => "volley",
            QuestEffect::Collapse { .. } => "collapse",
        }
    }

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

    /// The authored `sealed_hint` if this is a `close-gate` that declares one (DSL
    /// v0.8). `None` for every other effect **and** for a `close-gate` that takes
    /// the compiler's canonical English seal line.
    pub fn close_gate_sealed_hint(&self) -> Option<&str> {
        match self {
            QuestEffect::CloseGate { sealed_hint, .. } => sealed_hint.as_deref(),
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

    /// The declared `carrier` if this is a `give-item` that states one (v0.6,
    /// spec-0018). `None` for any other effect **and** for a `give-item` that
    /// leaves it absent — absent reads as [`Carrier::All`], and the distinction
    /// matters only for the pre-0.6 reserved-field gate.
    pub fn give_carrier(&self) -> Option<Carrier> {
        match self {
            QuestEffect::GiveItem { carrier, .. } => *carrier,
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
            | QuestEffect::CampaignComplete { .. } => None,
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
            | QuestEffect::DamagePlayers { .. }
            | QuestEffect::SetCheckpoint { .. }
            | QuestEffect::Bonfire { .. }
            | QuestEffect::BeginStealth { .. }
            | QuestEffect::EndStealth
            | QuestEffect::SpawnActor { .. }
            | QuestEffect::DespawnActor { .. }
            | QuestEffect::MoveActor { .. }
            | QuestEffect::UnleashActor { .. }
            | QuestEffect::SpawnNpc { .. }
            | QuestEffect::Sequence { .. }
            // spec-0022 trap-payload verbs are v0.6 — they report via `v06_effect`.
            | QuestEffect::Volley { .. }
            | QuestEffect::Collapse { .. } => None,
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
            QuestEffect::Bonfire { .. } => Some("bonfire"),
            QuestEffect::BeginStealth { .. } => Some("begin-stealth"),
            QuestEffect::EndStealth => Some("end-stealth"),
            QuestEffect::PlaySound { .. } => Some("play-sound"),
            QuestEffect::DamagePlayers { .. } => Some("damage-players"),
            QuestEffect::SpawnActor { .. } => Some("spawn-actor"),
            QuestEffect::DespawnActor { .. } => Some("despawn-actor"),
            QuestEffect::MoveActor { .. } => Some("move-actor"),
            QuestEffect::UnleashActor { .. } => Some("unleash-actor"),
            QuestEffect::Sequence { .. } => Some("sequence"),
            QuestEffect::SpawnNpc { .. } => Some("spawn-npc"),
            // spec-0022 trap-payload verbs — v0.6 surface, reserved earlier.
            QuestEffect::Volley { .. } => Some("volley"),
            QuestEffect::Collapse { .. } => Some("collapse"),
            _ => None,
        }
    }

    /// `(projectile, from_anchor, kill_zone, salvos, interval)` if this is a
    /// `volley` (spec-0022), with the documented defaults already applied.
    pub fn volley(&self) -> Option<(&str, &AnchorId, &StealthZone, u32, u32)> {
        match self {
            QuestEffect::Volley {
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
        match self {
            QuestEffect::Collapse {
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
        match self {
            QuestEffect::SpawnNpc { npc, .. } => Some(npc),
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

    /// `(anchor, on_rest)` if this is a `bonfire` effect (spec-0016 §1).
    pub fn bonfire(&self) -> Option<(&AnchorId, &[QuestEffect])> {
        match self {
            QuestEffect::Bonfire {
                anchor, on_rest, ..
            } => Some((anchor, on_rest.as_slice())),
            _ => None,
        }
    }

    /// The bonfire's authored rest-dialog strings, each `None` when unauthored
    /// (the compiler then bakes its canonical English). `None` for every other
    /// effect. spec-0016 §1, owner ruling 2026-08-03.
    pub fn bonfire_labels(&self) -> Option<BonfireLabels<'_>> {
        match self {
            QuestEffect::Bonfire {
                prompt,
                rest_label,
                save_label,
                ..
            } => Some(BonfireLabels {
                prompt: prompt.as_deref(),
                rest_label: rest_label.as_deref(),
                save_label: save_label.as_deref(),
            }),
            _ => None,
        }
    }

    /// The `within` filter zone if this is a v0.6 `damage-players` effect that
    /// declares one (the `in` spatial scope). `None` for an unscoped
    /// `damage-players` and for every other effect.
    pub fn damage_within(&self) -> Option<&StealthZone> {
        match self {
            QuestEffect::DamagePlayers { within, .. } => within.as_ref(),
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
        match self {
            QuestEffect::Sequence { steps } => steps.iter().map(|s| s.effects.as_slice()).collect(),
            QuestEffect::SetCheckpoint { on_respawn, .. } => vec![on_respawn.as_slice()],
            QuestEffect::Bonfire { on_rest, .. } => vec![on_rest.as_slice()],
            QuestEffect::BeginStealth { on_caught, .. } => vec![on_caught.as_slice()],
            QuestEffect::MoveActor { on_arrive, .. } | QuestEffect::MoveNpc { on_arrive, .. } => {
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
        match self {
            QuestEffect::Sequence { steps } => steps
                .iter_mut()
                .enumerate()
                .map(|(s, st)| (format!("seq.{s}"), st.effects.as_mut_slice()))
                .collect(),
            QuestEffect::SetCheckpoint { on_respawn, .. } => {
                vec![("respawn".to_string(), on_respawn.as_mut_slice())]
            }
            QuestEffect::Bonfire { on_rest, .. } => {
                vec![("rest".to_string(), on_rest.as_mut_slice())]
            }
            QuestEffect::BeginStealth { on_caught, .. } => {
                vec![("caught".to_string(), on_caught.as_mut_slice())]
            }
            QuestEffect::MoveActor { on_arrive, .. } | QuestEffect::MoveNpc { on_arrive, .. } => {
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
        match self {
            QuestEffect::Sequence { steps } => steps
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
            QuestEffect::SetCheckpoint { on_respawn, .. } => vec![(
                "on_respawn".to_string(),
                "respawn".to_string(),
                on_respawn.as_slice(),
            )],
            QuestEffect::Bonfire { on_rest, .. } => vec![(
                "on_rest".to_string(),
                "rest".to_string(),
                on_rest.as_slice(),
            )],
            QuestEffect::BeginStealth { on_caught, .. } => vec![(
                "on_caught".to_string(),
                "caught".to_string(),
                on_caught.as_slice(),
            )],
            QuestEffect::MoveActor { on_arrive, .. } | QuestEffect::MoveNpc { on_arrive, .. } => {
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
    /// (`anchor`, `to_anchor`, `in/anchor`, `zones/<i>/anchor`, `at/anchor`,
    /// `shots/<i>/path/<j>/anchor`, …).
    ///
    /// Not recursive: pair it with [`Self::visit_deep`] to sweep a whole effect
    /// tree. Every consumer that must resolve an anchor — the DSL's `DW0142`
    /// reference scan, the compiler's build-time resolution seal (`DW0355`) —
    /// goes through here, so a new anchor-bearing variant (or a new anchor field
    /// on an existing one) is picked up by all of them at once. That closes the
    /// silent-drop class of bug: an anchor-bearing effect whose anchor is typo'd
    /// used to emit *nothing* (the emitter's `for … if name == anchor` loop simply
    /// found no match) while every shallow validator looked straight past it.
    pub fn anchor_refs(&self) -> Vec<(String, &AnchorId)> {
        /// `(suffix, anchor)` for a shot's own anchor-bearing fields, under `base`.
        fn shot_refs<'a>(base: &str, shot: &'a CameraShot) -> Vec<(String, &'a AnchorId)> {
            let mut out: Vec<(String, &AnchorId)> = shot
                .path
                .iter()
                .enumerate()
                .map(|(j, w)| (format!("{base}path/{j}/anchor"), &w.anchor))
                .collect();
            if let Some(t) = &shot.look_at {
                out.push((format!("{base}look_at/anchor"), &t.anchor));
            }
            for (field, subject) in [("subject", &shot.subject), ("subject_b", &shot.subject_b)] {
                if let Some(CameraSubject::Anchor(s)) = subject {
                    out.push((format!("{base}{field}/anchor"), &s.anchor));
                }
            }
            out
        }
        match self {
            QuestEffect::OpenGate { anchor, .. }
            | QuestEffect::CloseGate { anchor, .. }
            | QuestEffect::SetBlock { anchor, .. }
            | QuestEffect::SetCheckpoint { anchor, .. }
            | QuestEffect::Bonfire { anchor, .. } => vec![("anchor".to_string(), anchor)],
            QuestEffect::MoveNpc { to_anchor, .. } | QuestEffect::MoveActor { to_anchor, .. } => {
                vec![("to_anchor".to_string(), to_anchor)]
            }
            QuestEffect::DamagePlayers {
                within: Some(zone), ..
            } => vec![("in/anchor".to_string(), &zone.anchor)],
            QuestEffect::BeginStealth { zones, .. } => zones
                .iter()
                .enumerate()
                .map(|(i, z)| (format!("zones/{i}/anchor"), &z.anchor))
                .collect(),
            QuestEffect::PlaySound {
                at: Some(SoundAt::Anchor { anchor }),
                ..
            } => vec![("at/anchor".to_string(), anchor)],
            // spec-0022 trap-payload verbs. Both anchors of a `volley` are
            // load-bearing for the coverage proof, so both register here — a
            // typo'd `kill_zone` must be a dangling-reference error, never a
            // silently zero-cell (and therefore vacuously "covered") volley.
            QuestEffect::Volley {
                from_anchor,
                kill_zone,
                ..
            } => vec![
                ("from_anchor".to_string(), from_anchor),
                ("kill_zone/anchor".to_string(), &kill_zone.anchor),
            ],
            QuestEffect::Collapse { region_anchor, .. } => {
                vec![("region_anchor/anchor".to_string(), &region_anchor.anchor)]
            }
            // Both cutscene spellings (`DW0199` polices mixing them): the v0.6
            // multi-shot list, or the v0.4 single-shot fields flattened at the
            // effect's own level.
            QuestEffect::Cutscene {
                shots,
                path,
                look_at,
                ..
            } => {
                let mut out: Vec<(String, &AnchorId)> = shots
                    .iter()
                    .enumerate()
                    .flat_map(|(i, s)| shot_refs(&format!("shots/{i}/"), s))
                    .collect();
                out.extend(
                    path.iter()
                        .enumerate()
                        .map(|(j, w)| (format!("path/{j}/anchor"), &w.anchor)),
                );
                if let Some(t) = look_at {
                    out.push(("look_at/anchor".to_string(), &t.anchor));
                }
                out
            }
            _ => Vec::new(),
        }
    }

    /// The `cutscene` camera subject if this is a single-shot `cutscene` carrying
    /// the v0.6 `look_at` field (reserved `DW0141` under a pre-0.6 campaign).
    pub fn cutscene_look_at(&self) -> Option<&CameraTarget> {
        match self {
            QuestEffect::Cutscene { look_at, .. } => look_at.as_ref(),
            _ => None,
        }
    }

    /// `true` if this is a `cutscene` written in the v0.6 multi-shot form
    /// (reserved `DW0141` under a pre-0.6 campaign).
    pub fn cutscene_multi_shot(&self) -> bool {
        matches!(self, QuestEffect::Cutscene { shots, .. } if !shots.is_empty())
    }

    /// The normalized shot list of a `cutscene`, whichever spelling was used: the
    /// v0.6 `shots` list as-is, or the v0.4 `path`/`seconds`/`look_at` fields as a
    /// single shot. `None` for a non-cutscene effect; an empty list for a cutscene
    /// whose shape is invalid (`DW0199` reports that).
    pub fn cutscene_shots(&self) -> Option<Vec<CameraShot>> {
        match self {
            QuestEffect::Cutscene {
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

    /// The `narrate` line's style and text if this is a `narrate` rendered **on
    /// screen** — `title`, `subtitle` or `art` — rather than in chat. These are the
    /// styles vanilla draws centred and unwrapped, so their rendered width is
    /// length-checked against the screen (`DW0330`); `chat` scrolls and wraps, so it
    /// is exempt.
    pub fn narrate_on_screen(&self) -> Option<(NarrateStyle, &str)> {
        match self {
            QuestEffect::Narrate {
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
            | QuestEffect::PlaySound { requires_flags, .. }
            | QuestEffect::DamagePlayers { requires_flags, .. }
            | QuestEffect::Volley { requires_flags, .. }
            | QuestEffect::Collapse { requires_flags, .. } => requires_flags,
            // Terminal / party- or session-global verbs are not per-effect
            // gatable: `campaign-complete` is terminal; `set-checkpoint`
            // (`spawnpoint @a`) / `begin-stealth` / `end-stealth` are party-wide
            // session state; and the actor staging verbs (`spawn-actor` /
            // `despawn-actor` / `move-actor` / `unleash-actor` / `sequence`) and
            // `spawn-npc` are world-global staging — none are per-player `@s`
            // effects. Gate these at the objective / dialogue-option level instead.
            QuestEffect::CampaignComplete { .. }
            | QuestEffect::SpawnNpc { .. }
            | QuestEffect::SetCheckpoint { .. }
            | QuestEffect::Bonfire { .. }
            | QuestEffect::BeginStealth { .. }
            | QuestEffect::EndStealth
            | QuestEffect::SpawnActor { .. }
            | QuestEffect::DespawnActor { .. }
            | QuestEffect::MoveActor { .. }
            | QuestEffect::UnleashActor { .. }
            | QuestEffect::Sequence { .. } => &[],
        }
    }

    /// The per-effect **negative** flag gate (DSL v0.6): flags whose being set
    /// (per player) **suppresses** this effect — the dual of
    /// [`QuestEffect::requires_flags`], accepted on exactly the same verbs.
    /// Emission wraps a gated effect's commands in a per-player
    /// `execute unless score @s dw.f_<flag> matches 1` guard, so an unset score
    /// counts as "not set" (flag scores are never pre-initialized). Empty for an
    /// ungated effect and for the verbs that are not per-effect gatable (see
    /// `requires_flags`); a pre-0.6 campaign that uses it is rejected (`DW0141`).
    pub fn forbids_flags(&self) -> &[FlagId] {
        match self {
            QuestEffect::OpenGate { forbids_flags, .. }
            | QuestEffect::CloseGate { forbids_flags, .. }
            | QuestEffect::GiveItem { forbids_flags, .. }
            | QuestEffect::SetFlag { forbids_flags, .. }
            | QuestEffect::SpawnWave { forbids_flags, .. }
            | QuestEffect::Narrate { forbids_flags, .. }
            | QuestEffect::SetBlock { forbids_flags, .. }
            | QuestEffect::DespawnNpc { forbids_flags, .. }
            | QuestEffect::MoveNpc { forbids_flags, .. }
            | QuestEffect::Cutscene { forbids_flags, .. }
            | QuestEffect::SetTime { forbids_flags, .. }
            | QuestEffect::SetWeather { forbids_flags, .. }
            | QuestEffect::PlaySound { forbids_flags, .. }
            | QuestEffect::DamagePlayers { forbids_flags, .. }
            | QuestEffect::Volley { forbids_flags, .. }
            | QuestEffect::Collapse { forbids_flags, .. } => forbids_flags,
            QuestEffect::CampaignComplete { .. }
            | QuestEffect::SpawnNpc { .. }
            | QuestEffect::SetCheckpoint { .. }
            | QuestEffect::Bonfire { .. }
            | QuestEffect::BeginStealth { .. }
            | QuestEffect::EndStealth
            | QuestEffect::SpawnActor { .. }
            | QuestEffect::DespawnActor { .. }
            | QuestEffect::MoveActor { .. }
            | QuestEffect::UnleashActor { .. }
            | QuestEffect::Sequence { .. } => &[],
        }
    }

    /// The `on_arrive` bundle if this is a `move-npc` carrying one (DSL v0.6;
    /// parity with `move-actor`). `None` for a bare `move-npc` and every other
    /// effect — the v0.6-reserved gate (`DW0141`) keys off `Some`.
    pub fn move_npc_on_arrive(&self) -> Option<&[QuestEffect]> {
        match self {
            QuestEffect::MoveNpc { on_arrive, .. } if !on_arrive.is_empty() => {
                Some(on_arrive.as_slice())
            }
            _ => None,
        }
    }

    /// The actor id this effect targets, if it is one of the actor staging effects
    /// (`spawn-actor`/`despawn-actor`/`move-actor`/`unleash-actor`). `sequence` has
    /// no single actor (its nested effects each carry their own).
    pub fn actor_ref(&self) -> Option<&ActorId> {
        match self {
            QuestEffect::SpawnActor { actor, .. }
            | QuestEffect::DespawnActor { actor, .. }
            | QuestEffect::MoveActor { actor, .. }
            | QuestEffect::UnleashActor { actor, .. } => Some(actor),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Stage 7 — world-edits (the map editor edit script, DSL v0.6, spec-0017)
// ---------------------------------------------------------------------------

/// Stage 7 payload (optional; DSL v0.6, spec-0017): the map-editor edit script.
///
/// The artifact of record for L3 world detailing: an ordered list of edit
/// batches the compiler replays deterministically **after** world assembly.
/// The world files are never truth — same DSL + same edits + same seed →
/// byte-identical world (ADR-0006). A campaign without a `world-edits.json`
/// builds byte-identically to one from before this stage existed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorldEditsContent {
    /// Ordered edit batches, replayed in order. After every batch the post-edit
    /// invariants re-prove (walkability, sealing + relight, boundary safety),
    /// so each batch is a valid, snapshot-reviewable world state.
    pub batches: Vec<EditBatch>,
}

/// One edit batch: an ordered group of edit verbs applied to a single area,
/// checked and snapshot-rendered as a unit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditBatch {
    /// Batch id (`batch/<kebab>`), unique in the script. Also the batch's
    /// snapshot name and seed-stream label — renaming a batch deliberately
    /// reseeds its noise.
    pub id: EditBatchId,
    /// The stage-1 area this batch edits. Frames and regions resolve against
    /// this area's placed pieces and anchors.
    pub area: AreaId,
    /// Authoring context (why this batch exists). Machine-ignored; **excluded**
    /// from l10n like `theme`/`premise`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Ordered edit verbs. `select` defines named regions; later verbs in the
    /// same batch refer back to them (strictly backward, like every DSL ref).
    pub edits: Vec<WorldEdit>,
}

/// One edit verb (spec-0017 L3). Every verb operates on named regions and every
/// seeded verb derives its noise stream from the campaign seed + its script
/// position (`edits/<batch>/<index>`) — no wall clock, no unseeded RNG.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "verb", rename_all = "kebab-case", deny_unknown_fields)]
pub enum WorldEdit {
    /// Define a named region (`region/<kebab>`) for later verbs in this batch.
    Select {
        /// The region name being defined (unique within the batch).
        name: RegionId,
        /// The region's shape (box, surface band, palette match, or a
        /// composition of earlier regions).
        shape: RegionShape,
    },
    /// Fill every cell of a region from a seeded palette recipe (value-noise
    /// keyed, so picks cluster into patches — never a uniform fill).
    Fill {
        /// The region (an earlier `select` in this batch) to fill.
        region: RegionId,
        /// The palette recipe to fill with.
        recipe: PaletteRecipe,
    },
    /// Like `fill`, but only rewrites cells whose current block matches one of
    /// `matching` (base ids; blockstate suffixes are ignored when matching).
    Replace {
        /// The region (an earlier `select` in this batch) to edit.
        region: RegionId,
        /// Base block ids to rewrite (e.g. `["minecraft:stone"]`).
        matching: Vec<String>,
        /// The palette recipe to rewrite them with.
        recipe: PaletteRecipe,
    },
    /// Clear a region to air. Sealing-aware: the carved region re-enters the
    /// sealing + relight passes and every walkability invariant re-proves.
    Carve {
        /// The region (an earlier `select` in this batch) to clear.
        region: RegionId,
    },
    /// Reshape terrain surface within a region (raise / lower / smooth).
    Morph {
        /// The region (an earlier `select` in this batch) whose columns to
        /// reshape. The region defines the footprint and where each column's
        /// surface is read (the highest occupied cell in the region's y-range);
        /// `raise`/`smooth` may add cells above the region's top — reshaping
        /// upward is the point — while removal only touches region cells.
        region: RegionId,
        /// The surface operation.
        op: MorphOp,
    },
    /// Seeded dressing scatter (spec-0017 PR 2): drop weighted single-block
    /// dressing (flora, rocks, props) onto standable cells of a region —
    /// air cells with an occupied cell directly below — honoring keep-clear
    /// envelopes (`avoid`). Per-cell white-noise density gate (dressing wants
    /// speckle, not the fill verbs' clustered patches), deterministic from the
    /// campaign seed + script position.
    Scatter {
        /// The region (an earlier `select` in this batch) to dress.
        region: RegionId,
        /// Weighted dressing blocks (blockstate suffixes allowed).
        items: Vec<PaletteBlock>,
        /// Per-candidate placement probability in `(0, 1]`.
        density: f64,
        /// Keep-clear envelopes: earlier regions whose cells (and columns —
        /// matched by `(x, z)`) never receive dressing.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        avoid: Vec<RegionId>,
        /// Optional minimum spacing: when set, candidates are taken in
        /// descending noise order and one is rejected while another accepted
        /// candidate is closer than this on **both** horizontal axes (the
        /// generators' spread rule).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        spacing: Option<u32>,
        /// Optional cap on how many items are placed (highest-noise first).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<u32>,
    },
    /// Structural flora (spec-0017 PR 2): plant hand-shaped trees via the
    /// lean-or-grow canopy rules (#121) — a canopy that would reach a
    /// keep-clear (`avoid`) column first leans one block away from it; if that
    /// still covers the corridor the tree grows tall instead, arching its
    /// whole canopy 3 blocks above the trunk's floor. No leaf is ever sliced.
    Plant {
        /// The region (an earlier `select` in this batch) to plant in. Trunk
        /// cells are standable region cells (air over an occupied cell).
        region: RegionId,
        /// The tree species (canopy shape rules are per-species).
        tree: TreeKind,
        /// How many trees to plant (≥ 1; highest-noise candidates first).
        count: u32,
        /// Keep-clear envelopes: trunks never stand in these columns and
        /// canopies lean/grow to clear them.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        avoid: Vec<RegionId>,
        /// Minimum trunk spacing (reject when closer on BOTH axes; default 4).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        spacing: Option<u32>,
    },
    /// Stamp a prefab fragment (spec-0017 PR 2): copy a library prefab's
    /// non-air cells into the world at a frame-resolved position. The fragment
    /// is a first-class library prefab — its provenance/license metadata is
    /// recorded and validated exactly like any placed prefab (ADR-0013);
    /// nothing outside the library can be stamped.
    Fragment {
        /// The library prefab to stamp.
        prefab: PrefabId,
        /// The frame `at` resolves in.
        frame: EditFrame,
        /// Where the fragment's local `(0, 0, 0)` lands (frame coordinates).
        at: [i32; 3],
        /// Placement rotation (default `none`), the same quarter-turn set as
        /// `/place template`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rotation: Option<FragmentRotation>,
    },
    /// Explicit region relight (spec-0017 PR 2, spec-0010 machinery): run the
    /// deterministic fixture-placement pass over ONE region and bake the
    /// resulting fixtures into the edit script's writes — authorial control of
    /// where fixtures land, instead of the whole-area pass's greedy siting.
    /// (The whole-area relight still re-proves after every batch either way.)
    Relight {
        /// The region (an earlier `select` in this batch) to relight: its
        /// reachable walkable cells are brought to `min_light`.
        region: RegionId,
        /// Fixture override; default = the area's declared `lighting.fixture`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fixture: Option<Fixture>,
        /// Target light override (1..=14); default = the area's declared
        /// `lighting.min_light`. Required (with `fixture`) when the area
        /// declares no `lighting`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min_light: Option<u8>,
    },
    /// L2 massing (spec-0017 PR 3): replace a placed piece with another
    /// library prefab that re-mates every currently-mated socket at its exact
    /// world pose (any rotation; overlap-checked). Applied at **plan** time —
    /// the whole downstream ladder (anchors, gate reachability, assembly,
    /// relight, nav, L3 detailing) re-runs over the massaged layout. Massing
    /// verbs live in massing-only batches, ordered before every detailing
    /// batch.
    SwapPiece {
        /// The piece's placement index in its area's solved layout (0-based,
        /// entry first).
        piece: u32,
        /// The prefab the indexed piece must currently be (drift guard).
        prefab: PrefabId,
        /// The library prefab to swap in.
        with: PrefabId,
    },
    /// L2 massing (spec-0017 PR 3): attach a new piece at a specific **open**
    /// (unmated) socket of an existing piece — the targeted form of the
    /// solver's frontier attach. The socket opens (its seal becomes a
    /// passage); the new piece's other sockets seal.
    InsertPiece {
        /// The host piece's placement index.
        at_piece: u32,
        /// The prefab the host piece must currently be (drift guard).
        prefab: PrefabId,
        /// The host's connector index (prefab metadata `connectors` order).
        socket: u32,
        /// The library prefab to attach.
        insert: PrefabId,
    },
    /// L2 massing (spec-0017 PR 3): remove a **leaf** piece (exactly one
    /// mated socket; never the entry piece). The neighbour's socket unmates
    /// and re-seals. Removal shifts later placement indices — order removals
    /// before other index-referencing massing verbs.
    RemovePiece {
        /// The piece's placement index.
        piece: u32,
        /// The prefab the indexed piece must currently be (drift guard).
        prefab: PrefabId,
    },
    /// L2 massing (spec-0017 PR 3): override one socket's seal — `open`
    /// clears the opening to a passage, `sealed` walls it up — independent of
    /// its mated state (sealing a mated doorway makes a wall between joined
    /// pieces; opening an unmated exterior socket exposes the outside, which
    /// the boundary-safety proof then judges).
    RewireSocket {
        /// The piece's placement index.
        piece: u32,
        /// The prefab the indexed piece must currently be (drift guard).
        prefab: PrefabId,
        /// The connector index (prefab metadata `connectors` order).
        socket: u32,
        /// The socket's new state.
        state: SocketState,
    },
    /// L2 massing (spec-0017 PR 3): re-pick this piece from its area pool's
    /// compatible members (weighted, seeded from the campaign seed + this
    /// verb's script position — moving the verb deliberately re-rolls). The
    /// current prefab is excluded, so a reseed always changes the piece or
    /// errors loudly.
    ReseedPiece {
        /// The piece's placement index.
        piece: u32,
        /// The prefab the indexed piece must currently be (drift guard).
        prefab: PrefabId,
    },
}

/// A socket seal state for `rewire-socket` (spec-0017 PR 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SocketState {
    /// The opening is cleared to a passage.
    Open,
    /// The opening is walled up.
    Sealed,
}

/// A tree species for the `plant` verb (spec-0017 PR 2). One species per
/// canopy-rule implementation; the shipped rule set is the #121 oak.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TreeKind {
    /// Small hand-shaped oak (3–4 logs, 5-wide leaf ball) with the
    /// lean-or-grow corridor rules.
    Oak,
}

/// A `fragment` stamp rotation (spec-0017 PR 2) — the `/place template`
/// quarter-turn set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum FragmentRotation {
    /// No rotation.
    None,
    /// 90° clockwise.
    Clockwise90,
    /// 180°.
    Clockwise180,
    /// 90° counterclockwise.
    Counterclockwise90,
}

/// A `select` verb's shape (spec-0017): primitive shapes resolve in a declared
/// frame; compositions combine earlier regions of the same batch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum RegionShape {
    /// An inclusive axis-aligned box, `min`/`max` in the declared frame.
    Box {
        /// The coordinate frame `min`/`max` resolve in.
        frame: EditFrame,
        /// Inclusive minimum corner (frame coordinates).
        min: [i32; 3],
        /// Inclusive maximum corner (frame coordinates; each axis ≥ `min`).
        max: [i32; 3],
    },
    /// The band of cells at `from..=to` blocks relative to each column's
    /// terrain surface (the highest non-air cell of the column) within an
    /// earlier region. `from: 1, to: 3` is the 3 cells of air-space above the
    /// surface; `from: 0, to: 0` is the surface cells themselves; negative
    /// offsets reach below the surface.
    SurfaceBand {
        /// The earlier region whose columns are scanned.
        over: RegionId,
        /// Inclusive band start, relative to each column's surface y.
        from: i32,
        /// Inclusive band end (≥ `from`), relative to each column's surface y.
        to: i32,
    },
    /// The cells of an earlier region whose current block matches one of
    /// `blocks` (base ids; blockstate suffixes ignored when matching).
    PaletteMatch {
        /// The earlier region to filter.
        within: RegionId,
        /// Base block ids to match (e.g. `["minecraft:grass_block"]`).
        blocks: Vec<String>,
    },
    /// The union of earlier regions.
    Union {
        /// The earlier regions to unite (≥ 2).
        of: Vec<RegionId>,
    },
    /// The intersection of earlier regions.
    Intersect {
        /// The earlier regions to intersect (≥ 2).
        of: Vec<RegionId>,
    },
    /// An earlier region minus other earlier regions.
    Subtract {
        /// The earlier region to start from.
        base: RegionId,
        /// The earlier regions to remove from it (≥ 1).
        remove: Vec<RegionId>,
    },
}

/// The coordinate frame a primitive [`RegionShape`] resolves in (spec-0017):
/// piece-local or anchor-relative — never raw world coordinates, so an edit
/// script survives a layout's world placement moving.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum EditFrame {
    /// The local frame of a placed piece of the batch's area: `[0, 0, 0]` is
    /// the piece's structure origin, axes as authored (the compiler applies
    /// the piece's placed rotation).
    PieceLocal {
        /// The piece's placement index in the area's solved layout (0-based,
        /// entry piece first — the order `delvec snapshot`'s manifest lists).
        piece: u32,
        /// The prefab the indexed piece must be (a drift guard: if a re-solve
        /// changed the layout, the mismatch is a loud compile error, never a
        /// silently misplaced edit).
        prefab: PrefabId,
    },
    /// Relative to a resolved anchor of the batch's area: `[0, 0, 0]` is the
    /// anchor cell, axes world-aligned.
    AnchorRelative {
        /// The anchor (prefab metadata, resolved by the compiler).
        anchor: AnchorId,
    },
}

/// A surface operation for the `morph` verb (spec-0017).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum MorphOp {
    /// Raise each column's surface by up to `by` blocks, drawing the added
    /// cells from `recipe` (value-noise keyed per cell, so a raised band reads
    /// as natural strata, not an extruded slab).
    Raise {
        /// How many blocks to raise each column (≥ 1).
        by: u32,
        /// The palette recipe for the added cells.
        recipe: PaletteRecipe,
    },
    /// Lower each column's surface by up to `by` blocks (carving the topmost
    /// solid cells to air).
    Lower {
        /// How many blocks to lower each column (≥ 1).
        by: u32,
    },
    /// Relax each column's surface toward the mean of its cardinal neighbours
    /// (one block per pass), turning steps into slopes. Added cells draw from
    /// `recipe`; removed cells carve to air. Deterministic double-buffered
    /// passes in fixed scan order.
    Smooth {
        /// Relaxation passes (≥ 1).
        passes: u32,
        /// The palette recipe for cells a pass adds.
        recipe: PaletteRecipe,
    },
}

/// A seeded palette recipe (spec-0017): weighted blocks picked per cell by a
/// smooth value-noise sample, the island/cave generators' proven primitive —
/// picks cluster into strata/patches instead of per-cell speckle, and a
/// single-entry recipe is the degenerate (discouraged) uniform case.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PaletteRecipe {
    /// Weighted palette entries (≥ 1; ≥ 2 for any visible surface).
    pub blocks: Vec<PaletteBlock>,
    /// Noise frequency in blocks⁻¹ (default `0.35` — patches a few blocks
    /// across). Larger = smaller patches. Must be finite and > 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
}

/// One weighted entry of a [`PaletteRecipe`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PaletteBlock {
    /// Vanilla block id (validated against the pinned 1.21.11 registry), with
    /// an optional verbatim blockstate suffix (`minecraft:oak_leaves[persistent=true]`).
    pub block: String,
    /// Relative weight (finite, > 0).
    pub weight: f64,
}

/// Where in the campaign one effect sits — the attribution every per-branch
/// proof and the branch chronicle need (spec-0025).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EffectSite {
    /// A quest's `on_objective_complete[<objective>]` bundle.
    Objective {
        /// The owning quest.
        quest: String,
        /// The objective whose completion fires the bundle.
        objective: String,
    },
    /// A quest's `on_complete` bundle.
    QuestComplete {
        /// The owning quest.
        quest: String,
    },
    /// An environment trigger's `effects` bundle — ambient, no DAG position.
    Trigger {
        /// The trigger id.
        trigger: String,
    },
    /// A trap's spec-0022 `payload` bundle — ambient, no DAG position.
    Trap {
        /// The trap id.
        trap: String,
    },
    /// A **dialogue option's** `set-checkpoint` `on_respawn` bundle — ambient, no
    /// DAG position, and the only site that does not live in the quests stage.
    ///
    /// This variant did not exist until the effect-root sweep, and its absence was
    /// load-bearing: `EffectSite` had no way to *represent* a dialogue-hosted
    /// bundle, so the four proofs that walk [`for_each_campaign_effect`]
    /// (`combat::actor_beats`, `validate::difficulty_checks`,
    /// `daylight::fightable_actor`, `nav::actor_fights`) could not have seen root 5
    /// even if their authors had thought of it. Widening the type is what let the
    /// walk widen.
    DialogueRespawn {
        /// The NPC whose dialogue tree hosts the option.
        npc: String,
        /// The node the option sits under.
        node: String,
    },
}

impl EffectSite {
    /// The quest this site belongs to, if it has a DAG position at all.
    pub fn quest(&self) -> Option<&str> {
        match self {
            EffectSite::Objective { quest, .. } | EffectSite::QuestComplete { quest } => {
                Some(quest)
            }
            EffectSite::Trigger { .. }
            | EffectSite::Trap { .. }
            | EffectSite::DialogueRespawn { .. } => None,
        }
    }
}

/// Visit **every** effect the compiler can lower — at every one of the five
/// effect roots, top-level and transitively nested — in a fixed deterministic
/// order, invoking `f(json_pointer, site, effect)`.
///
/// The roots come from [`crate::effects::for_each_effect_root`], the single
/// enumeration; nesting is descended through the single
/// [`QuestEffect::nested_effect_lists_labeled`] authority. Neither axis is
/// enumerated here, which is the point: this walk used to hand-list four of the
/// five roots (it had no `EffectSite` variant for the fifth), so every proof
/// defined in terms of it inherited that blind spot.
pub fn for_each_campaign_effect<'a>(
    c: &'a crate::envelope::Campaign,
    f: &mut dyn FnMut(&str, &EffectSite, &'a QuestEffect),
) {
    crate::effects::for_each_effect_root(c, &mut |root, list| {
        let site = match root.owner {
            crate::effects::EffectRootOwner::ObjectiveComplete { quest, objective } => {
                EffectSite::Objective {
                    quest: quest.id.as_str().to_string(),
                    objective: objective.to_string(),
                }
            }
            crate::effects::EffectRootOwner::QuestComplete { quest } => EffectSite::QuestComplete {
                quest: quest.id.as_str().to_string(),
            },
            crate::effects::EffectRootOwner::Trigger(t) => EffectSite::Trigger {
                trigger: t.id.as_str().to_string(),
            },
            crate::effects::EffectRootOwner::TrapPayload(t) => EffectSite::Trap {
                trap: t.id.as_str().to_string(),
            },
            crate::effects::EffectRootOwner::DialogueRespawn => {
                // The npc and node are in the root's path; parse them back rather
                // than widening the root walk's owner for one consumer.
                let seg = |n: usize| -> String {
                    root.path.split('/').nth(n).unwrap_or_default().to_string()
                };
                EffectSite::DialogueRespawn {
                    npc: seg(3),
                    node: seg(5),
                }
            }
        };
        for (i, eff) in list.iter().enumerate() {
            campaign_effect_deep(eff, &format!("{}/{i}", root.path), &site, f);
        }
    });
}

fn campaign_effect_deep<'a>(
    eff: &'a QuestEffect,
    path: &str,
    site: &EffectSite,
    f: &mut dyn FnMut(&str, &EffectSite, &'a QuestEffect),
) {
    f(path, site, eff);
    for (pseg, _kseg, list) in eff.nested_effect_lists_labeled() {
        for (j, inner) in list.iter().enumerate() {
            campaign_effect_deep(inner, &format!("{path}/{pseg}/{j}"), site, f);
        }
    }
}
