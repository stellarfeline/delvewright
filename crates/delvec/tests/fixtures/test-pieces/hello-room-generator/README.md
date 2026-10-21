# hello-room generator — provenance record

`src/main.rs` is the program that wrote `../hello-room.nbt`, the piece most of
the engine's test fixtures bind (`prefab/hello-room`). `../hello-room.json`, its
metadata, is maintained by hand beside it.

It is a provenance record (ADR-0018 §6): kept so a reader can see how the piece
was made, never built, never run, and never a gate. The piece is the data; this
is not its source of truth.

It last built at engine revision `5bd35334f2dc9600a12d7fd1132d3a6480edbdb4`,
where it lived at `prefabs/hello-room-generator` as a member of that revision's
`prefabs/` cargo workspace (`prefabs/Cargo.toml`, `prefabs/Cargo.lock`) and
depended on its `prefab-invariants` crate. Run there, it wrote a
`hello-room.nbt` whose sha256 is
`6b7e606b1e0b25c865db43b480ab933904972b181ad0e851d55eb10fb00d09c2`, the bytes
committed here. Its manifest at that revision:

```toml
[package]
name = "hello-room-gen"
version = "0.0.0"
edition = "2021"
publish = false

[[bin]]
name = "hello-room-gen"
path = "src/main.rs"

[dependencies]
prefab-invariants.workspace = true
fastnbt.workspace = true
flate2.workspace = true
serde.workspace = true
serde_json.workspace = true
```

Licence: the engine repository's (GPL-3.0, `LICENSE`).
