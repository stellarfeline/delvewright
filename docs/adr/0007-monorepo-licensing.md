# ADR-0007: Monorepo; GPL code, separately-licensed content

- **Status**: Accepted
- **Date**: 2026-07-29
- **Source**: kickoff handoff (2026-07-29)

## Context

One person plus agents maintain everything; cross-cutting changes (DSL schema →
compiler → harness) are the norm. Pipeline code and creative content have different
licensing needs and different distribution channels.

## Decision

Single monorepo `delvewright` (name verified free on GitHub, crates.io, and npm as of
2026-07-29) — **private for now, public when ready**:

```
CLAUDE.md   docs/adr/   docs/specs/   crates/   prefabs/   harness/   packtest/   validation/
```

- **Code** (crates, harness, packtest templates, validation infra):
  **GPL-3.0-or-later**.
- **Prefab/content assets in-repo**: original, CC0, or CC BY only; provenance recorded
  in prefab metadata. **CC BY-NC or unknown-license material is never ingested.**
- **Generated campaigns/worlds live outside this repo**: shipped via GitHub Releases /
  OCI registry, under **CC BY-SA 4.0**.
- Binary assets (`prefabs/*.nbt`) tracked via git-lfs.

## Consequences

- One PR can atomically change schema + compiler + tests — no cross-repo coordination.
- License boundary is a directory boundary; per-directory LICENSE files make it
  auditable.
- While private, GitHub Actions minutes are metered (free-tier quota applies); the
  free-unlimited-minutes benefit arrives when the repo goes public — revisit before
  the heavy CI tiers land.
- git-lfs adds a clone-setup step and has bandwidth quotas — acceptable at expected
  prefab volume; revisit if the library grows past quota.
