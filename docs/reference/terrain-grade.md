# How built ground meets natural ground — established practice

For the engine developer deciding what the terrain does beside a place whose floor stands higher or lower than the land next to it. It answers one question: when a flat built thing is set into uneven ground, who moves — the thing or the ground — where, how far, and with which blocks. The rules built on it are in spec-0138; this page holds only what was found.

**The gap this fills.** `compiler.md` (*The model*) says who owns each cell beside a place and stops at the ring: a ring column's fixed ground is "the site's terrain continued to the plot's edge". `place-craft.md` §6 says what the ground does at a seam between two places. Neither says what the terrain owes a place it does not meet at its own height.

## 1. What was read

| # | Source | Terms | Read how |
|---|---|---|---|
| S1 | The pinned server jar (`versions.toml [minecraft].server_jar_url`, sha256 `f83b8e093865806f931c7e34aae41b177d4c076335263dd124c75d6d65dd1726`), its inner `META-INF/versions/1.21.11/server-1.21.11.jar` (sha256 `ec47239a8de246335e1d54f6ac319bd35641778eb4b6a6da06372840d02fcebc`), with the server mappings piston-meta publishes (sha1 `5621e9253f05fd57872bbe7f8ddf5f9a7d525955`) | Mojang's; read, nothing copied | `javap -p -c -constants` from a JDK 21 over `net.minecraft.world.level.levelgen.Beardifier`, `Beardifier$Rigid` and `net.minecraft.world.level.levelgen.structure.TerrainAdjustment`; the 34 files of `data/minecraft/worldgen/structure/*.json` tallied by their `terrain_adaptation` key |
| S2 | Microsoft Learn, *Jigsaw Structures Documentation* (`learn.microsoft.com/en-us/minecraft/creator/reference/content/worldgenreference/examples/jigsawjigsawstructures`), the `terrain_adaptation` table | Microsoft's documentation, terms not established here; short quotation, nothing adopted | the page text, whole. It documents the Bedrock edition; it is used only for the vendor's own words on what each value is for |
| S3 | WorldEdit documentation, *Region Operations* (`worldedit.enginehub.org/en/latest/usage/regions/regionops/`) | No licence shown on the page; ideas only | **through a page summary only**; its quotations below are relayed |
| S4 | Salge, Green, Canaan and Togelius, *Generative Design in Minecraft (GDMC), Settlement Generation Competition* (arXiv:1803.09853) | arXiv; licence not established here; short quotation | **through a search summary only**; the judging criterion below is relayed |
| S5 | Salge and others, *Impressions of the GDMC AI Settlement Generation Challenge in Minecraft* (arXiv:2108.02955), the judges' appendix B | arXiv; licence not established here; short quotation | **through a page summary only**, which read all but the last 1,583 characters of the page; every quotation below is a relayed fragment |
| S6 | This repository's `docs/reference/body-movement.md` §4 and `docs/reference/dsl/fluid.md` (`DW0800`) | ours | the page text |

Two searches for a builder's rule of thumb — how many blocks of run a natural slope takes per block of rise — returned none: the terraforming guides found speak of irregular edges and of working from large shapes to small, and give no number. **No source read states a run per block of rise.**

## 2. What the pinned game does (S1)

**It has five answers, and most structures use none.** `TerrainAdjustment` has five values, serialised `none`, `bury`, `beard_thin`, `beard_box` and `encapsulate`. Of the 34 structure definitions the jar ships, 23 carry no `terrain_adaptation` key, 7 say `beard_thin` (the five villages, the pillager outpost, the nether fossil), 1 `beard_box` (the ancient city), 2 `bury` (the stronghold, the trail ruins) and 1 `encapsulate` (the trial chambers). S2 gives the vendor's words for the two beards: `beard_thin` — "Ambient block density will be added below the structure and block density will be reduced just above the ground"; `beard_box` — "added below the structure, and block density will be reduced within the entire box the structure occupies"; and for the name: "the act of adding mass below structures that happen to get spawned floating in air".

**The ground moves, the structure does not.** `Beardifier` is a density function the terrain generator adds to its own: it changes the terrain around a piece and never a block of the piece. `forStructuresInChunk` collects the pieces of every structure whose adjustment is not `NONE`; a jigsaw pool piece enters as a `Rigid(box, adjustment, groundLevelDelta)` only when its pool element's projection is `RIGID`. (`StructureTemplatePool.Projection` has the two values `RIGID` and `TERRAIN_MATCHING`; a terrain-matching piece — a village street — is the other answer, the piece following the ground, and gets no beard.)

**It works both ways from one plane.** `Beardifier.compute`, per rigid piece: `dx = max(0, box.minX − x, x − box.maxX)`, `dz` likewise, the ground plane `g = box.minY + groundLevelDelta`, and for `beard_thin` `dy = y − g`. `getBeardContribution(dx, dy, dz, y − g)` returns `−(d) × fastInvSqrt(lengthSquared(dx, d, dz) / 2) / 2 × kernel[dx, dy, dz]` with `d = (y − g) + 0.5`, multiplied by `0.8`. The sign is `−d`: positive under the plane, where it adds ground, and negative over it, where it removes ground. One function fills under a piece that floats and cuts beside a piece set into a slope.

**The distance is Euclidean from the piece's box, and it has a fixed reach.** `dx` and `dz` are distances outside the box on each axis, zero beside a face, both non-zero only past a corner, and the kernel is `exp(−lengthSquared(dx, dy + 0.5, dz) / 16)`: round a corner the contribution falls with the straight-line distance, so the adapted ground rounds the corner. The kernel is 24 cells on a side (`BEARD_KERNEL_SIZE` 24, radius 12) and `getBeardContribution` returns `0` outside it: the terrain adapts within 12 cells of the box and no further, whatever the height difference. A junction between pieces contributes the same function at `0.4`.

**Not established:** because the contribution is added to a noise density, the slope the game ends with is not a number this reading can state, and no world was generated to measure it. That a tall drop under a village house ends as a steep cone is the reader's recollection, unsupported here.

## 3. What map tools do (S3)

Terrain is edited as a **heightmap**: S3, of `//smooth` — "this command works with a heightmap, and while it excels at smoothing out surface terrain, it it not suitable for smoothing caves, walls, or objects". A column has one surface height, and smoothing moves that height.

Moved ground is **re-skinned in layers from its new surface**: S3, of `//naturalize` — it "will naturalize terrain by creating a layer of grass, followed by layers of dirt and then stone". The surface block is laid at the top of the column as it now stands, never carried from where the surface used to be.

## 4. What generated settlements are judged on (S4, S5)

S4 names **adaptability** as one of four judged criteria: "Does the settlement take advantage of terrain features or compensate for problems with the terrain?"

S5's judges, on what goes wrong at the edge of a built thing:

- the cut that shows: "maps look like they have been carved all around buildings, forcing them into the map"; "the biggest challenge seems to be how the buildings blend with the land";
- the step that cannot be walked: "making paths traversable across height differences of more than one block"; "if a step is 10 meters high and impassable";
- the platform on nothing: "a big plaza made out of cobblestone can balance on a 50 meter tall stone pillar"; "constructing reasonable foundations for structures on cliffs or hillsides";
- the flattened site: "generators that just flatten out a huge grid of terrain and then build a town on top";
- the unsafe edit: "instances of flooding where a generator makes unsafe changes near to water"; "collapse where changes are made near to (or consisting of) unsupported sand blocks".

## 5. What this repository already holds (S6)

- A body steps up `0.6` of a block without jumping (`step_height`, `body-movement.md` §4). A rise of one full block is jumped; a rise of two is not passed on foot.
- A fluid source with an open cell beside or below it runs, on the server's own clock (`dsl/fluid.md`, `DW0800`, measured on the pinned server). Removing the ground beside standing water opens such a cell.

## 6. What the sources do not say

- **A run per block of rise.** None found (§1). Spec-0138's two columns per block is authored.
- **A grade that scales with the drop.** The game's reach is fixed at 12 cells (§2); no source read describes ground that keeps a set slope over any height.
- **When to leave the drop.** The game leaves 23 of its 34 structures unadapted (§2), and S5's judges fault the flattened site as they fault the pillar; no source gives a rule for which edges should stay sheer.
- **A stepped grade in full blocks.** The game's adaptation is a change to a density, resolved to blocks by the generator; S3's is a filter over heights. Neither states the stepped profile as a rule.
