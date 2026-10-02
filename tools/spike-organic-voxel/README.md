# Spike: organic voxel structures (a whale skeleton)

Spike code, not an engine surface. The research record is
[`docs/reference/organic-structures.md`](../../docs/reference/organic-structures.md).

## Question

Can a curved, non-architectural set-piece (a hill-sized sky-whale skeleton
with a walkable spine, a rib cage to climb through and a skull holding a
temple) reach the reference image's quality? The bar is
`ref-view1-establishing.jpg` of the whale-fall study (content repo, branch
`demo/whale-fall-structure`, `demos/whale-fall/design/reference/`).

It is tried by two routes:
- **block-shape-aware voxelisation** of a CC0 scan;
- **a stylised implicit skeleton**, fitted by the same back end.

## Instrument

| Part | What |
|---|---|
| Engine | `delvec 1.7.1` release build of `origin/main` at `1ef29efa`, from a scratch worktree; cargo exit 0 asserted |
| Mesh | CC0 *Cetotherium riabinini* skeleton reconstruction, figshare doi:10.6084/m9.figshare.29644028, file 58189099. `fetch-mesh.sh` checks the zip MD5 (figshare's own `computed_md5`) and the STL's SHA-256 (`57657cfb…dcc71`): 19,938,276 triangles, 2.78 m long |
| Python | `uv run` with numpy 2.3.3 and scipy 1.16.2, pinned in `run.sh` |
| Front end A | `voxelize.py`: surface samples into a quarter-block grid, then exterior labelling to solidify |
| Front end B | `sdf_whale.py`: about 580 implicit primitives at a 320-block body, scaled by `--length` |
| Back end | `voxelize.blocks_from_solid`, in four steps: (1) octant fit to air / full / slab / straight stair, with a bias of 0.6 for stairs and 0.4 for slabs; (2) thin-plate refit; (3) stair `shape` by a port of `delvec::schem::stairs::derive_shape`; (4) tone shading into the `weathered` palette |
| Admission | `delvec schem convert` (tiling at 48), then `make_manifest.py`, then `delvec prefab audit` |
| Draft frames | `delvec snapshot` on the stub campaign in `campaign/`, with cameras from `views.py` |
| Engine readings | `gaps.sh` runs the light probe, `planes --write` and `delvec build` on a copy, with an entry anchor on the spine |
| Path-traced frames | `render-chunky.sh`: `delvec prefab gallery`, then `chunky-world.sh` (the pinned base server image stamps the gallery world), then `views.py --chunky`, then `validation/chunky.sh` at 48 spp. Sun at 38° and exposure 0.75 are authored |

Run it (scratch outside the repo, never `/tmp`):

```sh
tools/spike-organic-voxel/run.sh <delvec> <scratch> sdf224 sdf320 sdf448 224 320 448
EULA=TRUE tools/spike-organic-voxel/render-chunky.sh <delvec> <scratch> whale-sdf-320 48
uv run --with pillow python tools/spike-organic-voxel/sheet.py <scratch>/renders <ref-dir> --suffix -chunky whale-sdf-320
```

## Palette (measured)

These are `block-appearance.py` readings.
- **Sandstone shapes are too yellow.** The first warm ramp, smooth sandstone + sandstone + bone, read `chroma_mass 0.0543` with `chromatic_area 1.00`, against bone colour sampled off the reference (median `#b2a795`).
- **The adopted `weathered` ramp has four tones, each a full-block mix plus one stair/slab family:**
  - tone 0 (`bone_block 3, dead_horn_coral_block 3, calcite 2, diorite 2` | diorite);
  - tone 1 (`dead_horn_coral 3, dead_bubble_coral 3, andesite 2, diorite 2` | andesite; `chroma_mass 0.0079`, `chromatic_area 0.00`);
  - tone 2 (andesite, dead brain coral, tuff | andesite);
  - tone 3 (tuff, mossy cobblestone, andesite | mossy cobblestone).
- **Why dead coral:** the dead coral blocks were screened as warm low-chroma mid-greys (`L 0.57–0.60`, `C_mean 0.010–0.012`).
- **No bone-coloured stair exists.** `bone_block` and `calcite` have no stair or slab, so shapes always come from a neighbouring family.

## Readings

Engine `1ef29efa`, seed 1, sub-voxel 4, `weathered` palette.
- **Hashes** are each `.schem`'s SHA-256; a split piece gets the hash of its parts' hashes.
- **Islands** are disconnected bone groups larger than 4 blocks.
- **Audit:** `delvec prefab audit` on the tiled manifest. "stairs x/y" means stairs examined against stairs written. DW0801 holds on every stair, because the shape is derived, never chosen.

| piece | size (x×y×z) | filled | full / slab / stair | thin-plate refills | islands | audit | schem sha256 |
|---|---|---|---|---|---|---|---|
| `whale-sdf-224` | 105×84×224 | 51896 | 37340 / 6382 / 8174 (28% sub-block) | 6216 | 19 | pass, stairs 8174/8174 | `9fedbea54d4c` |
| `whale-sdf-320` | 150×120×320 | 144469 | 114176 / 12743 / 17550 (21% sub-block) | 12627 | 14 | pass, stairs 17550/17550 | `5100ef82492d` |
| `whale-sdf-448` | 210×168×448 | 386902 | 325410 / 25350 / 36142 (16% sub-block) | 25326 | 7 | pass, stairs 36142/36142 | `64675b1c5323` |
| `whale-224` (scan) | 97×48×228 | 20091 | 9607 / 4259 / 6225 (52% sub-block) | 5078 | 3 | pass, stairs 6225/6225 | `78a7664489b9` |
| `whale-320` (scan) | 137×66×324 | 49416 | 28109 / 8512 / 12795 (43% sub-block) | 10098 | 4 | pass, stairs 12795/12795 | `b426e46151f5` |
| `whale-448` (scan) | 190×91×452 | 119661 | 76960 / 16767 / 25934 (36% sub-block) | 20766 | 11 | pass, stairs 25934/25934 | `4010e99e35e6` |

**Determinism (built twice, compared):**
- `whale-224` and `whale-320` gave the same `.schem` hash on three runs each, including across the refactor that split the back end out.
- `whale-sdf-224` rebuilt to `9fedbea5…1d3d` both times.
- Chunky frames are excluded, as all render pixels are (`tools.md`).

**Scale against the reference** (measured, `docs/reference/organic-structures.md` §5): about 280–350 blocks long, ribs about 2.5 thick at a 5.5-block pitch.
- **Stylised piece:** ribs about 2.6 blocks at a 5.6-block pitch at 320 (authored to match). The pitch scales with length, so 224 squeezes the gaps toward 2 blocks.
- **Scan:** in a side projection at 448 the ribs read 5–7 blocks wide with 1–2 block gaps. That is the specimen, not the scale: the proportions are the same at every length.
- **Islands:** the stylised pieces keep 7–19 of them. Flipper digit tips, pelvis and fluke edges come apart at the coarser scales, and each is a fall a body cannot use.

## Renders

Everything is in `~/Documents/projects/delvewright-worktrees/wt-organic-research-scratch/renders/` (not committed). Best first:

1. **`compare-best.png`** is the one to look at. It has the far, ribcage and spine views in four columns: reference | the grammar study (snapshot) | scan at 448 (Chunky) | stylised at 320 (Chunky).
2. **Stylised at 320:** `whale-sdf-320-chunky-vs-reference.png` and `whale-sdf-320-chunky-sheet.png`.
3. **Scan at 448:** `whale-448-chunky-vs-reference.png` and `whale-448-chunky-sheet.png`.
4. **The other scales:** `whale-sdf-{224,448}-chunky-sheet.png` and `whale-{224,320}-chunky-sheet.png`.
5. **Draft frames** from `delvec snapshot`: `<piece>-sheet.png` and `<piece>-{far,spine,ribcage,skull}.png`.

Single path-traced frames are named `<piece>-<view>-chunky.png`.

## Obstacles met

1. **Light probe.**
   - **Scan pieces:** `delvec prefab lighting <piece>.json` gives `DW0752 … no ground-level entrance on any of the four vertical faces`.
   - **Stylised pieces:** they measure from whichever bone tip touches a side face (`whale-sdf-320`: `DW0751`, 174 of 1302 reachable floor cells dark at night, out of 14566 standable).
   - **Consequence:** `render piece` / `viewer` refuse with `DW0894`. The GPU piece renderer is unreachable, which is why Chunky was used.
2. **`walk_y`.** `delvec prefab planes` measures `walk_y` off bone tips: 3 of 9633 standable cells on `whale-320`, and 3 of 14566 on `whale-sdf-320` (`gaps.sh`).
3. **The build.**
   - **First refusals:** `delvec build` on the stub campaign needs an `entry` anchor (`DW0345`) and `walk_y` (`DW0886`).
   - **Then:** given both (`gaps.sh` adds them on a copy), the scan at 320 is refused with `DW0322`: 1124 reachable cells border a void drop. The stylised piece at 320 is refused with `DW0886`: blocks on its box faces, no `shown_faces`, on a void horizon. An earlier scan iteration was refused with `DW0921`: 157 soft-lock places.
   - **Consequence:** `validation/world-save.sh`, which needs a build tree, cannot make the world, so `chunky-world.sh` stamps a gallery world instead.
4. **10M-cell limit.** `delvec schem convert` refuses more than 10,000,000 cells (`DW0710`), so `voxelize.py` splits on 48-block z boundaries.
5. **No cloud-sea surround.** The frames show Chunky's sky over the void.

## Verdict

**Does any scale reach the reference? No.** It is much closer than the grammar study:

| view | stylised (implicit) | scan (CC0 mesh) |
|---|---|---|
| far | **Reads as a whale skeleton at a glance**, at 320 and 448. It has an open jaw, a ribcage with real gaps, a spine with separate neural spines, flippers with digits, and a fluke. The curves are smooth, with no corrugation. **The skull is wrong:** a bulbous braincase with a dark window reads as a dog or bird skull, where the reference has a long wedge. The bone surface is speckled with dark stair notches. | Reads as a real baleen skeleton, faithful and elegant from far. **It is not the reference animal:** the jaw is closed, the rostrum and mandibles are a long three-pronged beak, and the cage is a dense barrel. |
| ribcage | Reads as ribs with sky between them at 320. At 448 the computed camera lands against the spine, so that frame is a camera failure, not the piece's. | **At 448 it reads as a rib vault** when the camera looks forward and up the cage. At 224 and 320 the ribs nearly touch and it reads as a cave ceiling. |
| spine | **A walkable plateau of flat-topped centra** with spines at the midline. It is play-shaped, but up close it reads as a rocky ledge, not as the reference's drums and posts. | A narrow rocky ridge: there is nothing to walk on. |
| skull | It has a hall 18 blocks high entered through the foramen magnum, but the shape is wrong (above), and **no temple is built**. | A small braincase: no temple fits. |

**Which scale:** 320 for the stylised piece, the reference's measured 280–350. 448 for the scan, whose cage only opens up that large. **224 is too small for either.**

**The method:**
- **Sub-block octant fitting** (authored) plus vanilla-derived stair shapes fixes the study's stair-step corrugation. Every piece passes the admission audit.
- **The input decides the read.** A faithful scan gives a real animal; the reference is anatomy exaggerated for play. The stylised front end is where the reference's look can come from. It needs a skull redesign (a wedge with a side opening onto the hall) and a sub-block bias tuned against the notch speckle.

**What remains, beyond geometry:**
- **No cloud sea:** no surround exists; §6 of the reference doc.
- **No temple:** it would be a grammar piece placed in the hall.
- **No walk proof:** the instruments seed from grade. The light probe gives `DW0752` on the scan piece. On the stylised piece it measures only 1302 of 14566 standable cells, from an arbitrary flipper-tip grade.
- **No build** of either piece:
  - the scan at 320 is refused with `DW0322` (1124 reachable cells border a void drop);
  - the stylised piece at 320 is refused with `DW0886` (blocks on its box faces, no `shown_faces`, on a `void` horizon);
  - an earlier scan iteration was refused with `DW0921` (157 soft-lock places).

  All three refusals are correct for a floating skeleton. They are design work (catch surfaces, a return mechanic, declared faces), not engine defects.
- **Scale limit:** `delvec schem convert` refuses a schematic over 10,000,000 cells (`DW0710` "array size … greater than max sequence length"). The 448 stylised piece was split into z-slabs on 48-block boundaries to pass.
