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
- **`keep-tileset.md`** — the stone-keep connection convention, piece list, and
  live-probed lighting minimums that the generator implements and documents.
- **`cave-tileset.md`** — the cave/shore piece list, `cave:socket` convention,
  derived lighting, and the render-critique round notes (the probe's evidence).
- **`island-tileset.md`** — the island convention (ocean horizon, waterline
  y=2 / walk plane y=3, `island:socket` floor_y=2), the set-piece list, and the
  merged-vs-separate galley decision. The terrain worker's greenfield/mountain
  pieces must align to it.
