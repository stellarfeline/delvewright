# ADR-0005: Two-layer validation (static + dynamic)

- **Status**: Accepted
- **Date**: 2026-07-29
- **Source**: kickoff handoff (2026-07-29)

## Context

The product promise is "provably completable by machine before human QA". The owner has
one QA hour per delve; validation must catch everything cheaper than that hour.

## Decision

Two mandatory layers, both green before a delve reaches the owner:

**Static (compile time, no Minecraft process):**
- Quest-graph reachability analysis: every quest reachable from start, campaign
  completion reachable from every valid game state, no dependency cycles/deadlocks,
  no orphaned rewards or unreferenced IDs.
- Command/syntax validation of all compiler output against the pinned MC version's
  command schema.

**Dynamic (headless server, tooling mods allowed per ADR-0003):**
- **PackTest** (GameTest-based) assertions for mechanisms and milestones, run with
  `-Dpacktest.auto` in CI; exit code = failed tests.
- **mineflayer bot harness** that walks the critical path end-to-end on a headless
  server: join → class select → complete every mandatory quest → reach the end.
  Carpet fake players cover multi-player scenarios.

## Consequences

- CI tiering (ADR-0008): static on every push (fast), PackTest on PR, full bot
  playthrough on release candidates only (slow).
- The bot harness constrains delve design: the critical path must be walkable by a bot
  (no puzzle whose solution can't be scripted from DSL data). The compiler must emit a
  machine-readable critical-path trace alongside the datapack for the bot to follow.
- Static layer lives in the compiler (Rust); dynamic layer in `packtest/` + `harness/`.

## Revisit triggers

If bot playthroughs prove too flaky for CI, they may move to a scheduled/self-hosted
runner — but never become optional for a release.
