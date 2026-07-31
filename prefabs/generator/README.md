# prefabs/generator — "stone keep" tileset generator

Deterministic generator for the stone-keep prefab tileset. Emits, for each piece,
a gzip-framed vanilla structure `.nbt` and its metadata `.json` into an output
directory. Byte-identical on every run (ADR-0006): fixed iteration order, gzip
mtime pinned to 0, fixed compression level.

The library itself now lives in the **content repo** (`delvewright-campaigns`),
symlinked at `campaigns/` for local dev (spec-0007 Step 0). The generator stays
here (GPL code) and writes its outputs into the content repo's `prefabs/`:

```sh
# from repo root — writes campaigns/prefabs/keep-*.nbt and campaigns/prefabs/keep-*.json
# (campaigns/ is the symlink to the content repo; commit the outputs there).
cargo run --manifest-path prefabs/generator/Cargo.toml --release -- campaigns/prefabs/
```

This is a **standalone crate** (its own `[workspace]`), deliberately outside
`crates/` so it is not a member of the compiler workspace and never enters the
shipped `delvec` binary — mirroring how `crates/compiler/examples/gen_hello_room.rs`
generates `hello-room.nbt`, but kept out of `crates/` on purpose.

Piece geometry, the connection ("keep-socket-v1") convention, and the live-probed
lighting minimums are documented in `../keep-tileset.md`. `measured_min_light` in
each emitted JSON comes from a live 1.21.11 probe (sealed-doorway block light);
the table in `src/main.rs` (`measured_min`) records those measured values.
