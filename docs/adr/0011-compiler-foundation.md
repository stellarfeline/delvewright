# ADR-0011: Compiler foundation — Rust-native, with mecha as CI cross-check

- **Status**: Accepted
- **Date**: 2026-07-29
- **Source**: owner decision rule (2026-07-29 planning session): "don't reinvent
  wheels, but if our overlap with beet is low or second-development is heavy, go
  our own way" — applied to the overlap analysis below

## Context

Decision Point #1 from the kickoff: build the compiler on beet+mecha (Python, MIT,
actively maintained) or Rust-native. The owner's rule makes this hinge on **how much
of our compiler beet actually provides**. Overlap analysis:

| Our need | What beet/mecha provides |
|---|---|
| Staged JSON DSL + referential validation (spec-0001) | Nothing — custom either way |
| Quest-graph reachability analysis (ADR-0005 static) | Nothing — custom either way |
| Quest semantics → advancements/scoreboards/functions | Nothing (this is our domain logic); beet only plumbs the files |
| Datapack file layout, pack.mcmeta, zipping | Yes — but thin: a fixed directory scheme + JSON files, frozen by our version pin |
| mcfunction **parsing/validation** vs 1.21.11 command tree | Yes — mecha's genuinely valuable piece |
| mcfunction **emission** | We *generate* a closed, compiler-controlled command subset; string emission, not parsing — mecha's parser adds little |
| Byte-level determinism (ADR-0006) | Not guaranteed; third-party serialization internals would need auditing on every upgrade |
| Jigsaw/template_pool JSON, `.nbt` handling | Generic JSON + nbtlib; Rust equivalents exist (fastnbt/valence_nbt, MIT) |

The heart of the project is custom in either world. beet's real overlap is file
plumbing (small, and frozen by the ADR-0009 pin) plus mecha's command-tree validation
(consumable without building on it). Meanwhile beet/mecha track *latest* MC while we
pin 1.21.11 forever, and the determinism invariant is cheapest to enforce when we own
every output byte.

## Decision

- The compiler is **Rust-native** (`crates/compiler`), emitting datapack JSON,
  mcfunction text, and NBT directly; command syntax statically checked against the
  **vendored 1.21.11 command tree** (Mojang generated-data reports, checked into the
  repo).
- **mecha (pinned to the 1.21.11 command tree) runs as an independent cross-check in
  CI** — a tooling-layer step (ADR-0003 pattern) that re-validates our emitted
  mcfunction. Disagreement between our validator and mecha fails CI and is a bug in
  one of them. Python exists only in this CI step, never in the build path.
- beet is not used.

## Consequences

- M1 is slower: emission and validation are built, not imported. Accepted cost.
- Determinism (ADR-0006) is enforced in first-party code; no upstream serialization
  drift can break byte-identity.
- Single-language `crates/` workspace; owner reviews in her strongest language.
- The vendored command tree is part of the version pin: it changes only if ADR-0009's
  revisit triggers fire.

## Revisit triggers

If the emitted command surface outgrows the closed-enum design (spec-0001) to the
point where maintaining first-party validation is disproportionate, re-evaluate
building emission on mecha. The DSL and quest-graph layers are unaffected by any such
switch.
