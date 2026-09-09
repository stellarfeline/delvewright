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
   anchor carrying `"role": "entry"` in the piece's metadata, or — for pieces
   admitted before the role existed — an anchor literally named `spawn` or
   `entry`. **Very few pieces have one**, and a pool usually holds exactly one
   that does, so a multi-area campaign is a constraint on which piece each area
   may bind rather than a free narrative move. How few is a fact about the
   library and not about this page, so read it rather than taking a number from
   here — the library is a separate artifact on its own cadence:

```sh
python3 - <<'EOF'
import json, glob, os
LIB = os.environ["DELVEWRIGHT_PREFABS"]
n = 0
for f in sorted(glob.glob(f"{LIB}/*.json")):
    if os.path.basename(f) == "pools.json": continue
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

**Three ways to design against that, and the second is the one this page used to
omit.** Design against the entry member's anchors. Or *require* the anchor you
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
  you need* at that point. That is the section's second entrance; step 13.3 is
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
   exists**. `nodes[]` are places (`{id, intent, size_class | way_class, note?}`);
   `edges[]` are connections (`walk | stair | drop | barred | vision`, with
   `gating`, `one_way`, `shortcut`, `opens_from`). Plus `entry`, `goal`, an
   authored `critical_path[]`, and `beats[]` binding every place-bound quest beat
   to the node it happens in.
   - **A place is classified exactly once, and there are two vocabularies.**
     `size_class` is a rung of the size ladder and bounds the footprint on BOTH
     horizontal axes — that is what a room, a hall or an arena is. `way_class` is
     for a place bounded in one axis and free in the other: a road, a causeway, a
     corridor, a duct. Write a way when the shape is a ROUTE — a cut ledge one
     body wide climbing a whole cliff face is 4 by 90, and no rung admits that,
     because a class spanning 4..90 on an axis has stopped classifying. Declaring
     both, or neither, is `DW0875`.
     - A way class bounds the **cross-section** only. There is no length
       standard and there never will be one: the run is your plan's business, and
       all the engine asks is that the box's longer extent EXCEED the class's
       widest cross-section (`DW0832`). A square box can never be a way, which is
       the point — it is a room.
   - `size_class`, `way_class` and every seam `opening` name an entry in the
     **metrics table**. `delvec metrics` prints it — 341 lines of JSON on stdout,
     and a summary plus its binding counts on stderr, so read them separately:
     `delvec metrics > table.json`. **Write the BARE name** — `"size_class":
     "hall"`, `"way_class": "road"`, `"opening": "arch"` — and a name the table
     does not define is `DW0812`, which lists the defined set for that kind.
     The table's own JSON keys carry a namespace the document never writes:
     the entry is at `building["size-class.hall"]`, and the compiler puts the
     `size-class.` / `way-class.` / `opening.` in front of what you wrote to
     look it up, so `"size_class": "size-class.hall"` is `DW0812` and not a
     synonym. Strip the prefix when you read a key out of `table.json`.
   - The graph is checked as a graph, cheaply, before geometry exists to make
     it expensive: every place reachable under gating (`DW0816`), the authored
     critical path actually a quest-legal path (`DW0817`), no one-way edge that
     strands a body (`DW0819`), no "shortcut" that closes no loop (`DW0820`).
   - `intent` is a free label no check keys on — write what the place is *for*.

3. **`site-plan.json`** — the geometric embedding of that graph. `region` (the
   whole map's one box, in world coordinates), `datums` (named ground planes),
   **one `boxes[]` entry per node**, **one `seams[]` entry per traversal edge**,
   `volumes[]` for mass the whole owns (the mountain a cave is inside), a
   `sightlines[]` entry per `vision` edge, optional `views[]` for the walk to
   judge the silhouette from, and `identities[]` binding the plan back to the
   brief's facts.
   - **Extent flows down.** The region comes from the brief and the boxes
     partition it. A box is never grounds to grow the region (`DW0826`): shrink
     or move the box, or change the brief's fact and re-derive, visibly.
   - **A box says what it is; a seam says where two boxes meet; the engine
     derives the grid.** A box is `{node, extent, floor, ceiling}` — `extent`
     is the interior a body stands in, on the kit grid. Write `min` on **one**
     box (the entry) to say where the whole stands in the region; write it on
     no other box unless you mean to assert its corner, because a pin that
     disagrees with the seams is `DW0883`.
   - **A seam is `{edge, face, opening | contact, at?, meets?, stair_in?}`.**
     `face` is the side of the edge's `a` box the crossing is on. The engine
     puts `b` one cell beyond that face — the wall — and the crossing in the
     **middle** of both faces. When the door is not in the middle, say where:
     `at` is cells from `a`'s low corner along the face, `meets` cells from
     `b`'s (an integer on a wall face; `[dx, dz]` through a floor or ceiling).
     A seam that closes a loop places nothing: both boxes already stand, and
     the engine checks that its cells are the same seen from either side
     (`DW0828`).
   - **Nothing about a sill, a rise or a wall thickness is written anywhere.**
     The sill is the higher of the two floors; the rise is their difference;
     the wall is the one cell the packing leaves between neighbours.
   - **A seam is one of two kinds, and both or neither is `DW0876`.** Write
     `opening` for a PORTAL — a doorway at a standard the table names, whose
     every cell the built world must have open (`DW0829`, `DW0836`). Write
     `contact` for a FRONT — two places that simply meet, along a span of the
     face they share:
     `{"edge": …, "face": …, "at": <cells>, "contact": {"extent": [u, v]}}` —
     `at` is the seam offset above (one integer on a wall face, `[dx, dz]`
     through a floor or ceiling) and `extent` is the span `[u, v]` on the
     face's own two in-plane axes, anchored there. Omit `extent` to run the
     span from `at` to the far edge of the face on both axes.
     - A contact means **continuous ground**: no wall along the span, no frame,
       no sill, and crossing legitimate anywhere along it the step rule admits.
       Do not reach for a wide `opening` to spell a front — there is no standard
       the width of your courtyard and there is not going to be one, because a
       front's width is a fact of your two boxes and a table that enumerated it
       would gain an entry per campaign.
     - A contact must be **wider than the broadest standard opening**. Anything
       narrower could have been a portal, and is refused as one (`DW0876`).
     - A contact carries `walk` or `drop` only. A rim falling to a lower court is
       a real broad hand-off; a stair, a barred door and a sightline are not
       things a front can be.
     - The engine MEASURES which columns of the span a body crosses, over the
       built bytes, and refuses a front nothing can cross (`DW0877`). Saying the
       face is fine is not a declaration it accepts.
   - A stair's rise is not authored — it is the difference between the two
     floors the plan already chose — and its `stair_in` names which box pays for
     the run (`DW0830`). Treads rise off a walk plane, so `stair_in` is always
     the LOWER place.
     - **The run is spent on ONE axis, and the seam's face picks which.** For
       a seam on a wall face the host affords its extent along that face's
       normal — an east or west face spends x, a north or south face spends z
       — so a 4 × 40 host with the stair on its east face affords **4**, and
       `DW0830` says so in those words ("affords 4 on x"). Only a seam
       through a floor or ceiling gets the host's longer horizontal axis.
       Give the host its length on the axis the face points along, or host
       the stair in the other place.
   - `delvec validate` prints every box's derived corner and which seam placed
     it. Read your `volumes[]`, `sightlines[]` and `views[]` against that
     output — they are still world coordinates.

**A site-plan campaign has one area, `area/site`.** Quests, NPCs and waves name
it; `world.json`'s `areas[]` is empty, and declaring both authorities is
`DW0839`.

**The anchors are synthesized, and these are the only names there are** — there
are no prefabs to read anchor names out of:

| Anchor | Where |
| --- | --- |
| `spawn` | the entry node |
| `anchor/node-<place>` | the floor centre of each place — where NPCs, waves and `reach-anchor` objectives go |
| `anchor/seam-<edge>` | the gate region over a `barred` seam: what `open-gate` or a `shortcut` names |
| `anchor/unlock-<edge>` | the far-side affordance of a one-sided `barred` seam, where a `shortcut`'s `unlock` stands. Present only when `opens_from` is `a` or `b` |

`<place>` and `<edge>` are the part of the id after the `/` — `node/near-hall`
becomes `anchor/node-near-hall`. Every barred way must be opened by something
naming its own seam (`DW0818`), and a sealed door a player can push on owes an
answer (`DW0429`) — a `use` trigger anchored on the gate.

**The numbers the whole thing is built to are provisional** until the metrics
gym has been walked, and every build says so (`DW0813`). That is the gym's
second job: `delvec metrics --gym <dir>` builds a site-plan campaign out of the
table itself — one place per rung of the size-class ladder at each of its
bounds, one way per class at each width the kit grid lets a plan draw it at,
every standard opening, both stair pitches, a designed fall at the drop policy's
cap. It reports what the table defines that it could not instantiate
(`DW0840`) — read that line, not just the green.
