# ADR-0009: Pinned Minecraft version — 1.21.11

- **Status**: Accepted (owner confirmed 2026-07-29)
- **Date**: 2026-07-29
- **Source**: kickoff handoff (2026-07-29) requires a 1.20.5+ long-term pin; specific
  version researched and owner-confirmed 2026-07-29

## Context

The pin must be ≥1.20.5 (vanilla `/transfer` for v2) and simultaneously supported by
all three dynamic-validation tools (ADR-0005). Verified 2026-07-29:

| Tool | Newest MC supported | Evidence |
|------|--------------------|----------|
| mineflayer 4.37.1 (MIT) | **1.21.11** (no 26.x) | README supported-versions list — **binding constraint** |
| PackTest 2.4.0 (MIT) | 1.21.11 (2.5+/2.6-beta cover 26.x) | Modrinth versions |
| Carpet 1.4.194 (MIT) | 1.21.11 (v26.x line covers 26.x) | GitHub releases |

Context on the release line: 1.21.11 "Mounts of Mayhem" (2025-12-09, data pack_format
94.1) is the **final release of the 1.x line**; Mojang switched to year-based YY.D.H
versioning with 26.1 (2026-03) and is at 26.2 as of today. The 1.21.9+ datapack format
already uses the modern `min_format`/`max_format` pack.mcmeta scheme, and 1.21.11
includes the gamerule-registry and `/worldborder` tick-unit changes — so pinning here
means we're on the modern format conventions, not stranded before a cliff.

## Decision

Pin **Minecraft Java 1.21.11** as the long-term constant for the player-facing server,
the compiler's command/datapack target (pack_format 94.1), and all validation
infrastructure. The pin lives in exactly one place in the repo (a version manifest
consumed by compiler, compose, and CI) with the server-jar checksum beside it
(ADR-0010 EULA handling).

Supporting facts: `/transfer` requirement satisfied (≥1.20.5); GDMC-HTTP 1.8.4 — the
ADR-0004 fallback path — also targets 1.21.11; if beet/mecha are chosen in Decision
Point #1, mecha must be configured to the 1.21.11 command tree (it defaults to 26.2).

## Consequences

- Delve content is bounded by 1.21.11 vanilla features. Accepted: the pin's stability
  is worth more than new-version content for a curated 2–3h format.
- Players use a 1.21.11 client (vanilla launcher keeps all versions available).
- The v2 hub must also run 1.21.11 (`/transfer` between same-version servers).

## Revisit triggers

- mineflayer gains 26.x support **and** a delve-relevant feature exists only in 26.x —
  then evaluate a one-time migration; never track latest.
- A critical server-side security fix ships only for newer versions.
