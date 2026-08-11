# What a missing scoreboard entry does — measured, 1.21.11

The whole of `DW0495` (`compiler::seeding`) rests on one claim about the pinned
runtime: **a scoreboard entry that was never written is not zero — every
comparison against it is false.** A check built on an unverified premise about
the server is worth nothing, so it was measured rather than assumed. This note is
the transcript, so the next session can re-run it instead of re-deriving it.

Server: `validation/compose.yaml --profile play`, pinned Minecraft Java
**1.21.11** (`versions.toml` `[minecraft].version`), commands issued over
`rcon-cli`, observations read back from the server log (`say` is a broadcast, so
it survives the delve's `gamerule send_command_feedback false`). Sections E–Q
used a real client — a mineflayer bot on the pinned `[harness].mineflayer` pin —
because a joined **player** is the only holder whose criterion-backed objectives
the server maintains.

```bash
delvec build crates/dsl/fixtures/valid/hello-world -o validation/delve-output
EULA=TRUE docker compose -f validation/compose.yaml -f validation/ephemeral-port.yaml \
    -p dw-<yours> --profile play up -d
CID=$(EULA=TRUE docker compose -f validation/compose.yaml -f validation/ephemeral-port.yaml \
        -p dw-<yours> --profile play ps -q server)
docker exec "$CID" rcon-cli "<command>"
```

## 1. A comparison against a holder with no entry

`Ghost` has no entry anywhere; `Real` holds 1.

| command | fired? |
|---|---|
| `execute if score Ghost t.dummy matches 0 run say …` | **no** |
| `execute if score Ghost t.dummy matches 1 run say …` | no |
| `execute if score Ghost t.dummy matches 0.. run say …` | **no** |
| `execute if score Ghost t.dummy matches 1.. run say …` | no |
| `execute unless score Ghost t.dummy matches 0 run say …` | **yes** |
| `execute unless score Ghost t.dummy matches 1 run say …` | yes |
| `execute if score Real t.dummy > Ghost t.dummy run say …` | **no** |
| `execute unless score Real t.dummy > Ghost t.dummy run say …` | yes |
| `execute if score Ghost t.dummy = Ghost2 t.dummy run say …` (both unset) | no |

So the rule is uniform and has no exceptions: **the comparison is false**, and
`unless` is therefore true. The rows in bold are the ones that make this a defect
class rather than a curiosity — at an entry holding 0 each of them would have
gone the other way.

The dual matters as much as the direct case: `unless … matches 1` behaves
identically at "unset" and at 0, which is exactly why the flag idiom
(`docs/reference/compiler.md` §`set-flag`) is safe, and why the general rule is
about a **range that excludes 0** rather than about the sense of the test.

## 2. What creates and destroys an entry

| command | effect |
|---|---|
| `scoreboard players set <h> <o> <v>` | creates, at `v` |
| `scoreboard players add <h> <o> 0` | creates, at 0 (`Added 0 … (now 0)`) |
| `scoreboard players operation <h> <o> = <s> <so>` | creates the **target** — and also the **source**, at 0 if it had none |
| `execute store result score <h> <o> …` | creates |
| `scoreboard players enable <h> <trigger>` | creates, at 0 |
| `scoreboard players reset <h> <o>` | **destroys** the entry (a later `matches 0` does not fire; a whole-domain `unless` does) |

The `operation` row is the surprising one and is load-bearing for the checker:
reading a score as an operation's source creates it. Verified in isolation —
`SrcGhost` had no entry (`unless … matches -2147483648..2147483647` fired), then
`scoreboard players operation Dst p.op = SrcGhost p.op`, after which
`if score SrcGhost p.op matches 0` fired.

## 3. A selector's `scores={…}`

`execute as @e[tag=probe,scores={t.dummy=0}] run say …` matched **nothing** while
the marker had no entry, and matched after `scoreboard players set @e[tag=probe]
t.dummy 0`. A selector clause reads exactly like the `if score` it stands for.

## 4. A real player, and the criteria

A joined bot (`probe-bot`), before doing anything:

* `dummy` objective — **no entry**. Joining seeds nothing.
* `deathCount` objective — **no entry** until the player dies.
* `minecraft.custom:minecraft.jump` — **no entry** until the statistic moves.
* `trigger` objective — **no entry** until `scoreboard players enable`.

## 5. The death edge, both arms

The engine's own shape, run twice on the same bot's **first** death — one arm
seeded ahead of time, one not:

```
scoreboard objectives add x.deaths deathCount   # treatment
scoreboard objectives add x.ack    dummy
execute as probe-two run scoreboard players add @s x.deaths 0
execute as probe-two run scoreboard players add @s x.ack    0
scoreboard objectives add y.deaths deathCount   # control, unseeded
scoreboard objectives add y.ack    dummy
kill probe-two
```

Before the death, `x.deaths` read 0 and `x.ack` read 0. After the **first**
death, `x.deaths` read 1, and:

* `execute as probe-two run execute if score @s x.deaths > @s x.ack …` — **fired**
* `execute as probe-two run execute if score @s y.deaths > @s y.ack …` — **did not fire**

That is the whole defect and the whole fix, side by side on one death, and it
also settles the one worry about the fix: `add … 0` on a `deathCount` objective
does not disturb the criterion — it read 0 before the death and 1 after.
