# spec-0024: Release pipeline — from green campaign branch to joinable delve

- **Status**: Proposed
- **ADRs**: 0007 (split licensing), 0008 (CI as arbiter), 0009 (pinned MC),
  0010 (OCI packaging), 0014 (creator distribution)
- **Depends on**: island content PR merge (first release candidate);
  spec-0025 (release tier = all-branch bot runs, once implemented)

## Problem

A finished campaign today is a green branch plus a local compose profile.
"Ships as a versioned OCI image — one `docker run` = a joinable dungeon"
(CLAUDE.md) has no pipeline behind it: no release trigger, no multi-arch
image (the Pi is arm64), no player-facing install story, and the resource
pack (skins + art font) only works because the owner's client is hand-fed.

## Design

### 1. Trigger and versioning

A release is a tag on the content repo: `release/<campaign>/v<semver>`.
CI on that tag runs the FULL ladder at release tier (complete bot
playthrough; all branches once spec-0025 lands) against the engine pinned
in `versions.toml` — the tag proves nothing by itself; the ladder run on
the tag is the release gate (ADR-0008).

### 2. Artifacts (one GitHub Release + GHCR)

- **Delve image** `ghcr.io/<owner>/delve-<campaign>:v<semver>`, multi-arch
  (amd64 + arm64 — dev workstation and Pi prod), built by the existing
  Dockerfile.delve path from the tagged build output. Image labels carry
  provenance: campaign commit, engine version, content license.
- **ONE resource pack zip** attached to the GitHub Release: skins + art
  font, SHA-1 recorded. The shipped server.properties points
  `resource-pack` at that Release asset URL with its SHA-1, so a joining
  vanilla client is prompted automatically — no hand-fed client files.
- **Auto-generated release notes**: campaign title/blurb (player-facing
  voice, audience-separation rule applies).

### 3. Shipped defaults (differ from dev)

- `ONLINE_MODE=TRUE` — a released delve is joined by real accounts;
  offline stays a local-dev override.
- EULA remains operator-supplied at run time (`-e EULA=TRUE`), never baked
  (ADR-0010).
- Difficulty/world settings: unchanged, the compiler's (spec-0010/#209
  entrypoint chain).

### 4. Player-facing README (content repo)

One page per audience rule (CLAUDE.md audience separation): what a player
or host needs to act, nothing about pipeline internals.

- **Play**: install a launcher (links per OS), join `<host>:25565`.
- **Host**: the one `docker run` line; port-forward note; the resource
  pack arrives automatically.
- **Reset for a fresh party**: one line (`docker rm` + volume rm / compose
  `down -v`) — a delve is replayable by wiping world state, nothing else.
- **What you may do**: plain-language license summary — code GPL-3.0,
  campaign content its own license (ADR-0007), assets per prefab
  provenance (ADR-0013). Written for a player, reviewed against the actual
  license files.

### 5. Explicitly deferred

- Creator-mode plugin distribution (ADR-0014, M4).
- Server hosting beyond "a machine you own" (no cloud guides).
- Auto-update of running servers; a new version is a new image + fresh run.

## Acceptance criteria

- [ ] Pushing `release/<campaign>/v<semver>` on a green campaign produces:
      multi-arch image on GHCR (manifest lists amd64+arm64), resource pack
      zip + notes on a GitHub Release — and FAILS (no artifacts) if the
      release-tier ladder is red.
- [ ] A vanilla 1.21.11 client joining the released image from a clean
      machine is prompted for the resource pack and sees skins + art font
      (verified once by hand at first release; the SHA-1 wiring is
      CI-asserted every release).
- [ ] Image labels carry campaign commit, engine version, content license;
      byte-identical rebuild from the same tag yields the same datapack
      (ADR-0006 extended through the release path).
- [ ] README play/host/reset/license sections exist, contain no pipeline
      internals, and every launcher/license link resolves (docs link check).
- [ ] The current Mojang EULA/usage-guideline text is re-read and the
      distribution model checked against it before the first public
      release; the check is recorded in the release notes of v1.
