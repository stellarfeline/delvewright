# spec-0010 — Assembled-world lighting: measurement + deterministic relight pass

Status: Approved (owner-approved in conversation, 2026-07-31)
Amends: spec-0001 "Lighting contract" (DW0210 evidence base), spec-0007 (admission
probe role). Depends on: nav occupancy world (spec-0008 v0.4.1), environment
sealing time lock (spec-0002).

## Motivation

Today's lighting gate (DW0210) is second-hand: it reads the per-piece profile
recorded at admission and judges whole prefabs. It cannot see darkness at piece
seams, it counts sealed cavities a player can never enter (hollow-statue false
darks), and when it fires the only remedy is a night-vision kit item. There is
no supplemental-lighting path at all. Meanwhile the compiler already builds a
voxel model of the assembled world (nav) and time is pinned to noon by
environment sealing — so real, deterministic light measurement over the
assembled world is available at compile time.

## Design

### DSL surface (stage 1, per area, optional)

```json
"lighting": { "fixture": "lantern", "min_light": 7 }
```

- `fixture`: one of the fixture registry ids (below). Theme choice is a creative
  decision and stays in the DSL layer.
- `min_light`: 1..=14, default 7. The guarantee applies to reachable walkable
  cells only.

### Compiler pass (after solve + assembly model, before nav verification)

1. Build a light-voxel field over the assembled world model: per-cell opacity +
   emitter table (same algorithm family as the cave-generator's static
   estimator; block-light values per 1.21.11).
2. Sky exposure is computed geometrically; sky-open cells count sky light at the
   sealed noon time lock (spec-0002 invariant). If a future spec unpins time,
   this clause must be revisited before merge.
3. Flood block light + sky light deterministically; collect **reachable walkable
   cells** (nav reachability from the area's entry anchors) below the target.
4. **Relight placement** (only for areas with a `lighting` declaration):
   deterministic greedy loop — pick the darkest deficient cell (ties broken by
   ascending (y, z, x)), place the declared fixture at the best valid site per
   the fixture's placement rule, incrementally re-flood, repeat until no
   deficient cells or no valid site remains (then DW0211).
5. Placements must never occupy or obstruct a required nav path cell and never
   replace non-air blocks except where the fixture rule says wall/ceiling-embed.
   Nav verification re-runs after placement.

### Fixture registry (v1)

| id | placement rule |
|----|----------------|
| `torch` | on solid floor, off required paths; `wall_torch` on a wall face as fallback |
| `lantern` | hanging under a ceiling block; floor-sitting as fallback |
| `campfire` | on solid floor with headroom, never on or adjacent to required path cells (damage source) |
| `shroomlight` | embedded: replaces a solid wall/ceiling block |

### Emission

Fixtures are emitted as `setblock` commands in the existing world-init path
(spec-0002 sealing/init function ordering, after structure placement) — the
world is assembled in-game by jigsaw, so post-assembly block writes are the
intended vanilla mechanism (consistent with v0.4 `SetBlock`).

### Mitigation hierarchy (DW0210 redefined)

For each area, judged on **measured assembled light over reachable walkable
cells** (admission profiles are no longer an input to gating):

1. `lighting` declared → relight pass guarantees `min_light`; unsatisfiable →
   **DW0211** (error).
2. No declaration, measured min ≥ 3 → ok.
3. No declaration, measured min < 3, a class kit grants night-vision → ok
   (retained mitigation, owner decision 2026-07-31).
4. Otherwise → **DW0210** (error): dark area, no declared fixture, no
   night-vision.

Sealed cavities are unreachable by construction and never counted — this
resolves the hollow-statue false-dark class.

### Admission probe (spec-0007) demoted to advisory

`delve-admit lighting` keeps measuring and classifying (lit/dim/dark) as a
**selection signal** on catalog cards/metadata only; it no longer feeds any
gate. Its sealed-cavity limitation (counts unreachable interiors) is documented
and accepted at that advisory tier.

## Diagnostics

- `DW0210` (redefined): assembled-measured dark area with no declared fixture
  and no night-vision mitigation. Error, exit 2.
- `DW0211` (new): relight pass cannot reach `min_light` with the declared
  fixture (no remaining valid placement site). Error, exit 2.

## Acceptance criteria

1. Determinism: same DSL + seed → byte-identical relight `setblock` emission
   (ADR-0006 suite covers the new pass).
2. A campaign binding a `dark` piece with `lighting:{fixture:"lantern"}`
   compiles; the static model proves every reachable walkable cell ≥ 7; the
   emitted init function contains only registry fixture blocks.
3. Sealed-cavity fixture piece: no fixture placed inside the cavity; no DW0210.
4. Seam fixture: two individually-lit pieces joined by a dark seam corridor →
   at least one fixture emitted in the seam region; assembled min ≥ target.
5. Night-vision path preserved: dark area, no `lighting` declaration, kit
   grants night-vision → builds clean.
6. Dark area with neither declaration nor night-vision → DW0210, exit 2.
7. Declared but unsatisfiable placement → DW0211, exit 2.
8. Relight placements never break walkability: nav verification passes on every
   green fixture above.
9. Sky-open shore piece at noon lock: no fixtures demanded for sky-lit cells.

## Non-goals

- Runtime/dynamic lighting, variable time-of-day (time lock is a sealing
  invariant), aesthetic light-art direction beyond fixture choice, and fixing
  the admission probe's sealed-cavity counting (advisory tier, documented).
