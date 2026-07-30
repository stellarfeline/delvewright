# spec-0004: CI workflow

- **Status**: Draft (tiers 1–3 built for M1; owner approves at PR review)
- **ADRs**: 0006 (determinism gate), 0008 (CI as arbiter, tiering)

GitHub Actions. Three tiers; nothing merges red. Tiers 1–2 in `ci.yml`, tier 3 in
`release.yml`.

## Tier 1 — every push (target < 5 min) *(built)*

- `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` (workspace)
- Harness: `tsc --noEmit`, unit tests (no server), on Node 24
- Docs lint (lychee `--offline` local link check)
- **Determinism gate**: the double-build byte-identity test runs inside
  `cargo test --workspace` (`tests/cli.rs::build_is_byte_identical_across_runs`)
- Static analysis fixtures: `delvec validate` / `analyze` exit-code + `DW####` matrix
- mecha cross-check (PR-only, ADR-0011): re-validates every emitted `.mcfunction`

## Tier 2 — every PR (`ci.yml`, job `tier2-validation`) *(built)*

- Build the delve output (`delvec build`), reusing the cargo cache
- Build the delve image (`Dockerfile.delve`; amd64), boot it on the pinned vanilla
  server, and **assert zero server-side `ERROR` lines** in the boot log
  (the spec-0002 datapack-load criterion)
- PackTest suite via the compose `packtest` profile with `-Dpacktest.auto`
  (exit code = failed tests, spec-0003)
- Target < 10 min. `EULA=TRUE` is job env (owner's standing action, ADR-0010).

## Tier 3 — release candidates only (`release.yml`, tag `rc-*` + dispatch) *(built)*

- Full mineflayer critical-path playthrough via the compose `validate` profile
  (`--exit-code-from bot`, spec-0003)
- Then (only if green) multi-arch image build (amd64 + arm64, buildx + QEMU) and
  push to `ghcr.io/stellarfeline/delvewright/hello-world`
- GitHub Release draft with content license attached *(open — not yet wired)*

May later move to a self-hosted runner if hosted minutes/latency become a problem
(noted in ADR-0008; not built now).

## Mechanics

- Branch protection on `main`: PR required, tier 1+2 green required, linear history.
- Caching: cargo registry/target, npm, and the server jar (by version+checksum —
  EULA note in ADR-0010 means the jar is fetched, never committed).
- git-lfs checkout only in jobs that need `prefabs/` (tier 2+).

## Acceptance criteria

- [x] M0: tier 1 green on the empty workspace; branch protection active.
- [x] M1: all three tiers exist and target the hello-world delve. Tiers 2–3 verified
      equivalently via the local compose profiles (datapack loads with zero errors;
      PackTest passes; bot playthrough passes; multi-arch image builds for
      amd64+arm64). First live GitHub-runner execution lands with this PR.
- [ ] Tier 1 wall-clock < 5 min on GitHub-hosted runners *(measure on first run)*.
- [ ] A red tier 2 blocks merge (to verify once deliberately after merge).
- [ ] A seeded nondeterminism fixture fails the determinism gate *(gate is green;
      the negative fixture is deferred with the M2 fixture-matrix work)*.

## Notes for the future (packaging/release spec)

- The delve image's world-gen env in `Dockerfile.delve` is hand-mirrored from the
  compiler's `server/server.properties`; template it straight from that file so it
  cannot drift. (`GENERATOR_SETTINGS` quotes MUST be backslash-escaped in Docker
  ENV or the server dies on invalid `generator-settings` JSON.)
- Image tag scheme is provisional (`rc-*` tag or `dev-<sha>` + `latest`); settle the
  naming + a per-campaign matrix (multiple delves) in the release spec.
- GitHub Release draft + content-license attachment is stubbed, not wired.
