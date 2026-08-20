# "nobodys-cave island" set-piece prefabs (spec-0013 remake)

Original open-air island prefabs for the **nobodys-cave-island** remake — the
sea-level entry beach camp and the ancient-Greek galley moored offshore (the
escape-promise seen from minute one). Built by `prefabs/island-generator`
(deterministic, byte-identical per run — ADR-0006), a sibling of
`prefabs/cave-generator` reusing its NBT / socket / gravity-substrate machinery.

These are the **set-piece** half of the island. The terrain worker
built `island-greenfield` (connector) and `island-mountain` (terminal + cavern);
those pieces adopt the island convention below so the areas assemble as one
contiguous walkable island (design brief §1, §5).

## Island convention (shared — greenfield/mountain align to this)

The world horizon is `ocean` (spec-0013): a superflat with **sea level y=62**.
The compiler places an island area at **y=60** via the per-area datum
(spec-0026 §2: `walk_ref_y (63) − walk_y (3)` — every island piece declares
`walk_y: 3` in its metadata, generator-emitted; `DW0367` without it). The old
global `sea_level − 2` constant this convention used to assume is retired: the
island's placement is unchanged, but other tilesets now land on their own
declared walk planes instead of the island's. The piece's walk plane (local
y=3) lands at world y=63, one block above the sea, exactly like a vanilla
beach. Every island piece authors its own local geometry against these fixed
local heights:

| local y | meaning |
| ------- | ------- |
| `0`     | solid base (sand/gravel substrate + seabed) — never air under a beach |
| `1..2`  | sand beach body (dry land) **or** sea water column |
| `2`     | **waterline** — the top water block; with the piece base at world `sea_level-2` (y=60) the authored water meets the world ocean seamlessly |
| `3`     | **walkable land plane** — one block ABOVE the waterline |

Every piece declares this waterline in its metadata as **`waterline_y: 2`**, and
the compiler *enforces* that it lands at sea level when the area is placed
(`DW0344`). A piece built to a different datum is a build error, not a subtle
in-world defect: a mis-datumed island validates green — nav, boundary, POV and
PackTests all derive from placement — and ships as an island floating above an
inescapable sea.

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

The galley carries **no socket**. It exists two ways (see "merged vs separate",
below): as the standalone `island-galley` set-piece (a reusable offshore ship,
the `hero-galleon-oak` pattern), and — for the nobodys-cave campaign — **stamped
into `island-beach-camp`** so it is moored just offshore and boardable, since the
DSL has no scenery-offset primitive to place a standalone ship a few blocks off a
neighbouring area.

## Pieces

| id | role | size (X×Y×Z) | sockets | anchors |
| -- | ---- | ------------ | ------- | ------- |
| `island-beach-camp` | entry (+ moored galley) | 21×15×44 | N (`island:socket`, floor_y=2) | `entry`, `anchor/camp-fire`, `anchor/class-post`, `anchor/crew-a`, `anchor/crew-b`, `anchor/surf-wave`, `anchor/gangplank`, `anchor/deck`, `anchor/prow` |
| `island-galley` | set-piece (standalone) | 9×15×29 | — | `anchor/deck` |

**island-beach-camp** — sand shore rising from a ragged south tide line; a
campfire ring with stripped-log benches (the campfire doubles as the relight
fixture), two wool/fence A-frame tents, a barrel supply stack, a lantern class
post, and driftwood/rock/seagrass greeble — all camp anchors on the dry y=3
plane. The piece extends south into authored ocean where **the Greek galley is
stamped in, moored just offshore**: a bounded jetty runs off the sand and a
walkable **gangplank** (spruce treads on oak-fence piles) climbs from the jetty
head at y=3 up onto the galley deck at y=5, every stand cell rising ≤1 with ≥2
air overhead (DW0311-walkable, verified against the emitted NBT). Deck lanterns
fore and aft light the ship for the dusk beat. `anchor/deck` (boarding target,
faces the camp) and `anchor/prow` (scenic bow, ending beat) sit on the deck walk
plane y=5. Every pre-galley beach anchor keeps its original local coordinate; the
piece only grew +Z (open-sea side) and +Y (mast height).

**island-galley** — an ancient-Greek galley on its own authored water: a flared
plank hull with a dark waterline wale, a low ram (embolos) and rising stempost at
the prow, a curled aphlaston at the stern, oar rows (spruce trapdoors jutting from
both sides + button oar-ports), a single spruce mast with a white-wool square
sail and yard, and the apotropaic **eye (ophthalmos)** — white sclera + black
pupil — painted on both bows. The showcase piece; iterated on the render loop
until it reads unmistakably as a galley (ram + oars + square sail + eye).

### Merged vs separate (the set-piece decision)

We keep **both**: `island-galley` stays a standalone, reusable set-piece, and the
same hull is **merged into `island-beach-camp`** for the nobodys-cave campaign.

The merge is forced by a real constraint, not preference. Placing the galley "just
offshore" from the beach needs it a few blocks off the sand — but the DSL exposes
no scenery-offset primitive, and the solver spaces areas ~256 blocks apart, so a
*standalone* galley area could never sit offshore of the beach; the two would land
a quarter-kilometre apart with no bridge. (This was the reserved fallback all
along — design brief §5 and this generator's doc comment.) Merging puts the ship
in the beach piece's own authored ocean, so the campaign's critical path can walk
the gangplank onto the deck as one contiguous area.

The old worry that motivated "separate" — cross-seam flood between two water
bodies — is avoided by construction: `stamp_solids` copies only the hull's SOLID
cells over the beach's existing sea, so the waterline (local y=2) and seabed stay
a single water volume, never a second one. The standalone `island-galley` still
exists for any campaign that wants a ship positioned by socket/assembly rather
than moored to a specific beach.

## Terrain pieces (greenfield + mountain)

Built by the sibling `prefabs/island-terrain-generator` (its own `[workspace]`),
these complete the contiguous island: the connectors between the beach camp and the
mountain, and the mountain terminal with its cavern. All adopt the island convention
above (`island:socket` at `floor_y=2`, walk plane y=3) — built at the ground datum,
then lifted +2 onto a solid substrate so every socket/anchor lands on the shared
datum and mates with the beach camp's north socket.

| id | role | size (X×Y×Z) | sockets | anchors |
| -- | ---- | ------------ | ------- | ------- |
| `island-greenfield` | connector | 17×10×15 | S, N (`island:socket` floor_y=2) | `anchor/meadow`, `anchor/fold` |
| `island-greenfield-bend` | connector | 17×10×15 | S, E | `anchor/meadow`, `anchor/fold` |
| `island-mountain` | terminal | 36×28×42 | S (base, floor_y=2) | `anchor/mouth`, `anchor/boulder` (gate region), `anchor/cheese-store`, `anchor/fire-pit`, `anchor/ramp-top`, `anchor/pen`, `anchor/alcove-1..4`, `anchor/checkpoint-1..3`, `anchor/shaft-1..2` |

**island-greenfield / -bend** — open-air, sky-lit grazing meadow in a shallow grassy
dell: a flat walkable floor between the two sockets, a worn dirt path spine, scattered
scrub oaks, poppy/daisy/cornflower flowers, and a low mossy-cobblestone **empty sheep
fold** (foreshadowing — the sheep are the Cyclops'). The bend variant elbows S→E for
layout flexibility. Both are `lit` 15.

**island-mountain** — a solid rock massif built **fill-then-carve** (the exterior
silhouette is the domed rocky cap left after carving; a crown/face erosion pass breaks
the box outline, enclosure-safe). A terraced **switchback path** — grass-to-stone
gradient plus a coarse-dirt trail, a stair tread on every riser so it is walked
natively — climbs the south face to a **cave-mouth ledge**; the **boulder gate region**
(`anchor/boulder`, basalt: what `open-gate`/`set-block` fills to seal or open the
mouth) sits at the mouth, with a decorative Chekhov boulder beside it. Where that
boulder's blob reaches out over the ledge terrace it **weathers the tread**
(`cobblestone_stairs` → `mossy_cobblestone_stairs`, same facing) instead of dropping a
rock on it — `distress_blk`, after the owner found stray stone standing on the
cave-mouth steps (round 13). Distress embeds, it never stacks, and
`invariants::assert_distress_never_stacks` now fails any tileset that regresses. The
mouth opens
into ONE tall-wide **cavern hall** (interior 30×14×24, NOT rooms-and-corridors): a
cheese store by the entry, a central fire pit (baked lit campfire = the relight
fixture), a **rock-shelf ramp** (no ladders) up to an empty upper sheep pen, four dark
**shadow alcoves** for the stealth beats, dripstone + moss dressing, and **two ceiling
light shafts** open to the sky.

The cavern is declared **`dark`** honestly (firelit at the pit, dark at the vault edges
and alcoves by design). Rock fixture sites near every reachable cell and the two
sky-open shafts let the compiler relight the declared area minimally (spec-0010) while
the hall still reads dark for stealth. A 3D nav flood from the base socket reaches
every anchor (switchback → mouth → cavern → ramp → pen) with ≤1-block steps; the
compiler's `DW0311` is the authoritative critical-path gate at assembly.

## Palette

Beach: stone (base) · sand / gravel (shore) · water / seagrass · campfire ·
stripped-oak / oak logs · oak fence / planks (jetty, posts) · white / light-gray
wool (tents) · barrel · lantern · cobblestone / dead bush (greeble). Galley:
spruce / oak / dark-oak planks · spruce / stripped-spruce logs · spruce stairs /
trapdoors / buttons · white / black wool (sail + eye) · lantern · barrel ·
decorated pot · water. The merged `island-beach-camp` therefore draws from both
lists (the galley palette is stamped into it, plus spruce planks / oak fence for
the gangplank). Every id is on the `delve-admit` building allowlist (DW0730); no
command/structure blocks, no NBT-bearing block entities — re-audited after the
merge on both regenerated pieces.

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
