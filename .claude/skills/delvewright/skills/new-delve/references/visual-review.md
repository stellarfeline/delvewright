# Step 12 — visual review

## Contents

- [The player's eye first](#the-players-eye-first)
- [The threshold frames, read blind](#the-threshold-frames-read-blind)
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

## The threshold frames, read blind

**After the POV sequence, read every `threshold` frame blind.** The build draws
one per door of every place — the eye on the doorway's sill at eye height,
looking into the place — and one per open side, the eye in the cell outside
it; the plan's `threshold_shots` and the build's `threshold
shots: T over P place(s), D door(s), O open side(s), Q part(s)` line say how
many. A plot draws one more per part from each of its crossings, the eye on the
plot's side of the crossing and aimed at the part. Draw them without
`--labels`: a label would supply what the picture does not.

**Write three lines under each frame before you open its index entry**:

1. **The trade** — what the place is, in a word, from the picture alone.
2. **The front** — do the parts in frame (counter, door, window, posts, beam,
   wall, eave) belong to one construction, in one word, and if not, which part
   jars and against what: *the door stands in air over the counter*; *the beam
   stops short of the door*; *the third block kind has no job*. Write it with
   the place's entry in `validation/piece-census.json` open beside the frame —
   its `doors` (each door and gate with its jambs and head, present or absent)
   and its `openings` (per face, each opening's head, sill and width). A door
   entered with its head absent is read against the picture, never refused by
   it.
3. **The thing** — on a frame whose eye stands outside an open side looking into
   a place with no `clearance` ceiling (a square, a yard, a court), the name of
   what stands in it, in one word: *fountain*, *well*, *statue*, *tree*,
   *nothing*. Read the entry's `water` beside it — how much, and its highest
   and lowest course against the walk plane.
4. **The holding** — on a part frame (the shot carries `part`), what the part is
   for, in one phrase, and whether it belongs with what else is in frame: *a
   shed behind a house*; *a store beside a forge yard*; *a hall and a hut that
   have nothing to do with each other*. Read the plot's entry in
   `validation/plot-program.json` beside it — each part's declared and built
   cells, the setbacks, the gaps, the coverage and the remainder.

Then open the entry and read `expect` — *the place reads as its intent: …* —
against the three lines. **A trade that cannot be named or is named wrong, a
front with a jarring part, or a thing named wrong or not nameable is a finding
at the place's program**: step 9 again for that place, a fresh author, the
finding written into the node's `note` so the handout carries it with the four
style lines, and the census entry for that piece quoted beside the finding, so
the author sees the doors and their framing, the heads, the kinds and the water
you saw. State the read as one line in the review:
`threshold read: T frame(s), trade misses M, front misses F, thing misses N,
holding misses H`. **A use that cannot be named, or a part that does not belong,
is a finding at the plot's program** — the plan's parts and the piece together —
repaired by a fresh author with the finding and
`$DELVEWRIGHT_ENGINE/docs/reference/place-craft.md` §8.5 in its handout, the
plot's `plot-program.json` entry quoted beside the finding.

## Read every frame against the approved image

**Read it with `campaigns/<id>/design/concept/` open beside it, and say per
scene whether the built area is the place the approved image shows.** This is
the page's own discipline everywhere else — *author from the image, judge
against it, present every choice beside it*, the sentence step 4 wrote into
`design/README.md` — and step 12 is where the two are finally in the same hand.
Nothing upstream compares them: the gate confirms a design, the build assembles
the pieces the detail plan binds, and until this step they have never been
introduced. So for each POV frame, name the approved image it answers to
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
the game. **A clear frame draws the moon at its day's phase**, the stars and the
sunrise and sunset fan, as the client draws them at that clock; under rain or
thunder the client draws no moon, and no frame does. Declared-dark areas with a
`night-vision` mitigation are drawn without the effect and their `unchecked`
names the lightmap: judge their legibility in the game. Never move an hour to
make a picture come out, and never type a camera's exposure: the engine derives
it. A place that reads dark in a quick frame is dark in the game — a quick frame
is the lightmap the client draws from the light the world carries — so light it
where the room is designed: natural sources set in the walls and the vault,
artificial ones recessed behind a partial block of the surface's own family,
staggered at uneven spacings and heights, each on a face that holds it
(`DW1002`).

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
delvec --prefabs "$DELVEWRIGHT_PREFABS" frames "$DELVE_OUTPUT"
```

It writes `<stem>.png` and `<stem>.manifest.json` per shot and `shot-index.json`
under `"$DELVE_OUTPUT"/frames/`. **Quote its first line in the review**:
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
