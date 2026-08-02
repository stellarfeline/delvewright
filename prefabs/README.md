# prefabs/

**The prefab library has moved.** As of spec-0007 Step 0 the `.nbt` prefab
library and its per-item metadata live in the content repo
[`delvewright-campaigns`](https://github.com/stellarfeline/delvewright-campaigns)
under `prefabs/`, beside `campaigns/` — one content repo is the complete
authoring environment (clone it, reuse every piece, and a new prefab ships in
the same PR as the campaign that needs it). Licensing stays directory-scoped:
prefab items are per-item CC0 / CC BY / original (ADR-0007), campaigns are
CC BY-SA 4.0.

For local dev the content repo is symlinked at `campaigns/` in this repo, so the
library resolves at `campaigns/prefabs/`. That is the compiler's default
`--prefabs` path; CI checks the content repo out at the SHA pinned in
`versions.toml` `[content]` and passes `--prefabs` explicitly. The pin makes the
build deterministic (ADR-0006): same DSL + same seed + same content SHA →
byte-identical datapack, with the content SHA recorded in the build manifest.

## What stays here (GPL code)

- **`generator/`** — the deterministic "stone keep" tileset generator. Its
  **outputs** (the `.nbt` + metadata) are committed to the content repo, not
  here; the generator itself is GPL code and stays in the main repo. See
  `generator/README.md`.
- **`cave-generator/`** — the deterministic "Mediterranean cave/shore" tileset
  generator (prefab-ceiling probe), a sibling of `generator/` with its own
  `[workspace]`; keep output is untouched. Emits `pool/cave-shore`, a structural
  drop-in for `pool/stone-keep`. See `cave-generator/README.md`.
- **`island-generator/`** — the deterministic "nobodys-cave island" SET-PIECE
  generator (spec-0013 remake): the sea-level beach camp + the ancient-Greek
  galley. Another sibling with its own `[workspace]`; reuses the cave-generator
  NBT/socket/substrate machinery. Emits the `island:socket` convention. See
  `island-generator/README.md` and `island-tileset.md`.
- **`island-terrain-generator/`** — the deterministic "nobodys-cave island"
  TERRAIN generator: the greenfield connectors + the mountain terminal (shell +
  30×14×24 cavern, switchback slope on the face). Another sibling with its own
  `[workspace]`; reuses the cave-generator machinery and the `island:socket`
  floor_y=2 datum. See `island-terrain-generator/README.md` and `island-tileset.md`.
- **`tidal-keep-generator/`** — the deterministic "tidal keep" SOULS tileset
  generator: the six pieces of the drowned-shore set (barrow field, gatehouse,
  wall walk, courtyard/chapel, cistern, bell tower). Another sibling with its own
  `[workspace]`; reuses the cave/island generator machinery and emits the
  `tk:socket` convention on two datums (shore + keep plinth). See
  `tidal-keep-generator/README.md` and `tidal-keep-tileset.md`.
- **`keep-tileset.md`** — the stone-keep connection convention, piece list, and
  live-probed lighting minimums that the generator implements and documents.
- **`cave-tileset.md`** — the cave/shore piece list, `cave:socket` convention,
  derived lighting, and the render-critique round notes (the probe's evidence).
- **`tidal-keep-tileset.md`** — the `tk:socket` convention (two floor datums, all
  vertical gain authored inside a piece), the six-piece spine and its pool, the
  full anchor inventory the DSL authors against, the wear-gradient /
  observability / visible-perch design intents, and the generator invariants that
  keep them true.
- **`island-tileset.md`** — the island convention (ocean horizon, waterline
  y=2 / walk plane y=3, `island:socket` floor_y=2), the set-piece list, and the
  merged-vs-separate galley decision. The terrain worker's greenfield/mountain
  pieces must align to it.
