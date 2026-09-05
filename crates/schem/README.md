# delvewright-schem

Sponge schematic import for [`delvec`](https://crates.io/crates/delvec), the
delve creator for **Minecraft Java Edition 1.21.11** adventure maps. A `.schem`
(version 2 or 3) becomes a vanilla structure template `.nbt`; command blocks,
structure and jigsaw blocks and NBT-bearing spawners are stripped
unconditionally on the way; a schematic past the 48-block structure cap is
tiled into parts plus a manifest that reassembles it losslessly. Same input and
arguments, byte-identical output.

`delvec` mounts it as `delvec schem convert <in.schem> -o <out.nbt>`. The
crate also owns the structure-template writer every other prefab producer in
the engine goes through, and the prefab metadata document they all read.

## Use

```toml
[dependencies]
delvewright-schem = "1"
```

```rust
use delvewright_schem::{ConvertOutput, convert};
```

## Compatibility

- **Minecraft**: Java Edition 1.21.11.
- **Campaign format**: `dsl_version` `0.2.0` through `0.19.0`.
- **Rust**: 1.97.1 or newer.

## Documentation

- [Tool reference](https://github.com/stellarfeline/delvewright/blob/main/docs/reference/tools.md)
  — the flags, the safety strip, the split manifest and the format coverage.
- [Project repository](https://github.com/stellarfeline/delvewright).

## Licence

GPL-3.0-only.
