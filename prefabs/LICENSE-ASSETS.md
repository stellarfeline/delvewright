# Prefab & content asset licensing

This file states the licensing boundary for creative assets, per **ADR-0007**
(monorepo; GPL code, separately-licensed content) and the CLAUDE.md forbidden
zones.

## In-repo prefab assets

Prefab assets committed to this repository (`prefabs/**`) must be **original,
CC0, or CC BY only**. **CC BY-NC or unknown-license material is never
ingested.** The source and license of every asset are recorded in that asset's
prefab metadata, so provenance is auditable at all times.

## Generated delve content

Generated campaigns and worlds are **not** stored in this repository. They ship
separately — via GitHub Releases / the OCI registry — and are distributed under
**CC BY-SA 4.0** (ADR-0007). The pipeline code in this repo is licensed under
GPL-3.0-or-later (see root `LICENSE`); the license boundary is a directory
boundary.
