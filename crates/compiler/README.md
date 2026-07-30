# `delvewright-compiler` (`delvec`)

The deterministic compiler (spec-0002, ADR-0001/0006/0011): staged campaign DSL
in, datapack + server assets out. Rust-native emission; command syntax checked
against the vendored 1.21.11 command tree; mecha re-validates in CI only.

## CLI

```
delvec validate <campaign-dir>            # stages 1–6 schema + referential checks
delvec analyze  <campaign-dir>            # quest-graph reachability (ADR-0005 static)
delvec build    <campaign-dir> -o <out>   # full deterministic build
delvec schema   --stage <1..6|all>        # export JSON Schema (LLM authoring aid)
delvec --version                          # delvec x.y.z, dsl 0.2.0, mc 1.21.11
```

Global: `--json` (one JSON diagnostic per line, the spec-0002 shape), `--prefabs
<dir>` (prefab metadata + `.nbt`, default `prefabs`).

- **Exit codes**: `0` ok · `1` validation failure · `2` analysis failure · `3`
  build failure (prefab/anchor resolution, emission) · `≥10` internal error.
- `build` implies `validate` + `analyze`; `analyze` implies `validate`. A
  validation failure short-circuits with exit 1 before analysis runs.

## Validation vs analysis

`validate` runs the DSL's v0.2 rule groups (`DW01xx`, see `crates/dsl/README.md`)
with the **full** injected registries: the complete 1.21.11 item registry
(`data/items-1.21.11.json`, 1505 ids) and the real prefab metadata from
`prefabs/` (anchors, pool existence, lighting). (Note: item ids the DSL's 5-item
vendored subset rejects but that are real 1.21.11 items — e.g.
`minecraft:diamond_hoe` — validate here.)

`analyze` adds **deep semantic reachability** over the merged quest + objective +
stage-6 dialogue graph (distinct from the DSL's structural `DW0132`
convergent-sink check) plus the **dark-prefab lighting-mitigation** check. Codes
are a stable API; the CI matrix asserts them exactly.

### `DW02xx` analysis codes

| Code | Meaning |
|------|---------|
| `DW0201` | Finale quest can never complete (unreachable finale). |
| `DW0202` | Quest can never be triggered (unreachable / dead quest — its trigger's source never completes). |
| `DW0203` | Objective can never be completed (deadlock): an `after` chain that can never be satisfied, or a `talk-to` whose completing option is unreachable *through the trigger/`after` graph*. (The per-NPC static case — a `talk-to` with no reachable completing option in its own tree — is caught earlier by the DSL's `DW0123`.) |
| `DW0210` | A reachable `dark`-profile prefab has no proven light mitigation (spec-0001 "Lighting contract"). |

Model (fixpoint): a quest is *active* if `campaign-start` or `quest-complete(X)`
with X completable; an objective is *completable* if its quest is active, its
`after` prerequisites are completable, and its type is satisfiable (`talk-to` ⇒ a
reachable completing option in its NPC's stage-6 tree; `reach-anchor` ⇒ always);
a quest *completes* when active with all objectives completable; the finale is
reachable iff its quest completes. `tests/fixtures/unreachable-finale.json`
violates only this — the finale is triggered solely by its own completion, so it
never activates — and exits 2 / `DW0201`, proving analysis is not vacuous.

**Dark-prefab mitigation (`DW0210`).** A `dark` prefab (floor light < 3) is valid
only where analysis proves a mitigation. The **v0.2 sufficient mitigation is a
night-vision item in some class kit** (owner rule); `give-item` is still reserved,
so a quest-granted mitigation is not yet expressible and is out of scope for this
check. Because stage-3 kit items cannot yet carry potion-effect components, a
night-vision source is recognized by its item id or display name containing
`night_vision`/`night vision` — a static *policy gate* ("you declared darkness,
so declare a light source"), not a runtime guarantee. Every declared area is
treated as reachable in v0.2; pool-bound areas are skipped until jigsaw assembly
(M2 task #9). `tests/analyze.rs` is the dark/mitigated fixture pair.

## Command-tree validator depth (honest)

Every emitted `.mcfunction` line is checked against the vendored Brigadier tree
(`data/commands-1.21.11.json`). The validator checks **structure**, not argument
**values** (see `src/commands.rs` header):

- First token must be a known command root.
- `literal` nodes match exactly; `argument` nodes accept their tokens with a
  fixed per-parser arity (`vec3`/`block_pos` = 3, `vec2`/`column_pos`/`rotation`
  = 2, `message` / greedy `string` = rest, else 1 balanced token). Tokenizing is
  brace/bracket/quote-aware so NBT, block-states and selectors are single tokens.
- Matching **backtracks** across ambiguous argument branches (e.g. `teleport @s 5
  65 2` = targets + location, not teleport-to-destination), and follows
  `redirect`s (`… matches N` → `execute`, `run <cmd>` → tree root).
- A line is valid iff all tokens are consumed ending on an `executable` node.

It does **not** verify numeric coords, well-formed NBT/JSON, or that a block/item
id exists — that is mecha's job in the CI cross-check (ADR-0011), plus the DSL
item registry for kit items. This catches misspelled commands, wrong arity, and
bogus subcommand paths.

## Jigsaw layout solver (`src/solver.rs`, ADR-0004 amendment)

**The compiler is the jigsaw.** A `prefab_pool` area is assembled by the compiler,
not by runtime `/place jigsaw`: the solver grows a socket-graph layout from the
campaign seed and emits per-piece `/place template <piece> <pos> <rotation>`. This
keeps the shipped delve plain vanilla (ADR-0003), makes determinism trivial (one
campaign seed → a hand-rolled splitmix64 PRNG in named per-area streams, ADR-0006),
and gives the compiler full layout knowledge for anchors and the critical path.

- **Pool metadata** (`prefabs/pools.json`, compiler-owned): pool id → member
  pieces with `weight` + `role` (`entry` seeds the layout; `connector` fills the
  spine; `room`/`terminal` carry anchors). The prefab `.json` gains a
  `connectors[]` block (keep-socket-v1: `local_pos`, `facing`, `opening`).
- **Geometry**: `/place template <pos> <rotation>` places local `(0,0,0)` at
  `pos` then rotates about it. Two sockets mate when the child socket sits one
  block beyond the parent, facing opposite (`final_state=air` → clean 3×3
  passage); the mating rule fixes the child's rotation and position. All four
  cardinal rotations + AABB overlap rejection are implemented and unit-tested.
- **Growth**: a straight-line spine — entry → `connector` fillers (straight
  through only, so the pathfinder-free harness bot can walk it) → the referenced
  through-rooms inline → one dead-end terminal at the far end (`boss-hall` last
  when referenced). Branching layouts are a documented future extension.
- **Guarantees**: connected; exactly one entry; every campaign-referenced anchor
  in the area (NPC stands, `reach-anchor` targets, `open-gate` anchors) provided
  by exactly one placed piece; piece count within the DSL `pieces {min,max}`.
- **Sealing**: every mated socket's jigsaw block is cleared to `air`; every
  unmated socket is filled with `stone_bricks` (wall). Emitted as `/fill` in the
  bootstrap after placement.
- **Multi-area transport**: areas sit `AREA_SPACING` apart across void and the
  bot has no pathfinder, so when the critical path crosses areas the compiler
  teleports the player to the next area's entry spawn as the earlier objective
  completes (`plan.transport`, emitted in that objective's completion function).

### `DW03xx` build/solver codes

Build failures (exit 3) carry a stable `DW03xx` code, printed like validation
diagnostics (and as one JSON object per line under `--json`, `stage: "build"`).

| Code | Meaning |
|------|---------|
| `DW0300` | Generic build/resolution failure (missing prefab metadata or `.nbt`, unresolved anchor, critical-path dependency cycle). |
| `DW0301` | The bound pool declares no `entry`-role piece (or no `connector` filler when fillers are needed). |
| `DW0302` | A campaign-referenced anchor is provided by **no** member of the pool (unsatisfiable required anchor). |
| `DW0303` | The `pieces {min,max}` range is too small to fit the entry plus the required anchor-bearing pieces. |
| `DW0304` | The solver could not place a required piece without overlap, or more than one dead-end terminal was required (linear solver limit). |

`tests/fixtures/keep-unsatisfiable-anchor.json` (→ `DW0302`) and
`keep-range-too-small.json` (→ `DW0303`) both pass validate + analyze (the DSL
cannot see pool-area anchors) and fail only at build, proving the solver
diagnostics are reachable. `keep-crawl` is the valid multi-area / multi-piece
fixture (gatehouse single prefab + `pool/stone-keep`, critical path crossing
piece **and** area boundaries).

## Build output (`<out>/`)

`manifest.json` (SHA-256 of the six input stage files + every other output,
sorted), `datapack/` (pack.mcmeta min/max_format `[94, 1]`; see note below), `packtest-datapack/`,
`server/` (void/superflat fixed-seed config), `critical-path.json` (amended
bot-interaction contract). Determinism (ADR-0006): all maps are `BTreeMap`/sorted;
JSON is `serde_json` pretty (sorted keys) + trailing newline; no wall-clock,
hostname, locale or absolute path enters any byte. The double-build byte-identity
test (`tests/cli.rs`) is the ADR-0006 gate.

## Emission design decisions (within spec-0002's allowed choices)

- **NPC interaction**: a `minecraft:villager` (NoAI/Invulnerable/Silent, no
  profession) is the visual body; a co-located `minecraft:interaction` entity
  (tag `dw_npc_<npc>`) is the click target. An advancement
  (`player_interacted_with_entity`, matched by `type=minecraft:interaction` +
  exact `Tags` nbt) fires a reward function that revokes itself and shows the
  keeper dialog. The interaction entity avoids the villager trade screen and gives
  clean per-NPC detection. *(Runtime behaviour lands in the load task; if the
  interaction entity mis-detects, fall back to matching the villager directly.)*
- **Dialog ⇄ trigger bridge** (spec amendment): every dialog button is a
  `minecraft:run_command` firing `/trigger dw.class set N` (class) or
  `/trigger dw.dlg_<npc> set N` (dialogue option). A per-tick handler maps N →
  effect (navigation or `complete-objective`). Trigger objectives are
  `scoreboard players enable @a`-d every tick so the bot's chat command works too.
  The completion objective `dw.campaign` is `setdisplay sidebar` so the bot reads
  it.
- **World / level config**: `server/server.properties` uses
  `level-type=minecraft:flat` + `generator-settings={"biome":"minecraft:the_void",
  "layers":[]}` (a void world) with `level-seed=<stage-1 seed>`,
  `gamemode=adventure`, `difficulty=peaceful`. No region files are shipped; the
  `#minecraft:load` bootstrap places prefabs with `/place template` and summons
  NPCs, so byte-identity covers the whole tree. See `server/README.md` in output.
- **Coordinate scheme**: area *i* (stage-1 order) is placed at origin
  `[i·256, 64, 0]` (M1: one area → `[0,64,0]`); absolute anchor = origin + the
  prefab's local anchor. Players teleport to the first area's `spawn` anchor after
  class selection.
- **Naming**: scoreboard/function-safe names derive from DSL ids' local part
  (`dw.o_<obj>`, `dw.q_<quest>`, `dw.qa_<quest>` active, `dw.dlg_<npc>`, tag
  `dw_npc_<npc>`, dialog `<npc>_<node>`).

## Vendored data & dependency budget

`data/` holds the 1.21.11 item registry and command tree (see `data/PROVENANCE.md`).
Deps: `clap`, `sha2`, `serde`/`serde_json`, `delvewright-dsl`. The `delvec` binary
copies the committed prefab `.nbt` verbatim and never touches NBT, so `fastnbt`
and `flate2` are **dev-only** (the `gen_hello_room` example + prefab test).
`flate2` is beyond the spec budget and flagged: MC structure files are gzip-framed
NBT and neither NBT crate ships the gzip container.

## Live 1.21.11 verification (M1 load shakeout — resolved)

The following were confirmed/fixed against a live pinned 1.21.11 server (see the
`live_load_shakeout_fixes` + `packtest_suite_is_a_real_test` regression tests):

- `datapack/pack.mcmeta` emits `min_format`/`max_format` as `[94, 1]` (from
  `version.json`: `data_major 94, data_minor 1`). A bare `pack_format` is **rejected**
  for formats newer than 81 ("missing mandatory fields min_format and max_format").
- Dialog JSON (`minecraft:multi_action`, `run_command` action with a leading-slash
  `/trigger`) loads clean. The interaction advancement's `entity` condition must be
  the **single sub-predicate object** form, not a loot-condition list.
- `setup` **`forceload add`s** the prefab chunks before `place template` (else
  `place`/`summon`/`fill` silently no-op at `#minecraft:load` time while `#init` is
  still set) and `setworldspawn`s onto the prefab floor.
- `campaign_complete` broadcasts `[Delvewright] complete dw.campaign 1` — the bot's
  observation channel, because mineflayer 4.37.x cannot read 1.21.11 scoreboard
  scores (the sidebar objective is still displayed for the human/future).
- `packtest-datapack/` emits a real PackTest test at `data/<ns>/test/campaign.mcfunction`
  (misode/packtest 2.4.0) — `# @dummy` + `assert score` driving the completion chain.
  PackTest commands are exempt from the vanilla command-tree validator.
