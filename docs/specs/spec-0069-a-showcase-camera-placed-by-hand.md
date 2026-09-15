# spec-0069: A showcase camera placed by hand

- **Status**: Accepted
- **Ground**: the engine at `origin/main` `b1fa8df1`, which carries the showcase
  camera record: `design/cameras.json`, read by `compiler::view::camera`, whose
  scenes `delvec cameras` emits (merged from `feature/a-shot-shows-the-best-side`). The hand camera
  writes that record; §4 states the record as built. The one live fact the
  loop rests on — the overlay's trigger → storage → `say` stamp → `delvec
  harvest` chain works for a plain, un-opped player with no datapack reload —
  is `EULA=TRUE validation/rehearsal-flow.sh`, exit 0, re-run on the branch
  that lands this spec with the hand camera in the same session (§8.9).
- **What it is for**: a released delve's storybook and front page carry a
  handful of renders, and the agent chooses their cameras. When no camera the
  agent finds is one a person is satisfied with, the person places the camera
  herself — standing in the running game where the shot should be taken — and
  **exactly that camera** is what the render uses. Her part is to stand and
  look; she types no coordinate and edits no file.
- **Research**: every rule below is marked **cited** (a vanilla fact, an ADR,
  a spec, or a document in the tree) or **authored** (this spec chooses). The
  vanilla facts are cited from the minecraft.wiki page named beside each, and
  one is marked as stated from memory where the page does not say it.
- **Numbers**: `spec-0069` is the only number taken. No DW code, ADR or
  `dsl_version` is allocated; §7 counts what the design needs.
- **Non-goals**: the agent's own camera estimation (`showcase-shots.md`); any cutscene
  camera — spec-0019 stays the authority for those; a viewer-side "save this
  camera" control (§2); a camera path or animation; any per-piece render.

## 1. The defect, and the medium the judgement lives in

**Authored**, on the finding that opened this: the shipped showcase shots of a
released campaign were judged and rejected — the exterior too far and mostly
mountain, the interiors square-on to a compass direction and not the rooms at
their best.

The agent path answers this with estimation: a camera derived from the approved
concept image and refined by bracketing (`docs/reference/showcase-shots.md`).
That is the right first answer and it is sometimes wrong, because the last
judgement — *is this the picture* — is one no gate makes. spec-0019 states
where that judgement belongs for a cutscene: **in the running game**, where the
person stands in the place and looks. A showcase render is the same judgement
about a still frame, and this spec is the path from the place she is standing
to the scene Chunky renders.

The pieces it is built from:

- The creator overlay captures a player's eye on demand (`dw.mark`, as a block
  cell) and stamps a machine-readable line into the server log that `delvec
  harvest` turns into a versioned report.
- `design/cameras.json` states each showcase camera — `pos`, `yaw` and `pitch`
  in Minecraft's own rotation, vertical `fov`, `exposure`, frame and samples,
  and the approved image it answers — and `delvec cameras` turns each into a
  Chunky scene with the world path, the declared hour's sun and the horizon's
  water attached; `--preview` draws a camera flat-lit in seconds.

So the spec is small: **one trigger that captures a continuous pose, one that
takes the body out of itself, one harvester arm, one record writer, one proof
at build, and the words that make the loop walkable.**

## 2. Two ways a person places a camera, and which is built

**Authored**, from what each surface can and cannot read.

| | in the running game | in `delvec viewer` |
|---|---|---|
| what she looks at | the assembled world: every piece seated, neighbours, terrain, the sky at the declared hour, the cast standing where it stands | one piece or one tiled zone, unlit, no neighbours, no sky |
| coordinates captured | **world** coordinates — the record's own | **piece-local**; a world camera needs the area's placement (the origin and rotation `compiler::plan` records per area), which she cannot see and which is ambiguous when one piece is placed twice |
| eye position | exact: `execute anchored eyes` at the player's current eye, any pose (standing, sneaking, spectating) | exact, from the page's own camera state (`cameraEye()`), at the page's fixed eye height or its orbit |
| look direction | exact, continuous: the entity's `Rotation` (yaw, pitch) | exact, continuous: the page's yaw and pitch |
| field of view | **not readable**: a client option the server never receives; stated by her (§4.3) | known: the page's own projection |
| moving freely | spectator: any position, through walls, no footing needed | the walk and orbit modes, no collision |
| where the render comes from | the same build's world save, the same coordinates | a per-piece GPU render, or a world camera only after the placement transform |
| what she has already done to be there | joined the step 9 server and walked the delve | opened a page the agent built for one piece |

**The game route is the one built.** The picture she is rejecting is a picture
of the assembled world; the game is the one place she sees that world whole,
and she is already standing in it at step 9. What the game cannot read — her
field of view — is one number, asked once (§4.3). What the viewer cannot read —
where the piece stands in the world — is a transform she has no way to check,
and the moment it is wrong the camera is silently in the wrong room. The viewer
route is named and not built: a piece-level camera she wants is a standing view
(`render piece --view stand=…`), and a viewer "save camera" for a world render
owes the placement transform, the disambiguation of a twice-placed piece, and a
second capture format — three things for a case the game covers.

## 3. The loop, with her actions counted

**Authored.** From *the agent's shot is not good enough* to *the record is
committed*. She is in the game already (step 9, `tools/playtest-server.sh up`,
or the compose pair); the agent is at the terminal beside the same tree.

| # | who | does | she types |
|---|---|---|---|
| 1 | she | says which picture is wrong, in chat or in game | words |
| 2 | agent | names the row the picture is (an existing row of `design/cameras.json`, or a new name and the `design.json` image it answers) and gives her a slot number for it | — |
| 3 | she | `/trigger dw.free` to leave her body (spectator; through walls, any height); fires it again later to come back | one trigger, once per session |
| 4 | she | flies and looks until the frame is right | movement |
| 5 | she | `/trigger dw.cam set <n>` — `<n>` the slot the agent gave her; `/trigger dw.cam` alone is slot 1 | one trigger |
| 6 | overlay | stamps `[DelveCamera] slot=<n> eye=<mm,mm,mm> yaw=<c°> pitch=<c°> in=<air\|block>` to the server log — data, no verdict | — |
| 7 | agent | captures the log; `delvec harvest <log> <out>/creator-datapack/layout.json --camera-out camera-report.json`; `delvec place-camera <campaign> --name <row> --report camera-report.json --slot <n> --fov <her number> [--answers <image>]` writes the row as `source: hand` | — |
| 8 | agent | `delvec cameras <build> --campaign <campaign> -o <dir> --preview --only <row>` — the draft frame, seconds — and posts it | — |
| 9 | she | looks: yes, or move and fire `dw.cam` again (back to 4) | yes / no |
| 10 | agent | `delvec build`; `validation/world-save.sh`; `delvec cameras … --draft --only <row>`; Chunky renders **that one scene** at the draft budget, then at the stated frame on her yes | — |
| 11 | she | looks at the Chunky frame: yes, or back to 4 | yes / no |
| 12 | agent | commits the record and the frame; the storybook links the frame | — |

**Her count per camera: one trigger and two looks.** Once per session: the
`dw.free` toggle, turning *FOV Effects* off, and one question the agent asks in
chat — *what does your FOV slider say?* — because the server cannot read it
(§4.3). No coordinate is typed by anyone: the overlay reads it, the harvester
parses it, the writer writes it.

Three things the loop relies on:

- **The server she is standing in carries the overlay.**
  `tools/playtest-server.sh up` stages `creator-datapack/` beside the delve's
  datapack whenever the build emitted one; the compose `playtest` profile
  mounts it. Both are step 9's commands.
- **The overlay works for a plain player.** Every `dw.*` trigger is armed each
  tick for everyone; the rehearsal flow's bot is un-opped. `dw.free` needs no op
  either: a datapack function runs at the server's function permission level —
  **cited: minecraft.wiki, *Server.properties*, `function-permission-level`,
  default 2** — which `gamemode` and `forceload` need.
- **The draft is fast and the final is one scene.** The measured rate for one
  scene at the review budget is 299 s on ten threads (`visual-review.md`); a
  draft is `delvec cameras --draft` (a quarter of the frame, 128 samples, about
  20 s in a torch-lit room, `showcase-shots.md` §3.8). The loop renders one
  scene, never the set.

## 4. What reaches the render

**Authored** for the fields and their precedence; **cited** for every check it
reuses.

### 4.1 The record, and its one writer

`design/cameras.json` is the only format of a showcase camera; this spec adds
one field to it.

| field | meaning | who fills it |
|---|---|---|
| `name` | the row, a camera name: lowercase letters, digits, `.`, `+`, `-` | agent |
| `answers` | the `design.json` row whose approved image the camera answers | agent |
| `pos` | the lens, world blocks, three floats | harvester → writer |
| `yaw`, `pitch` | Minecraft's own rotation, degrees — the stamp's convention, written with no conversion | harvester → writer |
| `fov` | the vertical field of view, degrees (§4.3) | agent, from her answer |
| `exposure`, `width`, `height`, `spp` | the camera's exposure, the frame and the sample target; a new hand row takes 1.0, 1600×900 and 300, a replaced one keeps its own | agent |
| `source` | `estimated` or `hand` | the tool that wrote the camera |

Whatever else Chunky needs — world path, sun, loaded chunks, water — the scene
emitter derives from the plan and the build, and the record does not restate
it.

**`delvec place-camera` is the record's writer.** `--report` + `--slot` +
`--fov` writes a harvested pose as `hand`; `--candidates` + `--pick` writes a
camera from a record-format file (`candidates.json`) as `estimated`; `--delete`
removes a row. A first estimate is written into the record as `estimated` by
the agent; a `hand` row is written and removed by the writer alone.

**A showcase camera is not a plan shot.** `delvec cameras` reads the record and
emits its scene; `render-plan.json`'s shots, which `delvec scene` emits with the
review policy attached, never hold one. A camera is never moved by any tool (a
hand eye is where she stood), so a hand camera inside a block is a refusal and
never a silent move. With no roster in the overlay, slots are the agent's to
name in chat; the harvester keys the report by slot.

### 4.2 How the pose is validated

Two shapes of one rule (*a camera photographs the scene*), asked by `delvec
build` over the final assembled world, for every camera of the record:

1. **The eye is not inside a block** — `DW0724`, over the same `World::is_clear`
   every plan camera is judged by, on the lens's own cell. A hand camera's
   message names the row and the one remedy — *stand somewhere else and fire
   `dw.cam` again* — and never *move the geometry*, because the geometry is what
   she was photographing. Spectator mode makes this case ordinary: a body flying
   through a wall can fire the trigger from inside it, and the stamp's `in=`
   field says whether the eye's cell held a block, so the agent can tell her
   before a build runs.
2. **The frame holds the loaded world** — the lens lies within the world's build
   height (−64..320), and the ray from the lens along the view meets the box the
   scene's chunk list is cut from (`scene::loaded_extent`, widened to whole
   blocks), read back through the plan just built. A camera that fails this
   renders an empty sky at exit 0, the failure `render-shots.sh`'s world gate
   refuses in another form. It takes no code of its own (§7).

Both run over estimated cameras too, so an estimate inside a block reds the
same way with its own remedy. A record its reader refuses stops the build as
`DW0721`. `render-plan.json` states the count as `camera_eye_proof.showcase`.

### 4.3 The field of view

**Cited** where marked:

- A Minecraft client's FOV is an option the server is never sent; it is
  vertical, with *Normal* at 70° — **cited: minecraft.wiki, *Options***.
- *FOV Effects* "changes the FOV effects of anything affecting them" — **cited:
  minecraft.wiki, *Options***. That a flying spectator's view is one of those
  effects is **stated from memory**; the page does not list them. The page
  tells her to set it off for the session, or the frame she approved in flight
  is not the frame she gets.
- **Chunky's `fov` is vertical**, measured on the pinned core
  (`docs/reference/tools.md` §4a): two scenes of one camera at 400×200 and
  400×400 compare at luminance rms 2.65 under the vertical reading and 76.11
  under the horizontal one, with both frames' hashes recorded.

So the record's `fov` is her slider's number, and the writer requires it
(`--fov`): the agent asks once per session and passes her number for every
hand row of that session.

### 4.4 Which camera wins, and how it is recorded

One row, one camera. A hand-placed camera **replaces** the estimate in the same
row and sets `source: hand`; the superseded estimate is in git, not in the
record. The writer **refuses to write an estimate over a `hand` row** and names
the row; deleting the row is her act, asked in chat, and a new hand pose on the
row replaces the old one. A row keeps its `answers` value — a camera is an
answer to a picture, and re-aiming it at another picture is a new row.

## 5. Reach from the page

**Authored.** The procedure lives in `docs/reference/tools.md` §4a, under
*Placing a camera by hand*, and carries, in this order:

1. *The server she is in already has the overlay* — step 9's command; when it
   is not up, that command again. `owner-play.yaml` publishes
   `localhost:25565`; nothing else does.
2. *Tell her the two triggers*: `/trigger dw.free` to fly and to come back;
   `/trigger dw.cam set <n>` when the frame is right, `<n>` the slot given in
   chat. FOV Effects off; ask her slider's number.
3. *Read the log*: `docker logs dw-playtest > <file>` on the `playtest-server`
   path (the name `--name` sets); `docker compose … --profile playtest logs
   --no-color > <file>` on the compose path. The log is read before `down`.
4. *Harvest*: `delvec harvest <file> <out>/creator-datapack/layout.json
   --camera-out camera-report.json` — `<out>` the build the server runs.
5. *Write the row, show the draft, render the one scene*: §3 steps 7, 8 and 10.

The `/new-delve` page names it at **step 12** (the visual review — *a POV frame
that is the wrong picture*) and at **step 14** (the storybook — *the exterior or
starting-scene shot is not the one to ship*), in one line each, and carries the
five lines under one symptom in `references/tools-by-symptom.md` — *a render
camera nobody is satisfied with* — once the page's pin (`versions.toml`
`[engine].ref`) is a release that carries `dw.cam`, `dw.free` and `delvec
place-camera`. `tools/check-skill-page.py` rule 4 holds every `delvec`
subcommand and flag the page names to the pin's clap surface, and rule 20 holds
every `dw.*` trigger, `creator-datapack/` path, `docker logs` container and
compose profile the page names to the overlay, engine, scripts and compose files
at the pin.

## 6. What is reused, what is new, and what spec-0019 gives up

**Authored.**

Reused, by name, with nothing copied:

- **The trigger → `say` → harvester chain** (spec-0006 §3, spec-0019 §3–4):
  the same `dw.*` trigger family, armed the way `creator::tick` arms it (never
  reset by the tick); the same `say` channel, because `tellraw` never reaches
  the log; the same `split_log_line` and the same single harvest pass, which
  carries a `[DelveCamera]` arm beside `[DelveShot]` and `[DelveNote]` and
  writes its report only when the log carries a stamp.
- **`dw.aim`'s capture mechanism**, generalised: a marker summoned at `execute
  anchored eyes positioned ^ ^ ^` gives the eye point in any pose, and `execute
  store result … 1000` gives it in milli-blocks as an integer — the one NBT type
  a macro substitutes without a suffix. The stamp carries **fixed-point
  integers** (milli-blocks, centi-degrees from `Rotation` scaled by 100) and the
  harvester divides. No float crosses a macro.
- **`DW0724` and `World::is_clear`**, for the eye proof, and `DW0721`, the
  record's refusal code.
- **The record's rotation convention** (`compiler::view::camera`): Minecraft's
  own, so a harvested pose is written verbatim.
- **`delvec cameras` (`--preview`, `--draft`), `render-shots.sh`,
  `world-save.sh`**, unchanged.

New:

- `dw.cam`, `dw.free` and their handlers; the `[DelveCamera]` stamp; the
  harvester arm and `camera-report.json`; `delvec place-camera`; the record's
  `source`; the build's showcase proof; the PackTest templates and the live
  flow's captures; `check-skill-page.py` rule 20.

What spec-0019 gives up, and why it costs nothing:

- **`dw.free` is the general toggle.** spec-0019 §2 names it for a replay that
  is unimplemented; here it is *leave the body, come back to it*, which is what
  a creator watching a replay from outside also needs. One name, one meaning;
  when replay lands it consumes this toggle.
- **Nothing else moves.** `dw.mark` stays a block cell — its consumer is
  `anchor + integer offset`, and spec-0019 §5's refusal of free coordinates in
  the DSL stands: a showcase camera is a render record, not a cutscene, and
  never enters a stage document's `cutscene`. `delvec calibrate` is untouched;
  a camera report never passes through it.

## 7. Numbers this spec does not take, and why

**Authored.**

- **DW codes: zero.** The eye proof and the frame proof are `DW0724`'s second
  shape; the record's refusals — at `delvec cameras`, at `delvec place-camera`,
  and at the build that reads the record — are `DW0721`, the code the record
  carries.
- **`dsl_version`: none.** The record is not a stage document.
- **ADR.** ADR-0012 (the human gives ideas and plays the result) and ADR-0003
  (the overlay is tooling-side and never ships) are invoked, not moved.
- **Demo level.** `docs/demo-levels.md` queues mechanics a player meets; this is
  a creator tool a player never sees, confirmed on the rehearsal flow's own
  fixture (`crates/dsl/fixtures/valid/cutscene-shots`), never on a campaign.

## 8. Acceptance criteria

Machine-checkable; each names its instrument and its result on the branch that
lands this spec.

1. **The triggers.** `dw.cam` and `dw.free` are registered and armed by every
   overlay — a campaign with no cutscene included — under the never-reset rule
   (`rehearsal::the_tick_never_resets_a_trigger_it_arms`, extended to both), ride
   the ADR-0006 determinism gate (`rehearsal::overlay_emission_is_deterministic`),
   and never reach the shipped datapack
   (`rehearsal::rehearsal_overlay_absent_from_the_shipped_datapack`, extended to
   the camera names). *Result: `tests/rehearsal.rs`, 13 passed.*
2. **The stamp is the pose.** PackTest, in every campaign's suite, driving the
   overlay's own handler on a dummy: standing (`creator_camera_standing`), and
   spectating inside a stone block (`creator_camera_in_a_block`), the storage the
   stamp's macro substitutes holds the eye within 0.001 block of the dummy's
   position plus the eye height, the rotation within 0.01°, and `in` = `air` /
   `block`, refusing nothing. The live flow (§8.9) holds the log line itself,
   and a sneaking eye, to the bot's pose. **This is a loosening, declared:** the
   PackTest reads the storage the stamp substitutes and leaves the log line and
   the sneaking pose to the live tier, because PackTest's chat condition hears
   system messages only (`say` is a chat message) and a dummy's pose is set on
   its tick, where a sibling in the batch may put every dummy in spectator.
   *Result: `validation/packtest-run.sh` on `cutscene-shots`, 15 of 15 passed;
   each of four perturbations of the built overlay reds its template.*
3. **The toggle restores.** PackTest (`creator_free_returns`): `dw.free` from
   adventure enters spectator; `dw.free` again returns adventure at the prior
   position to the milli-block and the prior yaw, with no place marker left.
   *Result: passed in the same run; removing the handler's teleport reds it.*
4. **One harvest pass.** A fixture log holding `[DelveNote]`, `[DelveShot]` and
   `[DelveCamera]` lines yields all three reports from one `delvec harvest` run;
   a log with no camera stamp writes no camera report; two stamps for one slot
   keep the last and count 2. *Result:
   `hand_camera::one_harvest_pass_writes_every_report_the_log_carries` and
   `orchestrator::camera` unit tests, passed.*
5. **No conversion.** A harvested pose is written into the record exactly as
   stamped — eye, yaw, pitch
   (`hand_camera::a_hand_row_is_written_as_stamped_and_no_estimate_replaces_it`);
   the scene of a record camera looks along the direction Minecraft derives from
   that rotation (`camera::the_scene_looks_where_minecraft_would_look`, 66
   rotations); and `delvec cameras --preview` of a record camera is
   byte-identical to `delvec snapshot --camera` with the same numbers
   (`tests/snapshot.rs`). *Result: passed.*
6. **The proofs bind the record.** A hand row whose eye is in the floor is
   `DW0724` naming the row and *fire `/trigger dw.cam` again*, and the plan's
   shots are byte-equal to the plain build's (the camera is never pulled in); a
   row looking at empty sky above the layout is refused naming the framed
   extent; an **estimated** row in the floor reds identically with its own
   remedy; a record its reader refuses is `DW0721`. *Result:
   `hand_camera::the_build_proves_every_showcase_camera_photographs_the_scene`,
   passed; the gallery's record binds `camera_eye_proof.showcase` 2.*
7. **Precedence.** Writing an estimate over a `hand` row is refused naming the
   row and writes nothing; after the row is deleted the same write succeeds.
   *Result: the `hand_camera` test of criterion 5 and
   `camera::an_estimate_is_never_written_over_a_hand_camera`, passed.*
8. **Reach from the page.** `tools/check-skill-page.py` rule 20 holds every
   `dw.*` trigger the page names to the overlay at the pinned `ref`, and every
   `creator-datapack/` path, `docker logs` container and compose profile to the
   engine, scripts and compose files at `ref` — red when a trigger is renamed or
   a path moves. *Result: rule 20 passes on the committed page (13 of 13 names)
   and its perturbation tests pass.* **Debt:** the page's five lines and its
   step 12 and 14 lines (§5) name `dw.cam`, `dw.free` and `delvec place-camera`,
   which the pinned release `v1.5.0` does not carry, so rules 4 and 20 red them
   until the pin moves to a release that does; they land in the pull request
   that moves it. The agent-facing procedure is in `tools.md` §4a.
9. **The live tier.** `validation/rehearsal-flow.sh` — the same flow, extended
   — has the bot fire `dw.cam set 1` standing, `set 2` sneaking, `dw.free`,
   rise four blocks and turn, `set 3` there, and `dw.free` back; the harvested
   camera report equals the bot's poses (eye within 0.001 block, rotation within
   0.01°), the return equals the place and rotation it left, in adventure, and
   the existing `[DelveShot]` and `delvec calibrate` assertions pass in the same
   run. *Result: exit 0.*
10. **The axis is measured.** `docs/reference/tools.md` §4a states that the
    pinned Chunky core reads `fov` as vertical, with the scenes and the frames'
    hashes that settled it. *Result: stated.*
11. **Byte identity.** Two builds of a campaign with a hand row produce
    byte-identical `render-plan.json`, `delvec cameras` scene and `delvec index`
    shot index, and the record is a hashed build input (ADR-0006). *Result:
    `hand_camera::a_hand_row_builds_byte_identically`, passed.*
12. **The record.** `docs/reference/compiler.md` carries `DW0724`'s second
    shape, `DW0721` at the build and at `place-camera`, `delvec place-camera`,
    the overlay's two triggers, the stamp, `camera-report.json` and the PackTest
    templates; `docs/reference/tools.md` carries the harvester's third report,
    the writer, the loop and the measurement; spec-0019 §2's `dw.free` line is
    restated to §6's meaning. *Result: in the pull request that lands the
    code.*

## 9. Decisions

1. **The game route only** (§2). When she wants a picture of the assembled
   world, she joins the step 9 server; a viewer page never writes a world
   camera.
2. **The field of view is hers to state, once per session** (§4.3), with FOV
   Effects off while she frames.
3. **A hand camera is final until she deletes it** (§4.4). The agent does not
   re-estimate a shot she placed; when a later build moves the world under it,
   the row reds (`DW0724`) and she is asked.
