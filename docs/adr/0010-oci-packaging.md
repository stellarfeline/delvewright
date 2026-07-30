# ADR-0010: Delves ship as versioned OCI images

- **Status**: Accepted
- **Date**: 2026-07-29
- **Source**: kickoff handoff (2026-07-29), owner decision

## Context

A delve must be trivially runnable years later: one command, no setup, on the owner's
Raspberry Pi or any host. Its dependencies (server version, world, config, datapack)
must be frozen together.

## Decision

Each released delve is a **versioned OCI image**: pinned vanilla server (ADR-0009) +
generated world + server config + compiled datapack. `docker run <image>` yields a
joinable dungeon. The *same image* is used by the `validation/` compose profile, CI,
and prod — what the bot completed is what players join (with ADR-0006 making that
provable).

Images are multi-arch (amd64 for dev/CI, arm64 for the Raspberry Pi).

**EULA note**: Mojang's EULA does not permit redistributing the server jar. The image
must therefore fetch the pinned server jar (by version + checksum, from Mojang's
official download) at build-on-first-run or entrypoint time rather than baking it into
a published layer — exact mechanism to be settled in the packaging spec, with
determinism preserved by checksum-pinning. (Common prior art: itzg/docker-minecraft-
server downloads at runtime.)

## Consequences

- Release artifact = image tag + content license file; nothing else to distribute.
- "Byte-identical world" (ADR-0006) is auditable at the image-layer level for
  everything we author; the server jar itself is pinned by checksum instead.
- Registry choice (GHCR) and image naming scheme go in the release spec.
