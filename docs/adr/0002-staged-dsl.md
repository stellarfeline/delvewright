# ADR-0002: Staged, dependency-driven DSL

- **Status**: Accepted
- **Date**: 2026-07-29
- **Source**: kickoff handoff (2026-07-29); pattern from "From World-Gen to Quest-Line"
  (arXiv 2604.25482)

## Context

A single monolithic "write me a campaign" generation produces incoherent output: quests
referencing NPCs that don't exist, gear mismatched to classes, story beats detached
from the map. LLM output quality improves sharply when each generation step has a
narrow schema and conditions on validated earlier output.

## Decision

The DSL is split into **sequential stages, each with its own schema**, where every
stage's generation is conditioned on the validated outputs of all earlier stages:

1. **World/setting** — theme, tone, location palette, story premise
2. **NPCs** — cast, roles, dialogue voice, relationships
3. **Classes & gear** — player classes, starting loadouts, class-specific hooks
4. **Campaign quest plan** — quest-line skeleton: acts, milestones, dependencies
5. **Quest expansion** — each quest fleshed into triggers, objectives, rewards,
   dialogue, placement in the map

Cross-stage references are by ID and validated at each stage boundary (a quest may only
reference NPCs declared in stage 2, etc.). Schema design is the heart of the project —
see `docs/specs/spec-0001-dsl-schemas.md`.

## Consequences

- Each stage is independently retryable: a bad quest expansion doesn't force
  regenerating the world.
- Referential integrity is checkable mechanically at every boundary.
- The pipeline is inspectable: the owner can review/edit a stage output before later
  stages build on it.
- Cost: stage boundaries are rigid; a mechanic spanning stages (e.g. gear that changes
  mid-story) needs explicit cross-stage schema support.

## Revisit triggers

If stage boundaries prove wrong in practice (constant need for cross-stage edits),
restructure stages — the *staged, schema-per-stage* principle stays.
