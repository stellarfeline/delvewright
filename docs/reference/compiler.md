# delvec compiler — behavior reference

**Single authoritative record of *current* compiler behavior.** Specs
(`docs/specs/`) remain the historical decision records; this file is what the
compiler does today. A PR that changes compiler behavior updates this file in the
same PR (CLAUDE.md Methodology; CI enforces the DW-code subset — see
`tools/check-dw-codes.py`).

- Binary: `delvec` (`crates/compiler`, Rust-native, ADR-0011).
- Versions (as of this doc): `delvec 0.1.0`, `dsl 0.5.0`, `mc 1.21.11`.
  Supported campaign `dsl_version`: **`0.2.0`, `0.3.0`, `0.4.0`, `0.5.0`**
  (additive supersets; `0.2.0` output stays byte-identical across the later
  versions).
- spec-0010 (#35) has **landed** at `dsl_version 0.5.0`: stage-1
  `lighting`/`time`/`weather`, effect verbs `set-time`/`set-weather`, the
  assembled-world light model + deterministic relight pass (`crate::light`), the
  measured redefinition of `DW0210`, and diagnostics `DW0211`/`DW0196`.

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
| 7 | Assemble world model (placed pieces → voxel grid) | `compiler::plan` | `DW030x` (exit 3) |
| 8 | Assembled-light + relight (measure, place fixtures) | `compiler::light` | `DW0210`/`DW0211` (**exit 2**) |
| 9 | Nav checks (A* `move-npc`, cutscene clip, critical-path walkability — incl. relight fixtures) | `compiler::nav` | `DW0307`/`DW0308`/`DW0311` (exit 3) |
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
delvec --version                           # "delvec 0.1.0, dsl 0.5.0, mc 1.21.11"
```

Global flags: `--json` (one JSON diagnostic object per line), `--prefabs <dir>`
(default `campaigns/prefabs`), `--lang <code>` (default `en`; affects `build`
only — `validate`/`analyze` are language-independent apart from coverage).

**Exit codes**: `0` ok · `1` validation failure · `2` analysis failure · `3`
build failure · `≥10` internal error. Undeclared `--lang` is a validation-class
rejection (exit 1). Codes are stable API; the CI fixture matrix asserts them.

**`--json` diagnostic shape**:
`{ "code":"DW####", "severity":"error|warning", "stage":"<stage>",
"path":"<json-pointer-ish>", "message":"…" }`. Every v0 code is `error`;
`warning` exists in the shape for future advisory rules.

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
| `areas[]` | 1..N. Each binds **exactly one** of `prefab` or `prefab_pool`+`pieces{min,max}` (else `DW0160`). Area origin = `[i·256, 64, 0]`. | 0.1 / pool 0.2 |
| `areas[].lighting {fixture,min_light}` (opt) | spec-0010: relight pass guarantees `min_light` (1..=14, default 7; `DW0196` out of range) over reachable walkable cells by placing `fixture` (`torch`/`lantern`/`campfire`/`shroomlight`), else `DW0211`. | 0.5 |
| `time` (opt) | `day`/`noon`/`night`/`midnight` (default `noon`). Dimension-global initial state, emitted in the sealing baseline (`time set <kw>`). | 0.5 |
| `weather` (opt) | `clear`/`rain`/`thunder` (default `clear`; `clear` emits nothing — byte-identical to pre-0.5). Dimension-global, emitted after sealing (`weather <kw>`). Rain/thunder attenuate the assembled-light sky term. | 0.5 |

### Stage 2 — `npcs` (casting sheets, stationary)

| Field | Behavior | Since |
|-------|----------|-------|
| `id`,`name`,`area`,`anchor`,`base_entity` | NPC body placed at resolved anchor; `name` → l10n `npc.<n>.name`. | 0.1 |
| `role` | Enum `quest-giver|flavor`; `vendor`/`boss` reserved → `DW0141`. | 0.2 |
| `persona{archetype,speech_style,motivation,…,relationships[]}` | Structured; **excluded** from l10n; relationship refs validated in-stage (`DW0112`). | 0.2 |
| `skin{texture_id,model}` (opt) | Switches body to `minecraft:mannequin`; PNG baked to resourcepack. Missing PNG → `DW0309`; bad/dup id → `DW0190`. | 0.4 |

### Stage 3 — `classes`

1..4 classes. `kit[]` = vanilla item id + count + optional display `name`
(→ l10n `class.<c>.kit.<i>.name`). `name`/`blurb` player-visible. Reserved kit
fields `lore`/`enchantments`/`attributes` are **not defined** → unknown-field
`DW0100`. A kit night-vision item (id/name contains `night_vision`) is the
retained sufficient `DW0210` dark-mitigation (spec-0010 mitigation hierarchy).

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
| Objective `interact` | `{anchor,requires_item?,prop?,…}`; interaction entity; `requires_item` = `execute if items`. `prop{block}` `setblock`s the affordance (v0.4). | 0.3 / prop 0.4 |
| `after[]` | Ordering (acyclic → `DW0140`). | 0.1 |
| `requires_flags[]` | AND-gate on set flags (puzzle primitive). | 0.3 |
| `waves[]` | `{id,anchor,mobs[{entity,count,name?,attributes?,effects?}]}`; entity validated (`DW0173`); `attributes`/`effects` are v0.4 (`DW0192`). | 0.3 / tuning 0.4 |
| `triggers[]` | `{id,at,on:strike\|use\|approach{range},requires_flags?,once?,effects[]}` (v0.4). Bad/dup/`range 0` → `DW0194`. | 0.4 |
| Effect `open-gate` | Fills gate anchor to air. | 0.1 |
| Effect `campaign-complete` | Sets `dw.campaign`; finale fanfare. | 0.1 |
| Effect `spawn-wave` | Summons wave mobs (AI on), tag `dw_wave_<id>`. | 0.3 |
| Effect `give-item{item,count,name?}` | Grants item (`name` v0.4). | 0.3 |
| Effect `set-flag{flag}` | Sets `dw.f_<flag>` (per-player). | 0.3 |
| Effect `narrate{text,style?,sound?}` | chat/title/subtitle; `text` → l10n. | 0.4 |
| Effect `set-block{anchor,block}` | `setblock` at anchor; block validated (`DW0193`). | 0.4 |
| Effect `despawn-npc{npc}` | Removes NPC + hitbox. | 0.4 |
| Effect `move-npc{npc,to_anchor,speed?}` | A*-planned per-tick tp through walkable space; unroutable → `DW0307`. | 0.4 |
| Effect `cutscene{path[],seconds}` | Two-camera spectator dolly; clip → `DW0308`. | 0.4 |
| Effect `set-time{time}` | Instantaneous dimension-global cut (`time set <kw>`); persists (cycle frozen). | 0.5 |
| Effect `set-weather{weather}` | Instantaneous dimension-global cut (`weather <kw>`); persists (cycle frozen). | 0.5 |

Dialogue effects `set-flag` (v0.4) and `set-time`/`set-weather` (v0.5) and option
`requires_flags` mirror the quest forms. Under `0.2.0`, all v0.3 verbs/effects are
reserved → `DW0141`; likewise v0.4 surface under pre-0.4 and v0.5 surface
(`time`/`weather`/`lighting`, `set-time`/`set-weather`) under pre-0.5.

### Stage 6 — `dialogue`

Exactly one tree per stage-2 NPC (`DW0152`/`DW0153`). Nodes reachable from `root`
(`DW0120`/`DW0121`); `complete-objective` effects target a `talk-to` on the same
NPC (`DW0122`); every `talk-to` has ≥1 reachable (`DW0123`) and ≥1 **ungated**
(`DW0191`) completing option. Node `text` → l10n `dlg.<n>.<node>.text`, option
labels → `.opt.<i>.label`.

### l10n sidecars (`l10n/<code>.json`)

Envelope `{dsl_version,campaign_id,kind:"l10n",lang,content}`; `content` = flat
**stable key → translated string**. Key inventory derived from stage docs
(`world.title`, `area.<a>.name`, `class.<c>.name/.blurb/.kit.<i>.name`,
`npc.<n>.name`, `quest.<q>.goal`, `obj.<q>.<o>.title/.hint`,
`dlg.<n>.<node>.text/.opt.<i>.label`, `wave.<w>.mob.<i>.name`). Coverage is
**exact**: missing/absent/inconsistent → `DW0180`; orphan → `DW0181`. Excludes
authoring context (theme/premise/persona).

---

## 3. Verb → emission mapping

Mechanism level (not full mcfunction). See `crates/compiler/src/emit.rs`.

| Verb / effect | Emitted mechanism |
|---------------|-------------------|
| `talk-to` / dialog option | `minecraft:villager` body (NoAI/Invulnerable/Silent) + co-located `minecraft:interaction` (tag `dw_npc_<n>`); click advancement + `/trigger dw.dlg_<n>` both feed one per-tick option handler. |
| class select | Dialog button → `/trigger dw.class set <n>`. |
| `reach-anchor` | Per-tick `execute if entity @s[box ±1 on each axis]`; glowing `end_rod` `item_display` marker (tag `dw_r_<obj>`). |
| `kill` / `spawn-wave` | `spawn-wave` summons mobs (AI on) tag `dw_wave_<id>`, countdown `#<id> dw.wave`; `player_killed_entity` advancement decrements; `kill` completes at 0. Armed species get `equipment` NBT (drop 0): `wither_skeleton→stone_sword`, `skeleton`/`stray→bow`. |
| `collect` | Chest at anchor pre-loaded `count×item`; `inventory_changed` advancement runs guarded completion. |
| `interact` | `minecraft:interaction` (tag `dw_i_<obj>`) + `player_interacted_with_entity` advancement + `/trigger dw.i_<obj>`; `requires_item` = `execute if items`; glowing lantern `item_display` marker. `prop{block}` = `setblock` affordance. |
| `set-flag` / `requires_flags` | `dw.f_<flag>` scoreboard (per-player); required flags AND-ed into objective guards (layered on `after`). |
| `open-gate` | `/fill … air` over the gate region. |
| `give-item` | Grants item to player (`name` → SNBT text component). |
| `narrate` | chat / `title` / `subtitle` (+ optional sound). |
| `set-block` | `setblock` at resolved anchor. |
| `despawn-npc` | Kills body + interaction hitbox. |
| `move-npc` | Per-tick tp along A*-planned walkable waypoints (hitbox in lockstep). |
| `cutscene` | Per player: save gamemode+pos → spectator → alternate `spectate` between two co-located dolly cameras each tick → restore. |
| `campaign-complete` | `dw.campaign` = 1 (`setdisplay sidebar`); broadcast `[Delvewright] complete dw.campaign 1` (bot channel); title fanfare. |
| objective lifecycle | Activation shows `title`+`hint`+`note_block.pling` once (flag `dw.ann_<obj>`); completion sound `experience_orb.pickup`. |
| `set-time` / `set-weather` | `time set <kw>` / `weather <kw>` (dimension-global, no selector) inline in the effect/dialogue-option function; instantaneous cut, persists (cycle frozen). |
| relight fixtures (`lighting`) | `setblock` per placed fixture in `setup_finish`, after structure placement + socket seals (spec-0010). Blocks: `torch`/`wall_torch`, `lantern[hanging=…]`, `campfire[lit=true]`, `shroomlight`. |

Naming: `dw.o_<obj>`, `dw.q_<quest>`, `dw.qa_<quest>` (active), `dw.dlg_<npc>`,
`dw.f_<flag>`, tags `dw_npc_<npc>`/`dw_wave_<id>`/`dw_i_<obj>`/`dw_r_<obj>`.
`CustomName` is a plain SNBT text component (not `'{"text":…}'`).

---

## 4. Hard invariants

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
| — | `gamerule keep_inventory true` (box-garden death policy; **not in spec-0002** — see §6) |
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
- `datapack/pack.mcmeta`: `min_format`/`max_format` = `[94, 1]` (a bare
  `pack_format` is rejected for formats > 81).
- `<out>/`: `manifest.json` (SHA-256 of the 6 inputs + every output; non-`en`
  build adds `language` + hashes the sidecar), `datapack/`, `packtest-datapack/`,
  `server/`, `critical-path.json`, plus `resourcepack.zip`+`SKINS.md`
  (`resource_pack_sha1` in manifest) for a skinned campaign.

### Nav (compile-time, over the assembled voxel grid)

`move-npc` paths and the critical path are routed by A* over the placed-world
block data (every non-air block is an obstacle; gate cells are passable). Cutscene
dollies must pass only non-solid cells. Unroutable/clipping/stranded → `DW0307`/
`DW0308`/`DW0311` at build (never a runtime glitch).

---

## 5. Diagnostics catalog (complete, as of current `main`)

Every DW code in `crates/**/*.rs`. Grouped by range. `tools/check-dw-codes.py`
verifies this catalog is bidirectionally exact against source (CI docs job).

### DW01xx — validation (`dsl`; severity error; exit 1)

| Code | Meaning |
|------|---------|
| `DW0100` | Document does not conform to its stage schema (unknown field / wrong type / missing required field, incl. persona). Parse-time. |
| `DW0101` | `stage` field ≠ document slot. |
| `DW0102` | Unsupported `dsl_version` (not in `{0.2.0,0.3.0,0.4.0}`). |
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
| `DW0141` | Reserved enum value/field for the campaign's `dsl_version` (npc `vendor`/`boss`; under 0.2.0 the v0.3 verbs/effects; under pre-0.4 the v0.4 surface; under pre-0.5 the v0.5 surface: `time`/`weather`/`lighting`, `set-time`/`set-weather`). |
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
| `DW0172` | `requires_flags` references a flag no `set-flag` produces. |
| `DW0173` | Wave-mob `entity` is not a known vanilla entity id. |
| `DW0180` | l10n sidecar absent / inconsistent envelope / under-covers inventory (also if `en` is declared). Compiler-level. |
| `DW0181` | l10n sidecar has an orphan key (over-coverage). Compiler-level. |
| `DW0190` | Mannequin `skin.texture_id` malformed or duplicated. |
| `DW0191` | A `talk-to` has no **ungated** completing option (all `requires_flags`-gated → deadlock risk). |
| `DW0192` | Wave-mob `effects[].effect` not a known 1.21.11 status-effect id. |
| `DW0193` | `set-block`/`interact.prop` block id not a known 1.21.11 block id. |
| `DW0194` | Environment-trigger id malformed/duplicated, or `approach` `range` 0. |
| `DW0195` | A `talk-to` targets an NPC despawned by a prerequisite quest. |
| `DW0196` | Area `lighting.min_light` out of range (must be 1..=14). v0.5, spec-0010. |

### DW02xx — analysis (`compiler::analyze` reachability + `compiler::light` lighting; error; exit 2)

`DW0210`/`DW0211` are emitted by the assembled-world light model
(`crate::light`), surfaced through the build path but mapped to exit 2 (analysis
tier) in `main`; `DW0201`–`DW0203` come from `compiler::analyze` reachability.

| Code | Meaning |
|------|---------|
| `DW0201` | Finale quest can never complete (unreachable finale). |
| `DW0202` | Quest can never be triggered (dead quest — its trigger source never completes). |
| `DW0203` | Objective can never be completed (deadlock: unsatisfiable `after` chain, or a `talk-to` completing option unreachable through the trigger/`after` graph). |
| `DW0210` | **Measured** (spec-0010): a reachable walkable cell of an area is below light 3, under the darkest reachable (time, weather) sky, with no `lighting` declaration and no night-vision kit mitigation. Judged over the assembled world (per-seam, sealed-cavity aware — unreachable cavities are never counted). Admission `LightingProfile` is no longer a gating input. |
| `DW0211` | An area's declared relight `fixture` cannot raise every reachable walkable cell to `min_light` — no valid placement site remains (spec-0010). |

### DW03xx — build / solver / nav (`compiler`; error; exit 3, `stage:"build"`)

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
| `DW0308` | `cutscene` camera dolly clips a solid block. |
| `DW0309` | Mannequin NPC declares `skin.texture_id` but no `skins/<id>.png` to bake. |
| `DW0310` | `spawn-wave` references a wave whose spawn anchor resolves in no assembled area (dangling spawn). |
| `DW0311` | Critical path has a consecutive visited-anchor pair with no walkable A* connection and no inter-area transport (player stranded). |

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
| Assembled-relight, measured `DW0210`, `DW0211`/`DW0196`, stage-1 `lighting`/`time`/`weather`, `set-time`/`set-weather` (all v0.5) | spec-0010 (landed, #35) |
| Asset-pipeline tooling `DW07xx` (schem/render/admit) | spec-0007 |
| Determinism invariants | ADR-0006 |

### Known spec ↔ code drift (current, for maintainers)

- **spec-0002 CLI** lists stages `1..5`, `dsl 0.1.0`, and omits `--json`/
  `--prefabs`/`--lang`; code is stages `1..6`, `dsl 0.5.0`, all three flags.
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
