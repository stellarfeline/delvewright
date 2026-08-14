# ADR-0001: Campaign DSL → deterministic compiler → datapack

- **Status**: Accepted
- **Date**: 2026-07-29
- **Source**: kickoff handoff (2026-07-29)

## Context

Delve content is authored by an LLM. Raw mcfunction emitted by an LLM is unverifiable
at scale: syntax errors surface only at load time, quest logic errors only in play, and
no two generations are structurally comparable. We need machine-checkable authoring
output and a single place where all game commands originate.

## Decision

The LLM writes campaigns in a structured, schema-enforced **JSON DSL**. A
**deterministic compiler** translates DSL into a loadable datapack (advancements,
scoreboards, mcfunctions, structure/jigsaw references). The LLM never emits raw
mcfunction; every command in a shipped delve is compiler output.

## Consequences

- Schema validation catches malformed campaigns before any Minecraft process runs.
- Quest logic is analyzable as a graph at compile time (enables ADR-0005 static layer).
- Compiler bugs are fixed once and every past campaign benefits on recompile.
- Cost: every new gameplay mechanic requires a DSL feature + compiler support before
  the LLM can use it. This is accepted — it is the safety property, not overhead.

## Revisit triggers

None anticipated. This is the load-bearing decision of the project.
