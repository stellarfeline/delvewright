# delvewright-compiler

The compiler library behind [`delvec`](https://crates.io/crates/delvec), the
delve creator for **Minecraft Java Edition 1.21.11** adventure maps. It takes
the staged campaign documents of
[`delvewright-dsl`](https://crates.io/crates/delvewright-dsl), proves every
objective reachable and every quest completable, assembles the world from a
library of structure prefabs, and writes a datapack plus the world and server
assets that make it playable. The same documents and the same seed always
produce byte-identical output.

It also carries the CPU render surface `delvec` exposes — the review page,
Chunky scene emission, the contact sheet, the shot index — and the vanilla
command tree every emitted `.mcfunction` line is checked against before it is
written.

## Use

You almost certainly want the binary: `cargo install delvec`. As a library:

```toml
[dependencies]
delvewright-compiler = "1"
```

```rust
use delvewright_compiler::{DELVEC_VERSION, DSL_VERSION, MC_VERSION};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::analyze::analyze_campaign;
```

The library target is `delvewright_compiler`.

## Compatibility

- **Minecraft**: Java Edition 1.21.11. Output targets that version and no other.
- **Campaign format**: `dsl_version` `0.2.0` through `0.19.0`.
- **Rust**: 1.97.1 or newer.

## Documentation

- [Compiler reference](https://github.com/stellarfeline/delvewright/blob/main/docs/reference/compiler.md)
  — the CLI contract, the DSL surface stage by stage, how each verb is emitted,
  and the complete `DW####` catalogue.
- [Project repository](https://github.com/stellarfeline/delvewright).

## Licence

GPL-3.0-only.
