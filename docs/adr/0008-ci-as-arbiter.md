# ADR-0008: Spec-driven development; CI as sole arbiter

- **Status**: Accepted
- **Date**: 2026-07-29
- **Source**: kickoff handoff (2026-07-29)

## Context

Implementation is done by Claude Code sessions, possibly parallel ones, with no shared
memory beyond the repo. The owner will not review code line-by-line. Correctness must
therefore be established mechanically, and intent must be written down where every
session can read it.

## Decision

- **Spec-driven**: the owner writes/approves specs (`docs/specs/`) with
  machine-verifiable acceptance criteria. Implementation sessions work against specs.
  **No spec, no feature.**
- **CI is the sole arbiter of correctness.** Nothing merges red. The owner reviews PR
  descriptions and architecture-level diffs only.
- **Tiered testing**: unit + static analysis on every push; PackTest integration on
  PR; full bot playthrough only on release candidates (may move to a self-hosted
  runner later).
- **PR-based flow even solo**, on a public GitHub repo with GitHub Actions.
- **Docs are the agents' only persistent memory**: every session writes its lessons
  back into CLAUDE.md / ADRs / specs before ending.
- Repeated workflows get encoded as skills/slash commands (`/new-campaign`,
  `/validate`, `/release`) once performed manually twice.

## Consequences

- Test quality is load-bearing: a weak test suite silently lowers the correctness bar
  for the whole project. Specs must translate into CI checks, not prose.
- PR descriptions are a first-class deliverable (they are what the owner reviews).
- CI latency budget matters; hence the tiering. The push tier must stay fast (<5 min).

## Revisit triggers

If the acceptance-criteria → CI-check translation repeatedly fails for a class of
requirements (e.g. "is it fun"), that class is explicitly routed to the owner's QA
hour instead — never silently dropped.
