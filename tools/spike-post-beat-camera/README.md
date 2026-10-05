# spike-post-beat-camera — can the renderer be handed a world the compiler writes?

The rig behind spec-0089 §2. A Chunky scene names a world save, and the only
producer of one is a server boot (`validation/world-save.sh`), which is why no
picture of a delve can show the world after a story beat. The spec proposes that
the compiler write the world itself from its per-configuration block map. Before
that was written down, two things were measured here on a real save of the
gallery: what a server-written save holds that a block map does not, and whether
the pinned Chunky core renders a world holding block states and biomes and
nothing else as it renders the server's.

```
python3 tools/spike-post-beat-camera/anvil_rig.py census  <world> [--render-plan <build>/render-plan.json]
python3 tools/spike-post-beat-camera/anvil_rig.py rewrite <world> <out> [--minimal-level-dat] [--perturb FROM=TO]
python3 tools/spike-post-beat-camera/anvil_rig.py compare <a.png> <b.png>
```

Standard library only. The Anvil reader and writer follow the two minecraft.wiki
pages (Region file format; Chunk format) and are held to each other by the
rewrite's own read-back: every cell written is read back and compared.

## Instruments

- Engine: `delvec` built from this tree (`2316b1c6`) with `cargo build -p delvec`,
  exit 0; the gallery built with `delvec build gallery --prefabs <gallery-generator output>`.
- World: `EULA=TRUE validation/world-save.sh <gallery build> --project dw-spec0089`,
  exit 0, on the pinned 1.21.11 delve image.
- Renderer: `validation/chunky.sh` on the pinned core
  `chunky-core-2.5.0-SNAPSHOT.474.g156e2bb` (`tools/lib/chunky_core.py verify`,
  exit 0), scenes emitted by `delvec cameras --draft` (400×225, 128 samples),
  `-render … -f`.

## Observations

### What the server's save holds at load (`census`)

Gallery, world box `[0,64,0]..[262,73,30]` (`render-plan.json` `layout_aabb`):

| Measure | Value |
|---|---|
| chunks stored | 1453, of which 151 `minecraft:full`; the rest are proto-chunks (`structure_starts` 1032, `biomes` 106, `carvers` 90, `initialize_light` 74) holding no block |
| cells read / non-air | 142,835,712 / 5939, all 5939 inside the layout box |
| distinct block states | 27 |
| fluid states with a non-zero `level` | 0 |
| gravity blocks | `minecraft:gravel` 3 |
| block entities | `minecraft:chest` 2, `minecraft:trapped_chest` 1 |
| entities (`entities/*.mca`) | `minecraft:interaction` 16, `minecraft:item_display` 4, `minecraft:villager` 2, `minecraft:mannequin` 1, `minecraft:item` 1 |
| light arrays | `SkyLight` on 84 sections, `BlockLight` on 6, of 34,872 |
| per-chunk tags | `DataVersion` (4671), `xPos`, `zPos`, `yPos`, `Status`, `sections`, `block_entities`, `block_ticks`, `fluid_ticks`, `Heightmaps`, `InhabitedTime`, `LastUpdate`, `PostProcessing`, `structures`, `entities` (1302), `isLightOn` (151) |
| region timestamps non-zero | 1453 of 1453 (wall clock, as `world-save.sh` says) |
| biome palette entries | `gallery:void` 10,104, `minecraft:plains` 24,768 |

### A minimal world is the same cells (`rewrite`)

Per chunk `DataVersion`, `xPos`, `zPos`, `yPos`, `Status: minecraft:full`; per
section `Y`, `block_states`, `biomes`; zlib; every timestamp zero; only chunks
holding a block. The indices are re-packed from the palette the reader
produced, so the writer proves the packing rule rather than copying the
server's longs.

| Measure | Value |
|---|---|
| chunks written | 6 (the chunks holding a block) |
| cells written and read back equal | 589,824 of 589,824; 0 mismatched |
| bytes | 32,768 (one region file) |
| two rewrites of one save | byte-identical: `r.0.0.mca` sha-256 `77cb03d4baefa9991add258d5a6d7d1e9ceeb62a9f6be518f84f927eeab5adbe` both times |
| minimal `level.dat` | 94 bytes, gzip, `Data{DataVersion, LevelName, SpawnX, SpawnY, SpawnZ, version}`; sha-256 `5965f77bef5bb891d81be4cc1beb68b95d71902bfc0619bd6a7fe56074114c51`; the region file written beside it is the same `77cb03d4…` |

### The pinned core renders the minimal world as the server's (`compare`)

Camera `hall-exterior` (noon, clear, exposure 1.0), one draft frame per world,
luminance rms against the frame of the server's save; a second render of the
server's save gives the noise floor of a 128-sample frame:

| Frame pair | identical pixels | luminance rms | frame sha-256 (second of the pair) |
|---|---|---|---|
| server save, rendered twice (noise floor) | 60,943 of 90,000 | 2.102 | `28038de22580173474edacddb02987830984c974e527feac94a672f50299fa3d` |
| server save vs minimal world with the server's `level.dat` | 60,885 | 2.095 | `1cc302708d5e5daf1678ec391822907a3cc35e9d883ca07e475c17c793138e8e` |
| server save vs minimal world with the 94-byte `level.dat`, `minecraft:stone_bricks` swapped for gold (a block the frame does not see; 2 palette entries) | 60,877 | 2.094 | `ec9cfe614c632c99627c202351634523458ad952140155cb10f50e10d7fd41ea` |
| server save vs minimal world with `minecraft:stone` swapped for gold (the block the frame sees; 6 palette entries) | 60,032 | **33.798** | `6d33a556df2e8776a1f56cc1d82633746a2aeb0a9fe0040d1d6acfa884148af8` |

The first frame of the server's save is `f65b46bf8dc2ddca951a35d08e12c76506eceb1bbd83576da413450337948eb2`.
The two minimal worlds sit inside the noise floor; the perturbation only a block
map could make moves the rms sixteen-fold, so the comparison can see one block
state move and the first two rows are not a comparison that sees nothing.

Camera `hall-from-its-corner` (a torch-lit interior at exposure 4.0) was tried
first: at 128 samples its noise floor is rms 34.203 and the minimal world reads
34.174 — inside the floor, but a floor that high discriminates nothing, which is
why the exterior carries the measurement.

A path-traced frame is not byte-reproducible; the hashes name the frames each
reading was taken from.

### What the pinned core reads (`unzip -l` and `strings` over the core jar)

- Chunk loading (`se/llbit/chunky/chunk/*`, `world/Chunk`) names `sections`,
  `block_states`, `biomes`, `palette`, `data`, `entities` and neither
  `BlockLight` nor `SkyLight`: the core lights a scene itself.
- World loading (`se/llbit/chunky/world/World` and siblings) names `level.dat`,
  `Data`, `SpawnX`, `SpawnY`, `SpawnZ`, `version`, `region`, `Dimension`.
- Entity classes present: `ArmorStand`, `PlayerEntity`, `PaintingEntity`,
  `HeadEntity`, `SkullEntity`, the sign, banner and hanging-sign classes,
  `Book`, `Lectern`, `Campfire`, `BeaconBeam`, `LilyPadEntity`,
  `CoralFanEntity`, `SporeBlossom`, `CalibratedSculkSensorAmethyst`,
  `FlameParticles`, and the mobs `Sheep`, `Pig`, `Chicken`, `Cow`, `Mooshroom`,
  `Squid`, `CopperGolem`. There is no class for `mannequin`, `villager`,
  `interaction`, `item_display`, `text_display` or `item` — none of the entity
  kinds the engine summons is drawn by the pinned core from any world.

## Reclaim

`world-save.sh`'s own trap ran `fresh-volumes.sh --project dw-spec0089`; after
the run `docker ps -a`, `docker volume ls` and `docker images` list nothing
carrying `spec0089`. Everything else the rig wrote is under the session
scratchpad.
