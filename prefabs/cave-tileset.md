# "Mediterranean cave/shore" prefab tileset (prefab-ceiling probe)

Thirteen original jigsaw-compatible prefab pieces for the **nobodys-cave**
campaign (Odyssey Book 9 — Polyphemus's cave: bronze-age, firelight and salt
wind). Built by **direct creation** through a render-critique loop to test whether
self-created prefabs can reach showcase quality (the *prefab-ceiling probe*).
Generated deterministically by `prefabs/cave-generator` (ADR-0006), byte-identical
per run. **Structural mirror of the keep tileset** — same roles, socket geometry,
and anchor vocabulary — so `pool/cave-shore` is a drop-in for `pool/stone-keep`.

## Connection convention — `cave:socket`

Identical geometry to keep-socket-v1: every doorway is a 3-wide × 3-tall opening
centred on a wall at floor level with one `minecraft:jigsaw` block at the
bottom-centre wall cell (`name = target = pool = cave:socket / cave:pool`,
`joint = aligned`, `final_state = air`). The compiler's solver reads socket
geometry only (names are a connectivity vocabulary; `crate::solver`), so cave
pieces mate with the same machinery as keep pieces. `connectors[]` in each
metadata JSON records `local_pos`, `facing`, `opening`, `joint`. The stair
(`cave-descent`) carries the low/high y-offset sockets exactly like `keep-stair`.

> Known solver interaction: unmated sockets are sealed by the solver with
> `minecraft:stone_bricks` (hard-coded in `solver.rs`), so an unused cave doorway
> gets a smooth stone-brick patch at assembly. Per-piece renders are unaffected;
> a future compiler change could seal with a per-pool material.

## Pieces (role · size · sockets · derived min floor-light)

| id | role | size (X×Y×Z) | sockets | anchors | light |
| -- | ---- | ------------ | ------- | ------- | ----- |
| cave-shore | entry | 13×6×11 | N | `spawn`, `anchor/exit` | lit 15 (open-air) |
| cave-passage-straight | connector | 5×5×7 | N,S | — | lit 9 |
| cave-passage-corner | connector | 7×5×7 | N,E | — | lit 9 |
| cave-passage-tee | connector | 7×5×7 | N,E,W | — | lit 9 |
| cave-passage-cross | connector | 7×5×7 | N,S,E,W | — | lit 9 |
| cave-descent | connector (stair) | 5×9×11 | S(y1),N(y5) | — | lit 10 |
| cave-room-small | room | 7×5×7 | N | `anchor/npc-stand`,`anchor/chest` + hearth | lit 9 |
| cave-den | room | 9×5×9 | N,S | `anchor/npc-stand`,`anchor/wave` + sheep pen | lit 10 |
| cave-hollow | room | 9×5×7 | N,E | `anchor/npc-stand`,`anchor/door` | lit 8 |
| cave-mouth | terminal | 7×5×9 | N,S | `anchor/gate` (boulder),`anchor/keeper-stand` | lit 8 |
| cave-hearth | terminal | 9×5×9 | N | `anchor/objective` + hearth | lit 7 |
| cave-cavern | terminal (boss) | 13×6×15 | N | `anchor/boss`,`anchor/objective` + hearth + pen | lit 8 |
| cave-niche | terminal | 5×5×5 | N | — (dead-end) | lit 10 |

Palette family (one place): cobblestone / andesite / tuff / stone / mossy
cobblestone / cracked stone bricks (walls), gravel / sand / stone / coarse dirt
(floors), dripstone (ceiling accents), campfire + hanging lanterns (firelight),
pointed_dripstone (stalactites), oak fences / hay (pens), stripped/oak logs
(driftwood), water + seagrass (shore).

## Lighting

Declared **honestly and derived**, not asserted: a static flood-fill block-light
estimate over walkable floor cells sets `measured_min_light`, and the profile is
classified from it (`lit` ≥7 / `dim` 3–6 / `dark` <3). This is an **authoring
estimate, not a live 1.21.11 probe** (recorded in each JSON's `method`). All 13
pieces clear `lit`; `dim`-classified pieces (none in the shipped set, but the
`rationale` is emitted if the estimate lands in 3–6) would declare firelight
pockets as the feature. Open-air `cave-shore` is sky-lit.

## Provenance / license

All-original Delvewright assets, per-item license in each metadata JSON (GPL-3.0
pipeline-code license per `LICENSE-ASSETS.md`; ADR-0013). No third-party material
ingested.

## Render-critique loop (the probe's evidence)

Rendered with `delve-render batch` on the pinned 1.21.11 client jar; renders read
back multimodally against the brief. Three substantive rounds:

- **Round 1 (baseline).** The core risk — "does the wall read as natural rock or
  tiled noise?" — resolved in favour of **rock**: the value-noise-clustered
  palette produces coherent moss/andesite/tuff patches, not salt-and-pepper. The
  hearth shrine (moss ring + firelight) was already near-showcase. Problems seen:
  (a) `cave-cavern` came out **dark** (min-light 1 — a 13×15 vault on two hearths
  alone); (b) small corridors got **4 lanterns** (over-lit clutter) from the naive
  ≤6 grid; (c) the shore water was a **hard rectangular pool** with a straight
  edge — read as a swimming pool, not a shore.
- **Round 2 (fixes).** (a) Enabled a coarse lantern lattice on the cavern → lit 8,
  no dark pieces, firelight still pooling around the hearths + stalactite clusters.
  (b) Lantern placement now uses a single central lantern for tight interiors
  (corridors, small rooms) and a lattice only for large rooms → clean corridors.
  (c) Rewrote the shore: **irregular per-column tide line**, shallow→deep water
  gradient, scattered driftwood (varied axes) + seagrass + dead bushes. The pool
  look was gone; the cavern (stalactites + spaced firelight + sheep pen) became
  the standout piece.
- **Round 3 (polish).** The shore's wet-sand band used `coarse_dirt` and read
  **muddy**; narrowed it to a single darker-gravel tide row over clean sand. The
  beach now reads clean sand → gravel tideline → ragged shallows → sea.

## Honest self-assessment vs the brief

The pieces **read as natural rock and cohere as one place** (the main risk cleared
on round 1). The cavern and hearth are genuinely atmospheric; the shore is a
solid, storybook-leaning exterior. This is a clear step above the uniform-brick
keep shell — but it is **procedural, not hand-crafted**: shapes stay boxy (AABB
mating + pathability constrain silhouette), stalactites/rubble are noise-scattered
rather than composed, and the visible jigsaw socket block clutters beauty shots
(cleared only at assembly). Verdict: self-created prefabs can reach a **good,
coherent, thematically-legible** bar via the render loop; whether that is
*showcase* is for the owner and a blind A/B against an ingested comparable to
judge. Do not oversell.
