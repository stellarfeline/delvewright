# spec-0011 — Traps (lethal & non-lethal environmental hazards)

- **Status**: Approved — **signal half superseded by
  spec-0022**. Redstone keeps exactly one job, the
  **trigger**; signal transmission and the consequence itself are now commands
  (`traps[].payload`). Everything else in this spec **stands**: the trigger
  hardware layer (pressure plate / tripwire / trapped chest at an `anchor/trap`
  marker), the disarm affordance, the flag-gating surface (`DW0363`) and the
  completability proof obligations (`DW0342`). The `effect: {dispense}` surface
  below remains valid and byte-identical for existing campaigns, but new content
  authors `payload` instead. See `docs/specs/spec-0022-traps-v2-command-driven.md`.
  Note also that the §"Effect inventory" verdicts on **released falling blocks**
  ("excluded — landing cell unmodeled") are superseded specifically: spec-0022's
  `collapse` models the settled debris at compile time and re-proves the critical
  path against it, which is what made the effect admissible.
- **Owner framing** (paraphrased): the DSL must make room for traps — and
  the open question was whether they live as prefabs or need another
  implementation layer.
- **Answer (this spec)**: a **three-layer hybrid**, mirroring the existing
  wave-anchor split. ADRs: 0003 (vanilla-first), 0004 (prefab/jigsaw), 0006
  (determinism). Precedent: spec-0008 §7 environment triggers, spec-0010
  gravity-settle assembled model.
- **Hard invariant (this week's gravity incident)**: nothing may exist in the
  shipped world that the compiler does not model. Unmodeled mechanics poison the
  completability proofs. Every trap primitive below is admitted only because the
  compiler can model and prove it.

## Motivation

Delves want danger the world deals on its own — a dart volley from a tripped
plate, a spike pit, a released ambush. The DSL has no trap primitive. Traps are
almost entirely **redstone-native** (the hardware fires without any command), so
the temptation is to hide them in prefabs and forget them. That is exactly the
gravity-incident failure mode: a hazard the compiler cannot see cannot be proven
survivable or avoidable, and it silently breaks "provably completable."

## Architecture — three layers

1. **Hardware → prefab** (`anchor/trap` marker, like `anchor/wave`). The trap
   mechanism — tripwire+string+hooks, pressure plate, trapped chest, dispenser,
   the redstone wiring between them — is authored in the `.nbt` piece. The
   tileset generator provides trap-bearing pieces; admission records the trap
   hardware in metadata (block inventory + the `anchor/trap` region).
2. **Semantics → DSL stage-5** (`traps[]`). The author declares *what the
   hardware means*: trigger kind, effect/payload, lethality, disarmability,
   reset, and any quest coupling. Hardware is mute geometry until stage-5 gives
   it meaning — exactly as a `collect` chest is a prefab anchor whose item/count
   live in the DSL.
3. **Proofs → compiler**. Trap-aware walkability & waypoint export, the
   survivability/avoidance/disarm completability obligations, and PackTest
   assertions. The compiler emits **only** the dispenser payload and (if the
   trap is quest-coupled) the interaction-entity detection — the redstone is
   already in the prefab.

### The load-bearing split: HARM vs QUEST-COUPLING

- **Harm is redstone-native** → the compiler emits **no detection at all**. A
  lethal dart trap is plate → dispenser, entirely in-prefab; the compiler only
  *models* the trigger cell as a hazard and *proves* the delve completable
  around it.
- **Quest coupling** (disarming sets a flag; firing opens a gate) is observed
  **only** through the v0.4 interaction-entity triggers (`strike`/`use`/
  `approach`, spec-0008 §7) — never by reading block power. `execute if block
  <pos> …[powered=true]` on the tick is **polling** (verified live: the
  `powered` blockstate is readable that way) → a hack under the no-hack doctrine
  → **excluded**. Most traps therefore emit zero commands.

## Trigger inventory (1.21.11, live-verified)

All blocks exist in the pinned registry (live `setblock` on 1.21.11). Player-vs-
mob distinguishability matters because a sealed box-garden has controlled mobs.

| Trigger | Detects | Player-distinct? | Reset | Verdict |
|---------|---------|------------------|-------|---------|
| Stone pressure plate | any mob/player (not items/arrows) | no | auto (rearms on step-off) | **supported** |
| Wooden/weighted plate | + items/arrows/projectiles | no | auto | supported (weighted = analog) |
| Tripwire + 2 hooks + string | any entity crossing the line | no | auto | **supported** |
| Trapped chest | a player *opening* it (comparator/redstone pulse) | **yes** (open = player action) | on close | **supported** (only player-distinct trigger) |
| `approach` (interaction/`distance`) | player within range | yes | per-tick | supported (already v0.4) |
| Sculk / calibrated sculk sensor | *vibrations*; step = frequency **1**, no entity-type filter | **no** | 10t active +10t cooldown | **constrained** (see below) |
| Observer / target / lever | block-update / projectile-hit / manual | n/a | auto | hardware-internal only |

Sculk sensors cannot tell player from mob and fire on any step (verified: wiki
frequency table, "player or entity steps" = 1, no type filter). In a delve with
roaming mobs a sculk trap is non-deterministic as to *who* trips it → **admitted
only as pure ambience or where no mob can enter the sensor's range** (compiler
cannot prove otherwise, so it is not a completability-bearing trigger).

## Effect inventory (1.21.11, live-verified)

| Effect | Mechanism | Determinism / integrity | Verdict |
|--------|-----------|-------------------------|---------|
| Dispenser: arrows / tipped arrows | static `Items` NBT payload, redstone-fired | payload round-trips (`data get block … Items` verified); no terrain change | **supported (primary lethal)** |
| Dispenser: harming/poison/slowness splash potion | payload | same | **supported** |
| Dispenser: fire charge / lava bucket | payload | fire is bounded by sealing `fire_spread_radius_around_player 0`; lava placement is a modeled `setblock` | **constrained** (compiler must model the lava/fire cell as hazard geometry) |
| Mob release | reuse `spawn-wave` from a trigger's `effects` | already modeled & seated (DW0312) | **supported** |
| Lava release | `set-block` to lava at a modeled anchor | deterministic; lava-flow cells must be pre-modeled as hazard, not left to fluid tick | **constrained** |
| Falling-block hazard (suspended gravel/anvil ceiling) | **static**, settled by the gravity model | now provable — the assembled model already settles it | **supported, static only** |
| Released / dynamically-dropped falling block | runtime `falling_block` entity | landing cell + terrain change are **unmodeled** by the assembled model | **excluded** |
| **TNT** | primed TNT / ignited block | **block destruction deforms the sealed jigsaw world; determinism/integrity breach** | **excluded** |
| Crusher / dynamic piston terrain | moving blocks change the voxel grid at runtime | unmodeled world mutation | **excluded** |

### Why TNT is excluded (1.21.11 evidence)

No gamerule separates explosion **block** damage from **entity** damage.
Live-verified gamerule set: `tnt_explodes` (bool, all-or-nothing),
`tnt_explosion_drop_decay`, `block_explosion_drop_decay`,
`mob_explosion_drop_decay` (loot only, **not** destruction), `mob_griefing`
(does **not** gate TNT — TNT is not mob-griefing). There is no "damage players,
spare terrain" TNT. A TNT trap therefore mutates the assembled world the compiler
has proven → poisons every downstream proof (the gravity invariant). Excluded on
principle; the lethal punch TNT would provide is delivered by dispenser payloads,
which leave terrain untouched.

## Completability proof obligations

Death is recoverable but costly: sealing sets `keep_inventory true` and
`setworldspawn` = first area's `spawn` (verified in emission), so a trap death
**respawns the player at the delve entrance**, not in place. An unavoidable
lethal trap deep in the delve can thus soft-loop the player. For every trap whose
trigger cell touches the DW0311-proven **critical path**, the compiler must
discharge exactly one of:

- **(a) Avoidable** — the trigger cell is never a critical-path/waypoint cell;
  the exported waypoints already steer the bot clear. Preferred. Failure →
  **DW0314**.
- **(b) Survivable** — worst-case effect damage is bounded below lethal given
  the class kit (armor/health), **or** respawn-safe *and* non-re-triggering on
  the walk back from world-spawn (no soft-loop). 
- **(c) Disarmable** — a disarm affordance (an `interact`/`use` trigger that sets
  a `disarmed` flag gating the hazard, or a lever/route the redstone respects) is
  on the quest graph, reachable **before** the trap cell is forced.

Non-critical-path (branch/optional-area) lethal traps carry no obligation beyond
not sealing off a mandatory anchor (existing DW0306 gate-reachability covers
that).

### Harness (logic stays out)

The mineflayer bot **avoids** trap cells by following the compiler-proven
waypoints (which already route around hazards); it contains no trap logic.
Deliberate *trigger-for-coverage* is a **PackTest** concern — and must be:
GameTest ticks entities and simulates a player, whereas bare headless RCON in a
0-player void **does not tick entities** (verified: primed-TNT fuse frozen at 80,
falling sand suspended mid-air — see Findings). PackTest is the only place a trap
actually *fires* in validation.

## Authoring surface (concrete)

**Tileset generator provides**: trap-bearing pieces with one or more
`anchor/trap` markers and the pre-wired hardware (plate/tripwire → dispenser
socket; the dispenser block present, payload empty). Metadata records the trap
hardware block inventory.

**Stage-5 `traps[]`** (dsl_version bump; additive, `deny_unknown_fields`):

```json
{ "id": "trap/dart-hall",
  "at": "anchor/trap",
  "trigger": "pressure-plate | tripwire | trapped-chest | approach",
  "effect": { "dispense": { "item": "minecraft:arrow", "count": 8 } },
  "lethality": "lethal | harmful | nonlethal",
  "disarm": { "via": "anchor/lever", "sets_flag": "flag/darts-off" },
  "reset": "once | rearm",
  "requires_flags": [] }
```

`effect` is one of `dispense{item,count}`, `release-wave{wave}`, `set-hazard
{block}` (lava/fire, modeled). Payload item/mob ids validated against 1.21.11.

**Compiler emits**: the dispenser payload (`setblock`/`data` — deterministic,
verified static); detection **only** when the trap is quest-coupled or
disarmable (interaction entity, reusing v0.4); nothing otherwise. Models the
trigger + hazard cells in nav; runs the obligations above; emits PackTest
assertions (fires → damage/effect observed; `disarm` flag suppresses; `once`
honored).

## Diagnostics (proposed, per catalog conventions)

| Code | Tier / exit | Meaning |
|------|-------------|---------|
| `DW0197` | validation / 1 | Trap declaration invalid: `at` not an `anchor/trap` the bound prefab provides, bad `trigger`/`effect`/`lethality` enum, duplicate trap id, or `disarm.via`/`sets_flag` dangling. |
| `DW0198` | validation / 1 | Trap payload item/entity id not in the pinned 1.21.11 registry (mirrors DW0143/DW0173). |
| `DW0314` | analysis / **2** | A lethal trap's trigger cell lies on the forced critical path with **no discharge** — not avoidable, not proven survivable, not disarmable (player provably killed or soft-looped). Content-design mistake (move the trap off the path, make it survivable, or add a disarm), analysis-tier like DW0312. |
| `DW0733` | delve-admit / DW07xx | Audit: a piece contains trap hardware (dispenser, wired plate/tripwire, trapped chest) **not** declared by an `anchor/trap` marker + trap metadata — an unmodeled mechanism. Refuse admission (the gravity-incident guard: no hardware the compiler cannot model reaches a shipped world). |

## Acceptance criteria (machine-checkable)

1. Schema: `traps[]` on stage-5 with `deny_unknown_fields`; reserved under the
   pre-bump `dsl_version` → `DW0141`.
2. Determinism: same DSL + seed → byte-identical dispenser-payload and detection
   emission (ADR-0006 double-build).
3. A `dispense` trap on a bound `anchor/trap` piece compiles; PackTest fires the
   trigger and asserts the payload effect lands (arrow/potion damage observed).
4. Quest-coupled disarm: PackTest proves the hazard is active before the disarm
   flag and suppressed after; `once` honored.
5. Avoidance: a lethal trap whose trigger cell is off the critical path builds
   clean and the exported waypoints never enter the cell; the bot ladder passes
   without a trap death.
6. **DW0314**: a lethal trap placed on the forced critical path with no
   survivability/disarm → DW0314, exit 2.
7. **DW0733**: a prefab carrying an undeclared dispenser/wired-plate → admission
   refuses with DW0733.
8. TNT / released-falling-block / crusher `effect` → rejected at validation
   (not in the `effect` enum); documented non-goals stay non-goals.
9. Static gravity-hazard (suspended gravel ceiling) settled by the gravity model
   is modeled as hazard geometry and does not desync the assembled world.

## Non-goals

- TNT and any block-destroying explosion; dynamic/released falling blocks;
  crushers / runtime terrain mutation (all unmodeled → excluded).
- Block-power polling for quest state (hack; excluded).
- Look-at / break-attempt triggers (no vanilla primitive; same exclusion as
  spec-0008 §7).
- Per-area checkpoint respawn / respawn anchors — split out as spec-0012
  (owner decision 2).
- Trap "balance" tuning beyond `lethality` + payload; randomized/timed traps.

## Resolved decisions

1. **Lethal traps ARE allowed on the forced critical path**, gated by the hard
   proof obligations: every critical-path trigger cell must discharge
   avoidable OR survivable OR disarmable, else DW0314. Restricting lethal traps
   to branch areas was rejected — maximum expressive space, enforced by proof.
2. **Checkpoint / respawn-anchor feature approved as its own spec**
   (spec-0012, drafted alongside this spec; implementation may lag). Trap
   depth is therefore NOT bounded by respawn-to-entrance; until spec-0012 is
   implemented, the survivability proof must assume entrance respawn.
3. **Dispenser payload is DSL-authored** (compiler fills the prefab dispenser,
   mirroring `collect` item syntax). Prefab-baked payloads rejected.

## Design direction — souls-mode

Target expressiveness (owner, verbatim intent): Dark-Souls-grade malice —
corner kills, door-opening kills, alcove mobs that knock players off ledges,
timing-gated passages, lethal parkour, doors that cannot be opened from this
side; death or resting at a "bonfire" resets traps and enemies, making
death-driven trial-and-error a **legitimate design pattern**, not a failure.

This spec is the baseline and ships first. The souls-mode extension is designed
by the planning agent personally (owner assignment) in the expressiveness
phase, as a follow-up spec. Implications parked there: reset semantics
(vanilla `deathCount` scoreboard criterion + bonfire = spec-0012 checkpoint +
a compiler-emitted re-arm function for traps/waves — all modeled, no new
hacks); one-way doors need direction-aware nav edges in the walkability model;
timing-gated passages and lethal parkour need timing-/jump-aware provability —
the hardest proof extensions, likely staged.
