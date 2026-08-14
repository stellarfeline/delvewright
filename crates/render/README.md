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
- **Chunky** (GPL-3.0, out-of-process, path tracer) — the **official renderer**
  for whole-scene review frames, storybook scene illustrations and the per-release
  whole-map panorama (owner decision, 2026-08-06). **Not bundled**;
  `delve-render scene` / `panorama` emit Chunky scene JSON and `ChunkyLauncher.jar`
  renders them as a separate program (see "Chunky scenes" and
  [`docs/reference/tools.md` §4a](../../docs/reference/tools.md)).

## Commands

```
delve-render [--json] [--textures <jar>] [--size <px>] <command>

piece <prefab.nbt> -o <dir>     deterministic multi-angle set for one prefab
batch <dir> -o <dir>            piece set for every .nbt in a library dir
fidelity-gate [-o <dir>]        render the newest-block fixture; FAIL on placeholder
scene <build-dir> -o <dir>      Chunky scene JSON per shot from render-plan.json
panorama <build-dir> -o <dir>   the whole-map 45° oblique release panorama
                                [--bearing se|sw|ne|nw] [--spp 300]
index <build-dir> -o <file>     shot index (image ↔ expect pairs) for vision review
contact-sheet <dir> -o <png>    many candidate renders on ONE page, for curation
                                [--scores f] [--shot ext-se] [--columns N]
                                [--thumb 256] [--title T]
viewer <nbt|dir|manifest>... -o <html>
                                ONE self-contained interactive page: a camera the
                                reviewer drives [--title T]
palette <nbt|dir>... -o <json>  the derived per-blockstate colour/shape table
                                [--biome id]
```

**Exit codes**: `0` ok · `2` input/usage · `3` output · `4` **fidelity-gate
failure** (missing-texture placeholder detected) · `5` renderer/GPU/textures
error · `≥10` internal. Diagnostics (`DW072x`) go to stderr, one JSON object per
line under `--json`.

**Textures** (the 1.21.11 client jar — never committed, EULA) resolve from
`--textures <path>`, then `$DELVEWRIGHT_CLIENT_JAR`, then
`~/.chunky/resources/minecraft.jar`. The `scene` / `panorama` commands need no
textures (they emit JSON); Chunky itself reads the same jar when it renders them,
and it is never redistributed.

| Code | Meaning |
| --- | --- |
| `DW0720` | missing-texture (magenta) placeholder detected — the fidelity gate's failure |
| `DW0721` | input error (unreadable/unparseable `.nbt` / metadata / render-plan / scores) |
| `DW0722` | output error (cannot write) |
| `DW0723` | renderer/GPU error or textures not found |
| `DW0725` | contact-sheet ordering is not a total order over the candidates — the score RANKS, it never gates (exit 10) |
| `DW0726` | a contact sheet's score set bound to fewer candidates than the sheet holds; zero = error (exit 2), partial = warning. Also the `viewer`'s zero-anchor and zero-blockstate bindings (warning) |
| `DW0727` | an anchor's eye-level camera does not stand on the anchor's own cell, or could not be stood up at all (warning) |
| `DW0780` | a blockstate has no definition in the pinned asset source (`viewer` / `palette`) — warning, with its cell count |
| `DW0781` | a palette entry leaves shape-carrying properties unwritten, so the shape comes from the version's default state rather than from the file — warning, with the properties and the cell count |
| `DW0782` | the review page's resources do not hold together: the vendored renderer has lost its texture-id patch, or a block-entity texture id the emitter asks for is absent from an asset source declaring itself to be the pinned game (exit 10) |

(schem owns `DW0700..DW0702` + `DW0710`; render takes the `DW072x` block —
except `DW0724`, which the compiler's visual tier holds — plus `DW078x` for the
review page's resource findings. Take the next unused number from
`docs/reference/compiler.md`, not from the highest constant here.)

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
room, **and player-POV**), each shot with a camera (`pos` + `yaw`/`pitch` degrees,
optional `fov`) and a machine-generated `expect` checklist derived from the DSL.
`delve-render scene` converts each shot into a **Chunky scene description JSON**
(one file per shot, `chunkList` covering the layout AABB).

**Player-POV shots are the Chunky path, by design.** The `pov` shots
(spec-0003 #18) are first-person cameras at eye height (1.62) standing on each
critical-path waypoint, looking along the walk — a **free camera at a fixed point
inside the room**. Nucleation cannot render these: it is an orbit/turntable
renderer that fits the camera to the model bounds (it always backs out to frame the
whole model — there is no free-eye placement in `CameraConfig`). Chunky's scene
camera is a true free camera (`position` + `orientation` + `fov`), so POV shots
render through `scene` exactly as authored, carrying the first-person `fov` (~70°).
The compiler already proves every POV eye cell is clear over the assembled world
(`DW0724`), so a camera never looks out from inside a wall.

**Declared-dark shots get the night-vision REVIEW POLICY.** A shot stamped
`lighting: {"profile": "dark", "mitigation": "night-vision"}` by the compiler
frames an area that is meant to be dark with players kept under night vision —
an honest path trace of it is pure black (exposure boosts cannot reveal a sealed
cave; there is nothing to amplify). For those shots, and only those, the emitted
scene adds a review-only `materials` override: every non-emitting block of the
build's structure palettes at a low uniform emittance
(`scene::REVIEW_EMITTANCE`), approximating night vision's flat full-bright view;
real emitters are excluded so fixtures keep their genuine glow. The scene is
marked `delvewrightReviewPolicy: "night-vision-emulated — review only"` and the
shot index marks the entry (`review_policy`) — an **approximation for layout
review, never lighting ground truth** (the compiler's measured light model owns
that). See `src/scene.rs` module docs and the compiler reference.

**Limitations (recorded):**
- **Entity overlays** — NPCs and props are entities, not blocks, so they are absent
  from the `.nbt`/world geometry Chunky path-traces; POV/NPC shots render the *scene
  the actor stands in*, not the actor. Judging NPC placement/facing/name-tags stays
  a live-server concern. Recorded limitation, not a bug.
- **Running Chunky** stays out-of-process / CI-future (needs a booted-world save;
  see below); the shot set + index is the artifact, the vision verdict stays
  agent-driven (spec-0003).

**Ocean horizons get Chunky's water plane.** A campaign that declares
`horizon: ocean` ships a world save holding only its own chunks — the sea is the
level generator's, so a scene loading that save renders void past the shoreline.
The compiler therefore states the fact in `render-plan.json`
(`"horizon": {"kind": "ocean", "sea_level": 62}`) and emission raises Chunky's
ambient plane at the block-water surface, with `waterWorldHeightOffsetEnabled`
written explicitly (Chunky's default would drop it 0.125). `chunkList` stays the
layout's own chunks for the same reason: the plane is clipped out of loaded
chunks, so every extra chunk is more of the save's own block water beside it, and
the two read at visibly different tones. Trimming to the layout shrinks that seam
to the layout's chunk footprint. Void horizons emit no water keys at all and stay
byte-identical.

Verified live against the pinned core (2026-08-06): an ocean-horizon build
rendered from both the `se` and `nw` bearings — whole layout framed with even
margins, sea filling the frame to the horizon, camera-facing slopes lit.

Chunky itself is **not bundled** (GPL-3.0, out-of-process). Pinned snapshot core
(1.21.x needs a snapshot; stable stops at 1.20.4), self-installed by the launcher
from [chunkyupdate.lemaik.de](https://chunkyupdate.lemaik.de):
`chunky-core-2.5.0-SNAPSHOT.474.g156e2bb` (`versions.toml [render]`). To render a
scene:

```sh
# 0. once per machine: launcher + pinned core, and 1.21.11 assets
curl -LO https://chunkyupdate.lemaik.de/ChunkyLauncher.jar
java -jar ChunkyLauncher.jar --update snapshot
java -jar ChunkyLauncher.jar -download-mc 1.21.11
# 1. build the delve + boot once so the datapack places structures, copy world out
delvec build <campaign> -o out
EULA=TRUE docker compose -f validation/compose.yaml -f validation/owner-play.yaml \
  --profile play up --build server
#    docker cp <container>:/data/world ./world
# 2. emit scenes, point their world.path at ./world, render, extract the PNG
delve-render scene out -o scenes --world ./world
java -jar ChunkyLauncher.jar -scene-dir scenes -render <name> -f -target 500
java -jar ChunkyLauncher.jar -scene-dir scenes -snapshot <name> <name>.png
```

`<name>` is the scene file stem without `.json`. The core is CPU-only (the OpenCL
plugin is WIP and effectively unavailable on Apple Silicon): parallelise by
running one render **process per scene** with `-threads`, and tier `-target` —
~64 draft, ~300 final, 500 review set. The progress line `(N of 900)` counts
**scanlines**, not samples.

## `panorama` — the whole-map release illustration

`delve-render panorama <build-dir> -o <dir>` emits one scene framing the entire
delve: a 45° oblique camera on a corner bearing (`--bearing se|sw|ne|nw`, default
`se`), aimed at the centre of `layout_aabb` and pushed back until all eight
corners of the box are inside a 40° frame with a 12% margin — solved exactly, not
tuned. The sun sits at 50° altitude, 40° off the camera's own bearing, so the
slopes facing the viewer are lit but the relief still casts shadows.
`--spp` (default 300) sets the sample target; the file is
`<campaign>_panorama_<bearing>.json`, so several bearings coexist in one scene
directory.

Every content release ships one of these (owner decision, 2026-08-06). It is a
separate command rather than an extra shot in `scene` because `scene` keeps a
one-scene-per-plan-shot correspondence that `index` pairs with `expect` lines —
the panorama is a release artifact with no review pair, its own light and sample
budget, and a bearing chosen at render time.

## One stem per scene, and stale caches deleted for you

Chunky treats a scene's `name` field as its identity: loading `foo.json` whose
`name` is `bar` makes it save `bar.json` and key `bar.octree2` / `bar.dump` on
that name. Every file `delve-render` emits is therefore named after the scene's
own name (`<campaign>_<shot>`, `<campaign>_panorama_<bearing>`), so the scene
JSON, its caches and the rendered `.png` all share one stem — and a re-emission
lands on the same file Chunky would.

Chunky keeps a scene's loaded chunks in `<scene>.octree2` / `.emittergrid` and its
accumulated samples in `<scene>.dump` / `.dump.backup`. Re-rendering after a
change to `chunkList`, camera, sun or water settings **silently reuses them** —
the frame comes back without the edits and nothing warns you (2026-08-06). Both
`scene` and `panorama` therefore delete exactly those siblings for every scene
they write (`src/cache.rs`), and only those: another scene's in-progress render in
the same directory is untouched. Chunky's own `-reload-chunks` is not a
substitute — it re-reads the world but keeps averaging in the old `.dump`.

**Camera convention.** `render-plan.json` gives `yaw`/`pitch` in **degrees**,
`yaw = atan2(-dz, dx)` (0→+X east, 90→−Z north), `pitch = atan2(-dy, horiz)`
(+ looks down). Chunky's scene camera is **not** the same basis — verified against
the pinned snapshot core's bytecode (`Camera.updateTransform` =
`rotY(π/2+yaw)·rotX(π/2−pitch)·rotZ(roll)`, pinhole centre ray local `+Z`,
screen-`y` down). Scene emission therefore maps

```
yaw_chunky   = yaw_deg·π/180 + π      pitch_chunky = pitch_deg·π/180 − π/2   roll = 0
```

(a level 0° pitch → Chunky `−π/2`, upright). The earlier "straight deg→rad"
emission aimed every POV camera at the ground; the offsets were confirmed by
rendering nobodys-cave-island POV shots (2026-08-01). See `scene.rs` header for
the full derivation.

## `index` — (image ↔ expect) pairs for the vision reviewer

`delve-render index <build-dir> -o shot-index.json` reads `render-plan.json` and
writes one entry per shot: `id`, `kind`, `leg`/`objective` (POV shots), the `image`
filename a renderer produces (the scene's own stem + `.png`, matching `scene`), and the shot's
`expect` list. This is the visual tier's deliverable — a reviewing agent / vision
model is handed each shot's rendered image beside its expected content. **No
vision-model call is wired into CI**; the review stays agent-driven (spec-0003).
Order and bytes mirror `render-plan.json` (deterministic).

The ladder step `validation/render-shots.sh <build-dir>` runs `scene` +
`panorama` + `index` together, producing the Chunky scene set (review shots plus
`<campaign>_panorama_se`) and the index in one shot.

## `contact-sheet` — many candidates, one page, the owner's eye is the selector

`delve-render contact-sheet <dir> -o <sheet.png>` is the curation step of the
prefab authoring loop (spec-0027 §3): the grammar expander builds N seed-varied
candidates, `batch` images them, and this puts them on one page the owner picks
massing from. **It needs no GPU and no client jar** — it composites renders that
already exist — so it is the one command in this crate that runs everywhere,
including CI.

Two input layouts, chosen by what is there: one subdirectory of shots per
candidate (`batch` output; the representative angle is `--shot`, default
`ext-se`, else the first render by name), or a flat directory of `.png` renders.
An **explicitly given** `--shot` that some candidate lacks is an error — a page
whose cells face different directions is not a comparison, and silently
substituting another angle would make the comparison a lie.

`<stem>.json` is **always** written beside the PNG: cell → rank, id, image,
score, plus binding counts, layout and the rank source. It is how "she picked
number 7" resolves back to a prefab id, and it is the input `tools/refscore.py`
reads — which keeps this command the *single* discoverer of what a candidate is
and what it is called, so the scorer's ids cannot drift from the sheet's.

### The score RANKS; it never GATES

Owner ruling, spec-0028 §3. Cross-domain calibration between a painterly
reference image and a voxel render is unproven, so a similarity number may decide
**where** a candidate sits on the page and never **whether** it is on the page.

- The low scorer is present, **last**.
- An unscored candidate is present, **last**, and labelled unscored — a missing
  measurement is not a bad one.
- The ordering is a **seam**: `sheet::build_sheet` takes the order function and
  puts its result through `sheet::verify_total_order` before drawing a pixel.
  Anything that is not a permutation of the candidate set — a threshold that
  drops, a "best of" that duplicates, an off-by-one that loses the last cell —
  is refused with `DW0725`, exit 10.
- Every run states its **binding count**, on stderr and in the manifest. A score
  set that bound to zero candidates is `DW0726` at error tier (exit 2): it
  ordered nothing, and must not read as a successful ranking run.

Promoting the score to a threshold requires its own owner-approved amendment
backed by accumulated batch data. Until that exists, `DW0725` is the amendment's
absence spelled in code — do not satisfy it by relaxing the guard.

```sh
delve-render batch prefabs/zone2 -o .sheets/renders        # GPU + client jar
delve-render contact-sheet .sheets/renders -o .sheets/zone2.png
python3 ../../tools/refscore.py --sheet .sheets/zone2.json \
    --reference .refimg/zone2.png --backend open-clip -o .sheets/zone2-scores.json
delve-render contact-sheet .sheets/renders -o .sheets/zone2.png \
    --scores .sheets/zone2-scores.json
```

Cells are labelled with a built-in 5×7 bitmap font rather than a TrueType
rasterizer: hinting and antialiasing are the one part of this path that would
differ between a laptop and a runner, and a curation page that shifted between
runs would make "cell 7" mean two things. Two runs over the same inputs produce
the same page byte for byte (`tests/sheet.rs`). Sheets are working material like
every render — `.sheets/` is gitignored, nothing here ships.

## `viewer` — the camera the reviewer drives

A still render answers *is the set pretty*. Only a camera the reviewer drives
answers *what is it like to be in here*. `viewer` emits **one self-contained
`.html`** — no CDN, no external stylesheet, no fetch — so it opens from
`file://` and survives the strict CSP an Artifact is published under.

```sh
delve-render viewer campaigns/prefabs/island-mountain.nbt -o .sheets/mountain.html
delve-render viewer campaigns/prefabs -o .sheets/library.html   # all of them, one page
```

**The blocks are drawn by [deepslate](https://github.com/misode/deepslate)**
(MIT), vendored as `src/viewer/deepslate.bundle.js` and embedded in the page. It
walks the same chain the game does — `blockstates/<id>.json` → the matching
variant or `multipart` case → the model's `parent` chain → `elements` and
`textures` → the `.png`s — so a wall is a wall, a stair is a stair, a chest is a
chest and a torch has a flame. `viewer/resources.rs` extracts that slice of the
pinned client jar for the blockstates a page contains: typically a hundred
textures and two hundred models, all inline.

Two things the client jar cannot supply come from elsewhere. Per-block render
flags (`opaque` / `semi_transparent` / `self_culling`) are derived from model
geometry and texture alpha. Per-block **default state** — what the game reads an
unwritten property as — comes from the pinned block registry
(`delvewright_schem::blocks`), never from a guess: a bare
`minecraft:cobblestone_wall` is a wall POST, and "the first legal value" would
give `up=false` with `east=low`, which is a different block.

Rebuild the bundle with `tools/build-deepslate-bundle.sh`. It pins the versions,
applies one local patch — deepslate asks for `entity/banner/banner_base` and
`entity/shield/shield_base_nopattern`, paths no Minecraft version ships, while
1.21.11 has both at the jar's top level — and refuses if upstream has moved the
ids again. Every page build re-checks that the patch is present, because an
unpatched bundle renders every banner and shield as the missing-texture checker
and says nothing.

**Fidelity is reported, never assumed.** The page lists, with cell counts: a
blockstate the pinned version does not have (`DW0780`), a palette entry that
leaves properties unwritten so the shape comes from the default state rather than
from the file (`DW0781`), and — measured in the browser, by meshing each
blockstate alone — any block the renderer draws as nothing or draws with the
missing-texture checker. Beside them the page states what each check examined:
how many blockstates, how many textures and at what atlas size, how many
block-entity texture ids resolved. A page that examined nothing must not read
like a page that examined everything and found nothing.

**Controls**: `W`/`A`/`S`/`D` walk, the mouse looks, `Space`/`C` rise and sink,
`Shift` moves faster, arrow keys turn a little at a time, right- or middle-drag
slides the camera, and the wheel moves along the view axis. It works from the
first frame — there is no mode in which a movement key does nothing, and no
gesture whose meaning depends on which view is selected. `Orbit the whole piece`
is a labelled button for reading the massing from outside; while it is on,
dragging orbits, and any movement key returns the reviewer to their feet.

The mapping is `src/viewer/controls.js`, a DOM-free module that knows nothing
about how the page draws, so it survives a change of rendering core.
`tests/controls.test.mjs` presses keys and checks where the body ends up in each
of the four cardinal facings; CI runs it as a step of `rust (fmt, clippy, test)`.
Keys match on `KeyboardEvent.code`, the physical key — matching on `.key` makes
WASD dead under a Chinese IME, which reports `"Process"` for every letter.

**Presets**: `Ground level`, `Exterior ¾`, `Plan`, and a **point of view** per
declared anchor and jigsaw socket — eye at **1.62 blocks** above that cell's
floor. A socket faces *out* of the piece, so its view looks the other way. The
page opens on the first reserved way-in stem
(`spawn`/`entry`/`entrance`/`threshold`), else the first socket, skipping any
whose eye would land inside a block; a prefab declaring none starts the reviewer
standing on the ground off the south face. The cutaway slider hides everything
above a Y level and re-meshes — how a roofed interior gets read at all.

**Anchors come from `<basename>.json`**, the same sidecar `piece` reads, so
hand-built prefabs work today and a grammar snapshot's semantics sidecar loads
through the same reader. Zero anchors is a stated finding (`DW0726`), and the
page still renders with exterior and plan only.

**A tiled zone is one building.** A zone past the 48-per-axis structure-template
cap ships as several `.nbt` files and one manifest; `viewer` reassembles it
before it draws anything, exactly as `piece` does, and a lone tile passed by name
is refused and told which manifest claims it. Pointed at a directory holding such
a set, the page shows the zone and never its tiles.

**Packing**: the grid is run-length encoded as `(palette index u16, run length
u16)` and base64'd — 4 bytes per *run*, not per cell — and the structure is
rebuilt in the browser through the renderer's own `addBlock`. Measured over the
36 committed prefabs: `keep-gate-room` is a 368 KiB page, `island-mountain`
(42,336 cells) 464 KiB, and all 36 on one page 656 KiB — of which 281 KiB is the
vendored renderer, present once however many prefabs a page holds. Two runs over
one input are byte-identical (ADR-0006).

`#model=<id>&preset=<id>&cut=<y>` opens a specific view, so a link points at the
thing being discussed — and so a headless check can open any preset without
driving the UI.

`--textures` accepts an unpacked resource directory as well as a jar, which is
how the tests build pages with no client jar, and how a creator points the page
at their own resource pack. What an absent texture MEANS depends on which it is,
and the source says which: a jar declaring itself to be the pinned game is
complete by definition, so a block-entity texture it does not have is the
emitter's table and that version disagreeing (`DW0782`, exit 10), while a
resource pack is entitled to be partial and the same absence is an ordinary
`DW0780`.

`palette` writes the derived per-blockstate colour and shape table on its own —
the input the snapshot chart, the palette-selection tooling and the fidelity gate
read.

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
cargo test --manifest-path crates/render/Cargo.toml
```

The GPU path (`tests/gpu.rs`) is `#[ignore]` by default — it needs a GPU adapter
and the client jar:

```sh
DELVEWRIGHT_CLIENT_JAR=~/.chunky/resources/minecraft.jar \
  cargo test --manifest-path crates/render/Cargo.toml --test gpu -- --ignored --nocapture
```

## The `.nbt` → Nucleation adapter

Nucleation 0.9 has **no importer** for the binary gzip vanilla structure `.nbt`
our generator/compiler emit. The adapter (`nbt.rs`, from the spike) gunzips it
(fastnbt), reads the vanilla `size`/`palette`/`blocks` schema, and rebuilds a
`UniversalSchematic` via `set_block(BlockState::from_block_string(...))`. Textures
(the client jar) determine fidelity.
