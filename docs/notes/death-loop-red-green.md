# The death loop's red→green demonstrations

Audience: agents. This note exists so the bot tier's `death-loop` stage can be
**disbelieved and re-checked**, not taken on trust. Every red below was produced
by breaking the engine on purpose and watching the stage catch it; every command
is here so any session can reproduce it.

CLAUDE.md's debug doctrine: *a demonstration nobody can reproduce is a defect.*

## Why this stage exists at all

A PackTest fake player is **permanently undamageable** — measured independently
on 2026-08-03 and again on 2026-08-09 — so that tier cannot witness a player
death. Before this stage, `on_death`, a lethal volume's kill and the recovery
stake had **no runtime proof anywhere**, and the first person to exercise them
would have been the owner. The whole souls shape (die → forfeit → respawn → walk
back → recover) is exactly the part no other tier can reach.

## The setup, once

```sh
cargo run -p delvec --bin delvec -- \
  build crates/compiler/tests/fixtures/economy \
  -o validation/delve-output-economy --prefabs campaigns/prefabs
EULA=TRUE validation/bot-run.sh --project dw-death --output ./delve-output-economy
```

Green looks like this (the whole loop, in the bot's own words):

```
death plan: 1 lethal volume(s), 1 on_death effect(s), 1 stake(s), 2 respawn seat(s), 4 placement row(s); death-loop stage ON
[death-loop] lethal/the-drop: standing at [5, 65, 7]; walking into [5, 65, 8] to die there via the near lip [4, 65, 8]; expecting stake stake/embers on ledger dw.s_embers
[death-loop] lethal/the-drop: died at [5, 65, 8]; the volume's own line reached the player
[death-loop] lethal/the-drop: respawned at [5.50, 65.00, 4.50] (checkpoint anchor `anchor/keeper-stand`)
[death-loop] lethal/the-drop: collected — ledger 0 → 5; marker retired
```

…and `run-report.json`'s `death_loop.binding`, which is what makes it a pass
rather than a silence (playtest-methodology rule 1):

```json
{ "declared_volumes": 1, "volumes_entered": 1, "deaths_observed": 1,
  "stakes_examined": 1, "seats_matched": 1, "walks_back": 1, "unbound": false }
```

## The stale-build trap — read this before mutating anything

`sed -i.bak … && mv $F.bak $F` **leaves the mutated binary in place**: the
restored file's mtime predates cargo's artifacts, so cargo believes it is already
built and the next run silently re-tests the mutation. Every restore below is
`cp` **followed by `touch`**:

```sh
cp crates/compiler/src/emit.rs /tmp/emit.orig
# …mutate, build, run…
cp /tmp/emit.orig crates/compiler/src/emit.rs && touch crates/compiler/src/emit.rs
```

Each cycle is: mutate → `delvec build` → `bot-run.sh` → restore + `touch`.

## The finding the stage produced on its FIRST live run

Not a planted mutation — a real defect, in the engine, since spec-0012.

**`on_death` and the checkpoint respawn dispatch never fired on a player's FIRST
death.** `dw.death_seen` and `dw.death_ack` are `dummy` objectives, so a player
who has never died has no score in either. Measured on the pinned 1.21.11 server:

```
scoreboard players set #x probe_a 3          # A = 3, B unset
execute if score #x probe_a > #x probe_b …   # does NOT fire
scoreboard players add #z probe_c 0          # → "Added 0 … (now 0)": this CREATES the entry
```

So the corpse-side edge compared against a score that did not exist. The observed
red, with everything else about the loop working perfectly:

```
death-loop stage FAILED (2 finding(s)):
  lethal/the-drop: the death took the wrong amount. `dw.s_embers` was 5 before and 5 after;
    the campaign's declared forfeit rule says it should be 0 (a forfeit of 5)
  lethal/the-drop: no recovery stake stands at [4, 65, 8], the anchor the compile-time
    placement table chose for this death. The purse was taken and left nowhere
```

Confirmed by seeding the two scores from rcon the moment the bot joined, changing
nothing else — the same build then passed the whole loop. Both edges worked from
the **second** death onward, which is why every compile-time shape proof and every
manual test that dies twice had passed for months. Fixed in `cp_respawn_check`,
which now seeds each acknowledgement it reads ahead of the comparison;
`v06_checkpoints` and `v10_on_death` assert the ORDER, not merely the presence.

**Only the instance was turned into a test.** The general form — *no emitted
`execute if score @s A > @s B` may compare against a per-player score nothing has
written yet* — is a real, findable class and does **not** yet have a diagnostic.
Recorded here as an open finding and a risk item at the next staging review
(CLAUDE.md: a finding is not closed until its general form is a diagnostic).

## The planted mutations

Each is one edit to `crates/compiler/src/emit.rs`, and each was run end to end.

### M1 — the volume deals no damage

```rust
const LETHAL_DAMAGE: u32 = 1000;   →   const LETHAL_DAMAGE: u32 = 0;
```

```
the death-loop stage entered 1 lethal volume(s) and observed ZERO player deaths —
  every assertion downstream of the death edge is therefore unbound
lethal/the-drop: the bot stood inside the declared lethal volume at [5, 65, 8] and did
  NOT die. A lethal volume is the one thing in the engine whose entire contract is that
  entering it kills; nothing downstream of the death edge can be true if this is false
```

Two findings, deliberately: the binding count reports the vacuum independently of
the trial, so a stage that observed nothing can never read as one that passed.

### M2 — the volume kills in silence

Delete the `tellraw @s` line from `emit_lethal_functions`'s `lethal_<v>_kill`.

```
lethal/the-drop: the player died in the volume and the line it PROMISES never reached
  them. A volume with no words is a player who dies with no idea why — the wording is a
  required field precisely because there is no default that could be right
```

The death still happens, so exactly one clause fires. `DW0512` refuses a *blank*
wording at compile time; this is the runtime half — a wording that exists and is
never delivered.

### M3 — the forfeit takes the wrong amount

In `stake_forfeit_lines`, make `Forfeit::All` write a constant:

```rust
"scoreboard players operation {STK_AMT} dw.sys = @s {obj}"
  →  "scoreboard players set {STK_AMT} dw.sys 1"
```

```
lethal/the-drop: the death took the wrong amount. `dw.s_embers` was 5 before and 4
  after; the campaign's declared forfeit rule says it should be 0 (a forfeit of 5)
```

The expected number is computed by `expectedForfeit` from the **declared** rule in
`death-plan.json`, not from the emission — which is the only reason this can fail
at all.

### M4 — the placement table is ignored

In `emit_stake_functions`, drop the table rows from `stk_route`:

```rust
for row in &t.rows {   →   for row in t.rows.iter().take(0) {
```

```
lethal/the-drop: the recovery stake stands at [5.11, 65.00, 7.73], 0.98 blocks from
  [4, 65, 8] — the anchor the compile-time placement table proved reachable and safe.
  Every proof about where a stake may stand is about that cell, not this one
```

This is the degenerate branch swallowing the projected one: without the rows the
stake is left where the player fell — *inside the hazard* — instead of at its near
lip. The marker tolerance is 0.75 blocks precisely so a one-block miss cannot pass;
a correct placement measures 0.00.

### M5 — the collection is not idempotent

In `stk_take_<s>_<k>`, drop the line that clears the slot's live flag.

```
lethal/the-drop: the stake was collected and its hardware is still standing at
  [4, 65, 8]. A collected stake that does not vanish is an affordance that answers a
  click with nothing, forever
```

**Read this one carefully, because it is also a limitation.** The purse ended at
5 — correct — and `collect_clicks_sent: 2` counts packets **sent**, not collections
adjudicated. A client cannot observe how many clicks the server resolved in one
tick, and vanilla grants an interaction advancement at most once per tick, so the
second packet is normally absorbed. The same-tick double click is therefore a
best-effort exercise of the race; what actually caught this mutation is the
marker-retirement clause, which is not best-effort at all. Stated here rather than
left for a reader to assume the idempotency clause did the work.

## What this stage still does not assert, and why

- **The stake's `collected_message`.** It is delivered on the action bar
  (`title @s actionbar`), and mineflayer 4.37 models no action-bar event; decoding
  the raw `action_bar` component in the harness would be game logic in a place
  CLAUDE.md reserves for assertions and navigation. The volume's own wording IS
  asserted, because it is a `tellraw` and arrives on the chat stream.
- **A second death's retention policy** (`on_full: replace` / `keep`). The stage
  takes one death per declared volume. A campaign with two volumes exercises two
  deaths, but the fixture has one; and a respawned player is invulnerable for 59
  ticks, so a forced second death has to respect that window (spec-0032).
- **`collect_by: anyone`.** One bot, one player.

## Two harness actions on the world, both reverted, both named

Of exactly the class spec-0023 already sanctions for `/damage @s` and
`/effect give`, and neither reaches the shipped image:

1. **The currency objective is put on the sidebar display slot.** A vanilla server
   only broadcasts scores for an objective it is *tracking*, and tracking starts
   when the objective occupies a display slot. mineflayer's own scoreboard model
   cannot be used: its plugin gates every update on `packet.action === 0`, and
   1.21.11's `scoreboard_score` has no `action` field at all (reset was split into
   `reset_score`), so `bot.scoreboards` never updates on the pinned version. The
   raw packet decodes perfectly, so the harness reads it directly. The slot is
   released at the end of the stage.
2. **The bot takes its respawn manually, one second after dying.** mineflayer
   answers the death packet in the same event-loop turn, so a default bot is alive
   again on the very next server tick — and the engine's death edge is specified
   *on the corpse* (`if data entity @s {Health:0.0f}`). No human respawns inside
   one tick. This removes a client-library artifact; it does not weaken anything.

The bot also treats every declared lethal volume as impassable when pathfinding,
exactly as the compiler does, so the walk back from a death never routes through
the hazard that caused it.
