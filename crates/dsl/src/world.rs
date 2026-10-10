//! Stage 1 — the world: the setting's time, weather, difficulty, horizon,
//! boundary, areas, pieces, skies and texture overrides (spec-0001).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::serde_fields::default_true;
use crate::{AreaId, AtmosphereId, CelestialTime, Clock, MoonPhase, PoolId, PrefabId};

#[cfg(doc)]
use crate::Verb;

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
    /// The areas the delve is made of; each binds exactly one of `prefab` /
    /// `prefab_pool`.
    ///
    /// **A campaign declares its placement in exactly one document, so this
    /// list is empty on a site-plan campaign** (`DW0839`): where a
    /// `site-plan.json` is present the plan is the placement authority, its one
    /// place is `area/site`, and declaring `areas[]` as well gives every
    /// question about where something is two answers. Empty is therefore a
    /// legitimate and common value, not a campaign that forgot to place
    /// anything.
    pub areas: Vec<Area>,
    /// Additional author-declared translation languages (BCP-47-style codes, e.g.
    /// `["zh-cn"]`). English (`en`) is implicit, always canonical, and is **never**
    /// listed here (spec-0001 i18n addendum). Absent or empty = English-only. Every
    /// declared language must ship a fully-covering `l10n/<code>.json` sidecar
    /// (`DW0180`/`DW0181`). Stage docs themselves stay pure English.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<String>,
    /// **The hour this delve is played at** (DSL v0.5, spec-0010; required since
    /// spec-0061). Dimension-global; frozen by environment sealing
    /// (`advance_time false`) so the set state persists. Affects sky attenuation
    /// in the compiler's assembled-light model.
    ///
    /// **Required, and it has no default.** "This delve is played at noon" is a
    /// design decision, and a mechanism that supplies one silently when the
    /// author said nothing is exactly what `CLAUDE.md` forbids a primitive from
    /// encoding — the same ruling spec-0060 §4.1 made for `walk_y`. It is also
    /// the world half of the comparison `DW0890` makes against the approved
    /// design's rows, so every campaign has to state it for the comparison to
    /// have two sides. Emission is unchanged: `time set <kw>` was always
    /// emitted, so a campaign that already declared this builds
    /// byte-identically.
    pub time: WorldTime,
    /// **The weather this delve is played in** (DSL v0.5, spec-0010; required
    /// since spec-0061). Dimension-global; frozen by environment sealing
    /// (`advance_weather false`). Rain and thunder attenuate effective sky
    /// brightness in the assembled-light model.
    ///
    /// Required, with no default, for the reason [`WorldContent::time`] gives.
    /// Emission is unchanged: `weather <kw>` is emitted only for a declared
    /// non-`clear` weather, because `clear` is vanilla's own state.
    pub weather: WorldWeather,
    /// Declared combat difficulty (DSL v0.6). Absent =
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
    /// The scenic horizon: the ground and the sky the map stands in
    /// (spec-0026). Absent or `void` is the void world. `ocean` swaps the world
    /// generator for a deterministic superflat sea (bedrock/stone/water, sea
    /// level y=62) and drops the area datum to y=60 so island pieces meet the
    /// sea at their authored waterline. `valley` rings the map in a generated
    /// mountain annulus — the one base that builds terrain rather than picking
    /// a generator. Either a string shorthand or the object form
    /// `{base, …params}`; see [`Horizon`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizon: Option<Horizon>,
    /// **How far a player must be able to see**, in chunks (spec-0091): the
    /// server's `view-distance`, declared by the campaign whose far views need
    /// it. A thing farther from a body than the served radius is never sent to
    /// that body's client, so a landmark meant to be seen from across the map
    /// is a declaration here, not a hope. Absent = the engine's floor
    /// ([`crate::viewdistance::FLOOR`], 10 chunks = 160 blocks), which every
    /// proof in the engine is written against; declared in
    /// `FLOOR..=CEILING` (vanilla serves at most 32). A camera, a sightline, a
    /// view or a cutscene shot aimed past the served radius is refused
    /// (`DW0956`); the build states the heap the declared distance costs the
    /// host at the player cap, and the hosting side meets it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_distance: Option<u8>,
    /// **The skies a place can stand under** (spec-0080). Each one is declared
    /// once, here, beside `time`, `weather` and `horizon` — the other
    /// statements about the sky the party stands under — and ships as a
    /// datapack biome (`<ns>:atmosphere/<kebab>`) built from the pinned game's
    /// environment attributes. A place carries one from the first tick
    /// ([`Area::atmosphere`], `boxes[].atmosphere`), and a beat repaints a
    /// volume with another ([`Verb::SetAtmosphere`]). Absent or empty: every
    /// cell stands in the horizon's biome. One no place carries and no beat
    /// paints is refused (`DW0930`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub atmospheres: Vec<Atmosphere>,
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
    /// and turns on the analyzer's n-agent division
    /// proof. Out of `1..=4` is `DW0370`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_players: Option<u8>,
    /// **The vanilla textures this delve replaces** (spec-0084). Each row names
    /// one texture the pinned client ships and the campaign's own image for it,
    /// at `textures/<id>.png` in the campaign directory; the build bakes it into
    /// the resource pack at the vanilla path, so it is drawn wherever the client
    /// draws that texture — every mob of that kind, the moon over every area —
    /// for every player who accepted the pack. Absent or empty = vanilla's look.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub textures: Vec<TextureOverride>,
    /// **Whether a player must accept this delve's resource pack to play it**
    /// (spec-0084 §11). `true` is emitted as `require-resource-pack=true`: a
    /// player who declines is disconnected by the server. Absent or `false` =
    /// the pack is offered and may be declined, in which case the player reads
    /// English and sees vanilla's textures. A host may still set the server's own
    /// flip (itzg's `RESOURCE_PACK_ENFORCE`), which is obeyed as given.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub require_resource_pack: bool,
    /// **A fallen player waits before rejoining** (spec-0077). After clicking
    /// *Respawn*, a player whose party still has somebody in play watches a
    /// teammate as a spectator for `seconds`, and counts as down for the party
    /// wipe while they wait. Absent = no wait, and emission is byte-identical to
    /// a campaign that never had the field. Needs a checkpoint or bonfire to come
    /// back to; `seconds` outside `1..=120`, or no checkpoint, is `DW0925`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub respawn_wait: Option<RespawnWait>,
}

/// One vanilla texture a campaign replaces (spec-0084 §3.1). The row is a
/// judgement and nothing more: width, height and frame count are read off the
/// file and the pinned client's census, and the namespace is fixed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TextureOverride {
    /// A bare kebab token, unique among the campaign's textures (`DW0190`). The
    /// image is `textures/<id>.png` in the campaign directory (`DW0309`), and a
    /// `textures/<id>.png.mcmeta` beside it ships with it as the animation.
    pub id: String,
    /// The texture replaced, as a resource location in the `minecraft`
    /// namespace without `textures/` and without `.png` — the path vanilla's own
    /// models and atlases use (e.g. `minecraft:entity/zombie/drowned`,
    /// `minecraft:environment/celestial/moon/full_moon`). It must name a texture
    /// the pinned client ships (`DW0939`).
    pub replaces: String,
    /// Where the image came from and under what licence (ADR-0013): original
    /// work (`spdx` and `source` both `original`), or an allowlisted third-party
    /// image with its `url`, and its `attribution` for CC BY (`DW0741`).
    pub license: crate::license::LicenseEvidence,
}

/// One declared sky (spec-0080 §3.1): what the party sees and hears while it
/// stands in a cell painted with this atmosphere's biome.
///
/// The biome is vanilla's one channel for sky colour, fog, clouds, sky-light
/// tint, stars, ambient particles, music, ambience, grass, foliage and water
/// tint, and precipitation. In the overworld the day cycle stacks over it: it
/// multiplies the colours (a biome's value survives, darkened at night), takes
/// the maximum of `star_brightness`, and replaces the sun, moon and star
/// angles, the sunrise colour and the moon phase outright — which is why those
/// five ids are refused (`DW0928`) and the sun cannot be moved from a place.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Atmosphere {
    /// `atmosphere/<kebab>`, unique.
    pub id: AtmosphereId,
    /// Environment attributes, keyed by id (`visual/sky_color`; the
    /// `minecraft:` prefix is optional). Which ids a campaign may set, the
    /// shape of each value and the range the pinned codec accepts are vendored
    /// data (`crates/delvec/data/environment-attributes-1.21.11.json`), not
    /// DSL surface: a value outside them is `DW0928`. A float attribute may
    /// also be written in vanilla's modifier form, `{"argument": 0.85,
    /// "modifier": "multiply"}`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, serde_json::Value>,
    /// Grass, foliage, dry-foliage and water tint (`#rrggbb`), each optional.
    /// Absent, vanilla derives grass and foliage from the climate and water is
    /// the void biome's `#3f76e4`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint: Option<AtmosphereTint>,
    /// What falls here when the world's weather is rain or thunder. The
    /// compiler derives the three vanilla fields that must agree from it
    /// (`has_precipitation`, `temperature`, `downfall`), and it is the fact
    /// `DW0496` reads at a cell: `none` under a rainy world means the undead
    /// burn here.
    pub precipitation: Precipitation,
    /// Overrides the derived `temperature` / `downfall`, for vanilla's own
    /// grass colormap at a named point. Must agree with `precipitation`
    /// (`DW0930`): snow below 0.15, rain at or above it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub climate: Option<Climate>,
}

/// An atmosphere's tint (spec-0080 §3.1.3): the biome `effects` colours the
/// pinned data writes, each `#rrggbb`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AtmosphereTint {
    /// `grass_color`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grass: Option<String>,
    /// `foliage_color`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foliage: Option<String>,
    /// `dry_foliage_color`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dry_foliage: Option<String>,
    /// `water_color`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub water: Option<String>,
}

/// What an atmosphere's biome lets fall (spec-0080 §3.1.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Precipitation {
    /// Nothing falls: `has_precipitation: false`.
    None,
    /// Rain falls: `has_precipitation: true`, temperature 0.5.
    Rain,
    /// Snow falls: `has_precipitation: true`, temperature 0.0.
    Snow,
}

/// A biome's climate pair (spec-0080 §3.1.4).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Climate {
    /// `temperature`: vanilla snows below 0.15.
    pub temperature: f64,
    /// `downfall`.
    pub downfall: f64,
}

/// How long a fallen player waits before rejoining the party (spec-0077 §3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RespawnWait {
    /// Seconds a fallen player waits after clicking *Respawn*, `1..=120`
    /// (`DW0925`). Counted on the server only while the player is online.
    pub seconds: u16,
    /// Whether a player who comes back with nobody else present also waits.
    /// Default `false`: a party of one never waits.
    #[serde(default)]
    pub alone: bool,
}

/// A declared world time state (DSL v0.5, spec-0010; the celestial spelling
/// spec-0081). The sole difference from vanilla is that the daylight cycle is
/// frozen (`advance_time false`), so a set state persists for the whole delve
/// until a `set-time` effect cuts to another.
///
/// **Two spellings of one clock.** A keyword — `day`, `noon`, `dusk`, `night`,
/// `midnight`, `dawn` — states vanilla's word with vanilla's meaning: the hour,
/// on day 0, so a keyword night shows a full moon. A celestial statement
/// ([`CelestialTime`], `{"moon": "just-risen", "phase": "new-moon"}`) names one
/// body, where it stands and, where the moon shows, its phase, and the engine
/// computes the tick count, day included. Both resolve to one [`Clock`]
/// ([`WorldTime::clock`]); two values are equal when their clocks are.
///
/// Vanilla's `/time set` primitive takes **either** one of four keywords or a raw
/// tick count, and the tick form is the general one. `dusk` and `dawn` are the
/// tick form exposed first-class, per the no-hack rule; a celestial statement is
/// the same primitive reached from a designer's sentence. A keyword on day 0
/// still emits its keyword verbatim ([`WorldTime::token`]), so existing
/// campaigns are byte-identical.
///
/// **There is no `Default`** (spec-0061 §4). A default hour is a design decision
/// wearing a mechanism's clothes, and `#[default] Noon` is what let a delve whose
/// whole approved look was night build, light-check and render under a blue noon
/// sky. Removing the impl is what makes that unwritable rather than merely
/// discouraged: `WorldContent::time` is required, and nothing can supply an hour
/// the author did not.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WorldTime {
    /// Morning daylight (`/time set day`, 1000 ticks).
    Day,
    /// Midday, brightest (`/time set noon`, 6000 ticks).
    Noon,
    /// Sunset onset (`/time set 12000`).
    Dusk,
    /// Night, sun fully down (`/time set night`, 13000 ticks).
    Night,
    /// Deep night, darkest (`/time set midnight`, 18000 ticks).
    Midnight,
    /// First light, just before sunrise (`/time set 23000`).
    Dawn,
    /// A sky stated in a designer's words (spec-0081).
    Celestial(CelestialTime),
}

/// The six keyword spellings of a [`WorldTime`] — the wire and schema form of
/// its keyword half.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TimeKeyword {
    /// Morning daylight (`/time set day`, 1000 ticks).
    Day,
    /// Midday, brightest (`/time set noon`, 6000 ticks).
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

impl From<TimeKeyword> for WorldTime {
    fn from(k: TimeKeyword) -> WorldTime {
        match k {
            TimeKeyword::Day => WorldTime::Day,
            TimeKeyword::Noon => WorldTime::Noon,
            TimeKeyword::Dusk => WorldTime::Dusk,
            TimeKeyword::Night => WorldTime::Night,
            TimeKeyword::Midnight => WorldTime::Midnight,
            TimeKeyword::Dawn => WorldTime::Dawn,
        }
    }
}

impl Serialize for WorldTime {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            WorldTime::Celestial(c) => c.serialize(s),
            kw => s.serialize_str(kw.keyword_str().expect("a keyword")),
        }
    }
}

impl<'de> Deserialize<'de> for WorldTime {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<WorldTime, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = WorldTime;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str(
                    "a time: a keyword (day, noon, dusk, night, midnight, dawn) or an object \
                     naming one body, where it stands and its phase \
                     ({\"moon\": \"just-risen\", \"phase\": \"new-moon\"})",
                )
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<WorldTime, E> {
                use serde::de::IntoDeserializer;
                TimeKeyword::deserialize(v.into_deserializer()).map(WorldTime::from)
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                map: A,
            ) -> Result<WorldTime, A::Error> {
                CelestialTime::deserialize(serde::de::value::MapAccessDeserializer::new(map))
                    .map(WorldTime::Celestial)
            }
        }
        de.deserialize_any(V)
    }
}

impl JsonSchema for WorldTime {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "WorldTime".into()
    }

    fn json_schema(g: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "description": "A time: a keyword, vanilla's hour on day 0 (a keyword night is a full \
                            moon), or a celestial statement naming one body, where it stands and, \
                            where the moon shows, its phase (spec-0081).",
            "anyOf": [g.subschema_for::<TimeKeyword>(), g.subschema_for::<CelestialTime>()],
        })
    }
}

impl WorldTime {
    /// The keyword table: `(the /time set argument, daytime ticks)`, or `None`
    /// for a celestial statement.
    ///
    /// A state vanilla names keeps its keyword — the argument the compiler has
    /// always emitted — so no shipped campaign's bytes move. A state vanilla does
    /// not name emits the equivalent tick count, which is the same primitive.
    const fn spec(self) -> Option<(&'static str, i64)> {
        match self {
            WorldTime::Day => Some(("day", 1000)),
            WorldTime::Noon => Some(("noon", 6000)),
            WorldTime::Dusk => Some(("12000", 12000)),
            WorldTime::Night => Some(("night", 13000)),
            WorldTime::Midnight => Some(("midnight", 18000)),
            WorldTime::Dawn => Some(("23000", 23000)),
            WorldTime::Celestial(_) => None,
        }
    }

    /// Whether this is one of the six keywords.
    pub fn is_keyword(self) -> bool {
        !matches!(self, WorldTime::Celestial(_))
    }

    /// The celestial statement, if this is one.
    pub fn celestial(self) -> Option<CelestialTime> {
        match self {
            WorldTime::Celestial(c) => Some(c),
            _ => None,
        }
    }

    /// The phase this value states, if any.
    pub fn stated_phase(self) -> Option<MoonPhase> {
        self.celestial().and_then(|c| c.phase)
    }

    /// The `daytime` tick value this state sets (the `time query daytime`
    /// read-back). Keywords: day=1000, noon=6000, dusk=12000 (sunset onset),
    /// night=13000, midnight=18000, dawn=23000; a celestial statement its
    /// position's tick ([`crate::celestial::position_tick`]).
    pub fn daytime_ticks(self) -> i64 {
        match self.spec() {
            Some((_, t)) => t,
            None => self.celestial().expect("celestial").daytime(),
        }
    }

    /// The day this value names as **the world's own time**: a keyword is day 0;
    /// a celestial statement the day its `phase` names, or day 0 where it names
    /// none (the moon is below the horizon, so no phase is stated).
    pub fn world_day(self) -> i64 {
        self.stated_phase().map(MoonPhase::index).unwrap_or(0)
    }

    /// **The clock this value sets**, at a site whose world declares `world`
    /// (spec-0081 §3.3).
    ///
    /// - A celestial statement is its position's tick on the day its `phase`
    ///   names, or the world's day where it states none — a cut changes the
    ///   hour, and the moon keeps the phase the world declared.
    /// - A keyword is its table row on day 0 where it states a sky — the
    ///   world's own time, a design row, a camera ([`TimeSite::Sky`]) — and on
    ///   the world's day where it is a `set-time` cut ([`TimeSite::Cut`]), which
    ///   changes the hour and keeps the moon.
    pub fn clock(self, site: TimeSite, world: WorldTime) -> Clock {
        let daytime = self.daytime_ticks();
        let day = match (self, site) {
            (WorldTime::Celestial(c), _) => c
                .phase
                .map(MoonPhase::index)
                .unwrap_or_else(|| world.world_day()),
            (_, TimeSite::Cut) => world.world_day(),
            (_, TimeSite::Sky) => 0,
        };
        Clock { day, daytime }
    }

    /// The world's own clock: [`WorldTime::clock`] at the world's site.
    pub fn world_clock(self) -> Clock {
        self.clock(TimeSite::Sky, self)
    }

    /// **The vanilla `/time set` argument for `clock`**, the one token every
    /// `time set` the engine emits goes through: a keyword on day 0 emits its
    /// table argument verbatim (`night`, `12000`), so no keyword campaign's bytes
    /// move; any other clock emits the integer `day × 24000 + daytime`.
    pub fn token(self, clock: Clock) -> String {
        match self.spec() {
            Some((tok, t)) if clock.day == 0 && clock.daytime == t => tok.to_string(),
            _ => clock.absolute().to_string(),
        }
    }

    /// The keyword spelling, if this is a keyword.
    fn keyword_str(self) -> Option<&'static str> {
        match self {
            WorldTime::Day => Some("day"),
            WorldTime::Noon => Some("noon"),
            WorldTime::Dusk => Some("dusk"),
            WorldTime::Night => Some("night"),
            WorldTime::Midnight => Some("midnight"),
            WorldTime::Dawn => Some("dawn"),
            WorldTime::Celestial(_) => None,
        }
    }

    /// **What an author writes** — this state's spelling in a document: the
    /// keyword, or the canonical JSON of a celestial statement
    /// (`{"moon":"high","phase":"new-moon"}`).
    ///
    /// Not [`WorldTime::token`], which is the `/time set` argument and is a raw
    /// tick count for every state vanilla does not name. A diagnostic that asks
    /// an author to declare an hour has to say `dusk`, not `12000`.
    pub fn keyword(self) -> String {
        match self {
            WorldTime::Celestial(c) => c.spelling(),
            kw => kw.keyword_str().expect("a keyword").to_string(),
        }
    }
}

/// Where a time value is written, which decides the day a keyword names
/// ([`WorldTime::clock`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeSite {
    /// A statement of a sky: the world's own time, a design row, a camera.
    Sky,
    /// A `set-time` effect, quest or dialogue.
    Cut,
}

/// A declared weather state (DSL v0.5, spec-0010). Values are the vanilla
/// `/weather` keywords; frozen (`advance_weather false`), so a set state persists.
///
/// **No `Default`**, for the reason [`WorldTime`] gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum WorldWeather {
    /// Clear sky (`/weather clear`). Vanilla's own state, so emission writes no
    /// `/weather` command for it.
    Clear,
    /// Rain (`/weather rain`).
    Rain,
    /// Thunderstorm (`/weather thunder`).
    Thunder,
}

impl WorldWeather {
    /// The word an author writes — identical to [`WorldWeather::token`] for
    /// every state, and stated separately so a message that names a document's
    /// vocabulary reads the document's vocabulary. See [`WorldTime::keyword`],
    /// where the two differ.
    pub fn keyword(self) -> &'static str {
        self.token()
    }

    /// The vanilla `/weather` keyword.
    pub fn token(self) -> &'static str {
        match self {
            WorldWeather::Clear => "clear",
            WorldWeather::Rain => "rain",
            WorldWeather::Thunder => "thunder",
        }
    }
}

/// The declared combat difficulty of the delve (DSL v0.6). Values are the
/// vanilla `/difficulty` keywords.
///
/// Difficulty is the single largest lever on how hard a delve *feels*, so the
/// campaign declares it rather than letting the compiler choose. Easy **halves
/// incoming player damage** — `min(dmg / 2 + 1, dmg)` — so a campaign tuned
/// under `easy` is tuned against a halved world. A campaign that
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

/// A scenic horizon (spec-0026). A horizon is a **composition of orthogonal
/// axes**, not an enum of monoliths: a **base** — what surrounds the map — and
/// that base's params. The field accepts a plain string shorthand
/// ([`HorizonName`]) or the object form [`HorizonSpec`] `{base, …params}`.
///
/// Consumers never match this wire enum. [`Horizon::resolved`] desugars both
/// forms into one [`ResolvedHorizon`] with the pinned defaults applied, and
/// [`horizon_base`] answers the one question most callers have.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Horizon {
    /// A bare base name — `"ocean"` is exactly `{base: "ocean"}`. The two
    /// bases that predate the horizon library were spelled this way and still
    /// are, byte-identically; a base added since is spellable this way too,
    /// because a base with every param at its default has nothing else to say.
    Name(HorizonBase),
    /// The object form `{base, …params}`.
    Spec(HorizonSpec),
}

/// What surrounds the map.
///
/// **One enumeration of bases, reachable two ways.** The shorthand
/// `horizon: "ocean"` and the object form `horizon: {base: "ocean"}` name this
/// same variant, which is what stops a base from existing in one spelling and
/// not the other. A separate list of "names" beside this one would be two
/// enumerations of the same thing, and the second base added would land in
/// whichever of them its author was looking at.
///
/// What is deliberately NOT here is a name that stands for a base plus a set of
/// params. Such a name reads as a thing, and the whole claim of this design is
/// that it is not one — it is a base with params set. A spelling that hides
/// which params it sets makes that claim unverifiable by looking at the
/// document, and buys a few saved keystrokes for it. Each base carries its own params on
/// [`HorizonSpec`], and a param foreign to the declared base is refused rather
/// than ignored.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum HorizonBase {
    /// Void superflat; no surround. The default.
    #[default]
    Void,
    /// Pinned water superflat, sea level 62; no surround.
    Ocean,
    /// A mountain annulus around a flat gap floor, with void ambient below the
    /// tile skirt. The one base that generates terrain.
    Valley,
}

impl HorizonBase {
    /// The kebab wire name.
    pub fn token(self) -> &'static str {
        match self {
            HorizonBase::Void => "void",
            HorizonBase::Ocean => "ocean",
            HorizonBase::Valley => "valley",
        }
    }

    /// Whether this base generates a surround — compiler-built terrain outside
    /// the map's own extent. `void` and `ocean` are pure ambient and build
    /// nothing.
    pub fn has_surround(self) -> bool {
        matches!(self, HorizonBase::Valley)
    }
}

/// The `horizon` object form: a `base` plus that base's params, all optional
/// with pinned defaults ([`horizon_defaults`]).
///
/// The `valley` surround generator carries a second flora and a second surface
/// palette (a cherry grove over `minecraft:cherry_grove`) and **this struct
/// does not expose them.** Every engine surface owes a gallery element in the
/// change that lands it; the element a second flora needs is a valley overlay,
/// and one is writable now that a one-area campaign's single prefab states an
/// extent ([`crate::placement::Extent`], `DW0855`) — the reason recorded here
/// was that no two-file overlay could ring a map, and that reason is spent.
/// What is left is that nothing has written the element, and a surface lands
/// with its element or it does not land. The shape is flat rather than
/// per-base tagged, and a param foreign to the declared base is refused
/// (`DW0853`) — so an `ocean` cannot quietly carry a `rim_height` that nothing
/// reads.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HorizonSpec {
    /// The base — what surrounds the map.
    pub base: HorizonBase,
    /// `valley`: the surround's total footprint as a multiple of the map's, on
    /// each axis (`2.0..=3.0`, default 2.5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ratio: Option<f64>,
    /// `valley`: crest height of the rim over the gap floor (`16..=128`,
    /// default 48).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rim_height: Option<i32>,
}

/// Pinned horizon param defaults. One table, so the doc comments, the resolver
/// and the diagnostics cannot drift.
pub mod horizon_defaults {
    /// `valley.ratio`.
    pub const RATIO: f64 = 2.5;
    /// `valley.ratio` lower bound — below 2.0 the annulus has no room for a
    /// gap floor and a slope run both.
    pub const RATIO_MIN: f64 = 2.0;
    /// `valley.ratio` upper bound — above 3.0 the surround is mostly terrain a
    /// body never reaches, at a cost that is all shipped bytes.
    pub const RATIO_MAX: f64 = 3.0;
    /// `valley.rim_height`.
    pub const RIM_HEIGHT: i32 = 48;
    /// `valley.rim_height` lower bound — a rim under 16 does not close the
    /// horizon from a body standing on the gap floor.
    pub const RIM_HEIGHT_MIN: i32 = 16;
    /// `valley.rim_height` upper bound — the build range is 384 blocks tall and
    /// the surround has to fit under whatever the map puts above it.
    pub const RIM_HEIGHT_MAX: i32 = 128;
}

/// A horizon with both wire forms desugared and every default applied — the
/// only view downstream code reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedHorizon {
    /// The base.
    pub base: HorizonBase,
    /// `valley.ratio`.
    pub ratio: f64,
    /// `valley.rim_height`.
    pub rim_height: i32,
}

impl Default for ResolvedHorizon {
    fn default() -> Self {
        ResolvedHorizon {
            base: HorizonBase::Void,
            ratio: horizon_defaults::RATIO,
            rim_height: horizon_defaults::RIM_HEIGHT,
        }
    }
}

impl ResolvedHorizon {
    /// The resolved horizon of `base` with every param at its pinned default.
    pub fn of_base(base: HorizonBase) -> Self {
        ResolvedHorizon {
            base,
            ..Default::default()
        }
    }
}

impl Horizon {
    /// Desugar either wire form to the one resolved view, defaults applied.
    pub fn resolved(&self) -> ResolvedHorizon {
        match self {
            Horizon::Name(base) => ResolvedHorizon::of_base(*base),
            Horizon::Spec(s) => ResolvedHorizon {
                base: s.base,
                ratio: s.ratio.unwrap_or(horizon_defaults::RATIO),
                rim_height: s.rim_height.unwrap_or(horizon_defaults::RIM_HEIGHT),
            },
        }
    }

    /// The resolved base.
    pub fn base(&self) -> HorizonBase {
        self.resolved().base
    }

    /// True when this declaration needs the horizon-library surface: the object
    /// form, or a bare name for a base that did not exist before it.
    ///
    /// The two bases that predate the library stay writable as bare names at
    /// the version that introduced them, and their emission does not move —
    /// which is what makes this a widening rather than a break. What is fenced
    /// is saying something the old surface had no spelling for.
    pub fn needs_horizon_library(&self) -> bool {
        match self {
            Horizon::Spec(_) => true,
            Horizon::Name(base) => match base {
                HorizonBase::Void | HorizonBase::Ocean => false,
                HorizonBase::Valley => true,
            },
        }
    }
}

/// The resolved base of an optional stage-1 `horizon` field — `Void` when
/// absent. The one helper every downstream consumer (placement, the ambient
/// model, emission) goes through, so that a new base cannot be forgotten at one
/// of them.
pub fn horizon_base(horizon: &Option<Horizon>) -> HorizonBase {
    horizon.as_ref().map(|h| h.base()).unwrap_or_default()
}

/// The resolved view of an optional stage-1 `horizon` field, with defaults
/// applied for an absent one.
pub fn resolved_horizon(horizon: &Option<Horizon>) -> ResolvedHorizon {
    horizon.as_ref().map(|h| h.resolved()).unwrap_or_default()
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
    /// **Whether the boundary returns a player who leaves it** (spec-0092 §10).
    /// Default `true`: the per-second clock returns any player outside the region
    /// to the last checkpoint. `false` keeps the region — every proof that reads
    /// it reads the same box — and emits no clock: the creator's switch for a
    /// world nobody can leave, where a return only fights a creator flying out
    /// to look at a far view. Legal only where the build proves no body can walk
    /// or swim out of the region (`DW0960`).
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub returns: bool,
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde's `skip_serializing_if` hands a reference
fn is_true(b: &bool) -> bool {
    *b
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
    /// (1..=14, default 7). Range-checked (`DW0196`).
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
/// full jigsaw layout semantics are spec-0002's.
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
    /// **The sky this place stands under from the first tick** (spec-0080
    /// §3.2): one of `world.atmospheres[]`, painted at world setup over the
    /// area's placed bounds grown up and down as far as the client's biome
    /// blend reads. The volume is the placement's, never typed.
    /// Absent: the horizon's biome.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atmosphere: Option<AtmosphereId>,
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
// Validation — the checks `dsl::validate` runs over this object (ADR-0031)
// ---------------------------------------------------------------------------

use std::collections::BTreeSet;

use crate::diagnostic::{Diagnostic, DwCode, ExitTier, codes};
use crate::envelope::Campaign;
use crate::ids::is_kebab;
use crate::registry::AnchorRegistry;

crate::dw_code! {
    /// Area binds neither or both of `prefab` / `prefab_pool` (exactly one
    /// required).
    pub const PREFAB_BINDING: DwCode = DwCode::new("DW0160", ExitTier::Build);
}

crate::dw_code! {
    /// Area `prefab_pool` references a pool absent from `prefabs/` metadata.
    pub const POOL_UNKNOWN: DwCode = DwCode::new("DW0161", ExitTier::Build);
}

crate::dw_code! {
    /// Area `prefab` names a piece absent from `prefabs/` metadata — the same
    /// obligation [`POOL_UNKNOWN`] carries on the other arm of the binding. It
    /// is an error rather than a deferral because an area whose piece is absent
    /// contributes no anchor set at all, so every per-area anchor proof over it
    /// is SKIPPED rather than failed: a misspelling here is strictly less
    /// checked than a correct name.
    pub const PREFAB_UNKNOWN: DwCode = DwCode::new("DW0856", ExitTier::Build);
}

crate::dw_code! {
    /// (v0.6) `horizon: "ocean"` declared without a `boundary` (spec-0013):
    /// validation-tier (exit 1). An infinite swimmable sea with no return rule is
    /// an authoring error. Grouped in the DW032x world/region family by domain;
    /// unlike the compiler-tier DW030x geometry codes it is raised at DSL
    /// validation, so it exits 1.
    pub const OCEAN_NO_BOUNDARY: DwCode = DwCode::new("DW0320", ExitTier::Build);
}

crate::dw_code! {
    /// (v0.6) `boundary.margin` outside the `0..=64` range (spec-0013):
    /// validation-tier (exit 1).
    pub const BOUNDARY_MARGIN: DwCode = DwCode::new("DW0321", ExitTier::Build);
}

crate::dw_code! {
    /// A stage-1 `horizon` param is out of range, or is a param of a base other
    /// than the one declared (spec-0026): validation-tier (exit 1).
    pub const HORIZON_PARAM: DwCode = DwCode::new("DW0853", ExitTier::Build);
}

crate::dw_code! {
    /// A `horizon` whose base BUILDS terrain, on a campaign that states no
    /// extent for that terrain to stand around (spec-0026): validation-tier
    /// (exit 1).
    ///
    /// A surround rings a declared extent — a site plan's `region`. A campaign
    /// that seats its pieces with `areas[]` declares none, and the union of
    /// whatever gets placed is not a substitute: it is an artifact of the
    /// compiler's fixed area stride, mostly the void between areas, so ringing
    /// it builds a mountain range around empty space.
    pub const SURROUND_NO_REGION: DwCode = DwCode::new("DW0855", ExitTier::Build);
}

crate::dw_code! {
    /// (v0.6, spec-0018) `world.min_players` outside the `1..=4` range. A delve is
    /// played by ONE party of 1–4 (ADR/CLAUDE.md product definition), so a declared
    /// mandatory party size can never sit outside it. Validation-tier (exit 1).
    pub const PARTY_SIZE: DwCode = DwCode::new("DW0356", ExitTier::Build);
}

crate::dw_code! {
    /// (spec-0077 §7) **A respawn wait that cannot be honoured.**
    /// `world.respawn_wait.seconds` lies outside `1..=120`, or the campaign
    /// declares a `respawn_wait` and no `set-checkpoint` or `bonfire` for a
    /// fallen player to come back to (the wait hangs off the checkpoint respawn
    /// edge, so with none it is a silently dead declaration). One rule about what
    /// a wait needs, two ways to break it. Validation-tier (exit 1). The build's
    /// own self-check that a shipped selector cannot read a waiter is `DW0926`.
    /// Prescription: a value in `1..=120`, or a checkpoint, or drop the field.
    pub const RESPAWN_WAIT_INVALID: DwCode = DwCode::new("DW0925", ExitTier::Build);
}

crate::dw_code! {
    /// (v0.6) `world.difficulty` is `peaceful`. On
    /// peaceful the server discards every hostile-category mob as it is ticked —
    /// `/summon`ed, `NoAI`, `PersistenceRequired`, all of it — so a peaceful delve
    /// is one in which every wave, every hostile actor and every ambush silently
    /// ceases to exist. There is no delve that wants that, so the keyword is
    /// refused rather than honoured. Validation-tier (exit 1).
    pub const DIFFICULTY_INVALID: DwCode = DwCode::new("DW0468", ExitTier::Build);
}

crate::dw_code! {
    /// (spec-0084 §6.1) A `world.textures[]` row's `replaces` names a texture the
    /// pinned client does not ship — not the `minecraft` namespace, a path with
    /// `textures/` or `.png` left on, another version's path, a misspelling — or
    /// two rows replace one texture. Judged against the census vendored from the
    /// pinned client jar (`delvec::compiler::textures`); the duplicate half is
    /// judged in validation, where no census is needed.
    pub const TEXTURE_PATH: DwCode = DwCode::new("DW0939", ExitTier::Build);
}

/// **How far this campaign is from being one piece**, in a clause — the half of
/// `DW0855` that tells a creator which of the three moves is one step away.
///
/// It names the count it read, so a reader can see what the refusal counted
/// rather than being told a category.
fn one_piece_gap(c: &Campaign) -> String {
    let areas = &c.world.content.areas;
    match areas.len() {
        0 => ", and no area is declared at all".to_string(),
        1 => format!(
            ", and its one area `{id}` draws from a pool rather than binding a single `prefab`",
            id = areas[0].id.as_str(),
        ),
        n => format!(", which is {n} areas rather than one"),
    }
}

/// The spec-0026 **horizon library**: a declared horizon's params are
/// range-checked here, and a param that belongs to another base is refused.
pub(crate) fn horizon_param_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    use crate::{HorizonBase, horizon_defaults};

    let Some(h) = c.world.content.horizon.as_ref() else {
        return;
    };

    let r = h.resolved();

    // Params foreign to the declared base. The wire shape is flat — one schema
    // rather than one per base — so this is where a param finds out it is not
    // for the base beside it. Silently ignoring it is the worse answer: an
    // author who wrote `rim_height` on an `ocean` believes something is being
    // read.
    if let crate::Horizon::Spec(spec) = h {
        let mut foreign: Vec<&str> = Vec::new();
        if !matches!(r.base, HorizonBase::Valley) {
            if spec.ratio.is_some() {
                foreign.push("ratio");
            }
            if spec.rim_height.is_some() {
                foreign.push("rim_height");
            }
        }
        for name in foreign {
            d.push(Diagnostic::error(
                HORIZON_PARAM,
                "world",
                format!("/content/horizon/{name}"),
                format!(
                    "`{name}` is a `valley` param and this horizon declares base `{base}`, which \
                     reads nothing from it. Remove it, or declare `base: \"valley\"` — a param \
                     nothing reads is a statement the author believes is taking effect.",
                    base = r.base.token()
                ),
            ));
        }
    }

    // A base that BUILDS terrain needs a map to build it around, and whether
    // this campaign states one is `crate::placement::Extent`'s answer — the same
    // one `compiler::plan::surround_rect` derives the rectangle from, so the
    // tier that refuses and the tier that builds cannot disagree about which
    // campaigns have an extent. Refused here rather than at the build because it
    // is a fact about the documents: nothing has to be placed to know that
    // nothing states an extent.
    if r.base.has_surround() && !crate::placement::Extent::of(c).is_stated() {
        d.push(Diagnostic::error(
            SURROUND_NO_REGION,
            "world",
            "/content/horizon/base",
            format!(
                "`horizon` base `{base}` builds terrain around the map, and this campaign never \
                 says how big the map is. A surround rings a DECLARED extent, and this campaign \
                 declares none: it places {n} area(s) with `areas[]`{how}. The union of whatever \
                 those place is not a substitute — areas sit on the compiler's fixed stride with \
                 void between them, and a pool's footprint is whatever the solver drew — so that \
                 union is mostly nothing and the horizon would be a mountain range built around \
                 empty space. There are three moves and all three are reachable from here: make \
                 the map ONE PIECE — a single area bound to a single `prefab`, whose own declared \
                 region is then the map's extent, which is how a site (a building with its \
                 island, its moat and its banks in one box) is placed; or give the campaign a \
                 site plan and declare `areas` empty, which is the same choice `DW0839` asks for; \
                 or set `horizon` to `void` or `ocean`, which need no map to be a horizon of.",
                base = r.base.token(),
                n = c.world.content.areas.len(),
                how = one_piece_gap(c),
            ),
        ));
    }

    // Ranges. Checked on the RESOLVED view so a shorthand is judged by the same
    // rule as the object form it desugars to.
    if r.base.has_surround() {
        if !(horizon_defaults::RATIO_MIN..=horizon_defaults::RATIO_MAX).contains(&r.ratio)
            || !r.ratio.is_finite()
        {
            d.push(Diagnostic::error(
                HORIZON_PARAM,
                "world",
                "/content/horizon/ratio",
                format!(
                    "`ratio` = {} is out of range — set it within {}..={} ({} is the default). \
                     It is the surround's total footprint as a multiple of the \
                     map's: under {} there is no room for a gap floor and a slope run \
                     both, and over {} the surround is mostly terrain no body reaches, at a cost \
                     that is all shipped bytes.",
                    r.ratio,
                    horizon_defaults::RATIO_MIN,
                    horizon_defaults::RATIO_MAX,
                    horizon_defaults::RATIO,
                    horizon_defaults::RATIO_MIN,
                    horizon_defaults::RATIO_MAX,
                ),
            ));
        }
        if !(horizon_defaults::RIM_HEIGHT_MIN..=horizon_defaults::RIM_HEIGHT_MAX)
            .contains(&r.rim_height)
        {
            d.push(Diagnostic::error(
                HORIZON_PARAM,
                "world",
                "/content/horizon/rim_height",
                format!(
                    "`rim_height` = {} is out of range — set it within {}..={} ({} is the \
                     default). It is the crest's height over the gap floor: under {} \
                     the rim does not close the horizon from a body standing on that floor, and \
                     over {} the surround stops fitting under whatever the map puts above it.",
                    r.rim_height,
                    horizon_defaults::RIM_HEIGHT_MIN,
                    horizon_defaults::RIM_HEIGHT_MAX,
                    horizon_defaults::RIM_HEIGHT,
                    horizon_defaults::RIM_HEIGHT_MIN,
                    horizon_defaults::RIM_HEIGHT_MAX,
                ),
            ));
        }
    }
}

/// spec-0084: the half of a `world.textures[]` row that needs neither the
/// pinned client's census nor the campaign's files — the id (`DW0190`, the rule
/// a skin's `texture_id` already has), one row per replaced texture (`DW0939`),
/// and the licence (`DW0741`). The census half (`DW0939`, `DW0940`) and the file
/// (`DW0309`) are judged where the files are read, `delvec::compiler::textures`.
fn texture_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let mut ids: BTreeSet<&str> = BTreeSet::new();
    let mut replaced: BTreeMap<&str, usize> = BTreeMap::new();
    for (i, t) in c.world.content.textures.iter().enumerate() {
        if !is_kebab(&t.id) {
            d.push(Diagnostic::error(
                codes::SKIN_INVALID,
                "world",
                format!("/content/textures/{i}/id"),
                format!(
                    "texture `id` `{}` is malformed — it must be a bare kebab token (e.g. \
                     `red-moon`), matching the `textures/<id>.png` filename",
                    t.id
                ),
            ));
        } else if !ids.insert(t.id.as_str()) {
            d.push(Diagnostic::error(
                codes::SKIN_INVALID,
                "world",
                format!("/content/textures/{i}/id"),
                format!(
                    "duplicate texture `id` `{}` — each row names its own image; rename one \
                     (and its `textures/<id>.png`)",
                    t.id
                ),
            ));
        }
        if let Some(first) = replaced.insert(t.replaces.as_str(), i) {
            d.push(Diagnostic::error(
                TEXTURE_PATH,
                "world",
                format!("/content/textures/{i}/replaces"),
                format!(
                    "texture `{}` replaces `{}`, which `world.textures[{first}]` already \
                     replaces — a texture is drawn one way, so remove one of the two rows",
                    t.id, t.replaces
                ),
            ));
        }
        for reason in crate::license::image_license_refusals(&t.license) {
            d.push(Diagnostic::error(
                codes::LICENSE_REFUSED,
                "world",
                format!("/content/textures/{i}/license"),
                format!("texture `{}` (replaces `{}`): {reason}", t.id, t.replaces),
            ));
        }
    }
}

/// Stage-1 `horizon`/`boundary` validation (spec-0013), the party size
/// (spec-0018), the declared difficulty and the declared textures (spec-0084).
pub(crate) fn world_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    texture_checks(c, d);
    // spec-0091: the declared view distance's range, and every site-plan line
    // of sight judged against the radius it serves. The binding it states is
    // printed by the CLI, which asks for it again without the diagnostics.
    crate::viewdistance::checks(c, d);
    // spec-0018: a delve is played by ONE party of 1–4, so a declared
    // mandatory size outside that range can never be honoured.
    if let Some(n) = c.world.content.min_players
        && !(1..=4).contains(&n)
    {
        d.push(Diagnostic::error(
            PARTY_SIZE,
            "world",
            "/content/min_players".to_string(),
            format!(
                "`min_players` = {n} is out of range — a delve is played by one party of 1–4, \
                 so set it to a value in 1..=4 (absent = 1, a party of one)"
            ),
        ));
    }
    // spec-0077 §7: a respawn wait is `1..=120` seconds, and it hangs off the
    // checkpoint respawn edge, so it needs a checkpoint or bonfire to exist.
    if let Some(w) = c.world.content.respawn_wait {
        if !(1..=120).contains(&w.seconds) {
            d.push(Diagnostic::error(
                RESPAWN_WAIT_INVALID,
                "world",
                "/content/respawn_wait/seconds".to_string(),
                format!(
                    "`respawn_wait.seconds` = {} is out of range — a fallen player waits 1 to 120 \
                     seconds, so set it to a value in 1..=120, or drop `respawn_wait` for no wait",
                    w.seconds
                ),
            ));
        }
        if !crate::declares_checkpoint(c) {
            d.push(Diagnostic::error(
                RESPAWN_WAIT_INVALID,
                "world",
                "/content/respawn_wait".to_string(),
                "`respawn_wait` is declared but this campaign declares no `set-checkpoint` or \
                 `bonfire` — the wait begins on the checkpoint respawn edge, so with nothing to \
                 come back to it never runs. Add the checkpoint or bonfire a fallen player \
                 returns to, or drop `respawn_wait`."
                    .to_string(),
            ));
        }
    }
    // Declared combat difficulty. `peaceful` is the
    // one keyword the compiler refuses: on peaceful the server discards every
    // hostile-category mob as it ticks it — summoned, `NoAI` and
    // `PersistenceRequired` are all irrelevant — so a peaceful delve is one in
    // which the entire cast of threats quietly does not exist.
    if matches!(
        c.world.content.difficulty,
        Some(crate::WorldDifficulty::Peaceful)
    ) {
        d.push(Diagnostic::error(
            DIFFICULTY_INVALID,
            "world",
            "/content/difficulty".to_string(),
            "`difficulty: \"peaceful\"` is refused: on peaceful the server discards every \
             hostile-category mob as it ticks it — being `/summon`ed, `NoAI` or \
             `PersistenceRequired` does not save one — so every wave, hostile actor and \
             ambush in this campaign would silently cease to exist. Declare `easy`, `normal` \
             or `hard`; for a delve that is genuinely combat-free, simply omit `difficulty` \
             (a campaign that fields no wave and stages no body peaceful discards ships \
             peaceful by derivation)"
                .to_string(),
        ));
    }
    // A horizon whose ambient a body can ENTER needs a return rule. The
    // question is the ambient's, never the base's name: an ocean is an
    // infinite swimmable sea, and a valley's gap floor is walkable ground
    // that runs to the foot of the rim. `void` is the only base a body
    // cannot enter, because there is nothing out there to stand on.
    let entered_base = match crate::horizon_base(&c.world.content.horizon) {
        crate::HorizonBase::Void => None,
        crate::HorizonBase::Ocean => Some((
            "ocean",
            "an infinite swimmable sea with no return rule lets players wander off the map",
        )),
        crate::HorizonBase::Valley => Some((
            "valley",
            "the gap floor between the map and the rim is walkable ground, and with no \
             return rule a player who steps off the map is simply outside it",
        )),
    };
    if let Some((base, why)) = entered_base
        && c.world.content.boundary.is_none()
    {
        d.push(Diagnostic::error(
            OCEAN_NO_BOUNDARY,
            "world",
            "/content/horizon".to_string(),
            format!(
                "`horizon` base `{base}` needs a `boundary` — {why}. Add a `boundary` (a \
                 bare `{{}}` uses the default margin), or set `horizon` to `void`"
            ),
        ));
    }
    // `margin` range check (0..=64).
    if let Some(b) = &c.world.content.boundary
        && !(0..=64).contains(&b.margin)
    {
        d.push(Diagnostic::error(
            BOUNDARY_MARGIN,
            "world",
            "/content/boundary/margin".to_string(),
            format!(
                "`boundary.margin` = {} is out of range — set it to a value in 0..=64 (16 is \
                 the default)",
                b.margin
            ),
        ));
    }
}

/// Per-area `lighting` (spec-0010): `min_light` is range-checked (1..=14,
/// `DW0196`).
pub(crate) fn lighting_range_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    // Range-check min_light (1..=14) where a lighting block is declared.
    for (i, area) in c.world.content.areas.iter().enumerate() {
        if let Some(lighting) = &area.lighting
            && !(1..=14).contains(&lighting.min_light)
        {
            d.push(Diagnostic::error(
                codes::LIGHTING_RANGE,
                "world",
                format!("/content/areas/{i}/lighting/min_light"),
                format!(
                    "area `{}` `lighting.min_light` = {} is out of range — set it to a value \
                     in 1..=14 (7 is the default)",
                    area.id, lighting.min_light
                ),
            ));
        }
    }
}

pub(crate) fn prefab_binding(c: &Campaign, anchors: &dyn AnchorRegistry, d: &mut Vec<Diagnostic>) {
    for (i, a) in c.world.content.areas.iter().enumerate() {
        // Exactly one of `prefab` / `prefab_pool`.
        match (&a.prefab, &a.prefab_pool) {
            (Some(_), Some(_)) => d.push(Diagnostic::error(
                PREFAB_BINDING,
                "world",
                format!("/content/areas/{i}"),
                format!(
                    "area `{}` binds both `prefab` and `prefab_pool`; bind exactly one",
                    a.id
                ),
            )),
            (None, None) => d.push(Diagnostic::error(
                PREFAB_BINDING,
                "world",
                format!("/content/areas/{i}"),
                format!(
                    "area `{}` binds neither `prefab` nor `prefab_pool`; bind exactly one",
                    a.id
                ),
            )),
            _ => {}
        }
        // A bound PIECE must resolve against the prefab-metadata surface, on
        // exactly the terms the pool arm below already demands. The asymmetry
        // this replaces was not a missing message — it was a missing message
        // that TOOK A PROOF WITH IT. An area whose prefab the registry does not
        // hold contributes no set to [`AnchorProviders`], and every per-area
        // anchor check reads a missing set as *defer to the compiler* and
        // skips. So one mistyped character in `world.json` turned seven
        // `DW0142` refusals into silence on the gallery, and left the campaign
        // green in a way that is strictly less checked than a correct name —
        // the unbound vacuity mode, one keystroke away.
        //
        // `has_prefab` is asked rather than `anchors_for` because only the
        // first distinguishes *the library does not hold this* from *this
        // registry cannot say*: a subset registry answers `None` and nothing is
        // refused on its word.
        if let Some(prefab) = &a.prefab
            && prefab.is_valid_syntax()
            && anchors.has_prefab(prefab) == Some(false)
        {
            d.push(Diagnostic::error(
                PREFAB_UNKNOWN,
                "world",
                format!("/content/areas/{i}/prefab"),
                format!(
                    "area `{}` binds `prefab` `{prefab}`, which is not declared in the prefab \
                     metadata — bind a piece that exists in the prefabs dir, or add `{prefab}` \
                     to the prefab library. This is a prefab-library/naming issue, not a \
                     quest-logic one. It is refused rather than deferred because an area whose \
                     piece is absent declares NO anchors, so every anchor a quest in this area \
                     names would be accepted without being examined — a misspelling here \
                     switches the anchor proof (`DW0142`) off for the whole area instead of \
                     failing it",
                    a.id
                ),
            ));
        }
        // A bound pool must resolve against the prefab-metadata surface.
        if let Some(pool) = &a.prefab_pool
            && pool.is_valid_syntax()
            && !anchors.has_pool(pool)
        {
            d.push(Diagnostic::error(
                POOL_UNKNOWN,
                "world",
                format!("/content/areas/{i}/prefab_pool"),
                format!(
                    "area `prefab_pool` `{pool}` is not declared in the prefab metadata — bind a \
                     pool that exists in the prefabs dir, or add `{pool}` to the prefab library. \
                     This is a prefab-library/naming issue, not a quest-logic one"
                ),
            ));
        }
    }
}

/// **Every area id this campaign declares** — `world.areas[]`, plus the one
/// place a site plan lays out ([`crate::siteplan::SITE_AREA`]).
///
/// A site-plan campaign has no `areas[]` — `DW0839` refuses one that does —
/// and exactly one place instead: the site the plan lays out. NPCs and
/// planned quests name it like any other area, so it is a declared area id
/// here for the same reason `areas[]` entries are.
pub(crate) fn declared_area_ids(c: &Campaign) -> BTreeSet<&str> {
    let mut area_ids: BTreeSet<&str> = c
        .world
        .content
        .areas
        .iter()
        .map(|a| a.id.as_str())
        .collect();
    if c.site_plan.is_some() {
        area_ids.insert(crate::siteplan::SITE_AREA);
    }
    area_ids
}

/// `DW0110` over the world's ids: each area's id and the prefab or pool it
/// binds, and each atmosphere's id.
pub(crate) fn world_id_syntax(c: &Campaign, d: &mut Vec<Diagnostic>) {
    for (i, a) in c.world.content.areas.iter().enumerate() {
        crate::ids::id_syntax!(d, a.id, "world", format!("/content/areas/{i}/id"));
        if let Some(prefab) = &a.prefab {
            crate::ids::id_syntax!(d, prefab, "world", format!("/content/areas/{i}/prefab"));
        }
        if let Some(pool) = &a.prefab_pool {
            crate::ids::id_syntax!(d, pool, "world", format!("/content/areas/{i}/prefab_pool"));
        }
    }
    for (i, a) in c.world.content.atmospheres.iter().enumerate() {
        crate::ids::id_syntax!(d, a.id, "world", format!("/content/atmospheres/{i}/id"));
    }
}

/// `DW0111` over the area ids.
pub(crate) fn world_id_uniqueness(c: &Campaign, d: &mut Vec<Diagnostic>) {
    crate::ids::dup_check(
        c.world
            .content
            .areas
            .iter()
            .enumerate()
            .map(|(i, a)| (a.id.as_str(), format!("/content/areas/{i}/id"))),
        "world",
        "area",
        d,
    );
}

/// `DW0112` over every atmosphere reference (spec-0080): an atmosphere is named
/// by a place for its first tick and by a `set-atmosphere` for a repaint, and a
/// repaint's `place` names an area or a site-plan box. Each is the plain
/// unresolved-reference shape.
pub(crate) fn atmosphere_dangling_refs(c: &Campaign, d: &mut Vec<Diagnostic>) {
    use crate::ids::dangling;
    let atmosphere_ids: BTreeSet<&str> = c
        .world
        .content
        .atmospheres
        .iter()
        .map(|a| a.id.as_str())
        .collect();
    let atmosphere_remedy = |id: &str| {
        format!(
            "unknown atmosphere `{id}` — declare it in `world.atmospheres[]` or correct the \
             reference"
        )
    };
    for (i, a) in c.world.content.areas.iter().enumerate() {
        if let Some(id) = &a.atmosphere {
            dangling(
                d,
                atmosphere_ids.contains(id.as_str()),
                "world",
                format!("/content/areas/{i}/atmosphere"),
                atmosphere_remedy(id.as_str()),
            );
        }
    }
    let mut place_ids: BTreeSet<&str> = c
        .world
        .content
        .areas
        .iter()
        .map(|a| a.id.as_str())
        .collect();
    if let Some(sp) = &c.site_plan {
        for (i, b) in sp.content.boxes.iter().enumerate() {
            place_ids.insert(b.node.as_str());
            if let Some(id) = &b.atmosphere {
                dangling(
                    d,
                    atmosphere_ids.contains(id.as_str()),
                    "site-plan",
                    format!("/content/boxes/{i}/atmosphere"),
                    atmosphere_remedy(id.as_str()),
                );
            }
        }
    }
    crate::for_each_campaign_effect(c, &mut |path, site, e| {
        let crate::Verb::SetAtmosphere {
            atmosphere, place, ..
        } = &e.verb
        else {
            return;
        };
        let stage = match site {
            crate::EffectSite::DialogueRespawn { .. } => "dialogue",
            _ => "quests",
        };
        if let Some(id) = atmosphere {
            dangling(
                d,
                atmosphere_ids.contains(id.as_str()),
                stage,
                format!("{path}/atmosphere"),
                atmosphere_remedy(id.as_str()),
            );
        }
        if let Some(place) = place {
            dangling(
                d,
                place_ids.contains(place.as_str()),
                stage,
                format!("{path}/place"),
                format!(
                    "`set-atmosphere` repaints unknown place `{place}` — name an `area/…` from \
                     `world.areas[]` or a site-plan box's `node/…`"
                ),
            );
        }
    });
}
