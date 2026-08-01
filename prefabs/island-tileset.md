# "nobodys-cave island" set-piece prefabs (spec-0013 remake)

Original open-air island prefabs for the **nobodys-cave-island** remake — the
sea-level entry beach camp and the ancient-Greek galley moored offshore (the
escape-promise seen from minute one). Built by `prefabs/island-generator`
(deterministic, byte-identical per run — ADR-0006), a sibling of
`prefabs/cave-generator` reusing its NBT / socket / gravity-substrate machinery.

These are the **set-piece** half of the island (this task). The terrain worker
owns `island-greenfield` (connector) and `island-mountain` (terminal + cavern);
those pieces MUST adopt the island convention below so the areas assemble as one
contiguous walkable island (design brief §1, §5).

## Island convention (shared — greenfield/mountain align to this)

The world horizon is `ocean` (spec-0013): a superflat with **sea level y=62**;
areas sit at y=64+ so land reads as islands. Every island piece authors its own
local geometry against these fixed local heights:

| local y | meaning |
| ------- | ------- |
| `0`     | solid base (sand/gravel substrate + seabed) — never air under a beach |
| `1..2`  | sand beach body (dry land) **or** sea water column |
| `2`     | **waterline** — the top water block; place the piece base at world `sea_level-2` (y=60) and the authored water meets the world ocean seamlessly |
| `3`     | **walkable land plane** — one block ABOVE the waterline |

**Why the walk plane is one above the waterline.** The compiler flood
(`crates/compiler/src/assembled.rs`) is a conservative superset of vanilla water
flow: it spreads horizontally (7-decay) and downward but **never climbs**. A y=3
walk surface sitting on solid-at-y=2 land therefore can never be reached by the
sea — dry standable cells are dry *by construction*, not by keeping their
distance from the water. That is what makes the tutorial `surf-wave` anchor (a
drowned wave stumbling out of the surf onto the beach) and the whole camp
flood-safe. Sand always rests on the y=0 substrate, so no gravity block is ever
unsupported (generator invariant; compiler `DW0313` is the backstop).

## Connection — `island:socket`

Keep-socket geometry (3×3 opening, one `minecraft:jigsaw` block at the
bottom-centre, `joint=aligned`, `final_state=air`) under the `island:socket`
vocabulary, at **`floor_y=2`** — the opening is based at the y=3 walk plane. The
solver reads socket geometry only, so island pieces mate with the same machinery
as cave/keep pieces. `island-beach-camp` carries one inland (north) socket to
greenfield; **greenfield's beach-facing socket must also be `floor_y=2`.**

The galley carries **no socket**: it is a standalone set-piece (like the admitted
`hero-galleon-oak`), positioned offshore by anchor offset. This is deliberate —
see "merged vs separate", below.

## Pieces

| id | role | size (X×Y×Z) | sockets | anchors |
| -- | ---- | ------------ | ------- | ------- |
| `island-beach-camp` | entry | 21×8×17 | N (`island:socket`, floor_y=2) | `entry`, `anchor/camp-fire`, `anchor/class-post`, `anchor/crew-a`, `anchor/crew-b`, `anchor/surf-wave`, `anchor/gangplank` |
| `island-galley` | set-piece | 9×15×29 | — | `anchor/deck` |

**island-beach-camp** — sand shore rising from a ragged south tide line; a
campfire ring with stripped-log benches (the campfire doubles as the relight
fixture), two wool/fence A-frame tents, a barrel supply stack, a lantern class
post, a plank gangplank jetty reaching south over the water toward the galley,
and driftwood/rock/seagrass greeble. All camp anchors sit on the dry y=3 plane.

**island-galley** — an ancient-Greek galley on its own authored water: a flared
plank hull with a dark waterline wale, a low ram (embolos) and rising stempost at
the prow, a curled aphlaston at the stern, oar rows (spruce trapdoors jutting from
both sides + button oar-ports), a single spruce mast with a white-wool square
sail and yard, and the apotropaic **eye (ophthalmos)** — white sclera + black
pupil — painted on both bows. The showcase piece; iterated on the render loop
until it reads unmistakably as a galley (ram + oars + square sail + eye).

### Merged vs separate (the set-piece decision)

The galley is a **separate** standalone piece, not merged into the beach. The
solver handles this more robustly: a standalone set-piece has zero connectors, so
there is no inter-piece socket to mis-mate across a stretch of open sea and no
cross-seam flood interaction between two water bodies. It mirrors the proven
`hero-galleon-oak` admission pattern, keeps each piece's AABB tight (rendered and
lit independently), and lets the terrain worker position the galley offshore by a
simple anchor offset. Merging would bloat the entry piece with a large water
volume and couple the galley's determinism to the beach seed for no benefit.

## Palette

Beach: stone (base) · sand / gravel (shore) · water / seagrass · campfire ·
stripped-oak / oak logs · oak fence / planks (jetty, posts) · white / light-gray
wool (tents) · barrel · lantern · cobblestone / dead bush (greeble). Galley:
spruce / oak / dark-oak planks · spruce / stripped-spruce logs · spruce stairs /
trapdoors / buttons · white / black wool (sail + eye) · lantern · barrel ·
decorated pot · water. Every id is on the `delve-admit` building allowlist
(DW0730); no command/structure blocks, no NBT-bearing block entities.

## Lighting

Open-air, sky-lit: `profile: lit`, `measured_min_light: 15` (daylight on the
exposed land/deck; the campfire and lanterns supplement at night). A static
block-light BFS is not applicable to a roofless structure — declared honestly in
each metadata JSON's `method`, matching the `hero-galleon-oak` / `cave-shore`
convention.

## Provenance / license

All-original Delvewright assets (GPL-3.0 pipeline-code license per
`LICENSE-ASSETS.md`; ADR-0013). No third-party material ingested — the generator
reuses only Delvewright's own cave-generator primitives.

## Render-critique loop

Rendered with `delve-render piece` on the pinned 1.21.11 client jar. The galley's
square sail was moved off the mast plane (one cell forward) so the mast reads
behind it instead of bisecting it into a cross; the beach tents were raised from a
2-tall wool pile to a clear 3-tall A-frame with an open front gable. Renders are
reproducible and, per the content-repo contract, **not committed** (no images in
the repo) — the catalog cards record the deterministic render paths.
