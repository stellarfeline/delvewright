# spec-0080: A place has its own sky — a campaign declares atmospheres, a place carries one from the first tick, and a story beat repaints a volume with another

- **Status**: Accepted
- **Ground**: written against engine `4c0fb85e` (`origin/main`), read only — `horizon::{ground_biome, vanilla_precipitates, VOID_BIOME_PRECIPITATES, VOID_BIOME_TAGS}` and `daylight::{biome_at, precipitates_at, Clock, hour_burns}` in `crates/delvec/src/compiler/`; the bootstrap `/fillbiome` pass and `emit_ground_biome` in `compiler/emit.rs`; `surround::BiomeRect`; `Plan::zone_box` and `AreaPlacement::bounds` in `compiler/plan.rs`; `Area`, `WorldContent`, `StealthZone`, `LethalVolume`, `Verb::{FillRegion, ClearRegion, SetWeather}` in `crates/dsl/src/stages.rs`; `SitePlanContent`, `PlanBox` in `crates/dsl/src/siteplan.rs`; `view::blockcolor::Deriver::with_biome`; `tools/ci/check-gallery-coverage.py`; `tools/ci/check-capability-ownership.py`; the pinned command tree `crates/delvec/data/commands-1.21.11.json` (`fillbiome`). The vanilla spike `tools/spike-eldritch-visuals/` (README, `gen.py`, `observations.json`) on branch `research/eldritch-visuals`. The pinned 1.21.11 data, read for this spec from the server jar `versions.toml` `[minecraft]` `server_jar_sha256` `f83b8e09…dd1726` (bundled `META-INF/versions/1.21.11/server-1.21.11.jar`) and the 1.21.11 client jar sha256 `1473c948…6cd3bd`.
- **What it is for**: a delve has one sky. `world.time` and `world.weather` are dimension-global, `set-time` and `set-weather` cut the whole world at once, and the only biome a campaign stands in is the one the horizon lays (`<ns>:void`, `minecraft:ocean`, or a valley band's `cherry_grove` / `windswept_forest`) — none of which a creator chooses for its look. So a corridor cannot open onto a place where the air is wrong: an olive sky, fog at arm's length, ash in the air, silence where the music was. The pinned game has the primitive: a biome carries environment attributes, and `/fillbiome` repaints a volume while the party stands in it. This spec exposes it first-class — declared on the campaign, carried by a place, moved by a beat — and makes "which biome is where" one answer for the build, every proof that reads it, and the party.
- **Research**: §2 is this spec's research record. Each rule is marked **cited** (a reading of the pinned jar or its data, a measurement in the spike, a constitution rule) or **authored** (this spec chooses). A fact read from the data and not run on a server says so.
- **Numbers**: no ADR. Three DW codes, allocated at implementation — **`DW_ATTR`** (an attribute line the pinned game does not accept here, §6.1), **`DW_PAINT`** (a paint that reaches cells it may not, §6.2), **`DW_DECL`** (an atmosphere the campaign declares and never stands in, or declares against itself, §6.3) — one code per rule, the shapes under each listed so the reader sees none needs a code of its own. A dangling `atmosphere/<id>` reference is the existing unresolved-reference shape of stage validation and takes no new code. `dsl_version` moves to the number the planner hands the implementation; it numbers a surface and promises nothing.
- **Non-goals**: moving the sun, moon or stars, or recolouring the sunrise, from a place (§2.3: the overworld day cycle overrides them; the vanilla remedies are a custom dimension or an edited timeline, both world-wide — a different spec); gameplay attributes (§2.4, decided out of scope with the reason); a worldgen paint smaller than a place (§3.3); an atmosphere on a valley surround band (the horizon's biomes stay the generator's, spec-0026); a Chunky sky or fog for an atmosphere (§5.3); a fade — `/fillbiome` is a hard cut and the client blends fog and not grass (§2.2), a measured fact and not a choice.

## 1. The defect, and the object it belongs to

**Cited** (the tree at `4c0fb85e`): three things decide which biome a cell stands in, and nothing a creator writes reaches any of them. `horizon::ground_biome` lays the generator's biome; `surround::BiomeRect` paints a valley's bands in `setup_finish`; `daylight::biome_at` reads the two in that order to answer `precipitates_at`, the fact `DW0496` turns on. The biome is vanilla's one channel for sky colour, fog, clouds, sky-light tint, stars, ambient particles, music and ambience, grass, foliage and water tint, and precipitation (`compiler.md`, *The horizon's surround*, **Biome**), and the engine already emits a datapack biome of its own (`<ns>:void`) and already drives `/fillbiome` — for one field and one flora each.

**The object an atmosphere belongs to is a place.** A sky is not a property of the beat that turns it wrong, nor of the whole world: the whole point is that *here* differs from *there*. The engine's place classes are the `areas[]` area (its placed bounds, which the night-vision mitigation already uses as *the area's volume*) and the site plan's box (one per graph node, `PlanBox.region`). The engine's one runtime volume class is the anchor-centred box `StealthZone`, shared by `fill-region`, `clear-region`, `damage-players`' `in`, `collapse`, `volley` and a lethal volume, and resolved through the single `Plan::zone_box` (`LethalVolume::region`'s own doc comment names a private twin as `check-capability-ownership.py` check C by construction — **cited**). So: an atmosphere is declared once on the campaign, named by a place for its first tick, and named by a volume when a beat repaints — and the repaint is a verb of the physical-edit family `fill-region` / `clear-region` already form, because it is a runtime edit of the world keyed to a volume (CLAUDE.md, *a capability belongs to the object class it acts on, not to the verb that first needed it* — **cited**).

## 2. Research record

### 2.1 `/fillbiome` reaches a connected client, and what the party sees

**Cited**, the spike (`tools/spike-eldritch-visuals/observations.json`, `fillbiome`): on the pinned server image (`versions.toml` `[images.base]`), mineflayer 4.37.1, after a zone threshold fired two `/fillbiome` commands over a 32×40×40 slab, the bot received 2 `chunk_biomes` packets naming all 6 of the 6 changed chunks and 0 `map_chunk` / 0 `unload_chunk` for those chunks; `execute if biome` read the new biome inside and the old one outside. Cross-checked in the 1.21.11 client bytecode through Mojang's official mappings (README, *Readings*): `handleChunksBiomes` replaces each chunk's biomes, clears the tint caches, and marks the 3×3 sections around each chunk dirty. Fog and sky are read from the biome at the camera every frame, so they change at once; grass, foliage and water are re-meshed. The owner walked the lab and approved the look of station 2.

### 2.2 Edges

**Cited** (README, *Expected strength and limits*): biome cells are 4×4×4, so the edge of a painted volume is blocky in grass tint and blended in fog, by the client's own biome-blend setting. A `/fillbiome` over a block range paints every 4-cell that range touches; the painted volume is the enclosing quart box, up to three blocks beyond each face.

### 2.3 What a biome can and cannot set in the overworld

**Cited**, read from the pinned server jar's data (`data/minecraft/timeline/day.json`, `moon.json`, `early_game.json`; `data/minecraft/dimension_type/overworld.json`, whose `timelines` is `#minecraft:in_overworld`), not run on a server. Environment attributes stack dimension < biome < timeline < weather (spike README, *What a biome cannot do*). The `minecraft:day` timeline (period 24000) carries 18 tracks; the modifier on each decides what survives from a biome:

| Track | Modifier | Keyframes | What a biome's value becomes |
|---|---|---|---|
| `visual/sky_color`, `visual/fog_color`, `visual/sky_light_color` | `multiply` | `#ffffff` 133–11867, dark (`#000000`, `#0f0f16`, `#000000`) 13670–22330 | survives, darkened at night |
| `visual/cloud_color` | `multiply` | −1 (white) by day, −15132378 by night | survives, darkened at night |
| `visual/sky_light_factor` | `multiply` | 1.0 730–11270, 0.24 13140–22860 | survives, darkened at night |
| `visual/star_brightness` | `maximum` | 0.0 by day, 0.5 at night (12 keys) | **a biome can force stars at noon** |
| `visual/sun_angle`, `visual/moon_angle`, `visual/star_angle` | override | 0→360 over the day | **a biome cannot move them** |
| `visual/sunrise_sunset_color` | override | 32 keys | cannot |
| `visual/moon_phase` (`minecraft:moon`) | override | 8 phases over 192000 | cannot |
| `gameplay/monsters_burn` | `or` | `false` at 12542, `true` at 23460 | **a biome can force burning, never prevent it** (§2.4) |
| `gameplay/sky_light_level` | `multiply` | 1.0 by day, 0.26666668 at night | survives, darkened |
| the rest (`firefly_bush_sounds`, `bees_stay_in_hive`, `creaking_active`, `eyeblossom_open`, `cat_waking_up_gift_chance`, `turtle_egg_hatch_chance`) | `or` / override / `maximum` | — | gameplay, §2.4 |

Everything a biome sets that no track names — fog distances, cloud height and fog distance, water fog, ambient particles, dripstone particle, ambient sounds, music, music volume — reaches the client as declared.

### 2.4 The attribute registry of the pinned game

**Cited**, measured for this spec by two instruments sharing no file: `strings` over every class of the pinned server jar's bundled `server-1.21.11.jar`, and the same over the 1.21.11 client jar, each filtered to `(visual|audio|gameplay)/[a-z_]+` and sorted unique. Both yield **54** ids and the two lists are identical: 4 `audio/`, 29 `gameplay/`, 21 `visual/`. A third reading — every `attributes` key of the 65 vanilla biomes and 3 dimension types plus every track of the 3 timelines — names 42 of the 54 and nothing outside them. Ids the wiki lists that are **not** in the jar (`ambient_light_color`, `block_light_tint`, `night_vision_color`, spike README) are confirmed absent here. The 20 this spec admits:

| Id | Shape, as vanilla data writes it | Timeline | Set by the lab's station 2 |
|---|---|---|---|
| `visual/sky_color`, `visual/fog_color`, `visual/water_fog_color`, `visual/sky_light_color` | `#rrggbb` | multiply / multiply / — / multiply | yes |
| `visual/cloud_color` | `#aarrggbb` (overworld `#ccffffff`) | multiply | yes |
| `visual/fog_start_distance`, `visual/fog_end_distance`, `visual/sky_fog_end_distance`, `visual/cloud_fog_end_distance`, `visual/water_fog_start_distance`, `visual/water_fog_end_distance` | float, blocks; a biome may also write `{"argument": 0.85, "modifier": "multiply"}` (`swamp`) | — | all but `water_fog_start_distance` |
| `visual/cloud_height` | float (overworld 192.33) | — | no |
| `visual/sky_light_factor` | float (nether 0.0) | multiply | yes (0.55) |
| `visual/star_brightness` | float | maximum | yes (0.9) |
| `visual/ambient_particles` | `[{particle: {type}, probability}]` | — | yes |
| `visual/default_dripstone_particle` | `{type}` (nether) | — | no |
| `audio/ambient_sounds` | `{loop?, mood?: {sound, tick_delay, block_search_extent, offset}, additions?: {sound, tick_chance}}` | — | yes |
| `audio/background_music` | `{default?: {sound, min_delay, max_delay, replace_current_music?}}`; `{}` silences (`pale_garden`) | — | yes |
| `audio/music_volume` | float (`pale_garden` 0.0) | — | yes |
| `audio/firefly_bush_sounds` | bool | or | no |

The five `visual/` ids the day cycle overrides (§2.3) are refused (§6.1). The 29 `gameplay/` ids are **out of scope, with the reason**: `monsters_burn` — the one the coverage map proposed as a creator-stated replacement for `DW0496`'s refusal — stacks under the day timeline by `or`, so a biome's `false` is `true` throughout the burn window and the remedy does not exist in the overworld; `sky_light_level` is read by the light model (`light::effective_sky`), `DW0920` (`engage::bright_outside`) and `DW0496`, each of which would owe a per-cell reading before the surface could be admitted honestly, and no campaign has asked; the other 27 act on objects a delve has none of (villagers, raids, bees, pandas, piglins, beds, respawn anchors, fishing). Admitting a gameplay attribute later is a spec of its own that names the proofs it reaches.

### 2.5 The biome a delve already ships

**Cited**: `horizon::void_biome_definition` is `minecraft:the_void` field for field from the pinned jar except `has_precipitation`; it carries empty spawners, no carvers, one feature, temperature 0.5, downfall 0.5, and joins `#minecraft:without_wandering_trader_spawns`. Biomes load only when a world opens; `/reload` does not add them (spike README, *Play it*, 1), and the engine's datapack is in `datapacks/` before first boot on every boot path (`compiler.md`, *World / build output*). The spike's own biomes loaded on the pinned server with no load error (`observations.json`, `datapack_list`; README, *What was verified*).

## 3. The surface

### 3.1 `world.atmospheres[]` — declared once, on the campaign

**Authored.** Stage 1 (`world.json`) gains an optional list beside `time`, `weather` and `horizon`, which are the other statements about the sky the party stands under:

```json
"atmospheres": [
  {
    "id": "atmosphere/wrong-place",
    "attributes": {
      "visual/sky_color": "#3b4a1e",
      "visual/fog_color": "#56602f",
      "visual/fog_start_distance": 1.0,
      "visual/fog_end_distance": 26.0,
      "visual/star_brightness": 0.9,
      "visual/ambient_particles": [{ "particle": { "type": "minecraft:ash" }, "probability": 0.02 }],
      "audio/background_music": {},
      "audio/music_volume": 0.0
    },
    "tint": { "grass": "#6b6a2a", "foliage": "#5a4a2a", "dry_foliage": "#4a3a2a", "water": "#1a0f1f" },
    "precipitation": "none"
  }
]
```

1. **`id`** — `atmosphere/<kebab>`, unique, the prefix convention of `lethal/`, `volume/`.
2. **`attributes`** — a map from an admitted attribute id (§2.4; the `minecraft:` prefix is optional, as it is on `play-sound`'s sound id) to a value in that attribute's shape. The ids and shapes are **data** vendored from the pinned jar into `crates/delvec/data/environment-attributes-1.21.11.json` with its provenance in `data/PROVENANCE.md` (the two-jar reading of §2.4, the class each shape was read from), regenerated by a `tools/maintenance/` script that refuses when the two jars disagree — the same shape as the entity-tag table `DW0496` reads. Vanilla registry values are data, never units (spec-0039 — **cited**), so the one DSL unit here is the map. A value may be the modifier object form vanilla biomes use (§2.4, `swamp`).
3. **`tint`** — optional, each member optional: the biome `effects` keys the pinned data writes (`grass_color`, `foliage_color`, `dry_foliage_color`, `water_color`), `#rrggbb`. Absent, vanilla derives grass and foliage from the climate's colormap index and water is the void's `#3f76e4`.
4. **`precipitation`** — `none` | `rain` | `snow`, required. One judgement from which the compiler derives the three vanilla fields that must agree (`has_precipitation`, `temperature`, `downfall`): `rain` → `true`, 0.5, 0.5 (the engine's void biome); `snow` → `true`, 0.0, 0.5 (vanilla snows below 0.15 — **cited**, `horizon::void_biome_definition`'s own note); `none` → `false`, 0.5, 0.5. An optional **`climate: {temperature, downfall}`** overrides the derived pair for a creator who wants vanilla's own grass colormap at a named point; a `climate` that contradicts the `precipitation` (snow at or above 0.15, rain below it) is `DW_DECL` (§6.3). This is the field `DW0496` reads at a cell (§4.2): a creator who declares `none` under a rainy world has said the undead burn there, and the proof holds them to it.

### 3.2 A place carries one from the first tick

**Authored.** `Area` and `PlanBox` each gain `atmosphere: Option<AtmosphereId>`: the place's volume is its own bounds (`AreaPlacement::bounds`, `PlanBox.region`), handed by the tool and typed by nobody. Both structs are the engine's *place with a world box* class; the same optional field on each is one capability on one class, not a second bespoke field (`check-capability-ownership.py`, shape 1, read in its own words). A place with no `atmosphere` stands in the horizon's biome, exactly as today.

### 3.3 A beat repaints a volume — `set-atmosphere`

**Authored.** One verb in the physical-edit family:

```json
{ "type": "set-atmosphere", "atmosphere": "atmosphere/wrong-place", "region": { "anchor": "hall/threshold", "extent": [12, 8, 20] } }
{ "type": "set-atmosphere", "atmosphere": "atmosphere/wrong-place", "place": "area/hall" }
```

Exactly one of `region` (a `StealthZone`, resolved through `Plan::zone_box` — the one authority) or `place` (an area or box id, resolved to the place's bounds — the same function §3.2 reads). Two spellings because they are two kinds of input (CLAUDE.md, *every input is a judgement or a derivation* — **cited**): a volume inside a place is the creator's judgement (the lab's station 2 is a slab beyond a threshold), while a whole place's bounds are a derivation the creator must never type. Neither nor both is the existing `DW0160`/`DW0161` exclusivity shape under `DW_PAINT`. Repainting back is this verb naming the place's declared atmosphere, or `atmosphere: null` for the horizon's biome — no second verb. The verb carries `when` like every `QuestEffect` and fires from every effect root (an approach trigger, an objective, a dialogue option, a trap payload), which is what makes *the trigger is a story stage the player reaches, never time* expressible without a clock. A worldgen paint smaller than a place is deliberately not a surface: it is this verb fired from the campaign's first beat, and the tool reports it as such.

## 4. One authority for which biome is where

### 4.1 The biome map

**Authored, on cited parts.** `compiler::horizon` — already *the one place a campaign's horizon becomes declared physical facts* (its module doc) — gains `biome_map(plan) -> BiomeMap`: an ordered list of paints `(quart-aligned box, biome id, precipitates)` over the ground biome. Order: the valley's `BiomeRect`s (outside the map by construction, spec-0026), then every place carrying an atmosphere in declaration order, then the ground. `daylight::biome_at` is deleted and every reader asks the map; `emit`'s bootstrap `/fillbiome` pass iterates the map's paints and nothing else; `horizon::vanilla_precipitates` answers a vanilla id and the map answers a declared one, through one function. Two place paints whose quart boxes intersect with different atmospheres are `DW_PAINT` — the biome cell would belong to whichever command ran last, which is an order and not a declaration.

### 4.2 The proofs that read it

**Authored.** `precipitates_at(cell)` reads the map, so `DW0496` fires for a burning body staged in an atmosphere with `precipitation: none` under a rainy dusk, and stays quiet where it rains. A `set-atmosphere` is a cut of the same kind as `set-time` and `set-weather`: for the span `daylight::Clock` already computes, every repaint whose volume reaches a fight's cells adds its atmosphere to the set of states that fight can stand in, placed in the DAG the way a weather cut is, and a repaint with no DAG place (a trigger, a trap, a dialogue option) is met by every fight — which can only over-report (`compiler.md`, `DW0496`, *When the fight happens* — **cited**). Nothing else reads the biome today (`light::effective_sky` is keyed by weather and the pinned skyDarken model, not by biome), and §2.4 keeps it so.

### 4.3 What is emitted

**Authored.** Per atmosphere, `datapack/data/<ns>/worldgen/biome/atmosphere/<kebab>.json` — biome id `<ns>:atmosphere/<kebab>` — built from the engine's void definition (§2.5) with `features: []` (a painted biome never generates), the derived climate and precipitation, the `tint` as `effects`, and the `attributes` map with each id prefixed `minecraft:`; it joins `VOID_BIOME_TAGS`. Every declared atmosphere is emitted whether a place carries it or only a beat paints it, because the registry closes when the world opens (§2.5). The bootstrap pass paints each place's quart box in `setup_finish` where the surround's bands are painted today, under the same cap raise-and-restore, through one shared `fillbiome_lines(box, biome)` that the verb's emission also calls — a shared rule is extracted, never copied (CLAUDE.md — **cited**). Every emitted `fillbiome` is checked against the pinned command tree by the emitter as every command is.

**Determinism** (ADR-0006 — **cited**): `attributes` is a `BTreeMap`; colours are lower-cased on parse; floats round-trip through `f64` and serde's shortest representation; `put_json` writes sorted keys; the paint order is declaration order; no cell's biome depends on anything but the documents and the placed bounds. Two builds of one campaign are byte-identical, asserted by the existing double-build gate over a gallery that declares an atmosphere.

## 5. Proving it reaches the party

### 5.1 The binding line

**Authored.** Every build prints `atmosphere binding: A declared; P of N place(s) carry one; R repaint effect(s) over V volume(s); Q quart cells painted at bootstrap of M in the map; B biome file(s) emitted`. `A` with `P + R = 0` is the vacuous shape and is refused (§6.3), not printed green (CLAUDE.md, *every validation artifact states its binding count with its denominator* — **cited**).

### 5.2 The runtime half

**Authored, on the spike's instrument.** PackTest asserts the server's own reading — `execute if biome <x> <y> <z> <ns>:atmosphere/<id>` at a cell inside each carried place and `execute unless` at a cell outside — and, for each repaint reachable on the critical path, the same pair before and after the beat. The mineflayer critical-path bot additionally asserts the client-reach measurement of §2.1 for each repaint it performs: a `chunk_biomes` packet naming every chunk of the painted quart box, and no `map_chunk` for them (`harness/`, reading through the shared rcon rejection rule). This is spec-0005's two layers over one fact: the server holds the biome, and the client was told.

### 5.3 Renders

**Authored.** The flat-shaded viewer derives tint from a biome's JSON (`Deriver::with_biome`, which already reads `data/<ns>/worldgen/biome/<id>.json`), so a scene whose cells stand in a carried atmosphere is derived under that atmosphere's biome — the map says which. A Chunky frame's sky and fog stay spec-0079's, keyed by hour and weather: mapping an attribute set onto Chunky's sky block would be a second look table nobody has judged, and the pinned core draws no biome fog. How the pinned Chunky tints grass under a biome id it has no table for is **unmeasured**; the implementation measures it once on a gallery frame and records the reading in `docs/reference/tools.md` §4a. The look of an atmosphere is confirmed where the spike says it can be — a client (README, *What was verified*, last row) — on the demo level (§7), never on a campaign's renders.

## 6. Refusals

**Authored**, each raised at the document it is entered in (`delvec validate`, not the end of a build), with the remedy named and a row in `remedy_reachability.rs`.

### 6.1 `DW_ATTR` — an attribute line the pinned game does not accept here

- an id not in the vendored registry (names the nearest id by edit distance, as the block registry does);
- an id the overworld day cycle overrides (§2.3: the five), with the message saying what the party would see instead and that the vanilla remedy is world-wide;
- a `gameplay/` id (§2.4), with the reason in one line;
- a value not in the id's shape (a float for a colour, a `#rrggbb` where `#aarrggbb` is read, a particle list without `probability`);
- a value outside the range the pinned codec bounds — the range is read from the codec at implementation and recorded in the table with its class; where the codec bounds nothing, the engine bounds nothing (CLAUDE.md, *researched, never invented* — **cited**);
- a sound id in `audio/*` that is not a pinned sound event, through the one registry `DW0326` reads; a particle type not in the pinned `particle_type` registry, vendored with provenance if none exists at implementation.

### 6.2 `DW_PAINT` — a paint that reaches cells it may not

- a `set-atmosphere` with neither or both of `region` / `place`;
- a repaint volume with a cell outside the map's declared extent — the rectangle `plan::surround_rect` reads, the one statement of extent a campaign has: `fillbiome` into an unloaded chunk is the same silent no-op `place template` is (`emit.rs`'s own note — **cited**);
- two carried places whose quart boxes intersect with different atmospheres (§4.1).

### 6.3 `DW_DECL` — an atmosphere declared against itself or against nothing

- an atmosphere no place carries and no beat paints (unbound);
- `climate` contradicting `precipitation` (§3.1.4);
- a duplicate `id`.

## 7. The gallery, and the demo level

**Authored**, against `gallery/` at `4c0fb85e` (two areas, `horizon` void, `day` + `clear`, `dsl_version` 0.35.0).

1. **Bound.** `world.json` declares two atmospheres: one setting **all 20** admitted attributes (one per shape, including a modifier-object distance), a full `tint`, `precipitation: snow` — carried by `area/hall`; one with `precipitation: none` and two attributes, painted by a `set-atmosphere` with a `region` from an approach trigger and painted back with `place` from an objective. Every unit — `atmospheres[].{id, attributes, tint.*, precipitation, climate}`, `areas[].atmosphere`, the verb's three fields — is written, and the coverage gate (`delvec schema --stage all`) binds them. The site-plan `boxes[].atmosphere` is bound in the gallery's site-plan overlay, which is where `PlanBox` is already written.
2. **Refusal-proven**, one probe each, the primary plus a declared `patch`: `a-sky-the-sun-does-not-obey` (`visual/sun_angle`) and `an-attribute-the-game-never-heard-of` → `DW_ATTR`; `a-colour-that-is-a-number` → `DW_ATTR`; `a-snow-at-summer-heat` → `DW_DECL`; `an-atmosphere-nobody-stands-in` → `DW_DECL`; `a-repaint-past-the-edge-of-the-world` → `DW_PAINT`; `a-repaint-that-names-both-a-box-and-a-place` → `DW_PAINT`.
3. **Perturbation acceptance.** Changing the hall's `visual/sky_color` moves a byte of the emitted biome file; changing `areas[].atmosphere` to the other id moves the bootstrap `fillbiome` line; removing the repaint effect removes its `fillbiome` lines and reds the binding line's `R`.
4. **Demo level**, `docs/demo-levels.md` row: **The Threshold** — one hall with a door at its middle, built from the spike's station 2: the near half is the horizon's day; beyond the door the world is wrong (the lab's block, §2.4 last column — olive sky, fog from 1 to 26 blocks, red clouds, stars at noon, ash, silenced music, soul-sand-valley ambience, dead grass, dry); a bell at the far end repaints the near half too, so the party walks back through the place they came from and it is no longer there. The level's walk is the four unmeasured attributes' first confirmation (`cloud_height`, `default_dripstone_particle`, `water_fog_start_distance`, `firefly_bush_sounds`) and the owner's look at the 4-cell edge.

## 8. Decisions for the owner

Only what she would see.

1. **The first look (§7.4).** Recommendation: the demo level adopts the lab's station-2 block verbatim as `atmosphere/wrong-place` — the one atmosphere anyone has walked — and the gallery's values are not looked at, because the gallery is never released. What she sees: the demo level is the lab's threshold, in a delve, with the way back repainted. The cost of deciding later: the engine lands with no atmosphere anyone has stood in, and the first campaign to declare one is the first look.

## Acceptance criteria

Each criterion is checked against the tree at `4c0fb85e`; none is satisfied there, and criterion 5's first sentence (the void biome and its precipitation flag are one fact) holds there and must keep holding.

1. **Registry.** `crates/delvec/data/environment-attributes-1.21.11.json` lists 54 ids with a `scope` of `admitted` (20), `overridden` (5) or `gameplay` (29), a shape per admitted id, and the range the pinned codec bounds or `null`; `data/PROVENANCE.md` names both jars by sha256 and the method; `tools/maintenance/` regenerates it and refuses when the two jars' lists differ; a `cargo test` asserts the three counts and that every admitted id's shape parses the value the spike's `wrong_place.json` carries for it.
2. **Surface.** `delvec schema --stage world` exports `atmospheres[]` with `id`, `attributes`, `tint`, `precipitation`, `climate`, and `areas[].atmosphere`; `--stage site-plan` exports `boxes[].atmosphere`; `--stage quests` exports `set-atmosphere` with `atmosphere`, `region`, `place`, `when`; `compiler.md`'s field lists hold to the structs in both directions by the existing field-list tests.
3. **Emission.** For a campaign declaring an atmosphere, `datapack/data/<ns>/worldgen/biome/atmosphere/<kebab>.json` exists with `features: []`, the derived `has_precipitation` / `temperature` / `downfall`, `effects` from `tint`, every attribute under its `minecraft:` id, and the tag file carries the id; `setup_finish` carries one `fillbiome` per carried place over its quart box; a `set-atmosphere` emits `fillbiome` lines over `Plan::zone_box` or the place's bounds through the same helper (`grep` finds one writer of `fillbiome` under `crates/delvec/src/`); every emitted command passes the pinned command tree.
4. **Determinism.** Two builds of the gallery are byte-identical (existing gate); a test perturbs an attribute's float spelling (`1.0` vs `1`) and asserts the emitted bytes do not move.
5. **One authority.** `daylight::biome_at` is gone; `horizon::biome_map` is the only function that answers a cell's biome, asserted by `grep` over `crates/delvec/src/` for readers of `surround.biome` and `ground_biome` (the map's constructor is the only one); a test stages a burning body in a `precipitation: none` atmosphere under `dusk` + `rain` and asserts `DW0496`, and the same body in a `rain` atmosphere and asserts none; a test adds a `set-atmosphere` to `none` over the fight's cells from a trigger and asserts `DW0496`.
6. **Refusals.** Each shape of §6.1–6.3 is refused with its code and remedy by `delvec validate` on a fixture (`crates/delvec/tests/`), with a `remedy_reachability.rs` row each; `tools/ci/check-dw-codes.py` is green with the three codes in `compiler.md`.
7. **Binding line.** `delvec build` prints §5.1's line on every build; on a campaign with no atmospheres it prints `0 declared; 0 of N place(s) carry one` — a measured zero; asserted in a CLI test.
8. **Runtime.** The PackTest templates assert `execute if biome` inside and `execute unless biome` outside each carried place and before/after each critical-path repaint; `harness/` asserts a `chunk_biomes` packet for every chunk of a performed repaint's quart box and no `map_chunk` for them; both bound by `tools/ci/check-live-commands.py`.
9. **Gallery** (spec-0039). §7.1's elements are bound and §7.2's seven probes refused with the codes named, with the coverage gate's counts on its own line; `gallery/baseline/` is regenerated after the merge commit exists with every moved row attributed.
10. **Renders.** `docs/reference/tools.md` §4a records one gallery frame of the hall under its atmosphere rendered by `validation/chunky.sh` on the pinned core, with the scene and frame hashes and one sentence on what the pinned core did with the biome id; the viewer palette for a scene inside the hall is derived under the hall's biome (`view_cli.rs`).
11. **Docs and skill, same change.** `compiler.md`: stage 1 gains `atmospheres[]` and `areas[].atmosphere`, the site-plan section `boxes[].atmosphere`, §3 the verb's emission, the diagnostics catalog the three codes, *World / build output* the biome files and the map's order; `docs/reference/tools.md` the registry regenerator; the `/new-delve` skill (`.claude/skills/delvewright/skills/new-delve/references/placement.md`, where the horizon is described, and `references/quest-capabilities.md`, the verb vocabulary) says an atmosphere is declared on the campaign, carried by a place and moved by `set-atmosphere`, and that the sun does not move; `docs/specs/README.md` gains this spec's row; `docs/demo-levels.md` gains §7.4's row; the coverage map's `monsters_burn` finding is corrected to §2.4's reading where that document lands.
