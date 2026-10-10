# spec-0102: A sound beats over a place — a pulse is declared once, sounds from a mark on a fixed interval to every player standing in a place, loudest at the mark, and a story stage turns it on and off

- **Status**: Proposed
- **Ground**: written against engine `507a5e0e2` (`origin/main`), read only — `QuestsContent` and its runtime-object lists in `crates/dsl/src/quest/mod.rs`; `Guard`, `Verb::PlaySound`, `Verb::Sequence`, `Verb::SetAtmosphere` in `crates/dsl/src/quest/`; `Mark` (`crates/dsl/src/mark.rs`); `LethalVolumePlan`/`StagedGate` (`crates/delvec/src/compiler/plan/lethal.rs`), `TimedGatePlan`, `Plan::gate_terms` / `gate_terms_of` (`compiler/plan/state.rs`); `Audience` in `compiler/emit/effect.rs`; the `loops[]`, `timed_gates[]`, `sequence`, `volley`, `play-sound`, `set-atmosphere` and *A scheduled bundle has no `@s`* rows of `docs/reference/compiler.md`; the pinned command tree `crates/delvec/data/commands-1.21.11.json` (`playsound`, `schedule`); the vendored sound-event registry `crates/delvec/data/sounds-1.21.11.json` (1838 ids) and the environment-attribute registry (`audio/ambient_sounds`). The pinned 1.21.11 game, read from the client jar (sha256 `1473c948…6cd3bd`) and the dedicated server's bundled `META-INF/versions/1.21.11/server-1.21.11.jar` (server jar sha256 `f83b8e09…dd1726`, `versions.toml`), disassembled with `javap -c -p` and named through Mojang's official 1.21.11 client mappings (`client.txt`, sha1 `031a68be…bf232a`); the pinned client's `assets/minecraft/sounds.json` (asset index 29, sha1 `123370fb…391dfb`). `PlaySoundCommand` and `ClientboundSoundPacket` are byte-identical in the two jars (class sha256 `ee013693d4ef8e2a…` and `0a15bdd271f14e61…`), so every server-side fact below is the dedicated server's; every client-side fact is the client's.
- **What it is for**: a place that has a heartbeat. The engine plays a sound once (`play-sound`), plays a finite timeline once (`sequence`), and lays a continuous ambience over a place through its atmosphere (spec-0080). None of them is a *rhythm*: a slow beat that sounds through a whole interior on a fixed interval, loudest in one room, that begins when a story beat says so, quickens on a later one, and stops on the last. The first campaign to ask for it needs exactly that; the mechanism is general — a bell that tolls while a siege runs, a drip that counts while a chamber floods, a drum that marches until the party reaches the drummer.
- **Research**: §2 is this spec's research record. Each rule is marked **cited** (a reading of the pinned bytes or data, a reading of the tree, a constitution rule) or **authored** (this spec chooses). Nothing below was run on a server; what the bot tier must measure is named in §5.3.
- **Numbers**: no ADR. **Three DW codes**, allocated by the planner at implementation — **`DW0993`** (a pulse declared against itself, §6.1), **`DW0994`** (a pulse nobody can stand in hearing of, §6.2), **`DW0995`** (advisory: a pulse the forced route never hears, §6.3). The place reference's neither/both shape takes `set-atmosphere`'s existing `DW0929`, a sound id takes `DW0326`, a mark outside its piece takes `DW0897`: no second code for a rule that exists. `dsl_version` moves to the number the planner hands the implementation; it numbers a surface and promises nothing.
- **Non-goals**: a sound file of the campaign's own (a `sounds.json` entry and `.ogg` in the resource pack the delve already serves — a spec of its own, under ADR-0013's allowlist; this spec admits vanilla sound events only, §2.6); a continuous loop (that is `audio/ambient_sounds.loop` on an atmosphere, already first-class — §2.1); occlusion by walls (the pinned client has none, §2.3 — the place is the bound, not the geometry); a pulse one player hears and another does not (§3, `when`); cutting a sample already playing (a beat ends at the next interval; the sample finishes); the random `mood` and `additions` sounds of a biome (§2.1).

## 1. The defect, and the object it belongs to

**Cited** (the tree at `507a5e0e2`): `play-sound` is one `playsound` line at one beat; `sequence` is a finite `schedule` chain whose steps are declared one by one (`at_ticks`), and a nested `sequence` is `DW0329`, so an open-ended repetition cannot be written as one; `volley` repeats on a `schedule` chain but is a trap's projectile pattern. An atmosphere's `audio/ambient_sounds.loop` plays a vanilla loop file without pause at uniform volume wherever the camera stands in the biome (§2.1). So a creator who wants a heartbeat through a hall today writes nothing, or writes a `sequence` of a hundred `play-sound` steps that ends.

**The object a pulse belongs to is a runtime thing the campaign declares and a story stage switches.** The engine already holds that class in stage 5: a `lethal_volumes[]` volume is *live while its gate holds* (spec-0088, `when: Guard`), a `loops[]` loop *holds while its gate holds and stands down when it stops* (spec-0086), a `timed_gates[]` gate runs a *self-sustaining `schedule` ping-pong that costs nothing per tick* (spec-0016 §4). A pulse is all three at once: a declared object with a place, a clock on `schedule`, and the one `Guard` every gate consumer reads through `Plan::gate_terms`. It is not a verb pair (`start-pulse` / `stop-pulse`): a pair re-introduces order — whichever ran last wins — where a gate is a declaration every proof can read at every configuration (spec-0088 §4, the readings `may_live` / `is_live`), and the death plan, the PackTest templates and the emitted guard already share that one reading (**cited**: `Plan::gate_terms`'s doc comment, *the one reduction every consumer takes*).

## 2. Research record

### 2.1 Vanilla's primitives for a sound over an area, ranked

**Cited**, each row read as named.

| Primitive | What it gives | What it cannot give | Verdict |
|---|---|---|---|
| Biome `audio/ambient_sounds.loop` (spec-0080 atmosphere, switched by `set-atmosphere`) | a client-side continuous loop, free, per place; 40-tick linear cross-fade on biome change (`BiomeAmbientSoundsHandler.LOOP_SOUND_CROSS_FADE_TIME`; `LoopSoundInstance.tick`: volume `clamp(fade / 40, 0, 1)`); the `LoopSoundInstance` constructor sets `relative = true`, `looping = true`, `volume = 1` | a rhythm: the period is the sound file's own length and vanilla ships no beat-shaped loop (the five `ambient.*.loop` events are streamed drones); no position — a relative sound is at the ear; a tempo is a second file | **first-class for a texture, already exposed**; not this spec's object |
| Biome `mood` / `additions` | `mood`: one sound after `tick_delay` ticks in a dark enclosed spot, at a random offset (`BiomeAmbientSoundsHandler.tick`, the 15.0 / 0.001 counter); `additions`: `tick_chance` per tick, random | neither is an interval: both are rolls | rejected for a rhythm |
| **A `playsound` the server schedules** (`schedule function … <n>t`) | an interval exact in server ticks; one command per beat, nothing per tick (the `timed_gates[]` and `volley` shape); a positioned source with a range; a per-player audience, so the place is the bound; guarded by the gate's scoreboard terms; one packet per listener per beat | occlusion; a sample longer than the interval overlaps itself | **chosen** |
| Jukebox | a disc at a block, `record` category | one disc, one play; range is the block's; a custom disc is a resource-pack sound | rejected |
| Note block | a pitched tick | needs a redstone edge per beat — a scheduled `setblock` toggling power, which is the hack spec-0022 excluded (*redstone keeps exactly one job — the trigger*) | rejected |
| A warden for its heartbeat | the real `entity.warden.heartbeat` | an entity behaviour used for its side effect | rejected (CLAUDE.md, *no hacks at any layer*) |

### 2.2 What the server does with `playsound`

**Cited — `PlaySoundCommand.playSound` and `SoundEvent.getRange`, bytecode.** The command is `playsound <sound> <source> <targets> [<pos> [<volume> [<pitch> [<minVolume>]]]]`, `volume ≥ 0`, `pitch ∈ [0, 2]`, `minVolume ∈ [0, 1]` (the pinned command tree). The server draws **one** seed (`RandomSource.nextLong`) before the target loop, so every listener of one command hears the same variant of a multi-file event. It squares the event's range — `fixedRange` when the event declares one, else `volume > 1 ? 16 · volume : 16` — and for each target compares the squared distance from the target to `pos`: inside, the packet carries `pos` and `volume`; outside, nothing is sent unless `minVolume > 0`, in which case the packet carries a point two blocks from the target toward `pos` and `minVolume`. The only fixed-range registration in `SoundEvents` is the goat-horn helper; `entity.warden.heartbeat` is registered variable-range (`register(String)`).

### 2.3 What the client does with the packet

**Cited — `ClientPacketListener.handleSoundEvent` → `ClientLevel.playSeededSound` → `ClientLevel.playSound(…, distanceDelay = false, seed)` → `SoundEngine.play`, and `Channel.linearAttenuation`, bytecode.** The packet becomes a `SimpleSoundInstance` at the packet's position, played at once (no distance delay), with `AbstractSoundInstance`'s default attenuation `LINEAR` and `relative = false`. `SoundEngine.play` computes the attenuation distance `max(volume, 1) · Sound.getAttenuationDistance()` — 16 unless the event's `sounds.json` row sets `attenuation_distance` (44 of 1837 events do; `entity.warden.heartbeat` does not) — and the gain `clamp(volume, 0, 1) · categoryVolume · master`: **a volume above 1 does not make a sound louder at its source; it only extends how far it reaches.** `Channel.linearAttenuation(R)` sets OpenAL `AL_DISTANCE_MODEL = AL_LINEAR_DISTANCE`, `AL_MAX_DISTANCE = R`, `AL_ROLLOFF_FACTOR = 1`, `AL_REFERENCE_DISTANCE = 0`: a listener at distance `d ≤ R` hears gain **`1 − d / R`**, and `0` beyond. Nothing in the chain reads a block between source and listener: there is no occlusion.

So "loudest in one room, heard through the interior" is, in the pinned game, exactly one positioned source with a range and an audience: at the source the gain is 1, at distance `d` it is `1 − d/R`, and whoever is not addressed hears nothing.

### 2.4 Time

**Cited.** `schedule function <f> <n>t` runs `<f>` after `n` server ticks with the server command source (`compiler.md`, *A scheduled bundle has no `@s`*); `replace` mode re-arms rather than doubling (`timed_gates[]` row). A server tick is the unit the whole world moves in: every listener receives a beat's packet in the same tick, and lag stretches the interval for everyone alike. There is no drift to model; an interval is a count of ticks.

### 2.5 What a `Guard` is, read at every consumer

**Cited.** `Guard {requires_flags, forbids_flags, requires_state}` is one object, declared once and carried by every effect; `lethal_volumes[].when` carries it verbatim (spec-0088 §3.1); a loop carries its three fields flat (spec-0086). Flags are party state and never cleared; a `requires_state` term on a `player`-scoped datum is refused on a loop (`DW0949`) and on a volume (`DW0953`) because the thing's liveness is a fact about the place, not about one body. `Plan::gate_terms` reduces any gate to the `GateTerm` rows the emitted `execute` guard, the PackTest drive and `death-plan.json` all read.

### 2.6 The sounds a pulse may name

**Cited.** `DW0326` validates a sound id against `crates/delvec/data/sounds-1.21.11.json`, the pinned `sound_event` registry (1838 ids). A heartbeat exists there: `entity.warden.heartbeat`, four files (`mob/warden/heartbeat_1..4`), subtitle `subtitles.entity.warden.heartbeat` — a player with subtitles on reads *Heartbeat* at every beat, which a creator chooses knowingly. The delve already serves a resource pack (textures, skins, the art font); adding a sound event to it is the same shape as spec-0084's texture rows and carries ADR-0013's provenance obligation; it is a spec of its own, and until it lands the one registry `DW0326` reads is the whole vocabulary.

## 3. The surface

**Authored.** Stage 5 (`quests.json`) gains one list beside `loops[]`, `lethal_volumes[]` and `timed_gates[]`, the other runtime things a story stage switches:

```json
"pulses": [
  {
    "id": "pulse/the-heart",
    "sound": "entity.warden.heartbeat",
    "at": { "anchor": "hall/hearth", "offset": [0, -1, 0] },
    "place": "area/hall",
    "every": 30,
    "floor": 0.4,
    "when": { "requires_flags": ["flag/door-shut"], "forbids_flags": ["flag/heart-found"] }
  },
  {
    "id": "pulse/the-heart-racing",
    "sound": "entity.warden.heartbeat",
    "at": { "anchor": "hall/hearth", "offset": [0, -1, 0] },
    "place": "area/hall",
    "every": 14,
    "floor": 0.4,
    "pitch": 1.2,
    "when": { "requires_flags": ["flag/hearthstone-lifted"], "forbids_flags": ["flag/heart-found"] }
  }
]
```

1. **`id`** — `pulse/<kebab>`, unique (`DW0993`).
2. **`sound`** — a pinned sound event, `minecraft:` optional, through the one registry `DW0326` reads.
3. **`at`** — a `Mark` (spec-0066): the source, the anchor's cell plus `offset`, bound to its piece by `DW0897` as every mark is. The sound stands at the cell's centre. The mark need not lie inside the place it is heard in: a heartbeat from beyond a wall is a design, and the range is measured from wherever the source stands.
4. **Where it is heard** — exactly one of **`region`** (`{anchor, extent}`, the anchor-centred box every volume verb takes, resolved through `Plan::zone_box`) or **`place`** (an `area/…` or site-plan `node/…` id, resolved to the place's bounds). This is the pair `set-atmosphere` already takes, in the same words; the implementation extracts it as one type (`PlaceRef`) both carry, with `DW0929` as its neither/both refusal wherever it is read — a shared rule is extracted, never copied (CLAUDE.md — **cited**). `set-atmosphere` grows the bounds by the biome blend's reach because a paint must; a pulse takes the bounds as they are.
5. **`every`** — the interval in server ticks, `≥ 1` (`DW0993` at 0: `schedule … 0t` is the same tick again, forever).
6. **`floor`** — required, `0 ≤ floor < 1`: how loud the beat is at the farthest cell a body can stand in inside the place, as a fraction of the loudness at the source. The creator's judgement; the range is derived from it (§4.1). There is no default: a number nobody declared would be invented (CLAUDE.md, *researched, never invented* — **cited**).
7. **`pitch`** — optional, default `1.0`, within the command tree's `[0, 2]` (`DW0993` outside it).
8. **`when`** — optional, the `Guard` type verbatim, as on a lethal volume. Absent, the pulse beats from world load to the end. Present, it beats **while the gate holds**: the first beat sounds on the tick the gate opens, every `every` ticks after, and the chain stops at the first interval on which the gate no longer holds. `when: {}` and a `requires_state` term on a `player`-scoped datum are `DW0993`, for the reasons spec-0086 §3.2 and spec-0088 §3.2 give.

**Tempo is a second declaration.** A beat that quickens is two pulses on one source with disjoint gates (the example above: the slow one forbids the flag the fast one requires). One mechanism — the gate — does what a `tempo` field keyed to a flag would do, and a second gating surface would be the defect (CLAUDE.md, *a second bespoke field is the defect* — **cited**). Two pulses may share a source and a sound freely; what they may not share is an id.

**A pulse is a party fact.** It addresses every player standing in the place, never an actor, so it takes no `audience`; a player watching a cutscene (`dw_cutscene`) is not addressed, because the camera is not where the body is — the selector the loop and the boundary clock already write.

## 4. Emission

### 4.1 The derived range

**Authored, on §2.3's cited gain.** For a pulse, the compiler reads the standable cells of the place (the nav model's reading, `World::standable_fp`, the same cells every walk proof stands a body on), takes the standing eye of each (the eye the view proofs already judge from), and measures `far` = the greatest distance from the source point to any of them. The range is **`R = far / (1 − floor)`**, so the farthest standing ear hears exactly `1 − far/R = floor`, and the emitted volume is **`V = max(R / 16, 1)`** — vanilla's range is `16 · V` for `V > 1` and 16 below it, so a place smaller than sixteen blocks hears at least its floor and the line says so. `far`, `R` and `V` are printed per pulse (§5.1) and written to the ledger. A place with no standable cell has no `far` and is `DW0994`.

### 4.2 The lines

**Authored.** One function per pulse, `pulse_<s>` (`<s>` = `safe_local(id)`), on the one holder `#pulse_<s> dw.sys`:

```
scoreboard players set #pulse_<s> dw.sys 1
execute <terms> run playsound <sound> master @a[<box>,tag=!dw_cutscene] <x> <y> <z> <V> <pitch>
execute <terms> run schedule function <ns>:pulse_<s> <every>t replace
execute unless <term₁> run scoreboard players set #pulse_<s> dw.sys 0
… one clearing line per term
```

`<terms>` are `Plan::gate_terms` over `when`, each rendered by `GateTerm::clause`, the formatter every guard is written by; `<box>` is the place's box through `emit::box_selector_args`, the formatter the loop's slab takes; `<x> <y> <z>` is the mark's cell centre; `V` and `pitch` are written by the engine's one float formatter. The category is `master`, as `play-sound` writes it (one rule for every sound the engine plays). The clearing lines are the trap gate's shape — *not (every term holds)* is a disjunction, one line per term (`compiler.md`, trap `requires_flags` row — **cited**). For an **ungated** pulse the function is the `playsound` and the `schedule` line alone, and `setup_finish` calls it once; the holder, the latch line and the clearing lines are not emitted.

For a **gated** pulse, `setup_finish` writes `scoreboard players set #pulse_<s> dw.sys 0` and the `tick` function gains one line — `execute <terms> unless score #pulse_<s> dw.sys matches 1 run function <ns>:pulse_<s>` — the open edge: the first beat sounds on the tick the gate opens, and the chain carries itself from there. When the gate stops holding, the next scheduled call plays nothing, schedules nothing and clears the latch, so the chain is dead within one interval and the tick line re-arms it on the next opening. `schedule … replace` makes a second chain impossible by construction. A campaign that declares no pulse emits none of this and is byte-identical.

Every line is walked against the pinned command tree as every emitted line is. **Determinism** (ADR-0006 — **cited**): `far` is a maximum over a set the build already orders, `R` and `V` are `f64` arithmetic on it, the function order is declaration order; two builds are byte-identical, asserted by the existing double-build gate once the gallery declares a pulse.

## 5. Proving it reaches the party

### 5.1 The binding line and the ledger

**Authored.** Every build that declares a pulse prints `pulse binding: N pulse(s), S staged; far F₁..F₂ block(s) over C₁..C₂ standable cell(s), range R₁..R₂ (volume V₁..V₂); L of N live on the forced route in some configuration; H of N with a listening station` — every figure computed from the declarations with `N` as the denominator, zeroes included; a build with none prints `0 pulse(s)`, a measured zero. `validation/pulses.json` carries one row per pulse: id, sound, source point, box, `every`, `floor`, `pitch`, `far`, `R`, `V`, `gate_terms` (the `GateTerm` rows), and `stations` (§5.3).

### 5.2 PackTest

**Authored.** Per gated pulse, two synchronous templates, beside the lethal volume's: `pulse_<s>` drives the gate **open** (every required flag set, every forbidden flag reset, every numeric datum to the value `DatumSet::pick` chooses), runs the tick line's dispatch, asserts `#pulse_<s> dw.sys` is 1, then `schedule clear <ns>:pulse_<s>` so no chain outlives the template; `pulse_<s>_shut` sets the latch to 1, shuts the gate by exactly one term (the complement `lethal_<id>_shut` chooses), runs `pulse_<s>`, and asserts the latch is 0 — a stripped guard reds the first, a stripped clearing line reds the second. Per ungated pulse, one template asserts `setup_finish` reaches `pulse_<s>` and clears the schedule after. What a PackTest cannot observe is said: the `playsound` is a client-bound packet; the command tree walk proves the line is well-formed, and the bot hears it.

### 5.3 The bot

**Authored, on mineflayer 4.37.1's `soundEffectHeard (name, position, volume, pitch)`** (`lib/plugins/sound.js` at that tag — **cited**). The ledger's `stations` are computed by the compiler from the forced route: for each pulse, the first route cell inside its box whose arrival configuration has the pulse `is_live` (spec-0088 §4.1's reading, applied to this gate), and, where the route has one, the first cell outside the box after it under the same configuration. At a listening station the harness stands `2 · every + 10` ticks and asserts at least two `soundEffectHeard` events naming the sound, each at the source point (±0.5) with volume `V` and pitch `pitch` — the derived numbers, heard back; at a silent station it stands as long and asserts none. A pulse with no listening station is reported `not_heard: no station`, a stated fact and never a pass, and `DW0995` has already said so at build time. The event listener goes through the harness's existing packet reading; no new live command is spoken.

## 6. Refusals

**Authored**, each raised at the document it is entered in, with the remedy named and a `remedy_reachability.rs` row each.

### 6.1 `DW0993` — a pulse declared against itself (validation tier, `dsl::validate`)

- `every` of 0; `floor` outside `[0, 1)`; `pitch` outside `[0, 2]`;
- `when` present and empty; a `requires_state` term naming a `player`-scoped datum;
- a duplicate `id`.

The sound id is `DW0326`; neither or both of `region` / `place` is `DW0929`; a mark outside its piece is `DW0897`.

### 6.2 `DW0994` — a pulse nobody can stand in hearing of (build tier, `compiler::pulse`)

- the place's box holds no standable cell: `far` does not exist and the binding would be a zero (CLAUDE.md, *a zero binding is a finding* — **cited**). The message names the box and the nearest standable cell outside it; the remedy is the box.

### 6.3 `DW0995` — a pulse the forced route never hears (advisory)

- the forced route passes no configuration in which the pulse `is_live` while a route cell lies in its box: the ladder cannot exercise it (the shape of `DW0954`). The message names the pulse and the term that never held, or that no route cell lies in the box. None required: a beat the party need never hear is a design, and the report says it was not heard.

## 7. The gallery, and the demo level

**Authored**, against `gallery/` at `507a5e0e2` (`area/hall`, `area/annex`, `area/long-hall`; three staged and unstaged volumes; three timed gates; two loops).

1. **Bound.** `quests.json` declares three pulses. Two share a source under the hall's floor and `entity.warden.heartbeat`: the slow one (`every: 30`, `floor: 0.4`, `place: area/hall`) requires a flag a forced objective sets and forbids a later forced flag; the fast one (`every: 14`, `pitch: 1.2`) requires the later flag — the tempo idiom, bound. The third is ungated, in the annex, with a `region` and another sound, binding the `region` spelling and the no-`when` path. Every unit — `pulses[].{id, sound, at, region, place, every, floor, pitch, when}` — is written, and the coverage gate (`delvec schema --stage all`) binds them; `Guard`'s fields are enumerated once at `Guard` (`gallery_units.py`).
2. **Refusal-proven**, one probe each, the primary plus a declared `patch`: `a-beat-with-no-interval` (`every: 0`) → `DW0993`; `a-floor-as-loud-as-the-source` (`floor: 1.0`) → `DW0993`; `a-beat-one-player-hears` (a `player` datum in `when`) → `DW0993`; `a-beat-heard-both-here-and-there` (`region` and `place`) → `DW0929`; `a-beat-in-solid-rock` (a `region` inside the hall's wall) → `DW0994`; `a-beat-the-game-never-heard` (an unregistered sound) → `DW0326`.
3. **Perturbation acceptance.** Changing `every` moves the `schedule` line; changing `floor` moves `V` on the `playsound` line and `R` on the binding line; moving `at` by one cell moves the position; removing `when` removes the tick line and the holder; moving the source one cell farther from the hall's far wall moves `far`.
4. **Demo level**, `docs/demo-levels.md` row: **The Heart Under the Floor** — a small house of three rooms; a slow heartbeat sounds from under the hearth and is heard in every room, loudest at the hearth; it begins when the party shuts the door behind them, quickens when they lift the hearthstone, and stops when they find what beats. The level is the owner's first listen to `floor` (does the far room hear what the number promised), to the open-edge first beat, and to the cut at the next interval.

## 8. Decisions for the owner

Only what she would hear.

1. **A pulse is a declared thing with a gate, not a start/stop verb pair.** What she hears: a beat begins on the tick its stage is reached and stops within one interval of the stage that ends it, whichever order the beats fire in; a quickening is a second pulse. The alternative — two verbs — hears the same and can be fired in the wrong order.
2. **The far edge's loudness is her number; the reach is derived.** She writes how loud the farthest room is (`floor`); the engine computes the range and prints it. The alternative — writing a radius in blocks — is the number vanilla takes and nobody can hear.
3. **The place is the bound, not the walls.** A player who steps out of the declared place hears no next beat; a player inside hears it through every wall. The pinned client has no occlusion; the alternative is to pretend it does.
4. **Vanilla sounds only, for now.** The heartbeat is the warden's; a campaign's own sound file is a later spec under the license rules.

## Acceptance criteria

Each criterion is checked against the tree at `507a5e0e2`; none is satisfied there. `delvec` is this tree's binary, built from its manifests.

1. **Surface.** `delvec schema --stage quests` exports `pulses[]` with `id`, `sound`, `at`, `region`, `place`, `every`, `floor`, `pitch`, `when` and nothing else; `set-atmosphere` and a pulse name one `PlaceRef` type (`grep` over `crates/dsl/src/` finds one declaration of the `region` / `place` pair); `compiler.md`'s field lists hold to the structs in both directions by the existing field-list tests.
2. **Emission.** For a campaign declaring a gated pulse, `pulse_<s>` carries exactly the lines of §4.2 in that order, `setup_finish` seeds the holder, and `tick` carries the one edge line; for an ungated pulse `pulse_<s>` is two lines and `setup_finish` calls it; a campaign declaring none emits no function, no holder, no tick line and no ledger; every line passes the pinned command tree.
3. **The derived range.** A test builds a place whose farthest standable eye is `far` blocks from the source and asserts `V = max(far / (16 · (1 − floor)), 1)` on the emitted line for three `floor` values, and that a source outside the box is measured from where it stands.
4. **Determinism.** Two builds of the gallery are byte-identical (existing gate); a test perturbs `floor`'s spelling (`0.4` vs `0.40`) and asserts the emitted bytes do not move.
5. **Refusals.** Each shape of §6.1–6.3 is refused with its code and remedy by a fixture under `crates/delvec/tests/`, with a `remedy_reachability.rs` row each; `DW0929` fires on a pulse's neither/both; `tools/ci/check-dw-codes.py` is green with the three codes in `compiler.md`.
6. **Binding line and ledger.** `delvec build` prints §5.1's line on every build (`0 pulse(s)` for a campaign with none, asserted in a CLI test); `validation/pulses.json` has the row fields of §5.1 for every declared pulse.
7. **PackTest.** The two templates of §5.2 are generated per gated pulse and one per ungated pulse, each ending in `schedule clear`; the generated set is bound by `DW0811`'s claims as every generated template is.
8. **The bot.** `harness/` reads `validation/pulses.json`, listens at every listening station and silent station, asserts the counts, positions, volume and pitch of §5.3, and reports `not_heard: no station` per pulse without one; the gallery's slow pulse has a listening station and is heard on the gallery's run.
9. **Gallery** (spec-0039). §7.1's three pulses are bound and §7.2's six probes refused with the codes named, with the coverage gate's counts on its own line; `gallery/baseline/` is regenerated after the merge commit exists with every moved row attributed.
10. **Docs and skill, same change.** `compiler.md`: the stage-5 row for `pulses[]`, the emission row beside `loops[]`, the three codes in the diagnostics catalog, the binding line and the ledger under *World / build output*; the `/new-delve` skill's `references/quest-capabilities.md` says a beat that repeats is a pulse — declared once, heard in a place, loudest at its mark, switched by its gate, quickened by a second declaration — under *Things that change the world*; `docs/demo-levels.md` gains §7.4's row; `docs/specs/README.md` carries this spec's row.

### Recorded debts

Checked against the implementing tree; each is a debt, never a pass.

- **Criterion 1, third clause.** No test holds `compiler.md`'s stage field lists to the structs in either direction at this revision: the "existing field-list tests" do not exist. The `pulses[]` row is written by hand to match `Pulse`, and `crates/delvec/tests/pulse.rs` holds the exported schema to exactly the nine fields; nothing binds the doc row.
- **Criterion 10, the skill.** `tools/ci/check-skill-page.py` refuses a page naming `DW0993`–`DW0995` while the page pins an engine release that lacks them. The `references/quest-capabilities.md` text is held back until the release that carries this surface is pinned.
- **Criterion 9, the baseline.** Regenerated on the feature branch against its merge base; the regeneration after the merge commit belongs to the integration that lands it.
- **§4.1, an event with its own `attenuation_distance`.** The vendored sound registry carries ids only, so the 44 events whose `sounds.json` row sets `attenuation_distance` are derived as if it were 16; for them `floor` is not what the far ear hears. Recorded in `compiler.md`'s `pulses[]` row.
