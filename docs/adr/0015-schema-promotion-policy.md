# ADR-0015: Schema promotion policy — composition first, two gates to native

Status: Accepted (owner decision, 2026-08-04)

## Context

The demo-level phase drives DSL growth: each new mechanic forces a design
decision under the no-hacks rule (CLAUDE.md), usually with exactly one
motivating campaign (N=1). Two failure modes compete:

1. **Wrong-abstraction debt**: promoting each mechanic to a dedicated verb
   under single-example pressure yields a catalog-sized DSL. Every schema
   expansion also taxes generation quality — the authoring agent must learn
   when each verb applies (measured by the skill-learnability eval).
2. **Proof-blindness**: the DSL is not only an authoring language but a
   proof-obligation language. A mechanic expressed as an unlabeled
   composition can be invisible to the compiler's static proofs (e.g.
   `branch_points` exists for branch-completeness verification, `crush` for
   DW0378 window fairness — neither is expressiveness-motivated).

External-reviewer analysis (2026-08-04, owner-forwarded) surfaced risk 1;
the proof exemption is the project's own amendment.

## Decision

A new mechanic is first expressed as a **composition of existing
primitives**. It is promoted to a native schema structure only when at least
one of these gates is met:

- **(a) Second-campaign gate**: a second, different campaign needs the
  mechanic AND the compositional expression is demonstrably clumsy there.
- **(b) Machine-proof gate**: a static proof or diagnostic requires the
  semantic declaration; the promotion names the specific proof it enables.

While on v0.x, breaking the schema early beats freezing a wrong abstraction:
migrate fixtures rather than preserve a bad shape.

**Concurrency rule**: only one *novel* DSL design decision may be in flight
at a time. Mechanical field additions that follow an established idiom (e.g.
the version-fence pattern) may parallelize; every PR touching the DSL surface
lists its surface changes in a dedicated PR-description section so idiom
drift is reviewable.

Campaigns blocked on an unpromoted mechanic record the pain in their
GENERATION.md — that record is the evidence for gate (a).

## Consequences

- The DSL surface grows by need-proven steps; the skill-learnability eval
  (fixed prompt battery; repair-round and DW-distribution metrics) tracks
  whether the surface is taxing generation quality.
- Promotions carry their justification in the spec/PR (which gate, which
  second campaign or which proof).
- Some mechanics stay compositional forever; that is a success case, not a
  deferral.
