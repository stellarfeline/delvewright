# delvec compiler — behavior reference

**Single authoritative record of *current* compiler behavior.** Specs
(`docs/specs/`) remain the historical decision records; this file is what the
compiler does today. A PR that changes compiler behavior updates this file in the
same PR (CLAUDE.md Methodology; CI enforces the DW-code subset — see
`tools/check-dw-codes.py`).

- Binary: `delvec` (`crates/compiler`, Rust-native, ADR-0011). The other binaries
  and scripts around it — `delve-schem`, `delve-admit`, `delve-render`,
  `delve-harvest`, `tools/`, `validation/` — are indexed in
  [`tools.md`](tools.md).
- Versions (as of this doc): `delvec 0.1.0`, `dsl 0.6.0`, `mc 1.21.11`.
  Supported campaign `dsl_version`: **`0.2.0`, `0.3.0`, `0.4.0`, `0.5.0`, `0.6.0`**
  (additive supersets; `0.2.0` output stays byte-identical across the later
  versions).
- v0.6 amends spec-0010's mitigation hierarchy: the night-vision mitigation is now
  the stage-1 `areas[].mitigation` **declaration** (emitting a real clocked
  `effect give`), not a class-kit display-name heuristic.
- spec-0010 (#35) has **landed** at `dsl_version 0.5.0`: stage-1
  `lighting`/`time`/`weather`, effect verbs `set-time`/`set-weather`, the
  assembled-world light model + deterministic relight pass (`crate::light`), the
  measured redefinition of `DW0210`, and diagnostics `DW0211`/`DW0196`.
- spec-0012 (#47) checkpoints + the spec-0014 stealth verbs have **landed** at
  `dsl_version 0.6.0`: stage-5/dialogue `set-checkpoint{anchor, on_respawn?}`
  (party-wide `spawnpoint @a` + the `storage dw:cp pos` mirror), the stage-5
  `begin-stealth{zones, on_caught?, grace_ticks?}` / `end-stealth` verbs, the
  no-stranding / placement proofs `DW0315`/`DW0316`, and the stealth-zone proof
  `DW0327`. The `dw:cp` mirror is the shared "last checkpoint" contract spec-0013's
  boundary return reads.
- spec-0013 has **landed** at `dsl_version 0.6.0`: stage-1 `horizon`
  (`ocean` superflat sea backdrop) and `boundary` (derived playable region +
  per-second return-to-checkpoint clock), diagnostics `DW0320`/`DW0321`, and the
  `dw:region`/`dw:cp` storage mirrors. Absent `horizon`/`boundary` keeps v0.5
  output byte-identical.
- spec-0014 sound + art-title have **landed** at `dsl_version 0.6.0`: the
  `play-sound` effect (`DW0326` unknown sound, `DW0335` deferred `at: actor`) and
  the `narrate` `style: art` `delve:art` pixel-banner font (`DW0328` glyph
  coverage). The remaining v0.6 surface (scripted actors) lands in a sibling PR
  under the same version gate.
- task #55 has **landed** on `dsl_version 0.6.0`: per-effect `requires_flags` (a
  per-player `execute if score @s dw.f_<flag> matches 1` guard on any quest /
  trigger effect except `campaign-complete`) and a verbatim blockstate suffix
  `id[key=value,…]` on `set-block`/`interact.prop` blocks. Both default to
  absent, so a campaign that uses neither is byte-identical.
- spec-0011 (traps) has **landed** at `dsl_version 0.6.0`: the stage-5 `traps[]`
  surface (redstone-native hazards bound to `anchor/trap` markers — trigger
  `pressure-plate`/`tripwire`/`trapped-chest`, a compiler-filled `dispense`
  payload, `lethality`, `disarm`, `reset`), the completability proof `DW0342` (a
  forced lethal trap must be avoidable, `once`-survivable, or disarmable), the
  declaration/payload validation `DW0340`/`DW0341`, and the defense-in-depth
  `gamerule tnt_explodes false` seal (v0.6-gated). Absent `traps` keeps pre-0.6
  output byte-identical. The trap `effect` is `dispense` only in this landing;
  `release-wave`/`set-hazard` (spec-0011's other effect variants) and the
  `anchor/trap` prefab-hardware admission audit are deferred to follow-ups. The
  spec's reserved diagnostic numbers were stale (all taken since) and were
  renumbered off them (0197/0198/0314) to `DW0340`/`DW0341`/`DW0342`.

---

## 1. Pipeline overview

### Pass order (`delvec build`)

| # | Pass | Crate/module | Fails with |
|---|------|--------------|-----------|
| 1 | Load campaign dir (6 stage docs + optional `world-edits.json` + `l10n/` sidecars) | `compiler::load` | internal (≥10) on unreadable dir |
| 2 | Parse (serde, `deny_unknown_fields`) | `dsl::parse_campaign` | `DW0100` (exit 1) |
| 3 | Validate stages 1–7 (schema + referential, full injected registries) | `dsl::validate_campaign_with` | `DW01xx` (exit 1) |
| 4 | l10n sidecar coverage + reserved marker channel | `dsl::validate_l10n`, `dsl::validate_marker_channel` | `DW0180`/`DW0181`/`DW0182` (exit 1) |
| 5 | Analyze (branch-coherent quest/dialogue reachability + critical-path replay) | `compiler::analyze` over `compiler::flow` | `DW02xx` (exit 2) |
| 6 | Solve jigsaw layout (per `prefab_pool` area, from seed) | `compiler::solver` | `DW030x` (exit 3) |
| 7 | Assemble world model (placed pieces → voxel grid; ocean sea-level datum check) | `compiler::plan` | `DW030x`/`DW0344` (exit 3) |
| 8 | Replay the stage-7 edit script over the assembled model (spec-0017; per-batch invariant re-proofs — trap-hardware integrity, gravity, relight, walkability, boundary safety, block support; plus the advisory gate-region check). Skipped entirely for a campaign without one (byte-identical). | `compiler::edit` | `DW0322`/`DW0323`/`DW0352`/`DW0354` + reused invariant codes, batch-attributed (tier per code); advisory `DW0353`/`DW0354` |
| 9 | Assembled-light + relight (measure, place fixtures; over the **edited** model when a script exists) | `compiler::light` | `DW0210`/`DW0211` (**exit 2**) |
| 10 | Nav checks (A* `move-npc`/`move-actor` (footprint-aware), cutscene clip (authored polyline + rendered keyframe chords) + angular budget, critical-path walkability — incl. relight fixtures + water flood; talk-to endpoint snap; waypoint self-check; POV camera clear-eye self-check; v0.6 checkpoint no-stranding/placement + stealth-zone/onset + trap completability proofs; spec-0016 §6 TD lane polylines) — all over the **edited** model when a script exists | `compiler::nav` | `DW0307`/`DW0308`/`DW0311`/`DW0314`/`DW0315`/`DW0316`/`DW0325`/`DW0327`/`DW0342`/`DW0347`/`DW0355`/`DW0386`/`DW0724` (exit 3; `DW0342` → exit 2) |
| 11 | Referential + placement seals inside emission: every anchor-bearing effect resolves (`DW0360`), no generated name collides (`DW0361`), no body eclipses an interaction affordance (`DW0359`, `compiler::eclipse`) | `compiler::emit` | `DW0359`/`DW0360`/`DW0361` (exit 3) |
| 12 | Emit (datapack incl. the `world_edits` function, packtest, server, critical-path, resourcepack) | `compiler::emit` | `DW0300`+ (exit 3) |

- `build` ⟹ `validate` + `analyze`; `analyze` ⟹ `validate`. A validation failure
  short-circuits (exit 1) before analysis; analysis failure (exit 2) before build.
- The assembled-light gate (`DW0210`/`DW0211`) runs inside the build (it needs the
  placed geometry) but is analysis-tier: `main` maps a `DW02xx` build diagnostic to
  **exit 2**. Its relight fixtures feed both `setup_finish` emission and the nav
  re-verification in pass 9.
- Every emitted `.mcfunction` line is checked against the vendored 1.21.11
  Brigadier tree (`compiler::commands`; structure-only — arity/paths, not arg
  values). mecha re-validates in CI (ADR-0011); disagreement fails CI.
  **Single-entity arity** (round-7 live finding, spec-0018): an entity argument
  the tree marks `amount: "single"` rejects `@a`/`@e` without `limit=1`.
  `damage @a[…] 40 minecraft:generic` is a well-shaped command that 1.21.11
  refuses to *load* ("Only one entity is allowed…") — taking the whole enclosing
  function down with it, silently. The tree already carries the fact, so the
  compiler enforces it rather than leaving it to folklore; the party form of
  `damage-players` is `execute as @a[…] run damage @s …`.
- Determinism (ADR-0006): all map/set iteration is `BTreeMap`/sorted; the only
  randomness is stage-1 `seed` → a named splitmix64 per-area stream.

### CLI contract

```
delvec validate <dir>                      # stages 1–7 schema + referential
delvec analyze  <dir>                      # + quest-graph reachability
delvec build    <dir> -o <out>             # full deterministic build
delvec schema   --stage <1..7|all>         # export JSON Schema
delvec l10n-inventory <dir> [--lang <c>]   # l10n key inventory as JSON (translation input)
delvec snapshot <dir> [framing] [-o f.png] # draft frame + scene manifest (§7)
delvec blocking-chart <dir> [-o dir]       # per-elevation cutaway floor plans (§7)
delvec edit apply   <dir> [--batch f] [-o dir]  # replay edit script (+ candidate), persist on green (§7)
delvec edit preview <dir> [--batch f] [-o dir]  # same replay + renders, never persists
delvec calibrate <report> --layout <layout.json> [-o f.json]
                                           # harvested shot proposals -> anchor+offset DSL patch (§8)
delvec --version                           # "delvec 0.1.0, dsl 0.6.0, mc 1.21.11"
```

Global flags: `--json` (one JSON diagnostic object per line), `--prefabs <dir>`
(default `campaigns/prefabs`), `--lang <code>` (default `en`; affects `build`
only — `validate`/`analyze` are language-independent apart from coverage).

**Exit codes**: `0` ok · `1` validation failure · `2` analysis failure · `3`
build failure · `≥10` internal error. Undeclared `--lang` is a validation-class
rejection (exit 1). Codes are stable API; the CI fixture matrix asserts them.

**`--json` diagnostic shape**:
`{ "code":"DW####", "severity":"error|warning", "stage":"<stage>",
"path":"<json-pointer-ish>", "message":"…" }`.

**Severity is load-bearing.** `delvec` exits non-zero only on `error`. A `warning`
is printed and emitted in `--json` exactly like an error but never fails
`validate`/`analyze`/`build`. The tier is reserved for rules whose verdict depends
on something outside the campaign — `DW0330`, where how much text fits depends on
the player's window size and GUI scale; `DW0359`'s crowding tier, where whether a
neighbouring body shadows an affordance depends on the approach angle a player
takes — or on authorial judgement the compiler may measure but must not overrule
(`DW0351`, `DW0353`, `DW0354`'s decoration tier, `DW0379`, `DW0380`).

---

## 2. DSL surface (per stage)

Envelope (every stage): `{ dsl_version, campaign_id, stage, content }`,
`deny_unknown_fields`. IDs are type-prefixed kebab-case; all cross-stage refs are
**strictly backward**. Source of truth: `crates/dsl/src/stages.rs` (schemas
exported via `delvec schema`). Introduced-by column cites the spec.

### Stage 1 — `world`

| Field | Behavior | Since |
|-------|----------|-------|
| `title` | Player-visible; l10n key `world.title`. | 0.1 |
| `outro` (opt) | The closing line on the campaign-completion advancement — the last player-visible sentence of the delve. Player-visible, so it is l10n-inventoried as `world.outro` and sidecars translate it. Absent = the **finale quest's `goal`** (already campaign-derived and inventoried as `quest.<q>.goal`), so the line is never hardcoded English either way. Before this the description was the literal `"You left the keep."` on every delve ever built — the reference keep-crawl's line, shipped to campaigns with no keep and unaddressable by any sidecar key, because it never passed through a `Campaign` field. | 0.6 |
| `theme`, `premise` | Authoring context; **excluded** from l10n. | 0.1 |
| `seed` (u64) | Sole downstream randomness (layout PRNG). | 0.1 |
| `target_minutes` | Informational (pacing). | 0.1 |
| `languages[]` (opt) | BCP-47 codes; `en` implicit/never listed; drives l10n coverage + `--lang`. | 0.3 i18n |
| `areas[]` | 1..N. Each binds **exactly one** of `prefab` or `prefab_pool`+`pieces{min,max}` (else `DW0160`). Area origin = `[i·256, base_y, 0]`, where `base_y` is the **horizon datum**: `void` → 64, `ocean` → 60 (see `horizon`). | 0.1 / pool 0.2 |
| `areas[].lighting {fixture,min_light}` (opt) | spec-0010: relight pass guarantees `min_light` (1..=14, default 7; `DW0196` out of range) over reachable walkable cells by placing `fixture` (`torch`/`lantern`/`campfire`/`shroomlight`), else `DW0211`. | 0.5 |
| `areas[].mitigation` (opt) | `night-vision` — the first-class darkness declaration (v0.6). The compiler emits a self-rescheduling **1 s (20t)** `night_vision_tick` that runs `effect give @a[<this area's placed bounds>] minecraft:night_vision 12 0 true` (amplifier 0, particles hidden). 12 s ≫ vanilla's 10 s wind-down, so the remaining duration never drops below 11 s and the effect never blinks; a player who leaves the area keeps it ≤ 12 s. Independent of `lighting`. This declaration is the **sole** `DW0210` night-vision mitigation. | 0.6 |
| `time` (opt) | `day`/`noon`/`night`/`midnight` (default `noon`). Dimension-global initial state, emitted in the sealing baseline (`time set <kw>`). | 0.5 |
| `weather` (opt) | `clear`/`rain`/`thunder` (default `clear`; `clear` emits nothing — byte-identical to pre-0.5). Dimension-global, emitted after sealing (`weather <kw>`). Rain/thunder attenuate the assembled-light sky term. | 0.5 |
| `horizon` (opt) | spec-0013: `void` (default/absent, byte-identical to v0.5) or `ocean` — a pinned bedrock/stone/water superflat (sea level y=62), no structures/mobs. Drives `generator-settings` **and the area-origin datum**: ocean areas are placed at y=60 = `sea_level − 2`, so an island piece's authored waterline (local y=2) meets the world ocean and its walk plane (local y=3) is the vanilla-normal one block above the sea. Enforced by `DW0344`. | 0.6 |
| `min_players` (opt, 1..=4) | spec-0018: the party size the delve **requires**. Absent = 1 (a party of one is always legal; every pre-0.6 campaign reads as 1). `>= 2` emits the **lobby gate**: `tick` recomputes the live count into `#lobby dw.sys`, the class-selection dialog driver is prefixed `if score #lobby dw.sys matches <n>..` (so the delve cannot START short-handed), and unclassed players get a self-updating `x / n` actionbar (`{"score":{"name":"#lobby","objective":"dw.sys"}}` — one emitted line, no per-count strings; a compiler default, not an l10n key). Out of range = `DW0356`; a mandatory-n declaration with no n-way division of labour = `DW0358`. `min_players: 1` emits **nothing** (byte-identical). | 0.6 |
| `boundary {margin?,message?}` (opt) | spec-0013: declares a **derived** playable region (union of final placed-piece AABBs, inflated horizontally by `margin` (`0..=64`, default 16; else `DW0321`), unbounded up, floor = lowest placed block − 8). A 1s clock returns any player outside it to the last checkpoint (`dw:cp`) with an actionbar `message` (l10n `world.boundary.message`, English default when absent) + a soft sound; no damage, no item loss. `horizon:"ocean"` without a `boundary` = `DW0320`. | 0.6 |

### Stage 2 — `npcs` (casting sheets, stationary)

| Field | Behavior | Since |
|-------|----------|-------|
| `id`,`name`,`area`,`anchor`,`base_entity` | NPC body placed at resolved anchor; `name` → l10n `npc.<n>.name`. | 0.1 |
| `role` | Enum `quest-giver|flavor`; `vendor`/`boss` reserved → `DW0141`. | 0.2 |
| `persona{archetype,speech_style,motivation,…,relationships[]}` | Structured; **excluded** from l10n; relationship refs validated in-stage (`DW0112`). | 0.2 |
| `skin{texture_id,model}` (opt) | Switches body to `minecraft:mannequin`; PNG baked to resourcepack. Missing PNG → `DW0309`; bad/dup id → `DW0190`. Every summon whose entity id comes from **content** rather than this switch (`npc.base_entity`, `actor.entity`, and the `unleash` twin, which has no skin branch at all) is spliced with `pose:"standing"` when that id names a mannequin (`emit::mannequin_pose_nbt`): a mannequin summoned without an explicit pose serializes it as `DYING`, which the server then fails to encode at save (`Failed to encode value 'DYING'` in a PackTest world's teardown). A non-mannequin entity gains nothing, so existing campaigns stay byte-identical. | 0.4 |
| `deferred` (opt, bool) | **Not** summoned at world init; the NPC's body + hitbox appear only when a `spawn-npc` effect fires, at this same `anchor` (the dual of `despawn-npc`). Default `false` = pre-0.6 behavior, byte-identical. Never spawned → `DW0197`; a `talk-to` provably ahead of every spawn → `DW0198`. | 0.6 |

### Stage 3 — `classes`

1..4 classes. `kit[]` = vanilla item id + count + optional display `name`
(→ l10n `class.<c>.kit.<i>.name`) + optional `carrier` (v0.6, spec-0018:
`all` (default) or `one`). A kit is per-player gear by construction; `carrier:
"one"` marks a **party-unique** kit item — `class_apply_<c>` guards that one
`give` behind a `#kit_<class>_<i> dw.sys` latch, so exactly one copy enters the
party (the first player to take the class) while the rest of the kit is unchanged. `name`/`blurb` player-visible. Reserved kit
fields `lore`/`enchantments`/`attributes` are **not defined** → unknown-field
`DW0100`. Kit items carry **no semantics**: a night-vision potion in a kit is
flavor. The `DW0210` dark mitigation is the stage-1 `areas[].mitigation`
declaration only — the pre-0.6 heuristic that read a kit item's id/display name for
`night_vision` was **deleted** (see §4 "Semantics never key on player-facing text").
Because the signal is a declaration, the `DW0210` verdict is language-independent
by construction (ADR-0006) — nothing is threaded past the `--lang` localization
pass any more.

### Stage 4 — `quest-plan`

Quest DAG skeleton: `depends_on` acyclic (`DW0130`), `finale` declared
(`DW0131`) + convergent sink (`DW0132`), `mandatory: true` required
(`DW0133` on `false`). `goal` → l10n `quest.<q>.goal`.

### Stage 5 — `quests` (+ v0.3/v0.4 gameplay surface)

| Element | Fields → behavior | Since |
|---------|-------------------|-------|
| `trigger` | `campaign-start` \| `quest-complete{quest}`. | 0.1 |
| Objective `talk-to` | `{npc}`; completes via a stage-6 dialogue option (backward). | 0.1 |
| Objective `reach-anchor` | `{anchor,radius}`; completes on a 3×3×3 box at the anchor cell (v0.3+; v0.2 = sphere). | 0.1 |
| Objective `kill` | `{wave,after?,requires_flags?}`; completes when wave countdown hits 0. | 0.3 |
| Objective `collect` | `{item,count,anchor,…}`; chest at anchor, `inventory_changed` advancement. | 0.3 |
| Objective `interact` | `{anchor,requires_item?,prop?,…}`; interaction entity; `requires_item` = `execute if items`. `prop{block}` `setblock`s the affordance (v0.4); `block` accepts a verbatim blockstate suffix `id[key=value,…]` (v0.6). | 0.3 / prop 0.4 |
| `after[]` | Ordering (acyclic → `DW0140`). | 0.1 |
| `requires_flags[]` | AND-gate on set flags (puzzle primitive). | 0.3 |
| `forbids_flags[]` | Negative gate, accepted **everywhere `requires_flags` is** (objectives, `triggers[]`, per-effect, dialogue options, `traps[]`): the element is suppressed while ANY listed flag is set. Per-player sites emit `unless score @s dw.f_<flag> matches 1` clauses (unset-safe — flag scores are never pre-initialized, so a `scores={…=..0}` selector would wrongly fail on unset); trigger arming uses the any-player form `unless entity @a[scores={dw.f_<flag>=1..}]` (a positive selector inside a negation). Unknown flags get the same `DW0172` treatment as `requires_flags`. Reserved (`DW0141`) pre-0.6 at every site. | 0.6 |
| `waves[]` | `{id,anchor,mobs[{entity,count,name?,attributes?,effects?,equipment?}]}`; entity validated (`DW0173`); `attributes`/`effects` are v0.4 (`DW0192`). `equipment{head?,chest?,legs?,feet?,main_hand?,off_hand?}` is v0.6 (task #65; reserved `DW0141` pre-0.6): slot item ids validate against the pinned 1.21.11 item registry (`DW0143`, the give-item family); emitted as component-era `equipment`/`drop_chances` summon NBT (never legacy `ArmorItems`/`HandItems` — 1.21.11 ignores them) with **drop chance 0 on every slot** (no-grind: wave gear is never lootable). Explicit slots merge over the armed-mob main-hand default (a helmeted skeleton keeps its bow; explicit `main_hand` overrides). A helmet is the sanctioned daylight-undead fix — never `set-time`. | 0.3 / tuning 0.4 / equipment 0.6 |
| `waves[].respawns_on_rest` | `true` re-seats the wave on every bonfire rest **and** on every respawn at a bonfire (spec-0016 §1) — the souls contract: progress is kept, the enemies come back. Emission: `spawn_<wave>` additionally sets a seated sentinel `#wseat_<wave> dw.sys`, and `wave_reseat_<wave>` kills every survivor carrying `dw_wave_<id>` then re-runs the wave's own spawn (authored composition, DW0312-proven cells). A rest only re-seats waves the party has actually met — an unmet wave is never conjured. Declaring the field with **no** `bonfire` in the campaign is inert, so it is `DW0356`, not a silent no-op. Reserved `DW0141` pre-0.6. | 0.6 |
| `timed_gates[]` | `{id,gate,open_ticks,closed_ticks,phase?}` (spec-0016 §4, reserved `DW0141` pre-0.6) — a gate region on a deterministic open/close clock, so passage is a timing read rather than a permanent state. Emission is a **self-sustaining two-function ping-pong** (`tgate_open_<id>` / `tgate_close_<id>`), each half doing its `fill` and scheduling the other; `schedule` is replace-mode so the clock can never double up, and a timed gate costs **nothing per tick**. The gate is sealed by the prefab at world-load, so the clock's first act is always an OPEN (`phase` holds it shut that many ticks first). Structural errors are `DW0377` (id, a half-cycle of 0, a `phase` at or beyond the cycle, two clocks on one region, or a gate a `shortcut` already owns — a clock would re-seal what `DW0372` forbids re-sealing); a gate anchor with no declared fill `block` is `DW0343`. The design proof is `DW0378`: **not** all-phase passability (a gate that punishes bad timing is the point, owner ruling 2026-08-02) but ≥ 20% of the cycle admitting a crossing. | 0.6 |
| `ambushes[]` | `{id,at,actors[],trigger,telegraph[]?}` (spec-0016 §3, reserved `DW0141` pre-0.6) — **sugar**, not a new runtime mechanism. `parse_campaign` desugars each ambush into an ordinary one-shot `EnvTrigger` named `trigger/<local id>` at `at`, whose effects are the `telegraph` bundle, then a `spawn-actor` per listed actor, then an `unleash-actor` per listed actor. Everything downstream — validation, l10n, the flag/wave producer scans, nav, emission — sees only that trigger, so the sugar has no second code path to drift down and an ambush is exactly as debuggable as the trigger an author would otherwise type. The canonical form of a campaign is therefore its **desugared** form (the section is never serialized), which is what keeps the canonical round-trip idempotent. `telegraph` is **optional and stays optional** (owner ruling 2026-08-02): the un-telegraphed ambush is core souls vocabulary and nothing in the compiler asks for a tell. Declaration errors are `DW0375`; the counterplay obligation is `DW0376`. | 0.6 |
| `shortcuts[]` | `{id,gate,unlock,on_unlock[]?}` (spec-0016 §2, reserved `DW0141` pre-0.6) — the souls loop-back. The `gate` is **sealed from world-load** (the prefab carries the physical fill), and the `unlock` anchor on the FAR side opens it **permanently**. Declaration errors are `DW0371` (malformed/duplicate id, an anchor no prefab provides, or an `unlock` equal to its own `gate`); a gate anchor with no declared fill `block` is `DW0343` (the same rule `close-gate` obeys); a `close-gate` anywhere targeting a shortcut gate is `DW0372` — permanence is structural, there is no re-seal verb to reach for. Geometry proofs: `DW0373` (the long route exists while the gate is sealed) and `DW0374` (opening it strictly shortens the walk to the unlock — the anti-leak proof that makes `unlock` a far-side anchor rather than a label). Every shortcut gate is additionally **sealed for the whole completability model** (`Plan::build` registers it as a `close-gate` at step 0), so `DW0311`/`DW0315`/`DW0342` all prove the delve finishable with no shortcut ever taken. | 0.6 |
| `waves[].respawns_on_rest` | `true` re-seats the wave on every bonfire rest **and** on every respawn at a bonfire (spec-0016 §1) — the souls contract: progress is kept, the enemies come back. Emission: `spawn_<wave>` additionally sets a seated sentinel `#wseat_<wave> dw.sys`, and `wave_reseat_<wave>` kills every survivor carrying `dw_wave_<id>` then re-runs the wave's own spawn (authored composition, DW0312-proven cells). A rest only re-seats waves the party has actually met — an unmet wave is never conjured. Declaring the field with **no** `bonfire` in the campaign is inert, so it is `DW0370`, not a silent no-op. Reserved `DW0141` pre-0.6. | 0.6 |
| `waves[].lane` | `{waypoints[],aggro_radius}` (spec-0016 §6, reserved `DW0141` pre-0.6) — **routed while distant, feral once aggroed**, on vanilla's Raider patrol system (the intended primitive; live-verified 1.21.11, `docs/notes/td-routing-spike.md`). The squad spawns `Patrolling:1b` with one `PatrolLeader:1b` and the **snake_case int-array** `patrol_target:[I;x,y,z]`; a per-wave clock (`lane_tick_<wave>`, 30t, self-terminating) advances a shared waypoint index and per mob releases `Patrolling:0b` whenever a player is inside `aggro_radius`. `aggro_radius` is emitted verbatim as each lane mob's `follow_range` attribute — release radius and perception radius MUST be one number, so a contradicting per-mob override is `DW0381`. Lanes are raider-family only (`DW0382`: pillager / vindicator / evoker / ravager / witch), squad ≥ 2 (`DW0383`: a lone patroller self-cancels), and a lane pillager must keep its crossbow (`DW0384`: its only attack goal is crossbow-gated, so an otherwise-armed one deadlocks on target acquisition). Declaration errors (no waypoints, an invented waypoint anchor, a repeated consecutive waypoint, `aggro_radius` outside `4..=64`, `lane` + `summon: aggro-edge` together) are `DW0381`; lane geometry is the build-tier `DW0386`. Lane waypoints join the wave's spawn anchor in the layout solver's **required-anchor** set for the wave's area, so a prefab-pool area is guaranteed to draw a piece providing each one — without that a pool draw can legally omit a waypoint's carrier and the lane fails `DW0386` for a reason the author cannot act on. |
| `waves[].summon` | `anchor` (default, the pre-0.6 behaviour) or `aggro-edge` (spec-0016 §6, reserved `DW0141` pre-0.6). **Aggro-edge = spirit-summoned at the edge of perception**: species without patrol AI never march a lane, so each mob instead materializes on the ring at its own `attributes.follow_range` from the wave `anchor` — which in this mode is the **defended point**, not the spawn point. Candidate cells are standable, walk-reachable and in line of sight of that point, on the one-sided band `[follow_range - 2, follow_range - 1]`, ordered outermost-first: one full block INSIDE the mob's own perception, because ladder evidence (the drowned bell, runs 10/12) showed a mob seated exactly AT the radius acquires a defender at the anchor only marginally — vanilla target acquisition at the boundary is a coin flip, and a summoned mob that acquires nobody stands idle forever, timing out its kill objective. Never beyond perception, never on top of the party. `follow_range` is mandatory here (`DW0385`) — the ring radius is authored, never guessed from a vanilla defaults table the compiler cannot verify. A ring with too few valid cells is `DW0387`, not a silent short spawn. |
| `shortcuts[]` | `{id,gate,unlock,on_unlock[]?}` (spec-0016 §2, reserved `DW0141` pre-0.6) — the souls loop-back. The `gate` is **sealed from world-load** (the prefab carries the physical fill), and the `unlock` anchor on the FAR side opens it **permanently**. Declaration errors are `DW0371` (malformed/duplicate id, an anchor no prefab provides, or an `unlock` equal to its own `gate`); a gate anchor with no declared fill `block` is `DW0343` (the same rule `close-gate` obeys); a `close-gate` anywhere targeting a shortcut gate is `DW0372` — permanence is structural, there is no re-seal verb to reach for. Geometry proofs: `DW0373` (the long route exists while the gate is sealed) and `DW0374` (opening it strictly shortens the walk to the unlock — the anti-leak proof that makes `unlock` a far-side anchor rather than a label). Every shortcut gate is additionally **sealed for the whole completability model** (`Plan::build` registers it as a `close-gate` at step 0), so `DW0311`/`DW0315`/`DW0342` all prove the delve finishable with no shortcut ever taken. | 0.6 |
| `triggers[]` | `{id,at?,on:strike\|use\|approach{range}\|strike-npc{npc},requires_flags?,forbids_flags?,once?,effects[]}` (v0.4; `forbids_flags` and `strike-npc` v0.6, reserved `DW0141` earlier). `at` names a **place** and is required for `strike`/`use`/`approach`; `strike-npc` names a **character** and takes no `at` at all — either mismatch is `DW0194`, because an ignored anchor reads as meaningful and does nothing. A `strike-npc` target that stage 2 does not declare is `DW0112` (the trigger's tag would ride nothing). Bad/dup/`range 0` → `DW0194`. A trigger is armed while every `requires_flags` flag is held by some player AND no `forbids_flags` flag is set by anyone — e.g. a retaliation trigger armed by `flag/sealed` that stands down the moment `flag/asleep` is set (the wake beat takes over), with no re-arm plumbing. | 0.4 / forbids 0.6 |
| Effect `open-gate` | Fills gate anchor to air. | 0.1 |
| Effect `close-gate{anchor}` | The physical dual of `open-gate` (v0.6): fills the gate anchor's region with the block the anchor's prefab metadata declares (basalt boulder, iron bars), re-sealing an opened threshold into a wall. A gate anchor that declares no `block` is `DW0343`. Same anchor-existence check as `open-gate` (`DW0142`). Per-effect `requires_flags` like the other per-`@s` verbs. | 0.6 |
| Effect `campaign-complete` | Sets `dw.campaign`; finale fanfare. | 0.1 |
| Effect `spawn-wave` | Summons wave mobs (AI on), tag `dw_wave_<id>`. | 0.3 |
| Effect `give-item{item,count,name?}` | Grants item (`name` v0.4). | 0.3 |
| Effect `set-flag{flag}` | Sets `dw.f_<flag>` (per-player). | 0.3 |
| Effect `narrate{text,style?,sound?}` | chat/title/subtitle/**art**; `text` → l10n; `sound` validated (`DW0326`); `art` = the `delve:art` pixel-banner font, glyph-checked (`DW0328`), width-checked (`DW0330`). | 0.4 / art 0.6 |
| Effect `set-block{anchor,block}` | `setblock` at anchor; base block id validated (`DW0193`). `block` accepts a verbatim blockstate suffix `id[key=value,…]` (v0.6). | 0.4 / state 0.6 |
| Effect `requires_flags[]` / `forbids_flags[]` (any effect) | Per-effect gates (v0.6): `requires_flags` wraps the effect's command(s) in a per-player `execute if score @s dw.f_<flag> matches 1 … run …`; `forbids_flags` adds `unless score @s dw.f_<flag> matches 1` clauses to the same guard (suppressed once any listed flag is set for the acting player). Valid on any `on_objective_complete` / `on_complete` / trigger effect **except** terminal `campaign-complete`; refs resolve like objective flags (`DW0172`). | 0.6 |
| Effect `despawn-npc{npc}` | Removes NPC + hitbox. | 0.4 |
| Effect `spawn-npc{npc}` | The dual of `despawn-npc` (v0.6): summons a `deferred` stage-2 NPC — body + interaction hitbox + name display — at its declared anchor, via the **same** `npc_summon_commands` authority world init uses. Idempotent (per-entity tag guards), so a re-fire never doubles a body. Also a dialogue effect. World-global staging → no per-effect `requires_flags`. | 0.6 |
| Effect `move-npc{npc,to_anchor,speed?,on_arrive[]?}` | A*-planned per-tick tp through walkable space; unroutable → `DW0307`. `on_arrive[]` (v0.6, reserved `DW0141` earlier) fires once on the driver's final-waypoint tick — **exact parity with `move-actor.on_arrive`**: same arrival detection, same execution context (`mv_arrive_<key>` mirrors `ma_arrive_<key>`), and every deep effect walker (flag/wave producer scans, consumer-ref checks, checkpoint/stealth collector, l10n inventory + localization, nav flattening, emission) recurses into it via the shared `nested_effect_lists` authority. Lets content gate a beat on walk *completion* (`on_arrive` → `set-flag`) instead of fire-and-forgetting the walk. | 0.4 / `on_arrive` 0.6 |
| Effect `cutscene{shots[]}` / `cutscene{path[],seconds,look_at?}` | Two-camera spectator dolly; clip → `DW0308` (checked **per shot**, over both the authored polyline and the client-rendered keyframe chords); a shot panning over the 6°/tick angular budget → `DW0347`. Two mutually exclusive spellings, normalized to one shot list: multi-shot `shots: [{path[],seconds,look_at?}, …]` (v0.6) or the single-shot `path`+`seconds` fields (v0.4) — mixing/omitting both, or a shot with an empty `path`, is `DW0199`. Shots play back-to-back inside ONE save/restore bracket (hard cut). `look_at {anchor,offset?}` aims every dolly camera at that world point; absent = face along the direction of travel. **`shot_style` (v0.6, spec-0015)**: a shot may instead declare a style preset + `subject {anchor|npc|actor, offset?}` (+ optional `dist`, `bearing`, `degrees` (orbit only), `subject_b` (two-shot only)); the compiler expands the style deterministically into the dolly + aim + duration (see "Shot styles" below). Explicit `path`/`look_at`/`seconds` always override the corresponding expanded part. Style-shape violations are `DW0348`; a `side-track`/`low-follow` whose subject has no sibling `move-npc`/`move-actor` (same effect group or sequence) is `DW0349`; an unknown subject npc/actor is `DW0112`. | 0.4 / `look_at`+`shots`+`shot_style` 0.6 |
| Effect `set-time{time}` | Instantaneous dimension-global cut (`time set <kw>`); persists (cycle frozen). | 0.5 |
| Effect `set-weather{weather}` | Instantaneous dimension-global cut (`weather <kw>`); persists (cycle frozen). | 0.5 |
| Effect `play-sound{sound,at?,volume?,pitch?}` | Plays a sound event; `sound` validated (`DW0326`); `at` = `{anchor}`\|`players` (default)\|`{actor}` (deferred → `DW0335`); positional or per-player. | 0.6 |
| Effect `damage-players{amount,in?,damage_type?}` | Deals `amount` half-hearts of damage over vanilla `/damage` — a real `on_caught`/souls consequence. **Audience (spec-0018)**: on a quest beat / trigger the hazard is a fact about the delve, so it hits the whole party (`execute as @a[…] run damage @s …` — `/damage` takes ONE entity, see §1); inside a solo `on_caught`/`on_respawn` bundle it hits exactly that player (`execute if entity @s[…] run damage @s …`). `amount ≥ 40` is lethal through golden apples. `in {anchor,extent}` narrows to acting players inside the anchor-centred box (same box model as a stealth zone; anchor `DW0142`). `damage_type` is a **curated enum** of vanilla types that respect `keepInventory` and do NOT bypass totems (no `out_of_world`/`generic_kill`), default `generic`; an unknown value is `DW0100` (needs no registry). Named `damage_type`, not `type`, since the effect enum is internally tagged on `type`. Per-effect `requires_flags` allowed (per-`@s` verb). Every form is guarded by `tag=!dw_cutscene` — a player watching a cutscene is never harmed (§4). | 0.6 |
| Effect `set-checkpoint{anchor,on_respawn?}` | Party-wide respawn point: `spawnpoint @a` at the anchor + `storage dw:cp pos` mirror. Monotonic by quest order. `on_respawn[]` = per-player effects re-run on respawn while active (vanilla `deathCount` detection). Proofs `DW0315`/`DW0316`. Also a dialogue effect. | 0.6 |
| Effect `bonfire{anchor,on_rest?}` | The souls sibling of `set-checkpoint` (spec-0016 §1). The effect only **arms** a rest affordance at the anchor (a `minecraft:interaction` the party right-clicks; the campfire is prefab dressing) — the respawn point moves when the party actually **rests**. Every rest, and every respawn at this bonfire, runs the same `on_rest[]` scene reset and re-seats every wave declared `respawns_on_rest`. Arming is idempotent (guarded summon), resting is deliberately repeatable (unlike the one-shot trap disarm). Proofs are inherited: a bonfire is collected as a checkpoint, so `DW0316` (standable) and `DW0315` (no stranding — rooted at the ARMING beat, the earliest rest) apply unchanged. Quest/trigger effect only (not a dialogue effect). | 0.6 |
| Effect `begin-stealth{zones[{anchor,extent}],on_caught?,grace_ticks?}` | Per-tick: every player must be inside some zone — zone presence alone = hidden (owner ruling 2026-08-01; no sneak requirement, which collided with the spectator cutscene camera); exposed for `grace_ticks` (default 20) → `on_caught`. Zone standable/reachable proof `DW0327`; onset-survivability proof `DW0355` (a beat whose `on_caught` punishes must be escapable inside `grace_ticks` from where the player provably stands when it arms, and from every checkpoint that can respawn them into it). | 0.6 |
| Effect `end-stealth` | Ends the active stealth beat (clears the session marker). | 0.6 |
| Stage-5 `actors[] {id,entity,name?,skin?,anchor,facing?,vulnerable?}` | Scripted NoAI/Silent/no-loot puppets, tag `dw_actor_<id>` (+ puppet marker `dw_pup_<id>`); `Invulnerable` unless `vulnerable` (then knockback-immune); `skin` → mannequin. Summoned by `spawn-actor`, not at load. | 0.6 |
| Effect `spawn-actor{actor}` | Idempotent puppet summon at the actor's anchor. | 0.6 |
| Effect `despawn-actor{actor,style}` | `kill` = vanilla death animation in place; `vanish` = relocate-then-kill (silent, out of view). Targets `dw_actor_<id>` (puppet or twin). **Per-actor drop (round-8, live-observed):** `vanish` emits `execute as @e[tag=dw_actor_<id>] at @s run tp @s ~ -128 ~`, not `tp @e[…] ~ -128 ~` — the bare form resolves `~ ~` against the **command source**, and every path that reaches a `despawn-actor` (a `move-actor`'s `on_arrive`, a `sequence` step, a trigger bundle) runs from the server source at world spawn, so the island's herdsman standing at `6.5,-55.5` died at `10.0,-128.0,9.0`. Masked by the `kill` on the next line, but wrong data. | 0.6 |
| Effect `move-actor{actor,to_anchor,speed?,on_arrive[]}` | Footprint-aware A*-planned per-tick tp of the puppet, yaw along the path tangent; `on_arrive` fires at the destination cell; unroutable → `DW0325`. `move-npc` is a thin wrapper over the same planner (player footprint). **Chained origins (round-6, live-server proven):** an actor's (and NPC's) successive moves chain — the first leg plans from the declared anchor, every later leg from the previous leg's target. Planning every leg from the declared anchor degenerated a second consecutive move (island: mouth→fire-pit at t=260, whose declared anchor IS fire-pit) into a single-waypoint instant teleport — the giant snapped instead of walking on camera. Two moves sharing `(id, to_anchor)` still share one content-keyed driver, planned from the first occurrence's origin (documented limitation of the content key). **Handoff PackTest (round-6):** for the first `move-actor` whose `on_arrive` fires a `spawn-npc` (the walker→NPC scene handoff), a generated `v06_arrive_handoff` template seals every campaign gate (`close-gate` fill), drives the arrival tick, and asserts puppet gone / NPC body present / exactly one NPC hitbox — the beat a delve soft-locks on if the handoff half-fires; gates are re-opened and entities cleared afterwards (batch model). **Concurrent moves are independent (round-8):** each `(actor, to_anchor)` gets its own start function, per-tick driver, run latch `#arun_<bare>` and step counter `#at_<bare>`, and each driver teleports only its own `dw_pup_<id>` — so N moves in flight at once cannot starve one another whatever order they start in (the island cinematic runs four sheep plus the giant). Pinned by `concurrent_move_actors_share_no_state`. The owner's round-8 report of a scheduled move that appeared not to run was chased on a live server with and without a player joined, on clean and stale scoreboards: the driver ran correctly every time (`#at` 0→288 monotonic, latch set then cleared, puppet at the destination cell), so no engine change was made for it; the beat is invisible from the player's seat for a content reason (the walk is off-camera for its whole duration and the arrival lands after the `close-gate` seal). | 0.6 |
| Effect `unleash-actor{actor}` | Replaces the puppet with a real-AI twin (same entity/pos/name/tag, no puppet marker). Re-caging = `despawn-actor` + `spawn-actor`. **Spawn finalization (round-8, live-proven):** `/summon <entity> <pos> <nbt>` — *any* NBT compound, even `{}` — makes vanilla skip `finalizeSpawn`; `/summon <entity> <pos>` does not. The compiler always passes NBT (tags are how it addresses everything it owns), so every mob it summons is un-finalized. For `minecraft:warden` that is fatal: `finalizeSpawn` is the only place the `minecraft:dig_cooldown` brain memory is seeded, and a warden without it enters the DIG activity on its first AI tick, burrows, and despawns ~5 s later (the owner's round-8 report: strike the sleeping giant, watch the warden dig itself back into the ground). A/B on the pinned server: bare summon → `Brain{memories:{"minecraft:dig_cooldown":{value:{},ttl:1200L}}}`; summon with `{}` → `Brain{memories:{}}`, gone. The twin summon now carries that memory verbatim (vanilla's own 1200-tick value — the awake warden refreshes it itself, verified present and roaming past 80 s). Only the **twin** needs it: a caged puppet is `NoAI` and never runs `customServerAiStep`, which is why a puppet warden can stand in a meadow indefinitely. Species needing no finalization data are unchanged (byte-identical). **Aggro lock (owner directive, round 8):** an unleashed hostile targets the player who *struck* the trigger. The click trigger parks that player's UUID in `storage dw:strike player` (`data modify … set from entity <hitbox> attack.player` — vanilla's own record of who clicked) for the length of its own bundle and removes it after, so it can never go stale; `unleash_<id>` seeds the warden's vanilla `anger.suspects` from it at max anger (150), guarded on the storage holding a value. Live end-to-end: the warden left its spawn cell, closed on the seeded player and killed that player. Emitted only for a campaign whose click triggers actually unleash, so other campaigns' unleash functions are unchanged. **Limit:** the warden is the only species with a data-settable target that survives a tick on 1.21.11 — the `NeutralMob` pair (`AngerTime`/`AngryAt`) was tried against endermen, piglins, wolves and iron golems, with a real online player's UUID, and neither field reads back afterwards, so nothing is emitted for them and they acquire targets by vanilla's own nearest-player search. | 0.6 |
| Effect `sequence{steps[]{at_ticks,effects[]}}` | Deterministic timeline: one schedule chain firing effect groups at exact tick offsets. No nested `sequence` → `DW0329`. Effects nested in a step are **first-class**: the flag/wave producer scans, the checkpoint/stealth collector, the l10n inventory, and emission all descend into `sequence.steps` and every nested effect list (`on_respawn`/`on_caught`/`on_arrive`) via one shared traversal, so a `set-flag`/`set-checkpoint` nested in a step produces its flag / registers its indexed checkpoint exactly as at top level. A sequence is a **global timeline**: every step function (the inline `at_ticks: 0` one included) is emitted server-source-safe, so its per-player beats address the party rather than one acting player — §4 "A scheduled bundle has no `@s`". | 0.6 |
| `traps[]` | spec-0011: `{id,at,trigger,effect,lethality?,disarm?,reset?,requires_flags?,forbids_flags?}`. `at` binds an `anchor/trap` prefab marker (the trigger/hazard cell; its `dispenser` metadata cell holds the payload socket). `trigger` ∈ `pressure-plate`/`tripwire`/`trapped-chest` (all redstone-native; `trapped-chest` = the only player-distinct trigger). `effect` = `{dispense:{item,count}}` (item `DW0341`; a non-`dispense` key e.g. `tnt` is an unknown variant → `DW0100`). `lethality` ∈ `lethal`/`harmful`(default)/`nonlethal`. `disarm{via,sets_flag}` = a reachable affordance that turns the trap off. `reset` ∈ `once`/`rearm`(default). Structural errors `DW0340`; a lethal forced-path trap without discharge `DW0342`. `requires_flags`/`forbids_flags` are a **physical** gate (see §4 emission): the trigger block is removed from the world while the gate is shut and restored verbatim when it opens, so a gated trap is genuinely inert rather than nominally so — the trigger must be a plate/tripwire declaring `trigger_block` in its prefab metadata, else `DW0363`. Reserved (`DW0141`) before 0.6. | 0.6 |

Dialogue effects `set-flag` (v0.4), `set-time`/`set-weather` (v0.5),
`set-checkpoint`/`spawn-npc` (v0.6) and option `requires_flags` mirror the quest
forms.
Per-effect `requires_flags` is a v0.6 **quests-stage** surface only (dialogue
effects are not mirrored — a dialogue option's own `requires_flags` already gates
its whole effect bundle). Under `0.2.0`, all v0.3 verbs/effects are reserved →
`DW0141`; likewise v0.4 surface under pre-0.4, v0.5 surface
(`time`/`weather`/`lighting`, `set-time`/`set-weather`) under pre-0.5, and the
v0.6 surface (area `mitigation`, `close-gate`, `damage-players`, `set-checkpoint`,
`begin-stealth`/`end-stealth`, the `play-sound` effect + `narrate` `style: art`,
per-effect `requires_flags`, `forbids_flags` (every site), `move-npc.on_arrive`,
stage-2 `deferred` + the `spawn-npc` effect, stage-5
`actors` + the staging effects `spawn`/`despawn`/`move`/`unleash-actor`,
`sequence`, and the `traps[]` section) under pre-0.6.
The blockstate suffix on `set-block`/`prop` blocks is a lenient parse of an
existing field, not version-gated: the base id is registry-checked and the `[…]`
string is passed to `setblock` verbatim (vanilla validates the property
names/values); a malformed suffix (unbalanced `[]`, empty, non-`key=value`)
reuses `DW0193`.

### Stage 6 — `dialogue`

Exactly one tree per stage-2 NPC (`DW0152`/`DW0153`). Nodes reachable from `root`
(`DW0120`/`DW0121`); `complete-objective` effects target a `talk-to` on the same
NPC (`DW0122`); every `talk-to` has ≥1 reachable (`DW0123`) and ≥1 **ungated**
(`DW0191`) completing option — where "gated" means `requires_flags` OR (v0.6)
`forbids_flags`: either kind of flag gate can make the option unavailable exactly
when it is needed, and the static analysis does no temporal reasoning about which
flags end up set. Node `text` → l10n `dlg.<n>.<node>.text`, option
labels → `.opt.<i>.label`. **Display gating (v0.4+, task #54):** an option is
*shown* only when clicking it would fire — every `requires_flags` set and no
`forbids_flags` set (flag axes; the click handler mirrors both with fail-fast
guards, so a direct `/trigger` cannot bypass them) and every completed objective
active, i.e. `dw.qa_<quest>==1` and
`dw.o_<obj>!=1` (objective-state axis) — so `DW0191`'s ungated completing option
is visible exactly while its objective is active (the guarantee holds
automatically).

### Stage 7 — `world-edits` (optional; v0.6, spec-0017)

The map editor's edit script (`world-edits.json`), the artifact of record for
L3 world detailing. **Optional**: absent = no edit stage, and the build is
byte-identical to pre-stage-7. Replayed deterministically by the compiler
after world assembly (§1 pass 8); editing sessions leave no state outside the
script. `note` fields are authoring context — machine-ignored and **excluded**
from l10n (no stage-7 string is player-visible).

| Element | Behavior |
|---------|----------|
| `batches[]` | Ordered `{id: batch/<kebab>, area, note?, edits[]}`. Batch ids are unique (`DW0111`), the seed-stream label and the snapshot name; `area` must be a stage-1 area (`DW0112`). After EVERY batch the invariants re-prove (§4). |
| `select` | `{name: region/<kebab>, shape}` — defines a named region for later verbs **in the same batch** (strictly backward; dangling/forward = `DW0162`, duplicate = `DW0111`). Shapes: `box` (inclusive `min`/`max` in a declared frame), `surface-band` (`over` + `from..=to` offsets from each column's surface), `palette-match` (`within` + base-id `blocks`), `union`/`intersect` (`of`, ≥2), `subtract` (`base` − `remove`). |
| Frames | `piece-local` (`piece` placement index + `prefab` drift-guard — mismatch is `DW0323`) or `anchor-relative` (a resolved anchor of the batch's area) — never raw world coordinates, so a script survives placement moves. |
| `fill` / `replace` | Seeded palette-recipe write over a region (`replace` only rewrites cells whose base id is in `matching`). A recipe is weighted `blocks[]` (+ optional noise `scale`, default 0.35 blocks⁻¹) sampled by smooth value noise — picks cluster into strata/patches, never a uniform fill. Block ids validate against the pinned registry with optional verbatim blockstate suffix (`DW0193`); weights/scale finite > 0 (`DW0162`). |
| `carve` | Clear a region to air. Sealing-aware by construction: the carved region re-enters relight + walkability + boundary proofs. |
| `morph` | Surface reshape per region column: `raise{by,recipe}`, `lower{by}`, `smooth{passes,recipe}` (±1/pass relaxation toward the cardinal-neighbour mean). The region gives the footprint + where the surface is read; `raise`/`smooth` may add cells above the region top. |
| `scatter` (PR 2) | Seeded dressing over a region's **standable** cells (air over an occupied cell): weighted `items[]` (blockstate suffixes allowed), per-candidate white-noise `density` gate in `(0, 1]` (dressing wants speckle, not the fill verbs' clustered patches), keep-clear `avoid[]` region envelopes (matched by `(x, z)` column), optional both-axes `spacing` rule and `limit` cap taken in descending noise order — the greenfield generator's spread idiom, ported. |
| `plant` (PR 2) | Structural flora via the #121 **lean-or-grow** canopy rules (ported from the island terrain generator): up to `count` trees on the region's highest-noise standable cells (both-axes `spacing`, default 4; trunks never on `avoid[]` columns). A canopy that would cover an `avoid` column leans one block directly away; if that still covers it, the tree grows tall instead — its whole ball arched 3 above the trunk's floor. **No leaf is ever sliced**; leaves write only into air, so near walls/ceilings the ball may extend past them — review via the batch snapshot. `tree: oak` (per-species rule sets, extensible). |
| `fragment` (PR 2) | Stamp a **library prefab**'s non-air cells at a frame-resolved `at` (+ optional quarter-turn `rotation`) — semantically a `/place template` whose bytes the compiler models (non-air overwrites; authored air never erases). Only admitted library prefabs can be stamped, so provenance/license ride the prefab's own metadata (ADR-0013); an id outside the library is `DW0323`. Stamped cells keep their **full blockstate** (`assembled::structure_cells_stateful`; properties in sorted key order): the stamp's writes ARE the runtime `setblock` lines, and reading bare ids turned an authored `lantern[hanging=true]` into a floor lantern in mid-air (found by `DW0354`). **`rotation` turns POSITIONS only, and the compiler REFUSES rather than warns.** There is no rotate-aware blockstate rewriter, so a quarter-turned stamp would keep every `facing`/`axis`/`shape`/connection value unrotated and ship visibly deformed geometry — the silently-deformed-map class. A `rotation` other than `none` on a prefab carrying any **yaw-dependent** property (`facing` except `up`/`down`, `axis` except `y`, `shape`, `rotation`, `orientation`, `hinge`, `north`/`south`/`east`/`west`) is a build error (`DW0323`) naming the block, its prefab-local cell and the offending property; the prescription is to stamp unrotated or admit a pre-rotated prefab variant, never to hand-fix facings downstream. It is a **collision test, not a blanket ban**: prefabs whose every state is yaw-invariant (`hanging`, `half`, `waterlogged`, `open`, `lit`, `type`, `level`, `thickness`, `vertical_direction`, `axis=y`, `facing=up|down`) rotate correctly and stay allowed — `hello-room`, whose only property is `lantern[hanging=true]`, stamps fine at every quarter-turn. |
| `relight` (PR 2) | Run the spec-0010 fixture-placement pass over ONE region and **bake** the fixtures into the edit script's writes — authorial control of where fixtures land (the whole-area relight still re-proves after every batch). Fixture/target default to the area's declared `lighting`; `fixture` + `min_light` (1..=14) override, and are **required** when the area declares none (`DW0162`). An unlightable region is the area pass's own `DW0211`, batch-attributed; a region with no reachable walkable cell is `DW0323`. |
| L2 massing verbs (PR 3) | `swap-piece` (replace a piece with a library prefab that re-mates every mated socket at its exact world pose, any rotation, overlap-checked), `insert-piece` (attach at a specific **unmated** socket — the targeted form of the solver's frontier attach), `remove-piece` (a **leaf** only — exactly one mated socket, never the entry; the neighbour's socket unmates and re-seals), `rewire-socket` (`sealed` **unmates the doorway pair** — a graph operation: both planes wall up and the DW0306 connectivity proof loses the edge; `open` clears an unmated socket's fill — deliberately without granting the proof an edge, conservative), `reseed-piece` (seeded weighted re-pick among the area pool's compatible members, current excluded — a reseed always changes the piece or errors). All carry the `piece` index + `prefab` drift guard. Applied at **plan** time (`compiler::massing`, inside `Plan::build` right after `solve_area`): seals are regenerated from the massaged mated flags (`seal_layout`), and anchors, gate reachability, waterline, assembly, relight, nav and the L3 replay all run over the massaged layout — the full assembly validation re-runs by construction. Massing verbs live in **massing-only** batches ordered before every detailing batch (`DW0162`); an inapplicable verb is `DW0324`. `resize-piece` from the spec's initial list is **excluded**: the library has no size-parameterized piece primitive to express it through (no-hack doctrine); `swap-piece` covers the different-sized-variant case. |
| Seeding | Every seeded verb streams from `stream_seed(campaign_seed, "edits/<batch-id>/<edit-index>")` — renaming a batch (or moving an edit) deliberately reseeds it; nothing else does (ADR-0006). |
| Emission | The replay lowers to a `world_edits` function (x-run-coalesced `fill`/`setblock`), called from `setup_finish` after the socket seals and before the relight fixtures — the exact model order, and the reason `DW0352` exists (`trap_setup` runs later). `setup` additionally forceloads every batch's write AABB (an edit may write outside the piece bboxes — a leaning canopy, a stamped fragment — and a `setblock` on an unloaded chunk silently fails); those chunks then follow the **forceload lifecycle** below. `world-edits.json` is hashed into `manifest.json` inputs. |

### l10n sidecars (`l10n/<code>.json`)

Envelope `{dsl_version,campaign_id,kind:"l10n",lang,content}`; `content` = flat
**stable key → translated string**. Key inventory derived from stage docs
(`world.title`, `world.outro`, `area.<a>.name`, `class.<c>.name/.blurb/.kit.<i>.name`,
`npc.<n>.name`, `quest.<q>.goal`, `obj.<q>.<o>.title/.hint`,
`dlg.<n>.<node>.text/.opt.<i>.label`, `wave.<w>.mob.<i>.name`) plus effect strings
`fx.<q>.oc.<o>.<i>.narrate|.give`, `fx.<q>.done.<i>.…`, `fx.trig.<t>.<i>.…`.
**Nested effects** (DSL v0.6): a `narrate`/`give-item` inside a `sequence` step or
an `on_respawn`/`on_caught`/`on_arrive` bundle is inventoried and localized too,
under a position-derived child key = parent `fx.…` key + a stable segment
(`seq.<step>` for a sequence step; `respawn`/`caught`/`arrive` for the bundles) +
the effect's list index + leaf, e.g. `fx.<q>.oc.<o>.0.seq.1.0.narrate` (nesting is
arbitrary-depth). Keys are purely position-derived → deterministic + byte-stable.
Coverage is **exact**: missing/absent/inconsistent → `DW0180`; orphan → `DW0181`.
Excludes authoring context (theme/premise/persona).

**`delvec l10n-inventory <dir> [--lang <code>]`** emits that inventory as one JSON
document on stdout — the work list a translator (in-agent, human, or an external
API via `tools/i18n-translate.py`) is handed up front, instead of discovering it by
writing an empty sidecar and reading the coverage diagnostics back:

```
{ campaign_id, dsl_version, lang, declared, sidecar_present, world_title,
  npcs:    [{id, name, archetype, speech_style, demeanor?, motivation}],
  entries: [{key, en, speaker?, existing?}] }
```

`entries` is the inventory itself (a CLI test asserts the key set equals what
`DW0180` demands, so the two cannot drift). `speaker` is the NPC whose dialogue
tree the key belongs to (`dlg.<npc>.…`, `npc.<npc>.name`; a `.opt.<i>.label` is the
player's reply *inside* that tree); `existing` is what `l10n/<lang>.json` already
translates, so a re-run fills only the gaps. Persona rows carry voice, never plot
(`secret`/`backstory`/`relationships` are excluded). Runs **before** validation
gating — an incomplete sidecar is the normal state when you ask — and needs no
prefab library; only an unparseable campaign fails (exit 1). See
[i18n.md](i18n.md).

---

## 3. Verb → emission mapping

Mechanism level (not full mcfunction). See `crates/compiler/src/emit.rs`.

| Verb / effect | Emitted mechanism |
|---------------|-------------------|
| `talk-to` / dialog option | `minecraft:villager` body (NoAI/Invulnerable/Silent) + co-located `minecraft:interaction` (tag `dw_npc_<n>`); click advancement + `/trigger dw.dlg_<n>` both feed one per-tick option handler. |
| dialog display gating (v0.4+) | A node with any display-gated option (`requires_flags`, `forbids_flags` (v0.6) and/or `completes`) emits one `__m<mask>` variant per per-node availability bitmask + a chooser: `dmask_<n>_<node>` sets `dw.dmask` (bit `i` = the node's i-th gated option is displayable — its required flags set, no forbidden flag set (`unless score @s dw.f_<flag> matches 1`, unset-safe), and every completed objective active: `if …qa_<q>==1 unless …o_<obj>==1`), then `show_<n>_<node>` `dialog show`s the matching variant. Ungated nodes/options `dialog show` directly (v0.2/v0.3 byte-identical). Click handler keeps its own guard (defense-in-depth for the `/trigger` path). The `v04_dialogue_visibility` PackTest asserts the option-under-test's **isolated** bit (`(dmask>>bit)&1` via `%= 2^(bit+1)` then `/= 2^bit`), never the whole `dw.dmask` — sibling options in a node can share a `qa_<q>` score, so a whole-mask compare would read a sibling's bit as this option's. |
| dialogue trigger re-arm | `dlg_<npc>_<n>` consumes the trigger with `scoreboard players reset @s dw.dlg_<npc>` — which also **re-locks** it — and therefore re-arms it in the very next line (`scoreboard players enable @s dw.dlg_<npc>`), before the flag gate's `return fail` and before any `dialog show`. The per-tick `scoreboard players enable @a` stays as belt-and-braces but cannot close the window on its own: 1.21.9+ **freezes the integrated (singleplayer) server while a screen is open**, and the handler's last act is to show the next node, so ticking stops with the trigger locked and the player's next click is executed the instant ticking resumes — before the tick function — and vanilla rejects it ("You can't trigger this objective yet"), silently swallowing one dialogue choice. A dedicated server never pauses, so no rung of the validation ladder can reproduce it. The generated `dialogue_trigger_rearm` PackTest drives a terminal option and uses the trigger twice **with the tick function never run in between** — that suppression is the freeze. |
| class select | Dialog button → `/trigger dw.class set <n>`. |
| `reach-anchor` | Per-tick `execute if entity @s[box ±1 on each axis]`; glowing `end_rod` `item_display` marker (tag `dw_r_<obj>`), labeled with the objective `title` — an **untitled** objective gets a nameless glowing marker, never a raw-id label. Completion despawns the marker (`kill @e[tag=dw_r_<obj>]`). |
| `kill` / `spawn-wave` | `spawn-wave` summons mobs (AI on) tag `dw_wave_<id>`, countdown `#<id> dw.wave`; `player_killed_entity` advancement decrements; `kill` completes at 0. Armed species get `equipment` NBT (drop 0): `wither_skeleton→stone_sword`, `skeleton`/`stray→bow`, `pillager→crossbow`, `vindicator→iron_axe` (the pillager row is load-bearing, not cosmetic — see `waves[].lane`/`DW0384`). **Arming assertion (generated `verb_kill`)**: the test picks the wave's first mob with an **effective** main hand — the author's `equipment.main_hand` when given, the default table otherwise (`emit::effective_mainhand`, the same source the summon NBT reads) — and asserts that exact item via `execute if items entity … weapon.mainhand <item>`. Deriving it from the default table alone shipped a self-contradicting delve: the-drowned-bell summoned `stone_axe` vindicators while its generated test demanded `iron_axe`, so a correct campaign failed on a real server; the override case also *extends* coverage to authored weapons on species the table calls unarmed. **Mob placement (task #41):** each mob is seated on a distinct compiler-validated standable cell (2-tall clearance, solid floor) chosen by a deterministic BFS outward from the wave anchor over the assembled occupancy world (`compiler::nav`), ordered by ascending BFS distance with a fixed `(y,z,x)` tie-break. The flood-fill is confined to the anchor's own assembled piece, so a flock never crosses a socket seam into a neighbouring room. A wave needing more footing than its room offers is `DW0312` (never `+x`-strung mobs piling into blocks or spilling toward void). **spec-0016 §6 changes where, not how:** a `summon: aggro-edge` wave is seated on per-mob perception RINGS across the whole area instead (`DW0387`), and a `lane` wave additionally carries the patrol NBT and starts its `lane_tick_<wave>` clock at the end of its own `spawn_<wave>` (so a wave that never spawns never ticks, and a bonfire re-seat re-arms the clock through the same replace-mode `schedule`). |
| `collect` | Chest at anchor pre-loaded `count×item`; `inventory_changed` advancement runs guarded completion. |
| `interact` | `minecraft:interaction` (tag `dw_i_<obj>`) + `player_interacted_with_entity` advancement + `/trigger dw.i_<obj>`; `requires_item` = `execute if items`; glowing lantern `item_display` marker (also tag `dw_i_<obj>`, only when no `prop`), labeled with the objective `title` — untitled → nameless glow, never a raw-id label. `prop{block}` = `setblock` affordance. Completion despawns both entities (`kill @e[tag=dw_i_<obj>]`) so a finished objective is not clickable; the `prop` block persists as scenery. |
| environment `triggers[]` (v0.4) | `setup_finish` summons one `minecraft:interaction` per `strike`/`use` trigger at its `at` anchor (tag `dw_trig_<id>`); `approach` needs no entity. `tick`: `strike` fires on `nbt={attack:{}}`, `use` on `nbt={interaction:{}}`; `approach` is a `distance=..<range>` selector. **The click block is two phases, not one (round-8, island QA):** every click trigger's fire clause first, in declaration order, then every clear clause (`data remove entity @s <field>`). Emitting the pair inline per trigger is only sound while at most one trigger reads a given interaction entity, and several `strike-npc` triggers legitimately ride ONE NPC hitbox — the island's giant carried `wake-the-giant` (requires `flag/asleep`) and `his-house` (requires `flag/sealed`, forbids `flag/asleep`) on the same entity. Inline removal made the FIRST-DECLARED trigger consume the click even with its own gate shut, so `his-house` could never fire: a suppressed trigger starved its siblings and declaration order silently decided which of two legal triggers worked. Two phases make it order-independent — every trigger sharing a hitbox is offered the same click and fires exactly when its own gate says so — while consumption is unchanged (the record is gone by the end of the same `tick` pass, so a held click still fires once). Byte impact: a campaign whose click triggers are its last-declared triggers is unchanged; any other ordering moves the clear clauses to the end of the block. `once` guards on `#trig_<id> dw.sys`, which **every** trigger now writes on firing (not only `once` ones): the write is what makes dispatch observable at all — the starvation bug was a trigger that simply never fired, invisible to every automated check — and it is what the generated `v06_shared_hitbox` template reads. One added line per non-`once` trigger function. **Generated `v06_shared_hitbox` (round-8):** emitted for a campaign that has two click triggers on one NPC hitbox whose flags can tell them apart; it proves the hitbox really is shared, then writes the vanilla `attack` compound and runs the real `tick` twice — once with the later trigger's gate open and the earlier one's shut (the starvation case: the later one must fire, the earlier must stay silent, the record must still be consumed), once with the earlier one's gate open (so both are reachable). Players are shielded with Resistance V across each pass because a real `tick` runs real effects and a delve's effects include `damage-players`; flags, actors and NPCs are handed back untouched (batch model). **`strike-npc` — the body IS the target (v0.6, round-7):** `on: {on:"strike-npc", npc}` has **no anchor**. Its tag rides the interaction hitbox the named NPC already owns and `setup_finish` summons nothing for it, so it works wherever that NPC stands and whatever body it wears. This is the form that can express "hit the giant": a place-based `strike` summons its own entity at a *cell*, and a large NPC's body eclipses that cell (`DW0359`), so the click never reaches it — the owner's island round-7 finding, where striking Polyphemus did nothing. Right- and left-click stay separate all the way down because a `minecraft:interaction` records them in **two distinct NBT fields**: the dialogue advancement takes the right-click (`interaction`), the trigger takes the left-click (`attack`), and neither consumes the other's record. That separability is machine-proven, not assumed — the generated `v04_strike_npc` PackTest writes a right-click record on the shared hitbox, ticks, and asserts no `attack` record appeared and the trigger did not fire. **Strike on an NPC's anchor — one cell, one hitbox (round-6):** the pre-0.6 spelling of the same mechanism, kept working — when a `strike` trigger's `at` is also where an NPC stands, the NPC's own interaction hitbox carries `dw_trig_<id>` **and is the trigger's sole entity** — `setup_finish` suppresses the trigger's own summon. The NPC's body is `Invulnerable`, so without the shared tag a swing could land where nothing was watching and the trigger never fire (round-4 island QA); and with a *second*, exactly co-located hitbox (the round-4 form) the client's entity ray-pick is ambiguous — an exact tie resolves to whichever entity iterates first, in practice the world-init summon — so every right-click landed on an entity without `dw_npc_<n>` and the dialogue advancement never fired (round-6 island QA: Polyphemus untalkable after the boulder seal, proven on a live server). Consequences: the trigger's lifecycle follows the NPC's — a `deferred` NPC's strike trigger is armed only after its `spawn-npc` entrance, a `move-npc`'d NPC carries the strike target with it, and `despawn-npc` removes it entirely (which is the trigger's meaning: the thing being struck is the NPC). Scoped to left-clicks: right-click on an NPC already belongs to the dialogue advancement, so a co-located `use` trigger is rejected at validate time (`DW0350`) and again at build time (`DW0359`). Generated PackTests: `v04_strike_npc` writes the vanilla `attack` compound onto the NPC's hitbox and asserts the trigger fires, once, with the record consumed; `v04_strike_talk` pins the single-hitbox invariant — exactly one interaction entity wears the trigger tag, none wears it without the NPC tag, before and after an attack record is consumed (attack-then-talk must stay clickable). |
| `set-flag` / `requires_flags` / `forbids_flags` | `dw.f_<flag>` scoreboard (per-player); required flags AND-ed into objective guards (layered on `after`), forbidden flags (v0.6) joined as `unless score @s dw.f_<flag> matches 1` clauses in the same guard. **Per-effect** gates (v0.6) wrap each of the effect's emitted commands in `execute if score @s dw.f_<flag> matches 1 [… per required] unless score @s dw.f_<flag> matches 1 [… per forbidden] run <cmd>`; these effect functions already run per-player (`complete_<obj>` / `trig_<id>` are entered `as @a`/`@s`), and an ungated effect is emitted verbatim (byte-identical). In a **scheduled** bundle (`on_arrive`, `sequence` steps) there is no acting player: a per-player effect's gate stays `if score @s …` but under the effect's own `as @a`, while a global effect's gate degrades to the any-player predicate `if entity @a[scores={dw.f_<flag>=1..}]` — §4 "A scheduled bundle has no `@s`". `unless … matches 1` is the deliberate unset-safe spelling: flag scores are never pre-initialized to 0, so a `scores={…=..0}` selector would not match an unset score. **Trigger-level** `forbids_flags` is any-player: the fire condition gains `unless entity @a[scores={dw.f_<flag>=1..}]` per flag (a positive selector inside a negation — flags are campaign state, so one player's wake beat stands the trigger down for everyone); a suppressed strike/use still consumes the interaction record. Generated PackTests: `verb_flag_gate` (requires) and `verb_forbid_gate` (forbids: set flag → drive → assert NOT complete; clear → drive → assert complete). |
| `open-gate` | `/fill … air` over the gate region. |
| `close-gate` | `/fill <region> <block>` over the gate region with the anchor's declared fill block (no `replace` clause — the dual of `open-gate`). |
| `give-item` | Grants item to player (`name` → SNBT text component). |
| `narrate` | chat / `title` / `subtitle` (+ optional sound); `art` = `title` with a `{"font":"delve:art"}` text component, rendered uppercase in the pixel-banner font (6 font px/glyph → ~15 glyphs fit; see [The `delve:art` font](#the-delveart-font)). |
| `play-sound` | `playsound <sound> master @s [<pos>] [<vol> [<pitch>]]` — effects run `as @a`, so `@s` is each player: `anchor` uses the resolved anchor pos (all hear it there), `players` uses `~ ~ ~`. |
| `damage-players` | `execute as <audience>[tag=!dw_cutscene] run damage @s <amount> <type>` for a party beat (`@a`), `execute if entity @s[…] run damage @s …` inside a solo `on_caught`/`on_respawn` (default type `minecraft:generic`). With `in`, the stealth-zone box (`x=…,dx=2·ext,…`) joins the same selector, so each player is judged on their own position — no double-hit. `/damage` takes a single entity, so the party form re-binds rather than widening the target (§1, single-entity arity). A generated `v06_damage` PackTest summons a tagged dummy, applies the declared amount+type, and asserts its `Health` strictly dropped. |
| `set-block` | `setblock` at resolved anchor. |
| `despawn-npc` | Kills body + interaction hitbox. The generated `v04_despawn` PackTest targets the campaign's first `despawn-npc` NPC; when that NPC is **deferred** it runs its `spawn_npc_<id>` entrance right after `setup_finish` (a deferred NPC is deliberately absent from world init, so the presence assertion would otherwise read 0). The assertions themselves — 2 entities present, 0 after the kill — are identical in both cases, and the entrance line is emitted only for a deferred target, so a campaign with no deferred NPC keeps byte-identical PackTest output. |
| `spawn-npc` | `function <ns>:spawn_npc_<npc>` — the generated entrance function, emitted once per **deferred** NPC. Its two lines are the world-init summons, each independently guarded: body by `unless entity @e[tag=dw_npc,tag=dw_npc_<n>]`, hitbox by `unless entity @e[tag=dw_npc_<n>,tag=!dw_npc]` (both carry the id tag, so a single shared guard would let the body's own summon suppress the hitbox). The `npc_summons` PackTest fires each deferred NPC's entrance after `setup_finish` and asserts exactly one body. |
| `move-npc` | Per-tick tp along A*-planned walkable waypoints (hitbox in lockstep), at cell **centres** with L-shaped vertical steps — see §4 "Entity placement". `on_arrive` (v0.6): the driver's final-waypoint tick additionally runs `mv_arrive_<key>` (the bundle's effects), mirroring `ma_tick`/`ma_arrive_<key>` exactly; a bare move emits no hook (byte-identical). The arrive bundle runs with the **server** command source (the driver reached it through `schedule`), so its effects are split per-player / global — see §4 "A scheduled bundle has no `@s`". |
| `cutscene` | Per player: save gamemode+pos → spectator → alternate `spectate` between two co-located dolly cameras each tick (skipping any player actively holding sneak — `predicate=!<ns>:sneak_held`, see §4 "The `spectate` bounce is sneak-gated") → restore. **Keyframe dolly (task #64, `compiler::camera`)**: each shot's waypoint polyline is arc-length parameterized (equal distance per time, not equal segments) with baked smoothstep ease-in/ease-out, then emitted as a tick-0 snap + a `tp` every *N* ticks with display-entity `teleport_duration:N` armed via `data merge` — the **client** tweens position and rotation linearly between keyframes (spike-measured: one position-sync packet per keyframe, rotation interpolates, the `spectate` bounce cannot reset an in-flight tween, and a same-tick merge+`tp` applies the OLD duration because position syncs flush before metadata — which is exactly why the snap and its cadence merge may share a tick). Cadence *N* = the widest of {10, 5, 4, 2, 1} whose rendered chords stay within 0.25 blocks (perpendicular) and 2° (aim) of the exact eased path; a single-waypoint or 1-tick shot is a static snap (cadence 0, no merge). Each shot with a successor resets `teleport_duration:0` on its last owned tick so the next snap is a hard cut, not a glide. Every keyframe `tp` carries an explicit `<yaw> <pitch>` — **Minecraft** entity rotation (`yaw = atan2(-dx, dz)`, 0 = +Z south; `pitch = atan2(-dy, hypot(dx,dz))`, + = down), *not* the render-plan/Chunky yaw convention — computed at emission from the camera's own position: at the shot's `look_at` subject if it has one, else along the eased path's direction of travel. Never the summon default (yaw 0 = south). Positions and rotations rounded to 3 decimals, `-0.0` collapsed to `0.0`, so emission is byte-stable. The bracket also arms the `dw_cutscene` state on every player and releases it on restore — see §4 "A cutscene is pure observation". Multi-shot: all shots share one `#t_<bare>` counter — shot *k* owns `[offset_k, offset_k+len_k]` and the next starts at `offset_k+len_k+1` (hard cut); one marker, one `gamemode spectator @a`, one camera pair, one restore. Both single-shot spellings emit identical bytes. `critical-path.json`'s `cutscene_seconds` is the **total** across shots. Function key = `cs_<first anchor>_<seconds>_<waypoints>` (a pathless styled shot keys `cs_<style>_<subject>_…`), plus an 8-hex sha256 digest of the whole normalized shot list whenever the cutscene is not a bare single shot without `look_at`/`shot_style` (the key must be injective — two shots sharing a first waypoint must never collapse onto one function). Styled shots are expanded (`compiler::camera::expand_shot`) before keyframe planning; a moving subject's per-tick track comes from its sibling move's A* plan, aligned by effect-group/sequence timing. Deduplication stays DSL-content-keyed, so two byte-identical styled cutscenes in *different* move contexts plan from the first occurrence (documented limitation; give the shots distinguishing content to split them). |
| `campaign-complete` | `dw.campaign` = 1 (dummy objective, **never on the sidebar** — a raw internal id must not surface to players); broadcast `[dw:complete <campaign_id> campaign]` (dark-gray bot channel, the harness's completion signal — §4 "The completion-marker channel"); title fanfare. |
| objective lifecycle | Activation shows `title`+`hint`+`note_block.pling` once (flag `dw.ann_<obj>`); completion sets `dw.o_<obj>` = 1, immediately broadcasts the anchored marker `[dw:complete <campaign_id> obj/<id>]` (§4 "The completion-marker channel"), then plays `experience_orb.pickup`. The marker precedes the objective's effects deliberately: it timestamps *completion*, not the aftermath. **Marker cleanup (task #45):** completion despawns every entity the objective's activation summoned via the objective-scoped tag — `interact` hitbox + wayfinding marker (`dw_i_<obj>`), `reach` marker (`dw_r_<obj>`). Prop/affordance *blocks* (`interact.prop`, `collect` chest) are scenery and persist; `talk-to`/`kill` summon no per-objective marker. Gated on v0.3+ with a resolved activation, so v0.2 stays byte-identical. |
| `set-time` / `set-weather` | `time set <kw>` / `weather <kw>` (dimension-global, no selector) inline in the effect/dialogue-option function; instantaneous cut, persists (cycle frozen). |
| relight fixtures (`lighting`) | `setblock` per placed fixture in `setup_finish`, after structure placement + socket seals (spec-0010). Blocks: `torch`/`wall_torch`, `lantern[hanging=…]`, `campfire[lit=true]`, `shroomlight`. |
| `mitigation: "night-vision"` | `night_vision_tick`: one `effect give @a[x=…,dx=…,y=…,dy=…,z=…,dz=…] minecraft:night_vision 12 0 true` per declaring area (selector = the area's final placed bounds, compile-time literals), then `schedule function <ns>:night_vision_tick 20t` (vanilla replace-mode, so the clock can never double up). `setup_finish` arms it once. A generated `v06_night_vision` PackTest teleports a dummy into the declared bounds, runs one clock tick and asserts it holds the effect — then teleports it 1000 blocks out and asserts it does not. |
| `set-checkpoint` | Inline: `spawnpoint @a <x y z>` + `data modify storage dw:cp pos set value [x,y,z]` (the readable "last checkpoint" mirror); when any checkpoint has `on_respawn`, also `#cp dw.sys = <index>`. `setup_finish` seeds `dw:cp` to the spawn cell. `on_respawn`: `deathCount` objective (`dw.deaths`) + per-player ack; `tick` runs `cp_respawn_check` (fire on the death-count edge, dispatch `cp_on_respawn_<index>` for the active checkpoint). |
| `timed-gate` (spec-0016 §4) | `setup_finish` starts the clock: `function <ns>:tgate_open_<id>` at `phase: 0`, else `schedule … <phase>t`. `tgate_open_<id>` = `fill … minecraft:air replace <block>` + `schedule function <ns>:tgate_close_<id> <open_ticks>t`; `tgate_close_<id>` = `fill … <block>` + `schedule function <ns>:tgate_open_<id> <closed_ticks>t`. Both halves are pure world edits naming no player, so the server command source they are re-entered under is irrelevant (§4). Generated PackTest `souls_timed_gate`: re-seal, assert sealed, drive the real open, assert air, drive the real close, assert sealed again. |
| `shortcut` (spec-0016 §2) | `setup_finish` summons the far-side unlock affordance (`minecraft:interaction`, tag `dw_sc_<id>`) — and emits **nothing** for the gate, which the prefab already seals. `tick` polls the affordance's `interaction` record once, guarded by the `#sc_<id> dw.sys` sentinel → `shortcut_open_<id>`, then clears the record. `shortcut_open_<id>` latches the sentinel, clears the gate region (`fill … minecraft:air replace <block>`, the same command `open-gate` emits) and runs `on_unlock` server-source-safe. No emitted function anywhere ever re-fills a shortcut gate — the runtime half of permanence, asserted by a test over the whole datapack. Generated PackTest `souls_shortcut`: sealed before, air after, still air after a second unlock pass. |
| `lane` / `summon: aggro-edge` (spec-0016 §6) | A lane wave's `spawn_<wave>` summons the squad with `,Patrolling:1b[,PatrolLeader:1b],patrol_target:[I;x,y,z]` (leader = the first summoned mob, also tagged `dw_lead_<wave>`), `follow_range` forced to `aggro_radius`, then sets `#lane_<wave> dw.sys 0` and schedules `lane_tick_<wave>` at 30t. **Only the snake_case int-array routes** — 1.21.11's strict codec silently drops the legacy `PatrolTarget:{X,Y,Z}` compound and the squad then patrols to vanilla-rolled random points (working-but-drunk); `Patrolling`/`PatrolLeader` keep their camelCase names. `lane_tick_<wave>`: advance guards in DESCENDING index order (so one cycle steps at most one waypoint) firing when ANY squad member is within 8 blocks of the current waypoint (any member, not the leader — a dead leader must not strand the warband); one `data merge entity @s {Patrolling:0b}` release for every mob with a player inside `aggro_radius`; one per-index re-assert `{Patrolling:1b,patrol_target:[I;…]}` for every mob with nobody inside it (this is what defeats vanilla's arrival re-roll and the lone-patroller self-cancel, and it is inert during combat because the patrol goal cannot restart while the mob has a target); then a `schedule … 30t` re-arm guarded on the squad still existing, so the clock stops by itself. An `aggro-edge` wave carries no patrol NBT at all — only its ring placement. Generated PackTests `souls_td_patrol_nbt`, `souls_td_lane_march`, `souls_td_lane_release`, `souls_td_aggro_edge`. |
| `bonfire` (spec-0016 §1) | Inline at the arming beat: `execute unless entity @e[tag=dw_bonfire_<i>] run summon minecraft:interaction … Tags:["dw_bonfire_<i>"]` — nothing else; the checkpoint does not move. `tick` polls the affordance's `interaction` record (`nbt={interaction:{}}`) → `bonfire_rest_<i>`, then clears the record so the next rest re-fires (no one-shot sentinel). `bonfire_rest_<i>` = the three `set-checkpoint` lines (`spawnpoint @a`, the `dw:cp pos` mirror, `#cp dw.sys = <i>`) + the wave re-seats + the `on_rest` bundle, emitted **server-source-safe** (§4 — the tick has no `@s`, so per-player effects re-bind to `as @a` and global effects fire once). The respawn path runs the same `on_rest` bundle through `cp_on_respawn_<i>` under the player executor, so a bonfire with an empty `on_rest` still dispatches when it owes a re-seat. Generated PackTests `souls_bonfire_rest` (the real rest function moves `dw:cp` to the bonfire cell) and `souls_bonfire_reseat` (a met, wiped wave stands again at its authored count after a rest; an unmet one is not conjured). |
| `begin-stealth` / `end-stealth` | `begin` → `#stealth dw.sys = <session>` + reset per-player `dw.st_grace`. `tick` runs `stealth_tick_<session>` while active → per-player `stealth_eval_<session>`: safe iff inside some zone box (a pure position selector — **zone presence alone = hidden**, owner ruling 2026-08-01; the earlier sneak-edge requirement is gone, it collided with the spectator cutscene camera); grace resets when safe, climbs when exposed, and at `grace_ticks` fires `stealth_caught_<session>` (`on_caught`). `end` → `#stealth dw.sys = 0`. The `v06_stealth` PackTest disarms `#stealth` (sets it 0) after each `stealth_begin` because it drives `stealth_eval` explicitly: an armed session would make the world `tick` loop run a *second* judge pass in the same tick, double-counting exposure and mis-accruing grace (this only isolates the test; runtime gameplay has the tick loop as sole caller). It pins its dummy by tag (see "PackTest batch model" below), drives hidden/exposed purely by teleporting the dummy in/out of the zone box, runs the spare (safe-player) section first and the `on_caught` trip LAST — the trip executes arbitrary campaign `on_caught` content (possibly lethal), so nothing state-dependent follows it and the closing assert reads the dummy through the tag, which keeps matching even if the trip killed it. |
| `give-item` `carrier` (v0.6, spec-0018) | Absent/`all` → `give @a <item> <count>`: a quest beat arms the whole party. `one` → `give @s …`, the single quest prop handed to the player whose action fired the effect, for the party to pass around physically. `one` inside a scheduler-only bundle has no acting player and is rejected at validate time (`DW0357`). |
| trap `dispense` (spec-0011) | `setup_finish`: `item replace block <disp> container.0 with <item> <count>` fills the prefab's pre-wired dispenser socket (the `anchor/trap` metadata `dispenser` cell) — a static, deterministic payload, the same mechanism as a `collect` chest. **No detection** is emitted for the harm: the plate/tripwire/trapped-chest → dispenser redstone is already in the prefab. Pressure plates and tripwire are modelled **passable** in the assembled occupancy (`crate::assembled::is_passable_trap_trigger`) so nav routes a player ONTO a trigger cell rather than around a "solid" plate. |
| trap `requires_flags` / `forbids_flags` (spec-0011) | A **physical** gate, because the compiler owns world mutation: `trap_gate_on_<trap>` restores the trigger block declared by the `anchor/trap` metadata's `trigger_block` (verbatim, blockstate and all) and `trap_gate_off_<trap>` clears the cell to air, so a shut gate means a player stepping on the trigger steps on nothing. Edge-triggered on a `#trapgate_<trap>` sentinel, so the `setblock` fires on a flag transition rather than every tick. The gate is **campaign state, not per-player state** — flags are set by whoever reaches the beat — so the `tick` guards use the any-player form (`if entity @a[scores={dw.f_<flag>=1..}]`), one shutting clause per gating flag ("not (all required and no forbidden)" is a disjunction) and one opening clause carrying the full conjunction. `setup_finish` seeds the sentinel to the world the campaign starts in: a `requires_flags` gate starts shut (no flag is set yet) and clears the cell immediately, a `forbids_flags`-only gate starts open on the prefab's own block. An **ungated** trap emits none of this (byte-identical). Only sound for a trigger whose whole state is the block — `DW0363` rejects the rest rather than shipping folklore. PackTest `v06_trap_gate`: flag set → the trigger cell is air; flag cleared → the authored trigger is back. |
| trap `disarm` (spec-0011) | `setup_finish` summons a `minecraft:interaction` at the disarm `via` cell (tag `dw_trapdis_<trap>`); `tick` fires `trap_disarm_<trap>` once on a right-click (`nbt={interaction:{}}`, reusing the v0.4 `use` primitive). `trap_disarm_<trap>` sets the party-wide `dw.f_<flag>` and empties the dispenser (`data modify block <disp> Items set value []`) — the modeled, global disarm that actually stops a redstone dispense trap. |

**The party holder (`#party`, spec-0018).** Progress is a fact about the party,
not about a player. Every progression score — `dw.o_<obj>`, `dw.q_<quest>`,
`dw.qa_<quest>`, `dw.f_<flag>`, `dw.ann_<obj>` and `dw.campaign` — is read and
written on the single fake player `#party`, so any member's completing action
advances everyone, and `after: [obj/a, obj/b]` becomes a **division of labour**:
A clears one arm in one room, B the other in another, and the successor's guard
(every term a `#party` read) opens for both. A fake player needs no entity and
survives every join/leave, which is exactly the lifetime party state needs.

Consequences, all mechanical:

- the `announce_<obj>` / `activate_<obj>` tick drivers need no player context at
  all (their whole predicate is party state) and therefore fire **once for the
  party**; the completion drivers keep `as @a` because they still test a real
  player (proximity, held items, a fired trigger). Those stay single-fire because
  vanilla evaluates `execute as @a … if … run` per selected player *in turn*: the
  first player's `run` sets the party score and every later player's `unless score
  #party …` fails in the same tick;
- objective/quest/campaign UI addresses `@a` (`tellraw @a`, `title @a`,
  `playsound … @a`, `advancement grant @a`), so the party is told together;
- what stays **per-player** is exactly what belongs to a body: `dw.class` /
  `dw.classed` / `dw.dlg_shown`, `dw.dlg_<npc>` / `dw.i_<obj>` triggers,
  `dw.dmask` (this player's dialog screen — its *conditions* read `#party`),
  `dw.hold`, `dw.deaths` / `dw.death_ack`, `dw.st_grace` / `dw.st_safe`,
  inventory, position, and cinematic attach/restore.

CI-enforced by `tests/party.rs::no_per_player_progression_scoreboard_remains`, a
sweep over every emitted pack of every fixture family: a progression score may
appear only after the `#party` holder token (or in its `scoreboard objectives
add` declaration), and no selector may filter players by one. A *partial*
migration — player A's objective set, player B's guard still shut — is the
soft-lock that no single-player test can see.

Naming: `dw.o_<obj>`, `dw.q_<quest>`, `dw.qa_<quest>` (active), `dw.dlg_<npc>`,
`dw.f_<flag>`, tags `dw_npc_<npc>`/`dw_wave_<id>`/`dw_i_<obj>`/`dw_r_<obj>`.
`CustomName` is a plain SNBT text component (not `'{"text":…}'`).
v0.6 checkpoints/stealth (spec-0012/0014): storage `dw:cp pos` (last-checkpoint
mirror, a `[x,y,z]` int list); scores `dw.deaths` (`deathCount`) + `dw.death_ack`,
`dw.st_grace`/`dw.st_safe` (no sneak-stat scores — the judge is position-only);
markers `#cp`/`#stealth` on `dw.sys`. A campaign with a cutscene also ships the
datapack predicate `<ns>:sneak_held` (the cutscene bounce's re-attach gate, §4).

---

## 4. Hard invariants

### A scheduled bundle has no `@s` (executor contract)

`schedule function <ns>:<f> <n>t` re-invokes `<f>` with the **server** command
source: no executor, so `@s` resolves to nothing and every `@s`-addressed command
in it *silently does nothing* — no error, no log line. Three generated bundles are
reached only that way, and all three used to be emitted verbatim:

| bundle | reached from |
|---|---|
| `mv_arrive_<key>` | `mv_tick_<key>`, itself re-scheduled every tick |
| `ma_arrive_<key>` | `ma_tick_<key>`, likewise |
| `seq_<key>_<i>` | `seq_<key>`'s `schedule … <at_ticks>t` chain |

The cost (round-6 island, AUDIT-P0): two `on_arrive` bundles set the flags
`obj/take-cover` gates on, so the party soft-locked at "Get Into the Shadows" —
and the whole seal cinematic's `title`/`tellraw`/`playsound` beats were dead.

**The rule.** A bundle is emitted for an explicit **audience**
(`emit::Audience::{Party, Scheduled, Solo}`), and each effect is classified
individually, never the bundle as a whole (`emit_quest_effect` takes the audience
selector; the executor match is exhaustive — a new effect verb must state its
scope or the compiler refuses to build):

- `Party` — a party event entered as one player (`complete_<obj>`,
  `complete_q_<quest>`, `trig_<id>`): `@s` exists and is the completing player;
- `Scheduled` — the three bundles above: **no `@s` at all**;
- `Solo` — a checkpoint `on_respawn` / a stealth `on_caught`: the bundle belongs
  to the one player it fired for, and stays `@s` throughout (re-broadcasting one
  player's death would re-gift and re-narrate at every survivor).

Under `Party`/`Scheduled`:

- **player-facing** (`narrate`, `give-item`, `play-sound`, `damage-players`) →
  the command names `@a` directly (`tellraw @a`, `give @a`, `damage @a[…]`,
  `playsound … @a`), so the whole party sees the beat **once**. The one
  listener-relative form (a `players` sound with an explicit volume/pitch, which
  forces a `~ ~ ~`) is wrapped `execute as @a at @s run …` so `~ ~ ~` resolves at
  each listener rather than at the command's own position;
- **party-fact** (`set-flag` — now a `#party` write, gates, `set-block`,
  `spawn-wave`, `spawn`/`despawn`/`move`/`unleash-actor`,
  `spawn`/`despawn`/`move-npc`, `cutscene`, `set-time`/`set-weather`,
  `set-checkpoint`, `begin`/`end-stealth`, `sequence`, `campaign-complete`) →
  emitted **bare**, so it fires exactly once. A blanket `execute as @a run
  function <bundle>` — the obvious fix — is wrong: it would fire every `fill`,
  `summon`, driver start and `schedule` once per player.

**spec-0018 narrowed this seam to nearly nothing, and that is the point.**
Progression moved to the `#party` holder, so a scheduled `set-flag` writes
`scoreboard players set #party dw.f_<flag> 1` and names no executor — the exact
soft-lock the AUDIT-P0 fix was written for **cannot recur for flags**. Every
player-facing effect addresses `@a`, which needs no executor either. Exactly one
construct is still executor-shaped: a `carrier: "one"` `give-item`, which needs
the acting player, and is therefore rejected at validate time inside a
scheduler-only bundle (`DW0357`).

Per-effect flag gates have **one spelling** everywhere now (`if score #party
dw.f_<flag> matches 1` / `unless score #party …`, unset-safe): flags are party
state, so there is no per-player variant to diverge and no "does some player hold
it" selector to approximate it with. These bundles previously dropped the gate
entirely (they called the ungated emitter), so a gated effect inside an
`on_arrive`/`sequence` step fired unconditionally.

**`sequence` is a global timeline.** *Every* step function is emitted
server-source-safe, the inline `at_ticks: 0` one included — a timeline whose first
beat behaved differently from its second would be a trap, and `seq_<key>` is itself
reachable from a scheduled bundle (a `sequence` nested in an `on_arrive`).
Consequence: a sequence's per-player beats address the **party**, not one acting
player, wherever the timeline is started from.

**Enforcement** (all three; never relax one):

1. `tests/scheduled_executor.rs` walks the emitted call graph from every
   `schedule` site — following `function` calls that do *not* re-bind the
   executor — and asserts no function in that closure names `@s` outside an `as`
   clause (`positioned as`/`rotated as` do not bind). Fails on pre-fix output
   with the exact dead commands listed. Post-spec-0018 it passes because nothing
   in those bundles addresses a player at all — which is the strongest form the
   lesson can take.
2. Two generated PackTests drive the **real scheduler** (never an inline
   `function` call — running the driver inline *as the dummy* supplies exactly
   the executor the scheduler withholds, which is how a green suite hid this bug
   for a milestone): `sched_executor` (unconditional, so every campaign proves
   the seam live — it schedules a probe function emitted by the real
   scheduled-bundle emitter and awaits the flag on its own dummy) and
   `sched_arrive_flag` (the content path: the first `move-npc` whose `on_arrive`
   sets a flag; runs the real start function and lets the driver walk itself to
   the end). Both verified to go red on pre-fix emission on a live 1.21.11 server.
   Both now `await` the flag on `#party` and are the **sole owner** of the score
   they await (`tests/packtest_batch.rs::party_state_across_ticks_is_owned`).
3. The suite datapack may therefore carry `data/<ns>/function/` mechanism
   functions beside `data/<ns>/test/`. PackTest only discovers `test/`, so
   `tests/emit.rs` requires every `function/` file to be **named by some
   template** — an orphan there is a test PackTest would never run.

### Semantics never key on player-facing text

**No semantic verdict may key on player-facing free text** (item/NPC display names,
titles, blurbs, hints). Semantics live only in ids, structured schema fields, or
first-class declarations. The removed night-vision name heuristic (`light.rs`,
deleted in the v0.6 mitigation PR) is the cautionary precedent: it read a kit item's
display name for "night vision", so a renamed water bottle passed `DW0210` while
nothing in the shipped world granted night vision — a check that passed without the
feature existing. Player-facing text is also localizable, so keying on it makes a
verdict language-dependent (ADR-0006).

### The completion-marker channel (the bot's oracle)

The critical-path bot's ONLY evidence that something completed is a chat line of
the anchored form

```
[dw:complete <campaign_id> <token>]
```

`<token>` is `campaign` (the whole delve, from `campaign-complete`) or the
completing objective's own `obj/<kebab>` id (broadcast by `complete_o_<obj>`, as
the score flips, before that objective's effects run). Both are `tellraw @a`,
dark-gray. The harness matches the **whole line**, exactly (`harness/src/markers.ts`
mirrors `plan::marker_line`) — never a substring of a longer line.

Why this shape. Before it, the harness tested every chat line for the *substring*
`[Delvewright] complete <objective> <value>`, emitted only for the campaign. Two
holes, both observed live: nothing stopped authored or translated content from
containing that substring, and a `reach`/`interact`/`talk-to` step passed on
arrival/on the dialogue opening while its own objective never completed — a 22/22
green island run whose campaign had in fact completed at step 12, the last ten
steps hollow. Three properties now make a forged completion impossible rather than
merely unlikely:

1. player chat reaches a client as `<name> …`, so no player utterance can begin
   with the sigil;
2. the campaign id is part of the match, so a marker from other content cannot
   satisfy this campaign's step;
3. `DW0182` reserves the sigil in every player-visible string — authored English
   and every sidecar translation alike.

The harness side of the contract (`critical-path.json` `format_version` 2, the
per-step `objective` id, and the endgame rule that campaign completion belongs to
the last objective step) is described under "World / build output" below.

### A cutscene is pure observation (`dw_cutscene`)

While a cutscene plays, every player carries the entity tag `dw_cutscene` —
added by the cutscene `start` alongside `gamemode spectator @a`, removed by the
`end`/restore, so the state has exactly the cinematic's lifetime. **Campaign
machinery must neither require anything of a tagged player nor punish them:**
they are watching, not playing. Current consumers:

- the **stealth judge** is skipped for them (`stealth_tick` selects
  `@a[tag=!dw_cutscene]`). The judge is the only writer of `dw.st_grace`, so
  skipping it freezes the clock — grace neither accrues nor expires, and
  `on_caught` cannot fire mid-cinematic. The restore deliberately leaves
  `dw.st_grace` alone: the beat resumes exactly where it paused (the judge is
  position-only, so there is no other stealth state to re-sync).
- **`damage-players`** skips them: every form of the verb is guarded by
  `tag=!dw_cutscene`.

**A disconnect mid-cutscene must not strand the player.** The whole bracket is
`@a`-scoped, so `cs_end_<bare>` restores gamemode, teleports and untags exactly
*the players online when it ends*. A player who dropped during the shot is not
among them: they rejoin still tagged, still in spectator, and the marker they
would have been teleported to has already been killed — a ghost with no way back.
`join_place` cannot help, because it is gated on `dw_joined`, which survives a
relog exactly like the cutscene tag does. The repair is therefore its own `tick`
clause keyed on the **stuck state itself** — tagged while nothing is playing:

- the bracket refcounts itself on `#cs_live dw.sys` (`add 1` in `start`, *after*
  the re-entry `return fail` so a re-entrant start cannot inflate it; `remove 1`
  in `end`). A refcount, not a flag: nothing forbids two cutscenes overlapping,
  since each start only guards re-entry into itself. Never initialized, so the
  `unless … matches 1..` test reads correctly before the first cutscene runs.
- `execute unless score #cs_live dw.sys matches 1.. as @a[tag=dw_cutscene] run
  function <ns>:cs_repair` — a player tagged while a cutscene *is* playing is
  left alone, because `cs_end_` will collect them normally.
- `cs_repair` is strictly per-player (`@s`): `gamemode adventure`, drop the tag,
  and a macro `tp` to `storage dw:cp pos` (via `dw:cs at` + `cs_repair_tp`,
  the same shape the boundary return uses). The destination is the live
  checkpoint rather than the cutscene's own saved position because that marker
  is destroyed by `cs_end_` before this can ever run.

A cutscene-less campaign emits none of it (byte-identical).

**The `spectate` bounce is sneak-gated** (round-6 flicker fix). In spectator
mode the sneak key dismounts the spectated entity, so an unconditional per-tick
re-attach against a held key strobes: attach → client dismount → attach, every
tick. Both bounce lines therefore select
`@a[predicate=!<ns>:sneak_held]` — the vanilla `minecraft:player` `input`
sub-predicate (1.21.2+), which reads the client's raw input packet and so
reports the held key in every gamemode, spectator included. A player holding
sneak mid-cutscene settles into a stable detached spectator (frozen, staring at
the world — acceptable; strobing is not) and re-attaches on the first bounce
tick after release, resuming the shot. The predicate file
(`data/<ns>/predicate/sneak_held.json`) is emitted only for a campaign with at
least one cutscene; everything else stays byte-identical. This gate is also why
stealth no longer asks players to sneak: holding sneak and spectator cinematics
are inherently in conflict, so no delve mechanic may require a held sneak.

Any future verb that *demands input* or *deals harm* joins this list. The origin
is a round-4 island playtest where the stealth clock kept running through a dolly
and the catch killed the owner mid-shot, desyncing the beat.

### Shot styles (`shot_style`, v0.6 — spec-0015, camera dossier §2)

A styled shot expands at compile time into the same dolly + aim geometry an
explicit `path` produces — a pure function of (style, params, subject
geometry). `dist` is the only "lens" control (vanilla has no in-game FOV);
durations default from the dossier's film-editing ranges; every expanded path
runs the same `DW0308` clip (authored + rendered chords) and `DW0347` angular
budget as a hand-authored one. Placement is rule-based (no world-aware
candidate scoring yet — dossier §4's compile-time ClearShot is future work);
`bearing` (compass degrees: 0 = camera south of subject, 90 = west) steers the
placement, and an explicit `path`/`look_at`/`seconds` overrides any part.
Entity subjects (`npc`/`actor`) aim one block above the feet cell (torso)
before `offset`; `anchor` subjects use the block centre exactly.

A `subject` is discriminated by its key — exactly one of `anchor` / `npc` /
`actor`, plus an optional `offset`, **and nothing else**. Each spelling is its own
`deny_unknown_fields` type (`AnchorSubject` / `NpcSubject` / `ActorSubject`), so
both serde and the exported JSON Schema (`additionalProperties: false`) reject a
typo'd key or a subject naming two discriminators at once with `DW0100` (task
#78). Before that, the untagged enum silently *ignored* an unrecognised key: a
mistyped `ofset` deserialized fine with the offset dropped, and
`{"anchor": …, "npc": …}` quietly matched the anchor and discarded the npc —
shipping a shot framed somewhere the author never asked for.

| Style | Expansion (camera relative to subject S) | Aim | `dist` default | Default `seconds` | Notes |
|---|---|---|---|---|---|
| `insert` | Static at `dist` (3), +0.5 up | S | 3 | 2 | A prop, an inscription. Structurally judder-free. |
| `locked-off` | Static at `dist` (12) abeam the subject track's midpoint, +2 up | tracks S | 12 | 6 | Subject may be moving (aim pans) or static. |
| `push-in` | Dolly `dist` → `dist`/3 (min 2) along the bearing axis, +1 up | S | 12 | 4 | Dread; a line landing. |
| `pull-back-reveal` | Dolly `dist` → 4×`dist`, +1 up | S | 4 | 6 | "You are not alone." |
| `establishing-crane` | `dist`, +12 up → `dist`/2, +4 up (Δy −8) | S | 24 | 8 | First sight of an area. |
| `orbit-arc` | Arc of `degrees` (45–120, default 90) at radius `dist`, +2 up, from `bearing`; one waypoint per ≤10° | S | 12 | 8 | Constant angular speed via arc-length parameterization. |
| `side-track` | Per-tick camera = subject track + constant offset `dist` right of overall travel (`bearing` rotates it), +1 up — the Rockstar phantom-vehicle rig | tracks S | 8 | 8 | **Requires a moving subject** (`DW0349`); no easing — the subject's motion profile governs. |
| `two-shot` | Static on the AB perpendicular bisector nearest `bearing`, +1 up; d = (|AB|/2)/tan(α/2), α = 70°/3 (thirds framing), clamped 5–9; `dist` overrides | midpoint of A,B | Toric solve | 5 | Toric-space-inspired closed form (Lino & Christie, SIGGRAPH 2015 / SCA 2012 — ideas only). Needs `subject_b`. |
| `low-follow` | Per-tick camera = subject track + `dist` directly behind overall travel (`bearing` rotates), +0.5 up | tracks S | 4 | 5 | **Requires a moving subject** (`DW0349`). The dossier's worst-case style: the angular budget is the guard. |

### PackTest batch model (one dummy per test, one shared server)

PackTest runs the whole generated suite as **one batch on one shared server**:
every `# @dummy` test spawns its **own** dummy player, all dummies coexist, and
all test functions execute over the same server tick(s), sequentially in an
order the compiler does not control. The conversion is **total** and the rule is
hard: **every generated test is interleaving-independent — own dummy, own
scores, own init** (round-5 + round-6 island reds; `pin_dummy` in `emit.rs`;
CI-enforced over every fixture family by `tests/packtest_batch.rs`):

- **Own dummy — `@p` is not "the test's player".** It re-resolves from the test
  structure origin on every command — the moment a template teleports its dummy
  to absolute campaign coordinates, `@p` retargets to a *neighbor test's* dummy
  and later writes/asserts land on the wrong player (`v06_stealth` read a
  foreign dummy's grace). A template that drives per-player state tags its
  dummy on its first post-setup line (`tag @p add dw_t_<test>` — while its own
  dummy, inside its own structure, is still the nearest player) and addresses
  it exclusively via `@a[tag=…,limit=1]`, which — unlike `@p` — also keeps
  matching a dummy that campaign content has killed. `@s` (the executing dummy)
  is equally safe — the binding survives teleports. Bare `@a` writes are
  forbidden: they hit every coexisting dummy (`verb_flag_gate`'s withheld flag
  arrived via `verb_interact`'s old `@a` preamble).
- **Own scores.** Fake-player scratch holders on `dw.sys` are batch-global, so
  every template suffixes its own (`#n_sidm`, `#bx_bret`, `#dm_dvis`, …); no
  two templates share a holder. Real runtime scores (`#stealth`, `#placed`,
  `#trig_<id>`, the `#mt_`/`#at_`/`#arun_` move drivers) are deliberately
  shared — tests drive them and initialize them explicitly.
- **Own scores, extended to party state (spec-0018).** Progression now lives on
  the batch-global `#party` holder rather than on each test's dummy, so a
  template's baseline writes are visible to every sibling. Inside a template that
  is harmless — a template is one atomic mcfunction, so its baseline, its drive
  and its assert land in one tick with nothing in between. It stops being
  harmless the moment a template spans ticks: `party_state_across_ticks_is_owned`
  requires that any template containing an `await`/`schedule` be the **sole**
  template touching each `#party` score it uses (`sched_executor`'s probe flag is
  test-only for exactly this reason).
- **Own members (spec-0018).** A division-of-labour template needs more than one
  player, and `# @dummy` gives exactly one. It spawns the rest itself
  (`/dummy <name> spawn`, PackTest's own command), addresses them by
  `@a[name=…,limit=1]` — as exclusive as a tag, and admitted alongside it by rule
  2 — and removes every one it spawned (`spawned_members_are_uniquely_named_and_removed`
  also checks the ≤16-char player-name limit and cross-template name uniqueness).
- **Own init.** "Never set" is not 0 and "fresh world" does not exist here:
  every score a template asserts on is actively initialized by that template
  (`packtest_preamble` with `with_flags: false` clears withheld flags to 0),
  and every entity tag it counts on is cleared on entry. Sibling residue is
  real: `v06_unleash`'s leftover real-AI twin carried `dw_actor_<id>` with no
  puppet marker, so `v06_spawn_idempotent`'s guarded spawns
  (`unless entity @e[tag=dw_actor_<id>]`) no-op'd and it counted 0 puppets —
  a pass/fail decided purely by batch order on byte-identical packs. Templates
  also leave no residue of their own (actor tests kill the actor tag on exit),
  and templates that re-run the unguarded `setup_finish` clear every planned
  NPC tag first (its summons would otherwise duplicate bodies + hitboxes).
  Each template is a single mcfunction and therefore atomic — nothing can
  interleave *within* it; these rules make the boundaries between templates
  order-free.
- **Division of labour is not simulable with one dummy.** A single-dummy test of
  an AND-join proves only that one player can do both arms in sequence — which
  was already true before the party holder existed. The generated
  `party_join_<obj>` template therefore drives **n different players**, one arm
  each, and asserts the join's REAL emitted `pending_guard` (materialized into
  `#pj_<obj> dw.sys`) in three phases: shut with no arm, **still shut after only
  one** (the negative half that makes it an AND, not an OR), open after all of
  them — and then has the LAST member, never the one who cleared the first arm,
  complete the successor. n = the join's arm count, raised to `world.min_players`
  and capped at 4 (the party maximum); arms are handed out round-robin.
- **Drive the real mechanism, not a convenient stand-in.** A template that calls
  a *scheduled* driver inline (`function <ns>:mv_tick_<key>`) runs it **as its own
  dummy** — supplying exactly the executor the vanilla scheduler withholds, so the
  test passes while the shipped delve soft-locks (AUDIT-P0; §4 "A scheduled bundle
  has no `@s`"). Tests of scheduled machinery hand it to `schedule` and `await` the
  outcome (`sched_executor`, `sched_arrive_flag`); the pre-existing
  `v06_move_actor`/`v06_arrive_handoff` inline drives stay as entity-state
  assertions, which is all they ever claimed.

### Determinism (ADR-0006)

- Same DSL + seed (+ `--lang`) → byte-identical `<out>/` tree. Gated by the
  double-build test (`tests/cli.rs`).
- All map/set iteration `BTreeMap`/explicit sort; JSON is `serde_json` pretty
  (sorted keys) + trailing newline.
- **No** wall-clock, hostname, locale, or absolute build path in any output byte.
- Only randomness = stage-1 `seed` → named splitmix64 per-area streams. Solver
  retry (≤32 attempts) is seed-deterministic; attempt 0 reproduces pre-M2 growth.

### Environment sealing (bootstrap `#minecraft:load`, idempotent, `#init`-guarded)

**1.21.11 renamed every gamerule** (verified live 2026-07-30; legacy camelCase
and `minecraft:`-prefixed forms both rejected). Emitted sealing commands
(`emit::sealing_commands`):

| Legacy (spec text) | 1.21.11 accepted (emitted) |
|--------------------|----------------------------|
| `doMobSpawning` | `gamerule spawn_mobs false` |
| `doDaylightCycle` | `gamerule advance_time false` |
| `doWeatherCycle` | `gamerule advance_weather false` |
| `doFireTick` | `gamerule fire_spread_radius_around_player 0` (no boolean successor; radius 0 = no spread) |
| `mobGriefing` | `gamerule mob_griefing false` |
| `spawnRadius` | `gamerule respawn_radius 0` (spawn **scatter** off — vanilla otherwise scatters a first join / spawnpoint-less respawn uniformly in a square of this radius around world spawn; every scattered cell in a box garden is solid prefab or void, so the only correct radius is the exact compiler-chosen anchor) |
| — | `gamerule keep_inventory true` (box-garden death policy; **not in spec-0002** — see §6) |
| — | `gamerule tnt_explodes false` (**v0.6-gated**, spec-0011): defense-in-depth against a stray primed-TNT source deforming the sealed world. No gamerule separates explosion block vs. entity damage, so TNT is excluded as a trap payload by the schema and belt-and-braces sealed here. Emitted **only** when the world stage is `dsl_version 0.6.0`, so pre-0.6 fixtures stay byte-identical. |
| — | `time set <kw>` (declared `world.time`, default `noon` = daytime 6000; the sole seal with a vanilla read-back) |
| — | `weather <kw>` (declared `world.weather`; emitted **only when declared** — `clear` is the vanilla default, so an undeclared campaign emits nothing and stays byte-identical to pre-0.5) |

- Gamerule *values* have no vanilla read-back → asserted at compile time only;
  PackTest asserts the one queryable seal (`time = daytime_ticks(world.time)`,
  e.g. 6000 for `noon`, 18000 for `midnight`). Regression asserts exact forms and
  that legacy names never appear.
- Time/weather freeze: cycles are frozen (`advance_time`/`advance_weather false`);
  a set state persists until the next explicit set. Stage-1 `time`/`weather` +
  `set-time`/`set-weather` (spec-0010) make these first-class. The assembled-light
  model judges sky-open cells under the **darkest reachable (time, weather)**
  combination (initial ∪ every `set-time`/`set-weather` target).

### World / build output

- `server/server.properties`: `level-type=minecraft:flat` +
  `generator-settings={"biome":"minecraft:the_void","layers":[]}`,
  `level-seed=<seed>`, `gamemode=adventure`. `difficulty=peaceful` for wave-free
  campaigns; **`easy`** when any wave exists (peaceful removes summoned mobs). No
  server jar, no region files (ADR-0010) — the bootstrap `/place template`s
  prefabs, so byte-identity covers the whole tree.
- `horizon:"ocean"` (v0.6, spec-0013) swaps `generator-settings` for a pinned
  superflat `{"biome":"minecraft:ocean","layers":[bedrock×1, stone×118,
  water×8]}`: from the −64 build floor the top water block lands at **y=62** (sea
  level). Still no structures/mobs (`generate-structures=false` + gamerule
  `spawn_mobs false`). `void`/absent is byte-identical to v0.5.
- **Sea-level datum (ocean).** An ocean world places its areas at **y=60**
  (`plan::OCEAN_BASE_Y` = `SEA_LEVEL − ISLAND_WATERLINE_Y`), not at the void
  datum 64. The island tileset (`prefabs/island-tileset.md`) authors every piece
  with its waterline — the top authored water block — at **local y=2** and its
  walkable land plane at local y=3; placing the base at `sea_level−2` makes the
  authored water one body with the world ocean and puts the shore exactly one
  block above the sea, the vanilla-normal beach a swimming player can climb.
  Placed at 64 instead, the whole island floats ~4 blocks above the sea: the
  authored water pocket hangs in the air and open water becomes an inescapable
  moat. *Assumption:* the waterline height is a **library** constant, not a
  per-piece one — placement uses the single tileset convention (2) so every area
  of an ocean world shares one datum, and prefab metadata's optional
  `waterline_y` is a *declaration checked against* that datum (`DW0344`), never
  an input that moves it. Everything downstream (nav/critical path, boundary
  region, checkpoint storage, POV shots, PackTests) derives from placement and
  simply follows the new Y. The water-flood model is unaffected: it seeds only
  from authored `minecraft:water` cells inside placed pieces and never climbs,
  so the walk plane one block above the waterline stays dry by construction —
  the world ocean is backdrop, not a flood source.
- Prefab metadata may declare **`waterline_y`** (optional, integer, local y of
  the piece's top authored water block). Island-tileset pieces declare `2`;
  pieces that author no sea (keep/cave interiors, `hello-room`) omit it and are
  not checked. Consumed only by `DW0344`.
- `boundary` (v0.6, spec-0013) emits, in `setup_finish`: a `dw:region bounds`
  storage mirror (readable region contract), a `dw:cp pos` init to the spawn cell
  (shared with spec-0012 checkpoints — the last-checkpoint mirror the return
  reads; idempotent, gated once via `needs_cp_init`), and `schedule function
  <ns>:boundary_tick 20t`. `boundary_tick` (self-rescheduling 1s clock) ejects
  every `@a` outside the region via `boundary_return` (a macro `$tp @s $(x) $(y)
  $(z)` off `dw:cp`, + actionbar message + soft sound). The region selector is
  compile-time-derived literals; nothing is authored.
- **Entry point.** One cell per campaign — `setworldspawn`, the `class_apply_*`
  teleport, first-join placement, the `dw:cp` seed and the gate-deadlock proof's
  start node all use it. It is the first area's entry anchor, resolved by the
  compiler through an ordered alias list (`plan::ENTRY_ANCHOR_NAMES` = `spawn`,
  then `entry`): one concept with two spellings in the shipped tileset library
  (keep/cave/test say `spawn`, the island tileset says `entry`), so the compiler
  owns the resolution rather than leaving it to per-tileset folklore. Resolving
  **none** of them is `DW0345`.
- **First-join placement is datapack-owned** (not the server's reading of
  level.dat). `tick` runs `execute if score #placed dw.sys matches 1 as
  @a[tag=!dw_joined] run function <ns>:join_place`; `join_place` teleports `@s` to
  the campaign entry point (the first area's `spawn` anchor — the same cell
  `class_apply_*` uses) and then adds the `dw_joined` tag, so it fires exactly
  once per player and a relog keeps the player where they stood. **Respawn is
  untouched** (`spawnpoint @a` + the spec-0012 checkpoint machinery). The `#placed`
  gate makes the teleport land on real geometry — the prefabs are `/place
  template`d over the first ticks. *Why it exists:* the **integrated
  (singleplayer) server** does not reliably honour the emitted spawn state and
  drops the first join at the superflat floor (x/z of world spawn, y = build
  floor) — inside stone, unescapable except by dying. A dedicated server places
  the same world correctly, so no rung of the validation ladder can observe it;
  the assertion is therefore static (`first_join_placement_emitted`). The target
  is the entry point rather than the live `dw:cp` checkpoint deliberately: `dw:cp`
  is *seeded* to that same cell at setup, so they agree at world start and diverge
  only once a checkpoint has fired — at which point a *first*-joining player is a
  player who has not played, and the entry point is where the campaign begins.
- `datapack/pack.mcmeta`: `min_format`/`max_format` = `[94, 1]` (a bare
  `pack_format` is rejected for formats > 81).
- `resourcepack.zip` → `pack.mcmeta`: `min_format`/`max_format` = `[75, 0]`, and
  **no** bare `pack_format`. Resource packs and data packs share one
  `pack.mcmeta` codec; only the "must declare `min_format`/`max_format`"
  threshold differs — **64** for resource packs, 81 for data packs — and the
  codec cross-checks a bare `pack_format` against `max_format`, so emitting both
  risks a declaration-mismatch error. Formats are pinned in `versions.toml`
  (`[resourcepack] pack_format`) from the 1.21.11 client's `version.json`
  (`resource_major: 75, resource_minor: 0`). Getting this wrong is **client-side
  only**: the pack is refused whole ("Pack declares support for version newer
  than 64, but is missing mandatory fields min_format and max_format") and every
  NPC skin silently never loads, while no server — and therefore no rung of the
  validation ladder — parses a resource pack at all.
- `<out>/`: `manifest.json` (SHA-256 of the 6 inputs + every output; non-`en`
  build adds `language` + hashes the sidecar), `datapack/`, `packtest-datapack/`,
  `server/`, `critical-path.json`, plus `resourcepack.zip`+`SKINS.md`
  (`resource_pack_sha1` in manifest) for a skinned campaign.
- `<out>/critical-path.json`: the bot contract. `version` is the **campaign's DSL
  version**; `format_version` is the **contract's own** version, currently `2`
  (`plan::CRITICAL_PATH_FORMAT_VERSION`) — bumped when what the harness is told
  about proving the path changes, independently of the DSL. At format 2 every
  objective-bearing step (`talk-to`/`reach`/`kill`/`collect`/`interact`) carries
  `objective`: the `obj/<id>` that step must prove, and the harness passes the step
  only when **that** objective's anchored completion marker arrives (position
  arrival, an opened dialogue and an emptied chest are means, never proof). The
  harness rejects any other `format_version` outright rather than running a path it
  cannot verify. Endgame rule: campaign completion is due at the LAST objective step;
  the campaign marker arriving earlier fails the run on the spot, because every
  remaining step is then provably hollow.
- `<out>/validation/critical-path-waypoints.json`: the DW0311-proven per-leg route
  thinned to sparse waypoints (`from`/`to` = the `critical-path.json` step
  positions; a waypoint at each corner/floor-height change **and the corridor commit
  cell one step past each corner** — task #45: a wide-room→corridor corner is
  range-1-satisfiable from an off-route pocket beside it, so the post-corner cell
  gives the harness a close corridor-axis target for its stall-recovery). A leg
  that walks through a closed fence gate carries a `use_gates` array (task #59):
  the gate cells the player right-clicks open (an adventure-legal USE), each also
  force-kept as an explicit waypoint (never thinned away mid-run); the field is
  omitted for gate-free legs, so gate-free campaigns stay byte-identical. The
  harness replays these as successive nearby pathfinder goals so no single distant
  A* solve strands the bot on a large open cave (its pathfinder's `canOpenDoors`
  performs the gate click — harness PR #110, whose fence-lip waypoint filter
  remains as defence-in-depth). **Validation metadata, not shipped gameplay** —
  excluded from the delve image (like `packtest-datapack/`); emitted only when a
  walked critical leg exists, so a fully-transported campaign stays
  byte-identical.
  A campaign with `timed_gates[]` (spec-0016 §4) additionally carries a top-level
  `timed_gates` table — one entry per declared gate, in declared order, with
  `id`, `region: {min, max}` (inclusive, canonical world-coordinate bbox),
  `block`, `open_ticks`, `closed_ticks`, `phase` — and every leg whose proven
  route walks through one carries `timed_gates: [<id>, …]`. A leg **crosses** a
  gate iff at some cell of its full A* route the player's own 2-block occupancy
  (feet cell or the cell above) lies inside the region — i.e. closing the gate
  would land the fill on the walk. The test is stated over the *unthinned* route
  (a straight run through the gate thins to its endpoints) and is exact rather
  than proximity-based: a leg that merely walks *past* a gate is deliberately
  unmarked, because the mark is what licenses the harness to retry a failed leg
  and a looser mark would grant blanket retries that mask navigation
  regressions. The gate **mouth** — each in-region route cell plus its immediate
  route neighbours — is force-kept as waypoints, exactly as a `use_gates` cell
  is: corner-thinning would otherwise collapse a corridor through a gate to its
  endpoints and ask the bot to walk the whole run inside one open window (18
  blocks through a 5 s window on the-drowned-bell, which loses the race), where
  pinning the mouth splits it into an uninterruptible approach plus a one-block
  crossing — which is what `DW0378` actually proves admissible (the *span*, not
  an arbitrary run-up to it). `DW0378` proves the window is *readable*; this
  export is what lets the runtime rung act on it — the harness stands off (only
  when caught inside the fill), waits for the closed→open edge and retries,
  bounded by two full cycles plus margin, instead of failing the leg when the
  gate fills mid-approach. Both keys are omitted entirely for a campaign with no
  gate clock, so such campaigns stay byte-identical.
- `<out>/render-plan.json` **player-POV shots** (`crate::render_plan::pov_shots`):
  the visual tier the owner's concern demands — the *player's own eye*, not the
  overhead/orbit cameras of the other shot kinds. One first-person `pov` shot per
  corner-thinned critical-path waypoint (the same `thin()` list the harness
  replays), camera at eye height (`1.62` above the standing cell), oriented along
  the walk toward the next waypoint and — at each leg's final waypoint — toward the
  objective anchor it arrives at (approach-heading fallback when the anchor is
  underfoot, so an arrival never degenerates to a straight-down floor shot). Each
  shot carries `leg`, the served `objective`, `standing_cell`, a `camera` with the
  first-person `fov` (~70°), and an `expect` whose first entry is a one-sentence
  machine description composed from campaign data (area name + objective/anchor/NPC
  names + objective hint) — the (image ↔ expect) pair a vision model reviews.
  Deterministic (route order → waypoint order; no RNG/clock) and appended after the
  overhead kinds so the existing shot prefix is unchanged. Emitted only when a
  walked critical leg exists.
- **`lighting` stamp** (POV + interior shots, `crate::render_plan::area_lighting_stamp`):
  pure metadata derived from the shot's area's **stage-1 declarations** — never from
  measurement (the measured model gates via `DW0210`/`DW0211`). `lighting` declared
  → `{"profile": "lit"}`; only `mitigation: "night-vision"` declared →
  `{"profile": "dark", "mitigation": "night-vision"}`; both → lit profile plus the
  mitigation; neither → **no key** (absent, not null), so campaigns without lighting
  declarations build byte-identically. Purpose: a declared-dark scene is pure black
  to an honest path tracer (the first island Chunky run proved exposure boosts
  cannot reveal a sealed cave — no light, only amplified noise — while real
  emitters render), so the stamp tells `delve-render` exactly which shots need its
  night-vision review emulation (below) and guarantees it touches no others.

### Assembled-world model (shared, gravity-settled)

`crate::assembled` builds the one authoritative cell→block map of the world the
shipped delve actually assembles — placed prefab structures (`/place template`),
solver socket seals, gate clears — **then settles gravity-affected blocks**.

The map stores **full blockstates** (`minecraft:oak_slab[type=top]`), not bare
ids (task #78): waterlogging, slab halves and snow-layer counts are block *state*,
and the fluid and step models below are wrong without them. Consumers that need
the bare id call `crate::assembled::base_id`; state-sensitive rules read their
property with `state_value`.

The
delve ships into a `the_void` flat world (no natural floor), so a vanilla
`FallingBlock` (`sand`/`red_sand`/`gravel`/`*_concrete_powder`/anvils/`dragon_egg`)
placed unsupported by `/place template` immediately falls out of the world and
leaves air. Settling reproduces this per `(x,z)` column: non-falling **solid**
blocks are immovable supports (stone floats), each falling block drops onto the
highest support at or below it, and a falling block with no support anywhere below
it despawns into the void. **Fluids are not supports** (task #78): vanilla's
`FallingBlock.isFree` counts a water/lava cell as free space, so a falling block
sinks straight through and lands on the first genuinely solid block, *displacing*
the fluid in the cell it rests in — and a gravity block over water with no floor
beneath still despawns. (The pre-#78 model floated sand on the water surface, a
phantom floor that then dammed the flood and that nav walked on.)
`pointed_dripstone`/`scaffolding` attach upward / by
support-distance and are deliberately not settled by the below-support rule (a
ceiling stalactite must not be mistaken for an unsupported floor block). Both the
nav occupancy model (`crate::nav::World`) and the relight light model
(`crate::light::LightModel`) derive from this single settled map, so a `sand`
floor laid over void is a *hole* in every consumer — DW0311 walkability, DW0312
wave seating, the relight pass, and the waypoint export — exactly as in game, not
a phantom floor the model wrongly "proves" solid (task #42). Determinism
(ADR-0006): fixed placement/seal/gate order, `BTreeMap`-ordered column iteration,
bottom-up stacking.

The settle pass also feeds the **`DW0313` gravity-despawn gate**: a gravity block
that despawns (falls with no support anywhere below) is always a defect — no DSL
verb can intend it — so `crate::assembled::gravity_despawn_error` fails the build
directly at its start, before any consumer, listing the offending pieces/cells and
prescribing a substrate. This is the authoritative gate for the pitfall; a fall
that merely lands on support is left to the faithful settle model (no diagnostic),
and the tileset generator's own zero-unsupported invariant catches unintended
falls at authoring time (strongest-form defence, per the debug doctrine).

The settle pass is followed by a **water-flood pass** (task #45), the fluid peer of
gravity settling. Free `minecraft:water` cells **and every `waterlogged=true`
block** (task #78) seed a deterministic, **conservative superset** of vanilla flow (mirroring spec-0010's never-overestimate-walkability
stance): (1) infinite-water source formation — a supported air cell flanked by ≥2
source cells becomes a source, cascading, so a walled pool basin fills completely,
not just 7 cells from its seeds; (2) 7-level horizontal decay from the completed
source set plus infinite downward flow. Vanilla's drop-seeking *direction* rule is
omitted (spread goes every way), which only over-marks. Every flooded cell (any
water level, plus sources) is **impassable and never standable floor** for every
consumer — nav, wave seating, relight fixture placement, waypoint export — the same
single-model discipline as settle. This closes the water analogue of the gravity
divergence: a `cave-shore` pool floods `[261,66,1]`, a cell an unpatched model
routed a talk-to leg's step-up through.

**Waterlogging is water** (task #78). Since MC 1.13 a `waterlogged=true` block's
cell holds a genuine water *source* that ticks and spreads into adjacent air
exactly like a free source (place one waterlogged stair on dry land and water
flows around it). The pre-#78 model stored bare ids and asserted the opposite
("waterlogging never spreads to a neighbour"), which **under-marked** the flood —
the one direction the never-under-mark contract forbids, since an under-marked
cell ships as proven-dry and strands the bot. A waterlogged cell is now both a
flood source *and* its host block's normal collision class: nothing walks or flows
*into* it, and `flooded` stays disjoint from every block class (it means "a walker
would be in open water here").

Trap triggers (spec-0011): `*_pressure_plate`, `tripwire`, and `tripwire_hook`
(`crate::assembled::is_passable_trap_trigger`) are non-collidable in game, so the
occupancy model treats their cells as **passable** rather than solid. This is the
faithful model — a plate rests on a solid support block below, so standability is
unchanged — and it is load-bearing for the `DW0342` trap proof: a player must be
routed *onto* a trigger cell (so the compiler can prove the trap avoidable or not),
never around a phantom "solid" plate that would call every trap avoidable.

**Collision classes (task #59, `crate::assembled::Occupancy`).** The occupancy is
no longer a single every-non-air-block-is-a-1×1×1-cube solid set; cells are
classified:

| Class | Blocks | Walk through? | Stand on top? |
|---|---|---|---|
| solid | every other non-air block (full-cube, the conservative default) | no | yes |
| tall barrier | `*_fence` (incl. `nether_brick_fence`), `*_wall` — 1.5-tall | no | **no** |
| use-gate | closed `*_fence_gate` (1.5-tall, right-click-openable) | player: yes (USE); autonomous mobs: no | **no** |
| passable | open `*_fence_gate` (block state `open=true`, read from the prefab palette), trap triggers, thin decoration (< 8/16 collision) | yes | no |
| flooded | water reach | no | no |
| partial | a solid cell's true top-face height in sixteenths, when < 16 | no | yes, **at that height** |

**Partial floor heights (task #78).** `crate::assembled::collision_top_16` reports
a block's collision-box top face in sixteenths, against the 1.21.11 shapes:

| Block | Height | Note |
|---|---|---|
| `*_slab` `type=bottom` (**the default state**) | 8/16 | a half-step |
| `*_slab` `type=top` / `type=double` | 16/16 | the face is the cell top |
| `snow[layers=N]` | `(N-1)·2 / 16` | `layers=1` (the default) has **no** collision box; `layers=8` is 14/16 |
| `*_carpet`, `moss_carpet` | 1/16 | `pale_moss_carpet` only when `bottom=true`, else 0 |
| `dirt_path`, `farmland` | 15/16 | |
| everything else | 16/16 | the conservative default |

A block under 8/16 is **thin decoration**: its cell is passable and is never a
floor level of its own (a walker stands on whatever is below it, and a 2-high
corridor with a carpet in it stays walkable). At 8/16–15/16 the cell blocks
passage but its walkable face is recorded in `Occupancy::partial`, which is what
makes the nav step rule physical rather than cell-counting — see below.

Modelled **precisely**: fences, walls, fence gates (open vs closed), trap
triggers, thin decoration, water and waterlogging, and partial floor heights.
Modelled **conservatively** — treated as a full solid cube, never as
walkable-through: stairs, doors, trapdoors, and every other partial-collision
block. The tall/gate classes close the owner-hit soundness hole:
the full-solid model proved the island pen leg by standing the player ON TOP of a
1.5-tall `oak_fence` (a "legal" +1 step no vanilla player or bot can perform —
harness #110 worked around it by filtering fence-lip waypoints), and a gateless
fence ring would have PASSED the completability proof while being humanly
impassable (now `DW0311`). A tall/gate cell is never valid floor, which also
models the barrier's upper half blocking same-level walk-overs for free. Closed
fence gates are **use-gate** edges: walkable for the player (adventure-legal
right-click, the same action a human performs), exported first-class per leg (see
`use_gates` above) — and walkable for scripted `move-npc`/`move-actor` tp
polylines, whose firing beat's fiction controls the gate (the island ram walks
out through the pen gate the player just opened — through the threshold, no
longer teleport-hopping the fence-top). Autonomous placement (`spawn-wave`
seating) uses the no-gate-use view (`World::without_gate_use`): a spawned mob is
never seated in a gate threshold and the seating flood never spills through a
closed gate. Cutscene dolly clipping (`DW0308`) treats fence, wall, and gate
cells as solids — they contain visible geometry. Water flow is unaffected:
vanilla water flows only into air, so every non-air block (fences and gates
included) dams the flood exactly as before.

### Entity placement: cells are centred, blocks are not

Every entity the compiler **summons or teleports** is positioned at
`nav::cell_center(cell)` = `(x + 0.5, y, z + 0.5)` — the horizontal centre of its
proven-walkable cell. Block-targeting commands (`setblock`, `fill`, `place`,
`spawnpoint`) keep the bare integer cell, which is the coordinate space they take.

The distinction is load-bearing. A block cell `(x, y, z)` spans `[x, x+1)`, but an
*entity's* position is the centre of its AABB, so summoning at the bare integer
coordinate parks the body on the corner where four columns meet: a 0.6-wide villager
at `x = 7.0` occupies `[6.7, 7.3]`, i.e. **70 % of it inside column 6**. Against a
wall that is an NPC standing in the wall; along a walked path it was the owner's
"the NPC visibly passes through blocks" island finding — measured at **234 of 385**
waypoints on the beach→cave `move-npc` leg with the body AABB inside a solid, now 0.
Nav itself was never wrong (A* is strictly cardinal — `neighbors_fp` offers four
horizontal moves and no diagonal transition exists, so corner-cutting is structurally
impossible); the defect was entirely in cell→position conversion at emission.

Applies to: NPC bodies + interaction hitboxes, `spawn-npc` entrances, actor puppets,
wave mobs, `interact` hitboxes and `interact`/`reach` wayfinding markers,
environment-trigger and trap-disarm interactions, and every `move-npc`/`move-actor`
waypoint. Cutscene dolly cameras already used centred coordinates
(`nav::camera_points`). **Player** teleports keep integer coordinates: vanilla
resolves player-vs-block overlap by pushing out, and the `dw:cp` mirror is a
documented int-triple contract (spec-0013).

**Vertical steps interpolate as an L, not a diagonal.** A one-block step up rises
over the source column first, then crosses at the new height; a step down crosses at
the source height, then drops. A straight lerp between the two cell centres would
drag the body through the corner of the step block — the stair-shaped instance of the
same artifact. Both legs of the L stay inside cells `standable_fp` + the jump
head-clearance rule already proved clear.

### Nav (compile-time, over the assembled voxel grid)

`move-npc` paths and the critical path are routed by A* over the placed-world
block data (obstacles per the collision classes above — full-cube solids, 1.5-tall
fence/wall barriers, closed fence gates for walkers that cannot use them;
**water-flooded cells are impassable and are never valid floor**; compiler gate
regions are passable). Steps are cardinal, one cell up or down.

**The step rule is physical, not cell-adjacency (task #78).** Each standing cell
has a true **feet height** in sixteenths — the cell below's `partial` face height,
so standing on a bottom slab is `y - 0.5`, not `y` — and a candidate step is gated
on the **rise** between the two feet heights:

| Rise | Verdict |
|---|---|
| ≤ 9/16 (0.5625) | a walk-up. Vanilla `maxUpStep` is 0.6, so no jump — and therefore **no headroom** is required above the source cell |
| ≤ 20/16 (1.25) | a jump; the swept head cell above the source feet must be clear or the entity head-bonks |
| > 20/16 | **impossible** — a vanilla jump apex is ≈1.2522 blocks, so 1.3125 is unreachable |

This corrects the rule in both directions. It **rejects** what the old full-cube
model proved: stepping off a bottom slab (feet at `y+0.5`) onto a ledge whose face
is at `y+2` is a **1.5-block** rise, which the old rule read as an ordinary "+1
cell" step and certified as a walkable route no player or mineflayer bot can
perform. It **admits** what the old model refused: stepping from a full floor onto
a bottom slab is a 0.5-block auto-step, legal even directly under a ceiling that
would block a jump. Vertical candidates stay `{0, −1, +1}` cells — a `+2`-cell hop
between two very thin floors can be physically legal, but omitting it only ever
refuses a route, never proves one.

Cutscene dollies must pass only non-solid cells; the clip test is an **exact 3-D
grid walk** (Amanatides–Woo DDA) visiting every cell each segment intersects, with
no error term — it replaced a ≤0.25-block sampler that could miss a cell a shot
only grazed through a corner and certify the shot clear (task #78).
Unroutable/clipping/stranded → `DW0307`/`DW0308`/`DW0311` at build (never a
runtime glitch).

**Close-gate solidity (v0.6, DAG-causal).** The base occupancy model treats every
gate region as **passable** (the conservative "assume the gate the player needs is
opened" stance `DW0306` separately proves at the piece-connectivity level) — so
`open-gate` does not dynamically flip cells at nav time, and an `open-gate`-only
campaign routes exactly as before. `close-gate` is the physical dual: the compiler
collects every `open`/`close` firing with its firing objective's critical-path step
(`plan.gate_events`, content-ordered, **deep-walked through `sequence`/lifecycle
bundles** via the shared `visit_deep` authority — a gate nested in a timeline is
collected exactly like a top-level one), and each walked critical leg — and each
checkpoint→forward-path leg — is routed with a gate forced **solid** iff its
causally-latest firing **among the leg-objective's DAG ancestors** is a `close` (not
reopened by a later `open-gate`). The ordering is **DAG-causal, not linear**
(`plan.strict_ancestor_steps` / `Plan::gate_fired_before`): a gate only seals a leg
whose objective is a true causal descendant of the gate's firing objective, so a
gate on a **parallel quest branch** the lineariser merely interleaves ahead of a leg
does not falsely seal it (island `take-the-cheese` flee legs are not sealed by the
`hide` branch's boulder). The seal is applied only to a **causal leg** (its start
objective is itself a DAG ancestor of the arrival) — the lineariser concatenates
sibling branches into artifact "legs" the player never walks under the arrival's
gate state, and base `DW0311` already proved every leg walkable in the open world. A
genuinely-forced re-crossing (a causal leg whose sealed gate is never reopened
before it) still fails `DW0311` (`DW0315` from a checkpoint) with a message naming
the sealed gate — the "point of no return by geometry" the owner's staging vision
wants, provable at compile time.

**One leg model for every consumer (task #78).** The per-leg seal
(`nav::leg_seal`) and the routing that uses it (`nav::route_walked_legs`) are now
the single definition shared by the completability proof, the forced-cell set the
`DW0342` trap proof reasons about (`World::required_path_cells`), and the exported
harness waypoints (`nav::critical_path_routes`). Previously only the proof ran
under seals while the other two routed the fully-open world, so the compiler could
(a) hand the bot a route through a gate the campaign had already sealed and (b)
call a lethal trap "avoidable" when the player only walks its detour *because* a
`close-gate` shut the direct route. A trap's disarm-reachability search likewise
runs under the gate state of the earliest leg that crosses the trap cell, not the
fully-open world.

**Talk-to endpoint (task #45):** a talk-to leg's target anchor is the NPC's own
occupied cell (the mannequin stands there, its interaction hitbox fills it). The
leg's goal is snapped to the nearest standable cell *beside* the NPC — excluding
the NPC's own cell and any flooded cell — so a shore NPC resolves onto dry footing
within interaction range, never onto the mannequin or a water tongue.

**Deferred-NPC staging order (`DW0197`/`DW0198`, scope note).** The ordering proof
is the stage-4 `depends_on` closure (the same machinery `DW0195` uses), taken at DSL
validation tier — not the compiler's `plan.strict_ancestor_steps`. It therefore
proves only the **decidable** half: a `talk-to` whose every `spawn-npc` sits in a
strict DAG *descendant* quest is `DW0198`. Not proven, deliberately: a `spawn-npc`
fired from an environment trigger, a dialogue option, or the talk-to's own quest —
none of which has a position on the quest DAG — so those suppress the check rather
than risk a false positive on legitimate staging. `DW0197` (never spawned at all) is
total and covers the common defect.

**Waypoint self-check (`DW0314`, task #45):** after routing, every exported
critical-path waypoint is re-asserted standable in the FINAL world (settled +
water-flooded + relight fixtures). Since the routes come from A* over that same
world, this can only fire if a later pass mutates a cell nav relied on or an
endpoint resolves off the walkable set — making it structurally impossible to ship
a waypoint the game floods or walls (the water-flow / post-nav-mutation divergence
class), a loud build failure instead of a runtime strand.

### The map editor edit stage (spec-0017, v0.6)

`crate::edit` replays the optional stage-7 script over the assembled model
(§1 pass 8) so **every** downstream consumer — relight, nav, wave seating,
waypoint/POV export, `snapshot`/`blocking-chart`, emission — sees the edited
world. Invariants:

- **Per-batch re-proofs.** After each batch the replay re-settles gravity
  (`DW0313` on a despawn, batch-attributed), re-runs the spec-0010 relight
  (`DW0210`/`DW0211`), re-proves critical-path + checkpoint walkability
  (`DW0311`/`DW0315`/`DW0316`, with the relight fixtures solid), and runs the
  **boundary-safety** check (`DW0322`, `nav::verify_boundary_safety`, stated
  per **horizon** — see the `DW0322` catalog row and *Boundary safety and the
  world-generator ambient* below). This is the guarantee the greenfield berm
  provided physically, made checkable so an edit script may reshape a boundary
  into natural landform. Reused codes keep their tiers; failures are prefixed
  `after world-edits batch `<id>``, and every violation of a run is aggregated
  into one report (bounded listing + total), never just the first.
- **Trap-hardware integrity (`DW0352`).** No batch write may land on a trap's
  trigger/hazard cell, dispenser socket or disarm-affordance cell. `setup_finish`
  runs `world_edits` **before** `trap_setup`, so a colliding edit lands first and
  the trap is then wired into a block that is gone — vanilla's `item replace
  block … container.0` on a non-container fails with **no output**, shipping a
  dead trap while every geometry proof stays green (`DW0342` proves the *planned*
  hazard, not the surviving hardware). Structural, so it is checked first, before
  the geometry re-proofs.
- **Support validity (`DW0354`).** Every support-dependent block the script has
  placed (torch/lantern/campfire/rail family; flora) is re-checked at each batch
  close against the current world: a later batch that carved its support away, or
  a `scatter` that dropped flowers onto a non-soil block, leaves a block vanilla
  pops off as an item on the first chunk tick — the edit silently undone while
  every snapshot still shows it. **Advisory** for decoration (aggregated per
  reason + block, with a count and one example cell); **error** when the popped
  block is a fixture the script's own `relight` verb placed, since that is a
  declared `min_light` guarantee the `DW0211` proof accepted. Conservative by
  construction: a block whose support is sideways or above (`wall_torch`, a
  `hanging=true` lantern) is classified as needing none, and "support removed"
  means removed to **air** — the check never guesses about a block it cannot
  classify.
- **Boundary safety and the world-generator ambient (`DW0322`).** The check's
  premise is what a column the compiler modelled *nothing* into actually holds in
  the delivered world — a property of the level generator (`nav::Ambient`,
  spec-0013 `horizon`), not of the content. It rides on `nav::World`
  (`World::with_ambient`, set from the plan by `World::from_plan` and by the edit
  replay) and is read **only** by this proof: it never feeds the walkability
  sets, so routing, standability and every other proof stay byte-identical.
  - **`Ambient::Void`** (`horizon: void`, the default and every pre-0.6 campaign)
    — unchanged: bottomless columns are the hazard, exactly as before.
  - **`Ambient::Ocean`** (`horizon: ocean`) — the ambient is the pinned superflat
    (`plan::SEA_LEVEL` = 62 water top, `plan::SEA_FLOOR_TOP_Y` = 54 sea floor,
    bedrock below), present in every column **except** inside a placed piece's
    AABB (`/place template` writes the whole box, air included; the water *under*
    an island base is still ambient). Bedrock everywhere ⇒ the void premise is
    vacuous, and the real hazard is **stranding**, modelled as:
    1. **Entering.** A reachable walkable cell puts the player in the sea when a
       horizontally adjacent column is enterable at its level (feet + head clear
       of solids and 1.5-tall barriers — water does *not* block walking in) and
       that column is open, between that level and the sea surface, all the way
       to ambient water. Walking in, wading in and falling off a cliff are the
       same outcome: vanilla buoyancy leaves the player afloat at `sea_level`.
    2. **The sea.** A cell at `y == sea_level` is swimmable when it is neither
       solid nor tall and is either ambient water or *authored* water (a lagoon
       at sea level is physically the same plane). Swimmable cells 4-connect into
       **bodies**; a body reaching the edge of the search window (the placed
       geometry inflated by `nav::OPEN_SEA_MARGIN`) is the open sea, and all such
       bodies are one, since the ring beyond the window is untouched ambient
       water in every direction. Connectivity is taken on the surface plane only
       — a diver might swim under a land bridge into another body, which the
       model deliberately does not count on.
    3. **Climbing out.** A body is escapable when one of its surface cells is
       horizontally adjacent to a **proven reachable walkable** cell whose feet
       are at `sea_level` (a rim one block under the waterline: wade out of the
       shallows) or `sea_level + 1` (the canonical beach — land flush with the
       surface; this is the island tileset's own convention, waterline local y=2
       / walk plane local y=3). A lip two blocks above the surface is a wall to a
       swimmer, and adventure mode has neither boat nor blocks.

    A body the player can enter and cannot climb out of is the violation. The
    granularity is **per body**: an island with a perfect outer beach still fails
    on an inner pool with 2-high walls, which a global "is there a climb-out
    anywhere" test would pass. Requiring the climb-out cell to be in the
    *reachable* walk region is what makes it a return, not just a landing.
- **Gate-region collision (`DW0353`, advisory).** A write inside a `close-gate`
  region is filled solid when the gate closes and cleared to **air** when it
  opens, so one cycle erases it. The proofs stay sound (the occupancy model
  already treats the region as gate-controlled), and dressing the *sealed* state
  is a legitimate intent — hence a warning, one per colliding gate region.
- **Determinism (ADR-0006).** Edit noise is position-addressed value noise
  (the island/cave generators' primitive family, ported into `crate::edit`)
  seeded per script position; the double-build gate covers the edited fixture
  (`tests/edit.rs`).
- **View mode.** `snapshot`/`blocking-chart` replay the script **without**
  enforcing invariants (`edit::replay_view`) — a broken state must be
  viewable; only region-resolution failures (`DW0323`) stop a view.
- **The loop** (`delvec edit apply|preview`, §7): full validation → replay
  with invariants → one labelled snapshot + manifest per batch (framing the
  batch's edited AABB over the final edited world) → **the whole build-tier
  proof set**. `apply --batch` appends a candidate batch and persists
  `world-edits.json` (canonical form) only when all of that is green; `preview`
  never writes to the campaign dir. A red candidate can never leave a broken
  script behind.
- **One proof tier, not two.** The per-batch invariants are a *subset* of what
  `build` proves — they miss cutscene clipping (`DW0308`), stealth zones
  (`DW0327`), trap completability (`DW0342`), wave seating (`DW0312`),
  `move-npc`/`move-actor` routability, and the exported-route/POV self-checks.
  `edit` therefore runs `analyze` + `emit::build` (output discarded) before
  persisting, so a script `apply` accepts is a script `build` accepts. Measured
  cost: ~0.3 s on the largest content campaign, against ~0.34 s for the snapshot
  render the same command already does — a cheaper tier has no reason to exist.
- **Atomic persist.** `world-edits.json` is written to a sibling `.tmp` and
  renamed into place: the artifact of record (ADR-0006) is never left truncated
  by a crash or a full disk.
- **Forceload lifecycle.** `setup` forceloads every piece bbox *and* every edit
  AABB. Each edit chunk that no piece bbox covers gets its own convergence
  sentinel in `place_verify` (`execute if loaded <cell>` folded into `#placeok`),
  so `setup_finish` — and therefore the one-shot `world_edits` — cannot run into
  a still-loading chunk and lose those writes forever; the tick retry loop
  converges on them exactly as it does on piece placement. Those same chunks are
  then released (`forceload remove`) at the very **end** of `setup_finish`, after
  every other write in the function. **Piece forceloads are never released** —
  the gameplay tick machinery (gate fills, wave spawns, checkpoint and trap block
  reads) keeps addressing those chunks for the whole session.

---

## 5. Diagnostics catalog (complete, as of current `main`)

Every DW code in `crates/**/*.rs`. Grouped by range. `tools/check-dw-codes.py`
verifies this catalog is bidirectionally exact against source (CI docs job).

**Test-coverage gated** (owner, 2026-07-31; CLAUDE.md Conventions). The same
script also fails CI if any documented, landed code has no test asserting it —
either the literal code string or a symbolic diagnostic-code constant (e.g.
`DW_STRIP`) that resolves to it, scoped per crate to avoid cross-crate name
collisions (`DW_INPUT` names a different code in `delve-schem`, `delve-render`,
and `delve-admit`) — appearing in `crates/<crate>/tests/**/*.rs` or a
`#[cfg(test)]` module in `crates/<crate>/src/**/*.rs`. A code that is
genuinely unreachable without external resources (e.g. `DW0720`, which needs a
GPU adapter + the never-committed 1.21.11 client jar) may be declared in the
script's `ALLOWLIST` with a one-line justification — kept minimal; writing the
test is always preferred.

**Remediation contract (task #39).** Every DW message is the repair protocol for a
zero-context author: it states **what** is wrong (with the offending name/coord/
count/limit interpolated), **where** to fix it (the campaign stage/field, the
prefab/tileset, or — for an invariant breach — "compiler bug, escalate"), and
**how** to fix it; where a tempting wrong fix exists (weaken a threshold, reroll
the `seed` against ADR-0006, widen a socket seam, bypass the allowlist) the
message names it with an explicit "do NOT". The rows below summarize each code's
*meaning*; the emitted message additionally carries the prescription. Gold
standards: `DW0312`, `DW0210`/`DW0211`, `DW0304`, `DW0306`.

### DW01xx — validation (`dsl`; severity error; exit 1)

| Code | Meaning |
|------|---------|
| `DW0100` | Document does not conform to its stage schema (unknown field / wrong type / missing required field, incl. persona). Parse-time. |
| `DW0101` | `stage` field ≠ document slot. |
| `DW0102` | Unsupported `dsl_version` (not in `{0.2.0,0.3.0,0.4.0,0.5.0,0.6.0}`). |
| `DW0103` | `campaign_id` differs across stages. |
| `DW0110` | Malformed id syntax (not kebab-case / wrong-missing prefix). |
| `DW0111` | Duplicate id in namespace (incl. two dialogue trees for one NPC). |
| `DW0112` | Dangling / forward / undeclared reference (incl. persona relationship to unknown NPC). |
| `DW0120` | Dialogue node unreachable from `root`. |
| `DW0121` | Dialogue `root`/`next` references unknown node. |
| `DW0122` | Dialogue effect targets an objective unknown / not `talk-to` / on a different NPC. |
| `DW0123` | A `talk-to` has no reachable completing option in its tree (static half of `DW0203`). |
| `DW0130` | Quest `depends_on` cycle. |
| `DW0131` | `finale` is not a declared quest. |
| `DW0132` | `finale` is not the convergent sink (some quest is not a transitive dependency of finale). |
| `DW0133` | Non-mandatory quest (`mandatory:false`), reserved until M3. |
| `DW0140` | Objective `after` cycle. |
| `DW0141` | Reserved enum value/field for the campaign's `dsl_version` (npc `vendor`/`boss`; under 0.2.0 the v0.3 verbs/effects; under pre-0.4 the v0.4 surface; under pre-0.5 the v0.5 surface: `time`/`weather`/`lighting`, `set-time`/`set-weather`; under pre-0.6 the v0.6 surface: area `mitigation`, `close-gate`, `damage-players`, `set-checkpoint`, `begin-stealth`/`end-stealth`, `horizon`/`boundary`, the `play-sound` effect + `narrate` `style: art`, per-effect `requires_flags`, stage-2 npc `deferred` + the `spawn-npc` effect, stage-5 `actors` + `spawn`/`despawn`/`move`/`unleash-actor`, `sequence`, the `traps[]` section, the `bonfire` effect, wave `respawns_on_rest`, and the `shortcuts[]` / `ambushes[]` / `timed_gates[]` sections). |
| `DW0142` | Anchor not provided by the area's bound prefab. |
| `DW0143` | Item id not in the pinned 1.21.11 registry (kit / `collect` / `interact.requires_item` / `give-item`). |
| `DW0150` | Planned quest (stage 4) has no stage-5 expansion. |
| `DW0151` | Stage-5 quest not planned in stage 4. |
| `DW0152` | Stage-2 NPC has no stage-6 tree. |
| `DW0153` | Stage-6 tree references an NPC not in stage 2. |
| `DW0160` | Area binds neither or both of `prefab`/`prefab_pool`. |
| `DW0161` | `prefab_pool` references a pool absent from `prefabs/` metadata. |
| `DW0162` | Stage-7 edit script structurally invalid (v0.6, spec-0017): an edit names a region no earlier `select` in its batch defined (region refs are strictly backward within a batch), a `union`/`intersect` lists < 2 regions / a `subtract` removes nothing, a box `min` > `max` on an axis, a surface band `from` > `to`, a palette recipe is empty or carries a non-positive/non-finite `weight`/`scale`, a `matching` list is empty, or a morph `by`/`passes` is 0. (Unknown recipe/matching block ids reuse `DW0193`; id syntax `DW0110`; duplicate batch/region names `DW0111`; a `world-edits` doc under a pre-0.6 `dsl_version` `DW0141`.) |
| `DW0170` | `kill`/`spawn-wave` references an undeclared `wave/<id>`. |
| `DW0171` | A killed wave is never spawned by any `spawn-wave`. |
| `DW0172` | `requires_flags` references a flag no `set-flag` produces. The producer scan descends every nested effect list (`sequence` steps, `on_respawn`/`on_caught`/`on_arrive`), so a `set-flag` nested in a timeline still counts as a producer (no spurious fire). |
| `DW0173` | Wave-mob `entity` is not a known vanilla entity id. |
| `DW0180` | l10n sidecar absent / inconsistent envelope / under-covers inventory (also if `en` is declared). Compiler-level. |
| `DW0181` | l10n sidecar has an orphan key (over-coverage). Compiler-level. |
| `DW0182` | A player-visible string — authored English (the whole l10n inventory) or any sidecar translation — contains the reserved completion-marker sigil `[dw:complete`. That chat sequence is the validation bot's completion oracle (§4 "The completion-marker channel"); content carrying it could forge a passing critical-path step, so the sigil is **reserved**, not merely discouraged. Reword the line. |
| `DW0190` | Mannequin `skin.texture_id` malformed or duplicated. |
| `DW0191` | A `talk-to` has no **ungated** completing option (all `requires_flags`-gated → deadlock risk). |
| `DW0192` | Wave-mob `effects[].effect` not a known 1.21.11 status-effect id. |
| `DW0193` | `set-block`/`interact.prop` block id not a known 1.21.11 block id (base id checked; a malformed blockstate suffix `id[…]` — unbalanced `[]`, empty, or non-`key=value` tokens — reuses this code). |
| `DW0194` | Environment-trigger id malformed/duplicated, or `approach` `range` 0. |
| `DW0195` | A `talk-to` targets an NPC despawned by a prerequisite quest. |
| `DW0196` | Area `lighting.min_light` out of range (must be 1..=14). v0.5, spec-0010. |
| `DW0197` | A stage-2 NPC declares `deferred: true` but **no** `spawn-npc` effect anywhere (quest, trigger, nested timeline, or dialogue) summons it — the NPC never enters the world, so its dialogue tree and any `talk-to` on it are unreachable content. v0.6; the staging dual of `DW0195`. Prescription: add the `spawn-npc` at the entrance beat, or drop `deferred`. (0197/0198 were *reserved* by spec-0011's draft and released when it renumbered to `DW0340`/`DW0341`; no code ever emitted them.) |
| `DW0198` | A `talk-to` on a `deferred` NPC provably activates before the NPC exists: every `spawn-npc` for it fires in a quest that is a **strict DAG descendant** of the objective's quest. Conservative by construction — a spawn from a trigger, from dialogue, or from the objective's own quest is not DAG-ordered and suppresses the proof rather than risking a false positive (see the gap note below). v0.6. |
| `DW0199` | A `cutscene` effect's shape is invalid: it mixes the multi-shot `shots` list with the single-shot `path`/`seconds` fields, declares neither, omits `seconds` on a single shot, or gives a shot with an empty camera `path`. The two spellings normalize to one shot list, so this is where the shape is policed and emission may then assume a well-formed, non-empty list. v0.6. |
| `DW0320` | `horizon:"ocean"` declared without a `boundary` (an infinite swimmable sea with no return rule). v0.6, spec-0013. Numbered in the 032x world/region family but **validation-tier (exit 1)**, not a DW03x build code. |
| `DW0321` | `boundary.margin` outside `0..=64`. v0.6, spec-0013. Validation-tier (exit 1). |
| `DW0340` | Trap declaration structurally invalid (v0.6, spec-0011): a malformed/duplicate `trap/<id>`, an `at`/`disarm.via` that no area's prefab provides, or a `disarm.via` that collides with the trap's own trigger anchor. Renumbered off the spec's stale reserved number (0197). |
| `DW0341` | A trap `dispense` payload item id is not in the pinned 1.21.11 registry (v0.6, spec-0011; mirrors `DW0143`). Renumbered off the spec's stale reserved number (0198). |
| `DW0343` | A verb that needs a gate anchor's **fill block** targets an anchor that declares none (or is not a gate region at all): `close-gate` (v0.6), which fills the region back in, or a stage-5 `shortcut` (spec-0016 §2), whose unlock clears exactly that block and whose gate is sealed by it from world-load. Compiler-side (needs prefab metadata the DSL anchor registry does not carry), reported at **validation tier (exit 1)** like the atmos `DW032x` checks; scan is over every prefab (gate anchors resolve globally like `open-gate`), and **all** region-providers of the anchor must declare a `block`. Prescription: declare `block` on the gate anchor, or remove the `close-gate`. |
| `DW0348` | A `shot_style` declaration is semantically invalid (v0.6, spec-0015): a styled shot with no `subject`; style params (`subject`/`subject_b`/`dist`/`degrees`/`bearing`) on an unstyled shot; `subject_b` off `two-shot` (or a `two-shot` without one); `degrees` off `orbit-arc` or outside `45..=120`; `dist` outside `1..=48`; `bearing` outside `-360..=360`. Validation-tier (exit 1), `dsl::validate`. |
| `DW0349` | A `side-track`/`low-follow` shot whose subject provably cannot move: those styles dolly *with* a moving subject, so the subject must be an npc/actor with a matching `move-npc`/`move-actor` in the same effect group or the same `sequence` timeline (an `anchor` subject can never move; reaction lists `on_arrive`/`on_caught`/`on_respawn` start a fresh scope — their firing time is statically unknowable). Validation-tier (exit 1), `dsl::validate`. Prescription: add the move alongside the cutscene, or use a static style (`locked-off`, `push-in`). |
| `DW0356` | `world.min_players` outside `1..=4` (v0.6, spec-0018). A delve is played by ONE party of 1–4 (the product definition), so a declared mandatory size can never sit outside it. Absent = 1. Validation-tier (exit 1), `dsl::validate`. |
| `DW0357` | A `carrier: "one"` `give-item` sits in a bundle only the scheduler ever runs — a `sequence` step, or a `move-npc`/`move-actor` `on_arrive` (v0.6, spec-0018). Those run with the server command source and have no acting player, so the single prop would reach nobody. The walk **stops** at `set-checkpoint.on_respawn` / `begin-stealth.on_caught`: those are dispatched per player and do have an `@s`. Validation-tier (exit 1), `dsl::validate`. Prescription: drop `carrier` (arm the whole party), or move the hand-off onto the beat a player completes. |
| `DW0350` | A `use` trigger anchored where an NPC stands (round-6 island QA). Right-click on an NPC already belongs to its dialogue advancement; a second interaction hitbox in the same cell makes the client's entity ray-pick ambiguous, and whichever entity loses the tie is silently dead — the soft-lock class that starved the giant's dialogue of every right-click. Left-click triggers are exempt (a left-click has no dialogue meaning): they ride the NPC's own hitbox instead of summoning a second one. Validation-tier (exit 1), `dsl::validate`. Prescription: move the trigger to its own anchor, express the interaction as a dialogue option, or — if the NPC's body is genuinely the target — use `on: strike-npc`, which takes no anchor at all. |

| `DW0377` | A `timed-gate` declaration (spec-0016 §4) is structurally invalid: a malformed or duplicate `timed-gate/<id>`, an `open_ticks` or `closed_ticks` of 0 (a gate that never opens, or never closes — that is `open-gate`/`close-gate`, not a clock), a `phase` at or beyond the full cycle, two timed gates driving one region (two clocks race every tick and the region's state becomes emission order, not design), or a gate a `shortcut` already owns (a clock would re-seal what `DW0372` exists to forbid re-sealing). Validation-tier (exit 1), `dsl::validate`. |
| `DW0375` | An `ambush` declaration (spec-0016 §3) is structurally invalid: a malformed or duplicate `ambush/<id>`, an empty `actors` list (an ambush that springs nothing), or the same actor listed twice — `spawn-actor` is idempotent, so the second one is a silent no-op and the ambush is half the size it reads as. Validation-tier (exit 1), `dsl::validate`. Deliberately does **not** require a `telegraph`: the un-telegraphed ambush is core souls vocabulary (owner ruling 2026-08-02). Everything else about an ambush is checked as the trigger it desugars to (`DW0194`, the anchor seals, `DW0350`). |
| `DW0371` | A `shortcut` declaration (spec-0016 §2) does not resolve: a malformed or duplicate `shortcut/<id>`, a `gate`/`unlock` anchor no area's prefab provides, or an `unlock` equal to its own `gate` — the mechanism belongs on the far side of the door it opens, which is the entire point of the pattern. Validation-tier (exit 1), `dsl::validate`; anchor resolution stays lenient for pool areas the compiler resolves later, like the trap and trigger checks. |
| `DW0372` | A `close-gate` effect targets a gate a `shortcut` owns (spec-0016 §2). A shortcut opens **permanently** — that is the pattern — so permanence is made structural rather than left to authoring discipline: there is simply no way to spell the re-seal. The scan descends nested effect lists, so a `close-gate` buried in a `sequence` step is caught. `close-gate` on any other gate (the point-of-no-return staging beat) is untouched. Validation-tier (exit 1), `dsl::validate`. |
| `DW0381` | A wave's TD `lane` / `summon` declaration (spec-0016 §6) is structurally invalid or internally contradictory: an empty `waypoints` list, a waypoint anchor no area's prefab provides, a repeated consecutive waypoint (the squad would be sent where it already stands, and vanilla re-rolls a patrol target on arrival), an `aggro_radius` outside `4..=64`, a mob whose `attributes.follow_range` disagrees with `aggro_radius`, or `lane` together with `summon: aggro-edge`. The `follow_range` clause is the subtle one: release radius and perception radius must be the same number, because a patrolling raider that targets a player it cannot engage HOLDS GROUND instead of marching — the squad stalls mid-lane with every other proof green. Validation-tier (exit 1), `dsl::validate`; anchor resolution stays lenient for pool areas the compiler resolves later. |
| `DW0382` | A lane wave fields a non-raider species (spec-0016 §6). `Patrolling`/`patrol_target` are Raider NBT: on any other mob they are simply dropped and it stands where it spawned — the silent no-op class. Lane roster, all live-verified marching on 1.21.11: pillager, vindicator, evoker, ravager, witch. Validation-tier (exit 1). Prescription: use `summon: aggro-edge`, which needs no patrol AI, for everything else. |
| `DW0383` | A lane wave fields fewer than 2 mobs (spec-0016 §6). A lone patroller sets `Patrolling:0b` on ITSELF when it finds no companion within its follow range (vanilla, live-verified), so a one-mob lane cancels its own routing. Validation-tier (exit 1). |
| `DW0384` | A lane `pillager` is not holding a crossbow (spec-0016 §6). Its only attack goal is the crossbow goal, so on acquiring a target it has nothing runnable to do — while the patrol goal is meanwhile blocked BY that target — and it freezes in place indefinitely (live-verified deadlock). The compiler arms pillagers by default, so this fires only on an explicit `equipment.main_hand` override, which is exactly the remaining way into the deadlock. Validation-tier (exit 1). |
| `DW0385` | A `summon: aggro-edge` wave mob declares no `attributes.follow_range` (spec-0016 §6). That radius IS the summon ring — the distance at which the mob perceives the party — so it is authored, never guessed: the compiler will not fabricate a vanilla default it cannot verify against the pinned server. Validation-tier (exit 1). |
| `DW0370` | A wave declares `respawns_on_rest: true` but the campaign declares **no** `bonfire` (spec-0016 §1) — nothing can ever fire the re-seat, so the field is a silent no-op, the defect class this compiler always makes loud. Validation-tier (exit 1), `dsl::validate`; the scan descends every nested effect list (a `bonfire` inside a `sequence` step counts) over quests and triggers. Prescription: add the bonfire the re-seat hangs off, or drop the field — never leave a dead declaration in the DSL. |

### DW032x/033x — sound & art-title validation (`compiler::atmos`; error; exit 1)

v0.6 (spec-0014) content checks that need compiler-vendored data (the pinned
`sound_event` registry) or the `delve:art` font, run in the compiler's
`validate_stage` (so `validate`/`analyze`/`build` all catch them) and reported at
**validation tier (exit 1)** like the `DW01xx` codes — not build-tier. No-op for a
campaign that uses neither the `play-sound` effect nor the `narrate` `art` style.

**Nested-effect consumer recursion.** These scans (`sound_refs`/
`play_sound_actor_refs` → `DW0326`/`DW0335`, `art_narrates` → `DW0328`) descend
**every nested effect list** (`sequence` steps, `on_respawn`/`on_caught`/`on_arrive`
bundles) through the one `each_effect_ref` traversal, keyed by the same position
scheme as the l10n inventory — so a bad sound id / non-Latin art string buried in a
timeline is caught, not shipped unvalidated. The DSL-side effect-ref consumer scans
(`spawn-wave` → `DW0170`, `give-item`/`collect`/`requires_item` → `DW0143`,
`set-block`/`prop` → `DW0193`, `move-npc`/`despawn-npc` → `DW0112`, per-effect
`requires_flags` → `DW0172`) likewise recurse via `for_each_effect_deep` /
`for_each_trigger_effect_deep`. This matches how the flag/wave **producer** scans and
emission already descend (the class of bug fixed piecewise in #102/#104); top-level
paths/keys are unchanged, so a nesting-free campaign validates byte-for-byte
identically.

| Code | Meaning |
|------|---------|
| `DW0326` | A `play-sound.sound` (v0.6) or `narrate.sound` (v0.4) id is not a known 1.21.11 sound event (validated against the vendored `sound_event` registry, `crates/compiler/data/sounds-1.21.11.json`; `minecraft:` prefix optional). |
| `DW0328` | An `art`-styled `narrate` string — the English source **or** any declared-language sidecar translation — uses a character outside the `delve:art` font's glyph inventory (A–Z, 0–9, space, `! " ' ( ) , - . / : ; ?`; lowercase folds to uppercase). Forces per-language art titles to stay ASCII/Latin — a `zh-cn` art translation must be an ASCII rendition. |
| `DW0335` | A `play-sound` targets `at: {actor: …}`, accepted by the schema but not yet wired — the actors surface (spec-0014 `actors[]`) has not landed. Use `at: {anchor}` or `at: players`. (Graduates when the actors PR wires actor-position resolution.) |

#### The `delve:art` font

An original 5×7 pixel bitmap font authored in `compiler::atmos` (`ART_GLYPHS`), baked
into `resourcepack.zip` as `assets/delve/font/art.json` +
`assets/delve/textures/font/art.png` — and **only** when the campaign uses `style:
art`, so a non-art campaign's pack stays byte-identical. The PNG is written by a
hand-rolled deterministic encoder (stored DEFLATE, no compressor), like the pack's
hand-rolled ZIP/SHA-1.

| Constant | Value | Meaning |
|----------|-------|---------|
| `CELL` / `GW` / `GH` | 8 / 5 / 7 | atlas cell, glyph ink width, glyph ink height, in source px |
| `ART_SCALE` | **1** | the provider's source-pixel scale — the one knob for on-screen size |
| `ART_HEIGHT` / `ART_ASCENT` | 8 / 7 | provider `height` / `ascent` = `CELL·ART_SCALE` / `GH·ART_SCALE` |
| `ART_GLYPH_ADVANCE` | **6** | `GW·ART_SCALE + 1`, vanilla's `round(ink·height/cellHeight)+1` |
| `ART_SPACE_ADVANCE` | 4 | the `space` provider's advance, `4·ART_SCALE` |

**`ART_SCALE` must stay an integer** — the font atlas is sampled nearest-neighbour,
so a fractional scale splits a source pixel across screen pixels and the glyph edges
go ragged. It was **4** through v0.6, which is why art banners could not physically
fit: an art `narrate` renders in the vanilla **title** slot, so the provider scale
and the slot's ×4 pose scale multiply, and 21 font px/glyph against `DW0330`'s 90 px
budget left room for *four* glyphs. The island's `NOBODY` (126 px) and `HOMEWARD`
(168 px) ran off both edges on screen. Halving to 2 was not enough — 11 px/glyph fits
8, which `HOMEWARD` exactly exhausts — so `ART_SCALE` is **1**, the largest integer
scale that fits **15** glyphs. The title slot still draws it ×4, so an art title
remains a title-sized blocky all-caps banner; what changed is that it now occupies a
title's share of the screen instead of four times it. `ART_ASCENT = GH·ART_SCALE`
keeps the ink sitting exactly on the baseline at any scale.

The width model treats every glyph as a flat `ART_GLYPH_ADVANCE`. That is exact for
every letter and digit (all ink the full 5 columns) and deliberately **conservative**
for the few narrow punctuation glyphs (`'`, `(`, `!`), which vanilla advances less —
`DW0330` never under-measures a line.

### DW0330 — on-screen text fit (`compiler::textfit`; **warning**; exit 0)

An **advisory** code (with `DW0351`, one of the compiler's two). Vanilla draws a `title`, a `subtitle`
and an art title centred, on **one line, with no wrapping and no shrink-to-fit** —
text wider than the screen just runs off both edges, silently. `DW0330` measures
each on-screen `narrate` string's **rendered width in font pixels** and compares it
to the style's budget.

**Why measured, not counted.** `i` and `W` differ by 3× in the vanilla font, and a
Han glyph is 9 px against a Latin letter's 6 (1.5×, *not* the 2× a "CJK counts
double" rule assumes). A character count is unfair to whichever script it was not
tuned for, so the check sums real advances: the ASCII sheet's per-glyph widths, the
`unihex` full-width advance for CJK, and — for `art` — the `delve:art` font's own
glyph metrics, derived from the same constants that emit the font.

**Budget.** `Gui.renderTitle` renders a title at pose scale **×4** and a subtitle at
**×2**; an art title is a title, so it takes ×4 on top of the `delve:art` provider's
own scale (see [The `delve:art` font](#the-delveart-font)). Against a reference GUI
width of **426** scaled px (what Minecraft's auto GUI scale yields at 1280×720 and
2560×1440; 1920×1080 gives 480, and 320 is the auto floor) at **85%** usable width,
the budgets are **90** font px for `title` and `art` and **181** for `subtitle`.
`chat` has no budget — it wraps and scrolls. At the art font's 6 px/glyph that is
**15 art glyphs**; the lint reads the same `ART_GLYPH_ADVANCE` / `ART_SPACE_ADVANCE`
constants the font emission does, so the two cannot drift.

**Why warning, not error.** The true limit is a property of the player's window and
GUI scale, which the compiler cannot know; rejecting on it would dress a judgement
call as a fact, and would hard-block a translation for being honestly longer than its
English source. It reports, and the author shortens.

**Scope.** The canonical English source **and** every declared-language sidecar
rendition, walked by the same `each_effect_ref` traversal and l10n keying as
`DW0326`/`DW0328` — so a sidecar finding is reported at
`l10n/<lang>.json#/content/<key>`, naming the exact string to shorten. Nested
effects are covered.

| Code | Meaning |
|------|---------|
| `DW0330` | An on-screen `narrate` string (`title` / `subtitle` / `art`) — English source or any declared-language sidecar rendition — renders wider than fits on screen. Advisory (exit 0): shorten the line. Do **not** demote a title to `chat` to silence it, and do not assume a wider monitor fixes it — the overflow scales with GUI scale, not away from it. |

### DW0379/DW0380 — souls pacing lints (`compiler::nav`; **warning**; exit 0)

spec-0016 §7 is the design-contract section, and both of its rules are things the
compiler can **measure** but must not overrule — so both warn and neither ever
fails a build. Computed over the same assembled nav model as the completability
proofs, so their numbers are the numbers every other proof uses.

| Code | Meaning |
| --- | --- |
| `DW0379` | **Retry cost**: the proven walk from a rest point (`set-checkpoint` or `bonfire`) to the first beat it can respawn the party into exceeds **60 s** (1200 ticks at the 4 t/block sprint model `DW0355` uses). Dying must be an investment, not a commute — past the budget the loop stops teaching and starts taxing. Measured to the FIRST beat after the rest point, not the last. A warning because a long walk back can be the authored point (a pilgrimage, a set-piece approach). **Known limitation** (planner escalation, 2026-08-02): at 60 s the budget is ~300 blocks of walking, which no box-garden delve currently approaches, so the lint is effectively inert in practice. It is implemented to the spec's threshold rather than retuned — changing the number is an owner decision, not a compiler one. |
| `DW0380` | **Optional-elite bypass**: an enemy no `kill` objective requires has no route around it — with its aggro radius (declared `follow_range`, else vanilla's 16) forced solid, a forced critical-path leg that routed before no longer does. Every way forward runs through the fight, so "optional" is a lie. The Tree Sentinel pattern — a powerful optional enemy near the start, fight it or walk around it — is explicitly legitimate (owner ruling 2026-08-02); this is its one obligation. Two deliberate exclusions keep it about the ROUTE: a leg with an endpoint **inside** the sphere is contested ground by design (the landed "live threat" pattern, a wave seated on an objective anchor), and a leg that never routed in the clean world belongs to `DW0311`. |

### DW0351 — NPC location-continuity lint (`compiler::continuity`; **warning**; exit 0)

Tracks each NPC's **staged location history** through the campaign timeline —
the stage-2 anchor (or off-stage while `deferred`), every `move-npc`
destination, every `despawn-npc`/`spawn-npc` pair (a `spawn-npc` always places
at the NPC's declared anchor) — and warns when an NPC materializes or vanishes
at a location discontinuous with where it was last staged, with no movement in
between (owner, island QA round 6: `npc/perimedes` popped into an alcove
mid-story having never been staged entering; `npc/antiphos` was "grabbed at the
cave mouth" while his body vanished at the beach camp).

Three shapes warn:

* **re-entry jump** — `spawn-npc` re-materializes an NPC at its declared anchor
  after it was last staged elsewhere;
* **unstaged entrance** — a never-yet-staged deferred NPC materializes
  mid-story with no staged arrival. The accepted staging shape is firing the
  `spawn-npc` from a `move-actor`/`move-npc` `on_arrive` whose destination IS
  the NPC's anchor (walk a stand-in to the spot, swap the npc in on arrival);
* **remote dismissal** — `despawn-npc` fires from a beat whose scene anchor
  (the completing objective's anchor; a `talk-to`'s scene is its target NPC's
  staged spot) differs from where the NPC's body stands.

**Conservative model (no temporal reasoning).** Locations are symbolic anchor
names (same name = same place; no geometry). The timeline is the quest-DAG
linearization (stage-4 `depends_on` topo order; objectives in `after` order;
bundles in declared order, descending into `sequence` steps and `on_arrive`
lists in place). Anything whose firing time is statically unknowable makes the
NPC **untracked** instead of guessed at: a lifecycle effect fired from an
environment trigger, a dialogue option, an `on_respawn`/`on_caught` reaction
bundle, or carrying a `requires_flags`/`forbids_flags` gate excludes that NPC
from the lint entirely.

**Why warning, not error.** Whether a jump reads as broken is authorial taste —
narrative cover ("he slipped away while you slept") legitimizes any of these.
The message names the discontinuity concretely and prescribes the remedy
(stage a walk / spawn at the last staged location / accept with narrative
cover); the author decides.

| Code | Meaning |
|------|---------|
| `DW0351` | An NPC materializes (`spawn-npc`) or vanishes (`despawn-npc`) at a location discontinuous with its last staged location, with no movement in between. Advisory (exit 0): stage the move, re-anchor, or accept with narrative cover. |
| `DW0353` | A world-edits batch writes inside a `close-gate` region (v0.6, spec-0017). The gameplay seal fills that region solid and `open-gate` clears it to **air**, so one close/open cycle erases the edit — the dressing `delvec snapshot` shows is not what players see after the beat fires. Advisory (exit 0), `compiler::edit`, one finding per colliding gate region: dressing the *sealed* state is a legitimate authorial intent, so this reports rather than rejects. |

### DW02xx — analysis (`compiler::analyze` reachability + `compiler::light` lighting; error; exit 2)

`DW0210`/`DW0211` are emitted by the assembled-world light model
(`crate::light`), surfaced through the build path but mapped to exit 2 (analysis
tier) in `main`; `DW0201`–`DW0204` come from `compiler::analyze` over the
branch-coherent flow model (`compiler::flow`).

**The emitter table never overestimates (`crate::light::emission`).** Both gates
are only sound if the modelled light is a *lower* bound on the game's — a block
modelled brighter than vanilla lets a genuinely dark area ship unmitigated. The
table is evaluated over each block's **actual blockstate** (the assembled map
carries full states) against verified 1.21.11 values, with a source cited per
entry in the code. Blocks absent from the table emit 0 (an underestimate, the safe
direction). Task #78 corrected seven entries that broke the contract by collapsing
a state-dependent block onto its brightest state, or on plain wrong values:
`sea_pickle` 15 → `3 + 3·pickles` when waterlogged and **0 when dry**;
`redstone_ore` 7 → **0** idle (9 when `lit`); `respawn_anchor` 7 → **0** at
`charges=0`; `amethyst_cluster` 7 → 5 (buds 4/2/1); `brewing_stand` and
`brown_mushroom` 3 → **1**; `glow_item_frame` 7 → **0** (it is an entity, not a
block, and emits no block light in Java — the 7 was a Bedrock value). A
nonexistent `minecraft:froglight` id was dropped, and the furnace family now
reports 13 when `lit`. Blocks whose `lit`/`charges`/`berries` state has a *bright*
default (campfire, soul campfire, redstone torch) still evaluate bright from a
bare id, so the compiler's own relight fixtures are unaffected.

**The opacity table is coupled to nav passability
(`crate::light::passes_light`).** The opacity side defaults the other way — an
unlisted block is **opaque**, which under-measures light and is the safe direction
for a block a walker cannot enter anyway. It is *not* safe for a block
`assembled::occupancy_of` deliberately leaves **passable**: then a cell the player
really stands in is measured at light 0 while the game lights it normally, and the
gate manufactures a `DW0210` no amount of relighting can clear. The invariant is
therefore:

> every block class whose cell `occupancy_of` leaves player-occupiable must be
> light-passing in `passes_light`.

Three classes are player-occupiable by construction — trap triggers
(`is_passable_trap_trigger`: `*_pressure_plate`, `tripwire`, `tripwire_hook`, kept
walkable *on purpose* so `DW0342` can reason about a player stepping onto a
critical-path trap), thin decoration (`is_thin_decoration`: every `*_carpet`, and
`snow` at 1–4 layers), and fence gates (`*_fence_gate`: open = a passable
threshold, closed = passable-with-use). All of them are `filterLight = 0` in
vanilla 1.21.11 and all of them now pass light; before this fix only
`oak_fence_gate` did, so any roofed prefab carrying a plate, a tripwire, a carpet
or a non-oak gate failed `DW0210` on cells that are in fact lit. Verified against
the pinned `minecraft-data` block table (`.../pc/1.21.9/blocks.json`):
`filterLight = 0` for all 16 pressure plates, `tripwire`, `tripwire_hook`, all 20
carpets, `snow`, and all 12 fence gates; `filterLight = 15` for the control cubes
`stone`/`dirt`/`oak_planks`/`cobblestone`/`deepslate`/`sand`/`gravel`/`obsidian`
and for `snow_block` (a full cube — deliberately still opaque). Blocks vanilla also
calls transparent but that `occupancy_of` classifies **solid or tall** (fences,
walls, buttons, levers, rails, slabs, stairs, doors, trapdoors, chests, signs)
stay opaque here: their cells are never player-occupiable, so their opacity can
only make the gate stricter, never manufacture a false pass. The invariant is
CI-pinned by `light::tests::every_nav_passable_block_passes_light`, which drives
the real classifier — a future passability change that forgets the light table
fails there rather than in a campaign.

| Code | Meaning |
|------|---------|
| `DW0201` | Finale quest can never complete (unreachable finale). |
| `DW0202` | Quest can never be triggered (dead quest — its trigger source never completes). |
| `DW0203` | Objective can never be completed **in any branch** (deadlock: unsatisfiable `after` chain, an unproducible `requires_flags` gate, or a `talk-to` completing option unreachable through the trigger/`after`/dialogue graph). |
| `DW0358` | A declared `min_players: n` (n ≥ 2) has **no n-agent division of labour** (v0.6, spec-0018). Completability is proven with `min_players` agents: n = 1 is the unchanged single-agent proof, and n ≥ 2 additionally requires the proven playthrough to contain an AND-join with n arms that are *independently reachable at the join's frontier* — the replay state just before its earliest arm — with no arm waiting on a sibling, a flag a sibling sets, or a quest that is not active yet (`flow::Flow::divide`). Names the widest join and how many arms it actually offers, or says the campaign has no AND-join at all. Reported on `world`/`/content/min_players`, exit 2. Prescription: split one beat into n `after`-arms completable from the same frontier, or lower `min_players`. |
| `DW0204` | The exported critical path is not a playthrough any player can walk: some step is not activatable/completable at its position, or `campaign-complete` fires before the final step (the signature of two mutually exclusive endings sharing one path). Names the first incoherent step. |
| `DW0210` | **Measured** (spec-0010): a reachable walkable cell of an area is below light 3, under the darkest reachable (time, weather) sky, with no `lighting` declaration and no `mitigation` declaration. Judged over the assembled world (per-seam, sealed-cavity aware — unreachable cavities are never counted). Admission `LightingProfile` is no longer a gating input. **v0.6:** keys on the stage-1 `areas[].mitigation` declaration; the display-name heuristic is deleted, so a renamed water bottle in a class kit no longer passes the gate. |
| `DW0211` | An area's declared relight `fixture` cannot raise every reachable walkable cell to `min_light` — no valid placement site remains (spec-0010). |

**The branch-coherent flow model (`compiler::flow`).** Reachability is not one
union fixpoint over "every `set-flag` anywhere". A **choice group** is a dialogue
node with ≥2 options that each set a flag — taking one means not taking its
siblings, so the options are XOR alternatives. A **world** picks one alternative
per group (the product over the *flag-reading* groups only, capped at 512;
groups past the cap stay unconstrained, i.e. exactly the pre-model behavior).
The fixpoint runs **per world**, and a quest/objective is reported unreachable
only when it is unreachable in **every** world — so the branch model makes
`DW0202`/`DW0203` strictly more precise, never looser.

A flag producer is conditional on its gating context:

| Producer | Available when |
|----------|----------------|
| `set-flag` in `on_objective_complete[o]` | `o` is completable **and** every `requires_flags` gate on the enclosing effect chain is satisfied |
| `set-flag` in a quest's `on_complete` | that quest completes, same gate rule |
| `set-flag` on a dialogue option | the option is reachable from its tree `root` through options whose own gates are satisfied, and is the world's selected alternative of its group |
| `set-flag` in an environment trigger's `effects` | the trigger's `requires_flags` are satisfied — **ambient** (a `strike`/`use`/`approach` trigger is player-initiated and has no DAG position) |
| a trap's `disarm.sets_flag` | the trap's `requires_flags` are satisfied (ambient, same reasoning) |
| `set-flag` in an `on_respawn` / `on_caught` reaction bundle | **never** — reaction bundles fire at statically unknowable times, so nothing inside one is a producer (the conservative stance `compiler::continuity` already takes) |

Consequences worth stating plainly: a `set-flag` gated on the very flag it sets
(the "re-affirm the branch" idiom) produces nothing; a flag produced only on the
`flag/flee` branch cannot satisfy a gate on the `flag/wait` branch; and flags set
from dialogue, triggers and trap disarms are first-class producers, so those
legitimate shapes no longer die as spurious `DW0203`.

**The exported critical path is one branch (`DW0204`).** `compiler::plan` does
not walk the finale's whole stage-4 `depends_on` closure. It walks the
**playthrough** the flow model proves: the first world (deterministic
enumeration order, all-first-alternative first) whose finale quest completes,
restricted to the quests that complete in it, with each `talk-to` taking the
completing dialogue option that belongs to that branch. Before export, the
sequence is **replayed** step by step through the flag/objective/quest state
machine: every step's quest must be active, its `after` prerequisites completed,
its `requires_flags` set and its `forbids_flags` unset *at that position*, its
completing dialogue option reachable *at that position*, and `campaign-complete`
must fire exactly at the final step. The first violation is `DW0204`, naming the
step. `compiler::plan`'s gate-aware reachability (`DW0306`) judges the same
sequence, so the static proofs and the exported bot contract agree by
construction. When no world completes the finale the campaign is already
`DW0201`; the model then degenerates to the whole closure so the geometry-only
commands (`chart`, `snapshot`) still run on an unanalyzable campaign.

**`forbids_flags` and producibility (v0.6, conservative).** The reachability
fixpoint models `requires_flags` producibility (a gating flag must be producible
by an already-completable producer on the same branch) but deliberately
**ignores** `forbids_flags`: whether a forbidden flag is set when an element is
needed depends on play order — full temporal reasoning the existence fixpoint
does not attempt. An element with a negative gate is therefore treated as
fireable, so `forbids_flags` can never cause a spurious `DW0202`/`DW0203`. The
**compensating stronger check** is the `DW0204` path replay, which does have a
concrete order and enforces every negative gate at its real position on the
exported path. The other static guarantees that hold: every `forbids_flags`
reference resolves to a produced flag (`DW0172`), and a completing dialogue
option gated only by `forbids_flags` still counts as gated for `DW0191`.

### DW03xx — build / solver / nav (`compiler`; error; exit 3, `stage:"build"`)

Exit 3 except `DW0312` (wave-capacity), `DW0313` (gravity-despawn) and `DW0342`
(lethal-trap completability), which are analysis-tier and mapped to exit 2 in
`main` like the `DW02xx` codes — see their rows.

| Code | Meaning |
|------|---------|
| `DW0300` | Generic build/resolution failure (missing prefab metadata/`.nbt`, unresolved anchor, critical-path dependency cycle). |
| `DW0301` | Bound pool declares no `entry` piece (or no `connector` filler when needed). |
| `DW0302` | A campaign-referenced anchor is provided by no pool member. |
| `DW0303` | `pieces{min,max}` too small to fit entry + required anchor-bearing pieces. |
| `DW0304` | Solver could not place a required piece without overlap (after retry), or a branching layout's pool declares no branch piece (tee/cross). |
| `DW0305` | A campaign-referenced anchor is defined by >1 placed piece (ambiguous); or a required anchor's only carrier is the `entry` piece. |
| `DW0306` | Gate-aware reachability deadlock (an anchor reachable only through a gate no earlier objective opens). Modelled by **splitting the carrying piece into two halves** along the gate plane, joined only by the gate cut-edge — so an in-piece bypass *around* the gate is invisible to it and a far-side objective always reads as a deadlock. **Shortcut gates are exempt by construction, not by a special case**: the heuristic's gate set comes from `open-gate` effect anchors only (`collect_open_gate_anchors`), and a stage-5 `shortcut` gate has no `open-gate` effect, so it never enters the piece-split graph. Its deadlock obligation is discharged by a strictly stronger proof instead — `Plan::build` seals every shortcut gate at step 0, so the cell-level `DW0311` critical-path proof must find the long route over real geometry (delete the long route and the build fails at `DW0311`, not `DW0306`). This is why a souls loop-back — whose `unlock` sits on the far side by definition — must be declared as a `shortcut` and not as a plain `open-gate` reward on the far-side objective; the latter is genuinely self-deadlocking and is still rejected. Pinned by `souls_shortcut::{a_shortcut_owned_gate_is_not_a_dw0306_deadlock, the_same_gate_as_a_plain_open_gate_is_still_dw0306, a_shortcut_with_no_long_route_is_rejected_by_the_critical_path_proof}` — the same geometry, green as a shortcut and red as a plain gate. |
| `DW0307` | `move-npc` destination unreachable by A* over the solved voxel grid. |
| `DW0308` | `cutscene` camera dolly clips a solid block (checked per shot; the message names the shot and segment). Checked over **both** the authored waypoint polyline and the client-rendered keyframe chord path (`compiler::camera` — the client tweens straight between emitted keyframes, so a chord can cut up to 0.25 blocks inside an authored corner; the chord message names the keyframe pair). |
| `DW0309` | Mannequin NPC declares `skin.texture_id` but no `skins/<id>.png` to bake. |
| `DW0310` | `spawn-wave` references a wave whose spawn anchor resolves in no assembled area (dangling spawn). |
| `DW0311` | Critical path has a consecutive visited-anchor pair with no walkable A* connection and no inter-area transport (player stranded). Routed over the collision-classified occupancy (task #59), so a required anchor sealed behind an unbroken 1.5-tall fence/wall ring with no fence-gate opening fails here — the full-solid model wrongly proved such pens by standing the player on a fence-top. |
| `DW0312` | A `spawn-wave` needs more standable spawn cells near its anchor than the anchor's own room provides (task #41). **Analysis-tier: exit 2**, like `DW02xx` — a content-design capacity mistake (shrink the wave or use a larger room), not a geometry defect; the message names the wave, area, and needed-vs-found count. |
| `DW0313` | A placed gravity block (`sand`/`gravel`/`concrete_powder`/anvil/`dragon_egg`) despawns into the void at placement — an unsupported gravity floor over the `the_void` world falls out on the first block update, holing the shipped map even off the critical path (task #42). The authoritative gravity-settle gate (`crate::assembled`), not a downstream DW0311/DW0312 side effect. **Analysis-tier: exit 2** — a prefab/generator defect; the message attributes despawned cells+counts per piece and prescribes a non-falling substrate. Blocks that fall but **land on support** are faithfully modelled by the settle pass (no diagnostic): the shipped geometry is exact for every consumer, and the generator's own zero-unsupported invariant catches an *unintended* fall at authoring. Anti-dodge: swapping the floor palette to non-falling blocks to silence this is explicitly rejected — gravity floors are a first-class content need; add the substrate. |
| `DW0314` | An exported critical-path waypoint is not standable in the FINAL assembled world (settled + water-flooded + relight fixtures) — the build-time self-check that makes the water-flow / post-nav-mutation divergence class structurally impossible to ship (task #45). Routes come from A* over that same world, so this fires only if a later pass mutates a cell nav relied on or an endpoint resolves off the walkable set; the message names the offending cell and leg. Fix the prefab/water or the assembly — never nudge the waypoint. |
| `DW0315` | A `set-checkpoint` (spec-0012) strands the party: re-rooting the DW0311 reachability at the checkpoint cell, the first remaining required critical-path anchor is no longer walkable from it (a checkpoint behind a one-way drop the forward path can't re-cross after respawn). The message names the checkpoint and the first unreachable anchor and prescribes moving the checkpoint or adding a return route — never deleting the checkpoint to silence the proof. |
| `DW0316` | A `set-checkpoint` anchor has no standable footing within snap range on the final assembled model (a trap-trigger / hazard / mid-air cell) — the party would respawn into void or a wall (spec-0012). Because the relight pass already proves every reachable walkable cell meets the area's `min_light`, a checkpoint that clears this and DW0315 provably meets `min_light` too. |
| `DW0378` | A `timed-gate` (spec-0016 §4) is a **coin flip, not a timing read**: the entry phases from which a walking player clears the span before it shuts cover less than **20%** of the cycle. All-phase passability is explicitly NOT the requirement (owner ruling 2026-08-02) — punishing bad timing is the point; punishing *every* timing is a slot machine no amount of learning the level makes fair. The crossing cost is the A* step count between the footings either side of the region with the gate open, charged at the same 4 t/block sprint model `DW0355` uses; the admitting window is `max(0, open_ticks − cross + 1)` of `open_ticks + closed_ticks`, computed in integers (no float rounding in a proof, ADR-0006) and rounded DOWN. `compiler::nav::check_timed_gates`, build-tier (exit 3). Prescription: lengthen `open_ticks`, shorten `closed_ticks`, or narrow the span — never lower the floor. The runtime counterpart is the waypoint artifact's `timed_gates` table + per-leg crossing marks (see above): the harness bot waits out the window instead of failing a leg the gate shut on. |
| `DW0376` | An `ambush` (spec-0016 §3) with **no counterplay**: standing every ambusher on the cell it will occupy, no checkpoint, bonfire or campaign entry is walkable from the trigger cell any more — the party is sealed in a pocket with the ambush and can only trade blows blind. The `DW0342` trap-avoidability machinery generalized from one hazard cell to an occupied cell set. This is NOT a telegraph requirement: 初见杀 is legitimate and determinism guarantees the second attempt meets the same ambushers in the same cells; what this proves is that the second attempt has a *play* — a retreat, luring ground, an exit. `compiler::nav::check_ambushes`, build-tier (exit 3). |
| `DW0373` | A `shortcut` (spec-0016 §2) has **no long route**: with its gate sealed, the far-side `unlock` affordance is not walkable from the campaign entry, so the mechanism that opens the shortcut sits behind the shortcut and can never be pulled. `compiler::nav::check_shortcuts`, build-tier (exit 3). Prescription: connect the far side by a long route, or move the unlock onto one — never open the gate at world-load to silence it. |
| `DW0374` | A `shortcut` (spec-0016 §2) **leaks**: opening its gate does not strictly shorten the A* walk from the campaign entry to its own `unlock`, so the unlock is not on the far side of anything and the loop-back the shortcut exists for never happens. The classic form is an `unlock` placed on the NEAR side of its own gate — this is the proof that makes `unlock` a far-side anchor rather than a label. Both distances are measured over the same nav model, differing only in the gate. `compiler::nav::check_shortcuts`, build-tier (exit 3). |
| `DW0386` | A TD `lane` (spec-0016 §6) does not survive contact with the assembled world: a waypoint anchor that resolves nowhere in the wave's area, a waypoint with no standable footing within 3 blocks, a leg the squad cannot walk (routed on the same **no-gate-use** view wave seating uses — lane mobs cannot right-click a fence gate open), or a leg of **10 blocks or less**. The spacing rule is not taste: vanilla re-rolls a patrol target to a random point once the patroller is within 10 blocks of it, so a tighter lane is one the engine quietly stops following — it reads as working-but-drunk, not as a bug. The spike's measured working default is 12. `compiler::nav::plan_lanes`, build-tier (exit 3); the message names the wave, both leg endpoints and the measured length. |
| `DW0387` | A `summon: aggro-edge` wave (spec-0016 §6) whose perception ring offers fewer valid cells than the stack has mobs. The ring is the standable, walk-reachable, line-of-sight cells on `[follow_range - 1, follow_range]` around the defended anchor, inside the area. An error rather than a silent short spawn on purpose: the round-1 lesson was a wave that never fully appeared, so its `kill` countdown could never reach zero and the delve soft-locked with every other proof green. `compiler::emit::plan_aggro_edge_spawns`, build-tier (exit 3). Prescription: give the arena room at that radius, lower the stack's `follow_range` to a ring the arena actually has, or move the defended anchor off the wall. |
| `DW0324` | An L2 massing verb cannot apply to the solved layout (v0.6, spec-0017 PR 3): the target area binds a single `prefab` (no jigsaw layout to mass), a `piece` index / `prefab` guard mismatches the placement (layout drift), a `swap-piece`/`reseed-piece` candidate cannot re-mate every mated socket without overlap (or the pool has no compatible variant), an `insert-piece` socket is already mated or nothing attaches without overlap, a `remove-piece` targets the entry piece or a non-leaf, or a `rewire-socket` names an out-of-range connector / seals an already-sealed (opens an already-open) socket. `compiler::massing`, build-tier (exit 3); every message names the batch and prescribes re-inspecting the layout with `delvec snapshot` — never deleting the drift guard or the sockets. |
| `DW0322` | Post-edit boundary safety (v0.6, spec-0017 invariant 4): after a world-edits batch, the reachable walk region fails "one step off the proven ground is survivable **and recoverable**". `nav::verify_boundary_safety`, run after every edit batch (never on the no-edit path, whose worlds provide the guarantee physically). One code, one rule — *stated against the world-generator ambient* (`nav::Ambient`, spec-0013 `horizon`), because what an unmodelled column contains is the generator's property, not the content's. **`horizon: void`**: a reachable walkable cell borders a **void drop** — a horizontally adjacent column the player can step (or open a gate) into with no fall-arrest of any kind below (no solid, no fence/wall/gate top, no water); one step off the proven ground falls out of the world. Prescription: extend the terrain under the exposed edge (fill/morph a slope or outcrop) or reinstate a barrier shape. **`horizon: ocean`**: the pinned bedrock/stone/water superflat puts ground under *every* column, so nothing can fall out of an ocean world and the void premise is vacuous — the rule is the **stranding** invariant instead (the hazard `plan::OCEAN_BASE_Y` already names): a reachable walkable cell lets the player into a body of water with no climb-out back into the reachable walk region. Prescription: give the shoreline a step at the waterline (a beach or a bank), or wall the edge so the water cannot be entered there. Both branches **aggregate**: one report per run listing up to 6 violations plus a total, so the scale of a breach (one cell vs. the whole coastline) is visible without re-probing. Build-tier (exit 3); the message names the batch. Numbered in the 032x world/region family beside the spec-0013 boundary pair (`DW0320`/`DW0321` are validation-tier; this one is build-tier — it needs the edited geometry). Never weaken the check or reroute the path around it. |
| `DW0323` | A stage-7 edit fails to **resolve** against the solved layout (v0.6, spec-0017): a piece-local frame's `piece` index is out of range or its `prefab` guard mismatches the placed piece (layout drift — the loud alternative to a silently misplaced edit), an `anchor-relative` frame names an anchor the batch's area does not resolve, or a verb's target region resolves to **zero cells** (a silent no-op is always a defect: the select drifted off the content it targeted). Also the `fragment` verb's own resolution failures: a prefab outside the admitted library, one decoding to zero non-air cells, and a `rotation` other than `none` on a prefab carrying yaw-dependent blockstate — rotate-aware stamping is not implemented, so the compiler refuses the stamp instead of shipping unrotated facings (see the stage-7 `fragment` row). `compiler::edit`, build-tier (exit 3); the message names the batch and prescribes re-inspecting the layout with `delvec snapshot` — never deleting the prefab guard or leaving a dead edit. |
| `DW0352` | A world-edits batch writes into a cell a trap's hardware occupies (v0.6, spec-0017 + spec-0011): its trigger/hazard cell, its dispenser socket, or its disarm-affordance cell. `setup_finish` runs `world_edits` **before** `trap_setup`, so the edit lands first and the trap is loaded into a block that is no longer there — vanilla's `item replace block … container.0` on a non-container fails with **no output**, so the delve ships a dead trap with every proof green (`DW0342` proves the *planned* hazard, not the surviving hardware; no geometry proof models "is this still a dispenser"). `compiler::edit`, checked first in the per-batch invariants, build-tier (exit 3). The message names the batch, the cell, the trap and which role the cell plays; prescription is to move the region off the trap's cells or re-anchor the trap — never to assume the edit leaves the redstone intact. |
| `DW0354` | A support-dependent block the edit script placed has no valid support in the post-batch world (v0.6, spec-0017): a torch/lantern/campfire/rail-family block with **nothing below it** after a later batch carved its support away, or flora rooted in a block flowers cannot stand on (a `scatter` over bare stone). Vanilla pops such a block off as an item on the first chunk tick, so the write silently vanishes from the delivered world while every snapshot still shows it. **Two tiers, one code**: advisory (exit 0) for decoration, aggregated per reason + block with a count and one example cell; **error (exit 3)** when the popped block is a fixture the script's own `relight` verb placed — that is a declared `min_light` guarantee the `DW0211` proof accepted, and losing it re-darkens the region. `compiler::edit`, evaluated at every batch close over the cumulative placement set. Deliberately conservative: blocks supported sideways or from above (`wall_torch`, `hanging=true` lanterns) are classified as needing no support, and "support removed" means removed to **air** — the check never guesses about a block it cannot classify. |
| `DW0325` | A `move-actor` destination is unreachable over the assembled geometry for the **actor's footprint** (per-entity dims table; warden 0.9×2.9 needs 3 cells of headroom, so it can be stranded where a player fits), or an actor spawn/destination anchor resolves to no world position (spec-0014). Build-tier (exit 3), `compiler::nav`; the message names the actor, the leg, and a best-effort first blocked cell. |
| `DW0327` | A `begin-stealth` (spec-0014) zone has **no** standable cell, or **no** standable cell of the zone is reachable from the player's position at the beat that activates the stealth check — a guaranteed-unwinnable stealth beat. Reachability is **reachable-any over every cell of the zone box** (task #78): testing only the cell nearest the zone centre raised a spurious `DW0327` whenever that one cell happened to snap into a walled-off pocket of an otherwise perfectly reachable zone. The message names the zone and prescribes placing it over reachable floor / within walkable reach of the activating beat. |
| `DW0355` | A **punishing** `begin-stealth` beat whose grace window cannot be beaten (spec-0014 + spec-0016): from a position a player legally occupies the instant the session arms, no zone is reachable within `grace_ticks` at sprint speed over the assembled geometry. DW0327 proves cover exists and is *connected*; this proves it is reachable **in time** — the gap that shipped the island's blinding beat, where the beat armed under the player's feet at the fire-pit and killed every player (bot and human alike) ~2 s later, on a first honest ladder run. Start positions: the activating objective's anchor **and** every `set-checkpoint` reigning inside the beat's active window `[fire_step, end_step]` — a respawn point that cannot beat the window makes the retry loop non-terminating rather than a souls retry. Cost model: 4 ticks/block (vanilla sprint 0.2806 blocks/tick, rounded up — no sprint-jump credit) + 6 ticks per block climbed + 10 ticks of standing-start reaction; routed over the same per-leg geometry DW0311/DW0315 use (gates causally sealed by the firing step forced solid). Build-tier (exit 3), `compiler::nav`. The message names the beat, the start, the nearest zone cell, the measured flee time and the tick deficit. Scope: only beats whose `on_caught` tree actually punishes (`damage-players` / `spawn-wave`) — a narrate-only beat has nothing to escape. Prescription: raise `grace_ticks` to at least the measured need plus a tension margin, put a zone within reach of where the beat starts, move the checkpoint into/beside a zone, or arm the beat from a less exposed objective. **Delaying the arm does not discharge it** (a `sequence` step buys drama, not proof: the clock still starts with the player free to be standing at the start cell), and deleting the `on_caught` consequence is explicitly not a fix. Numbered `DW0355`, not `DW0352`: this rule and the map editor's trap-hardware check were developed on parallel branches, each picking the next free code against its own branch point, and collided on merge — `tools/check-dw-codes.py` now gates one-code-one-rule so that class fails CI instead of shipping. |
| `DW0329` | A `sequence` effect is nested inside another `sequence` (directly, or reachable via a nested `move-actor` `on_arrive`) — timelines do not recurse (spec-0014). Validation-tier (exit 1), `dsl::validate`. Flatten the inner steps into the outer timeline (shift their `at_ticks`). |
| `DW0342` | A **lethal** trap (spec-0011) whose trigger cell lies on the forced critical path with no discharge — not avoidable (the trigger cell is a required path cell), not survivable (`rearm`, so a respawn walk-back re-triggers it → soft-loop), and not disarmable (no disarm affordance reachable before it, over the world with the trap cell blocked). The player is provably killed or soft-looped. **Analysis-tier: exit 2**, like `DW0312` — a content-design mistake, not a geometry defect; the message names the trap and prescribes moving it off the path, setting `reset: once`, or adding a reachable `disarm`. Renumbered off the spec's stale reserved number (0314 — since taken by the waypoint self-check). |
| `DW0344` | In a `horizon: ocean` world, a placed piece whose prefab metadata declares `waterline_y` does not land that waterline at sea level (`piece.y + waterline_y ≠ 62`) — the piece floats above the sea (its shore an unclimbable cliff, its authored water pocket hanging in the air) or is drowned under it. Build-tier (exit 3), `compiler::plan`, checked after placement. Nothing downstream can catch this: nav, boundary, POV and PackTest all derive from the very placement that is wrong, so a mis-datumed island validates green and ships unplayable. The message names the area, prefab, placed y and the signed offset, and prescribes correcting the declared `waterline_y` (the local y of the piece's top water block; the island convention is 2) or rebuilding the piece against the convention — ocean areas are placed at y=60 and a piece with a different waterline cannot share that datum. Pieces declaring no `waterline_y` author no sea and are not checked. |
| `DW0345` | The assembled world resolves **no entry anchor** — no placed piece declares any of the entry-anchor names (`spawn`, `entry`; see §4 "First-join placement"). The compiler then has no cell to call the campaign's start: no `setworldspawn`, no class-apply teleport, no first-join placement, no `dw:cp` seed. Build-tier (exit 3), `compiler::emit`. Silent before — the delve compiled clean and fell back to the vanilla spawn search, which a **dedicated** server resolves to the surface (so every rung of the validation ladder stayed green) and the **integrated singleplayer** server resolves to the build floor, i.e. inside solid stone. Prescription: give the pool's entry-role prefab an entry anchor in its metadata `anchors`, or bind the area to a prefab that has one. |
| `DW0346` | A prefab metadata `*.json` (or `pools.json`) in the prefabs dir failed to read or parse (task #62). The canonical trigger is an **older delvec meeting newer metadata**: `deny_unknown_fields` rejects a field this delvec predates. Previously a silent skip — the prefab vanished from the registry and the run failed much later as a baffling `DW0300` "prefab not found" (or a `DW0160` binding error) with no hint of why. Now `PrefabRegistry::load_dir` records a per-file diagnostic naming the file and the serde error, folded into every `validate`/`analyze`/`build` at **validation tier (exit 1)**; loading continues for the other files (report-all, not fail-fast). Prescription: upgrade delvec, or fix the named field. |
| `DW0347` | A `cutscene` shot's aim sweeps faster than the angular budget: over 6°/tick (120°/s) peak on the exact eased path — at 20 Hz that reads as a spin, not a shot (the camera dossier's comfortable band is ≤ 2°/tick; thresholds are the dossier's proposal — the spike rig has no rendering client to calibrate against footage). Typical cause: a `look_at` subject too close to a fast dolly, or a sharp travel-aim corner. Build-tier (exit 3), `compiler::nav` (task #64). An **error**, not a warning: the shot is provably nauseating before it ships, and the fix is always available — more camera distance, a longer `seconds`, or splitting the move into two shots (the hard cut between shots is the idiomatic fast reframe). |

| `DW0360` | An anchor-bearing quest/trigger effect — at **any** nesting depth — names an anchor that resolves to no position in the assembled world. The single resolved-anchor-or-diagnostic seal over the whole effect surface, driven by `QuestEffect::anchor_refs` (the referential sibling of `nested_effect_lists`). It exists because every anchor consumer in emission fails **open**: `open-gate`/`close-gate` scan `plan.anchors` for a name match and fall out of the loop, `set-block`/`set-checkpoint`/`play-sound`/`damage-players` bail out of an `if let Some(pos)`, and a cutscene waypoint silently degrades to `[0, BASE_Y, 0]`. One typo'd anchor therefore emitted **nothing** — a door that never opens, a checkpoint bound to nothing — in a delve that compiled clean. `DW0142` catches what the DSL can see (an area's declared anchor set); this re-asks the question of the *assembled* world, so pool areas and cross-area camera anchors are covered too. Build-tier (exit 3), `compiler::emit`, run **first** among the referential proofs: an unresolved waypoint degraded to the origin otherwise surfaces as a bogus `DW0308` camera clip, sending the author to move a shot that was never the problem. |
| `DW0361` | Two different generated artifacts (function / dialog / advancement) sanitize to the same name, so one would silently overwrite the other in the emitted pack. `plan::safe_local` is doubly lossy — it drops an id's `<kind>/` prefix and folds `-`, `/` and `.` all into `_` — so wave `wave/npc-x` and npc `npc/x` both name `spawn_npc_x`, and `move-npc npc/guard-a → anchor/post` collides with `npc/guard → anchor/a-post` (which also aliases their tick counters and re-entry sentinels: two live movement drivers sharing one score). The output map is a `BTreeMap`, so the loser used to vanish without a word — the wave simply never spawned. Re-emitting the **same bytes** under one name stays legal (the emitters dedup by content key); only a genuine divergence fails. Build-tier (exit 3), `compiler::emit`. Prescription: rename one of the colliding ids so their sanitized local parts differ. |
| `DW0362` | A dialogue node declares more than `MAX_GATED_DIALOGUE_OPTIONS` (10) conditionally-visible options (`requires_flags` / `forbids_flags` / a `complete-objective` effect). Vanilla cannot hide a `dialog` option, so the compiler encodes visibility by precomputing **every combination**: `n` gated options emit `2^n` dialog JSONs plus a `2^n`-clause dispatcher keyed on a `dw.dmask` bitmask. Ten is 1024 variants for one node — already an order of magnitude past anything authorable (the largest node in any shipped campaign gates four), and the point past which pack size rather than the author decides what the delve is. Behind the soft cap is a hard wall: the mask is built with `1u32 << i` (a debug-build **panic** at 32 — the original symptom) and compared against a Minecraft scoreboard, i.e. an `i32`. Build-tier (exit 3), `compiler::emit`; the message names the node and npc. Prescription: split the node into a short chain, or move some gating onto the objective that reaches it. |
| `DW0363` | A trap declares a flag gate (`requires_flags` / `forbids_flags`) whose trigger hardware the compiler cannot remove and restore. Trap flag-gating is a **physical** gate: the trigger block leaves the world while the gate is shut and is put back verbatim (blockstate and all) when it opens, so it is only sound for a trigger whose entire state is the block — a pressure plate or a tripwire. A `trapped-chest` trigger carries a block entity with an inventory that removal would destroy, and a gated trap whose `anchor/trap` metadata declares no `trigger_block` names nothing the compiler could put back. Rejecting the gating surface for those cases is deliberate: the alternative is shipping the documented behaviour as folklore, which is exactly what happened before (the flag lists were planned and `DW0172`-checked but read by **no** emission site, so "inactive while the flag is set" did not exist). Build-tier (exit 3), `compiler::emit`. Prescription: declare the plate/tripwire as `trigger_block` on the anchor's prefab metadata (with its blockstate, as a gate anchor declares its fill `block`), switch the trap to a `pressure-plate`/`tripwire` trigger, or gate the story beat that arms the trap instead. |
| `DW0359` | An NPC or actor **body** stands on, or immediately in front of, an interaction affordance the party has to click (owner island QA, round 7). Bodies are boxes: a mannequin wears its `base_entity`'s standing hitbox (`nav::entity_dims` — one dims table, shared with actor-footprint routing), or the player model's 0.6 × 1.8 when it declares a `skin`; every affordance the compiler summons is a `minecraft:interaction` of `width:1.0f,height:2.0f`, i.e. exactly its anchor cell's column two blocks tall. Five affordance sources, one shape: `interact` objectives, `use`/`strike` triggers, `bonfire` rest points, `shortcut` unlocks and trap `disarm` affordances. **Two tiers, one code**: **error (exit 3)** when the boxes overlap in all three axes — the client's ray-pick reaches the invulnerable body and the affordance can never be clicked, so a required objective is unreachable and the delve soft-locks; **advisory (exit 0)** when they are apart but within 1 block horizontally (Chebyshev) with overlapping vertical spans, because whether a neighbouring body actually shadows the crosshair depends on the approach angle the player takes, which the compiler cannot know. `compiler::eclipse`, run with the referential seals before any occupancy model (pure box arithmetic over resolved cells). This is the geometric statement of `DW0350`, which is symbolic (same anchor *name*) and sees only `use` triggers — an NPC body over an *objective's* affordance, or a 1.95-wide ravager's shoulder reaching into the cell next door, passed silently. It is the check the round-7 island needed: `npc/polyphemus`, a 0.9 × 2.9 warden on `anchor/fire-pit`, hid `obj/harden` and `obj/blind` behind itself. Two exemptions, both about not inventing certainty: a `strike` trigger on an NPC's own anchor summons **no** entity (it rides the NPC's hitbox — nothing to eclipse), and a body the campaign ever **moves** (`move-npc`/`move-actor`, any depth) is skipped, because a declared anchor is only a walker's starting mark and deciding "is it still there when the affordance goes live?" needs a timeline the compiler will not guess (known blind spot: a body walked *onto* an affordance, which wants a destination rule of its own). Prescription: move the body's anchor or the interaction's anchor 2+ blocks apart — **never** make the body intangible, which trades a dead objective for a character the party cannot talk to. |

### DW039x — shot calibration (`delvec calibrate`; spec-0019)

`calibrate` is the only subcommand that reads no campaign and builds no world —
just a harvested `rehearsal-report.json` plus the build's
`creator-datapack/layout.json`. Its codes therefore carry their own exit
mapping, stated per row rather than by the DW03xx section default.

| Code | Meaning |
|------|---------|
| `DW0390` | A harvested shot proposal names a cell with **no declared anchor within the 16-block snap radius**, so it cannot be written back into the DSL at all — the DSL has no free-floating world coordinates (spec-0019 §5). Reported per offending cell with the nearest anchor and its distance; the whole shot is left un-patched (a half-snapped dolly would fly a path nobody authored), while every other shot of the same session still is. **Exit 3**, and the patch file is still written. Prescription: declare an anchor near that cell in the prefab's metadata and re-mark the shot, or move the shot to an anchored spot — do NOT widen the radius and do NOT write a raw coordinate into the stage document. |
| `DW0391` | The rehearsal report and the `--layout` manifest name **different campaigns**: the proposals would snap onto another delve's anchors and silently relocate every camera. Refused before any snapping. **Exit 1**. Prescription: point `--layout` at the `creator-datapack/layout.json` of the build that session actually played — do NOT reuse an older build's manifest. |
| `DW0392` | The rehearsal report is unreadable, is not a rehearsal report, or carries a schema `version` this `delvec` does not understand (likewise for an unreadable layout manifest). **Exit 1**. Prescription: re-run `delve-harvest` over the session log — the report is a machine artifact and is never hand-written or hand-edited. |

### DW07xx — workspace tooling (spec-0007; **not `delvec`**)

Separate binaries with their own exit-code schemes; diagnostics to **stderr**.
Catalogued here so the DW namespace is complete and CI-checked.

| Code | Tool | Meaning |
|------|------|---------|
| `DW0700` | `delve-schem` | Strip hook: a forbidden block/entity was removed. |
| `DW0701` | `delve-schem` | Oversize schematic tiled into structure parts. |
| `DW0702` | `delve-schem` | Source `DataVersion` ≠ pinned MC 1.21.11. |
| `DW0710` | `delve-schem` | Input unreadable / not a Sponge schematic. |
| `DW0720` | `delve-render` | Missing-texture (magenta) placeholder detected (fidelity gate; exit 4). |
| `DW0721` | `delve-render` | Input (`.nbt`/metadata/`render-plan.json`) unreadable (exit 2). |
| `DW0722` | `delve-render` | Output file could not be written (exit 3). |
| `DW0723` | `delve-render` | GPU renderer failed / textures absent (exit 5). |
| `DW0724` | `delvec` (visual tier) | A player-POV camera eye cell is occupied (solid/water) in the FINAL assembled world — the frame would render the inside of a block, not the player's view. The self-check behind the visual tier (`compiler::nav::verify_pov_cameras`), mirroring the DW0314 waypoint self-check: every POV camera stands at the eye-height (1.62) of a DW0314-proven-standable waypoint, so this can only fire if the eye-height/standing-cell derivation changes to place the eye in a ceiling/wall (or a later pass mutates the cell). Numbered in the `DW072x` visual/render range; emitted by the compiler's nav pass (exit 3). Fix the camera derivation — never nudge the waypoint or the geometry. |
| `DW0730` | `delve-admit` | Audit: a palette block is not in the allowlist. |
| `DW0731` | `delve-admit` | Audit: a hard-forbidden code-injection vector (command/structure block, NBT spawner, embedded `Command`). |
| `DW0732` | `delve-admit` | Input error (unreadable `.nbt`/metadata/JSON). |
| `DW0740` | `delve-admit` | Catalog card schema/field validation failure. |
| `DW0741` | `delve-admit` | Catalog card license not in the ADR-0013 allowlist. |
| `DW0750` | `delve-admit` | Admission tooling (socket/anchor/lighting) failure. |
| `DW0751` | `delve-admit` | Lighting probe: a `dark` interior was measured (advisory; no longer gates — spec-0010). |
| `DW0760` | `delve-admit` | Gallery emission / curation failure. |

`delve-render` exit codes: `0` ok · `2` input · `3` output · `4` fidelity-gate
failure · `5` renderer/GPU · `10` internal.

#### `delve-render` dark-shot REVIEW POLICY (night-vision emulation)

For shots stamped `{"profile": "dark", "mitigation": "night-vision"}` — and only
those — `delve-render scene` emits the Chunky scene with a review-only
`materials` override: every non-light-emitting block of the build's shipped
structure palettes (union over `datapack/data/*/structure/*.nbt`, sorted,
deduped, state brackets stripped) gets a low uniform emittance
(`scene::REVIEW_EMITTANCE` = 0.05), and the scene carries
`"delvewrightReviewPolicy": "night-vision-emulated — review only"` (Chunky
ignores unknown keys). `delve-render index` marks the same shots with
`review_policy` and passes the `lighting` stamp through. **This is an honest
approximation, not ground truth**: faint uniform self-glow is the closest
Chunky analogue of Minecraft night vision (which renders every block at full,
flat brightness), chosen after the exposure-boost route failed on the island
cavern; real emitters are deny-listed out of the override so a placed fixture
still reads as a genuine glow. Legibility of geometry/layout in an emulated
frame is reviewable; its *lighting* is not — the compiler's measured light
model remains the only light-truth. Lit-stamped and unstamped shots are
byte-untouched; a dark-stamped shot with no structure palette available is a
`DW0721` error (a silently-black "reviewable" scene would re-blind the review).
Deterministic throughout (`BTreeMap`-sorted override keys, sorted file walk).

---

## 6. Spec cross-reference

Which spec introduced or last amended each area (specs are historical records;
this doc is current behavior).

| Area | Spec |
|------|------|
| DSL schemas, stages 1–6, envelope, ids, l10n key scheme | spec-0001 (v0.1/0.2/0.3 + i18n addendum) |
| CLI, exit codes, build output, world config, environment sealing, critical path, gameplay-verb emission, jigsaw solver, `--lang` build | spec-0002 (v0 + v0.3 / M2 vertical / i18n addenda) |
| Validation ↔ runtime split; `DW02xx` analysis role | ADR-0005 / spec-0005 |
| v0.4 surface (dialogue state, props, narrate, wave tuning, NPC lifecycle, skins, triggers, cutscene, `DW0190`–`DW0195`, `DW0307`–`DW0311`) | spec-0008 |
| Skins toolchain, resourcepack bake (`DW0309`) | spec-0009 |
| v0.6 scripted actors + staging effects (`actors[]`, `spawn`/`despawn`/`move`/`unleash-actor`, `sequence`; footprint-aware nav; `DW0325`/`DW0329`) | spec-0014 |
| Assembled-relight, measured `DW0210`, `DW0211`/`DW0196`, stage-1 `lighting`/`time`/`weather`, `set-time`/`set-weather` (all v0.5) | spec-0010 (landed, #35) |
| Stage-1 `horizon` (ocean superflat), `boundary` (derived playable region + 1s return clock), `dw:region`/`dw:cp` mirrors, `DW0320`/`DW0321` (all v0.6) | spec-0013 (landed) |
| Sound + art-title surface (`play-sound`, `narrate` `art`, `delve:art` font, `DW0326`/`DW0328`/`DW0335`) | spec-0014 (v0.6) |
| Traps: stage-5 `traps[]`, `anchor/trap` dispenser fill + disarm emission, `tnt_explodes` seal, passable plate/tripwire model, `DW0340`/`DW0341`/`DW0342` (all v0.6) | spec-0011 (landed) |
| Visual authoring loop: `delvec snapshot` + `delvec blocking-chart`, the voxel raycaster, scene manifest and cutaway floor plans (§7) | spec-0015 (P1+P2 landed) |
| Souls-mode timed gates: stage-5 `timed_gates[]`, the two-function schedule clock, `DW0377`/`DW0378` (≥20% of cycle) (v0.6) | spec-0016 §4 |
| Souls-mode ambushes: stage-5 `ambushes[]` (parse-time desugaring to a trigger), `DW0375`/`DW0376`, optional telegraph (v0.6) | spec-0016 §3 |
| Souls-mode TD lanes: wave `lane{waypoints,aggro_radius}` + `summon: aggro-edge`, the Raider-patrol clock, `DW0381`–`DW0387`, `pillager`/`vindicator` added to the armed-mob default table (v0.6) | spec-0016 §6 |
| Souls-mode shortcut doors: stage-5 `shortcuts[]`, `DW0371`/`DW0372`/`DW0373`/`DW0374`, shortcut gates sealed for the whole completability model (v0.6) | spec-0016 §2 |
| Souls-mode pacing lints: retry cost `DW0379`, optional-elite bypass `DW0380` (both warning tier) (v0.6) | spec-0016 §7 |
| Souls-mode bonfires: `bonfire{anchor,on_rest?}`, wave `respawns_on_rest`, `DW0370` (v0.6) | spec-0016 §1 |
| The map editor: stage-7 `world-edits.json`, the full L3 verb set (`select`/`fill`/`replace`/`carve`/`morph`/`scatter`/`plant`/`fragment`/`relight`), the L2 massing verbs (`swap`/`insert`/`remove`/`rewire-socket`/`reseed`; `resize` excluded — no size primitive), per-batch invariant re-proofs, `DW0162`/`DW0322`/`DW0323`/`DW0324`, `delvec edit apply|preview` (all v0.6) | spec-0017 (PRs 1–3) |
| Map-editor audit fixes: trap-hardware integrity `DW0352`, gate-region + block-support advisories `DW0353`/`DW0354`, out-of-bbox edit-chunk load convergence + forceload release, `edit` running the full build-tier proof set, blockstate-preserving `fragment` stamps | map-editor audit (post-#145/#146/#149) |
| Party-shared progression: the `#party` holder, party-addressed UI, `world.min_players` + lobby gate, `give-item`/kit `carrier`, the n-agent division proof and the n-dummy `party_join_<obj>` PackTests, `DW0356`/`DW0357`/`DW0358` (all v0.6) | spec-0018 (landed) |
| Asset-pipeline tooling `DW07xx` (schem/render/admit) | spec-0007 |
| Determinism invariants | ADR-0006 |

### Known spec ↔ code drift (current, for maintainers)

- **spec-0002 CLI** lists stages `1..5`, `dsl 0.1.0`, and omits `--json`/
  `--prefabs`/`--lang`; code is stages `1..6`, `dsl 0.6.0`, all three flags.
  (Spec is the original record; addenda + code are current — this doc governs.)
- **`gamerule keep_inventory true`** is emitted by the sealing baseline but is
  **not** in spec-0002's environment-sealing list (added as box-garden death
  policy; recorded here).
- **spec-0018 runtime tier, partial by design.** The static half is complete
  (completability is proven with `min_players` agents; `DW0358`) and the runtime
  half is complete for AND-joins (the generated n-dummy `party_join_<obj>`
  templates). The **critical-path bot** is still single-bot: `critical-path.json`
  and its replay describe one abstract playthrough of party state, which is
  exactly right for `min_players: 1` and remains a *sound* (if not maximal) proof
  for a bigger party — one agent can always walk what n can divide. Running
  `min_players` bots is harness work, tracked as a follow-up, not a gap in this
  layer's contract.
- **Sky attenuation constants** (`crate::light::effective_sky`, spec-0010): the
  stored sky-light baseline (15 at a sky-open cell) and the `time`/`weather` set
  commands are live-verified (1.21.11 itzg VANILLA); the per-state *effective*
  attenuation follows the documented vanilla `getSkyDarken` surface model
  (noon/day 15, night/midnight 4, rain −3, thunder −8 by day) applied
  conservatively — the effective (time-attenuated) value is not directly
  command-readable, so it is not a live measurement. Noted for maintainers.

---

## 7. Visual authoring loop (spec-0015)

A **view-only** tier of `delvec`: draft renders of the assembled world plus a
structured description of the same frame, so an authoring agent can look at its
own build mid-authoring instead of waiting on a full build + Chunky pass. Two
commands — `snapshot` (a perspective viewport: what does it look like from here)
and `blocking-chart` (orthographic cutaway plans: is there room). Both add no DW
diagnostics, change no emission (build output is byte-identical), and never write
a datapack.

### `delvec snapshot`

```
delvec snapshot <campaign-dir>
    [--camera x,y,z,yaw,pitch[,fov]]      # explicit eye
    [--at <anchor> [--orbit <deg>] [--dist <n>]]   # frame a subject
    [--shot <render-plan id>]             # reuse a planned camera
    [-o out.png] [--labels]
    [--width 960] [--height 540] [--timing] [--json]
```

Framing precedence (the first three are mutually exclusive, enforced by clap):
`--camera` → `--at` → `--shot` → a default dollhouse overview of the whole
layout. Details:

- **`--camera`** — `x,y,z` is the eye in world coordinates; `yaw`/`pitch` are
  **Minecraft** degrees (`0` = south/+Z, `90` = west/−X, `180` north, `270`
  east; pitch positive looks **down**), the same convention the v0.6 cutscene aim
  uses. `fov` is optional (vertical, default `70`).
- **`--at <anchor>`** — accepts a bare anchor name (`anchor/fire-pit`, matched in
  the first declaring area) or `area:anchor` (`area/island:anchor/pen`) to
  disambiguate; a gate anchor resolves to its region centre. `--orbit` is a
  compass bearing in the same yaw sense (`0` = the camera stands due south of the
  subject looking north, `90` due west looking east); `--dist` is blocks (default
  `14`), with the eye raised `0.45 × dist`. **The eye is then pulled along its own
  sight line until it stands in open air**, so `--at` frames an interior (a
  cavern fire pit, an alcove) instead of rendering the inside of the mountain.
- **`--shot <id>`** — reuses a `render-plan.json` camera by id (`interior/…`,
  `npc/…`, `interact/…`, `gate/…`, `seam/…`, `pov/leg{L}/wp{W}`). The render plan
  states cameras in its own Chunky yaw convention, so the bridge reads only its
  `pos`/`look_at` world points and re-derives Minecraft yaw/pitch. `pov/…` ids
  additionally compute the DW0311 critical-path routes; other ids do not.
  An unknown id lists the available ones.

**Pipeline stages required** — parse → `Plan::build` (placement) → read the
placed `.nbt` → `assembled::assembled_blocks`. That is all: no relight, no nav
proofs, no emission. Validation diagnostics are printed but **never gate** the
render (only an unparseable campaign, exit 1, or a placement failure, exit 3,
stops it), because the loop exists precisely to look at builds that are not
finished yet.

**Renderer** — a voxel DDA raycaster (`compiler::snapshot`) over a chunked
flattening of the assembled block map. Shading is flat block-palette colour ×
face brightness (top brightest, bottom darkest, the two horizontal axes
distinct — the "ambient occlusion by face orientation") × a block-edge relief
darkening, then a distance fade toward the horizon. Background is a sky
gradient; for an `ocean`-horizon campaign the world generator's sea plane is
drawn analytically at `SEA_LEVEL` (a world-generation backdrop, never part of
the voxel model, never occluding a manifest target).

Three properties worth stating explicitly:

- **There is no lighting model.** The raycaster sees geometry regardless of block
  light, so a pitch-black cavern renders as legibly as a noon meadow — which is
  exactly what makes this the right tier for reviewing dark areas. A frame that
  looks fine here and black in Chunky has a *lighting* defect, not a geometry
  one, and the two tiers now separate that. Emissive blocks (glowstone, lantern,
  campfire, torch, …) still render at full brightness so a fire pit reads as one.
- **Only blocks exist.** Entities (NPC mannequins, scripted actors, item
  displays) are not in the assembled model and are not drawn; their *posts* are
  in the manifest and, with `--labels`, stamped on the frame.
- **Unknown blocks render magenta** (`255,0,255`, the same missing-texture key
  `delve-render`'s fidelity gate scans for). The palette resolves exact vanilla
  ids first, then material-family substrings (`_planks`, `_wool`, `stone`, …); a
  unit test asserts every block the shipped prefab library places has a real
  colour, so magenta in a frame means "a prefab introduced a block the palette
  has never seen" — extend the palette.

**`--labels`** burns in: a coordinate lattice tinted onto every visible **top**
face on a 16-block X/Z line (so it follows the terrain rather than an invented
flat plane), `x,z` readouts at the ten nearest visible lattice intersections, an
outline per in-frustum target (dim when occluded), and the target's name. Names
are placed **visible-first** and nudged down to avoid overlap; an occluded name is
stamped only where it lands clear on the first try. `--labels` changes the frame
and never the manifest.

**Output** — the PNG at `-o` (default `snapshot.png`) and a manifest sidecar at
the same path with its extension replaced: `shot.png` → `shot.manifest.json`.

### Scene manifest (`manifest_version: 2`)

```json
{
  "manifest_version": 2,
  "campaign_id": "nobodys-cave-island",
  "delvec": "0.1.0",
  "image":  { "path": "shot.png", "width": 960, "height": 540 },
  "camera": { "pos": [x,y,z], "yaw": 0.0, "pitch": 25.7, "fov": 70.0,
              "convention": "minecraft degrees: yaw 0 = south (+Z) …" },
  "world":  { "block_kinds": 48,
              "bounds": { "min": [x,y,z], "max": [x,y,z] },
              "sea_plane": 62 },
  "pieces": [
    { "area": "area/island", "index": 1, "prefab": "prefab/island-greenfield",
      "origin": [0, 60, -30], "size": [16, 12, 16], "rotation": "none",
      "box": { "min": [0, 60, -30], "max": [15, 71, -15] } }
  ],
  "targets": [
    { "id": "anchor/fire-pit", "kind": "anchor", "area": "area/island",
      "pos": [9, 69, -56],
      "screen_bbox": { "x": 466, "y": 264, "w": 28, "h": 36 },
      "occluded": false, "distance": 14.724 }
  ],
  "out_of_frame": [ { "id": "anchor/pen", "kind": "anchor", "area": "…",
                      "pos": [x,y,z] } ]
}
```

- **`pieces`** is the **layout** half of the scene, beside the point/region
  targets: every placed structure piece of the whole plan (not just the ones in
  frame), in plan order — areas as the plan holds them, pieces entry-first
  within each area. It carries exactly the inputs a `piece-local` edit frame
  resolves against (`edit::resolve_frame_point`: `origin + rotation(local)`
  against `area.pieces[index]`): the per-area `index` (the frame's `piece`
  field), the `prefab` guard value, the `/place template` `origin` + `rotation`
  token, the unrotated `size`, and the resulting inclusive `box`. Without it an
  editor authoring a piece-local frame had to back-solve the index and the
  transform from the rendered geometry by hand.
- **Kinds**: `anchor` · `gate` · `npc-post` · `actor-post` · `interact` ·
  `stealth-zone` · `trigger`. A point target carries `pos` (an inclusive cell);
  a region target carries `box: {min,max}` (inclusive cells) — never both.
  `gate` is the gate region, `stealth-zone` a `begin-stealth` zone box
  (`stealth-<beat>/<anchor>`), `trigger` an `EnvTrigger` — a box of its `range`
  for `approach`, the single interaction cell for `strike`/`use`.
- **Deliberate duplication**: an `interact` objective's marker and the `anchor`
  it binds to are the same cell under two ids. They are different *things* —
  "the interact is occluded" and "the anchor is occluded" are different findings
  — so no deduplication is applied.
- **`screen_bbox`** is the projected inclusive cell box, clipped to the frame,
  in whole pixels with the origin top-left. This is the vocabulary spec-0015
  pillar 2 asks for: review feedback and edits address ids and boxes.
- **`occluded`** = every one of nine sight lines (the box centre plus the corners
  of a slightly inset box) meets a block that is **not part of the target**. Both
  refinements matter: a marker often *is* a block (`anchor/fire-pit` names the
  campfire), and a single centre ray grazing the rim of a platform would call the
  thing standing on it hidden.
- **`out_of_frame`** carries the same world-space fields, with no screen box, for
  every known target outside the frustum. It is what makes "the subject is absent
  entirely" machine-visible instead of something a reviewer has to notice.
- **Ordering** is `(kind, area, id)`; floats are rounded to 3 decimals.

**Determinism (ADR-0006)** — no RNG, clock, parallelism or hash-order iteration:
the voxel palette comes from a `BTreeMap` walk, targets are sorted, and the PNG
encoder (`compiler::png`) pins its DEFLATE level. Two runs on one input produce
byte-identical PNG **and** manifest; `crates/compiler/tests/snapshot.rs` asserts
both.

**Performance** (measured, `nobodys-cave-island`, release build, macOS/M-series,
single-threaded): assemble + voxel-grid flattening ≈ **30 ms**, a 960×540 frame
with `--labels` + manifest ≈ **190 ms**. `--timing` prints both to stderr (never
to the output, so it cannot affect byte-identity).

### `delvec blocking-chart`

```
delvec blocking-chart <campaign-dir> [-o <dir>] [--timing] [--json]
```

Per-elevation **cutaway** floor plans: one orthographic top-down PNG per
detected walkable band per area, plus `blocking-chart.json`. Default output
directory `blocking-chart/`. It answers the question a viewport structurally
cannot — *is there room* — so NPC crowding, a post blocking a doorway, or a
stealth zone lying across the only corridor are visible before the build exists.

**Cutaway, because there is no camera.** A roofed cavern cannot be photographed
from above, so the renderer simply excludes everything above the cut plane — a
dollhouse view straight from the voxel model. For a band whose walkable floor is
`Y`, each column of the area is drawn from the **topmost block in
`[Y-1, Y+3)`**: the floor a player stands on plus anything up to head height, so
a lintel and a waist-high obstacle both read, and no ceiling sneaks in.

**Bands are found, not declared.** Walkable cells (a BFS rooted at the area's
anchors, so a sealed void pocket never counts) are histogrammed by Y, and a band
is a local maximum of that histogram that

1. holds ≥ `BAND_MIN_CELLS` (6) cells, and
2. stands out from its neighbouring elevations by `BAND_RELIEF` (3×).

**Relief, not share** — this is the load-bearing choice. A share rule ("≥4% of
the area's walkable cells") makes a storey's status depend on how big the rest of
the *area* is, and the island's sheep pen — unambiguously a second floor — fails
it purely because the beach and meadow below are large. Relief asks the local
question instead: does walkable area *concentrate* here relative to what is
immediately above and below? A floor and a mezzanine do; a ramp, contributing one
or two transit cells per Y, never does. Maxima closer than `MIN_BAND_GAP` (3)
merge into the more populated one; at most `MAX_BANDS` (8) survive.

A **coverage pass** then guarantees the chart set is trustworthy: *every*
populated elevation must fall inside some band's cut. Relief finds storeys, but
rolling outdoor ground (the island's meadow climbing from beach to cave mouth)
has no storeys at all, and without this pass a walkable stretch would appear on
no chart at all with nothing to signal it. Uncovered elevations get fill-in
bands, lowest first, bypassing the merge rule — coverage outranks tidiness.

**Each slice is cropped** to its own band's walkable cells and markers (plus a
5-block margin), so a campaign whose single area runs from beach to mountain-top
gives the cavern its own tight frame at its own larger scale, instead of a small
drawing in a large field of void.

**Overlays**, in order: terrain flat-shaded by [`snapshot::block_color`] and
lightened with height within the cut (so a step reads as a step); a green wash on
the band's walkable cells; the DW0311-proven critical-path walk corridor as an
orange tint; then an outlined, labelled marker for every anchor, gate, NPC/actor
post, interact marker, stealth zone and trigger region whose elevation range
meets the cut. Labels use the same kind colours as `snapshot --labels` and the
same deterministic placer; a label that cannot be fitted on the plan is dropped
from the image but recorded in the index, never pushed off the edge. Routing is
best-effort — a campaign whose critical path does not route yet charts without
the corridor tint rather than refusing to chart.

Orientation is **+X right, +Z down (north up)**; each slice carries a title bar
naming its area, band index, floor Y and cut range.

**Index** (`blocking-chart.json`, `chart_version: 1`):

```json
{ "chart_version": 1, "campaign_id": "…", "delvec": "0.1.0",
  "orientation": "top-down orthographic; +X right, +Z down (north up)",
  "cut": "each slice draws world Y in [floor-1, floor+3) …",
  "areas": [ { "area": "area/island",
               "bounds": { "min": [x,y,z], "max": [x,y,z] },
               "walkable_cells": 1500,
               "bands": [ { "index": 0, "floor_y": 69, "walkable_cells": 420,
                            "y_range": [68, 71], "file": "island-band0-y69.png",
                            "width": 504, "height": 526,
                            "labelled": ["anchor/fire-pit", "…"] } ] } ] }
```

**Performance** (measured, `nobodys-cave-island`, release): **≈90 ms** for the
whole campaign's four slices, including the nav model and critical-path routing.

### `delvec edit apply` / `delvec edit preview` (spec-0017)

The map editor's write half, closing the loop with the read half above: edit
verb → deterministic replay → snapshot. Both subcommands run full validation
(exit 1 on any error — unlike the view commands, an edit session must not
build on a broken campaign), `Plan::build`, the checked replay (§4 "The map
editor edit stage"), then render **one labelled snapshot + manifest per
batch** into `-o` (default `edit-shots/`): the camera frames the batch's
edited AABB over the final edited world, dollhouse-style, pulled into open
air like `--at` (so an interior edit is viewed from inside its room). File
names are the batch's kebab (`batch/dress-floor` → `dress-floor.png` +
`dress-floor.manifest.json`).

After the snapshots, both subcommands run the **entire build-tier proof set** —
the DW02xx reachability analysis and `emit::build` itself, output discarded. The
per-batch invariants are only a subset (they miss `DW0308` cutscene clipping,
`DW0327` stealth zones, `DW0342` trap completability, `DW0312` wave seating,
`move-npc`/`move-actor` routability and the exported-route/POV self-checks), and
persisting on a subset let `apply` write a script the very next `build` rejects.
There is one proof tier: what `apply` accepts, `build` accepts.

`--batch <file>` appends one candidate `EditBatch` object (the `delvec schema
--stage 7` shape's batch element) to the script in memory. `apply` persists
the augmented script to `world-edits.json` (canonical 2-space form, trailing
newline) **only after the replay AND the build-tier proofs are green** — a red
candidate exits with its diagnostic and writes nothing, so a session can never
leave a broken script behind. The write is tmp + rename, so a crash mid-write
cannot truncate the artifact of record. `preview` is byte-for-byte the same run
but never writes to the campaign directory. `apply` without `--batch` replays +
re-renders only. Exit codes: 0 green · 1 validation · 2/3 replay or build-proof
failure by the failing code's tier (same mapping as `build`).

### PNG writing

`compiler::png` is a hand-rolled 8-bit RGBA writer shared by two callers, for the
same reason `compiler::resourcepack`'s ZIP/SHA-1 is hand-rolled — byte-stability
must be a function of this repo:

- `encode_rgba_stored` (uncompressed) — the `delve:art` font atlas, whose bytes
  are hashed into a shipped resource pack. Moved here verbatim from
  `compiler::atmos`; resource-pack bytes are unchanged.
- `encode_rgba` (DEFLATE at a pinned level, via the existing `flate2` dep) — the
  snapshot renders, which are megapixel review artifacts.

---

## 8. Cutscene rehearsal + shot calibration (spec-0019)

LLMs are bad at authoring camera positions as `anchor + offset` numbers — three
island QA rounds shipped shots that pointed the wrong way. spec-0019 moves the
judgement into the running game: the creator adjusts a **proposal** live and
harvests it once; the DSL stays the artifact of record.

**Landed (this reference describes only what `delvec` does today):** the shot
proposal in data storage, the calibration verbs that mutate it, the `dw.done`
harvest, `delve-harvest`'s `rehearsal-report.json`, and `delvec calibrate`.
**Not yet landed:** playback — the macro-function dolly, `dw.beat` / `dw.shot`
replay, `dw.free`, and the compiler-derived state-restore inverses.

### The proposal (`dw:rehearsal` storage, creator overlay only)

`compiler::rehearsal` enumerates every rehearsable **beat** (an effect bundle
containing a `cutscene`, at any nesting depth) and every **shot** inside it, in
campaign declaration order, giving each a 1-based id and the **JSON pointer**
that names its `cutscene` **effect** in the `quests` stage document, plus its
0-based index within that effect. The pointer names the effect and not the shot
on purpose: the single-shot spelling (`{path, seconds}`) and its one-entry
`shots` equivalent are the same cutscene and must emit byte-identical output
(`v06_cutscene::single_shot_spellings_are_byte_identical`), so a shot's identity
cannot depend on which spelling was used. A patch applies at
`<pointer>/shots/<index>` under the multi-shot spelling and at `<pointer>`
itself under the single-shot one. `compiler::creator` bakes
that inventory into the overlay:

- `creator/rehearsal/defaults` writes the compiled values into
  `dw:rehearsal base` (immutable) and copies them to `dw:rehearsal shots` (the
  live proposal). It runs from `#minecraft:load` **guarded on
  `unless data storage dw:rehearsal shots`**, so a `/reload` does not discard a
  proposal the creator is midway through.
- A campaign with no cutscene emits **no rehearsal artifacts at all** — the
  overlay is byte-identical to its pre-spec-0019 form, and no dead trigger is
  registered.

**Everything in the proposal is an integer block cell.** That is the DSL's own
granularity (a camera waypoint is `anchor + integer offset`, resolved by
`nav::anchor_offset_point` to `cell + 0.5`), so the write-back round trip is
lossless — the snap error is identically zero, not "small". It is also the only
NBT numeric type a function macro substitutes without a type suffix: a `double`
expands as `12.5d`, which is an unparseable argument to `say` and `tp`. Each
shot additionally carries `pstr`/`lstr`, the pre-formatted strings the harvest
stamp substitutes, maintained in lockstep with the numeric `path`/`look` by
every verb that writes them.

### Calibration verbs (trigger objectives, overlay only)

All take a **1-based** shot id (`-0` cannot express "reset shot 0"). All mutate
`dw:rehearsal` storage and nothing else — no datapack write, no world edit, no
campaign scoreboard — which is what lets adjust-and-replay cycle with no reload.

| Trigger | Effect |
|------|---------|
| `/trigger dw.mark set <s>` | Append the creator's **eye cell** as the next waypoint of shot `s`. The first mark after a (re)set *replaces* the compiled path (so "first call = start, second = end" reads true); later marks append. The eye cell is derived as `floor((Pos + eye height) × 1000 / 1000)` via scoreboard division, which floors correctly below `y=0`/`z=0` where plain `int 1` truncation would be off by one. |
| `/trigger dw.mark set -<s>` | Reset shot `s` to its compiled values (`base[s]`). |
| `/trigger dw.aim set <s>` | Set shot `s`'s `look_at` to the block the creator is looking at. A **bounded, one-shot** ray — `execute anchored eyes positioned ^ ^ ^0.25`, 256 steps ≈ 64 blocks, run on demand, never polled — whose hit cell is read back off a `marker` summoned and killed inside the same command chain (vanilla has no position→score primitive). |
| `/trigger dw.faster set <s>` / `dw.slower set <s>` | Scale `seconds` by ∓20 % with a floor of one whole second, clamped to 2..30. The one-second floor is why the step is `max(1, 20 %)`: plain integer scaling leaves a 2 s shot at its fixpoint forever. |
| `/trigger dw.done` | The single harvest — one `[DelveShot]` line per shot. |

The overlay also `say`-stamps a one-line `[DelveShotRoster]` the first time each
player joins, mapping shot ids to their JSON pointers; without it the creator
has no way to know what `dw.mark set 3` addresses.

**A `trigger` objective is armed by its score entry, so `scoreboard players
reset` disarms it.** Vanilla stores "this player may `/trigger` this objective"
as a lock flag on the score entry itself; deleting the entry deletes the
permission, and `scoreboard players enable` re-creates it at `0`. A tick that
both `enable`s an objective and `reset`s it therefore leaves it permanently
unusable: every `/trigger` answers *"You cannot trigger this objective yet"* to
the player and writes **nothing** to the server log — so no report, no PackTest
assertion and no amount of reading the emitted commands makes it visible. This
cost a live debugging round: a per-tick hygiene clause clearing the no-op value
(`scores={dw.mark=0}`) matched the entry `enable` had just created, so every
adjust verb was silently refused while `dw.done` — which had no such clause —
worked. **A fired trigger is cleared inside its handler, never in the tick**;
the next tick's `enable` re-arms it. Pinned by
`rehearsal::the_tick_never_resets_a_trigger_it_arms`, which fails the build's
tests if any overlay function ever again arms and disarms the same objective.

### The harvest stamp

```text
[DelveShot] shot=<n> beat=<n> ptr=<json-pointer> idx=<n> seconds=<n> look_at=<x,y,z|none> path=<x,y,z;…>
```

`say`, not `tellraw` — the same channel and the same reason as `[DelveNote]`
(spec-0006 §3): a system message to players never reaches the server stdout log
the harvester reads. `shot`/`beat`/`ptr`/`idx` are compile-time constants, so a
harvested proposal always knows which DSL node its patch belongs on; only the
live values are macro-substituted.

### `rehearsal-report.json` (`delve-harvest`)

The same harvest pass that writes `playtest-report.json` also parses
`[DelveShot]` lines into a versioned `rehearsal-report.json` **beside** it,
written only when the session actually stamped a proposal. Schema version
`0.1.0`; per shot: `shot`, `beat`, `pointer`, `shot_index`, `path`, `look_at`, `seconds`, the
stamp's `at` timestamp, and `stamps` (how many times that shot was stamped).
`dw.done` fired twice keeps the **last** reading — the creator's final word — so
a report can never silently mix an early and a late state of one loop.

### `delvec calibrate`

```
delvec calibrate <rehearsal-report.json> --layout <creator-datapack/layout.json> [-o shot-patch.json]
```

Snaps every proposal cell to the **nearest declared anchor** within
`SNAP_RADIUS` (16 blocks) and emits `anchor + integer offset` — a zero offset is
spelled as a bare `{"anchor": …}`, exactly as the DSL does. Ties break on anchor
id, so the converter is a pure function of its inputs (ADR-0006). The
resolved-anchor vocabulary comes from `creator-datapack/layout.json`, which
spec-0019 extended with an `anchors` array (`id`, `area`, `kind`, resolved
`pos`) and a `shots` roster; it lives there rather than in a new build output
because it is a creator-loop artifact and the shipped image never carries
`creator-datapack/`.

The patch is **never applied here**: nothing writes to a stage document from the
game. The agent applies it, reruns `delvec build`, and the normal proofs
(`DW0308` air corridors, `DW0347` angular budget) gate the result exactly as
they gate a hand-written shot.
