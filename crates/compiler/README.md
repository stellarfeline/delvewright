# `delvewright-compiler` (`delvec`)

The deterministic compiler (spec-0002, ADR-0001/0006/0011): staged campaign DSL
in, datapack + server assets out. Rust-native emission; command syntax checked
against the vendored 1.21.11 command tree; mecha re-validates in CI only.

## CLI

```
delvec validate <campaign-dir>            # stages 1–5 schema + referential checks
delvec analyze  <campaign-dir>            # quest-graph reachability (ADR-0005 static)
delvec build    <campaign-dir> -o <out>   # full deterministic build
delvec schema   --stage <1..5|all>        # export JSON Schema (LLM authoring aid)
delvec --version                          # delvec x.y.z, dsl 0.1.0, mc 1.21.11
```

Global: `--json` (one JSON diagnostic per line, the spec-0002 shape), `--prefabs
<dir>` (prefab metadata + `.nbt`, default `prefabs`).

- **Exit codes**: `0` ok · `1` validation failure · `2` analysis failure · `3`
  build failure (prefab/anchor resolution, emission) · `≥10` internal error.
- `build` implies `validate` + `analyze`; `analyze` implies `validate`. A
  validation failure short-circuits with exit 1 before analysis runs.

## Validation vs analysis

`validate` runs the DSL's six rule groups (`DW01xx`, see `crates/dsl/README.md`)
with the **full** injected registries: the complete 1.21.11 item registry
(`data/items-1.21.11.json`, 1505 ids) and the real prefab anchor metadata from
`prefabs/`. (Note: item ids the DSL's 5-item vendored subset rejects but that are
real 1.21.11 items — e.g. `minecraft:diamond_hoe` — validate here.)

`analyze` adds **deep semantic reachability** over the merged quest + objective +
dialogue graph — distinct from the DSL's structural `DW0132` (convergent-sink)
check. Codes are a stable API; the CI matrix asserts them exactly.

### `DW02xx` reachability codes

| Code | Meaning |
|------|---------|
| `DW0201` | Finale quest can never complete (unreachable finale). |
| `DW0202` | Quest can never be triggered (unreachable / dead quest — its trigger's source never completes). |
| `DW0203` | Objective can never be completed (deadlock): a `talk-to` with no dialogue option reachable from its NPC's `root` that fires `complete-objective` for it, or an `after` chain that can never be satisfied. |

Model (fixpoint): a quest is *active* if `campaign-start` or `quest-complete(X)`
with X completable; an objective is *completable* if its quest is active, its
`after` prerequisites are completable, and its type is satisfiable (`talk-to` ⇒ a
reachable completing dialogue option; `reach-anchor` ⇒ always); a quest
*completes* when active with all objectives completable; the finale is reachable
iff its quest completes. `tests/fixtures/unreachable-finale.json` violates only
this (removes the objective-completing dialogue option) and exits 2 / `DW0201`,
proving analysis is not vacuous.

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

## Build output (`<out>/`)

`manifest.json` (SHA-256 of the five input stage files + every other output,
sorted), `datapack/` (pack_format 94; see note below), `packtest-datapack/`,
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

## Provisional / follow-ups for later tasks

- `datapack/pack.mcmeta` emits `"pack_format": 94`. The 1.21.11 data pack version
  is 94.1; the `.1` minor is not required for loading and the exact mcmeta
  major.minor schema is confirmed against a live server in the load task.
- `packtest-datapack/` emits a byte-stable pack + a provisional vanilla-command
  assertion function; the exact PackTest test-registration wiring is finalized in
  the spec-0003 task.
- The dialog JSON schema (`minecraft:multi_action`, `run_command` action) follows
  the public 1.21.6 dialog docs and is verified on a live 1.21.11 server in the
  load task.
