# Death edge + mid-fall teleport — spike findings (Minecraft Java 1.21.11)

Status: **spike note**, measured 2026-08-09. Nothing here is implemented; no
engine, DSL or campaign file changed. Rig: `tools/spike-death-teleport/`
(`EULA=TRUE tools/spike-death-teleport/run.sh`), raw per-sample observations
committed beside it as `observations.json` (4140 kept samples), the gamerule
identifier dump as `gamerules-1.21.11.txt`.

Server: the pinned image digest (`versions.toml [images.base] mirror_of`),
`/version` = `1.21.11 / data 4671 / protocol 774 / pack_data 94.1`, vanilla, no
mods, superflat, difficulty hard. Q1 = 5 causes × 3 repeats (15 trials); Q2 = 26
configs × 2 repeats (52 trials, 46 of them with a teleport). **Every repeat of
every case agreed exactly** — no intermittency in either question's results. The
one intermittency the rig did hit was in its own instrument, and §4b root-causes
it.

## Verdict

**Q1.** A pre-respawn death signal exists for **every** cause measured (void,
fall, drowning, lava, mob), and the **exact death position is readable at that
moment** — but not through an advancement. The mechanism is the pair the engine
already arms (`deathCount`) plus `LastDeathLocation`, both of which are written
on the death tick and readable for as long as the player stays on the death
screen. The one advancement trigger that names a death, `entity_killed_player`,
fires **only** for the mob kill and never for the four environmental causes.

**Q2.** `/teleport` does **not** touch accumulated fall distance. The value
carries across the teleport unchanged — down, and 143–157 blocks *up* — and is
charged in full at whatever the player next lands on. **An arriving lift car
does not catch a falling player; it is the surface they die on.** Landing damage
is `floor(fall_distance) − 3` in all 46 teleport trials.

## 1. Method, and a correction to the premise

The engine's death edge is **not advancement-based**. `crates/compiler/src/
emit.rs::emit_checkpoint_functions` arms `dw.deaths`, a vanilla **`deathCount`
scoreboard criterion**, against a `dw.death_ack` dummy, and `cp_respawn_check`
holds both the fire and the acknowledgement behind `unless data entity @s
{Health:0.0f}`. That is a scoreboard edge with an aliveness guard. The spike
therefore probes three candidate mechanisms side by side rather than one:

| probe | how |
|---|---|
| `deathCount` edge | `dw.deaths` / `dw.death_ack`, plus the engine's guard evaluated verbatim, and the same score comparison **without** the guard |
| `minecraft:entity_killed_player` | `dwspike:killed_by_entity`, a criteria-only advancement in `spikepack/` (no `rewards.function`), read with `@s[advancements={…}]` |
| `minecraft:entity_hurt_player` | `dwspike:hurt_any`, same shape — probed to learn whether environmental damage reaches advancement triggers at all |

Alongside each sample: `Pos`, `Health`, `fall_distance`, `OnGround`,
`DeathTime`, `LastDeathLocation`, and the gametime tick.

The instrument is a mineflayer bot (the harness's pinned 4.37.1) created with
`respawn: false`, so the corpse **stays on the death screen until the rig
dismisses it** — every "at the moment of death" reading is taken from a ~3 s
window of held death screen, not from one lucky tick. Values are read from the
**server's** NBT/scoreboard over a pipelined rcon channel (p50 round trip 2 ms,
p95 13 ms), never from the bot's client-side belief: for players the server
computes fall damage itself from movement packets, so the server's
`fall_distance` is the quantity that decides whether they die.

Deaths are induced for real (a 100-block drop, a sealed water box, a sealed lava
pit, a one-shot zombie, a teleport to y = −300), not with `/damage`.
`natural_health_regeneration` is off for the whole run.

`LastDeathLocation` cannot be cleared (players reject `/data modify`) and three
repeats of one cause die on the same block, so every Q1 trial is preceded by a
**scrub death** at a pad no cause uses, confirmed by reading the value back.
Without it, repeats 2 and 3 of every cause would have reported
"`LastDeathLocation` absent" — a false negative on the question being asked.

## 2. Q1 — the pre-respawn death signal, per cause

The three repeats of each cause agreed on every answer below and on every
coordinate, to the digit; only the number of samples taken during the hold
differs. "Corpse pos" is the position read while `Health` = 0 with the death
screen still up, and it was **stable for the whole hold** (drift 0.000 in every
one of the 15 trials — a corpse stops falling, even one that died mid-air).

| cause | deathCount edge fires? | before respawn? | corpse `Pos` readable | is it the death position? | `LastDeathLocation` | `entity_killed_player` | `entity_hurt_player` |
|---|---|---|---|---|---|---|---|
| **void** (y = −300) | yes | **yes**, 63 samples with `Health` 0 | `[0.5, −312.73, 0.5]` | yes — `floor` = the LDL block | **yes**, same tick, `{pos:[I;0,−313,0]}` | **no** | yes (first damage tick) |
| **fall** (100 blocks) | yes | **yes**, 61–62 samples | `[20.5, 100, 20.5]` | yes, exact | **yes**, same tick, `{pos:[I;20,100,20]}` | **no** | yes |
| **drowning** | yes | **yes**, 62–63 samples | `[40.5, 100, 40.5]` | yes, exact | **yes**, same tick, `{pos:[I;40,100,40]}` | **no** | yes |
| **lava** | yes | **yes**, 62–63 samples | `[60.5, 100, 60.5]` | yes, exact | **yes**, same tick, `{pos:[I;60,100,60]}` | **no** | yes |
| **mob kill** (control) | yes | **yes**, 62–63 samples | `[80.5, 100, 80.5]` | yes, exact | **yes**, same tick, `{pos:[I;80,100,80]}` | **yes** | yes |

Timing, in gametime ticks (batches are not tick-atomic, so ±1 is instrument
resolution, not a real lag):

- `deathCount` increments on the same tick `Health` reaches 0 (`edge − hp0` was
  0 or −1 across all 15 trials).
- `LastDeathLocation` appears on that same tick (lag 0 in all 15 trials).
- The score edge is **armed on the corpse**: the bare comparison
  `dw.deaths > dw.death_ack` was true in every held-death sample.
- The engine's guarded form was **never** true while dead, in any trial, and
  became true on the first sample after the death screen was dismissed. So
  `cp_respawn_check` is not late by accident — the `Health:0.0f` guard is
  exactly what defers it, and the edge waits, intact, for as long as the player
  leaves the screen up.

Three consequences for a "run effects at the moment of death" surface:

1. **The signal exists for all causes, pre-respawn.** Anything that wants to
   fire on death rather than on respawn can key off the same `deathCount` edge
   the engine already arms — it just must not carry the aliveness guard.
2. **The death position is available two ways**, and they agree:
   `LastDeathLocation` (a block pos, written on the death tick, survives the
   respawn) and the corpse's own `Pos` (exact doubles, stable, but only until
   the player respawns). The *last live sample before death* is not a
   substitute: in two of the three fall trials it read `y = 102.48` against a
   death at `y = 100`.
3. **An advancement is the wrong instrument.** `entity_killed_player` covers one
   of five causes. `entity_hurt_player` does reach environmental damage — it
   granted for all five, including the three with no source entity — but it
   fires on the **first** damage event, not the fatal one: for void, drowning
   and lava — all repeated-damage causes — the sample that first saw it granted
   still read `Health` 16–20, far above 0. It identifies "was hurt", never "died
   here". (The 100-block fall deals one damage event and it is fatal, so that
   cause cannot distinguish the two readings at all.)

## 3. Q2 — fall distance across a mid-fall teleport

`fall_distance` is a **double** on 1.21.11 (`FallDistance` no longer exists).
Controls first, with no teleport. The sampled column is the last value read
before the landing tick — sampling cannot catch the landing tick itself, which
is why it sits just under the launch height:

| launch height | last sampled `fall_distance` | outcome |
|---|---|---|
| 6 | 5.688 | 3 damage (hp 17) — i.e. `floor(6) − 3` |
| 30 | 28.515 | **died** |
| 120 | 117.269 | **died** |

Teleport trials — `car` = teleported onto the destination floor, `car+1` = one
block above it, `deep` = onto a point 160 blocks above a distant floor (so the
player keeps falling and the trajectory itself is visible). 46 teleport trials in
total, two repeats of each config, every repeat identical; representative rows:

| trial | teleported at | `fall_distance` before → after tp | carried | at landing | outcome |
|---|---|---|---|---|---|
| h=6 early → `car` | y 104.4 | 1.593 → 1.593 | **0.0000** | 1.593 | hp 20 |
| h=6 late → `car` | y 101.2 | 4.844 → 4.844 | **0.0000** | 4.844 | 1 damage |
| h=6 late → `car+1` | y 101.2 | 4.844 → 4.844 | **0.0000** | 5.309 | 2 damage |
| h=30 early → `car` | y 122.4 | 7.560 → 7.560 | **0.0000** | 7.560 | 4 damage |
| h=30 late → `car` | y 103.2 | 26.777 → 26.777 | **0.0000** | 26.777 | **died** |
| h=120 early → `car` | y 189.7 | 30.297 → 30.297 | **0.0000** | 30.297 | **died** |
| h=120 late → `car` | y 116.9 | 103.051 → 103.051 | **0.0000** | 103.051 | **died** |
| h=30 late → `deep` (+157 up) | y 103.2 | 26.777 → 26.777 | **0.0000** | 186.453 | **died** |
| h=120 late → `deep` (+143 up) | y 116.9 | 103.051 → 103.051 | **0.0000** | 262.727 | **died** |

So, answering the three options in the question: the accumulated fall distance
**carries** — `after − before` was exactly `0.0000` in 46 of 46 teleport trials,
including teleports 143 and 157 blocks straight *up* — and then **applies on
landing at the destination**. It never resets. The `deep` rows are the clean
demonstration: the counter does not restart after the teleport, it continues
from the carried value (26.777 → 186.453). No trial ever landed before its
teleport fired (`tp_missed` false in all 46), so none of this is an artefact of
a missed trigger.

### The catch threshold

Landing damage fits `floor(fall_distance) − 3` exactly in every row above and
every row below; the sweep teleported the player onto the car at a chosen
accumulated distance, from full health (20):

| `fall_distance` at teleport | damage | hp left |
|---|---|---|
| 1.145 | 0 | 20 |
| 5.688 | 2 | 18 |
| 10.807 | 7 | 13 |
| 18.773 | 15 | 5 |
| 20.279 | 17 | **3 — survived** |
| 23.435 | 20 | **died** |
| 25.083 | — | died |
| 30.297 | — | died |

**A lift car catches a falling player only if it arrives within the first ~20
blocks of the fall.** Past that the car is simply the floor they hit: nothing
about being teleported onto it softens the landing. For a full-health player the
fitted rule puts the last survivable value at `floor(fall_distance) = 22`; the
measured bracket is *survived at 20.279, died at 23.435* (see §5 — the values in
between were not sampled).

## 4. Incidental findings

**(a) 1.21.11 renamed the entire gamerule registry to snake_case, and a legacy
name is silently rejected.** All eight legacy identifiers probed live
(`fallDamage`, `doDaylightCycle`, `doWeatherCycle`, `doMobSpawning`,
`doImmediateRespawn`, `keepInventory`, `spawnRadius`, `naturalRegeneration`)
answer `Incorrect argument for command` and change nothing. Several were also
reworded: `doMobSpawning` → `spawn_mobs`, `doDaylightCycle` → `advance_time`,
`doWeatherCycle` → `advance_weather`, `naturalRegeneration` →
`natural_health_regeneration`. The compiler already emits the new names
(`crates/compiler/tests/emit.rs` asserts them). **Two places in this repo still
emit the old ones and neither can notice:**

- `crates/admit/src/gallery.rs` — the gallery's `load` function emits
  `doDaylightCycle`, `doWeatherCycle`, `doMobSpawning`, `doImmediateRespawn`.
  All four are rejected at run time, so a gallery world cycles day and weather,
  spawns mobs, and does not immediate-respawn.
- `tools/spike-jump-arc/measure.mjs:186` — `gamerule fallDamage false`, whose
  comment states the bot takes no fall damage during the jump trials. It does.

Both are **live**, not historical. Neither reads the command's response, which is
the general lesson: this rig checks every one (`ok()`), and the check is what
caught its own `natural_regeneration` typo and a `fill` into an unloaded chunk
that would otherwise have measured a player falling through a floor that was
never placed. The measured registry (name = default on a fresh 1.21.11 server) is
in `observations.json` under `incidental.gamerule_registry`.

**(b) A respawned player is invulnerable for 59 ticks (≈3 s), and `/kill` lies
about it.** This surfaced as two instrument defects — `/kill <player>` answering
`Killed <player>` and not killing (2 of 3 in a first loop), and the Q1 scrub
death needing a second `/damage` issue in 14 of 15 trials — which are the same
thing. Measured directly (`incidental.respawn_invulnerability`, 2 repeats,
identical): from the tick the server first reports the respawned player alive,
a 1-point `/damage` is **refused for 59 ticks** and accepted on the 60th. The
refusal is explicit: `Target is invulnerable to the given damage type`.
A `/kill` issued at the top of that window answers `Killed <player>` and the
player's `Health` is still 20 afterwards — **success is reported either way**.
`/damage` outside the window killed on the issuing tick in every observation,
which is what the rig's scrub uses.

For a souls-shaped delve this is load-bearing, not trivia: any beat that damages
or kills a player within ~3 s of their respawn does nothing, and if it is spelled
`/kill` there is no signal that it did nothing. The engine today only emits
`kill @e[tag=…]` against non-player entities, so nothing shipped is affected.

**(c) Player NBT spellings on 1.21.11** (probed live): `fall_distance` (double)
exists, `FallDistance` does not; `OnGround`, `Motion`, `Air`, `Health`, `Pos`,
`DeathTime`, `LastDeathLocation` keep their old capitalised names. Players
reject `/data modify` entirely (`No entity was found`), so `LastDeathLocation`
cannot be cleared and `Health` cannot be set — the rig heals with
`instant_health` and scrubs `LastDeathLocation` by dying somewhere else first.

**(d) A conditional `execute` with no `run` returns an EMPTY rcon response on
1.21.11** — there is no `Test passed` / `Test failed` text, and empty is
indistinguishable from a rejected command. Every boolean probe in the rig is
therefore spelled `execute <conditions> run time query gametime`, and a response
that is neither the time nor empty aborts the run.

**(e) A pipelined rcon batch is not tick-atomic.** A batch that straddles the
killing tick reports e.g. `Health: 4` beside `deaths: 1`. This is why no Q1
claim rests on a single sample; it is also why the timing column above is quoted
to ±1 tick.

## 5. What remains unknown

- **The exact survivable/lethal boundary in Q2.** Free fall makes
  `fall_distance` discrete in ~3-block steps by that point in the drop, so the
  trigger asking for "≥ 22" and "≥ 23" both fired at the same sampled value
  23.435. Measured: survived 20.279, died 23.435. The `floor(fall) − 3` rule
  (which fits every trial in §3) predicts the boundary at 23.0, but **that
  specific value was never observed** — it is a fit, not a measurement. Landing
  a player at a chosen `fall_distance` needs a slower fall (e.g. slow_falling,
  or a teleport ladder) and was not attempted.
- **Whether a real vanilla client behaves as the mineflayer bot did in Q2.** The
  numbers here are the server's own `fall_distance`, and for players the server
  computes fall damage from movement packets, so the reading is authoritative —
  but the packets came from prismarine-physics, not from Mojang's client. Not
  independently confirmed with a human client.
- **What *does* reset fall distance.** Only `/teleport` was tested, and only
  within one dimension. Water, landing, dimension change, vehicles, and
  `slow_falling` were not measured; nor was any command-driven way to zero the
  counter (`/data modify` on a player is rejected, see §4c). If a lift is
  supposed to catch people, the mechanism that makes it safe is **unmeasured**.
- **Q2 with a moving destination.** The "car" here is static stone that the
  player is teleported onto. A car that is itself being moved (block-swapped,
  or a rideable entity) was not tested.
- **Death causes not covered**: suffocation, fire/burning without lava, cactus,
  starvation, `/kill`, explosion, magic/potion, falling anvil, and death in
  another dimension. Nothing here says whether `LastDeathLocation` behaves the
  same for them — though it was written by all five measured causes, including
  three with no source entity.
- **Whether the 59-tick respawn window is fixed or conditional.** Both repeats
  gave exactly 59, and an earlier ad-hoc probe gave 59 twice more, but nothing
  was varied — not difficulty, not gamemode, not movement (the bot stood still
  throughout), not `immediate_respawn`. Whether the window ends early when the
  player moves or attacks is **not measured**.
- **The rig's own setup phases are not in the raw record.** `observations.json`
  keeps every sample of the death window, the fall and the settle, but the
  samples taken during a trial's reset and scrub are consumed and dropped. That
  is why §4b had to be re-measured with a separate probe rather than read out of
  the committed data; the probe's numbers are now in the file, its samples are
  not.
- **Whether `deathCount` can be observed by a datapack on the death tick
  itself.** The rig samples over rcon from outside; the sample window proves the
  value is *readable* while the corpse is up, not that a `tick` function
  scheduled that same tick would see it. The two are almost certainly the same
  thing — the score is written before the tick loop is polled — but that was not
  measured directly.

## 6. Re-running

```bash
cd <repo>/harness && npm ci          # once; the rig uses the harness's mineflayer
EULA=TRUE tools/spike-death-teleport/run.sh [--out <path>]
```

Boots its own throwaway container from the pinned digest on an **ephemeral**
loopback port (never 25565, so it needs no mutex and runs alongside any ladder),
installs `spikepack/`, runs the trials, writes `observations.json` +
`gamerules-1.21.11.txt` beside the `--out` path, and removes the container on
exit. `SPIKE_Q1_REPS` / `SPIKE_Q2_REPS` change the repeat counts,
`SPIKE_CONTAINER` the container name. Roughly 25 minutes at the committed
settings (3 / 2), most of it the three drowning trials.
