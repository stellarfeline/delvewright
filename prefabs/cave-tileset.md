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

**Walk-plane datum (spec-0026 §2)**: the cave convention is substrate + floor
top local y=1, feet at local **y=2** — declared as `walk_y: 2` in every piece's
metadata (generator-emitted; `DW0367` without it in a non-void horizon).
`cave-shore`'s beach walks flush at local 2 with its authored water topping at
the same y (an ankle-deep wading shore) — under an ocean horizon its datum
places that walk plane at 63, one above sea level, and its authored pond then
sits one block above the world sea; it declares no `waterline_y`, so `DW0344`
does not demand sea mating (the piece predates the island convention and no
tracked ocean campaign binds it).

> Known solver interaction: unmated sockets are sealed by the solver with
> `minecraft:stone_bricks` (hard-coded in `solver.rs`), so an unused cave doorway
> gets a smooth stone-brick patch at assembly. Per-piece renders are unaffected;
> a future compiler change could seal with a per-pool material.

## Pieces (role · size · sockets · derived min floor-light)

| id | role | size (X×Y×Z) | sockets | anchors | light |
| -- | ---- | ------------ | ------- | ------- | ----- |
| cave-shore | entry | 13×6×11 | N | `spawn`, `anchor/exit` | lit 15 (open-air) |
| cave-passage-straight | connector | 5×5×7 | N,S | — | lit 10 |
| cave-passage-corner | connector | 7×5×7 | N,E | — | lit 9 |
| cave-passage-tee | connector | 7×5×7 | N,E,W | — | lit 9 |
| cave-passage-cross | connector | 7×5×7 | N,S,E,W | — | lit 9 |
| cave-descent | connector (stair) | 5×9×11 | S(y1),N(y5) | — | lit 10 |
| cave-room-small | room | 7×5×7 | N | `anchor/npc-stand`,`anchor/chest` + hearth | lit 10 |
| cave-den | room | 9×5×9 | N,S | `anchor/npc-stand`,`anchor/wave` + sheep pen | lit 7 |
| cave-hollow | room | 9×5×7 | N,E | `anchor/npc-stand`,`anchor/door` | lit 9 |
| cave-mouth | terminal | 7×5×9 | N,S | `anchor/gate` (boulder),`anchor/keeper-stand` | lit 9 |
| cave-hearth | terminal | 9×5×9 | N | `anchor/objective` + hearth | lit 8 |
| cave-cavern | terminal (boss) | 13×6×15 | N | `anchor/boss`,`anchor/objective` + hearth + pen | lit 8 |
| cave-niche | terminal | 5×5×5 | N | — (dead-end) | lit 11 |

Palette family (one place): cobblestone / andesite / tuff / stone / mossy
cobblestone / cracked stone bricks (walls), gravel / sand / stone / coarse dirt
(floors), dripstone (ceiling accents), campfire + hanging lanterns (firelight),
pointed_dripstone (stalactites), oak fences / hay (pens), stripped/oak logs
(driftwood), water + seagrass (shore).

## Lighting

Declared **honestly and derived**, not asserted: a static flood-fill block-light
estimate over walkable **interior** floor cells sets `measured_min_light`, and the
profile is classified from it (`lit` ≥7 / `dim` 3–6 / `dark` <3). Doorway-mouth
threshold cells (walkable cells on the piece boundary) are excluded — the shell is
solid wall there except where a socket is carved, so an open doorway is dark only
in isolation and mates against the lit neighbouring piece at assembly; measuring
the interior is the honest "does the room read as lit?" question and can only raise
the min. This is an **authoring estimate, not a live 1.21.11 probe** (recorded in
each JSON's `method`). All 13 pieces clear `lit`; `dim`-classified pieces (none in
the shipped set, but the `rationale` is emitted if the estimate lands in 3–6) would
declare firelight pockets as the feature. Open-air `cave-shore` is sky-lit.

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

### Algorithm-adoption pass (round-2 tileset task)

A second, larger pass adopts the researched GDMC/PCG algorithms (attribution in
`docs/ACKNOWLEDGEMENTS.md`) to fix the two round-1 defects a player sees —
**rectangular skeletons** and **hard water edges** —
without moving any socket (all pieces stay a byte-compatible drop-in for the solver;
same seed → byte-identical `.nbt`). Techniques re-implemented from description
(ideas-only; no code ingested):

- **(A1) weathering palette bias.** Block choice is biased by an edge-distance /
  height term on top of the value-noise field, so mossy + cracked variants
  concentrate near floors and walls — a wear gradient, not uniform speckle.
- **(A5) silhouette / edge roughening.** A ragged eroded crown along the roofline,
  chamfered vertical corners, and outer-face divots on thick pieces break the box
  outline into a rock massif with a cave inside. The roughening only *removes* shell
  rock (never crosses the AABB) and skips doorway columns, so sockets are untouched.
- **(A8) CA cave shaping.** A 4-5-rule cellular automaton perturbs the inner wall
  face into alcoves and bumps (above walk height, so the floor path stays clear),
  giving organic interiors instead of straight walls.
- **Soft shoreline.** `cave-shore`'s rectangular pool is replaced by a graded
  sand → wet-gravel → sand-bottom shallow → stone-bottom sea that laps in from the
  open front, with a ragged inlet tide line; the depth grows from the **seabed
  dropping**, not water stacking, so there is no vertical wall of water. The spawn
  now lands dry on the beach at the water's edge.

Before/after renders: `delve-demos/cave-tileset-renders/round2-algorithms/`.

### Round-2 walkability regression fix

The round-2 organic pass shipped two latent **seam-walkability** defects that a
full assembly (`nobodys-cave`, seed 1184) surfaced — the critical-path bot dead-ended
with "No path to the goal!" and wave sheep wandered off the map to y=-163. Both are
now fixed in the generator; the pieces stay a **byte-compatible drop-in** (ids, sizes,
socket placements, anchors, and all metadata JSON are byte-identical — only three
`.nbt` bodies change: `cave-mouth`, `cave-cavern`, `cave-shore`), and the round-2 art
(organic silhouettes, graded shoreline) is preserved.

- **Decoration in the walk envelope → wedged doorway seams.** The detailing pass
  scattered `glow_lichen` across interior wall faces starting at `y=2` — head height
  for a 2-tall entity standing on the `y=0` floor. In a *narrow* piece (e.g. the
  3-wide `cave-mouth` throat) a lichen patch spanned the full width of the doorway
  throat, and a block-occupancy pathfinder (the compiler's `nav` A* **and** the
  mineflayer bot) reads a plant curtain across a corridor as a wall. Fix: wall
  greebles now start at `y=3`, so nothing ever lands in the walk envelope (feet `y=1`,
  head `y=2`); the lichen hugs the upper wall / ceiling line, the walk tube through
  every socket stays clear rock-and-air.
- **Open-air cove leaked entities to the void.** `cave-shore` is open-air; its side
  cliffs tapered *down* toward the sea (`sy-1 - z/2`) and its front was fully open.
  Packed against enclosed cave pieces, the tapered sides were a climbable stair
  (≤1 block/step) up onto the piece **roofscape** — whose eroded crowns leave
  standable pockets open to the void at the outer edges — and the open front let an
  entity walk/swim straight off the world's edge. Fix: all four enclosing cliffs now
  rise the full piece height, so the cove is a bounded, sky-lit lagoon (箱庭): open
  upward only, walled against the void on every horizontal edge. The graded shoreline,
  seabed depth profile, water, and scatter inside the basin are unchanged.

Both are now caught at **compile time** by `DW0311` (critical-path walkability over
the assembled geometry, `crates/compiler/src/nav.rs`), so this bug class fails the
build instead of a bot run: with the pre-fix `.nbt`, `delvec build nobodys-cave`
fails `DW0311` on the entry→cavern leg; with the fixed `.nbt` it routes clean and no
walkable cell borders the void.

### Pen placement / flock-crowding fix

Follow-up from the nobodys-cave bot ladder: the sheep pens sat on the proven
corridor, and the wave-spawned flock crowded against a pen fence at its spawn cell.
Two placement fixes (only `cave-den` + `cave-cavern` `.nbt` change; sizes, sockets,
connectors, and anchors byte-identical — the sole metadata delta is `cave-den`'s
re-derived `measured_min_light` 7→9, since removing the fence maze lets the light
estimate reach more floor; still `lit`):

- **`cave-den` pen removed.** The den is a 9×5×9 room whose 5×5 interior is *entirely*
  consumed by the door-to-door critical corridor (both the N and S sockets open on
  the same `x=3..5` columns, so nav's A* fills them). Any fenced pen there sits on or
  immediately flanks the proven path — inflating the mineflayer A* search space — and
  the round-2 pen's west fence ran one cell off the wave-spawn cell, boxing the six
  spawning sheep against the corridor. The flock is a `spawn-wave`, not penned stock,
  so the pen was pure dressing; it is dropped here (the representative pen lives in
  `cave-cavern`). Removing it de-crowds the spawn and clears the corridor.
- **`cave-cavern` pen relocated** from the path-hugging `x9..11, z3..5` (a fence line
  on the wall, ~3 cells off the centreline the path runs) to the front-west alcove
  `x2..3, z2..6`, hard against the wall and ≥3 cells clear of both the `x=6` path and
  the `x5..7` door span — off the corridor, still good greeble.

The den piece is provably mob-tight (its shell is solid except the two mated doors;
floor and ceiling intact over the whole walkable area), and nav's flock-containment
model finds **zero** walkable cells bordering the void across the assembled cave
after the fix — the flock is contained.

### The sea was written above the beach

The graded shoreline above put the water surface at `y=2` and the beach's top
solid block at `y=1` — the sea standing a block **proud** of the sand it laps.
Every cell of it along the ragged coast, and along the pinned-dry spawn strip,
therefore sat beside the air over the beach: 33 fluid cells with **seven ways
out**, which the world flattens on the first tick. The `.nbt`, the review render
and the contact sheet all drew still water, so nothing said a word until the
admission audit's fluid rule (`DW0800`) was first run over the library.

Two things were wrong, and the second was hiding inside the paragraph that
claimed to have fixed it:

- **The surface plane.** A shoreline's water surface is the same plane as the
  wet sand it meets — vanilla's own beaches put the topmost water block level
  with it and the dry sand one above. The sea is now at `y=1`. The piece's own
  bottom layer is its floor, so the water is one cell deep and the depth
  gradient the round-2 art carried in the water column is carried by the **bed**
  instead: sandstone where the shallows read sandy, stone where the sea reads
  deep, both non-falling because a gravity block on the bottom layer rests on
  nothing.
- **The front cliff was never there.** "Walled against the void on every
  horizontal edge" was written, and the surface loop then ran to `z = sz`, so it
  overwrote the front row's cliff with beach and sea for two of its rows. The
  cove was open at the front from `y=1` to `y=2` and a swimmer left the piece
  through it; the audit counted sixteen run directions leaving the piece's outer
  face. The loop now stops at `z = sz - 1`, and the count is zero.

Only `cave-shore.nbt` changes; its size, sockets, connectors, anchors and every
byte of its metadata JSON are identical. The class is now caught **at the
emitter**: `invariants::assert_fluid_is_contained` runs in every generator
before any bytes are written, sharing the block rules with `delve-admit audit`
rather than restating them, and prints each piece's binding.

## Honest self-assessment vs the brief

The pieces **read as natural rock and cohere as one place** (the main risk cleared
on round 1). The cavern and hearth are genuinely atmospheric; the shore is a
solid, storybook-leaning exterior. This is a clear step above the uniform-brick
keep shell — but it is **procedural, not hand-crafted**: the round-2 adoption pass
breaks the box silhouette (eroded crown, chamfered corners, CA-perturbed walls) and
softens the shoreline, but the envelope is still AABB-bounded and pathability-
constrained, greebles are noise-scattered rather than composed, and the visible
jigsaw socket block clutters beauty shots (cleared only at assembly). Verdict:
self-created prefabs can reach a **good,
coherent, thematically-legible** bar via the render loop; whether that is
*showcase* is for the owner and a blind A/B against an ingested comparable to
judge. Do not oversell.
