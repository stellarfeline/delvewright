# delvec

A deterministic compiler for **Minecraft Java Edition 1.21.11** adventure maps.

You describe a map as JSON — the world and its areas, the NPCs, the classes
players pick from, the quest plan, the quests, the dialogue. `delvec` type-checks
those documents, proves every objective is reachable and every quest completable,
assembles the world from a library of structure prefabs, and writes a datapack
plus the world and server assets that make it playable. Nobody hand-writes an
`.mcfunction`: every command in the output comes from the compiler.

The same documents and the same seed always produce byte-identical output.

## Install

```sh
cargo install delvec
```

Prebuilt archives — Linux (x86-64 and arm64, statically linked), macOS (Apple
Silicon and Intel), Windows (x86-64), each with a published SHA-256 — are on the
[releases page](https://github.com/stellarfeline/delvewright/releases/latest).

## Use

```sh
delvec --version                        # engine, campaign-format and Minecraft versions
delvec schema --stage all               # the JSON Schema every stage is written against
delvec validate <campaign-dir>          # schema + cross-document reference checks
delvec analyze  <campaign-dir>          # + quest-graph reachability and deadlock proofs
delvec build    <campaign-dir> -o out   # datapack, world, and server assets
```

`build` implies `analyze`, which implies `validate`. `<campaign-dir>` holds one
JSON file per stage (`world.json`, `npcs.json`, `classes.json`,
`quest-plan.json`, `quests.json`, `dialogue.json`), optionally a `world-edits.json`
edit script and an `l10n/<code>.json` sidecar per translated language.

Rooms come from a prefab library — Minecraft structure `.nbt` files plus JSON
metadata declaring their anchors, jigsaw sockets and lighting. Point `delvec` at
one with `--prefabs <dir>`.

| Command | What it does |
|---|---|
| `validate` | Schema and referential checks over all stages. |
| `analyze` | Reachability, dialogue deadlocks, unlit rooms. |
| `build` | The full deterministic build. |
| `fmt` | Rewrite authored JSON in canonical form; `--check` reports instead. |
| `schema` | Export a stage's JSON Schema (`--stage` takes `1`–`7` or `all`). |
| `l10n-inventory` | The translatable-string inventory as JSON. |
| `snapshot` | Render one frame of the assembled world plus a scene manifest. |
| `blocking-chart` | Per-elevation cutaway floor plans of every area. |
| `edit` | Replay the world-edit script, re-proving the invariants per batch. |
| `calibrate` | Turn a harvested rehearsal report into camera DSL patches. |

Every problem is reported as a stable `DW####` code with a severity, the stage,
and a path into the document. `--json` writes one JSON object per diagnostic,
one per line. Exit codes: `0` clean, `1` validation failure, `2` analysis
failure, `3` build failure, `≥10` internal error. Only `error`-severity
diagnostics are fatal; `warning`s are reported and do not fail the run.

Every `.mcfunction` line the compiler writes is checked against the vendored
1.21.11 Brigadier command tree before it is emitted, so a misspelled command,
a wrong argument count or a bogus subcommand path fails the build rather than
the server.

## As a library

```toml
[dependencies]
delvec = "1"
```

The library target is named `delvewright_compiler`:

```rust
use delvewright_compiler::{DELVEC_VERSION, DSL_VERSION, MC_VERSION};
```

## Compatibility

- **Minecraft**: Java Edition 1.21.11. Output targets that version and no other.
- **Campaign format**: `dsl_version` `0.2.0` through `0.11.0`.
- **Rust**: 1.97.1 or newer.
- The binary is self-contained: no JVM, no runtime dependencies.

## Documentation

- [Compiler reference](https://github.com/stellarfeline/delvewright/blob/main/docs/reference/compiler.md)
  — the CLI contract, the DSL surface stage by stage, how each verb is emitted,
  and the complete `DW####` catalogue.
- [`delvewright-dsl`](https://crates.io/crates/delvewright-dsl) — the campaign
  format's types and JSON Schemas.
- [Project repository](https://github.com/stellarfeline/delvewright).

## Licence

GPL-3.0-only.
