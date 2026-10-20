# Step 12 — visual review

## Contents

- [The player's eye first](#the-players-eye-first)
- [Read every frame against the approved image](#read-every-frame-against-the-approved-image)
- [Drawing the set](#drawing-the-set)
- [What the set costs](#what-the-set-costs)
- [Reading the set](#reading-the-set)


Yours to do, not a checklist to hand off: judging a frame is the whole task.

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
answers for the whole set.** Every frame is drawn under the sky its shot states:
the hour and weather `world.json` declares, moved by every `set-time` and
`set-weather` the critical path fires before that shot — so a frame of a leg
after a cut to dusk is a frame at dusk, and its `sky.set_by` names the step that
set it. `DW0890` has already held every hour the world reaches equal to
`design.json`'s rows at every `validate` since step 5. So a frame whose light
disagrees with the row for the image it answers to is a row that states an hour
the picture does not show — the one half of this no machine can check. It is a
one-field edit and a rebuild.

**The frame carries the hour and the weather's light, not the weather in the
air.** A `rain` or `thunder` frame is lit and skied as the game lights and skies
that hour under that weather, but no rain falls and no cloud is drawn; each
frame's `unchecked` list says so. A delve declared at `night` or `midnight` is
drawn dark, and that is the frame: the sun is below the horizon, exactly as in
the game. **No frame shows the moon**, so a night frame says nothing about the
phase; the phase is confirmed in the game. Declared-dark areas with a
`night-vision` mitigation are drawn without the effect and their `unchecked`
names the lightmap: judge their legibility in the game. Never move an hour to
make a picture come out, and never type a camera's exposure: the engine derives
it. A place that reads dark in a quick frame is dark in the game — a quick frame
is the lightmap the client draws from the light the world carries — so light it
where the room is designed
([new-pieces.md](new-pieces.md#light-inside-a-cave-or-a-body)).

**A change to how frames are rendered is judged on the standard level first:
The Glasshouse** (content `demo/the-glasshouse`), its seven showcase cameras
under every sky the change can move, measured against the game's own values
(`$DELVEWRIGHT_ENGINE/docs/reference/showcase-shots.md` §6). A campaign's set is
never where a render change is first looked at.

What the machine cannot say, and this review can: **the place is that place
with something wrong in it** — a floating slab, a seam that does not meet,
furniture the design does not have. Those are document-level fixes: a piece's
program at step 9, or the plan. **No approved image shows the blockout** — step
4 says why — so the built map's own judgement is this POV review, read against
the scene concept art.

## Drawing the set

**One command draws the whole set**, in-process, from the worlds the build
wrote — one per configuration its shots stand in, under
`validation/worlds/<key>/` — with the textures of the client jar Init I5 put on
this machine:

```sh
delvec --prefabs "$DELVEWRIGHT_PREFABS" frames "$DELVEWRIGHT_ENGINE/validation/delve-output"
```

It writes `<stem>.png` and `<stem>.manifest.json` per shot and `shot-index.json`
under `validation/delve-output/frames/`. **Quote its first line in the review**:
`renderer: lumenwright <version>, backend <name> …` says what the frames came off,
and whether a GPU drew them or the CPU did (`auto: fell back to cpu` with the
reason). A shot naming a world the build directory does not hold is refused by
name with the remedy, a rebuild, before anything is drawn; so is a plan an older
engine wrote. `--shot <id>` draws one shot (repeatable), `--labels` stamps every
target's outline and name on the frame, `--backend cpu` names the CPU. No
server boot, no install, no other program: a frame is never drawn from a world
the build did not write.

## What the set costs

**Seconds to minutes for a whole set.** Measured on a ten-core development
machine: a 24-view demo level's set in 9 s on the CPU and 4 s on the GPU (Metal), its 1600 × 900 panorama in 45 s and 2 s, a 200-frame set in 68 s and 40 s. The frames' own binding lines state the set's wall time,
peak memory and per-frame times on every run — read them rather than carrying a
number off this page. Every shot is drawn, in route order in the index; a set
you did not draw is reported as not drawn, per shot.

Every camera in `render-plan.json` is proven to stand in open air in the world
its frame stands in (`DW0724`); read `camera_eye_proof` for how many were
examined, how many had to be pulled in off a stand-off that was inside geometry
or behind a wall, and in how many worlds, and treat a shot carrying
`camera.requested_pos` as a hint about the build — the room is tighter than the
shot wanted, which is worth a look while you are there.

## Reading the set

**Read each frame's index entry before its picture**: `shows` (every target in
frame, with its screen box, distance and whether a block hides it — the bodies
the world holds among them, as kind `body`), `undrawn` (the bodies in frame the
renderer did not draw — a kind it has no model for, a holder of an item it cannot
draw — each with its reason
and how to judge it instead; `DW1039` on stderr), and `unchecked` (what no frame
can show). A frame whose `shows` holds no NPC where one should stand is a
finding; an `undrawn` marker is not — it is real and the renderer has no model
for it yet. Then open the picture and check it against its `expect` line: room
not dark? NPC facing the camera? seam clean? **Findings are document-level** —
fix the campaign (a place's program, an anchor, NPC facing, a name string) and
rebuild. Never hand-edit output. Never brighten a scene to make a review pass.

`delvec --prefabs "$DELVEWRIGHT_PREFABS" viewer <nbt|dir|manifest.json> -o <page.html>` is the CPU half of the
same channel and needs no GPU. Read its fidelity list before handing the page to
anyone: it names every blockstate the page cannot draw as the game draws it — a
block the pinned version does not have (`DW0790`), and the one that reads as
fine and is not, a palette entry that leaves shape-carrying properties unwritten
(`DW0791`), where the shape comes from the version's default state rather than
from the file. That is a defect in the piece, not in the page.
