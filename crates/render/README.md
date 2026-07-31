# delve-render

The deterministic **render layer** (spec-0007 rendering infra + spec-0003 visual
tier, M3). Productionizes the `spike-render-fidelity` spike. Renders are
**validation artifacts** — they prove a delve *looks right* before a human joins
(invisible interact markers, unlit rooms, backwards NPCs, literal-JSON name tags)
— and are **never shipped** (so they are excluded from ADR-0006 byte-identity;
see "Stability" below).

Two renderers, one 1.21.11 **fidelity gate**:

- **Nucleation** (Rust, MIT, headless wgpu) — per-prefab multi-angle sets + the
  fidelity-gate fixture. Fast (<1s / frame). Pinned by git rev in
  `versions.toml [render]`.
- **Chunky** (GPLv3, out-of-process, path tracer) — whole-scene beauty shots.
  **Not bundled**; `delve-render scene` emits Chunky scene JSON, running Chunky
  stays manual / CI-future (see "Chunky scenes").

## Commands

```
delve-render [--json] [--textures <jar>] [--size <px>] <command>

piece <prefab.nbt> -o <dir>     deterministic multi-angle set for one prefab
batch <dir> -o <dir>            piece set for every .nbt in a library dir
fidelity-gate [-o <dir>]        render the newest-block fixture; FAIL on placeholder
scene <build-dir> -o <dir>      Chunky scene JSON per shot from render-plan.json
```

**Exit codes**: `0` ok · `2` input/usage · `3` output · `4` **fidelity-gate
failure** (missing-texture placeholder detected) · `5` renderer/GPU/textures
error · `≥10` internal. Diagnostics (`DW072x`) go to stderr, one JSON object per
line under `--json`.

**Textures** (the 1.21.11 client jar — never committed, EULA) resolve from
`--textures <path>`, then `$DELVEWRIGHT_CLIENT_JAR`, then
`~/.chunky/resources/minecraft.jar`. The `scene` command needs no textures.

| Code | Meaning |
| --- | --- |
| `DW0720` | missing-texture (magenta) placeholder detected — the fidelity gate's failure |
| `DW0721` | input error (unreadable/unparseable `.nbt` / metadata / render-plan) |
| `DW0722` | output error (cannot write) |
| `DW0723` | renderer/GPU error or textures not found |

(schem owns `DW0700..DW0702` + `DW0710`; render takes the `DW072x` block.)

## `piece` — per-prefab shot set

Nucleation is an **orbit / turntable** renderer: it fits the camera to the model
bounds and optionally aims at a `target` — it does **not** place a free camera
inside a room. So the deterministic per-piece set is:

- **4 exterior corner-isometric** (`ext-ne/se/sw/nw`, yaw 45/135/225/315, pitch
  30) of the full schematic — Nucleation's strength.
- **1 top-down** floor plan (`top`) on a **ceiling-stripped cutaway** so the floor
  is visible instead of the roof.
- **interior doorway** (`door-<i>`) — one per socket in the prefab metadata,
  ceiling-stripped, aimed through the opening.
- **anchor** (`anchor-<name>`) — one per metadata anchor (point → position, gate →
  region centre), ceiling-stripped.

Metadata is read from `<basename>.json` beside the `.nbt` (sockets from
`connectors`, anchors from `anchors`, lighting from `lighting`); it **degrades
gracefully** — a missing/partial file still yields the exterior + top-down set.

True free-camera **in-room** shots are the **Chunky path** (`scene`), which places
cameras anywhere in the built world. The per-piece cutaways are the fast
per-prefab approximation for authoring review of floor plan, anchor placement, and
lighting. Sample output: `docs/samples/` (keep-gate-room, 256²).

```sh
delve-render piece campaigns/prefabs/keep-gate-room.nbt -o /tmp/gate --size 512
delve-render batch campaigns/prefabs -o /tmp/library
```

## `fidelity-gate` — the newest-block gate

Renders an in-code fixture packed with the newest 1.21.11 blocks (pale oak,
crafter, copper family, tuff set, trial-chamber `trial_spawner`/`vault`, …) and
**fails (exit 4)** if any block meshes as the magenta missing-texture placeholder
(a color-key scan, `detect::scan`).

`minecraft:heavy_core` is deliberately **excluded** from the gate fixture: its bare
`"texture":"all"` model is unresolved by Nucleation and always renders as a
placeholder (a known upstream gap). The detector's ability to catch a real
placeholder is proven separately — a unit test against a committed `heavy_core`
crop (`tests/fixtures/heavy_core_placeholder.png`) and an end-to-end
`#[ignore]` GPU test that renders heavy_core and asserts the gate trips.

```sh
delve-render fidelity-gate -o /tmp/fg          # exit 0 = pass, 4 = placeholder found
```

## `scene` — Chunky scenes from `render-plan.json`

The compiler emits `render-plan.json` in every build output — a deterministic
shot list (spawn, per-NPC, interact, gate both-sides, piece seam, one interior per
room), each shot with a camera (`pos` + `yaw`/`pitch` degrees) and a
machine-generated `expect` checklist derived from the DSL. `delve-render scene`
converts each shot into a **Chunky scene description JSON** (one file per shot,
`chunkList` covering the layout AABB).

Chunky itself is **not bundled** (GPLv3, out-of-process). Pinned snapshot core
(1.21.x needs a snapshot; stable stops at 1.20.4), from
[chunkyupdate.lemaik.de](https://chunkyupdate.lemaik.de):
`chunky-core-2.5.0-SNAPSHOT.474.g156e2bb` (`versions.toml [render]`). To render a
scene (manual / CI-future):

```sh
# 0. get the snapshot core once, download 1.21.11 assets
java -cp 'chunky-lib/*' se.llbit.chunky.main.Chunky -download-mc 1.21.11
# 1. build the delve + boot once so the datapack places structures, copy world out
delvec build <campaign> -o out
EULA=TRUE docker compose -f validation/compose.yaml --profile play up --build server
#    docker cp <container>:/data/world ./world
# 2. emit scenes, point their world.path at ./world, render
delve-render scene out -o scenes --world ./world
java -cp 'chunky-lib/*' se.llbit.chunky.main.Chunky -scene-dir scenes -render <name> -f -target 500
```

**Camera convention** (shared by `render-plan.json` and scene emission): `yaw`/
`pitch` in **degrees**, `yaw = atan2(-dz, dx)` (0→+X east, 90→−Z north),
`pitch = atan2(-dy, horiz)` (+ looks down). This matches the axes the spike
verified for Chunky (`yaw ≈ π/2` faces −Z, +pitch = down); scene emission converts
degrees → radians directly.

## Stability (double-render)

**Measured 2026-07-30 (macOS/Metal): a double render of the same prefab is
byte-identical** — all nine keep-gate-room shots produced identical PNG bytes,
zero per-pixel delta. On a fixed machine + driver + wgpu version, renders *are*
byte-stable.

This is **not guaranteed across GPUs / drivers / platforms** (float
rasterization), so the render layer is **excluded from ADR-0006 byte-identity** —
renders are validation artifacts, not shipped output. The committed stability test
(`tests/gpu.rs::piece_double_render_is_stable`) therefore asserts **pixel-equality
within a tolerance** (the portable guarantee); it happens to hold exactly on this
machine.

## Tests

Pure logic (adapter parse, shot planner, camera/scene math, missing-texture
detector incl. a real `heavy_core` crop, Chunky-scene golden) runs everywhere:

```sh
cargo test -p delvewright-render
```

The GPU path (`tests/gpu.rs`) is `#[ignore]` by default — it needs a GPU adapter
and the client jar:

```sh
DELVEWRIGHT_CLIENT_JAR=~/.chunky/resources/minecraft.jar \
  cargo test -p delvewright-render --test gpu -- --ignored --nocapture
```

## The `.nbt` → Nucleation adapter

Nucleation 0.9 has **no importer** for the binary gzip vanilla structure `.nbt`
our generator/compiler emit. The adapter (`nbt.rs`, from the spike) gunzips it
(fastnbt), reads the vanilla `size`/`palette`/`blocks` schema, and rebuilds a
`UniversalSchematic` via `set_block(BlockState::from_block_string(...))`. Textures
(the client jar) determine fidelity.
