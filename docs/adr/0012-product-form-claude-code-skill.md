# ADR-0012: Product form — Claude Code skill as the generation front-end

- **Status**: Accepted
- **Date**: 2026-07-30

## Context

The end product is: the user writes a prompt — anything from a bare theme to a
detailed brief with specific levels and plot — and the system generates a playable
delve end-to-end. That requires an agent runtime: something that writes the staged
DSL, runs `delvec validate`/`analyze`, iterates on the DW#### diagnostics, and walks
the validation ladder. v1 is for the owner's own use.

Building such a runtime from scratch (retry loops, tool scheduling, context
management, multi-stage orchestration) is months of undifferentiated work, and every
generation would be metered API spend.

## Decision

- **v1's front-end is a Claude Code skill** (working name `/new-delve`; companions
  `/validate`, `/release`). Claude Code **is** the agent runtime; the owner's Claude
  subscription carries the heavy DSL-writing and test-iteration load. The product =
  this repo (skills + `delvec` + validation ladder) + a Claude Code session.
- **The user prompt is a constraint set over the five DSL stages**: stages the
  prompt pins down are honored verbatim; unspecified stages are invented by the
  agent. Arbitrary depth of user control, no new machinery.
- **The generated DSL documents are the artifact of record.** The determinism
  boundary (ADR-0006) starts at the DSL: every generated delve's stage JSONs are
  saved, and any delve can be rebuilt byte-identically from them without any LLM.
  A skill version that fails to persist the DSL is broken by definition.
- The skill's inner loop uses the cheap validation tiers (static, then PackTest);
  full bot playthroughs remain reserved for release candidates (spec-0004 tiering).
- `crates/orchestrator`'s role shrinks accordingly: Claude Code owns orchestration;
  the crate becomes thin CLI glue for the skill (or folds into `delvec` — decide
  during M2 planning).
- **Writing our own agent runtime from scratch is out of scope for this project,
  permanently.**

## Consequences

- Product coupling: rate limits, subscription terms, and skill semantics of Claude
  Code directly affect the production line. Acceptable for personal-use v1.
- Skills are versioned markdown in this repo — prompt engineering is reviewable and
  evolvable through the same PR + owner-review process as everything else.
- Multi-user or hosted scenarios do not scale through one owner subscription — a
  deliberate non-goal for v1.

## Revisit triggers

After the first genuinely usable version, the owner may evaluate other agent
runtimes (e.g. Codex) as alternative front-ends. The skill layer is deliberately
thin so the DSL + compiler + validation contract stays runtime-agnostic; porting the
front-end must never require touching `crates/`.
