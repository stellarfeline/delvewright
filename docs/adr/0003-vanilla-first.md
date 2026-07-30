# ADR-0003: Vanilla-first gameplay; mods in tooling layer only

- **Status**: Accepted
- **Date**: 2026-07-29
- **Source**: kickoff handoff (2026-07-29), owner decision

## Context

Modded servers impose a permanent upgrade tax (every mod must support every version
bump), complicate the player-facing install, and couple delve longevity to third-party
maintenance. The players are a fixed group of 1–4 on a vanilla client.

## Decision

Gameplay runs on **datapacks + command functions + vanilla mechanics only**. The
player-facing server is pinned vanilla — no mod loader. Mods are permitted exclusively
in the tooling/validation layer (PackTest, Carpet fake players, any future dev
tooling) and are never required to *play* a delve.

## Consequences

- Any vanilla client of the pinned version can join with zero setup.
- Delve images stay playable indefinitely; no mod-compatibility maintenance.
- Gameplay mechanics are bounded by what datapacks can express. Accepted: the delve
  format (quest-driven, adventure mode) fits datapack capabilities well.
- v2 hub-and-instance topology can use vanilla `/transfer` (1.20.5+) — no proxy mod.

## Revisit triggers

A delve concept that fails specifically on the vanilla presentation ceiling
(documented in `docs/notes/vanilla-capability-ceiling.md`: cutscene camera, custom
entity models, scripted NPC movement). Agreed fallback path (owner, 2026-07-29): not
a general mod loadout, but pinning a **small, fixed set of specific low-level
engine-extension mods**, adopted via a superseding ADR — weighing that it breaks
zero-setup vanilla joins. Until that ADR exists, mod-free stands.
