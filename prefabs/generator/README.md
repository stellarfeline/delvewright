# prefabs/generator — "stone keep" tileset generator

Deterministic generator for the stone-keep prefab tileset. Emits, for each piece,
a gzip-framed vanilla structure `.nbt` and its metadata `.json` into an output
directory. Byte-identical on every run (ADR-0006): fixed iteration order, gzip
mtime pinned to 0, fixed compression level.

```sh
# from repo root — writes prefabs/keep-*.nbt and prefabs/keep-*.json
cargo run --manifest-path prefabs/generator/Cargo.toml --release -- prefabs/
```

This is a **standalone crate** (its own `[workspace]`), deliberately outside
`crates/` so it is not a member of the compiler workspace and never enters the
shipped `delvec` binary — mirroring how `crates/compiler/examples/gen_hello_room.rs`
generates `hello-room.nbt`, but kept out of `crates/` on purpose.

Piece geometry, the connection ("keep-socket-v1") convention, and the live-probed
lighting minimums are documented in `../keep-tileset.md`. `measured_min_light` in
each emitted JSON comes from a live 1.21.11 probe (sealed-doorway block light);
the table in `src/main.rs` (`measured_min`) records those measured values.
