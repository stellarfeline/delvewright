# ADR-0006: Determinism as a hard invariant

- **Status**: Accepted
- **Date**: 2026-07-29
- **Source**: kickoff handoff (2026-07-29)

## Context

Validation results are only trustworthy if the artifact validated is the artifact
shipped. Debugging generated content requires reproducing it exactly. LLM generation is
inherently nondeterministic — so everything downstream of the DSL must not be.

## Decision

**Same DSL + same seed → byte-identical datapack and byte-identical world.** This is a
hard invariant enforced by tests from day one (double-compile + hash-compare in CI, on
every push, starting with the very first compiler commit).

Rules this implies for the compiler:
- All randomness flows from the single campaign seed through a named, versioned PRNG.
- No wall-clock timestamps, hostnames, absolute paths, or locale-dependent formatting
  in any output byte.
- Iteration order over collections is defined (sorted or insertion-ordered), never
  hash-map order.
- Output archives (zip/OCI layers) are built reproducibly: fixed mtimes, sorted
  entries, fixed metadata.

## Consequences

- CI can prove "the thing the bot completed is bit-for-bit the thing in the release".
- Any diff between two builds of the same input is by definition a bug.
- Dependencies must be vetted for deterministic behavior (e.g. NBT serialization
  ordering) — a criterion in Decision Point #1 (compiler foundation choice).
- World determinism depends on the server generating jigsaw content identically from a
  fixed seed on the pinned version — verified by test in Milestone 1; if it fails, that
  triggers the ADR-0004 fallback.

## Revisit triggers

None. If a feature can't be made deterministic, the feature changes, not the invariant.
