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
3. **Blocking chart** — per-area annotated orthographic top-down slice
   (theater blocking): all anchors, NPC/actor posts, interact markers,
   stealth zones, trigger regions, walk corridor. NPC-crowding-class defects
   become visible pre-build.
4. **Shot assertions** (optional, compiled) — a cutscene shot or authored
   camera may declare `expect_in_frame: [{target, min_screen_share?}]`;
   the compiler proves it via the manifest math or fails with a DW code.
   Camera quality becomes a checkable constraint (the sea-facing seal shot
   would have been a compile error).
5. **Partial rebuild + visual regression** — `delvec preview --beat <obj>`
   recomputes only the placement/emission the edit touches and re-renders
   only affected shots; image-diff against the previous set proves a fix
   changed only what it intended.

## Loop contract (skill-level)

Author edits DSL → sub-second model rebuild → agent snapshots what it is
unsure about (its own choice of cameras) → reads image + manifest → edits →
re-snapshots. Fixed render tiers (orbit sets, POV ladder, Chunky beauty)
demote to regression/review checkpoints, not the iteration medium.

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
