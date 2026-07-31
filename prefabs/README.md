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
- **`keep-tileset.md`** — the stone-keep connection convention, piece list, and
  live-probed lighting minimums that the generator implements and documents.
