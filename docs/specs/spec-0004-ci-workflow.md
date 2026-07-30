# spec-0004: CI workflow

- **Status**: Skeleton
- **ADRs**: 0006 (determinism gate), 0008 (CI as arbiter, tiering)

GitHub Actions on the public repo. Three tiers; nothing merges red.

## Tier 1 — every push (target < 5 min)

- `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` (workspace)
- Harness: `tsc --noEmit`, lint, unit tests (no server)
- Docs lint (markdown link check; ADR/spec index consistency)
- **Determinism gate** (from M1 on): double-build fixture campaigns, hash-compare
- Static analysis fixtures: `delvec validate` / `analyze` exit-code matrix

## Tier 2 — every PR (adds to tier 1)

- Build the validation image (amd64 only)
- Datapack load check on headless pinned server (zero log errors)
- PackTest suite via `-Dpacktest.auto` (exit code contract, spec-0003)

## Tier 3 — release candidates only (tag `rc-*`)

- Full mineflayer critical-path playthrough via the compose profile (spec-0003)
- Multi-arch image build (amd64 + arm64) and push to GHCR
- GitHub Release draft with content license attached

May later move to a self-hosted runner if hosted minutes/latency become a problem
(noted in ADR-0008; not built now).

## Mechanics

- Branch protection on `main`: PR required, tier 1+2 green required, linear history.
- Caching: cargo registry/target, npm, and the server jar (by version+checksum —
  EULA note in ADR-0010 means the jar is fetched, never committed).
- git-lfs checkout only in jobs that need `prefabs/` (tier 2+).

## Acceptance criteria (to be made precise in Draft)

- [ ] M0: tier 1 green on the empty workspace; branch protection active.
- [ ] M1: all three tiers exist and are green on the hello-world delve; a seeded
      nondeterminism (test fixture) fails the determinism gate.
- [ ] Tier 1 wall-clock < 5 min on GitHub-hosted runners.
- [ ] A red tier 2 blocks merge (verified once deliberately).
