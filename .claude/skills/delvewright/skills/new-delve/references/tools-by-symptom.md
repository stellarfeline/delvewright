# Reference: tools by symptom

Two classes, one rule each:

- **Workflow tools are steps, not options.** Where a step above says "always" or
  "mandatory", skipping it is skipping validation.
- **Human-in-the-loop tools are offered, never required.** When the flow reaches
  the marked point, say in one line that the tool exists and what it would catch
  — then keep going. Never block or wait on a use/don't-use answer.

The full inventory — every binary, script and flag that exists today — is
`$DELVEWRIGHT_ENGINE/docs/reference/tools.md`. Check it before assuming a capability is missing.

- **Judging any visual outcome** (cutscene framing, set dressing, terrain):
  `delvec snapshot` (`--camera x,y,z,yaw,pitch`, `--at <anchor> --dist`,
  `--shot <render-plan id>`, `--labels`) and `delvec blocking-chart` (per-floor
  cutaways). *Always* for cutscenes: render start, mid and end of every dolly
  segment and **look at the frames** before calling the step done — `DW0308`
  proves the path is air, not that the shot is pointed at the subject, and an
  inside-out cinematic can be fully DW-green.
- **Terrain or visual fixes beyond swapping prefabs**: `delvec edit` — the map
  editor loop (edit script batch → replay → snapshot). The script is a campaign
  document, `world-edits.json`: batches of declared edits, replayed
  deterministically, with the post-batch invariants enforced. Never hand-patch
  `.nbt` or invent block edits outside it.
- **An approved picture no view of the built world answers** (`DW0900`): the
  showcase camera record, `campaigns/<id>/design/cameras.json` — one camera per
  row of `design.json`, estimated from the picture and drawn over the assembled
  world in seconds. `delvec --prefabs "$DELVEWRIGHT_PREFABS" cameras <build-dir>
  --campaign <campaign-dir> -o <dir> --preview` draws every camera flat-lit in
  seconds and `--bracket` writes the candidates beside them; `delvec --prefabs
  "$DELVEWRIGHT_PREFABS" place-camera <campaign-dir> --name <row> --candidates
  candidates.json --pick <candidate>` is the record's one writer. The build
  refuses a record that leaves any row without a
  camera and names the pictures; the repair is a camera for each, never
  re-aiming one camera at a second picture. A campaign that has placed no camera
  at all still builds — the first build is what a camera is estimated against —
  and is stopped at the staging gate instead. Workflow, at step 8b; the craft is
  `$DELVEWRIGHT_ENGINE/docs/reference/showcase-shots.md` §4.
- **A render camera nobody is satisfied with**: stop estimating and let her
  stand where the shot is. Five steps, in this order. **(1)** The server she is
  in already has the overlay — it is step 9's command, and when it is not up,
  that command again; `owner-play.yaml` publishes `localhost:25565` and nothing
  else does. **(2)** Tell her the two triggers: `/trigger dw.free` to fly and to
  come back, and `/trigger dw.cam set <n>` when the frame is right, `<n>` the
  slot you give her in chat. FOV Effects off; ask her slider's number. **(3)**
  Read the log before `down`: `docker logs dw-playtest > <file>` on the
  `$DELVEWRIGHT_ENGINE/tools/creator/playtest-server.sh` path, or `docker compose …
  --profile playtest logs --no-color > <file>` on the compose path. **(4)**
  Harvest it: `delvec --prefabs "$DELVEWRIGHT_PREFABS" harvest <file>
  <out>/creator-datapack/layout.json --camera-out camera-report.json`, `<out>`
  the build the server runs. **(5)** Write the row with `delvec --prefabs
  "$DELVEWRIGHT_PREFABS" place-camera <campaign-dir> --name <row> --report
  camera-report.json --slot <n> --fov <her slider>`, show her the draft
  (`cameras --draft`) and render that one scene (`cameras --only <row>`). A hand pose replaces the estimate in its own row and
  the writer refuses to put an estimate back over it; deleting the row is her
  call, asked in chat. Human-in-the-loop, at step 12 and at step 14; the full
  procedure is `$DELVEWRIGHT_ENGINE/docs/reference/tools.md` §4a.
- **Handing a build to a playtester**: the playtest note flow — `/trigger dw.note`
  in-game, then `delvec harvest` → `playtest-report.json`. Human-optional.
- **Delivering or revising a cutscene**: shot calibration — in-game
  `/trigger dw.mark set <s>` (stand where the camera should be), `dw.aim set <s>`
  (look at the subject), `dw.faster`/`dw.slower set <s>`, then `/trigger dw.done`
  once; `delvec harvest` writes `rehearsal-report.json` and `delvec --prefabs "$DELVEWRIGHT_PREFABS" calibrate
  <report> --layout <out>/creator-datapack/layout.json` turns it into an
  anchor+offset patch you apply and rebuild. Human-optional. Beat replay does
  not exist at this engine; do not promise it.
- **A prefab library needing human taste, not machine checks**: `delvec prefab
  gallery` (browse world) → a reviewer walks it and leaves notes → `delvec prefab
  curate` / `curate-merge` fold them into the catalog cards. Human-optional.
- **Several candidate prefabs for one slot, and a human has to pick**: `delvec --prefabs "$DELVEWRIGHT_PREFABS" 
  contact-sheet <renders> -o <png>` — all the candidates on one page, each
  labelled with its rank and id, with `$DELVEWRIGHT_ENGINE/tools/creator/refscore.py` optionally ordering the
  page by similarity to the design gate's reference image. Human-optional. Say
  plainly that the score only orders the page: every candidate is on it, and the
  low scorer is present, last — the human is the selector, the number is not.
- **A picture cannot say what a prefab is like to be inside**: `delvec --prefabs "$DELVEWRIGHT_PREFABS" viewer
  <nbt|dir|manifest.json> -o <page.html>` — one self-contained HTML page with a
  camera the reviewer drives: exterior, plan, a player point of view at eye
  height (1.62) standing at every declared anchor and doorway, plus a cutaway
  slider that takes the roof off. A zone that ships as several tiles and a
  manifest shows as one building. Pass a directory to put a whole library on one
  page. Human-optional; read its fidelity list first (step 12).
- **A picture of the whole map** (storybook hero image, release asset): `delvec --prefabs "$DELVEWRIGHT_PREFABS" 
  panorama <build-dir> -o <dir> [--bearing se|sw|ne|nw] [--spp N]` — a 45° oblique
  scene framing the entire layout, computed from the plan. Never hand-edit a scene
  JSON to get one.
- **An NPC needs a look no vanilla mob gives you**: the skin toolchain,
  `PYTHONPATH="$DELVEWRIGHT_ENGINE/tools/creator/skin" .venv-skin/bin/python -m delve_skin all <cast.json>
  --skins-dir … --preview-dir … --catalog-dir …`, in the venv *NPC skins* establishes at step 5. **Look at the previews**, and
  always set `model` (`wide`/`slim`) — an omitted model renders slim and distorts
  a wide skin. The compiler bakes the PNG into the delve's resource pack from
  `campaigns/<id>/skins/`.
- **Cleaning up a ladder or a play session by hand**: `$DELVEWRIGHT_ENGINE/validation/fresh-volumes.sh
  --project <id>`. `--project` is required everywhere and there is no daemon-wide
  mode. It reclaims what the project owns — containers, volumes and networks —
  and proves it. The `--profile play` stack from step 9 pins a fixed container
  name, so tear that one down with `docker compose … down -v` or
  `$DELVEWRIGHT_ENGINE/tools/creator/playtest-server.sh down` rather than with `fresh-volumes.sh`.
