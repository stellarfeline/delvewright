# spec-0081: A sky is stated in a designer's words — a time names the sun or the moon, where it stands and what phase it shows, and the engine computes the ticks

- **Status**: Approved
- **Ground**: written against engine `4c0fb85e` (`origin/main`), read only — `WorldTime` (`spec`, `token`, `daytime_ticks`, `keyword`) in `crates/dsl/src/stages.rs`; `effective_sky` and `reachable_time_weather` in `crates/delvec/src/compiler/light.rs`; `hour_burns`, `MONSTERS_BURN_OFF_AT`/`_ON_AT` and `Clock` in `compiler/daylight.rs`; `time_index` and `check` in `compiler/design.rs`; `sun_at`, `sky_angle_rad`, `daylight_class` and `sky_of` in `compiler/view/scene.rs`; `sky_fact` in `compiler/render_plan.rs`; `sealing_commands` and the `sealed_state` PackTest in `compiler/emit.rs`; `Camera.sky` in `compiler/view/camera.rs`. The pinned 1.21.11 data and code, read for this spec from the server jar `versions.toml` `[minecraft]` `server_jar_sha256` `f83b8e09…dd1726` (bundled `META-INF/versions/1.21.11/server-1.21.11.jar`, sha256 `ec47239a…02fcebc`) through Mojang's published 1.21.11 obfuscation maps, and from the 1.21.11 client jar sha256 `1473c948…6cd3bd`. The live measurements are the spike `tools/spike-celestial-time/ at 4849172ae054` (`run.sh`, `measure.mjs`, `observations.json`), on this branch. The per-biome limits of spec-0080 §2.3 (branch `origin/feat/area-atmosphere`) and the spike `tools/spike-eldritch-visuals/` (branch `origin/research/eldritch-visuals`, README *What a biome cannot do in the overworld*) are cited where this spec stops.
- **What it is for**: the engine already freezes the day cycle and sets the clock (`time set <kw|ticks>`, `advance_time false`), and a creator states the hour in six words. A designer's sentence about a sky — *a new moon just above the horizon*, *a full moon high in the sky*, *the sun just set* — is a statement about the same clock, and today it cannot be written: the six words name six points of one day, and the moon every one of them shows is full, because `time set night` is day 0 and nothing says so. This spec lets the creator write the sentence and makes the engine compute the one tick count that produces it, day count included, with every proof that reads the hour reading the same value.
- **Research**: §2 is this spec's research record. Each rule is marked **cited** (a reading of the pinned jar or its data, a measurement in the spike, a published definition, a constitution rule) or **authored** (this spec chooses). A fact read from the data and not run on a server says so.
- **Numbers**: no ADR. One DW code, **`DW0931`**: every refusal here is a rule of one value's shape (§6), and the shapes are listed so the reader sees none needs a code of its own. An unknown word is the schema's refusal (`DW0100`) as every enum's is. `dsl_version` moves to the number the planner hands the implementation; it numbers a surface and promises nothing.
- **Non-goals**: moving the sun or moon for one place rather than the world (spec-0080 §2.3 — the overworld day timeline overrides `visual/sun_angle`, `visual/moon_angle` and `visual/moon_phase` per biome; the vanilla remedies are a custom dimension or an edited timeline, both world-wide, and neither is this spec); a moon in a Chunky frame (§5.4, the pinned core has no moon); the stars, the sunrise band and the sky's colours, which the day timeline derives from the same clock and this spec does not touch; a timeline of beats (spec-0061 §12: times are compared as a set); the `time add` form (a relative cut is an order, not a declaration); a world with a fixed hour (`fixed_time`), which is a dimension type and not a time.

## 1. The object, and what the six words leave unsaid

**Cited** (the tree at `4c0fb85e`): `WorldTime` is six keywords, one table (`WorldTime::spec`) maps each to its `/time set` argument and its `daytime` read-back, and the type is used at five sites — `world.time`, the `set-time` quest effect, the `set-time` dialogue effect, `design.json`'s `references[].time` and a showcase camera's `sky.time`. Every reader of the hour reads the enum: `light::effective_sky` by variant, `daylight::hour_burns` through `daytime_ticks`, `design::time_index` by variant, `scene::sun_at` through `daytime_ticks`, and `sky_fact` writes the keyword and the ticks.

**Cited** (§2.1, measured): the game's clock is one number, `dayTime`. `time set` writes it absolutely; `time query daytime` reads it modulo 24000 and `time query day` divided by it; the moon's phase is that quotient modulo 8. So the hour the engine states is a point on a circle of 24000, and the engine always sets it on day 0 — the moon in every night delve built so far is full, and no document says so. That is spec-0061 §1's shape again — a design decision inside a mechanism — on the half of the clock the six words do not reach.

**The object is the time value.** Whatever states a time states the whole clock, so the capability lands on `WorldTime` and reaches all five sites at once (CLAUDE.md, *a capability belongs to the object class it acts on* — **cited**); nothing here is a field of `set-time` or of the world.

## 2. Research record

### 2.1 The clock, the command and the client

**Cited**, from the pinned server jar's code (through the published maps) and measured on the pinned server image (`observations.json`):

- `TimeCommand.setTime` calls `ServerLevel.setDayTime(long)` on every level, which stores the value as `PrimaryLevelData.dayTime`, then `MinecraftServer.forceTimeSynchronization()`. Measured: `time set 109000` answers `Set the time to 109000`; `time query day` reads 4, `time query daytime` 13000, and `time query gametime` moved only by the 4 ticks the round trip took — the game clock is a different number and `time set` does not touch it.
- **A keyword is absolute too.** After day 4, `time set night` answers `Set the time to 13000`, and `time query day` reads **0**: the keyword resets the day count. `time add 13000` after `time set 96000` keeps day 4. The argument refuses a negative (`The tick count must not be less than 0`) and accepts the suffixes `d`, `s`, `t` (`4d` sets 96000, `1s` sets 20).
- **The client is told at once.** The `update_time` packet (`ClientboundSetTimePacket`: `gameTime`, `dayTime`, `tickDayTime`) carrying `dayTime` 109000 reached a connected mineflayer bot **41 ms** after the command was issued, with no reconnect; the routine sync follows every 20 ticks. `tickDayTime` reads `false` under `advance_time false`, and the set state held unchanged over 2.5 s (two reads).
- **The server can read the phase.** The loot predicate `minecraft:time_check` with `period: 192000` tests `dayTime` modulo the moon cycle: `dwspike:new_moon` (`96000..119999`) answered true at 109000 and false at 13000; `dwspike:full_moon` (`0..23999`) the reverse. A PackTest can therefore assert the phase on the server, not only the hour.

### 2.2 The moon

**Cited**, `data/minecraft/timeline/moon.json` of the pinned jar: period 192000; the track `minecraft:visual/moon_phase` has eight keyframes, one per 24000 ticks, in this order — `full_moon` 0, `waning_gibbous` 24000, `third_quarter` 48000, `waning_crescent` 72000, `new_moon` 96000, `waxing_crescent` 120000, `first_quarter` 144000, `waxing_gibbous` 168000. The timeline is driven by `dayTime` (`AttributeTrackSampler.dayTimeGetter`; `KeyframeTrackSampler.loopTicks` is `floorMod(dayTime, period)`), so the phase is `(dayTime / 24000) mod 8` and nothing else — `MoonPhase.PHASE_LENGTH` is 24000 and `COUNT` 8. The enum's serialised names are the eight above. A fresh world is day 0 and shows a full moon.

### 2.3 The sun's path

**Cited**, `data/minecraft/timeline/day.json` (period 24000): `minecraft:visual/sun_angle` is two keyframes at tick 6000, value 360 then 0, eased by `cubic_bezier [0.362, 0.241, 0.638, 0.759]`; `moon_angle` is the same track at 540 → 180, so **the moon stands exactly opposite the sun** and the two bodies are one position; `star_angle` equals the sun's. `EasingType$CubicBezier` solves the x-curve for the progress by Newton–Raphson and samples the y-curve — the CSS `cubic-bezier` convention — and the client's `SkyRenderer.extractRenderState` reads `SUN_ANGLE`, `MOON_ANGLE`, `STAR_ANGLE` and `MOON_PHASE` from the attribute system, so this track is what the party sees.

**Measured against the engine**: `scene::sun_at` implements minecraft.wiki's closed form, cross-checked against the pre-timeline `DimensionType.timeOfDay`. Evaluated at every tick of the day, the pinned bezier track and that closed form differ by at most **0.056°** (at tick 11457). At the six keywords the sun's altitude on the pinned track is `day` 27.55°, `noon` 90°, `dusk` 12.37°, `night` −3.52°, `midnight` −90°, `dawn` −3.52°. The sun's centre crosses the horizon at tick **12782** (west) and **23218** (east).

### 2.4 What the client draws

**Cited**, the client's `SkyRenderer` constants and the celestial textures: `SUN_SIZE` 30 and `MOON_SIZE` 20, both at height 100, so the sun's quad spans ±16.70° and the moon's ±11.31°. Each texture (`celestial/sun.png`, `celestial/moon/<phase>.png`, 32×32) is a **disc of 8 pixels** at full brightness inside a glow that fades to the quad's edge — the middle row reads `1, 2, 4, 7, 10, 14, 19, 23, 27, 32, 36, 40, 255 ×8, 40 … 1` for the sun, and the moon's eight phases share the same 8-pixel core. So the **drawn disc** is a quarter of the quad: the sun's half-angle is `atan(30 × 8/32 / 100)` = **4.29°**, the moon's `atan(20 × 8/32 / 100)` = **2.86°**, and the glow around each is what the quad's remaining extent carries. The `night` keyword (−3.52°) leaves the top of the sun's disc 0.77° above the horizon.

### 2.5 What the game reads off the clock, and what the engine does

**Cited**, `day.json`: `minecraft:gameplay/sky_light_level` is a `multiply` track with four linear keyframes — 1.0 at 133 and 11867, 0.26666668 (= 4/15) at 13670 and 22330 — so the sky light is at its full 15 on the day plateau, at 4 on the night plateau, and between them in two twilight ramps. Evaluated at the keywords: `dusk` 12000 → 0.946 (14), `night` 13000 → 0.539 (8), `dawn` 23000 → 0.539 (8), `midnight` → 4. `minecraft:gameplay/monsters_burn` is `false` from 12542 and `true` from 23460 — the window `daylight::hour_burns` already reads by tick.

**Cited**, the engine: `light::effective_sky` holds `day`/`noon` at 15 and the other four at 4 and says in its own comment that dusk and dawn are held at the night floor as the **conservative** reading. The track confirms the two plateaus are 15 and 4 and that every one of the four non-day keywords sits at or below the game's value — the hand table is conservative at `dusk`, `night` and `dawn` and exact at `midnight`.

## 3. The surface

### 3.1 A time is a keyword or a celestial statement

**Authored.** `WorldTime` becomes an untagged pair: the six keywords as today, or an object naming one body, where it stands and, where it shows, the moon's phase:

```json
"time": "night"
"time": { "sun": "just-set", "phase": "new-moon" }
"time": { "moon": "just-risen", "phase": "new-moon" }
"time": { "moon": "high", "phase": "full-moon" }
"time": { "sun": "high" }
```

1. **Exactly one of `sun` / `moon`**, each a position (§3.2). Two bodies, one position, because the moon stands opposite the sun (§2.3): `{"moon": "rising"}` and `{"sun": "setting"}` are one hour spelled from two sides, and a creator writes the body the sentence is about.
2. **`phase`** — one of the eight names the pinned `moon.json` gives, kebab-cased (`full-moon`, `waning-gibbous`, `third-quarter`, `waning-crescent`, `new-moon`, `waxing-crescent`, `first-quarter`, `waxing-gibbous`), validated against the vendored timeline (§4.1). The phase is **data**, not an enum the engine authors: the names are vanilla's own, read from the pinned file, the way a block id is (spec-0039, *vanilla registry values are data, never units* — **cited**).
3. **When `phase` is written.** It is **required** on `world.time` whenever the moon is above the horizon at the stated position (sun altitude at or below 0°) — the party can see it, so nobody may leave it to a default (spec-0061 §4 — **cited**). It is **refused** at any position where the moon is below the horizon: a judgement about a body nobody can see reaches nothing (the shape spec-0079 §3.3 refuses — **cited**). On a `set-time`, a design row or a camera sky, an absent `phase` is **the world's**: a cut changes the hour, and the moon keeps the phase the world declared unless the cut states another — a derivation the creator never types (CLAUDE.md, *every input is a judgement or a derivation* — **cited**).
4. **A keyword states vanilla's word with vanilla's meaning where it states a sky, and changes only the hour where it is a cut.** On `world.time`, a design row or a camera's `sky.time`, a keyword is the hour on day 0 (§2.1, `time set night` is day 0): the moon a keyword night shows is therefore `full-moon`, and the binding line (§5.5) prints it, so the creator who wrote `midnight` sees *moon full-moon, 90°* and can change it. On a `set-time` cut, quest or dialogue, a keyword is the hour on **the world's day**: a cut changes the hour and keeps the moon the world declared, exactly as a celestial cut with no `phase` does (§3.1.3) — so `noon` cut in a world whose moon is new is day 4, `time set 102000`. In a world stated by a keyword (day 0) the two readings coincide, so the keywords are kept as spellings and nothing about the emission of a keyword campaign moves (§4.4).

### 3.2 The positions

**Authored on cited geometry.** Six words, each one point of a body's arc, defined by the body's drawn disc (§2.4) and the horizon:

| Position | Definition | Sun's altitude | Sun tick | Moon's altitude | Moon tick |
|---|---|---|---|---|---|
| `rising` | the disc's centre on the eastern horizon | 0° E | 23218 | 0° E | 12782 |
| `just-risen` | the disc stands wholly clear of the eastern horizon | +4.29° E | 23486 | +2.86° E | 12959 |
| `high` | the body at the zenith | +90° | 6000 | +90° | 18000 |
| `setting` | the disc's centre on the western horizon | 0° W | 12782 | 0° W | 23218 |
| `just-set` | the disc has wholly gone below the western horizon | −4.29° W | 13047 | −2.86° W | 23397 |
| `below` | the body at the nadir — the other body is `high` | −90° | 18000 | −90° | 6000 |

`rising` and `setting` are the geometric rise and set — the centre crossing the horizon; the game has no refraction and no limb, so the published −0.833° correction does not apply (**cited**: the astronomical definition; **authored**: the centre, since the drawn body is a disc on a quad). `just-risen` / `just-set` are the disc, not the glow: *a new moon just above the horizon* is the whole moon showing, and *the sun just set* is the last of the disc gone. The alternative — civil twilight's −6° — is a statement about the light on the ground, not about the disc, and is **not chosen**; it is noted so the choice is visible. `high` is upper culmination (**cited**); `below` is its opposite and exists so a creator can say *no sun* without naming the moon. The twelve ticks are the integers nearest the pinned track's solution (§2.3, bisection on the bezier, floating point in a test and never on the emission path), and `{"moon": "rising"}` = `{"sun": "setting"}` = 12782 by construction.

### 3.3 Composing with the six keywords

**Authored.** The two spellings resolve through one function to one value, `Clock { day, daytime }`: a keyword resolves to its table row on the day §3.1.4 gives — day 0 where it states a sky (the world, a design row, a camera), the world's day where it is a `set-time` cut; a celestial statement resolves to its position's tick and the day its `phase` names (§2.2's index), or the world's day. `WorldTime::daytime_ticks` returns `daytime` and keeps every caller; `WorldTime::keyword` returns the author's spelling for a keyword and the JSON of the object for a celestial time, which is what a diagnostic prints back. Two values are **equal when their clocks are equal**, so `reachable_time_weather` collects clocks, and `DW0890` compares the design rows' clocks with the world's reachable clocks exactly as it compares keywords today — a row may state `{"sun": "just-set"}` against a world that reaches it.

## 4. One authority

### 4.1 The pinned timelines, vendored

**Authored.** `crates/dsl/data/timeline-day-1.21.11.json` and `timeline-moon-1.21.11.json` are the two files of §2.2–2.3 copied byte for byte from the pinned server jar, with the jar's sha256 and the path inside it in `crates/dsl/data/PROVENANCE.md`, regenerated by a `tools/maintenance/` script that refuses when the extracted bytes differ from the committed ones — the shape the entity-tag table already takes. The phase names, the sun track and its ease, the sky-light and burn keyframes are all read from these files and from nothing hand-typed: `MONSTERS_BURN_OFF_AT`/`_ON_AT` become readings of the vendored track, asserted equal to today's constants.

### 4.2 The position table is derived, then frozen

**Authored.** The twelve ticks of §3.2 are integer constants in `crates/dsl` (ADR-0006 — **cited**: no trigonometry on the emission path), and one test re-derives every one from the vendored `sun_angle` track, the renderer's two constants (`SUN_SIZE` 30, `MOON_SIZE` 20, height 100, disc 8 of 32 — recorded in the test with the texture reading of §2.4) and the horizon, and fails when a constant differs from the derivation by a tick. The disc fraction is a measurement of the pinned client's textures; a client-jar pin bump re-measures it.

### 4.3 The sun the engine draws is the sun the game draws

**Authored.** `scene::sky_angle_rad` evaluates the vendored bezier track instead of the closed form, so the frames and the position table read one curve. The test `the_two_published_sun_angle_formulas_agree` becomes the measurement of §2.3 — the closed form agrees with the pinned track to 0.056° — and keeps both expressions. **The cost, declared**: every clear scene's `sun.altitude` and `azimuth` move by up to 0.001 rad, so the goldens of `clear_scenes_keep_their_base_bytes` (spec-0079 criterion 4) are re-emitted once, with the per-hour deltas in the commit body; a clear scene is still byte-identical between two emissions.

### 4.4 What is emitted

**Authored.** `WorldTime::token` becomes a function of the clock: a keyword on day 0 emits its keyword verbatim as today, so no existing campaign's bytes move; any other clock emits the integer `day × 24000 + daytime`, which is the same primitive `dusk` and `dawn` already use. The sealing baseline and every `set-time` go through this one token. The `sealed_state` PackTest asserts both read-backs — `time query daytime` equal to `daytime` and `time query day` equal to `day` — and, for a world whose moon is up, `execute if predicate <ns>:moon_<phase>` through a `time_check` predicate the compiler emits over `period: 192000` (§2.1), so the phase the creator wrote is proven on the server. `render-plan.json`'s `sky` keeps `{time, daytime_ticks, weather}` with `time` the author's spelling (a string or the object); it gains no phase key, because no consumer of the plan can act on one (§5.4), which is the rule spec-0079 §1 gives for an unemitted key.

## 5. What reads the clock

### 5.1 The light model

**Authored, on §2.5.** `effective_sky` takes a clock and reads the vendored `sky_light_level` track at `daytime`: on the day plateau (value 1.0) the base is 15; **anywhere else it is 4** — the night plateau — exactly today's rule for the six keywords, now stated as a rule over any tick: a twilight is judged as night. This is the conservative direction and is **not a loosening**: every one of the six keywords keeps its value. The game's own value at the tick is printed beside the judged one on the binding line (§5.5), so a creator sees *judged at 4; the game's sky light here is 14* and knows the margin. Reading the track's value itself would raise `dusk` from 4 to 14 and `night` and `dawn` from 4 to 8; that is a loosening of `DW0210` in those words, and this spec does not make it.

### 5.2 The daylight-burn proof

**Authored.** `hour_burns` already reads the burn window by tick, so a celestial time answers it with no new rule: `{"sun": "just-set"}` (13047) does not burn, `{"sun": "setting"}` (12782) does not (the window closes at 12542), `{"moon": "just-set"}` (23397) does not (it reopens at 23460), and `{"sun": "rising"}` (23218) does not. `daylight::Clock` orders the cuts it already orders; a cut's phase is irrelevant to it.

### 5.3 The design record

**Authored.** `DW0890` compares clocks (§3.3). Its message prints each side as the author's spelling followed by the clock — `{"moon":"high","phase":"new-moon"} (day 4, 18000)` — so a row stating `midnight` against a world declaring `{"moon": "high", "phase": "new-moon"}` is refused as two hours that differ by their day, with the remedy naming both spellings.

### 5.4 Renders

**Authored.** Every scene builder takes its sun from `sun_at(daytime)` (§4.3), so a camera whose `sky.time` is celestial renders under the sun its position names, and spec-0079's daylight class follows the altitude as it does today. **The pinned Chunky core draws no moon**: the sky package at `156e2bba…` is `Sky`, `Sun`, `SimulatedSky`, `NishitaSky`, `PreethamSky`, `SkyCache`, and `moon` occurs in none of them — so a night frame carries no phase, a statement the camera's `sky:` line makes (*moon not drawn by the renderer*), and the phase is confirmed in the game (§7.4), never on a render. Chunky's `Sun.drawTexture` under a sky whose sun is below the horizon is already what a night frame does.

### 5.5 The binding line

**Authored.** Every build prints one line per time value the campaign states — the world's and each cut's — and a summary:

```
clock: world {"moon":"just-risen","phase":"new-moon"} -> day 4 daytime 12959 (dayTime 108959); sun -2.86° W, moon +2.86° E new-moon; sky light judged 4, game 8; burns: no
clock: set-time noon -> day 4 daytime 6000 (dayTime 102000); sun +90.00°, moon below the horizon (new-moon); sky light judged 15, game 15; burns: yes
clocks: 1 world + 2 cut(s); 2 celestial, 1 keyword; phases stated {new-moon}; days {4}
```

The `set-time noon` line is a keyword cut, so it resolves on the world's day 4 (§3.1.4). A campaign of keywords prints `0 celestial` and `days {0}` — the measured zero, with the full moon it implies spelled on the world's line (CLAUDE.md, *every validation artifact states its binding count with its denominator* — **cited**).

## 6. Refusals — `DW0931`

**Authored**, raised at the document the value is entered in (`delvec validate`), with the remedy named and a row each in `crates/delvec/tests/remedy_reachability.rs`:

1. a celestial object naming neither or both of `sun` / `moon` (the `DW0160` exclusivity shape, under this code because the object is a time);
2. `phase` on a position where the moon is below the horizon — `{"sun": "high", "phase": …}`, `{"moon": "below", "phase": …}`, `{"sun": "just-risen", …}` — the message names the moon's altitude there;
3. `world.time` with no `phase` at a position where the moon is above the horizon — `{"sun": "just-set"}` as the world's time — the message lists the eight names;
4. a `phase` on a `set-time`, a row or a camera equal to the world's declared phase (a restatement, spec-0079 §3.3's shape — **cited**), with the remedy *remove it*.

A `phase` outside the eight names and a position outside the six words are `DW0100`, listed so the reader sees they are not a fifth shape.

## 7. The gallery, and the demo level

**Authored**, against `gallery/` at `4c0fb85e` (`world.time` `day`; four `set-time` cuts in `quests.json` — `dusk`, `dawn`, `night`, `midnight` — and two in `dialogue.json` — `noon`, `noon`; six design rows; seven cameras, none with a sky).

1. **Bound.** The units are `WorldTime`'s celestial branch — `sun`, `moon`, `phase` — and the six `Position` variants; the eight phase names are data (§3.1.2). The gallery's `world.time` becomes `{"moon": "just-risen", "phase": "new-moon"}` with its design row stating the same clock, and the four quest cuts become celestial statements covering the other five positions (`{"sun": "high"}`, `{"sun": "setting"}`, `{"moon": "high"}`, `{"moon": "just-set", "phase": "waxing-crescent"}`, `{"sun": "rising"}` — one of them on a dialogue option so both effect roots are written), each with the design row `DW0890` demands; one camera states a celestial `sky` so the camera record binds the branch too. Keywords stay bound by the remaining cuts.
2. **Refusal-proven**, one probe each, the primary plus a declared `patch`: `a-moon-named-under-the-noon-sun` (shape 2), `a-night-whose-moon-nobody-named` (shape 3), `a-sky-with-two-bodies` (shape 1), `a-phase-that-restates-the-world` (shape 4), `a-ninth-phase` (`DW0100`).
3. **Perturbation acceptance.** Changing the world's `phase` from `new-moon` to `full-moon` moves the sealing line from `time set 108959` to `time set 12959` and the `sealed_state` PackTest's `day` assertion from 4 to 0; changing `just-risen` to `rising` moves the daytime by 177 ticks and the emitted sun's altitude; the regenerated `gallery/baseline/` moves every row the clock reaches, attributed.
4. **Demo level**, `docs/demo-levels.md` row: **The Stargazer's Roof** — one roofless tower top with a bell. The party arrives under *a new moon just risen* and a sign says so in those words; each ring of the bell cuts the sky to the next sentence on the sign — *a full moon high in the sky*, *the sun just set*, *the sun rising* — and the player looks up and reads the sky against the sentence. The level is where the eye confirms the position table once: that `just-risen` shows the whole disc clear and `just-set` none of it, and that the phase changes with the day count while the hour does not. Its second half is the four refusal transcripts of §6, and the binding lines of §5.5 beside each ring.

## 8. Decisions for the owner

Only what she would write.

1. **The words (§3.1–3.2).** Recommendation: the six positions `rising`, `just-risen`, `high`, `setting`, `just-set`, `below`, and the moon's eight names exactly as the pinned game spells them, kebab-cased — one authority for the phase names, and a position vocabulary a stranger can read off the table. What she sees: these are the words in every `world.json`, every `set-time` and every design row from now on; the sentence *a new moon just above the horizon* is written `{"moon": "just-risen", "phase": "new-moon"}`. The alternative is shorter phase words (`full`, `new`, …) at the cost of a mapping table the engine keeps; the cost of deciding later is that the gallery, the skill and the demo level are written in words that then move.

## Acceptance criteria

Each criterion is checked against the tree at `4c0fb85e`; none is satisfied there except the first sentence of criterion 4 (every keyword campaign's bytes are what they are), which must keep holding.

1. **Surface.** `delvec schema --stage world` exports `time` as a keyword or an object with `sun`, `moon` (the six positions) and `phase` (a string); `--stage quests` and `--stage dialogue` export the same type on `set-time`, `--stage design` on `references[].time`, and `delvec schema cameras` on `sky.time`; `compiler.md`'s field lists hold to the structs by the existing field-list tests.
2. **Vendored data.** `crates/dsl/data/timeline-day-1.21.11.json` and `timeline-moon-1.21.11.json` are byte-identical to the pinned jar's files, `PROVENANCE.md` names the jar by sha256 and the paths, the `tools/maintenance/` regenerator refuses a differing extraction; a `cargo test` asserts the eight phase names in order, the burn keyframes 12542 / 23460 and the sky-light keyframes of §2.5 from the files.
3. **The table is the curve.** A test derives all twelve ticks of §3.2 from the vendored `sun_angle` track, the renderer constants and the disc fraction, and asserts each constant equal to its derivation; a second test asserts `sky_angle_rad` against the closed form at every tick of the day within 0.06° and at the six keywords within 0.056°.
4. **Byte identity and emission.** Every world document in `crates/`, `prefabs/` and `gallery/` that states a keyword builds to the same datapack bytes before and after (the gallery baseline's manifests, compared on the datapack only; the celestial gallery of criterion 9 is excluded by name); a fixture declaring `{"moon": "just-risen", "phase": "new-moon"}` emits `time set 108959` in the sealing baseline and `time set 102000` for a `set-time noon` cut (a keyword cut keeps the world's day 4, §3.1.4) and never `time set noon`; its `sealed_state` PackTest asserts `daytime` 12959, `day` 4 and `predicate <ns>:moon_new-moon`; the emitted command passes the pinned command tree.
5. **Refusals.** Each shape of §6 is refused `DW0931` with its remedy on a fixture, a `remedy_reachability.rs` row per remedy, `tools/ci/check-dw-codes.py` green with the code in `compiler.md` and zero new allowlist entries.
6. **The readers.** `effective_sky` over a clock: 15 on the day plateau, 4 elsewhere, asserted at the six keywords (unchanged) and at 12782, 13047, 23218, 23397 (all 4) and 280 (15); `hour_burns` at the four horizon ticks of §5.2 is false; `DW0890` refuses a `midnight` row against a `{"moon": "high", "phase": "new-moon"}` world — the row's keyword states a sky, so it is day 0, and the message prints both sides as `midnight (day 0, 18000)` and `{"moon":"high","phase":"new-moon"} (day 4, 18000)` — and passes a `{"moon": "high"}` row (the world's phase), both in one test.
7. **Renders.** A camera with a celestial `sky.time` emits `sun_at` of its position's tick; the `sky:` line of `delvec cameras` carries *moon not drawn by the renderer* on every scene whose moon is up; the re-emitted goldens of `clear_scenes_keep_their_base_bytes` are committed with the per-hour altitude deltas in the commit body.
8. **Binding line.** `delvec build` prints §5.5's lines on every build; on an all-keyword campaign it prints `0 celestial` and `days {0}` with `full-moon` spelled on the world's line; asserted in a CLI test.
9. **Gallery** (spec-0039). §7.1's elements are bound and §7.2's five probes refused with the codes named, with the coverage gate's counts on its own line; `gallery/baseline/` is regenerated after the merge commit exists, every moved row attributed.
10. **Determinism.** Two builds of the celestial gallery are byte-identical (existing gate); a test asserts that `{"moon": "rising"}` and `{"sun": "setting"}` resolve to one clock and one emitted line.
11. **Docs and skill, same change.** `compiler.md`: the stage-1 `time` row states both spellings, the position table and the day rule; the `set-time` rows the same; *Environment sealing* the integer form and the two read-backs plus the phase predicate; the diagnostics catalog `DW0931`; `render-plan.json`'s `sky` the spelling. `docs/reference/tools.md` §10 names `tools/spike-celestial-time/ at 4849172ae054`. The `/new-delve` skill's references that name the hour (`workspace.md`, `pitfalls.md`, `visual-review.md`) say a time is a keyword or a body, a position and a phase, and that a keyword night is a full moon. `docs/specs/README.md` gains this spec's row; `docs/demo-levels.md` gains §7.4's row.
