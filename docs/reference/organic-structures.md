# Organic structures — curved, non-architectural forms

Reader: the agent planning engine work. This is the research record behind the
question "how does the engine build a hill-sized creature skeleton that reads
like its reference image", and the measured result of one spike
([`tools/spike-organic-voxel/ at d67aec1e34d6`]).
Every claim is **cited** (a link) or **authored** (said so). Nothing here is a
spec; the capability it sketches needs one before it is built.

## 1. Why the grammar route stops short

The grammar back end has no smooth curve, no diagonal and no noise by design
([`prefab-procedure.md`](prefab-procedure.md) §0, [`grammar.md`](grammar.md)).
A structure study built a sky-whale skeleton with it (content repo, branch
`demo/whale-fall-structure`, `demos/whale-fall/README.md`). Its obstacles are the
starting point here: curves came out as one-block stair-step corrugation, the
skull read as a ridged box, the piece was 128 blocks long, and the walk and light
instruments seed from grade, so a floating set-piece got no walk proof.

## 2. Prior art

Licences verified on the date of this record. "Fit" means: deterministic,
headless, licence-clean, and able to emit stair and slab states into a `.nbt`
that the engine's gates read.

| Tool / source | Licence (how verified) | What it does | Fit |
|---|---|---|---|
| [ObjToSchematic](https://github.com/LucasDower/ObjToSchematic) | BSD-3-Clause (GitHub API; `LICENSE` at `b611b52`) | Mesh to schematic. Ray voxelisers, including a normal-corrected one; block choice by visible-face colour; ordered or random dither | Algorithms are portable. It places **full blocks only**: `grep -i "stair\|slab"` over `src/` at `b611b52` matches nothing. Its `random` dither calls `Math.random`, so it is excluded under ADR-0006. |
| VoxelVision / BlockBlender (Blender add-on, [store page](https://superhivemarket.com/products/blockblender-2)) | Paid, closed source | Says "stairs and slabs automatically spawn if a full block cannot be placed inside the model" | Ineligible. It is evidence that shape-aware fitting is the quality bar. |
| [Bloxelizer](https://bloxelizer.com/editor/voxelizer) | No licence found | Browser voxeliser with a stairs/slabs option | Ineligible; its algorithm is not published. |
| Amulet Editor | `LICENSE`: "All rights reserved" | World editor | Ineligible. |
| [binvox](https://www.patrickmin.com/binvox/) | Binary only, no source | Mesh voxeliser | Ineligible. |
| [SDFGen](https://github.com/christopherbatty/SDFGen) | MIT (README) | Mesh to signed distance grid | Usable for a mesh-to-SDF step. The spike did not need it (§4). |
| WorldEdit / FastAsyncWorldEdit | GPL-3.0 (`LICENSE`) | In-game brushes; `//deform` is a deterministic expression warp; [`//smooth`](https://worldedit.enginehub.org/en/latest/usage/regions/regionops/) works on heightmaps only | Concepts only. These are plugins, so they cannot run on the player-facing server (ADR-0003). |
| VoxelSniper (TVPT) / FastAsyncVoxelSniper / goPaint | MIT / GPL-3.0 / GPL-3.0 | Blend-ball and spline brushes, the build teams' organics tools | Concepts only, for the same reason. Arceon exists only as a Patreon plugin with no public source, so its licence is unknown and it is excluded. |
| GDMC ([arXiv 1803.09853](https://arxiv.org/abs/1803.09853)), "Organic building generation" ([arXiv 1906.05094](https://arxiv.org/abs/1906.05094)), 3D-Craft / VoxelCNN | Papers; 3D-Craft is CC BY-NC (`LICENSE`) | Settlements and buildings; "organic" there means organic layout | None of them generates organic shapes; 3D-Craft is also forbidden by its licence. |

**Community technique (cited):**
- Build a wireframe "stick-man" of the pose first, then fill it ([Hypixel organics guide](https://hypixel.net/threads/guide-an-easy-beginner-method-for-building-organics-handmade.1664494/)).
- Avoid straight diagonals and floating corners (same guide).
- A curve with a radius under about 6 blocks reads as a pitched roof, not a curve ([Minecraft Wiki, curved roofs](https://minecraft.fandom.com/wiki/Tutorials/Curved_roofs)).
- A slab is one half-height box and a stair is two boxes ([Microsoft, voxel shapes](https://learn.microsoft.com/en-us/minecraft/creator/documents/voxelshapes)). Both are what sub-block smoothing works with.

**No published minimum scale for a creature was found.** §5 measures one
instead of inventing it.

**Meshes, checked against ADR-0013:**
- [Cetotherium riabinini skeleton reconstruction](https://doi.org/10.6084/m9.figshare.29644028): **CC0**, read from the figshare API's `license` field. Adopted.
- NHM blue whale scan: CC BY-NC-SA (read on the Sketchfab page). **Forbidden.**
- Pisa minke whale on Wikimedia Commons: CC BY-SA 4.0. That is Track-2 for prefabs under ADR-0013, so **not admissible**.
- Ingenium North Atlantic right whale: CC BY. Its only host is Sketchfab, whose downloads need an account, so it was not fetched. It is the CC BY candidate if a real large baleen whale is wanted.

## 3. The finding in one line

No licence-clean tool fits stair and slab shapes to sub-block occupancy, so the
spike authored that step:
- **Octant fitting** (authored): sample each block as 2×2×2 octants, then pick air, full, slab, or straight stair (any facing or half) by least occupancy error, with a cost bias against sub-block shapes.
- **Stair corners:** vanilla derives them. The fitter never chooses them, so `DW0801` holds by construction. The derivation is ported from `delvec::schem::stairs::derive_shape`.

This removes the grammar's stair-step corrugation on curves. Across a curve, a
quarter-block step replaces a one-block step.

## 4. The spike's two front ends, one back end

The back end (`voxelize.blocks_from_solid`) turns a solid sub-voxel grid into
blocks in seven steps:
1. **Octant fit** (§3).
2. **Thin-plate refit** (authored): only where the fit says air, refit from the solid thickened by one sub-voxel. Thin bone plates otherwise become holes.
3. **Stair corners** derived by vanilla's rule (§3).
4. **Shading tone** (authored): taken from the smoothed surface normal, the openness at crevice and body scale, and low-frequency seeded noise.
5. **Palette:** the tone picks a palette family.
6. **`bone_block` axis:** the axis of the longest local run.
7. **Output:** a Sponge `.schem`, which `delvec schem convert` tiles into structure parts.

The front ends:

- **Mesh** (`voxelize.py`) samples the CC0 scan's surface into the grid and solidifies it by labelling the exterior.
  - It is faithful to a real 2.78 m dwarf whale.
  - That anatomy is not the reference's. The ribs are broad and nearly touch: in a side projection at 448 blocks they are 5–7 blocks wide with 1–2 block gaps. The vertebral column is a narrow ridge, the jaw is closed and the braincase is small.
- **Implicit** (`sdf_whale.py`, authored) states a stylised skeleton as roughly 580 tapered capsules, ellipsoids and discs, with play-scale rules written as numbers. Examples:
  - discs 22 blocks across at the neck;
  - ribs about 2.6 blocks thick at a 5.6-block pitch;
  - an 18-block-high hall inside the skull, entered through the foramen magnum.

  The same back end fits it.

Readings and renders: the spike README. The verdict is in §7.

## 5. Scale (measured)

These are measured off `ref-view1-establishing.jpg` (1200×669):
- **Block pitch at the skull's depth: about 4 px.** The temple columns are 2 blocks and about 8 px wide; the stair-step spacing along the skull edge is about 4.4 px.
- **Projected body length: about 1120 px, so about 280 blocks.** The view is oblique, so the true length is at least that.
- **Skull: about 515 px projected, so about 130 blocks.** The skull opening holding the temple is about 37 blocks across.
- **Ribs: about 2.5 blocks thick at a 5.5-block pitch.** The neural spines are about 15 blocks tall.

So the reference whale is **about 280–350 blocks long**. The study's 128 blocks
is under half of that. Below about 220 blocks a 2.6-block rib cannot keep a
3-block gap, and the cage closes into a wall (authored, from the fitted pitch).

## 6. Where it would live (sketch, not built)

**A creator-facing verb, `delvec sculpt`** (ADR-0023: one binary). Authored sketch:
- **Input:** a mesh plus a provenance record, or an implicit program like `sdf_whale.py`'s, written as a schema-checked document rather than code.
- **Licence check:** the provenance record is refused unless it names an ADR-0013 licence. The licence and checksum go into the piece's `license` block.
- **Output:** structure parts and metadata directly, with no `.schem` detour.
- **Rust port of the back end:** surface sampling, exterior labelling, octant fit, `schem::stairs::derive_shape`, and the shading ramp measured from `block-appearance-1.21.11.json`. The RNG is seeded PCG, as ADR-0006 requires.
- **Gates it would pass:** the always-on gates `delvec grammar expand` runs (blocks-exist, states-complete, oriented-fills, stair-shape), and `delvec prefab audit`. The spike's pieces already pass the audit: `stairs_examined` equals every stair written, and `underspecified` is 0.
- **Route-table row:** it is the "smooth curve … in-house generator work" row of [`prefab-procedure.md`](prefab-procedure.md) §0.
- **Red line respected:** it voxelises meshes and implicit solids, never images.

**The floating-structure gap.** Every refusal below was met on the spike's pieces at engine `1ef29efa`:
- **Light probe:** `delvec prefab lighting` refuses with `DW0752` ("no ground-level entrance on any of the four vertical faces"). The probe seeds from `nav::ground_entry`. As a result `render piece`, `viewer` and `render batch` refuse the piece with `DW0894`.
- **`walk_y`:** `delvec prefab planes` measures it off 2 of 8770 standable cells, the flipper tips.
- **The build** needs an `entry` anchor (`DW0345`). Given one (`gaps.sh`), it refuses:
  - the scan piece with `DW0322` (1124 reachable cells border a void drop);
  - the stylised piece with `DW0886` (blocks on its box faces, no `shown_faces`, on a `void` horizon);
  - an earlier scan iteration with `DW0921` (157 soft-lock places a body can drop into and not leave).
- **Light probe, stylised piece:** it does measure, but from whichever bone tip touches a side face: 1302 of 14566 standable cells.
- **Size:** `delvec schem convert` refuses a schematic over 10,000,000 cells (`DW0710`). A hill-sized piece needs a writer that emits parts directly.
- **What the instruments need (authored):** seed from the piece's declared entry anchor or socket when no side face carries grade. Then reachability, light, `walk_y` and enclosure measure the walk a body actually has.
- **`DW0921`, `DW0322` and `DW0886` are correct, not defects.** A floating skeleton is a soft-lock and void-edge field. Two answers:
  - **Geometry:** catch surfaces and climbable routes, or move the bones.
  - **The return mechanic** for falls into the void (see `tools/spike-death-teleport/ at 84f364997d24`).
  - The owner's "danger is visible" ruling decides which falls are allowed.
- **Cloud sea:** a cloud sea is a surround, not a prefab, and none exists.
  - The pinned Chunky core's `Sky` carries `cloudsEnabled`, `cloudSize` and `cloudOffset` (strings in `Sky.class`), so Minecraft-style block clouds can be rendered.
  - A `horizon: clouds` kind would have to be analytically known so that the proofs read it as void (§0's surround row).
  - Nothing in the engine emits these today.

## 7. Verdict (measured on the spike's path-traced frames)

**No scale reaches the reference image. Both routes are far past the grammar study.** Read against the reference by eye, on frames from the pinned Chunky:

- **The fitter works.** Curves come out smooth at 224, 320 and 448 blocks. Every piece passes `delvec prefab audit`, with stairs examined equal to stairs written. The pieces are deterministic: rebuilt and compared by hash.
- **A faithful scan gives the wrong animal for this reference.**
  - The CC0 *Cetotherium* reads as a real baleen skeleton from far.
  - Its cage only opens into a rib vault at 448, and its spine is a ridge with nothing to walk on.
  - Its closed jaw and small braincase cannot hold the scene.
- **A stylised implicit skeleton is the route to the reference's look.**
  - At 320, the reference's measured length, the far view reads as the whale at a glance: open jaw, a cage with gaps, separate neural spines, a fluke.
  - The spine is a walkable plateau of flat-topped centra.
  - What is missing is design work, not capability: the skull is a bulb, not a wedge; dark stair notches speckle the bone; a temple must be built in the hall.
- **Remaining engine gaps:**
  - instruments that seed from a declared entry instead of grade (§6);
  - a `clouds` surround;
  - a part writer past the 10M-cell `.schem` limit;
  - the authored craft decisions behind `DW0322`, `DW0886` and `DW0921`: catch surfaces, a return mechanic for void falls, declared faces.

**Recommended next step (authored):** a spec for `delvec sculpt` taking implicit-skeleton documents first and meshes second, plus a spec for entry-seeded piece instruments. The skull redesign and the temple are content work on top.
