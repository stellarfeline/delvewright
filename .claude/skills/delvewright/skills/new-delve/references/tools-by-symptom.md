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
- **Handing a build to a playtester**: the playtest note flow — `/trigger dw.note`
  in-game, then `delvec harvest` → `playtest-report.json`. Human-optional.
- **Delivering or revising a cutscene**: shot calibration — in-game
  `/trigger dw.mark set <s>` (stand where the camera should be), `dw.aim set <s>`
  (look at the subject), `dw.faster`/`dw.slower set <s>`, then `/trigger dw.done`
  once; `delvec harvest` writes `rehearsal-report.json` and `delvec --prefabs "$DELVEWRIGHT_PREFABS" calibrate
  <report> --layout <out>/creator-datapack/layout.json` turns it into an
  anchor+offset patch you apply and rebuild. Human-optional. (Beat replay —
  `dw.beat` / `dw.shot` / `dw.free` — does not exist; do not promise it.)
- **A prefab library needing human taste, not machine checks**: `delvec prefab
  gallery` (browse world) → a reviewer walks it and leaves notes → `delvec prefab
  curate` / `curate-merge` fold them into the catalog cards. Human-optional.
- **Several candidate prefabs for one slot, and a human has to pick**: `delvec --prefabs "$DELVEWRIGHT_PREFABS" 
  contact-sheet <renders> -o <png>` — all the candidates on one page, each
  labelled with its rank and id, with `$DELVEWRIGHT_ENGINE/tools/refscore.py` optionally ordering the
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
  `PYTHONPATH="$DELVEWRIGHT_ENGINE/tools/skin" .venv-skin/bin/python -m delve_skin all <cast.json>
  --skins-dir … --preview-dir … --catalog-dir …`, in the venv *NPC skins* establishes at step 5. **Look at the previews**, and
  always set `model` (`wide`/`slim`) — an omitted model renders slim and distorts
  a wide skin. The compiler bakes the PNG into the delve's resource pack from
  `campaigns/<id>/skins/`.
- **Cleaning up a ladder or a play session by hand**: `$DELVEWRIGHT_ENGINE/validation/fresh-volumes.sh
  --project <id>`. `--project` is required everywhere and there is no daemon-wide
  mode. It reclaims what the project owns — containers, volumes and networks —
  and proves it. The `--profile play` stack from step 9 pins a fixed container
  name, so tear that one down with `docker compose … down -v` or
  `$DELVEWRIGHT_ENGINE/tools/playtest-server.sh down` rather than with `fresh-volumes.sh`.
