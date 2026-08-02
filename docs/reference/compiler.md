# delvec compiler — behavior reference

**Single authoritative record of *current* compiler behavior.** Specs
(`docs/specs/`) remain the historical decision records; this file is what the
compiler does today. A PR that changes compiler behavior updates this file in the
same PR (CLAUDE.md Methodology; CI enforces the DW-code subset — see
`tools/check-dw-codes.py`).

- Binary: `delvec` (`crates/compiler`, Rust-native, ADR-0011).
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
| 1 | Load campaign dir (6 stage docs + `l10n/` sidecars) | `compiler::load` | internal (≥10) on unreadable dir |
| 2 | Parse (serde, `deny_unknown_fields`) | `dsl::parse_campaign` | `DW0100` (exit 1) |
| 3 | Validate stages 1–6 (schema + referential, full injected registries) | `dsl::validate_campaign_with` | `DW01xx` (exit 1) |
| 4 | l10n sidecar coverage | `dsl::validate_l10n` | `DW0180`/`DW0181` (exit 1) |
| 5 | Analyze (deep quest/dialogue reachability) | `compiler::analyze` | `DW02xx` (exit 2) |
| 6 | Solve jigsaw layout (per `prefab_pool` area, from seed) | `compiler::solver` | `DW030x` (exit 3) |
| 7 | Assemble world model (placed pieces → voxel grid; ocean sea-level datum check) | `compiler::plan` | `DW030x`/`DW0344` (exit 3) |
| 8 | Assembled-light + relight (measure, place fixtures) | `compiler::light` | `DW0210`/`DW0211` (**exit 2**) |
| 9 | Nav checks (A* `move-npc`/`move-actor` (footprint-aware), cutscene clip, critical-path walkability — incl. relight fixtures + water flood; talk-to endpoint snap; waypoint self-check; POV camera clear-eye self-check; v0.6 checkpoint no-stranding/placement + stealth-zone + trap completability proofs) | `compiler::nav` | `DW0307`/`DW0308`/`DW0311`/`DW0314`/`DW0315`/`DW0316`/`DW0325`/`DW0327`/`DW0342`/`DW0724` (exit 3; `DW0342` → exit 2) |
| 10 | Emit (datapack, packtest, server, critical-path, resourcepack) | `compiler::emit` | `DW0300`+ (exit 3) |

- `build` ⟹ `validate` + `analyze`; `analyze` ⟹ `validate`. A validation failure
  short-circuits (exit 1) before analysis; analysis failure (exit 2) before build.
- The assembled-light gate (`DW0210`/`DW0211`) runs inside the build (it needs the
  placed geometry) but is analysis-tier: `main` maps a `DW02xx` build diagnostic to
  **exit 2**. Its relight fixtures feed both `setup_finish` emission and the nav
  re-verification in pass 9.
- Every emitted `.mcfunction` line is checked against the vendored 1.21.11
  Brigadier tree (`compiler::commands`; structure-only — arity/paths, not arg
  values). mecha re-validates in CI (ADR-0011); disagreement fails CI.
- Determinism (ADR-0006): all map/set iteration is `BTreeMap`/sorted; the only
  randomness is stage-1 `seed` → a named splitmix64 per-area stream.

### CLI contract

```
delvec validate <dir>                      # stages 1–6 schema + referential
delvec analyze  <dir>                      # + quest-graph reachability
delvec build    <dir> -o <out>             # full deterministic build
delvec schema   --stage <1..6|all>         # export JSON Schema
delvec l10n-inventory <dir> [--lang <c>]   # l10n key inventory as JSON (translation input)
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
on something outside the campaign — currently only `DW0330`, where how much text
fits depends on the player's window size and GUI scale. Every other code is `error`
and rejects as before.

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
| `boundary {margin?,message?}` (opt) | spec-0013: declares a **derived** playable region (union of final placed-piece AABBs, inflated horizontally by `margin` (`0..=64`, default 16; else `DW0321`), unbounded up, floor = lowest placed block − 8). A 1s clock returns any player outside it to the last checkpoint (`dw:cp`) with an actionbar `message` (l10n `world.boundary.message`, English default when absent) + a soft sound; no damage, no item loss. `horizon:"ocean"` without a `boundary` = `DW0320`. | 0.6 |

### Stage 2 — `npcs` (casting sheets, stationary)

| Field | Behavior | Since |
|-------|----------|-------|
| `id`,`name`,`area`,`anchor`,`base_entity` | NPC body placed at resolved anchor; `name` → l10n `npc.<n>.name`. | 0.1 |
| `role` | Enum `quest-giver|flavor`; `vendor`/`boss` reserved → `DW0141`. | 0.2 |
| `persona{archetype,speech_style,motivation,…,relationships[]}` | Structured; **excluded** from l10n; relationship refs validated in-stage (`DW0112`). | 0.2 |
| `skin{texture_id,model}` (opt) | Switches body to `minecraft:mannequin`; PNG baked to resourcepack. Missing PNG → `DW0309`; bad/dup id → `DW0190`. | 0.4 |
| `deferred` (opt, bool) | **Not** summoned at world init; the NPC's body + hitbox appear only when a `spawn-npc` effect fires, at this same `anchor` (the dual of `despawn-npc`). Default `false` = pre-0.6 behavior, byte-identical. Never spawned → `DW0197`; a `talk-to` provably ahead of every spawn → `DW0198`. | 0.6 |

### Stage 3 — `classes`

1..4 classes. `kit[]` = vanilla item id + count + optional display `name`
(→ l10n `class.<c>.kit.<i>.name`). `name`/`blurb` player-visible. Reserved kit
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
| `waves[]` | `{id,anchor,mobs[{entity,count,name?,attributes?,effects?,equipment?}]}`; entity validated (`DW0173`); `attributes`/`effects` are v0.4 (`DW0192`). `equipment{head?,chest?,legs?,feet?,main_hand?,off_hand?}` is v0.6 (task #65; reserved `DW0141` pre-0.6): slot item ids validate against the pinned 1.21.11 item registry (`DW0143`, the give-item family); emitted as component-era `equipment`/`drop_chances` summon NBT (never legacy `ArmorItems`/`HandItems` — 1.21.11 ignores them) with **drop chance 0 on every slot** (no-grind: wave gear is never lootable). Explicit slots merge over the armed-mob main-hand default (a helmeted skeleton keeps its bow; explicit `main_hand` overrides). A helmet is the sanctioned daylight-undead fix — never `set-time`. | 0.3 / tuning 0.4 / equipment 0.6 |
| `triggers[]` | `{id,at,on:strike\|use\|approach{range},requires_flags?,once?,effects[]}` (v0.4). Bad/dup/`range 0` → `DW0194`. | 0.4 |
| Effect `open-gate` | Fills gate anchor to air. | 0.1 |
| Effect `close-gate{anchor}` | The physical dual of `open-gate` (v0.6): fills the gate anchor's region with the block the anchor's prefab metadata declares (basalt boulder, iron bars), re-sealing an opened threshold into a wall. A gate anchor that declares no `block` is `DW0343`. Same anchor-existence check as `open-gate` (`DW0142`). Per-effect `requires_flags` like the other per-`@s` verbs. | 0.6 |
| Effect `campaign-complete` | Sets `dw.campaign`; finale fanfare. | 0.1 |
| Effect `spawn-wave` | Summons wave mobs (AI on), tag `dw_wave_<id>`. | 0.3 |
| Effect `give-item{item,count,name?}` | Grants item (`name` v0.4). | 0.3 |
| Effect `set-flag{flag}` | Sets `dw.f_<flag>` (per-player). | 0.3 |
| Effect `narrate{text,style?,sound?}` | chat/title/subtitle/**art**; `text` → l10n; `sound` validated (`DW0326`); `art` = the `delve:art` pixel-banner font, glyph-checked (`DW0328`), width-checked (`DW0330`). | 0.4 / art 0.6 |
| Effect `set-block{anchor,block}` | `setblock` at anchor; base block id validated (`DW0193`). `block` accepts a verbatim blockstate suffix `id[key=value,…]` (v0.6). | 0.4 / state 0.6 |
| Effect `requires_flags[]` (any effect) | Per-effect AND-gate (v0.6): wraps the effect's command(s) in a per-player `execute if score @s dw.f_<flag> matches 1 run …`. Valid on any `on_objective_complete` / `on_complete` / trigger effect **except** terminal `campaign-complete`; refs resolve like objective flags (`DW0172`). | 0.6 |
| Effect `despawn-npc{npc}` | Removes NPC + hitbox. | 0.4 |
| Effect `spawn-npc{npc}` | The dual of `despawn-npc` (v0.6): summons a `deferred` stage-2 NPC — body + interaction hitbox + name display — at its declared anchor, via the **same** `npc_summon_commands` authority world init uses. Idempotent (per-entity tag guards), so a re-fire never doubles a body. Also a dialogue effect. World-global staging → no per-effect `requires_flags`. | 0.6 |
| Effect `move-npc{npc,to_anchor,speed?}` | A*-planned per-tick tp through walkable space; unroutable → `DW0307`. | 0.4 |
| Effect `cutscene{shots[]}` / `cutscene{path[],seconds,look_at?}` | Two-camera spectator dolly; clip → `DW0308` (checked **per shot**). Two mutually exclusive spellings, normalized to one shot list: multi-shot `shots: [{path[],seconds,look_at?}, …]` (v0.6) or the single-shot `path`+`seconds` fields (v0.4) — mixing/omitting both, or a shot with an empty `path`, is `DW0199`. Shots play back-to-back inside ONE save/restore bracket (hard cut). `look_at {anchor,offset?}` aims every dolly camera at that world point; absent = face along the direction of travel. | 0.4 / `look_at`+`shots` 0.6 |
| Effect `set-time{time}` | Instantaneous dimension-global cut (`time set <kw>`); persists (cycle frozen). | 0.5 |
| Effect `set-weather{weather}` | Instantaneous dimension-global cut (`weather <kw>`); persists (cycle frozen). | 0.5 |
| Effect `play-sound{sound,at?,volume?,pitch?}` | Plays a sound event; `sound` validated (`DW0326`); `at` = `{anchor}`\|`players` (default)\|`{actor}` (deferred → `DW0335`); positional or per-player. | 0.6 |
| Effect `damage-players{amount,in?,damage_type?}` | Deals `amount` half-hearts of damage to the acting player(s) — a real `on_caught`/souls consequence over vanilla `/damage` (`damage @s …`). Per-`@s`: top-level hits every player once, in `on_caught` the caught player. `amount ≥ 40` is lethal through golden apples. `in {anchor,extent}` narrows to acting players inside the anchor-centred box (same box model as a stealth zone; anchor `DW0142`). `damage_type` is a **curated enum** of vanilla types that respect `keepInventory` and do NOT bypass totems (no `out_of_world`/`generic_kill`), default `generic`; an unknown value is `DW0100` (needs no registry). Named `damage_type`, not `type`, since the effect enum is internally tagged on `type`. Per-effect `requires_flags` allowed (per-`@s` verb). Every form is guarded by `tag=!dw_cutscene` — a player watching a cutscene is never harmed (§4). | 0.6 |
| Effect `set-checkpoint{anchor,on_respawn?}` | Party-wide respawn point: `spawnpoint @a` at the anchor + `storage dw:cp pos` mirror. Monotonic by quest order. `on_respawn[]` = per-player effects re-run on respawn while active (vanilla `deathCount` detection). Proofs `DW0315`/`DW0316`. Also a dialogue effect. | 0.6 |
| Effect `begin-stealth{zones[{anchor,extent}],on_caught?,grace_ticks?}` | Per-tick: every player must be inside some zone **and** sneaking (`sneak_time` stat); exposed for `grace_ticks` (default 20) → `on_caught`. Zone standable/reachable proof `DW0327`. | 0.6 |
| Effect `end-stealth` | Ends the active stealth beat (clears the session marker). | 0.6 |
| Stage-5 `actors[] {id,entity,name?,skin?,anchor,facing?,vulnerable?}` | Scripted NoAI/Silent/no-loot puppets, tag `dw_actor_<id>` (+ puppet marker `dw_pup_<id>`); `Invulnerable` unless `vulnerable` (then knockback-immune); `skin` → mannequin. Summoned by `spawn-actor`, not at load. | 0.6 |
| Effect `spawn-actor{actor}` | Idempotent puppet summon at the actor's anchor. | 0.6 |
| Effect `despawn-actor{actor,style}` | `kill` = vanilla death animation in place; `vanish` = relocate-then-kill (silent, out of view). Targets `dw_actor_<id>` (puppet or twin). | 0.6 |
| Effect `move-actor{actor,to_anchor,speed?,on_arrive[]}` | Footprint-aware A*-planned per-tick tp of the puppet, yaw along the path tangent; `on_arrive` fires at the destination cell; unroutable → `DW0325`. `move-npc` is a thin wrapper over the same planner (player footprint). | 0.6 |
| Effect `unleash-actor{actor}` | Replaces the puppet with a real-AI twin (same entity/pos/name/tag, no puppet marker). Re-caging = `despawn-actor` + `spawn-actor`. | 0.6 |
| Effect `sequence{steps[]{at_ticks,effects[]}}` | Deterministic timeline: one schedule chain firing effect groups at exact tick offsets. No nested `sequence` → `DW0329`. Effects nested in a step are **first-class**: the flag/wave producer scans, the checkpoint/stealth collector, the l10n inventory, and emission all descend into `sequence.steps` and every nested effect list (`on_respawn`/`on_caught`/`on_arrive`) via one shared traversal, so a `set-flag`/`set-checkpoint` nested in a step produces its flag / registers its indexed checkpoint exactly as at top level. | 0.6 |
| `traps[]` | spec-0011: `{id,at,trigger,effect,lethality?,disarm?,reset?,requires_flags?}`. `at` binds an `anchor/trap` prefab marker (the trigger/hazard cell; its `dispenser` metadata cell holds the payload socket). `trigger` ∈ `pressure-plate`/`tripwire`/`trapped-chest` (all redstone-native; `trapped-chest` = the only player-distinct trigger). `effect` = `{dispense:{item,count}}` (item `DW0341`; a non-`dispense` key e.g. `tnt` is an unknown variant → `DW0100`). `lethality` ∈ `lethal`/`harmful`(default)/`nonlethal`. `disarm{via,sets_flag}` = a reachable affordance that turns the trap off. `reset` ∈ `once`/`rearm`(default). Structural errors `DW0340`; a lethal forced-path trap without discharge `DW0342`. Reserved (`DW0141`) before 0.6. | 0.6 |

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
per-effect `requires_flags`, stage-2 `deferred` + the `spawn-npc` effect, stage-5
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
(`DW0191`) completing option. Node `text` → l10n `dlg.<n>.<node>.text`, option
labels → `.opt.<i>.label`. **Display gating (v0.4+, task #54):** an option is
*shown* only when clicking it would fire — every `requires_flags` set (flag axis)
and every completed objective active, i.e. `dw.qa_<quest>==1` and
`dw.o_<obj>!=1` (objective-state axis) — so `DW0191`'s ungated completing option
is visible exactly while its objective is active (the guarantee holds
automatically).

### l10n sidecars (`l10n/<code>.json`)

Envelope `{dsl_version,campaign_id,kind:"l10n",lang,content}`; `content` = flat
**stable key → translated string**. Key inventory derived from stage docs
(`world.title`, `area.<a>.name`, `class.<c>.name/.blurb/.kit.<i>.name`,
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
| dialog display gating (v0.4+) | A node with any display-gated option (`requires_flags` and/or `completes`) emits one `__m<mask>` variant per per-node availability bitmask + a chooser: `dmask_<n>_<node>` sets `dw.dmask` (bit `i` = the node's i-th gated option is displayable — its flags set and every completed objective active: `if …qa_<q>==1 unless …o_<obj>==1`), then `show_<n>_<node>` `dialog show`s the matching variant. Ungated nodes/options `dialog show` directly (v0.2/v0.3 byte-identical). Click handler keeps its own guard (defense-in-depth for the `/trigger` path). The `v04_dialogue_visibility` PackTest asserts the option-under-test's **isolated** bit (`(dmask>>bit)&1` via `%= 2^(bit+1)` then `/= 2^bit`), never the whole `dw.dmask` — sibling options in a node can share a `qa_<q>` score, so a whole-mask compare would read a sibling's bit as this option's. |
| dialogue trigger re-arm | `dlg_<npc>_<n>` consumes the trigger with `scoreboard players reset @s dw.dlg_<npc>` — which also **re-locks** it — and therefore re-arms it in the very next line (`scoreboard players enable @s dw.dlg_<npc>`), before the flag gate's `return fail` and before any `dialog show`. The per-tick `scoreboard players enable @a` stays as belt-and-braces but cannot close the window on its own: 1.21.9+ **freezes the integrated (singleplayer) server while a screen is open**, and the handler's last act is to show the next node, so ticking stops with the trigger locked and the player's next click is executed the instant ticking resumes — before the tick function — and vanilla rejects it ("You can't trigger this objective yet"), silently swallowing one dialogue choice. A dedicated server never pauses, so no rung of the validation ladder can reproduce it. The generated `dialogue_trigger_rearm` PackTest drives a terminal option and uses the trigger twice **with the tick function never run in between** — that suppression is the freeze. |
| class select | Dialog button → `/trigger dw.class set <n>`. |
| `reach-anchor` | Per-tick `execute if entity @s[box ±1 on each axis]`; glowing `end_rod` `item_display` marker (tag `dw_r_<obj>`), labeled with the objective `title` — an **untitled** objective gets a nameless glowing marker, never a raw-id label. Completion despawns the marker (`kill @e[tag=dw_r_<obj>]`). |
| `kill` / `spawn-wave` | `spawn-wave` summons mobs (AI on) tag `dw_wave_<id>`, countdown `#<id> dw.wave`; `player_killed_entity` advancement decrements; `kill` completes at 0. Armed species get `equipment` NBT (drop 0): `wither_skeleton→stone_sword`, `skeleton`/`stray→bow`. **Mob placement (task #41):** each mob is seated on a distinct compiler-validated standable cell (2-tall clearance, solid floor) chosen by a deterministic BFS outward from the wave anchor over the assembled occupancy world (`compiler::nav`), ordered by ascending BFS distance with a fixed `(y,z,x)` tie-break. The flood-fill is confined to the anchor's own assembled piece, so a flock never crosses a socket seam into a neighbouring room. A wave needing more footing than its room offers is `DW0312` (never `+x`-strung mobs piling into blocks or spilling toward void). |
| `collect` | Chest at anchor pre-loaded `count×item`; `inventory_changed` advancement runs guarded completion. |
| `interact` | `minecraft:interaction` (tag `dw_i_<obj>`) + `player_interacted_with_entity` advancement + `/trigger dw.i_<obj>`; `requires_item` = `execute if items`; glowing lantern `item_display` marker (also tag `dw_i_<obj>`, only when no `prop`), labeled with the objective `title` — untitled → nameless glow, never a raw-id label. `prop{block}` = `setblock` affordance. Completion despawns both entities (`kill @e[tag=dw_i_<obj>]`) so a finished objective is not clickable; the `prop` block persists as scenery. |
| environment `triggers[]` (v0.4) | `setup_finish` summons one `minecraft:interaction` per `strike`/`use` trigger at its `at` anchor (tag `dw_trig_<id>`); `approach` needs no entity. `tick`: `strike` fires on `nbt={attack:{}}`, `use` on `nbt={interaction:{}}`, then clears the record (`data remove entity @s <field>`); `approach` is a `distance=..<range>` selector. `once` guards on `#trig_<id> dw.sys`. **Strike on an NPC's anchor:** when a `strike` trigger's `at` is also where an NPC stands, the NPC's own interaction hitbox additionally carries `dw_trig_<id>`, so the trigger's single selector watches both entities. Two interaction entities share the cell and a left-click reaches only whichever the attack raycast finds first — an order the compiler does not control and that changes across chunk reloads — while the NPC's body is `Invulnerable`, so the swing could land where nothing was watching and the trigger never fire (round-4 island QA). Scoped to `strike`: right-click on an NPC already belongs to the dialogue advancement, so a co-located `use` trigger is an authoring conflict, not a detection bug, and is left alone. Note the shared tag rides the NPC's hitbox, so a `move-npc`'d NPC carries the strike target with it, and `despawn-npc` removes it (the trigger's own hitbox remains). A generated `v04_strike_npc` PackTest writes the vanilla `attack` compound onto the NPC's hitbox and asserts the trigger fires, once, and that the record is consumed. |
| `set-flag` / `requires_flags` | `dw.f_<flag>` scoreboard (per-player); required flags AND-ed into objective guards (layered on `after`). **Per-effect** `requires_flags` (v0.6) wraps each of the effect's emitted commands in `execute if score @s dw.f_<flag> matches 1 [… per flag] run <cmd>`; these effect functions already run per-player (`complete_<obj>` / `trig_<id>` are entered `as @a`/`@s`), and an ungated effect is emitted verbatim (byte-identical). |
| `open-gate` | `/fill … air` over the gate region. |
| `close-gate` | `/fill <region> <block>` over the gate region with the anchor's declared fill block (no `replace` clause — the dual of `open-gate`). |
| `give-item` | Grants item to player (`name` → SNBT text component). |
| `narrate` | chat / `title` / `subtitle` (+ optional sound); `art` = `title` with a `{"font":"delve:art"}` text component, rendered uppercase in the pixel-banner font (6 font px/glyph → ~15 glyphs fit; see [The `delve:art` font](#the-delveart-font)). |
| `play-sound` | `playsound <sound> master @s [<pos>] [<vol> [<pitch>]]` — effects run `as @a`, so `@s` is each player: `anchor` uses the resolved anchor pos (all hear it there), `players` uses `~ ~ ~`. |
| `damage-players` | `damage @s <amount> <type>` (per-`@s`; default `minecraft:generic`). With `in`: `execute if entity @s[x=…,dx=2·ext,…] run damage @s …` (the stealth-zone box model, so it stays per-`@s` — no double-hit). A generated `v06_damage` PackTest summons a tagged dummy, applies the declared amount+type, and asserts its `Health` strictly dropped. |
| `set-block` | `setblock` at resolved anchor. |
| `despawn-npc` | Kills body + interaction hitbox. The generated `v04_despawn` PackTest targets the campaign's first `despawn-npc` NPC; when that NPC is **deferred** it runs its `spawn_npc_<id>` entrance right after `setup_finish` (a deferred NPC is deliberately absent from world init, so the presence assertion would otherwise read 0). The assertions themselves — 2 entities present, 0 after the kill — are identical in both cases, and the entrance line is emitted only for a deferred target, so a campaign with no deferred NPC keeps byte-identical PackTest output. |
| `spawn-npc` | `function <ns>:spawn_npc_<npc>` — the generated entrance function, emitted once per **deferred** NPC. Its two lines are the world-init summons, each independently guarded: body by `unless entity @e[tag=dw_npc,tag=dw_npc_<n>]`, hitbox by `unless entity @e[tag=dw_npc_<n>,tag=!dw_npc]` (both carry the id tag, so a single shared guard would let the body's own summon suppress the hitbox). The `npc_summons` PackTest fires each deferred NPC's entrance after `setup_finish` and asserts exactly one body. |
| `move-npc` | Per-tick tp along A*-planned walkable waypoints (hitbox in lockstep), at cell **centres** with L-shaped vertical steps — see §4 "Entity placement". |
| `cutscene` | Per player: save gamemode+pos → spectator → alternate `spectate` between two co-located dolly cameras each tick → restore. Every dolly `tp` carries an explicit `<yaw> <pitch>` — **Minecraft** entity rotation (`yaw = atan2(-dx, dz)`, 0 = +Z south; `pitch = atan2(-dy, hypot(dx,dz))`, + = down), *not* the render-plan/Chunky yaw convention — computed at emission from the camera's own position: at the shot's `look_at` subject if it has one, else along the polyline segment being traversed. Never the summon default (yaw 0 = south). Rounded to 3 decimals, `-0.0` collapsed to `0.0`, so emission is byte-stable. The bracket also arms the `dw_cutscene` state on every player and releases it on restore — see §4 "A cutscene is pure observation". Multi-shot: all shots share one `#t_<bare>` counter — shot *k* owns `[offset_k, offset_k+len_k]` and the next starts at `offset_k+len_k+1` (hard cut); one marker, one `gamemode spectator @a`, one camera pair, one restore. A one-shot cutscene reduces to the pre-multi-shot timeline, so both single-shot spellings are byte-identical. `critical-path.json`'s `cutscene_seconds` is the **total** across shots. Function key = `cs_<first anchor>_<seconds>_<waypoints>`, plus an 8-hex sha256 digest of the whole normalized shot list whenever the cutscene is not a bare single shot without `look_at` (the key must be injective — two shots sharing a first waypoint must never collapse onto one function). |
| `campaign-complete` | `dw.campaign` = 1 (dummy objective, **never on the sidebar** — a raw internal id must not surface to players); broadcast `[Delvewright] complete dw.campaign 1` (dark-gray bot channel, the harness's completion signal); title fanfare. |
| objective lifecycle | Activation shows `title`+`hint`+`note_block.pling` once (flag `dw.ann_<obj>`); completion sound `experience_orb.pickup`. **Marker cleanup (task #45):** completion despawns every entity the objective's activation summoned via the objective-scoped tag — `interact` hitbox + wayfinding marker (`dw_i_<obj>`), `reach` marker (`dw_r_<obj>`). Prop/affordance *blocks* (`interact.prop`, `collect` chest) are scenery and persist; `talk-to`/`kill` summon no per-objective marker. Gated on v0.3+ with a resolved activation, so v0.2 stays byte-identical. |
| `set-time` / `set-weather` | `time set <kw>` / `weather <kw>` (dimension-global, no selector) inline in the effect/dialogue-option function; instantaneous cut, persists (cycle frozen). |
| relight fixtures (`lighting`) | `setblock` per placed fixture in `setup_finish`, after structure placement + socket seals (spec-0010). Blocks: `torch`/`wall_torch`, `lantern[hanging=…]`, `campfire[lit=true]`, `shroomlight`. |
| `mitigation: "night-vision"` | `night_vision_tick`: one `effect give @a[x=…,dx=…,y=…,dy=…,z=…,dz=…] minecraft:night_vision 12 0 true` per declaring area (selector = the area's final placed bounds, compile-time literals), then `schedule function <ns>:night_vision_tick 20t` (vanilla replace-mode, so the clock can never double up). `setup_finish` arms it once. A generated `v06_night_vision` PackTest teleports a dummy into the declared bounds, runs one clock tick and asserts it holds the effect — then teleports it 1000 blocks out and asserts it does not. |
| `set-checkpoint` | Inline: `spawnpoint @a <x y z>` + `data modify storage dw:cp pos set value [x,y,z]` (the readable "last checkpoint" mirror); when any checkpoint has `on_respawn`, also `#cp dw.sys = <index>`. `setup_finish` seeds `dw:cp` to the spawn cell. `on_respawn`: `deathCount` objective (`dw.deaths`) + per-player ack; `tick` runs `cp_respawn_check` (fire on the death-count edge, dispatch `cp_on_respawn_<index>` for the active checkpoint). |
| `begin-stealth` / `end-stealth` | `begin` → `#stealth dw.sys = <session>` + reset per-player `dw.st_grace`/`dw.st_sneakack`. `tick` runs `stealth_tick_<session>` while active → per-player `stealth_eval_<session>`: safe iff sneaking this tick (`dw.st_sneak`=`sneak_time` stat rose vs. ack) AND in a zone box; grace resets when safe, climbs when exposed, and at `grace_ticks` fires `stealth_caught_<session>` (`on_caught`). `end` → `#stealth dw.sys = 0`. The `v06_stealth` PackTest disarms `#stealth` (sets it 0) after each `stealth_begin` because it drives `stealth_eval` explicitly: an armed session would make the world `tick` loop run a *second* judge pass in the same tick, consuming the sneak edge and mis-accruing grace (this only isolates the test; runtime gameplay has the tick loop as sole caller). It pins its dummy by tag (see "PackTest batch model" below), runs the spare (safe-player) section first and the `on_caught` trip LAST — the trip executes arbitrary campaign `on_caught` content (possibly lethal), so nothing state-dependent follows it and the closing assert reads the dummy through the tag, which keeps matching even if the trip killed it. |
| trap `dispense` (spec-0011) | `setup_finish`: `item replace block <disp> container.0 with <item> <count>` fills the prefab's pre-wired dispenser socket (the `anchor/trap` metadata `dispenser` cell) — a static, deterministic payload, the same mechanism as a `collect` chest. **No detection** is emitted for the harm: the plate/tripwire/trapped-chest → dispenser redstone is already in the prefab. Pressure plates and tripwire are modelled **passable** in the assembled occupancy (`crate::assembled::is_passable_trap_trigger`) so nav routes a player ONTO a trigger cell rather than around a "solid" plate. |
| trap `disarm` (spec-0011) | `setup_finish` summons a `minecraft:interaction` at the disarm `via` cell (tag `dw_trapdis_<trap>`); `tick` fires `trap_disarm_<trap>` once on a right-click (`nbt={interaction:{}}`, reusing the v0.4 `use` primitive). `trap_disarm_<trap>` sets the party-wide `dw.f_<flag>` and empties the dispenser (`data modify block <disp> Items set value []`) — the modeled, global disarm that actually stops a redstone dispense trap. |

Naming: `dw.o_<obj>`, `dw.q_<quest>`, `dw.qa_<quest>` (active), `dw.dlg_<npc>`,
`dw.f_<flag>`, tags `dw_npc_<npc>`/`dw_wave_<id>`/`dw_i_<obj>`/`dw_r_<obj>`.
`CustomName` is a plain SNBT text component (not `'{"text":…}'`).
v0.6 checkpoints/stealth (spec-0012/0014): storage `dw:cp pos` (last-checkpoint
mirror, a `[x,y,z]` int list); scores `dw.deaths` (`deathCount`) + `dw.death_ack`,
`dw.st_sneak` (`sneak_time` stat) + `dw.st_sneakack`/`dw.st_grace`/`dw.st_safe`;
markers `#cp`/`#stealth` on `dw.sys`.

---

## 4. Hard invariants

### Semantics never key on player-facing text

**No semantic verdict may key on player-facing free text** (item/NPC display names,
titles, blurbs, hints). Semantics live only in ids, structured schema fields, or
first-class declarations. The removed night-vision name heuristic (`light.rs`,
deleted in the v0.6 mitigation PR) is the cautionary precedent: it read a kit item's
display name for "night vision", so a renamed water bottle passed `DW0210` while
nothing in the shipped world granted night vision — a check that passed without the
feature existing. Player-facing text is also localizable, so keying on it makes a
verdict language-dependent (ADR-0006).

### A cutscene is pure observation (`dw_cutscene`)

While a cutscene plays, every player carries the entity tag `dw_cutscene` —
added by the cutscene `start` alongside `gamemode spectator @a`, removed by the
`end`/restore, so the state has exactly the cinematic's lifetime. **Campaign
machinery must neither require anything of a tagged player nor punish them:**
they are watching, not playing. Current consumers:

- the **stealth judge** is skipped for them (`stealth_tick` selects
  `@a[tag=!dw_cutscene]`). The judge is the only writer of `dw.st_grace`, so
  skipping it freezes the clock — grace neither accrues nor expires, and
  `on_caught` cannot fire mid-cinematic. The restore re-acknowledges the vanilla
  `sneak_time` stat (`dw.st_sneakack = dw.st_sneak`) so the first tick after the
  cut compares against the current stat, and deliberately leaves `dw.st_grace`
  alone: the beat resumes exactly where it paused.
- **`damage-players`** skips them: every form of the verb is guarded by
  `tag=!dw_cutscene`.

Any future verb that *demands input* or *deals harm* joins this list. The origin
is a round-4 island playtest where the stealth clock kept running through a dolly
and the catch killed the owner mid-shot, desyncing the beat.

### PackTest batch model (one dummy per test, one shared server)

PackTest runs the whole generated suite as **one batch on one shared server**:
every `# @dummy` test spawns its **own** dummy player, all dummies coexist, and
all test functions execute over the same server tick(s), sequentially in an
order the compiler does not control. Two authorship rules for emitted templates
follow (round-5 island reds; `pin_dummy` in `emit.rs`):

- **`@p` is not "the test's player".** It re-resolves from the test structure
  origin on every command — the moment a template teleports its dummy to
  absolute campaign coordinates, `@p` retargets to a *neighbor test's* dummy
  and later writes/asserts land on the wrong player (`v06_stealth` read a
  foreign dummy's grace). A template that drives per-player state must tag its
  dummy on its first post-setup line (`tag @p add dw_<test>` — while its own
  dummy, inside its own structure, is still the nearest player) and address it
  exclusively via `@a[tag=…,limit=1]`, which — unlike `@p` — also keeps
  matching a dummy that campaign content has killed.
- **`@a` writes leak across tests.** A sibling template's `@a` write hits every
  dummy, so "this score was never set" is not provable by omission: a template
  asserting the *absence* of state must actively clear it on its own dummy
  (`verb_flag_gate`'s withheld flags arrived pre-set via `verb_interact`'s
  `@a`; `packtest_preamble` with `with_flags: false` now clears them to 0).

The remaining templates still write `@a`-wide and pass by batch-order luck;
convert them to pinned dummies whenever one is touched.

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

### Assembled-world model (shared, gravity-settled)

`crate::assembled` builds the one authoritative cell→block map of the world the
shipped delve actually assembles — placed prefab structures (`/place template`),
solver socket seals, gate clears — **then settles gravity-affected blocks**. The
delve ships into a `the_void` flat world (no natural floor), so a vanilla
`FallingBlock` (`sand`/`red_sand`/`gravel`/`*_concrete_powder`/anvils/`dragon_egg`)
placed unsupported by `/place template` immediately falls out of the world and
leaves air. Settling reproduces this per `(x,z)` column: non-falling blocks are
immovable supports (stone floats), each falling block drops onto the highest
support at or below it, and a falling block with no support anywhere below it
despawns into the void. `pointed_dripstone`/`scaffolding` attach upward / by
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
gravity settling. Free `minecraft:water` cells seed a deterministic, **conservative
superset** of vanilla flow (mirroring spec-0010's never-overestimate-walkability
stance): (1) infinite-water source formation — a supported air cell flanked by ≥2
source cells becomes a source, cascading, so a walled pool basin fills completely,
not just 7 cells from its seeds; (2) 7-level horizontal decay from the completed
source set plus infinite downward flow. Vanilla's drop-seeking *direction* rule is
omitted (spread goes every way), which only over-marks. Every flooded cell (any
water level, plus sources) is **impassable and never standable floor** for every
consumer — nav, wave seating, relight fixture placement, waypoint export — the same
single-model discipline as settle. This closes the water analogue of the gravity
divergence: a `cave-shore` pool floods `[261,66,1]`, a cell an unpatched model
routed a talk-to leg's step-up through. (Waterlogged solids keep their host id and
stay solid; they do not seed the flood — vanilla waterlogging never spreads.)

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
| passable | open `*_fence_gate` (block state `open=true`, read from the prefab palette), trap triggers | yes | no |
| flooded | water reach | no | no |

Modelled **precisely**: fences, walls, fence gates (open vs closed), trap
triggers, water. Modelled **conservatively** — treated as a full solid cube, never
as walkable-through: slabs, stairs, carpets, snow layers, doors, trapdoors, and
every other partial-collision block (the only inaccuracy this can introduce is a
floor face one cell off the true surface height, which may over-block a route but
never over-proves one). The tall/gate classes close the owner-hit soundness hole:
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
regions are passable). Steps are
cardinal, one block up or down; a step **up** additionally requires head clearance
to jump (the cell two above the source feet must be air), so a routed/exported path
is one an entity — including the mineflayer bot — can actually walk (a ramp up under
a low ceiling is unroutable, not a silent strand). Cutscene dollies must pass only
non-solid cells. Unroutable/clipping/stranded → `DW0307`/`DW0308`/`DW0311` at build
(never a runtime glitch).

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
| `DW0141` | Reserved enum value/field for the campaign's `dsl_version` (npc `vendor`/`boss`; under 0.2.0 the v0.3 verbs/effects; under pre-0.4 the v0.4 surface; under pre-0.5 the v0.5 surface: `time`/`weather`/`lighting`, `set-time`/`set-weather`; under pre-0.6 the v0.6 surface: area `mitigation`, `close-gate`, `damage-players`, `set-checkpoint`, `begin-stealth`/`end-stealth`, `horizon`/`boundary`, the `play-sound` effect + `narrate` `style: art`, per-effect `requires_flags`, stage-2 npc `deferred` + the `spawn-npc` effect, stage-5 `actors` + `spawn`/`despawn`/`move`/`unleash-actor`, `sequence`, and the `traps[]` section). |
| `DW0142` | Anchor not provided by the area's bound prefab. |
| `DW0143` | Item id not in the pinned 1.21.11 registry (kit / `collect` / `interact.requires_item` / `give-item`). |
| `DW0150` | Planned quest (stage 4) has no stage-5 expansion. |
| `DW0151` | Stage-5 quest not planned in stage 4. |
| `DW0152` | Stage-2 NPC has no stage-6 tree. |
| `DW0153` | Stage-6 tree references an NPC not in stage 2. |
| `DW0160` | Area binds neither or both of `prefab`/`prefab_pool`. |
| `DW0161` | `prefab_pool` references a pool absent from `prefabs/` metadata. |
| `DW0170` | `kill`/`spawn-wave` references an undeclared `wave/<id>`. |
| `DW0171` | A killed wave is never spawned by any `spawn-wave`. |
| `DW0172` | `requires_flags` references a flag no `set-flag` produces. The producer scan descends every nested effect list (`sequence` steps, `on_respawn`/`on_caught`/`on_arrive`), so a `set-flag` nested in a timeline still counts as a producer (no spurious fire). |
| `DW0173` | Wave-mob `entity` is not a known vanilla entity id. |
| `DW0180` | l10n sidecar absent / inconsistent envelope / under-covers inventory (also if `en` is declared). Compiler-level. |
| `DW0181` | l10n sidecar has an orphan key (over-coverage). Compiler-level. |
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
| `DW0343` | A `close-gate` (v0.6) targets a gate anchor whose prefab metadata declares no fill `block` (or is not a gate region at all), so the compiler cannot seal it — `close-gate` fills the region with the anchor's declared block. Compiler-side (needs prefab metadata the DSL anchor registry does not carry), reported at **validation tier (exit 1)** like the atmos `DW032x` checks; scan is over every prefab (gate anchors resolve globally like `open-gate`), and **all** region-providers of the anchor must declare a `block`. Prescription: declare `block` on the gate anchor, or remove the `close-gate`. |

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

The only **advisory** code in the compiler. Vanilla draws a `title`, a `subtitle`
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

### DW02xx — analysis (`compiler::analyze` reachability + `compiler::light` lighting; error; exit 2)

`DW0210`/`DW0211` are emitted by the assembled-world light model
(`crate::light`), surfaced through the build path but mapped to exit 2 (analysis
tier) in `main`; `DW0201`–`DW0203` come from `compiler::analyze` reachability.

| Code | Meaning |
|------|---------|
| `DW0201` | Finale quest can never complete (unreachable finale). |
| `DW0202` | Quest can never be triggered (dead quest — its trigger source never completes). |
| `DW0203` | Objective can never be completed (deadlock: unsatisfiable `after` chain, or a `talk-to` completing option unreachable through the trigger/`after` graph). |
| `DW0210` | **Measured** (spec-0010): a reachable walkable cell of an area is below light 3, under the darkest reachable (time, weather) sky, with no `lighting` declaration and no `mitigation` declaration. Judged over the assembled world (per-seam, sealed-cavity aware — unreachable cavities are never counted). Admission `LightingProfile` is no longer a gating input. **v0.6:** keys on the stage-1 `areas[].mitigation` declaration; the display-name heuristic is deleted, so a renamed water bottle in a class kit no longer passes the gate. |
| `DW0211` | An area's declared relight `fixture` cannot raise every reachable walkable cell to `min_light` — no valid placement site remains (spec-0010). |

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
| `DW0306` | Gate-aware reachability deadlock (an anchor reachable only through a gate no earlier objective opens). |
| `DW0307` | `move-npc` destination unreachable by A* over the solved voxel grid. |
| `DW0308` | `cutscene` camera dolly clips a solid block (checked per shot; the message names the shot and segment). |
| `DW0309` | Mannequin NPC declares `skin.texture_id` but no `skins/<id>.png` to bake. |
| `DW0310` | `spawn-wave` references a wave whose spawn anchor resolves in no assembled area (dangling spawn). |
| `DW0311` | Critical path has a consecutive visited-anchor pair with no walkable A* connection and no inter-area transport (player stranded). Routed over the collision-classified occupancy (task #59), so a required anchor sealed behind an unbroken 1.5-tall fence/wall ring with no fence-gate opening fails here — the full-solid model wrongly proved such pens by standing the player on a fence-top. |
| `DW0312` | A `spawn-wave` needs more standable spawn cells near its anchor than the anchor's own room provides (task #41). **Analysis-tier: exit 2**, like `DW02xx` — a content-design capacity mistake (shrink the wave or use a larger room), not a geometry defect; the message names the wave, area, and needed-vs-found count. |
| `DW0313` | A placed gravity block (`sand`/`gravel`/`concrete_powder`/anvil/`dragon_egg`) despawns into the void at placement — an unsupported gravity floor over the `the_void` world falls out on the first block update, holing the shipped map even off the critical path (task #42). The authoritative gravity-settle gate (`crate::assembled`), not a downstream DW0311/DW0312 side effect. **Analysis-tier: exit 2** — a prefab/generator defect; the message attributes despawned cells+counts per piece and prescribes a non-falling substrate. Blocks that fall but **land on support** are faithfully modelled by the settle pass (no diagnostic): the shipped geometry is exact for every consumer, and the generator's own zero-unsupported invariant catches an *unintended* fall at authoring. Anti-dodge: swapping the floor palette to non-falling blocks to silence this is explicitly rejected — gravity floors are a first-class content need; add the substrate. |
| `DW0314` | An exported critical-path waypoint is not standable in the FINAL assembled world (settled + water-flooded + relight fixtures) — the build-time self-check that makes the water-flow / post-nav-mutation divergence class structurally impossible to ship (task #45). Routes come from A* over that same world, so this fires only if a later pass mutates a cell nav relied on or an endpoint resolves off the walkable set; the message names the offending cell and leg. Fix the prefab/water or the assembly — never nudge the waypoint. |
| `DW0315` | A `set-checkpoint` (spec-0012) strands the party: re-rooting the DW0311 reachability at the checkpoint cell, the first remaining required critical-path anchor is no longer walkable from it (a checkpoint behind a one-way drop the forward path can't re-cross after respawn). The message names the checkpoint and the first unreachable anchor and prescribes moving the checkpoint or adding a return route — never deleting the checkpoint to silence the proof. |
| `DW0316` | A `set-checkpoint` anchor has no standable footing within snap range on the final assembled model (a trap-trigger / hazard / mid-air cell) — the party would respawn into void or a wall (spec-0012). Because the relight pass already proves every reachable walkable cell meets the area's `min_light`, a checkpoint that clears this and DW0315 provably meets `min_light` too. |
| `DW0325` | A `move-actor` destination is unreachable over the assembled geometry for the **actor's footprint** (per-entity dims table; warden 0.9×2.9 needs 3 cells of headroom, so it can be stranded where a player fits), or an actor spawn/destination anchor resolves to no world position (spec-0014). Build-tier (exit 3), `compiler::nav`; the message names the actor, the leg, and a best-effort first blocked cell. |
| `DW0327` | A `begin-stealth` (spec-0014) zone is unstandable, or unreachable from the player's position at the beat that activates the stealth check — a guaranteed-unwinnable stealth beat. The message names the zone and prescribes placing it over reachable floor / within walkable reach of the activating beat. |
| `DW0329` | A `sequence` effect is nested inside another `sequence` (directly, or reachable via a nested `move-actor` `on_arrive`) — timelines do not recurse (spec-0014). Validation-tier (exit 1), `dsl::validate`. Flatten the inner steps into the outer timeline (shift their `at_ticks`). |
| `DW0342` | A **lethal** trap (spec-0011) whose trigger cell lies on the forced critical path with no discharge — not avoidable (the trigger cell is a required path cell), not survivable (`rearm`, so a respawn walk-back re-triggers it → soft-loop), and not disarmable (no disarm affordance reachable before it, over the world with the trap cell blocked). The player is provably killed or soft-looped. **Analysis-tier: exit 2**, like `DW0312` — a content-design mistake, not a geometry defect; the message names the trap and prescribes moving it off the path, setting `reset: once`, or adding a reachable `disarm`. Renumbered off the spec's stale reserved number (0314 — since taken by the waypoint self-check). |
| `DW0344` | In a `horizon: ocean` world, a placed piece whose prefab metadata declares `waterline_y` does not land that waterline at sea level (`piece.y + waterline_y ≠ 62`) — the piece floats above the sea (its shore an unclimbable cliff, its authored water pocket hanging in the air) or is drowned under it. Build-tier (exit 3), `compiler::plan`, checked after placement. Nothing downstream can catch this: nav, boundary, POV and PackTest all derive from the very placement that is wrong, so a mis-datumed island validates green and ships unplayable. The message names the area, prefab, placed y and the signed offset, and prescribes correcting the declared `waterline_y` (the local y of the piece's top water block; the island convention is 2) or rebuilding the piece against the convention — ocean areas are placed at y=60 and a piece with a different waterline cannot share that datum. Pieces declaring no `waterline_y` author no sea and are not checked. |
| `DW0345` | The assembled world resolves **no entry anchor** — no placed piece declares any of the entry-anchor names (`spawn`, `entry`; see §4 "First-join placement"). The compiler then has no cell to call the campaign's start: no `setworldspawn`, no class-apply teleport, no first-join placement, no `dw:cp` seed. Build-tier (exit 3), `compiler::emit`. Silent before — the delve compiled clean and fell back to the vanilla spawn search, which a **dedicated** server resolves to the surface (so every rung of the validation ladder stayed green) and the **integrated singleplayer** server resolves to the build floor, i.e. inside solid stone. Prescription: give the pool's entry-role prefab an entry anchor in its metadata `anchors`, or bind the area to a prefab that has one. |
| `DW0346` | A prefab metadata `*.json` (or `pools.json`) in the prefabs dir failed to read or parse (task #62). The canonical trigger is an **older delvec meeting newer metadata**: `deny_unknown_fields` rejects a field this delvec predates. Previously a silent skip — the prefab vanished from the registry and the run failed much later as a baffling `DW0300` "prefab not found" (or a `DW0160` binding error) with no hint of why. Now `PrefabRegistry::load_dir` records a per-file diagnostic naming the file and the serde error, folded into every `validate`/`analyze`/`build` at **validation tier (exit 1)**; loading continues for the other files (report-all, not fail-fast). Prescription: upgrade delvec, or fix the named field. |

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
| Asset-pipeline tooling `DW07xx` (schem/render/admit) | spec-0007 |
| Determinism invariants | ADR-0006 |

### Known spec ↔ code drift (current, for maintainers)

- **spec-0002 CLI** lists stages `1..5`, `dsl 0.1.0`, and omits `--json`/
  `--prefabs`/`--lang`; code is stages `1..6`, `dsl 0.6.0`, all three flags.
  (Spec is the original record; addenda + code are current — this doc governs.)
- **`gamerule keep_inventory true`** is emitted by the sealing baseline but is
  **not** in spec-0002's environment-sealing list (added as box-garden death
  policy; recorded here).
- **Sky attenuation constants** (`crate::light::effective_sky`, spec-0010): the
  stored sky-light baseline (15 at a sky-open cell) and the `time`/`weather` set
  commands are live-verified (1.21.11 itzg VANILLA); the per-state *effective*
  attenuation follows the documented vanilla `getSkyDarken` surface model
  (noon/day 15, night/midnight 4, rain −3, thunder −8 by day) applied
  conservatively — the effective (time-attenuated) value is not directly
  command-readable, so it is not a live measurement. Noted for maintainers.
