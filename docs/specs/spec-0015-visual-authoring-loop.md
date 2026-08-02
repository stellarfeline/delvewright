# spec-0015 — The visual authoring loop (agentic viewport)

- **Status**: Draft (planner, 2026-08-01; from the owner's round-4 QA findings
  and design discussion — camera control belongs to the designing LLM)
- **Problem**: 3D work (scenes, camera moves, NPC blocking) has no tight
  feedback loop. Content that compiles can look wrong; the render tiers are
  fixed shot-lists that miss what the designer is actually unsure about
  (sea-level mismatch, sheared canopies, an entire invisible cavern); review
  feedback is prose without a coordinate vocabulary; the
  edit→compile→render→review cycle is minutes long and whole-campaign wide, so
  visual fixes get mangled by unrelated compile churn.
- **Model** (owner-endorsed): like an LLM iterating on a web page with
  screenshots — the creator agent gets tools to LOOK at its own build,
  wherever it decides it needs to look, sub-second, mid-authoring. Voyager's
  lesson applies (iterative environment feedback loops), but where Voyager is
  text-only, our loop is dual-channel: image + structured manifest.

## Pillars

1. **`delvec snapshot`** — in-compiler voxel raycaster over the assembled
   world model. `--camera x,y,z,yaw,pitch[,fov]` or `--at <anchor> --orbit`
   or `--shot <render-plan id>`. Block-palette draft quality, sub-second,
   deterministic. Works on a partial build (any stage after placement).
   Optional `--labels` burns in anchor/NPC tags + a coordinate grid.
2. **Scene manifest** — every snapshot (and every existing render-tier shot)
   emits a JSON sidecar: entities/anchors/props in frustum with world coords
   and screen-space bboxes, occlusion flag. The shared vocabulary: review
   feedback and edits reference ids and boxes, never prose-only ("raise the
   shelf in [200..204, 63..66, -12..-8]" is now expressible and mappable).
3. **Blocking chart** — per-elevation CUTAWAY orthographic slice rendered
   straight from the voxel model (no in-world camera exists, so ceilings are
   simply excluded above the cut plane — dollhouse view); walkable-Y
   clustering auto-detects levels (cavern floor vs ramp/pen = two slices).
   All anchors, NPC/actor posts, interact markers, stealth zones, trigger
   regions, walk corridor labeled. NPC-crowding-class defects visible
   pre-build; interiors fully covered by construction.
4. **Shot grammar = the assertion** (owner correction: hard share thresholds
   would strangle wide shots). A shot declares its GRADE (`establishing` /
   `medium` / `close` / `insert`) plus optional framing (screen-position
   soft zone, FOV); the ONLY compiled check is "the frame matches the
   declared grade": establishing asserts subject-in-frame & unoccluded
   (a small silhouette passes — that is what a wide shot is), close asserts
   a share floor, undeclared asserts nothing. Creative control and proof are
   the same declaration; the sea-facing shot still dies at compile (its
   subject was absent entirely).
5. **Partial rebuild + visual regression** — `delvec preview --beat <obj>`
   recomputes only the placement/emission the edit touches and re-renders
   only affected shots; image-diff against the previous set proves a fix
   changed only what it intended.

## Loop contract (skill-level)

Author edits DSL → sub-second model rebuild → agent snapshots what it is
unsure about (its own choice of cameras) → reads image + manifest → edits →
re-snapshots. Fixed render tiers (orbit sets, POV ladder, Chunky beauty)
demote to regression/review checkpoints, not the iteration medium.

## Shot-style template library (research-backed)

Procedural cinematic cameras in RDR2/GTA V cinematic mode and Unity
Cinemachine are algorithmic, not AI: a curated template bag (static
pan-tracking, side-track dolly, crane/orbit, low-follow), damped look-at
springs, per-template lens/FOV, min/max shot durations, cut-on-occlusion
rather than wall-sliding. spec adds `shot_style` presets the cutscene DSL can
invoke; the compiler expands a style deterministically into path + look-at +
FOV. A research dossier (sources → ACKNOWLEDGEMENTS) parameterizes the
template set before implementation.

## Delivery order

P1: snapshot + labels + manifest (compiler has all data; pure additive tool).
P2: blocking chart; manifest for existing tiers. P3: shot assertions + DW.
P4: partial rebuild + image-diff regression. Chunky stays the final-beauty
pass; night-vision-emulated dark-area review folds in via task #60's flag.

## Acceptance criteria

1. `delvec snapshot --at anchor/fire-pit --orbit` produces a PNG + manifest
   in <1 s on the island build; manifest lists the fire-pit prop and any
   NPC posts in frame with screen bboxes.
2. A cutscene with `expect_in_frame` on a subject that is out of frame fails
   with the new DW code; re-aimed, it passes.
3. Blocking chart for area/island shows every anchor/NPC/interact/stealth
   region labeled; the round-3 cheese-store crowding is visible on it.
4. `delvec preview --beat` after moving one NPC re-renders only shots whose
   frustum contains the change; image diff is empty elsewhere.

## Non-goals

Beauty rendering in the loop (Chunky remains out-of-band); free-form 3D
editing UI; any runtime (in-delve) tooling.
