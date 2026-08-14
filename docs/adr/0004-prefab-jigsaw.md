# ADR-0004: Prefab library assembled via vanilla jigsaw

- **Status**: Accepted
- **Date**: 2026-07-29
- **Source**: kickoff handoff (2026-07-29)

## Context

Block-by-block LLM generation of maps is slow, incoherent at architectural scale, and
impossible to validate aesthetically by machine. Curated building blocks with
mechanical assembly give consistent quality and bounded validation surface.

## Decision

Maps are assembled from a **prefab library**: hand-curated (or owner-approved) `.nbt`
structure files with metadata, composed via **vanilla jigsaw / `template_pool`
mechanics**. The compiler controls jigsaw seeds so layout is reproducible (ADR-0006).
The DSL references prefabs and pools by ID; it never describes blocks.

Prefab provenance (author, license) is recorded in metadata. Only original, CC0, or
CC BY assets are admitted (ADR-0007).

**Fallback (not built now)**: GDPC-style offline assembly — the compiler placing
structures into the world file directly — if jigsaw's layout control proves
insufficient.

## Consequences

- Map quality is a curation problem (grow the library) not a generation problem.
- Layout validation reduces to graph properties over placed pieces.
- Jigsaw constraints (pool weights, piece connectivity, depth limits) become part of
  the compiler's output surface.
- Prefabs are binary assets → git-lfs (`prefabs/`).

## Verification (2026-07-30)

The M2 seed-stability experiment (`docs/experiments/m2-jigsaw-seed-stability/`)
confirmed `/place jigsaw` layout is a **pure (world seed, position) function** on
pinned 1.21.11 — 6/6 fresh worlds identical per seed, order-independent. The GDPC
fallback below remains documented but is not needed.

## Amendment (2026-07-30): the compiler is the jigsaw

The **compiler** solves the layout and emits per-piece
`/place template <piece> <pos> <rotation>` calls, rather than `/place jigsaw` at runtime.
Runtime jigsaw is verified reproducible (above), but predicting its layout means
replicating Mojang's algorithm; solving it in-compiler is simpler and keeps the
shipped delve plain vanilla (ADR-0003), determinism trivial (campaign-seeded
PRNG, ADR-0006), and gives full layout knowledge for anchors, the critical path,
and global constraints. This promotes the "Fallback status" mechanism the
seed-stability experiment documented to the **primary** path; `keep:pool` socket
names become a connectivity vocabulary the solver reads (`crate::solver`).

## Revisit triggers

Adopt the offline-assembly fallback if any of these occur:
- Jigsaw cannot guarantee critical-path connectivity for a required layout pattern.
- Seed-controlled jigsaw output proves non-reproducible across server runs of the
  pinned version.
- Layout requirements need global constraints jigsaw can't express (e.g. "boss room
  farthest from entrance").
