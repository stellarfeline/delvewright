# The shield, and one body against a five-mob wave

Measured on the pinned Minecraft Java 1.21.11 server (`versions.toml`
`[images.base]` `mirror_of`) by `tools/spike-guard-fight/run.sh`. Every number
here is the SERVER's: the player statistics `damage_blocked_by_shield`,
`damage_taken`, `damage_resisted` and `damage_dealt`, read as scoreboard
objectives whose creation is verified before any zero from them is believed, each
cross-checked against entity NBT (`Health`, the wave's summed health) — two arms
that share no configuration. Raw observations sit beside the rig.

The subject of §2 and §3 is `campaign/vesperhold`'s `wave/unremembered-guard`
built by `delvec build` and booted as the shipped delve image, so the fight
happens in the campaign's own great hall with the campaign's own bytes and
`difficulty=normal`.

## 1. The shield

`shield.mjs`, superflat rig, one NoAI husk, `/damage <bot> 6.0
minecraft:mob_attack by <husk>`, the off-hand shield raised with mineflayer's
`activateItem(true)` — the same call the harness makes.

**It blocks, and it blocks completely.** A blocked blow moves
`damage_blocked_by_shield` by the whole 6.0 and `Health` not at all. The server's
`equipment.offhand` holds the shield and `execute … if items entity @s
weapon.offhand minecraft:shield` passes, so `activateItem(true)` does put the
SERVER in the blocking state. None of this was ever in evidence before, because
every instrument the harness had is blind to a blocked blow (`melee.ts`,
`MeleeTally`).

**Warm-up** (N = 6 per row, the shield raised for a known number of host ticks
before the blow is sent):

| ticks up | blocked | health lost |
| --- | --- | --- |
| never raised | 0/6 | 3.36 every time |
| 0 | 0/6 | 3.36 |
| 2 | 0/6 | 3.36 |
| 3 | 3/6 | 3.36 on the three that did not block |
| 4, 5, 6, 8, 12, 20 | 6/6 | 0 |

One rcon round trip lands between the last `sleep` and the blow, so the host-tick
column runs about a tick behind the server's own count; the boundary it brackets
is vanilla's `block_delay_seconds` 0.25 s. `SHIELD_WARMUP_MS = 250` stands.

**Arc** (N = 4 per row, shield warm, yaw offset from facing the attacker):
blocked 4/4 at 0°, 45°, 80°, 85°, 88° and 90°; blocked 0/4 at 92°, 95°, 110° and
180°. That is `blocks_attacks`'s `horizontal_blocking_angle` of 90 — a 180°
arc — and it is the whole reason a shield is worth so much less against a crowd
than against one body (§3).

## 2. What the two sides actually carry

Read with `attribute … get`, which is the total after the held item's own
modifiers — never the declared attribute alone.

| | declared | effective |
| --- | --- | --- |
| Guard `max_health` | 34.0 | 34.0 |
| Guard `armor` | 2.0 (zombie base) | **10.0** (iron helmet + iron chestplate) |
| Guard `attack_damage` | **6.0** | **11.0** (the iron sword adds +5) |
| Sellsword `armor` | — | 14.0, toughness 0 |
| Sellsword `attack_damage` | — | 6.0 (iron sword) |
| Sellsword `attack_speed` | — | 1.6 |

A Guard's blow on the Sellsword: 11 × (1 − 8.5/25) = 7.26 after armour, × 0.4
under spec-0023's Resistance III = **2.904**, which is what the statistics show
(`damage_resisted` / (`damage_resisted` + `damage_taken`) = 60.3% on every
fight). A Sellsword swing on a Guard: 6 × (1 − 7/25) = **4.32**, confirmed by
`damage_dealt` (8.6 for two swings) and by the wave's summed health.

## 3. One body cannot clear it

**As the harness fights** (`guard.ts` — the harness's own `MineflayerExecutor`
running its own `fightWave` under its own `withAssist`, nothing replaced;
N = 12): **0 won, 12 died**, five of five bodies still standing every time.
Mean 7.28 s to die (5.56–8.77). Damage in 3.39 HP/s, damage out 1.58/s — 7.3% of
the wave's 170 health. At that output the wave takes 108 s, which is **14.8×**
the bot's own lifetime.

**The defensive ceiling** (`hold.mjs` — a body that never swings, holds the
shield up for the whole fight and turns to the nearest Guard every 100 ms;
N = 4): it dies anyway, in 12.39 s (11.88–12.80), having blocked 8.75 blows and
taken 8.99. So **49% blocked is the most a single body can get out of a 180° arc
against five**, blows arrive at 1.43/s, and the body takes 2.11 HP/s with the
shield up and 4.16 HP/s with it down.

The arithmetic those two measurements close:

- clearing 5 × 34 = 170 health at 4.32 a swing needs **39.4 full-charge swings**,
  which at the harness's own `fullChargeMs(1.6)` = 650 ms is **25.6 s of
  uninterrupted swinging**;
- a swinging body's shield is down, so those 25.6 s cost **106.3 HP** at the
  measured 4.16 HP/s — before any time spent walking, re-targeting or waiting;
- the Sellsword's pool, crediting all four Vigil Draughts at face value and for
  free, is 20 + 4 × 8 = **52 HP**.

**106.3 against 52: one body is 2.04× short**, and that is the generous bound.
Each draught takes 32 ticks to drink with the shield down and no swing made —
about 6.7 HP of incoming for 8 HP of healing — so the four of them are worth
nearer 5 HP than 32, and the honest figure is around 4×.

**Four bodies close it.** 39.4 swings split four ways is 9.9 each, 6.4 s of
swinging; five attackers spread over four targets cut the per-body arrival from
1.43 to roughly 0.36 blows/s, so each body pays about 6.7 HP against a 20 HP bar
— a ~3× margin before a single draught. The encounter is winnable at the top of
the delve's 1–4 party range and not at the bottom, and the machine ladder drives
one body.

## 4. What the compiler would have had to know

spec-0023 §2 asks the compiler to prove "incoming damage is survivable-with-play".
`crates/delvec/src/compiler/combat.rs` implements that arm only for
*unconditional scripted* damage — `collect_unconditional_damage` over a quest's
own effect bundles, against `PLAYER_MAX_HEALTH`. A wave's melee output is never
weighed against the party's effective HP at all, and the time-to-kill arm counts
swings against a budget rather than against how long the party lives. So this
encounter compiles green, and the two numbers that would have caught it — the
mob's effective `attack_damage` (11, not the declared 6) and the swing count
already computed one function away (39.4, against a 20 HP bar) — are both
already in the compiler's hands.
