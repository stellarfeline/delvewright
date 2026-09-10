# Step 12 — visual review

## Contents

- [The player's eye first](#the-players-eye-first)
- [Read every frame against the approved image](#read-every-frame-against-the-approved-image)
- [Saving the world, and installing Chunky](#saving-the-world-and-installing-chunky)
- [What the set costs](#what-the-set-costs)
- [The pieces, on an `areas[]` campaign](#the-pieces-on-an-areas-campaign)


Yours to do, not a checklist to hand off: judging a frame is the whole task.

**Read this step's whole cost before you run its first command.** Rendering is
the largest single bill on this page — hours, not minutes, and it scales with
your campaign's scene count rather than with its size on disk. *What the set
costs*, below, gives the rate and where to read the count, and it decides which
frames you render rather than how fast you render them.

## The player's eye first

**Judge the player's eye first and the set second.** The question a playtest asks
is *what does a player walking in experience*, and only a first-person frame on
the actual assembled route answers it. The build emits those: a `pov` camera at
eye height on every corner-thinned critical-path waypoint, looking along the walk
and, at each leg's end, toward the objective it arrives at, each with its own
machine `expect` line. Every POV eye sits on a proven-standable waypoint.

**So read the POV sequence in route order before you open a single orbit
render, and treat it as the primary evidence.** A scene that photographs well
from outside and reads as a corridor of grey stone from the doorway is a
finding, not a pass.

**Read each frame against its own `expect` line and nothing else.** An arrival
frame is aimed at the objective, so *the objective should be ahead in frame* is
a claim you hold the picture to. Where the standing waypoint lands on the
objective's own cell the sentence says so instead — *it is under the frame and
not in it* — and a frame that does not show the objective there is correct;
what you judge is the room it arrives in.

## Read every frame against the approved image

**Read it with `campaigns/<id>/design/concept/` open beside it, and say per
scene whether the built area is the place the approved image shows.** This is
the page's own discipline everywhere else — *author from the image, judge
against it, present every choice beside it*, the sentence step 4 wrote into
`design/README.md` — and step 12 is where the two are finally in the same hand.
Nothing upstream compares them: the gate confirms a design, the build assembles
whatever the jigsaw seated from the pool, and until this step they have never
been introduced. So for each POV frame, name the approved image it answers to
and write one of two things — *this is that place*, or a finding saying which
element of the image is not there. A frame with no approved image to answer to
is itself the finding: the design gate approved something the build does not
contain.

**Read the sky on the first frame, because it answers in one glance and it
answers for the whole set.** Every scene carries a sun placed at the hour
`world.json` declares — `render-plan.json` states it as `sky`, and `delvec
scene` refuses a plan that does not, so no frame comes off a renderer's own
default. `DW0890` has already held that hour equal to `design.json`'s rows at
every `validate` since step 5. So a frame whose light disagrees with the row for
the image it answers to is a row that states an hour the picture does not show —
the one half of this no machine can check. It is a one-field edit and a rebuild,
and it is cheap only if you catch it on the first frame instead of after reading
the set: an hour that is wrong is wrong in every frame, and re-rendering the set
costs what *What the set costs* says it costs.

**The frame carries the hour and not the weather, and it carries the hour the
delve STARTS at.** Chunky has no rain, so a `rain` or `thunder` row is judged on
everything except its sky and no frame is evidence about it either way; and if
your story moves the clock with `set-time`, every frame is still of the hour
`world.json` declares, because that is what the world save was written at. Say
both in the review rather than reading a clear noon render as a contradiction of
a row that belongs to a later beat.

**A delve declared at `night` or `midnight` renders dark, and that is the
frame.** The sun is below the horizon at those hours, exactly as it is in the
game. Declared-dark areas with a `night-vision` mitigation are the one thing the
scene emitter makes legible, and it marks those frames as emulations; everything
else you read as the player will see it. Never raise a budget, an exposure or an
hour to make a picture come out.

Two shapes to expect, because they are what the machine cannot say:

- **The area is a different place.** A shore drawn as a black rock headland
  built as a flat sand floor in a box of cobble is not a lighting note or a
  detail gap; it is the pool having no piece that is the thing the design is
  about, arriving five steps after step 2A could have acted on it. Record it as
  a finding against the placement model, not against the render.
- **The area is that place with something wrong in it** — a floating slab, a
  seam that does not meet, furniture the design does not have. Those are
  document-level fixes, below.

**A site-plan campaign has no approved image of its blockout** and this
paragraph does not apply to it — step 4 says why, and its own judgement happened
at the walk.

## Saving the world, and installing Chunky

**Save the world first — `delvec build` does not write one.** A delve's geometry
is stamped by the datapack over the first ticks of a server boot, so a build tree
carries no world save, and a Chunky scene names the world it loads. This boots
the tree once, waits over rcon until the datapack reports it has finished
placing, stops it, and writes `<build-dir>/world/`:

```sh
EULA=TRUE "$DELVEWRIGHT_ENGINE/validation/world-save.sh" \
    "$DELVEWRIGHT_ENGINE/validation/delve-output" --project dw-<id>
```

Docker, as for every other boot. `--project` is required and has no default —
it is the compose project this boot owns, so two of them run side by side
instead of tearing each other's volumes down; `--timeout` is the wait on the
datapack and defaults to 600 seconds. Nothing else in either repository produces
a world save, and the world it writes is server-written and not byte-reproducible
— which is why nothing hashes it and why re-running it is free.

`render-shots.sh` refuses without it, by name, rather than emitting scenes over
a world that is not there. That refusal is the whole reason this step is
separate: Chunky renders a missing world as an empty sky at exit 0, with the
reason buried in a Java stack trace, so the alternative is hundreds of plausible,
identical pictures of nothing and every command in the recipe green.

```sh
"$DELVEWRIGHT_ENGINE/validation/render-shots.sh" "$DELVEWRIGHT_ENGINE/validation/delve-output"
```

That writes **Chunky scenes, not images** — one per shot, plus the shot index.
Turning them into pictures needs Chunky, and **this is the step that installs
it**: Init named it and deliberately did not fetch it. Two commands, once per
machine, and they need the network:

```sh
curl -LO https://chunkyupdate.lemaik.de/ChunkyLauncher.jar
java -jar ChunkyLauncher.jar --update snapshot
```

The launcher self-installs a core into the settings directory **it** resolves —
usually `.chunky` under your account's home, which is not necessarily what
`$HOME` says, and which the paragraph below has you confirm — and what
`--update snapshot` installs is **today's** snapshot, never the pinned one:
`--update` takes a release channel, and the update site's `lib/` path serves the
current core whatever name it is asked for, so no command installs the pin. A
snapshot core is required either way — the stable line does not read 1.21.x
worlds. `render-shots.sh` has already named the pinned core, the directory it
looked in and how it resolved it, and every core that directory holds. Read all
four verdicts as different facts: `NONE installed` and `MISMATCH` both mean the
frames come off a renderer this project has not verified its scene format
against; **the pin being installed beside another core is not the same as the
pin being the renderer**, because the launcher chooses its own and has no flag
that names one. In every case but "the only core there", say in the review which
core the frames came off.

**Confirm the install by asking Chunky where it is looking, not by reading
"No updates found".** That line means the launcher found nothing newer in the
directory *it* resolved, which is not the same as an install landing where you
expected — a run can print it with the directory you thought you were installing
into not existing at all. `java -jar ChunkyLauncher.jar --help` ends with the
line `The default scene directory is <dir>/scenes`, and that `<dir>` is Chunky's
own answer for where it keeps everything, cores included. Check it against the
`chunky home:` line `render-shots.sh` printed. If they differ, set
`DELVEWRIGHT_CHUNKY_HOME` to Chunky's answer and run `render-shots.sh` again
before reading a single verdict off it. `java -jar ChunkyLauncher.jar --version`
prints the core version the launcher will actually run with.

`curl -LO` drops the jar in the current directory, which is your working
directory's root, and `java -jar ChunkyLauncher.jar` only resolves from there. `*.jar` is
not ignored here, so put it somewhere outside the tree or under `.out/` and
**write down the absolute path** — step 14 invokes it again, quite possibly in a
later session, and this page prints the bare form for readability.

Chunky reads the client jar placed at Init I5; it needs no Java 21 of its
own. Then, one process per scene, in parallel:

```sh
java -jar ChunkyLauncher.jar -scene-dir "$DELVEWRIGHT_ENGINE/validation/delve-output/shots/scenes" \
    -render <scene-name> -f -threads <n>
java -jar ChunkyLauncher.jar -scene-dir "$DELVEWRIGHT_ENGINE/validation/delve-output/shots/scenes" \
    -snapshot <scene-name> <out>.png
```

`<scene-name>` is the file stem without `.json`. **`-target` is the sample
budget** — how many samples per pixel the path tracer accumulates before it
stops — and it is the one knob that trades render time against noise; it is not
a lighting, framing or quality setting, and nothing about the picture's content
changes with it. The command above passes none, so each scene renders to the
budget `delvec scene` already wrote into it — `sppTarget: 500`, the review tier.
Add `-target <n>` only to go somewhere else on the ladder: ~64 for a draft you
only need to judge framing on, ~300 for final art (`delvec --prefabs "$DELVEWRIGHT_PREFABS" panorama --spp`'s own
default), 500 for a review frame. A POV frame you are reading as primary
evidence is rendered at the scene's own number, and 64 is for deciding whether
the camera is pointed at the right thing. Chunky's progress counter
reads `(N of <image height>)` and counts scanlines rather than samples, so watch
`spp` against the target and not that number. The core is CPU-only, so the way
to go faster is one process per scene in parallel with `-threads <n>` each, never
a smaller budget on the frame you are about to judge. This step does not skip —
a visual channel that fails soft is a review that passed without looking.

## What the set costs

**What the set costs, before you start it rather than four hours into it.** The
path tracer is CPU-bound, so the bill is a rate times a count and neither half is
hidden from you. **The rate, measured**: one POV scene at the review tier the
scene carries, `-threads 10` on a ten-core machine, took **299 s** — about five
minutes of the whole machine for one frame — and a thirteen-frame POV sequence on
that machine ran **82 minutes** end to end, so roughly 380 s a frame once a set
is going. **The count is what `render-shots.sh` already printed**: its last line
is `shot set ready: <N> Chunky scene(s) (incl. the whole-map panorama)`, and that
`<N>` is the number to multiply — read it, do not assume a size, and do not carry
a number off this page, because it is a property of your campaign. Even a
two-scene delve's full set runs to hours on that machine.

**Parallelism buys wall time, never total work.** One process per scene fills the
cores you have and stops there; the core-hours are the same whatever the process
layout, so the advice above is about where not to spend the budget rather than
about making the bill smaller. So decide the reading order up front and say what
you did: the POV sequence in route order is the primary evidence and is rendered
at the scene's own budget; the interior, seam, NPC and spawn-orbit shots come
after it. **A set you did not render is reported as not rendered**, per scene,
never left in a report where it reads as a frame that passed.

Every camera in `render-plan.json` is proven to stand in open air (`DW0724`);
read `camera_eye_proof` for how many were examined and how many had to be pulled
in off a stand-off that was inside geometry, and treat a shot carrying
`camera.requested_pos` as a hint about the build — the room is tighter than the
shot wanted, which is worth a look while you are there.

## The pieces, on an `areas[]` campaign

**Then the pieces**, for an `areas[]` campaign. Render **the pieces your
campaign actually uses**, one at a time:

```sh
delvec --prefabs "$DELVEWRIGHT_PREFABS" render piece "$DELVEWRIGHT_PREFABS/<piece>.nbt" \
    -o <workspace>/renders/<piece>
delvec --prefabs "$DELVEWRIGHT_PREFABS" render fidelity-gate
```

`delvec --prefabs "$DELVEWRIGHT_PREFABS" render batch <dir>` renders every prefab in a directory — 36 pieces and
435 shots for the shipped library — which is a library-curation tool, not a
campaign-review one. A site-plan campaign has no prefabs at this step at all.

**A piece whose light nobody has measured is refused here** (`DW0894`) —
`delvec --prefabs "$DELVEWRIGHT_PREFABS" prefab lighting <piece> --write` measures
it over its own bytes and writes the profile; a measured `dark` renders. Read the
`DW0895` line the same run prints: how much of that piece's roofed floor no body
can walk to.

Open the exterior/top/interior/anchor PNGs and check each against its `expect`
line: marker visible? room not dark? NPC facing the camera with its name as text
rather than JSON? seam clean? **Findings are document-level** — fix the campaign
(lighting profile, anchor, NPC facing, name string) and rebuild. Never hand-edit
output. Declared-dark interiors render faithfully dark, and no render will tell you
whether one is playable: that judgement belongs to the user's walk at step 9,
under the night-vision mitigation. Put it on the list you hand them there.
Never brighten a scene to make a review pass.

`delvec --prefabs "$DELVEWRIGHT_PREFABS" viewer <nbt|dir|manifest.json> -o <page.html>` is the CPU half of the
same channel and needs no GPU. Read its fidelity list before handing the page to
anyone: it names every blockstate the page cannot draw as the game draws it — a
block the pinned version does not have (`DW0790`), and the one that reads as
fine and is not, a palette entry that leaves shape-carrying properties unwritten
(`DW0791`), where the shape comes from the version's default state rather than
from the file. That is a defect in the prefab, not in the page.
