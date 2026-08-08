# `delvec` — the compiler (crate `delvec`, lib target `delvewright_compiler`)

The deterministic compiler (spec-0002, ADR-0001/0006/0011): staged campaign DSL
in, datapack + server assets out. Rust-native emission; command syntax checked
against the vendored 1.21.11 command tree; mecha re-validates in CI only.

## Behavior reference

**`docs/reference/compiler.md` is the authoritative record of current compiler
behavior** — the CLI contract, exit codes, per-stage DSL surface, verb→emission
mapping, hard invariants (determinism, environment sealing, the 1.21.11 gamerule
table), and the complete DW diagnostics catalog. Read it there; do not duplicate
it here. This README is crate-local dev notes only. Any PR that changes compiler
behavior updates that reference in the same PR (CLAUDE.md Methodology; the docs CI
job runs `tools/check-dw-codes.py`).

## Crate identity (ADR-0017)

The package is **`delvec`** — `cargo install` resolves by crate name, never by
binary name — while the LIBRARY target keeps the name **`delvewright_compiler`**,
so the 366 in-tree `use delvewright_compiler::…` paths did not churn for the
rename. An external dependent therefore writes:

```toml
delvec = "1"
```
```rust
use delvewright_compiler::{DELVEC_VERSION, DSL_VERSION};
```

`delvewright-dsl` is published alongside it on its own `0.x` line, pinned by an
exact `=` requirement; `versions.toml [engine]` is the source of truth for both
and `validation/check-versions.sh` binds them.

## Build & test

```
cargo build -p delvec                  # build the delvec binary
cargo run -p delvec --bin delvec -- \  # run it
    build <campaign-dir> -o out --prefabs campaigns/prefabs
cargo test -p delvec                   # unit + integration tests
```

Tests read the prefab library (`campaigns/prefabs/*.nbt`, git-lfs) from the
content repo checked out at `campaigns/` (spec-0007 Step 0). Integration tests:
`tests/cli.rs` (the ADR-0006 double-build byte-identity gate + `--lang` builds),
`tests/analyze.rs` + `tests/flow.rs` (reachability, branch coherence, path
replay), `tests/solver.rs` (layout determinism / overlap), `tests/emit.rs`,
`tests/v04.rs`.

## Module map (`src/`)

| Module | Role |
|--------|------|
| `main.rs` | CLI (clap): `validate`/`analyze`/`build`/`schema`, flags, exit codes. |
| `lib.rs` | Version constants; re-exports. |
| `load.rs` | Read a campaign dir (6 stage docs + `l10n/` sidecars) into raw bytes. |
| `registry.rs` | Full pinned-1.21.11 item/entity registries + `PrefabRegistry` (anchors, pools, lighting) injected into DSL validation. |
| `analyze.rs` | Branch-coherent quest/objective/dialogue reachability diagnostics (`DW02xx`, exit 2). |
| `flow.rs` | The flow model behind it: XOR dialogue branches, gate-conditional flag producers, single-branch critical-path extraction + step-by-step replay proof (`DW0204`). |
| `solver.rs` | Jigsaw layout solver — grows a socket-graph layout from the seed, emits `/place template` per piece (`DW030x`). |
| `plan.rs` | Resolve a validated campaign → placement + naming model; assembled voxel grid. |
| `nav.rs` | Compile-time A* over the voxel grid: `move-npc`/`move-actor` (footprint-aware) routing, cutscene clip, critical-path walkability (`DW0307`/`DW0308`/`DW0311`/`DW0325`); per-entity dims table + `Footprint`. |
| `emit.rs` | Deterministic emission of the whole `<out>/` tree. |
| `commands.rs` | Vendored Brigadier command-tree validator (see below). |
| `render_plan.rs` | `render-plan.json` (visual tier, spec-0003/0007). |
| `resourcepack.rs` | Per-delve NPC-skin resource pack bake (spec-0009). |
| `creator.rs` | Creator-overlay `creator-datapack/` emission (spec-0006; never shipped). |

## Command-tree validator depth (`src/commands.rs`, honest)

Every emitted `.mcfunction` line is checked against the vendored Brigadier tree
(`data/commands-1.21.11.json`). It checks **structure**, not argument **values**:

- First token must be a known command root; `literal` nodes match exactly;
  `argument` nodes accept their tokens with a fixed per-parser arity
  (`vec3`/`block_pos` = 3, `vec2`/`column_pos`/`rotation` = 2, `message`/greedy
  `string` = rest, else 1 balanced token). Tokenizing is brace/bracket/quote-aware
  so NBT, block-states and selectors are single tokens.
- Matching **backtracks** across ambiguous argument branches and follows
  `redirect`s (`… matches N` → `execute`, `run <cmd>` → tree root). A line is valid
  iff all tokens are consumed ending on an `executable` node.

It does **not** verify numeric coords, well-formed NBT/JSON, or that a block/item
id exists — that is mecha's job in the CI cross-check (ADR-0011), plus the DSL item
registry for kit items. This catches misspelled commands, wrong arity, and bogus
subcommand paths.

## Vendored data & dependency budget

`data/` holds the 1.21.11 item registry and command tree (see
`data/PROVENANCE.md`). Deps: `clap`, `sha2`, `serde`/`serde_json`,
`delvewright-dsl`. The `delvec` binary copies committed prefab `.nbt` verbatim and
never touches NBT, so `fastnbt` and `flate2` are **dev-only** (the `gen_hello_room`
example + prefab test). `flate2` is beyond the spec budget and flagged: MC
structure files are gzip-framed NBT and neither NBT crate ships the gzip container.

## Live 1.21.11 lore (M1/M2 load shakeout — resolved)

Crate-specific findings proven against a live pinned 1.21.11 server (regression
tests `live_load_shakeout_fixes`, `packtest_suite_is_a_real_test`); the resulting
behavior contract lives in the reference. Keep these in mind when touching emission:

- `pack.mcmeta` must emit `min_format`/`max_format` `[94, 1]` — a bare
  `pack_format` is rejected for formats > 81.
- The interaction-advancement `entity` condition must be the **single
  sub-predicate object** form, not a loot-condition list.
- `setup` must `forceload add` prefab chunks before `place template` (else
  `place`/`summon`/`fill` silently no-op at `#minecraft:load`).
- Bot observation is the anchored marker channel — one whole chat line,
  `[dw:complete <campaign_id> <token>]`, broadcast per objective
  (`obj/<id>`) and once for the campaign (`campaign`). mineflayer 4.37.x cannot
  read 1.21.11 scoreboard scores, and the completion objective is NOT put on the
  sidebar (a raw internal id must never surface to players), so this is the sole
  observation channel. It is matched exactly, never as a substring, and `DW0182`
  reserves the sigil in all player-visible text — see docs/reference/compiler.md
  §4 "The completion-marker channel".
- Wave mobs must use component-era `equipment` NBT with zero `drop_chances` —
  legacy `HandItems`/`HandDropChances` are silently ignored on `/summon`.
- PackTest tests emit to `data/<ns>/test/…` (misode/packtest 2.4.0); PackTest
  commands are exempt from the vanilla command-tree validator.
