# prefabs/island-generator — "nobodys-cave island" set-piece generator

Deterministic generator for the island remake's **set pieces** (spec-0013): the
sea-level entry `island-beach-camp` and the showcase `island-galley` (an ancient
Greek galley moored offshore). A **sibling** of `prefabs/cave-generator` — its own
`[workspace]`, outside `crates/`, so it never enters the shipped `delvec` binary
and no other tileset's `.nbt` changes (ADR-0006). It reuses the cave-generator
primitives (splitmix64 + trilinear value noise, the structure-NBT writer with
`mtime` pinned to 0, keep-socket geometry, the gravity-substrate invariant).

```sh
# from repo root — writes island-*.nbt + island-*.json into the content library
cargo run --manifest-path prefabs/island-generator/Cargo.toml -- campaigns/prefabs/
```

Byte-identical on every run (double-run hash-checked). Both pieces are open-air,
sky-lit island scenery, not enclosed rock:

- **`island-beach-camp`** (21×8×17) — sand shore rising from a ragged tide line;
  a campfire ring with log benches (the relight fixture), two wool/fence A-frame
  tents, a barrel supply stack, a lantern class post, a plank gangplank jetty, and
  driftwood / rock / seagrass greeble. One inland `island:socket` (floor_y=2) to
  greenfield. Anchors: `entry`, `anchor/camp-fire`, `anchor/class-post`,
  `anchor/crew-a`, `anchor/crew-b`, `anchor/surf-wave`, `anchor/gangplank`.
- **`island-galley`** (9×15×29) — a flared plank hull with a dark waterline wale,
  a ram + stempost prow, a curled aphlaston stern, oar rows (spruce trapdoors +
  button ports), a single mast with a white-wool square sail, and the apotropaic
  **eye (ophthalmos)** on both bows. Standalone set-piece, one `anchor/deck`.

See `../island-tileset.md` for the shared island convention (waterline y=2, walk
plane y=3, `island:socket` floor_y=2), the flood-safety rationale, and the
merged-vs-separate galley decision that the terrain worker's greenfield/mountain
pieces must adopt.
