# Step 2 — placement, and which model a campaign takes

## Contents

- [The test, and how to run it](#the-test-and-how-to-run-it)
- [2A. `areas[]`](#2a-areas)
- [2B. The site plan](#2b-the-site-plan)

## The test, and how to run it

"There is no prefab that is the building the story is about" is the whole
routing question, and it is not answered by reading pool names: a pool called
`vertical-keep` is an entry hall, corridors and terminal rooms, which is the
*inside* of a keep and not a keep. So ask the question in the form that has an
answer:

> **Does the thing the delve is named after have an exterior silhouette the
> player is meant to read** — a shape they stand outside of and see against the
> sky? A pool of interiors can be the inside of a building. It can never be the
> building.

Then look, rather than deciding from memory. One command draws the whole shipped
library as one self-contained page — orbit, plan, cutaway, and a player eye at
every anchor:

```sh
delvec --prefabs "$DELVEWRIGHT_PREFABS" viewer "$DELVEWRIGHT_PREFABS" -o .out/library.html
```

Open it and put one question to the thing the delve is named after. **Is it on
that page, as a shape?** If it is not, and the story asks the player to look at
it from outside, `areas[]` is over: take the site plan, where the geometry is
derived from your own plan and there is no prefab left to be missing. **If
instead it is a place the party walks through**, and the members of some pool
are its rooms, `areas[]` holds — say so in `GENERATION.md` in one line, so the
reading is visible to the next round rather than implied.

Take the site plan on a tie. It costs four more documents and the map's own
reference views; taking `areas[]` wrongly costs step 2 over again, and the page
cannot get you back there from step 5.

---


## 2A. `areas[]`

`world.json`'s `content.areas` seats pieces from the prefab library. Prefer
`prefab_pool` for real layouts; read `"$DELVEWRIGHT_PREFABS"/pools.json` and the
per-prefab metadata for the pools, anchors and lighting profiles available.
Respect the lighting contract — darkness only as declared design, with a
mitigation the quest graph provides.

**Two rules about multiple areas, and both are about pieces rather than about
your story.** Areas sit 256 blocks apart across void with no walkable link, so
crossing between them is not a walk — the compiler emits a one-way teleport on
the objective that crosses, which is what makes "the boulder seals the cave" a
fact about the geometry rather than an assertion. That crossing is emitted only
under two conditions:

1. **The campaign's first beat plays in the area the party starts in.** A
   crossing rides on an objective completing, and at the spawn nothing has
   completed yet — so there is no beat to hang the first crossing on. A delve
   whose opening move would be a teleport out of its own spawn area has put its
   spawn in the wrong area; move the spawn, or put a beat in the spawn area
   first.

2. **Every area a beat crosses into declares an entry point.** That is an
   anchor carrying `"role": "entry"` in the piece's metadata, or an anchor
   literally named `spawn` or `entry`. **Very few pieces have one**, and a pool usually holds exactly one
   that does, so a multi-area campaign is a constraint on which piece each area
   may bind rather than a free narrative move. How few is a fact about the
   library and not about this page, so read it rather than taking a number from
   here — the library is a separate artifact on its own cadence:

```sh
"$DELVEWRIGHT_PYTHON" - <<'EOF'
import json, glob, os
LIB = os.environ["DELVEWRIGHT_PREFABS"]
n = 0
for f in sorted(glob.glob(f"{LIB}/*.json")):
    if os.path.basename(f) == "pools.json" or f.endswith(".report.json"): continue
    n += 1
    a = json.load(open(f)).get("anchors") or {}
    if any(v.get("role") == "entry" for v in a.values()) or {"spawn","entry"} & set(a):
        print(os.path.basename(f)[:-5])
print(f"-- out of {n} piece(s) in the library")
EOF
```

A crossing that was never emitted is not a quiet difference — it is a delve the
party cannot finish. See *Reference: when something goes red* for what it looks
like.

**A pool area guarantees its `entry`-role member's anchors, plus whatever this
campaign REQUIRES.** An area declaring `pieces: {min: 3, max: 4}` over a
thirteen-member pool seats a subset: the `entry` member at the area origin on
every draw, one carrier for each anchor the campaign requires the solver to
guarantee, and `connector` fillers drawn from the seed. A `room` or `terminal`
member nothing requires is never seated, and an anchor it declares is not in the
built world.

Ask the engine, before the story is shaped around something that is not there —
it reads the library and needs no campaign, so you can ask it at this step:

```sh
delvec --prefabs "$DELVEWRIGHT_PREFABS" prefab anchors            # every pool
delvec --prefabs "$DELVEWRIGHT_PREFABS" prefab anchors --pool pool/cave-shore
```

It prints, per pool, the member count, the `entry` member, the anchors that
member declares — the unconditional guarantee — and every other name with the
carrier it would have to arrive on and the role that decides when the layout
seats it.

**Read what it prints; do not carry a number off this page.**

**Three ways to design against that.** Design against the entry member's anchors. Or *require* the anchor you
want, at a site the solver must honour — an objective, an NPC stand, a wave
spawn, a lane waypoint, or an anchor-bearing effect in that area — which forces
its carrier to be seated, and everything else that carrier declares with it. Or
bind a single `prefab` instead of a `prefab_pool`, in which case every anchor it
declares is yours.

**When you get it wrong, `validate` says so, not the build.** `DW0889`
(advisory) names every anchor your documents carry that the area does not
guarantee, with the piece it lives on and why the layout may not seat it. It is
a warning because the filler draw may well seat the piece — but if it did not,
`DW0360` fails the build at step 8. `DW0498` is a different line about the same
area: it reports a pool that seated one anchor-bearing piece *twice*, which
needs the settled draw and so still arrives at step 8.

**One more piece fact worth knowing before you place anything.** An anchor name
is unique per *area*, so binding the same prefab to two areas makes every anchor
it declares ambiguous (`DW0857`); the fix available to you is a **different
piece for one of the two areas**, not renaming an anchor in the shared library.
A `DW0498` repeat is a hard `DW0305` the moment an objective, NPC stand, gate or
wave spawn hangs on one of the names it made ambiguous.

**When the library has no piece the design needs, decide which of two things you
are looking at.** Neither answer is "make a prefab now".

- **The missing piece is the building the story is about** — the keep, the mill,
  the tower the delve is named after. Then `areas[]` was never the right
  placement model: go back to *Which placement model* and take the site plan,
  where the geometry is derived from your own plan and there is no prefab left to
  be missing. This is the one decision that cannot be changed later without
  redoing step 2, so change it here and not at step 5.
- **The missing piece is a room `areas[]` still wants** — a second area's entry
  point, a room whose chest a `collect` has to adopt, a shape the pools do not
  hold. Then you owe a piece, and you build it **after the design gate at step 4
  and before step 5**, entering *Reference: when the prefab library has no piece
  you need* at that point. That is the section's second entrance; step 9.2 is
  the other.

**After the gate, for the gate's own reason.** Step 4 confirms the design on
concept art drawn *before any prefab exists*; a piece built first and rendered
for the gate is a build being approved as a design, which is the inversion step
4 names. And before step 5, because step 5 binds anchors and the anchors are the
piece's own — there is no `cast` block to write against a piece that has not
declared its marks.

**Until then, do not carry a name for a piece that does not exist.** An
`areas[]` entry naming a piece the library does not have is `DW0856`, and the
refusal is not the expensive part: an area whose piece is absent contributes no
anchor set at all, so every per-area anchor proof over it is SKIPPED rather than
failed, and steps 3 and 4 then read quieter than they are. Write the area entry
when the piece is admitted; at the gate, say which pieces the design still owes
and what each one is for.

## 2B. The site plan

Three documents, in this order, each the input the next one needs. **The order
is the only order that compiles** — there is no blockout document and nothing to
author early, so no later document can reach green first.

**These three are the hardest documents on the page to read out of a schema, and
`delvec metrics --gym <dir>` writes all three of them filled in.** It builds a
whole site-plan campaign out of the metrics table — nine complete documents that
build — so it is where you go for a worked `geometry-brief.json`,
`layout-graph.json` and `site-plan.json` rather than dereferencing
`delvec schema --stage all`. Read the gym's copy, then write your own; the
schema is still the authority on form.

**Before any of them: the whole map gets a reference of its own.** A composition
written without one is free invention with no criterion. On path A of Init I7 it is already in `design/reference/` — read it. On path B, draw it now; the
form and the commands are in *Reference: drawing the map's reference*.

1. **`geometry-brief.json`** — the whole's written design reduced to *numbers*:
   `facts[]` of `{id, value, unit?, note}`. A fact is a number with a name.
   Write the numbers the design actually commits to — how far across the site
   is, how tall the thing the campaign is named after stands, how far the
   approach runs. Reference imagery is style authority and never dimensional
   authority: an identity binds to a number, never to a picture.

2. **`layout-graph.json`** — the space as a graph, **before any coordinate
   exists**. `nodes[]` are places (`{id, intent, stations?, note?, reached?}`);
   `edges[]` are connections (`walk | stair | climb | drop | barred | carry |
   vision`, with `gating`, `one_way`, `shortcut`, `opens_from`). Plus `entry`,
   `goal`, an authored `critical_path[]`, and `beats[]` binding every
   place-bound quest beat to the node it happens in.
   - **A place carries no size class.** Its size is its box's `extent` in the
     site plan — your declaration — and nothing in the graph classifies it or
     refuses it. A road, a ledge one body wide climbing a whole cliff face and a
     hall are all just places with the boxes you draw for them.
   - **A `climb` is a way up a ladder or a vine the lower place hangs**: a hole
     through a floor, or a door high in a wall, with no treads and no sill. Its
     rise is the two floors' difference; a climb between two places on one
     plane is `DW0992` — a doorway called a climb. A climb **inside one place**
     is not a graph edge: it is the piece's own, two spaces and a `climb`
     contract edge between them (step 9).
   - **`"reached": false` declares scenery** — a place built to be seen and
     never entered, a tree's crown over a treehouse. The checks confirm it both
     ways: the closure must not reach it (`DW0816`) and no body may get into it
     in the built world (`DW0837`); a `vision` edge is how a place is seen.
     Scenery still owns its outside and stitches to the ground, owes no node
     anchor and no play light, and its piece is judged sealed. Absent means
     reached.
   - Every seam `opening` that names a standard names an entry in the **metrics
     table**. `delvec metrics` prints it — 223 lines of JSON on stdout, and a
     summary plus its binding counts on stderr, so read them separately:
     `delvec metrics > table.json`. **Write the BARE name** — `"opening":
     "arch"` — and a name the table does not define is `DW0812`, which lists
     the defined set. The table's own JSON keys carry a namespace the document
     never writes: the entry is at `building["opening.arch"]`, and the compiler
     puts the `opening.` in front of what you wrote to look it up, so
     `"opening": "opening.arch"` is `DW0812` and not a synonym. Strip the
     prefix when you read a key out of `table.json`.
   - The graph is checked as a graph, cheaply, before geometry exists to make
     it expensive: every place reachable under gating (`DW0816`), the authored
     critical path actually a quest-legal path (`DW0817`), no one-way edge that
     strands a body (`DW0819`), no "shortcut" that closes no loop (`DW0820`).
   - `intent` is a free label no check keys on — write what the place is *for*.

3. **`site-plan.json`** — the geometric embedding of that graph. `region` (the
   whole map's one box, in world coordinates), `datums` (named ground planes),
   **`fill`** (what the land is), **one `boxes[]` entry per node**, **one
   `seams[]` entry per traversal edge**, `volumes[]` for mass the whole owns
   (the mountain a cave is inside), a `sightlines[]` entry per `vision` edge,
   optional `views[]` to judge the silhouette from, `identities[]` binding the
   plan back to the brief's facts, and an optional `max_drop`.
   - **Extent flows down.** The region comes from the brief and the boxes
     partition it. A box is never grounds to grow the region (`DW0826`): shrink
     or move the box, or change the brief's fact and re-derive, visibly.
   - **`fill` is required, with no default**: what every cell no place claims
     and no volume covers becomes. `{"kind": "solid", "block": …}` is an
     enclosed site whose places are carved out of rock; `{"kind": "open",
     "terrain": …, "surface": …, "below": …}` is ground under sky — the
     `surface` block at the terrain's height, `below` under it, air above.
     `terrain` is `{"kind": "flat", "datum": …}` (the datum is the terrain's
     walk plane) or `{"kind": "heightmap", "heightmap": …, "base_y": …,
     "range": …}` — a greyscale PNG in the campaign, exactly the region's
     `x × z` pixels, each pixel the surface `y` `base_y + value × range / 255`.
     A plan without `fill` does not parse (`DW0100`); a heightmap of the wrong
     size or outside the region is `DW0826`. On an `open` site, ground no
     place claims is walkable commons.
   - **A box says what it is; a seam says where two boxes meet; the engine
     derives where it stands.** A box is a cuboid,
     `{node, extent: [dx, dz], floor, ceiling, base?, roof?, min?}`. `extent`
     is the play space a body stands in, any whole number of blocks; `floor`
     is `{"datum": …}` or `{"y": …}`; `ceiling` is `{"clearance": n}` (a lid
     `n` over the floor) or `{"open": n}` (sky-open: exactly `n` courses of air
     over the walk plane and nothing above them). Write `min` on **one** box
     (the entry) to say where the whole stands in the region; write it on no
     other box unless you mean to assert its corner, because a pin that
     disagrees with the seams is `DW0883`.
   - **A place owns its outside.** Its claim is its footprint grown by a
     one-cell ring — where its walls stand — from its bottom up to its open
     top, its lid or its roof zone; two places conflict only where their claims
     overlap (`DW0827`). `base` says where the bottom is. `"ground"` (the
     default) reaches down to the terrain, and the whole hands the place that
     ground and continues the terrain to the plot's edge. `{"aloft": n}` hangs
     the place — a gantry, a treehouse platform, a bridge's deck — its claim
     stopping `n` courses under its floor course, handed no ground; terrain
     reaching into that claim is `DW0990`, and the remedy is `"ground"`, a
     higher floor, or lower terrain. A place hung over an `open` place stands
     in its sky: a climb up into it is a hole through its floor course, which
     the lower place's headroom reaches when its top is one course under it
     (one short is `DW0828`, naming the gap).
   - **`roof: {courses, eaves}`** on a roofed box reserves the roof zone the
     place's piece draws: the shell footprint grown by `eaves`, from the
     ceiling course up `courses` courses. A roof on an `open` box, or one
     rising into another place's play space or floor course, is `DW0988`.
     Choose both numbers against
     `$DELVEWRIGHT_ENGINE/docs/reference/roof-and-facade-craft.md`.
   - **A seam is `{edge, face, form, opening | contact, at?, meets?,
     stair_in?}`.** `face` is the side of the edge's `a` box the crossing is
     on. The engine puts `b` one cell beyond that face — the wall — and the
     crossing in the **middle** of both faces. When the door is not in the
     middle, say where: `at` is cells from `a`'s low corner along the face,
     `meets` cells from `b`'s (an integer on a wall face; `[dx, dz]` through a
     floor or ceiling). A seam that closes a loop places nothing: both boxes
     already stand, and the engine checks that its cells are the same seen
     from either side (`DW0828`).
   - **`form` is required**: what the crossing is, in a few words — "a wooden
     arch bridge, 3 wide", "a ladder up through the hall's floor". Both places
     the seam joins are handed it, so each designs its side knowing what meets
     it. A connector that is itself a structure — a bridge, a long stair over a
     gap — is a place of its own with its own box and piece, and its
     neighbours give it a landing or an opening at each seam.
   - **Nothing about a sill, a rise or a wall thickness is written anywhere.**
     The sill is the higher of the two floors; the rise is their difference;
     the wall is the one cell the packing leaves between neighbours.
   - **A seam is one of two kinds, and both or neither is `DW0876`.** Write
     `opening` for a PORTAL — a doorway whose every cell the built world must
     have open (`DW0829`, `DW0836`), and that must lead a body through once its
     bar is open (`DW0986`). The opening is a standard the table names
     (`"opening": "arch"`) or a size the seam declares itself, `{"width": w,
     "height": h}` on the face's two in-plane axes — a one-cell rope-bridge end
     is as legal as a gateway. `DW0829` refuses either when it does not fit
     the shared face or its sill cannot be reached. Write `contact` for a
     FRONT — two places that simply meet, along a span of the face they share:
     `{"edge": …, "face": …, "form": …, "at": <cells>, "contact": {"extent":
     [u, v]}}` — `at` is the seam offset above (one integer on a wall face,
     `[dx, dz]` through a floor or ceiling) and `extent` is the span `[u, v]`
     on the face's own two in-plane axes, anchored there. Omit `extent` to run
     the span from `at` to the far edge of the face on both axes.
     - A contact means **continuous ground**: no wall along the span, no frame,
       no sill, and crossing legitimate anywhere along it the step rule admits.
       Its width is yours: a front one cell wide is as legal as one fifty-five
       wide.
     - A contact carries `walk` or `drop` only. A rim falling to a lower court is
       a real broad hand-off; a stair, a barred door and a sightline are not
       things a front can be (`DW0876`).
     - The engine MEASURES which columns of the span a body crosses, over the
       built bytes, and refuses a front nothing can cross (`DW0877`). Saying the
       face is fine is not a declaration it accepts.
   - A stair's rise is not authored — it is the difference between the two
     floors the plan already chose — and its `stair_in` names which box pays for
     the run. `DW0830` refuses a stair seam with no `stair_in`, one between two
     floors on one plane, and one hosted in the HIGHER place: treads rise off a
     walk plane, so `stair_in` is always the LOWER place.
     - **A run no standard pitch fits is a warning, never a refusal**: the
       stand-in the derivation masses an undetailed host with cannot lay its
       treads, so until the host is detailed the place above is unreached
       (`DW0837`). The piece you detail into the host at step 9 carries its own
       stair, judged over its bytes. For a standard stand-in stair, mind that
       the run is spent on ONE axis and the seam's face picks which: on a wall
       face the host affords its extent along that face's normal — an east or
       west face spends x, a north or south face spends z — so a 4 × 40 host
       with the stair on its east face affords **4**, and `DW0830` says so in
       those words ("affords 4 on x"). Only a seam through a floor or ceiling
       gets the host's longer horizontal axis.
   - **A drop's depth is yours, under two caps.** It may never be deeper than a
     body survives unarmoured, and when the plan declares `max_drop` — your own
     policy, in blocks — no drop seam may be deeper than that (`DW0831`
     refuses both). Absent `max_drop`, only the survivable fall applies.
   - `delvec validate` prints every box's derived corner and which seam placed
     it. Read your `volumes[]`, `sightlines[]` and `views[]` against that
     output — they are still world coordinates.
   - **A sightline or a view is only as long as the served view distance**:
     `16 × world.view_distance` blocks, 160 at the floor, and the build refuses
     a longer one. A vista
     longer than that declares the distance it needs (the refusal names the
     fewest chunks), because what its far end looks at is never sent to a
     client standing at its near end.

**A site-plan campaign has one area, `area/site`.** Quests, NPCs and waves name
it; `world.json`'s `areas[]` is empty, and declaring both authorities is
`DW0839`.

**The anchors are synthesized**, plus whatever a node's `stations[]` declares —
there are no prefabs to read anchor names out of:

| Anchor | Where |
| --- | --- |
| `spawn` | the entry node |
| `anchor/node-<place>` | the floor centre of each reached place — where NPCs, waves and `reach-anchor` objectives go |
| `anchor/seam-<edge>` | the gate region over a `barred` seam: what `open-gate` or a `shortcut` names |
| `anchor/unlock-<edge>` | the far-side affordance of a one-sided `barred` seam, where a `shortcut`'s `unlock` stands. Present only when `opens_from` is `a` or `b` |
| a `stations[]` entry's `anchor` | a station a layout-graph node declares; it may not take a derived name |

`<place>` and `<edge>` are the part of the id after the `/` — `node/near-hall`
becomes `anchor/node-near-hall`. A `barred` way declares a non-empty `gating` —
one with none is passable from world load and is refused (`DW0818`) — and is
opened by an `open-gate` or `shortcut` naming its own `anchor/seam-<edge>`; a
sealed door a player can push on owes an answer (`DW0429`) — a `use` trigger
anchored on the gate.

**The numbers the whole thing is built to are provisional** until the metrics
gym has been walked, and every build says so (`DW0813`). That is the gym's
second job: `delvec metrics --gym <dir>` builds a site-plan campaign out of the
table itself — a spine of bays chained by one seam per standard opening, two
climbs to one rise whose hosts differ only in the run they afford (so one takes
the gentlest standard pitch and the other the steepest), and a designed fall
with a stair back out of it. It reports what the table defines that it could
not instantiate (`DW0840`) — read that line, not just the green.

**A place's own sky** (spec-0080). A campaign declares its skies once, in
`world.atmospheres[]`; a place carries one from the first tick
(`areas[].atmosphere`, `boxes[].atmosphere`); a beat repaints with
`set-atmosphere`, naming a `place` (the tool hands its cells) or a `region` you
size yourself, never both (`DW0929`). An atmosphere nothing carries and nothing
paints is refused (`DW0930`), and so is an attribute the pinned game does not
accept on it (`DW0928`, naming the nearest ids). The sun, moon and stars keep
the overworld's course whatever a place says: an atmosphere colours the sky,
the fog and the air, and never moves a body in it. **Fog and sky are blended by the client over twelve blocks round the
camera, up and down included**, so a carried place is painted that far past its
own bounds and the sky is whole only where every cell within twelve blocks is
painted: the first twelve blocks past the line are a gradient, and a place
narrower than the blend never shows its fog at all. Every build prints
`atmosphere reach: <place> — W of E standing eye(s) read <biome> whole` per
carried place (a box's eyes stand over its floor, an area's wherever the party
can stand in it) — read it: where the sky's look matters, most eyes should
read it whole, and when few do, widen the place. Two carried places whose
paint meets under different atmospheres are refused at the build (`DW0929`):
leave a gap between them, or carry one atmosphere across both. A `region`
repaint is painted exactly as sized, so size it past the blend the same way.
