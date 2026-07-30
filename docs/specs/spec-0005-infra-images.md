# spec-0005: Infrastructure images & version manifest

- **Status**: Draft (owner directional approval 2026-07-30; approves at PR review)
- **ADRs**: 0003 (mods tooling-only), 0006 (pins), 0009 (MC 1.21.11), 0010 (EULA)

Owner decision (2026-07-30): everything is version-pinned, so stop rebuilding from
external images on every run — publish our own fixed infrastructure images once and
pull them by digest thereafter.

## The two images (GHCR, **public** packages)

Both contain zero private content (mirrors of public images + MIT/Apache mods), so
they are published as public packages — free unlimited storage/bandwidth — while the
repo itself stays private. Private delve images build `FROM` the public base, so
their private layers are only the KB-sized datapack/config deltas.

1. **`ghcr.io/stellarfeline/delvewright-base`** — the delve runtime base: a mirror
   of `itzg/minecraft-server` at our pinned digest (multi-arch amd64+arm64),
   re-tagged under our namespace. No server jar, no mods. Shields us from Docker Hub
   anonymous-pull rate limits and upstream tag/image deletion. `Dockerfile.delve`
   switches its `FROM` to this.
2. **`ghcr.io/stellarfeline/delvewright-toolserver`** — the validation tooling
   server: base + **pre-baked** Fabric loader, Fabric API, and PackTest 2.4.0 (all
   MIT/Apache — legally redistributable). Kills the per-run Modrinth downloads in
   the compose `packtest` profile and their availability/rate-limit risk. Never used
   for anything player-facing (ADR-0003).

**EULA boundary (unchanged, ADR-0010): the Mojang server jar is baked into NEITHER
image.** Both fetch it at first boot via the itzg entrypoint (version+checksum
pinned). CI mitigates the repeated download with `actions/cache` keyed on
version+sha256 — caching for our own runs is not redistribution.

## Version manifest — one source of truth

New repo-root **`versions.toml`**: MC version (1.21.11), server-jar sha256, both
image digests (recorded after each infra publish), Fabric loader / Fabric API /
PackTest versions, mineflayer pin. Dockerfiles (via `ARG`), compose, and CI read
from it; a tier-1 CI check greps that no Dockerfile/compose/workflow hardcodes a
version or digest that disagrees with the manifest. This also discharges the
spec-0004 "hand-mirrored env can drift" debt.

## Publish workflow

`infra-images.yml`, `workflow_dispatch` only (rare, deliberate): buildx multi-arch
build of both images → push to GHCR → print digests. The operator (owner or
planning agent) then records the new digests in `versions.toml` via a normal PR.
Re-publishing is expected only when ADR-0009's revisit triggers fire or tooling-mod
versions bump.

## Consumers switch

- `Dockerfile.delve`: `FROM ghcr.io/stellarfeline/delvewright-base@sha256:…`
- compose `packtest` service: `image: ghcr.io/stellarfeline/delvewright-toolserver@sha256:…`
  (drops `TYPE=FABRIC` + `MODRINTH_PROJECTS` runtime fetching)
- CI tier 2/3: pull by digest; server-jar `actions/cache` step added.

## Acceptance criteria

- [ ] Both images published multi-arch to GHCR as public packages; digests recorded
      in `versions.toml`.
- [ ] A full tier-2 CI run performs **no** Modrinth fetch and **no** Docker Hub pull
      (only GHCR by digest + the Mojang jar). *Amended 2026-07-30 after the first
      live run: the planned jar actions/cache was removed — a single-file ro bind
      mount cannot be replaced by itzg's installer rename ("Device or resource
      busy"), and runners fetch piston-data at >100 MB/s (<1 s), so the cache
      optimized nothing and added a failure mode. Jar checksums stay in
      `versions.toml` as provenance.*
- [ ] `docker compose --profile packtest up` cold-start time drops measurably vs the
      Modrinth-fetching baseline (record before/after in the PR).
- [ ] The manifest-consistency check fails CI when a Dockerfile/compose/workflow
      version disagrees with `versions.toml` (verified with a deliberate mismatch).
- [ ] Bot playthrough and PackTest remain green against the switched images.

## Open

- Whether `delvewright-base` should be a true mirror (`buildx imagetools create`)
  or a `FROM`+relabel build — implementer picks, documents the choice.
- GHCR package retention/cleanup policy for old delve images — release-spec matter.
