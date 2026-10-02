# spec-0085: A perception bundle — timed screen and sound effects played to an audience: the audience belongs to every player-facing effect, a timeline keeps its actor, a particle is an effect, and a sound stands where the listener's body says

- **Status**: Approved
- **Ground**: written against engine `c0f22c51` (`origin/main`), read only —
  the stage-5 effect vocabulary (`Verb`, 38 variants by `delvec schema --stage
  all` built from this tree, 542968 bytes), the effect envelope
  (`QuestEffect {when, happening, verb}`), `EnvTrigger.audience` /
  `TriggerAudience`, `SoundAt`, `SequenceStep`, `emit::Audience` and
  `root_audience`, `sequence_fns`, `emit_play_sound`, `effect_selector`,
  `effect_give_command`, `tests/scheduled_executor.rs`, `compiler::lethal`
  (`DW0891`, `posted_places`, the keep-out), `nav::World::body_moves`,
  `dsl::metrics` (`WALK_SPEED_BLOCKS_PER_SECOND`, `FALL_DAMAGE_ONSET_BLOCKS`,
  `unarmoured_survivable_fall_blocks`), the pinned command tree
  `crates/delvec/data/commands-1.21.11.json`, the findings-ledger row whose
  general form is that a granted sight effect outlasts any authored camera it
  can overlap, the gallery (`give-effect` once, `play-sound` five times,
  `sequence` once in `gallery/quests.json`), the harness (`harness/src`, 30
  files, 0 of them naming blindness, darkness or nausea) — and against the
  eldritch spike, branch `research/eldritch-visuals` at `2bbb1f28`,
  `tools/spike-eldritch-visuals/` (README, `gen.py`, `observations.json`),
  whose station 3 is the thing this spec makes authorable. Nothing here was
  measured on a server by this spec; the spike's readings are cited as the
  spike's, and the demo level (§9) is where the rest is looked at.
- **What it is for**: a beat where the world goes wrong for a moment — the
  screen darkens and swims, a face fills it, something sounds from behind —
  written as one timeline of effects the vocabulary already has plus the two
  it lacked, played to the player who caused it, to everyone in a place, or
  to everyone, and refused where a blinded player could walk into a death.
- **Research**: the game facts consumed are the Minecraft Wiki's pages
  *Blindness*, *Darkness*, *Nausea*, *Commands/effect*, *Commands/particle*,
  *Coordinates*, *Options* and *Options.txt*, read for this spec and marked
  **cited** where used; the spike's `observations.json` is cited as a
  measurement; everything built on them is **authored**. One claim is from
  memory and is named as such (§7.3, the particle setting).
- **Numbers**: no ADR. **Four new DW codes** — `DW0941` (§4.3, an unknown
  particle), `DW0942` (§3.3, an audience on a party fact), `DW0943` (§6, the
  blind reach), `DW0944` (§5.3, a sight under a camera); `DW0503` gains a
  fourth shape and no number (§3.3). **`dsl_version` does not move**: the
  envelope gains two fields, the vocabulary gains a verb, a sound origin
  gains an offset, under the number the surface already carries.
- **Non-goals**: a `perception` verb (§2 says why there is none); a particle
  that takes options (`dust`, `block`, `item`, …, §4.3); a sound category
  other than `master` (§4.4); the player's own volume and video settings,
  which the engine cannot read (§7); a runtime check of where a player
  stood when a blinding landed (§6 is static); any content work.

## 1. The defect

**Cited** (the tree at `c0f22c51`, read by `delvec schema --stage all`): the
export carries the word `particle` nowhere and `elder_guardian` nowhere;
`audience` appears once, on `triggers[]`; `in` appears six times — the same
box filter on three verbs (`give-effect`, `clear-effect`, `damage-players`),
in each of the two stage documents that carry effects; `at_ticks` is the
`sequence` step's. **Cited** (`emit::sequence_fns`): every step of a
`sequence` is emitted under `Audience::Scheduled`, so its player-facing
effects address `@a` wherever the timeline was started from —
`docs/reference/compiler.md` §4 states it as *a sequence is a global
timeline*. **Cited** (the spike, `perception/hit.mcfunction` and
`perception/second.mcfunction`): station 3 is `effect give @s darkness 8 0
true`, `effect give @s nausea 10 0 true`, a forced `elder_guardian` particle
at the player, two sounds, then **fifty ticks later** a `blindness 2 0 true`,
a second face and a second sound to **the same player**, carried across the
`schedule` by a tag the hit set, and sixty ticks after that a sound
`positioned ^ ^ ^-3` behind that player.

So the three things station 3 does that the vocabulary cannot are: play a
particle; time a beat to one player rather than to the party; put a sound in
the listener's own frame. The status effects and the sounds themselves are
already verbs.

## 2. There is no `perception` verb — the capability belongs to four object classes

**Authored**, against `CLAUDE.md`'s *This is a general engine* paragraph: *a
capability belongs to the object class it acts on, not to the verb that first
needed it; a second bespoke field is the defect.*

A perception bundle is a **`sequence`** whose steps hold **`give-effect`**,
**`particle`** and **`play-sound`** effects, addressed by the **envelope's
audience**. Each of the four things the spike needed is a property of the
object it is a property of, and each is then available to every verb of that
class, not only to the horror beat:

| the spike's need | the object class it belongs to | what this spec does there |
|---|---|---|
| timing inside the bundle | the timeline (`sequence`) | nothing new in shape; the timeline **keeps its actor** (§3.2) |
| the player who acted / everyone in a region / everyone | the effect envelope (`QuestEffect`) | `audience` and `in` move onto the envelope, for every player-facing verb (§3.1) |
| a full-screen face | a one-shot point effect, the next member after `firework` (spec-0068 §7) | the `particle` verb, at a mark or at each player (§4.3) |
| a sound behind the player | the sound's origin (`SoundAt`) | `players` gains an `offset` in the listener's frame (§4.4) |
| darkness, nausea, blindness | the status-effect grant (`give-effect`) | nothing new in shape; the grant owes the blind-reach proof wherever it is written (§6) |

A `perception` verb would have been the shape the constitution names as the
review defect: a general mechanism — audience, timing, a point effect —
privately re-implemented inside one verb, reachable from nowhere else. The
spike's own `gen.py` writes the bundle as four plain commands and one
`schedule`; the DSL writes it the same way.

## 3. The audience belongs to the envelope

### 3.1 What exists, in three pieces

**Cited.** Who a bundle addresses is decided today in three places, none of
them on the effect:

1. **The root's dispatch** (`emit::root_audience`, bound to
   `EffectRootKind::runs_with_acting_player`): `Party` for an objective's or
   quest's completion (entered as the completing player, player-facing
   effects address `@a`), `Solo` for `on_death`, a dialogue `on_respawn`, a
   shop offer and an `on_kill` (`@s` throughout), `Scheduled` for a trigger,
   a trap payload and a shortcut's `on_unlock` (no `@s` at all).
2. **A trigger's own field** (`audience: party | presser`, DSL v0.11): a
   `presser` trigger is dispatched by the interaction advancement and runs as
   the clicking player.
3. **A per-verb box** (`in {anchor, extent}`) on exactly three verbs,
   narrowing the audience to players inside it.

The spike's three audiences map onto these as: *the player who acted* is
`@s` where the root has one and inexpressible under `Party` (an objective
completion cannot address its completer alone); *everyone in a region* is
`in`, available to three verbs and not to a sound or a particle; *everyone*
is the default. Two of three exist as mechanisms, each bound too narrowly to
reach the objects a perception beat is made of.

### 3.2 The surface

**Authored.** Two optional fields on `QuestEffect`, beside `when` and
`happening`, so that on the wire they stand in the same object as the verb's
own fields — the three verbs that carry `in` today are byte-identical:

```json
{ "type": "particle", "particle": "minecraft:elder_guardian", "at": "players",
  "audience": "actor",
  "in": { "anchor": "anchor/plate", "extent": [2, 1, 2] } }
```

- `audience` — `party` or `actor`. **Default: the root's own answer** (§3.1
  item 1, with a `presser` trigger answering `actor`), so every campaign
  written before this field is byte-identical and the field is written only
  where the beat differs from its root. `actor` is the one player whose act
  fired the root: the completing player, the presser, the dying or
  respawning player, the buyer, the credited killer.
- `in` — the same `{anchor, extent}` box the three verbs carry today,
  resolved through the one `Plan::zone_box`; the audience narrowed to players
  standing in it at the moment the effect fires. Composes with `audience`:
  `actor` + `in` is the actor if they stand in the box.

**A timeline keeps its actor.** The spike carried its player across the
`schedule` with a tag, and so does the engine. A `sequence` started from a
root that has an actor tags that player (`dw_seq_<root>_<n>`, the timeline's
own function name) in its start function; each scheduled step is dispatched
`execute as @a[tag=dw_seq_<root>_<n>] run function <ns>:seq_<root>_<n>_<i>`,
so inside the step `@s` is the actor and the step is emitted under the root's
audience; the last step removes the tag. A timeline started from a
`Scheduled` root has no actor to carry and is emitted exactly as today. Two
consequences, stated:

1. **The seam narrows.** `DW0357` and `DW0503` say a `sequence` step drops
   the actor; under this spec it does so only under a `Scheduled` root, and
   `move-npc`/`move-actor` `on_arrive` remains the seam everywhere. Both
   codes re-derive their shape (2) from `EffectRootSite::runs_with_acting_player`
   and the step's new dispatch; neither asserts less — *no `@s` where emission
   has none* is still the whole claim — so this is not a loosening.
   `tests/scheduled_executor.rs` holds as written: its own rule treats
   `execute as … run function` as a re-binding of the executor.
2. **The timeline stays global.** `schedule` is replace-mode (the pinned tree
   carries `append` and `replace`; the emitter writes neither and vanilla's
   default is replace — cited, `Commands/schedule` by way of the record's
   `timed_gates[]` row), so a second start before the first ends re-times the
   chain for both tagged players. That is today's property with a tag on it,
   and the skill says so (§8).

A timeline shared by two roots of different actor-bearing (today: one
function, first declaration wins) is keyed by its value **and** whether it
carries an actor, so the two forms never share a body.

### 3.3 What is refused

**Authored.**

- **`audience: actor` where the root has no actor** — a `Scheduled` root, or a
  `sequence` step under one — is `DW0503`'s rule (*no `@s` where emission has
  none*) in a fourth shape: an actor-addressed effect, beside a
  `player`-scoped datum. One code, because the remedy is the same sentence:
  move the beat onto a site a player drives, or address the party.
- **`audience` or `in` on a party-fact verb** — a verb the emitter classifies as
  emitted bare because it fires once for the world (`docs/reference/compiler.md`
  §4's list — `set-flag`, the gates, `set-block`, the regions, waves, actors,
  NPCs, `cutscene`, time and weather, `set-checkpoint`, `bonfire`, stealth,
  `sequence` itself, `campaign-complete` — and the verbs that joined the class
  after it was written: the volume-selected `teleport`, `volley`, `collapse`,
  `firework`) — is `DW0942`,
  validation tier (exit 1), `dsl::validate`, naming the verb and the field.
  A box has no party and a world fact has no audience; a `sequence` carries
  none because its steps each state their own. The classification is read
  from the emitter's own exhaustive match, never restated in the validator.

## 4. The effects

### 4.1 Status effects: nothing new in shape, and how they end

**Cited** (`Verb::GiveEffect`, spec-0031): `give-effect {effect, seconds,
amplifier?, hide_particles?}` grants any pinned-1.21.11 status effect;
`seconds` is required, `1..=50000`, and **a duration is how a grant ends** —
there is no infinite form, and pairing a grant with a `clear-effect` of the
same effect in one bundle is `DW0540`. A perception bundle's length is
therefore `max(at_ticks + 20 × seconds)` over its grants, plus the longest
shot of any cutscene in it (§5.1), and nothing in it needs a step to end.

**Cited** (*Commands/effect*): a second grant of an effect the player already
has succeeds only with a higher amplifier or a longer duration; a same-level
grant with a shorter remaining duration fails. A bundle re-armed inside its
own length re-applies nothing visible, which is told to the creator (§8) and
not checked: the engine reads no command's response at runtime.

**Cited** (the spike's `observations.json`): at 0.5 s the bot carried
`darkness` with 148 ticks left and `nausea` with 188, both `show_particles:
0b`; at 3.5 s `blindness` with 15 ticks left beside them — the 50-tick
`schedule` landed on the tagged player. `hide_particles: true` is what the
spike wrote on all three and is what the skill recommends (§8): with it the
swirl and the HUD icon are gone, and the beat reads as the world rather than
as a potion.

**Cited**, per effect, from the wiki — what the screen does, which is what §6
turns on:

| effect | on screen | hides the floor | movement |
|---|---|---|---|
| `blindness` | thick black fog; only the immediate area visible | yes | **cannot sprint**, no critical hits |
| `darkness` | fog; brightness pulses every few seconds between complete darkness and roughly 14 blocks of sight | yes, at the trough | unchanged |
| `nausea` | the view warps and wobbles, ramping up and down at the start and end; purely visual | no | unchanged |

### 4.2 Sounds: nothing new in shape

**Cited** (`Verb::PlaySound`, spec-0014): `play-sound {sound, at?, volume?,
pitch?}`, the id validated against the pinned sound registry (`DW0326`),
emitted in the `master` category, at an anchor or at each listener. The
spike's three sounds (`entity.elder_guardian.curse`,
`entity.warden.heartbeat`, `entity.warden.nearby_closest`) are ids in that
registry and need nothing but §4.4.

### 4.3 A particle is an effect

**Authored**, on spec-0068 §7's sentence: *`particle` is the next one-shot
point effect and takes the same mark … a verb, a mark, an emitted vanilla
command, a proof about what the primitive reaches.*

```json
{ "type": "particle", "particle": "minecraft:elder_guardian", "at": "players" }
{ "type": "particle", "particle": "minecraft:soul", "count": 40,
  "at": { "anchor": "anchor/well", "offset": [0, 1, 0] },
  "spread": [1.0, 0.5, 1.0], "speed": 0.02 }
```

- `particle` — a vanilla particle id, `minecraft:` prefix optional, validated
  against a pinned registry `crates/delvec/data/particles-1.21.11.json`
  (provenance recorded in `PROVENANCE.md` beside the sound registry's), the
  spike's `elder_guardian` among them. An unknown id, or an id of a particle
  type that **takes options** (`block`, `block_marker`, `dust`,
  `dust_color_transition`, `dust_pillar`, `entity_effect`, `falling_dust`,
  `item`, `sculk_charge`, `shriek`, `vibration`, `trail`, `tinted_leaves`,
  `block_crumble` — authored from memory, pinned from the wiki by the test
  that lands the registry, which marks each entry), is
  `DW0941`, validation tier (exit 1), `dsl::validate`.
  A particle with options is excluded until the registry says what each
  takes; none of this spec's needs has one.
- `at` — the mark the particle is spawned at (spec-0066's `Mark`, the same
  type `firework` and `play-sound`'s anchor form take), **or** the literal
  `players`: spawned at each addressed player's own position. The face is
  the second form: the client draws `elder_guardian` over the whole view of a
  player it is sent to, so its position is wherever that player is.
- `count` (default 1), `spread` (`[x, y, z]` standard deviations, default
  `[0, 0, 0]`), `speed` (default 0) — vanilla's `<count>`, `<delta>` and
  `<speed>` [cited — *Commands/particle*]; `count: 0` is refused by the
  schema (`minimum: 1`): it is vanilla's spelling of *one particle with a
  velocity*, a second meaning for one field.

**Emission.** One command per effect, always in **`force` mode**, the
viewers the effect's audience selector:

```
particle minecraft:elder_guardian ~ ~ ~ 0 0 0 0 1 force @s          (at: players; wrapped execute as <who> at @s run …)
particle minecraft:soul <x>.5 <y> <z>.5 1 0.5 1 0.02 40 force <who>  (at: a mark)
```

`force` is written, never chosen per beat, for a cited reason: in `normal`
mode a particle is sent to players within 32 blocks and *may not be shown if
the Particles option is Minimal*; in `force` mode it is sent within 512
blocks and *always shown even if the option is Minimal* [cited —
*Commands/particle*]. An authored beat is not ambience; a creator who writes
a particle means it to be seen, and the knob that could lose it is not
offered. The walk against the pinned tree is the tree's own:
`particle/name/pos/delta/speed/count/force/viewers` is a node in
`commands-1.21.11.json` (cited — read by this spec).

### 4.4 A sound stands where the listener's body says

**Authored.** `SoundAt::Players` gains `offset: [x, y, z]`, default
`[0, 0, 0]`, in **the listener's local frame**: `+x` to the listener's left,
`+y` up, `+z` the way the listener faces [cited — *Coordinates*, local
coordinates]. The pitch is flattened so *forward* is horizontal and *behind*
stays at ear height whatever the player is looking at: the emitter writes
`execute as <who> at @s rotated ~ 0 positioned ^<x> ^<y> ^<z> run playsound
<sound> master @s ~ ~ ~ <volume> <pitch>`. The spike's `^ ^ ^-3` is
`offset: [0, 0, -3]`.

Two rules about it:

1. **The listener inside the loop is `@s`, never the audience selector.** The
   spike's own `third.mcfunction` plays to `@a[tag=…]` from inside `as
   @a[tag=…]`, which with two tagged players plays each sound twice, once at
   each player's behind. The emitter's existing listener-relative form
   (`execute as <who> at @s run playsound … @s ~ ~ ~`) already has this right
   and the offset form is that line with two more clauses.
2. **Integer blocks**, as spec-0066's offsets are: a sound three blocks behind
   is as exact as the ear needs, and a second offset type for one idea is the
   defect the mark type was made to end.

`play-sound`'s `at: {anchor, offset}` is unchanged; `at: {actor}` stays
`DW0335`.

## 5. Timing: how the bundle composes with `sequence` and `cutscene`

### 5.1 Composition

**Authored.** A perception bundle is one `sequence`. Its steps are ordinary
steps; a `cutscene` may be a step beside them, and a cutscene's own clock is
its shots' `seconds`, so a face at the cut and a blind window under a shot
are both written as `at_ticks` against one origin. `DW0329` (no nested
`sequence`) is untouched: a bundle that wants two timelines flattens them.

The spike's station 3, as a campaign writes it — a plate `trigger` with
`audience: presser`, whose bundle is:

```json
{ "type": "sequence", "steps": [
  { "at_ticks": 0, "effects": [
    { "type": "give-effect", "effect": "minecraft:darkness", "seconds": 8, "hide_particles": true },
    { "type": "give-effect", "effect": "minecraft:nausea",   "seconds": 10, "hide_particles": true },
    { "type": "particle",    "particle": "minecraft:elder_guardian", "at": "players" },
    { "type": "play-sound",  "sound": "minecraft:entity.elder_guardian.curse", "volume": 1.0, "pitch": 0.7 },
    { "type": "play-sound",  "sound": "minecraft:entity.warden.heartbeat", "volume": 1.0, "pitch": 0.6 } ] },
  { "at_ticks": 50, "effects": [
    { "type": "give-effect", "effect": "minecraft:blindness", "seconds": 2, "hide_particles": true },
    { "type": "particle",    "particle": "minecraft:elder_guardian", "at": "players" },
    { "type": "play-sound",  "sound": "minecraft:entity.elder_guardian.curse", "volume": 1.0, "pitch": 0.5 } ] },
  { "at_ticks": 110, "effects": [
    { "type": "play-sound",  "sound": "minecraft:entity.warden.nearby_closest",
      "at": { "at": "players", "offset": [0, 0, -3] }, "volume": 1.0, "pitch": 0.5 } ] } ] }
```

No step names `audience`: the root is a `presser` trigger, so every step's
default is `actor`, and the timeline carries the presser through its two
`schedule`s (§3.2). Written at an objective's completion instead, the steps
would say `"audience": "actor"` to reach the completer alone, or nothing to
reach the party.

### 5.2 A cutscene is pure observation, and a blind window is not harm

**Cited** (`emit::CUTSCENE_TAG`): while a player watches a cutscene the
stealth judge is suspended for them and `damage-players` skips them; *any
future verb that demands input or deals harm joins this list*. **Authored**:
a status effect, a particle and a sound neither demand nor harm, so none of
the three joins it — a face over a shot and a blind cut at the end of one are
the creator's picture. What a sight effect does owe a camera is §5.3.

### 5.3 A sight effect outlasts the camera it overlaps

**Cited** (the findings ledger, an open capability row): *a granted sight
effect outlasts any authored camera it can overlap, plus vanilla's wind-down,
so it can never begin ramping down on screen*; the row
binds today only to an area's `mitigation` grant, whose lease the compiler
derives, and states that *a `give-effect` sight grant's author-chosen
`seconds` has no check against the cameras it can overlap*.

**Authored.** This spec gives the row its `give-effect` half, because a
perception bundle is exactly a sight grant timed beside a camera.
`DW0944`, validation tier (exit 1),
`dsl::validate`: in one timeline, a `give-effect` of a sight effect
(`night_vision`, `blindness`, `darkness` — the set the ledger row names, held
as one list in `crates/dsl` beside the blinding set of §6) whose window
`[at_ticks, at_ticks + 20 × seconds)` overlaps a `cutscene` step's window and
**ends inside it, or within that effect's wind-down after it**, is refused
naming the grant, the shot and the tick the grant ends. The wind-down per
effect is pinned from the wiki by the test that lands the list (authored from
memory: night vision flashes in its last seconds; blindness and darkness fade
over about a second) and written in `crates/dsl` with its page. A grant that
ends before the shot begins, or after the shot plus wind-down, is green; a
grant with no cutscene in its timeline is not examined. The remedy is the
one the ledger row states: a longer `seconds`, or a later `at_ticks`.

## 6. The refusal — a blinding beside a drop

### 6.1 Whether one is owed

**Authored, on the ruling in the operating practice** (*danger is visible, or
the engine refuses it*; spec-0062 is its world half).

Spec-0062 proves every killing volume shows itself to a player who can see.
A blinding takes the seeing away: a player who walked up to a pit's rim with
the pit in view and is then blinded for two seconds stands beside a hazard
they cannot see, and the bot cannot fail it for them — mineflayer's
navigation reads the server's blocks, not the client's fog, and the harness
names none of the three effects (cited: 0 of 30 files). That is the invisible
hazard class with the invisibility moved from the floor to the eye, and the
ruling's own test applies: *can a body standing on solid ground be caught*.
With `blindness` on, the honest answer is *yes, if it walks*. So a refusal is
owed, and it is owed to the **grant** (`give-effect` of a blinding effect),
wherever the grant is written — a perception bundle, a trap payload, a
dialogue respawn — never to the perception beat in particular.

The ruling also fixes the remedy's direction: *the repair for an invisible
hazard is to move the hazard, never to mark walkable-looking ground
unwalkable*. Here the thing the creator moves is the blinding — where it
lands, how long it lasts, which effect it is — or the volume, as spec-0062
already lets them; the walk graph loses nothing.

### 6.2 The rule, in cells

**Authored**, from predicates the engine already computes.

| term | definition | instrument |
|---|---|---|
| a **blinding** effect | `blindness`, `darkness` — the two whose screen hides the floor (§4.1's table); `nausea` is not one | one list in `crates/dsl`, pinned from the wiki by its test |
| the **standing set** `S` of a grant | where the players it reaches can stand when it lands (table below) | per root kind |
| the **reach** `n` | `ceil(seconds × speed)` body moves: walking speed (`WALK_SPEED_BLOCKS_PER_SECOND`, 4.317) for `blindness`, which forbids the sprint [cited]; the sprint speed for `darkness`, which does not — a constant `dsl::metrics` does not hold today (0 sprint constants, measured) and gains, pinned from the wiki page *Sprinting* by its test | `dsl::metrics` |
| the **blind reach** `R` | every cell a body can end up in from `S` in at most `n` moves of `World::body_moves` — the walk step, the fall a body survives, the jump — over the assembled world with lethality removed (`World::without_lethal`, as `DW0891`'s population is) | `nav::World` |
| **caught** | `R` meets a killing volume's keep-out (`metrics::keep_out_box` over `Plan::zone_box`), or a column beside a cell of `R` that a body steps off has no landing within `unarmoured_survivable_fall_blocks` (`World::settle_fp` yields nowhere) | `compiler::lethal`, `nav::World` |

The standing set, per the root the grant hangs off, and narrowed to the `in`
box when the effect declares one:

| the grant's audience stands | `S` |
|---|---|
| in an `in` box | the standable cells of the box (an empty set is a finding, §6.3) |
| at an `approach` trigger | the standable cells within its `range` of its anchor |
| at a `use` / `strike` trigger, a plate or tripwire trap, a shop, a bonfire | the cells a press can be made from, as `compiler::pressable` describes the body at the anchor |
| at an objective's completion (`reach-anchor`) | the completion footprint `reach::check_reach_footprint` judges |
| on a respawn (`on_respawn`, `on_death`) | the active checkpoint's or bonfire's seat |
| anywhere else (a quest's `on_complete`, `talk-to`, `kill`, `collect`, `interact`, `on_kill`, `on_unlock`, campaign start) | the walked population `P` of spec-0062 §2 |

A grant whose `R` is caught is **`DW0943`**, build tier
(exit 3), `compiler::lethal`'s neighbour, asked after `DW0891` (so every
volume it reasons about is one the player could see) and before the route
proofs. The message names the grant, `|S|`, `n`, the first caught cell by
floor as `DW0881` prints them, and the volume or column that catches it. The
remedy, in the geometry's own terms: *shorten `seconds` so the reach stops
short; draw `in` so the standing set is farther from the hazard; use
`nausea`, which leaves the floor visible; or move the volume as `DW0891`'s
remedy says — never remove a cell from the walk.*

**What it deliberately does not catch, stated.** A fall between four and
twenty-two blocks hurts and does not kill; it is not caught, because the
ruling is about killing volumes and the walk model already calls such a
drop a landing. A player already blind from a potion of their own is
nobody's grant. A player who sprints *before* the blinding lands and
carries momentum into it is a body the static model does not have. The demo
level is where those are looked at, and its round summary says so.

### 6.3 The ledger

Every build that assembles a world prints one line, zeroes included:

```
blind-reach binding: 3 blinding grant(s) examined; standing sets of 9, 1 and 412 cell(s); reaches of 9, 9 and 35 move(s); 0 caught.
```

and `validation/lethal-gate.json` gains, per blinding grant, `standing`,
`reach_moves`, `reached` (`|R|`) and `caught` (the cells). A grant whose `S`
is empty — an `in` box over no standable cell — is a declaration the bytes do
not bear out, refused under the same code with a count of zero, the shape
`DW0887` and `DW0891`'s second arm already refuse. A campaign with no blinding
grant prints the line with zeroes.

## 7. Accessibility: what the creator is told

**Cited** (*Options*, *Options.txt*; the spike README names the first three):

| setting (menu) | `options.txt` key | at less than full | what survives |
|---|---|---|---|
| Distortion Effects | `screenEffectScale` (0–1, default 1) | nausea's warp is increasingly replaced by a green vignette; at 0 the warp is gone | the vignette, until 0 |
| Darkness Pulsing | `darknessEffectScale` (0–1, default 1) | at 0 the pulse is gone | the fog, always |
| Particles | `particles` (All / Decreased / Minimal) | may drop `normal`-mode particles | a `force` particle, always (§4.3) |
| sound sliders | `soundCategory_master` and ten categories | the engine writes `master`, so only the master slider applies | nothing, at 0 |

**Authored**, three sentences the skill carries (§8), each stated once:

1. **A perception beat is never the only signal.** Every one of these effects
   is on the player's side of a slider the engine cannot read, so a state
   change a player must act on is also told by something the sliders do not
   reach — a `narrate`, a changed block, a gate — and the perception bundle
   is the mood over it, not the message. This is a design rule and is not
   machine-checked: whether a story hangs on a face is not a fact about a
   document.
2. **`hide_particles: true`** on every grant in a perception bundle: the swirl
   and the HUD icon are what make a blinding read as a potion.
3. **What the engine fixed for them**: `force` on every particle; `master` on
   every sound; a duration on every grant.

### 7.3 One claim from memory

The spike README writes that *a client with Particles below All may drop*
the face and that force mode was *not tested at Minimal*. The wiki page
*Commands/particle* states force mode is *always shown even if the option is
Minimal*, and this spec writes `force` on that sentence. Whether the pinned
1.21.11 client honours it for `elder_guardian` at Minimal is confirmed by the
demo level (§9), at the setting, before the skill's sentence 3 is written as
a fact; until then the skill says *written in force mode* and no more.

## 8. What the gallery, the record and the skill owe

**Authored.**

- **The gallery element** (spec-0039). The hall's `obj/press-the-case`
  completion gains a `sequence` of three steps: at 0 a `particle` at
  `players` with `audience: actor` and `hide_particles` on a `give-effect` of
  `nausea` (`in` the pedestal box the element already uses); at 30 a
  `particle` at a mark (`anchor/pedestal`, offset `[0, 1, 0]`, `count` 24,
  `spread`, `speed`); at 60 a `play-sound` at `players` with `offset
  [0, 0, -3]`. Units: the envelope's `audience` and `in` (`in` is already
  bound on `give-effect`; `audience` and `in` on `particle` and `play-sound`
  are new bindings), `particle` and each of its fields, both `at` forms,
  `players.offset`. Bound by perturbation: changing `offset` moves the
  `positioned ^…` clause; changing `audience` moves the step's selector from
  `@a` to `@s`; changing `count` moves the `particle` line. The timeline
  starts from a `Party` root, so the tag form (§3.2) is emitted and the
  emitted step function names `@s` — exactly what `tests/scheduled_executor.rs`
  must accept through the `as` re-binding, which is the test's own rule.
  `nausea` is chosen over `blindness` because the hall holds two killing
  volumes (`DW0891`'s own fixtures) and a blinding there is the probe, not
  the element; a vanilla effect id is data, never a unit, so no unit is lost.
- **The probes.** Five, each the primary plus a declared edit:
  `a-particle-the-game-does-not-draw` (`DW0941`, an
  unknown id); `a-face-shown-to-nobody-in-particular`
  (`DW0942`, `audience` on a `set-block`);
  `an-actor-the-tick-does-not-have` (`DW0503`, `audience: actor` inside a
  `party` trigger's bundle); `a-blinding-at-the-rim`
  (`DW0943`, a two-second `blindness` `in` a box at the
  west pit's rim — the refusal names the pit's keep-out); and
  `a-sight-that-fades-under-the-camera` (`DW0944`,
  a timeline holding a `night_vision` of 3 seconds and a 10-second cutscene
  shot, both at tick 0 — the hall's cutscenes are top-level effects today, so
  the probe adds the timeline).
  Perturbing `a-blinding-at-the-rim` toward the vacuous shape — the same grant
  as `nausea`, or the box drawn thirty cells from either pit — is green with a
  non-zero standing set on the binding line.
- **The record.** `docs/reference/compiler.md`: the envelope's two new field
  rows beside `when`; the `particle` surface and emission rows beside
  `play-sound`'s; `players.offset` in the `SoundAt` row; the §4 *global
  timeline* paragraph rewritten for the tag form; `DW0357`/`DW0503`'s
  re-stated seam (2); the four new codes' rows; the binding line; the
  particle registry in `PROVENANCE.md` — in the pull request that lands the
  code.
- **The skill.** `references/quest-capabilities.md`, under *Things the
  player sees and hears*: a perception beat is a `sequence` of
  `give-effect`, `particle` and `play-sound`; `audience` and `in` on any of
  them; the face is `particle minecraft:elder_guardian at players`; a sound
  behind is `players` with `offset [0, 0, -3]`; §7's three sentences; a
  blinding near a killing volume is refused and the remedies are §6.2's; a
  re-armed bundle inside its own length re-applies nothing visible; a
  timeline re-started before it ends re-times both players.
- **The demo level.** A row in `docs/demo-levels.md`, queued now and marked
  blocked on this spec (§9).

## 9. The demo level

**Authored.** *Something Looks Back* — one small room with three plates and a
lit pit at its far end. Plate one plays the bundle of §5.1 to the player who
stepped on it while a second player beside them sees and hears nothing but
the room; plate two plays it to everyone in the room's box; plate three to
everyone. The fourth declaration — the same bundle with `blindness` at the
pit's rim — is the level's refusal transcript, not a plate. The level is
played twice: at full settings, and at Distortion Effects 0, Darkness Pulsing
0, Particles Minimal; the round summary records what survived the second
pass per effect, which is where §7.3's claim is confirmed and where the
behind-sound's position (unmeasured by the spike, whose window ended before
its tick) is heard.

## 10. Acceptance criteria

Machine-checkable; each names its instrument. Every criterion was checked
against the tree at `c0f22c51` before being written and is recorded as a
**debt** where the tree cannot yet satisfy it. `delvec` is the tree's
`target/debug/delvec`, built from the crate manifests the tree carries.

1. **The surface.** `delvec schema --stage all` exports `audience` (`party` |
   `actor`) and `in` on `QuestEffect`, no `in` inside `give-effect`,
   `clear-effect` or `damage-players`; a `particle` verb with `particle`,
   `at` (a `Mark` or the constant `players`), `count` (`minimum: 1`),
   `spread`, `speed`; and `offset` on `SoundAt::players`; the effect union's
   `oneOf` names 39 verbs. *Tree: debt — 38 verbs; `particle` 0 times;
   `audience` once (the trigger's); `in` six times, all inside verbs.*
2. **The default audience is the root's.** A test builds one campaign with a
   player-facing effect under each of the nine roots and a `presser` trigger,
   none declaring `audience`, and asserts the emitted selector per root equals
   `emit::root_audience`'s (and `@s` for the presser); two builds are
   byte-identical (ADR-0006), and every gallery and fixture build on the tree
   is byte-identical before and after the field exists. *Tree: debt.*
3. **A timeline keeps its actor.** A test declares a `sequence` with an
   `audience: actor` step under an objective completion and reads the emitted
   pack: the start function carries `tag @s add dw_seq_<root>_<n>`, each
   scheduled step is dispatched `execute as @a[tag=…] run function …`, the
   step body names `@s`, the last step removes the tag; the same timeline
   under a `party` trigger emits no tag and addresses `@a`;
   `tests/scheduled_executor.rs` is green unchanged. *Tree: debt — every step
   is `Audience::Scheduled`.*
4. **`DW0503`'s fourth shape and the party-fact refusal.** `audience: actor`
   inside a `party` trigger's bundle is `DW0503`; `audience` or `in` on
   each verb the emitter classifies as a party fact is
   `DW0942`, and the set of verbs it refuses
   is read from the emitter's classification by one test that adds a verb to
   neither side and fails. *Tree: debt.*
5. **The particle.** A test declares both `at` forms and reads the emitted
   lines: `force` on both, `@s` as viewers inside `execute as <who> at @s`
   for `players`, the mark's `.5` centre and `<who>` for a mark; an unknown
   id and an options-taking id are `DW0941`; the
   registry file exists with its `PROVENANCE.md` entry and every entry
   states whether it takes options. *Tree: debt — no registry, no verb.*
6. **The listener's frame.** A `players` sound with `offset [0, 0, -3]` emits
   `execute as <who> at @s rotated ~ 0 positioned ^0 ^0 ^-3 run playsound
   <sound> master @s ~ ~ ~ …`; with the default offset the line is today's.
   *Tree: debt — `SoundAt::Players` is a unit variant.*
7. **A sight effect outlasts the camera.** A timeline with a 3-second
   `night_vision` at tick 0 and a 10-second cutscene shot at tick 0 is
   `DW0944`; at 15 seconds it is green; the
   sight set and each wind-down are one table in `crates/dsl` with the wiki
   page per row; the ledger row whose general form is *a granted sight
   effect outlasts any authored camera it can overlap, plus vanilla's
   wind-down, so it can never begin ramping down on screen* gains a
   `give-effect` binding and names this code as its general form. *Tree:
   debt — the row is open and binds only `mitigation`.*
8. **The blind reach.** On the gallery, a two-second `blindness` `in` a box
   at the west pit's rim is `DW0943` naming the pit; the
   same grant as `nausea` is green; the same `blindness` thirty cells from
   both pits is green with `|S| > 0`; an `in` box over no standable cell is
   refused with a count of zero; the reach uses `body_moves` over
   `World::without_lethal` (a test perturbs to the lethal-applied world and
   asserts the rim grant goes green — the vacuous shape — and that the check
   is bound to the other); the sprint constant exists in `dsl::metrics` with
   its page. *Tree: debt — the walk model refuses the keep-out already, so
   the population is lethality-free only where `DW0891` asks for it; no
   sprint constant (0 matches).*
9. **The ledger.** Every assembling build prints §6.3's line and
   `lethal-gate.json` carries the per-grant fields; the primary gallery
   prints zeroes. *Tree: debt.*
10. **The gallery.** §8's element builds green; the five probes are refused
    with the codes they name; `tools/ci/check-gallery-coverage.py` reports
    0 units in neither state; the perturbations of §8 each move an emitted
    byte. *Tree: debt — the gallery's one `sequence` is `spawn-actor` steps
    under a `campaign-start` trigger, and no element addresses an actor.*
11. **The record and the skill.** §8's rows and paragraphs in the pull
    request that lands the code; `tools/ci/check-dw-codes.py` binds all four
    codes with zero new allowlist entries. *Tree: debt.*
12. **The demo level** is queued in `docs/demo-levels.md` as *Something Looks
    Back*, `pending (blocked on 0085)`. *Met by this spec's pull request.*

## 11. Decisions for the owner

- **There is no `perception` verb**: the beat is a `sequence` of
  `give-effect`, `particle`, `play-sound`. The alternative — one verb with its
  own timing and audience — re-implements three general mechanisms inside
  one verb and reaches nothing else.
- **`audience` and `in` move to the envelope** and are refused on party
  facts; the default is the root's own answer, so nothing already built
  changes a byte. The alternative — a fourth per-verb `in` and a per-verb
  `audience` — is the second-bespoke-field defect.
- **A timeline carries its actor by a compiler-owned tag**, exactly as the
  spike did; the timeline stays global (a re-start re-times both players).
  The alternative — `schedule … append` per player — would let the first
  run's last step strip the second player's tag.
- **A particle is always `force`.** The alternative, a per-beat mode, offers
  a knob whose only effect is to lose an authored beat on some clients.
- **A sight grant that ends under a camera is refused** — the ledger's own
  open general form, given its `give-effect` half here rather than in a spec
  of its own.
- **Falls that hurt but do not kill are not caught**, and the record says so.
- **`dsl_version` does not move**; four codes, `DW0941`–`DW0944`; no ADR.

**Settled under the danger-is-visible rule, not put to the owner.** A
blinding whose reach meets a killing volume is refused (one new code), with
`darkness` counted as blinding beside `blindness`: a hazard must read as a
hazard to the player, and darkness's trough is complete darkness. The cost: a
`darkness` or `blindness` anywhere in a campaign with a killing volume must
be `in`-scoped or short enough that its reach stops short; `nausea` is the
effect for a beat beside a pit.
