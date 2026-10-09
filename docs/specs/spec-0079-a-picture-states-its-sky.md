# spec-0079: A picture states its sky — a showcase camera renders under its picture's hour and weather, or under a sky it states, and the engine emits the whole of it

- **Status**: Accepted
- **Ground**: written against engine `19eba477` (`origin/main`), read only — `Camera` / `CameraSheet`, `Frame`, `world_scene` and `emit` in `crates/delvec/src/compiler/view/camera.rs`; `ChunkyScene`, `ChunkySun`, `Sky`, `sun_at`, `plan_sky` and `round6` in `crates/delvec/src/compiler/view/scene.rs`; `sky_fact` in `crates/delvec/src/compiler/render_plan.rs`; `effective_sky` in `crates/delvec/src/compiler/light.rs`; `Reference` in `crates/dsl/src/design.rs`; `WorldTime` and `WorldWeather` in `crates/dsl/src/stages.rs`; `ViewCommand::Cameras` and `::PlaceCamera` in `crates/delvec/src/compiler/view/cli.rs`; `tools/ci/check-gallery-coverage.py` with `tools/ci/gallery_units.py`; the gallery's `design.json` and `design/cameras.json`; `docs/reference/showcase-shots.md` §2b; and the pinned Chunky core, `versions.toml` `[render] chunky_revision` `156e2bba2526fe4cf218243704b6f3e6d0281d1a`, read at that revision.
- **What it is for**: a storybook picture of a delve declared at dusk in the rain is today rendered under a clear dusk sky, because `delvec cameras` emits the hour's sun and nothing else, and the renderer's own default sky is clear. The one set that got an overcast sky got it by patching the emitted scene file by hand, which the skill forbids in so many words (`references/hand-over.md`: *never hand-edit a scene JSON; a frame you cannot emit is a render gap to report*). This spec closes the gap at the record: the sky a picture is taken under is a property of that picture, stated once in the camera record or derived from the approved image the camera answers, and the engine emits every byte Chunky needs for it.
- **Research**: §2 is this spec's research record. Each rule below is marked **cited** (a Chunky source line at the pinned revision, a published source, a measurement in the tree, or a constitution rule) or **authored** (this spec chooses). A source read only through a search-result summary is marked *(summary only)*.
- **Numbers**: no ADR, no DW code — every refusal here is a rule of the camera record, raised under the record's own code (`DW0721`, as spec-0069 §7 did), and the refusal shapes are listed in §6 so the reader can see none needs a code of its own. `dsl_version` does not move: the camera record is not a stage document (spec-0069 §7), and `design.json` and `world.json` do not change shape.
- **Non-goals**: rain itself — drops, wet surfaces, lightning — which the pinned core cannot draw (§2.1) and this spec does not fake; a cloud layer (Chunky's `cloudsEnabled` draws Minecraft's cloud texture as geometry and was not researched as a shading device); a camera's `exposure`, which stays the camera's own judgement; the storybook caption rule for a frame taken under a sky other than the declared one (`showcase-shots.md` §2b.5 stands as written); the `--draft` help text, which already states the draft constants' own values.

## 1. The defect, and the object it belongs to

**Cited** (the tree at `19eba477`): `render-plan.json` states `sky` as `{time, daytime_ticks}` — the world's declared initial hour — and its module says *the weather is deliberately not here: Chunky has no rain, and a key a renderer cannot act on is the same unemitted shape one level along*. `camera::world_scene` turns that hour into `sun.altitude` / `sun.azimuth` and writes no `sky`, `fog` or other `sun` key, so every scene inherits the renderer's defaults: a simulated clear sky, sun intensity 1.25, no fog (§2.2). The design record (`design.json`, spec-0061) states per approved picture the `time` **and** `weather` it was drawn under, and every camera `answers` exactly one such row; the weather half of that row reaches nothing.

Two consequences follow, and both are visible in the gallery's own record. `rampart-against-the-sky` answers `concept/dusk-rampart`, drawn at `dusk` + `rain`: it renders under a clear dusk sky. `crypt-stair` answers `concept/midnight-crypt`, drawn at `midnight` + `thunder`: it renders under the **world's** hour (`day`), because the plan's `sky` is the initial hour and not the picture's. The second is the same defect as the first: the frame is of the picture, and the picture's sky is on its row.

**The object the sky belongs to is the picture, not the command.** A frame taken under a sky other than its row's is a creative judgement about that one frame (the record's own case: a chapel declared at dusk in the rain is shown at a clear noon because the declared sky leaves the room unreadable, §2b.5). So the sky is a field of the camera, recorded beside its `exposure`, and never a flag of `delvec cameras`: a flag is typed per run and recorded nowhere, so the frame it produced could not be re-asked for from the repository, which is what the record exists to make possible (**cited**: CLAUDE.md, *a capability belongs to the object class it acts on*, and spec-0069 §4.1, *the record is the only format of a showcase camera*).

## 2. Research record

### 2.1 What the pinned renderer can draw

**Cited** from the Chunky source at `156e2bba2526fe4cf218243704b6f3e6d0281d1a` (the pinned core), read file by file:

| Object | Key | Default at the pin | What the code does with it |
|---|---|---|---|
| `sky` (`renderer/scene/sky/Sky.java`) | `mode` | `SIMULATED` | one of `SIMULATED`, `SOLID_COLOR`, `GRADIENT`, `SKYMAP_EQUIRECTANGULAR`, `SKYMAP_ANGULAR`, `SKYBOX` |
| | `color` | `(0, 0, 0)` | *Color used for the SOLID_COLOR sky mode*; an RGB object `{red, green, blue}` in 0–1 (`util/JsonUtil.rgbToJson`) |
| | `skyLight` | `1.0` | *Set the sky light modifier*: scales the sky's **lighting** contribution (`ray.color.scale(skyLightModifier)` in `getSkyColor` and `getSkyColorDiffuseSun`) |
| | `apparentSkyLight` | `1.0` | scales the sky the **lens sees** (`getApparentSkyColor`, `getSkyColorInterpolated`), not what it lights |
| | `gradient` | a blue default | the `GRADIENT` mode's stops |
| `sun` (`renderer/scene/sky/Sun.java`) | `altitude`, `azimuth` | π/3, π/2.5 | the direction; this engine already emits both (`scene::sun_at`) |
| | `intensity` | `1.25` | scales the sun's emittance: `emittance.scale(pow(intensity, DEFAULT_GAMMA))` |
| | `color` | `(1, 1, 1)` | the sun's colour, RGB object |
| | `drawTexture` | `true` | whether the sun disc is drawn |
| `fog` (`renderer/scene/Fog.java`, `FogMode.java`) | `mode` | `NONE` | `NONE`, `UNIFORM`, `LAYERED` |
| | `uniformDensity` | `0.0` (`Scene.DEFAULT_FOG_DENSITY`) | the uniform fog's density |
| | `color` | from the user's persistent settings | RGB object |
| | `skyFogDensity` | `1.0` | how much the fog colour blends into the visible sky |
| | `layers[]` | empty | `{y, breadth, density}` bands, `LAYERED` mode only |

`Scene.toJson` at the pin writes these as three nested objects — `json.add("sky", …)`, `json.add("sun", …)`, `json.add("fog", …)` — and declares `SDF_VERSION = 10`. Chunky's published scene-format page documents sdf 9 with `fogDensity` and `skyFogDensity` at the top level ([Scene Format](https://chunky-dev.github.io/docs/reference/technical/scene_format/)); the pinned source is the authority, and the record below confirms the nested `fog` object is what the pinned core read. Nothing in the pinned core draws precipitation, and nothing occludes the sun by cloud: *weather* reaches a Chunky frame only as the sky's radiance and colour, the sun's intensity and disc, and fog.

### 2.2 What an overcast sky is

**Cited**, each with its source:

1. *Under an overcast sky, there is no direct sunlight, and all light results from diffused skylight radiation.* Its intensity *ranges (roughly) from 1⁄6 of direct sunlight for relatively thin clouds down to 1⁄1000 of direct sunlight under the extreme of thickest storm clouds*, and *the cloud droplets are larger than the light's wavelength and scatter all colors approximately equally* — the light is not coloured by the cloud. Wikipedia, [*Diffuse sky radiation*](https://en.wikipedia.org/wiki/Diffuse_sky_radiation).
2. Illuminance by condition: bright sunlight 111,000 lux; shade under a clear blue sky at midday 20,000 lux; *typical overcast day, midday* 1,000–2,000 lux; *sunrise or sunset on a clear day* 400 lux; *fully overcast, sunset/sunrise* 40 lux; thickest storm clouds at midday under 200 lux. Wikipedia, [*Daylight*](https://en.wikipedia.org/wiki/Daylight). So at one hour, overcast is darker than clear by one to two orders of magnitude, and an overcast sky at midday is 25–50 times brighter than an overcast sky at sunset: **the hour still matters under cloud**.
3. Overcast daylight is 6,500 K — the D65 white point, a neutral grey — against 5,000 K for horizon daylight and 15,000–27,000 K for a clear blue sky. Wikipedia, [*Color temperature*](https://en.wikipedia.org/wiki/Color_temperature). An overcast sky is grey, not blue.
4. The CIE standard overcast sky (Moon and Spencer 1942, adopted 1955) has luminance *L = L_z (1 + 2 sin γ) / 3* for a sky element at elevation γ: the zenith is three times brighter than the horizon. Darula and Kittler, [*CIE general sky standard defining luminance distributions*](https://publications.ibpsa.org/proceedings/esim/2002/papers/esim2002_o2.pdf) *(summary only)*.
5. Vanilla's own ordering of the three weathers, as this engine already models it: `light::effective_sky` holds daytime sky light at 15 clear, 12 rain, 7 thunder, after the documented `getSkyDarken` surface model — thunder is darker than rain, and both are no darker than clear.

### 2.3 What has been measured on this engine

**Cited**, `docs/reference/showcase-shots.md` §2b (four rounds by look on one delve's dusk exteriors, keys set on the emitted scene beside the camera `delvec cameras` wrote, rendered by the pinned core):

- the overcast dusk that was shipped: `sky.mode SOLID_COLOR`, `sky.color (0.10, 0.11, 0.14)`, `sky.skyLight 1.6`, `sky.apparentSkyLight 0.6`; `sun.intensity 0.25`, `sun.color (0.9, 0.75, 0.65)`, `sun.drawTexture false`, the sun's position left where the hour put it; `fog.mode UNIFORM`, `fog.uniformDensity 0.002`, `fog.color (0.20, 0.22, 0.26)`, `fog.skyFogDensity 0`;
- the simulated default *reads as a clear summer evening: a warm band on the horizon and hard shadows*;
- while `skyFogDensity` is 1 the fog colour paints the visible sky, so the sky colour cannot be darkened without setting it to 0;
- `apparentSkyLight 0.6` darkens the sky the lens sees without darkening what the sky lights;
- a `GRADIENT` sky looked the same as the solid one under this fog and was dropped;
- uniform fog at 0.006 turned a castle 250 blocks away into a silhouette; 0.002 separates a valley wall from the castle without hiding either;
- an exterior under that sky exposes at 2.0 where the clear frames took 1.0.

The record's own §5 says no source on reproducing weather in Chunky was read and the values were chosen by look on one delve. §2.2 above is the research that section says was missing: it fixes the **shape** of an overcast block — no sun disc, direct sun cut to a fraction, a neutral grey sky, radiance that still follows the hour, thunder darker than rain — and the measured values satisfy every one of those constraints. The numbers themselves stay **authored** (§4).

## 3. The surface: `sky` on a camera

**Authored.** `design/cameras.json` gains one optional field per camera:

```json
{
  "answers": "concept/dusk-chapel",
  "exposure": 10.0,
  "fov": 66.0,
  "height": 900,
  "name": "chapel-from-the-nave",
  "pitch": 12.0,
  "pos": [37.5, 85.0, 33.0],
  "sky": { "time": "noon", "weather": "clear" },
  "source": "estimated",
  "spp": 1024,
  "width": 1600,
  "yaw": 300.0
}
```

1. **`sky` is `{time, weather}`**, typed as `WorldTime` and `WorldWeather` — the two enums `world.json` and `design.json` already use. One authority for what an hour or a weather is (**cited**: spec-0061 §2, *a new hour added to the world is an hour a row can state, with no second table*). Both members are required when the object is present: a sky is one state, and a half-stated sky would make the other half a derivation hidden inside a stated object.
2. **Absent means the picture's.** A camera that states no `sky` renders under the `time` and `weather` of the `design.json` row it `answers`. That row's sky is a sky the party can be in (**cited**: `DW0890` holds the rows' skies equal to the world's reachable set), so a default frame is always of an hour and a weather the delve reaches. This replaces the plan's initial hour as the camera's default sun; §5 says what the plan's hour still governs.
3. **A stated `sky` equal to the row's is refused.** It is a derivation typed where a judgement belongs, and it goes stale the moment the row changes (**cited**: CLAUDE.md, *every input to a surface is either a creative judgement or a procedural derivation, handed by the tool, never typed*). The message names the camera, the row, and the one remedy: remove the field.
4. **The record's writer takes it.** `delvec place-camera` gains `--sky <time>,<weather>`, parsed with the two enums' own keywords and refusing anything else: a hand row is written and removed by the writer alone (spec-0069 §4.1), so a person's camera that wants its own sky needs the writer to carry it. `--candidates --pick` copies the candidate's `sky` verbatim, since `candidates.json` is in the record format; `--bracket` writes each candidate with its camera's `sky`.
5. **Key order stays alphabetical**: `pitch, pos, sky, source, spp, width, yaw`. `crates/delvec/tests/hand_camera.rs` already holds `compiler.md`'s field list to the reader's structs in both directions; `sky` enters that list.

## 4. What the engine emits for a sky

**Authored, on §2's cited shape.** One function, `scene::sky_of(time, weather)`, beside `sun_at`, yields the whole sky block of a scene: sun direction, sun intensity, colour and disc, sky mode, colour and the two light modifiers, fog. Every scene builder in `crates/delvec/src/compiler/view/` writes its sky through it and nothing else (§5), so a solved camera, a stated camera and a review frame are one sky shape as they are already one scene shape.

1. **`clear` is the renderer's own sky.** For `weather: clear` the block is the hour's sun direction and nothing more — exactly today's emission — because Chunky's simulated sky with its default sun *is* a clear sky, and writing its defaults back would move every clear scene's bytes for no capability. A clear scene at `19eba477` and a clear scene after this spec are byte-identical (ADR-0006 — **cited**).
2. **`rain` and `thunder` are overcast blocks**, written in full on every non-clear scene:
   - `sun.altitude` / `sun.azimuth` from `sun_at` as today — what little direct light there is comes from the right side;
   - `sun.drawTexture false` and `sun.intensity` well under Chunky's 1.25 — there is no visible sun and no hard shadow under cloud (§2.2.1);
   - `sky.mode SOLID_COLOR` with a neutral grey `sky.color` (§2.2.3; the `GRADIENT` shape §2.2.4 prescribes was measured to look the same under this fog, §2.3, and a solid colour is one number fewer to judge); `sky.skyLight` carries the sky's lighting and `sky.apparentSkyLight` what the lens sees (§2.1);
   - `fog.mode UNIFORM`, a low `fog.uniformDensity`, a slate `fog.color`, `fog.skyFogDensity 0` so the sky colour is the sky colour (§2.3);
   - `thunder` is darker than `rain` in sky radiance and denser in fog (§2.2.5).
3. **The hour reaches the overcast sky through the sun's altitude.** An overcast sky at midday is 25–50 times brighter than one at sunset (§2.2.2). The block's radiance is therefore keyed by a **daylight class derived from the emitted sun's altitude**, never by the hour's name: `high` at or above 20° (today `noon` 90°, `day` about 28°), `low` from 0° up to 20° (`dusk` 12.4°), `below` under 0° (`night` and `dawn` about −3.5°, `midnight` −90°). The altitudes are computed from the sky-angle curve `sun_at` implements and the dusk value agrees with the one the showcase record read off an emitted scene (0.2169 rad); the implementation's test is the measurement. A seventh hour added to `WorldTime` falls into a class by its sun, with no row to remember (**cited**: CLAUDE.md, *a primitive encodes a mechanism*). The 20° threshold is authored.
4. **The table.** Three classes by two weathers: six cells, each the full block of item 2. The `low × rain` cell is the measured block of §2.3, adopted verbatim — the one cell anyone has looked at. The other five are first values stated here for the owner to judge (§8 decision 1), derived from that cell by two authored rules: `high` doubles the measured cell's `skyLight` and `apparentSkyLight`, `below` sets both to a tenth and the sun's intensity to 0 (the sun is under the horizon; §2.2.2's 25–50× is a photometric ratio and this is a look table, so the factors are tempered and said to be); `thunder` takes the `rain` cell of its class at 0.6 of its two light modifiers, half its sun intensity, and 1.5 times its fog density. Every cell is a constant in `scene.rs`, printed by `delvec cameras` as part of the sky line (§6), and the table is written down once: `compiler.md` carries it as the record of what the engine emits, and `showcase-shots.md` §2b points at it instead of restating numbers.
5. **Determinism.** The block is constants, a class decision on the already-rounded altitude, and `round6` on every float; serde writes the struct in declaration order; no float is derived by trigonometry other than the sun's direction, which is already platform-stable (ADR-0006 — **cited**). Two emissions of one record against one plan are byte-identical, and the test that already proves it for a hand row (`hand_camera::a_hand_row_builds_byte_identically`) extends to a record carrying a stated and a derived overcast sky.

## 5. Where the sky comes from, per scene kind

**Authored.** Three kinds of scene leave this crate, and each states where its sky is read from — one function, three callers, no private copy:

| Scene | `time`, `weather` from | Why |
|---|---|---|
| a showcase camera (`delvec cameras`) | the camera's `sky`, else the `design.json` row it `answers` | §3.2: the frame is of the picture |
| the whole-map panorama (`delvec panorama`) | `render-plan.json`'s `sky` | it answers no row; it is the delve at its declared hour |
| a review frame (`delvec scene`: POV, interior, seam, …) | `render-plan.json`'s `sky` | the player's own view at the initial hour, which is what the review judges |

For the second and third to carry a weather, **`render-plan.json`'s `sky` fact gains `weather`** — the world's declared initial weather, as a keyword beside `time` — and `plan_sky` requires it as it requires the hour, so a plan written by an older engine is refused by name (today's `DW_INPUT` rule, unchanged in shape). The reason the plan left the weather out no longer holds: the renderer can act on it now (§2.1, §4). The cost: every build's `render-plan.json` moves by one key, and the gallery baseline's rows move with it, each attributed in the commit body that regenerates it. Clear campaigns' **scenes** do not move (§4.1).

Review frames of a rain or thunder campaign change: they gain the overcast block. The night-vision review emulation (`REVIEW_POLICY`) is a material override and is independent of the sky, so a declared-dark, night-vision-mitigated room under rain gets both. The alternative — the overcast sky for showcase cameras only — is a general mechanism privately bound to one verb (CLAUDE.md names that shape as the defect — **cited**), and would leave the review frames the owner judges at the visual-review step under a sky the party never sees.

## 6. Refusals and the binding line

**Authored.** Every refusal is a rule of the record and is raised under `DW0721` by the record's one reader, so `delvec cameras`, `delvec place-camera` and `delvec build` give one answer (spec-0070 §5 — **cited**):

- a `sky` with a member missing, or a value outside the two enums (serde, as every other field);
- a `sky` equal to the answered row's (§3.3);
- `--sky` on `place-camera` that does not parse as `<time>,<weather>` in the enums' keywords;
- a plan whose `sky` has no `weather` (`DW_INPUT`, the existing shape, with the message naming the engine that wrote the plan).

**The binding line**, printed by `delvec cameras` on every run, refused or not, beside the existing `answers: K of N` sentence: one line per emitted camera — `sky: <name> <time>+<weather> <derived from <row> | stated> class <high|low|below>` — and a summary `skies: D derived, S stated, over C camera(s); weathers emitted: {…}`. A run whose record states no sky prints `0 stated`, a measured zero; a run that emitted no non-clear weather prints the empty set, so a creator whose rain delve is rendering clear sees it on the line (CLAUDE.md, *every validation artifact states its binding count with its denominator* — **cited**).

## 7. The gallery's obligation, and the gate that cannot see it yet

**Authored**, against `gallery/` at `19eba477`: `design.json` has six rows — `day`, `noon`, `dusk`+`rain`, `night`, `midnight`+`thunder`, `dawn`, the rest `clear` — and `design/cameras.json` seven cameras answering them, none stating a sky.

1. **Bound elements.** By derivation alone the gallery already reaches every weather and every daylight class the moment §4 lands: `rampart-against-the-sky` (dusk+rain, `low`), `crypt-stair` (midnight+thunder, `below`), and five clear cameras across `high`, `low` and `below`. One camera additionally **states** a sky differing from its row — the chapel shape of §1 — so the stated path is bound too. A probe, `a-sky-that-restates-its-picture`, patches one camera's `sky` to its own row's pair and is refused `DW0721`, under the probe mechanism that already patches `design/cameras.json` (`a-picture-nobody-looks-at`).
2. **The gate cannot count it today.** `tools/ci/check-gallery-coverage.py` enumerates units from `delvec schema --stage all`, which exports `Stage::ALL`; the camera record is not a stage document, `CameraSheet` derives no `JsonSchema`, and the existing camera probe says so in its own words — *the camera record is not a schema unit*. So at `19eba477` the record's units are enumerated **0 of N** by the one gate that is supposed to prove every surface is written somewhere, and `sky` would be an unenumerated unit on an unenumerated document. That is a finding of the vacuous kind CLAUDE.md names (unbound: the gate matched zero objects of this class), and it is closed here rather than recorded: `CameraSheet` and `Camera` derive `JsonSchema`; `delvec schema cameras` exports the record beside `walk-record` and `prefab-metadata`, which are the non-stage documents `delvec schema` already answers to; the export names the file the record lives at (`camera::CAMERAS_FILE`), and the coverage gate enumerates and binds the record's units against `gallery/design/cameras.json` exactly as it binds a stage document, reading the path from the export and listing nothing itself (`gallery_units.stage_files`'s own rule — **cited**). After this, `Camera.sky`, `Camera.exposure` and every other field of the record are units, bound or refusal-proven, with the counts on the gate's own line.
3. **Perturbation acceptance.** Changing a gallery camera's stated `sky` from `clear` to `rain` moves the emitted scene's bytes (the overcast block appears); changing the answered row's `weather` moves a derived camera's scene the same way. No CI job emits the gallery's camera scenes today; the test of criterion 7 does, in process, over the gallery's own record.

## 8. Decisions for the owner

Only what she would see.

1. **The look table (§4.4).** Recommendation: adopt the measured dusk-rain block as the `low × rain` cell, and the five derived cells as first values, each confirmed on a gallery camera's draft frame before the pull request leaves draft — never on a campaign's renders. What she sees: every rain or thunder picture of every delve takes these skies until a cell is changed, and a cell is changed by looking at a frame, not by a campaign patching a scene. The cost of deciding later: the table lands with the dusk cell measured and five cells unjudged, and the first delve rendered at a rainy noon is the first time anyone looks at that cell.
2. **Review frames take the weather too (§5).** Recommendation: yes. What she sees: at the visual-review step, the POV and interior frames of a rain or thunder delve are overcast, and so are their draft renders; a clear delve's frames do not change. If no: only showcase cameras change, and the frames she judges a room's light by are under a sky the party never stands in.

## Departures

Recorded at implementation, each with its reason. A departure that reduces what a criterion asserts says so in the word *loosening*.

1. **A clear night is not the renderer's own sky.** §4.1 took Chunky's simulated sky with the hour's sun for every clear scene, on the reading that a sun below the horizon renders as night. It does not: the pinned core's `PreethamSky.updateSun` (read from the `chunky-core-2.5.0-SNAPSHOT.474.g156e2bb` jar's bytecode) clamps the sun's altitude to `[0, π]` before it shades the sky, so a clear scene under the moon drew a late-afternoon sunset — the Treehouse Camp's `lantern-night` camera, answering a row drawn under `{"moon": "high"}`, rendered with its sun at −90° and a bright evening sky. The camera already carries the time (its row's, or its own `sky`); the emission was the defect. A clear scene of the `below` class now writes the whole block of a seventh cell, the **night cell** (`scene::BELOW_CLEAR`): **cited** from the vendored day timeline's night plateau — `visual/sky_color` `#000000` (the lens sees no sky: `apparentSkyLight` 0), `visual/sky_light_factor` 0.24 and `visual/sky_light_color` `#7a7aff` (`skyLight`, `sky.color`), `visual/fog_color` `#0f0f16`; **authored** — the mapping of those factors onto Chunky's keys against the simulated sky's `skyLight` 1, the plateau taken for the whole class (a twilight under the horizon is drawn as night, the direction the sky-light model already judges it in), no sun, no fog density. **Criteria 4 and 5, a loosening**: byte identity with the base, and "no block keys", now hold for clear scenes with the sun at or above the horizon only; a clear night carries the whole block. Shown in `scene.rs` `the_clear_night_cell_is_the_timelines_night_plateau` and `every_scene_builder_writes_the_whole_block_or_none_of_it`, and `camera_sky.rs` `a_camera_at_a_night_beat_renders_the_night_sky`. Criterion 10's draft frame for this cell is not yet rendered: an open debt, owed the first render on the pinned core.

## Acceptance criteria

Each criterion is checked against the tree at `19eba477`: criterion 4's first sentence holds there (clear scenes are already the renderer's default sky plus the sun) and must keep holding; none of the others is satisfied there, and criterion 8 names the gap in the tree the implementation closes.

1. **Surface.** The camera record accepts an optional `sky: {time, weather}` typed as `WorldTime` / `WorldWeather`; `delvec schema cameras` exports the record's schema with `sky` on it; `crates/delvec/tests/hand_camera.rs`'s field-list test holds `compiler.md`'s list to the structs with `sky` in it, in both directions.
2. **Default and override.** `crates/delvec/tests/view_cli.rs` (or a sibling): a camera with no `sky` emits the sun of its **answered row's** hour and the block of its row's weather — a camera answering a `midnight`+`thunder` row in a `day`+`clear` world emits a midnight sun and the thunder block; a camera stating `sky` emits the stated pair and nothing of the row's; the three scene kinds of §5 read their sky from the source the table names, asserted by emitting a panorama and a review scene of the same rain plan and a stated-clear camera against it.
3. **Refusals.** A `sky` equal to the row's, a half-stated `sky`, an unknown keyword, and a `place-camera --sky` that does not parse are each refused `DW0721` with the camera and row named, by `delvec cameras`, `delvec build` and `delvec place-camera` alike, with the message's remedy a row in `crates/delvec/tests/remedy_reachability.rs`; a plan without `sky.weather` is refused by `plan_sky` naming the engine that wrote it.
4. **Byte identity.** A `clear` scene emitted after this change is byte-identical to one emitted at `19eba477` for the same record, plan and options (fixture scenes committed from the base and compared); two emissions of a record carrying one derived-overcast and one stated-overcast camera are byte-identical to each other (`hand_camera::a_hand_row_builds_byte_identically`, extended).
5. **The block is whole and one.** For every non-clear scene, the emitted JSON carries `sky.mode`, `sky.color`, `sky.skyLight`, `sky.apparentSkyLight`, `sun.intensity`, `sun.color`, `sun.drawTexture`, `fog.mode`, `fog.uniformDensity`, `fog.color` and `fog.skyFogDensity`, and for every clear scene none of them; `cargo test` asserts both over the six-cell table and over the three scene builders, and `grep` finds `sky_of` as the only writer of those keys under `crates/delvec/src/compiler/view/`.
6. **The class is the sun's.** A test asserts each of the six `WorldTime` values' daylight class from `sun_at`'s altitude (`noon`, `day` → `high`; `dusk` → `low`; `night`, `midnight`, `dawn` → `below`), and that the `thunder` cell of every class is darker than its `rain` cell in `skyLight`, `apparentSkyLight` and `sun.intensity` and denser in `fog.uniformDensity`.
7. **Gallery** (spec-0039). `gallery/design/cameras.json` has one camera stating a `sky` that differs from its row's; a `cargo test` emits every gallery camera's scene in process against the gallery's build and asserts the set of weathers emitted is `{clear, rain, thunder}` and the set of classes `{high, low, below}`, with the counts printed; the probe `a-sky-that-restates-its-picture` is refused `DW0721`; the regenerated `gallery/baseline/` moves `render-plan.json`'s `sky` in every point and nothing in any clear point's scenes, and the commit body attributes every moved row.
8. **The gate sees the record.** `tools/ci/check-gallery-coverage.py` enumerates the camera record's units from `delvec schema cameras` and binds them against `gallery/design/cameras.json`; its run line reports the record's units enumerated and bound, with `Camera.sky` bound and 0 of the record's units in neither state; perturbing the gallery record to state no sky anywhere reds that unit. The debt this closes is stated in the pull request body with the before count (0 of N) and the after.
9. **Binding line.** `delvec cameras` prints the per-camera `sky:` line and the `skies:` summary of §6 on a record with no stated sky (`0 stated`) and on one with a stated sky, asserted in `view_cli.rs`.
10. **Determinism across the pin.** The six-cell table and the `SOLID_COLOR` / `UNIFORM` keys are the pinned core's (§2.1); `docs/reference/tools.md` §4a records one draft frame per non-clear cell rendered by `validation/chunky.sh` on the pinned core, with the scene's hash and the frame's hash, as the record of what each cell looks like.
11. **Docs and skill, same change.** `docs/reference/compiler.md`: the camera record's field list gains `sky`; the `delvec cameras` section states §5's table and the six-cell look table; the `render-plan.json` section states `sky.weather`; `delvec place-camera` gains `--sky`. `docs/reference/tools.md` §4a: the field table gains `sky`, and the hand-camera loop says when to pass `--sky`. `docs/reference/showcase-shots.md` §2b is rewritten to point at the engine's table and to say a frame under a sky other than its row's states `sky` on its camera and says so in its caption; §5's last line (no source on weather in Chunky) is replaced by a pointer to §2 here. The skill's `references/hand-over.md` keeps *never hand-edit a scene JSON* and gains the one line that the sky is a camera field; `tools/ci/check-skill-page.py` is green with `--sky` on the pinned release's clap surface or the line lands with the pin that carries it. `docs/specs/README.md` gains this spec's row. No row in `docs/demo-levels.md`: a render capability a player never meets is confirmed on the gallery's frames (criterion 10), not on a demo level (spec-0069 §7's precedent).
