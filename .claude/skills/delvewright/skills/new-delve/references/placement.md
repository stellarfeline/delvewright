# Step 2 — placement: the three site-plan documents

## Contents

- [The order, and the worked example](#the-order-and-the-worked-example)
- [The map's reference first](#the-maps-reference-first)
- [The three documents](#the-three-documents)
- [One area, and anchors nobody authored](#one-area-and-anchors-nobody-authored)
- [The numbers are provisional](#the-numbers-are-provisional)
- [A place's own sky](#a-places-own-sky)

**The map is a site plan, whatever its size** (*The map is a site plan* on the
page). A map that is one building is a plan of one box and no seam; a town is
many boxes and the seams that join them. The three documents below are the same
in both cases, and nothing on this page asks which one you have.

## The order, and the worked example

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

## The map's reference first

**Before any of them: the whole map gets a reference of its own.** A composition
written without one is free invention with no criterion. On path A of Init I7 it is already in `design/reference/` — read it. On path B, draw it now; the
form and the commands are in *Reference: drawing the map's reference*.

## The three documents

1. **`geometry-brief.json`** — the whole's written design reduced to *numbers*:
   `facts[]` of `{id, value, unit?, note}`. A fact is a number with a name.
   Write the numbers the design actually commits to — how far across the site
   is, how tall the thing the campaign is named after stands, how far the
   approach runs. Reference imagery is style authority and never dimensional
   authority: an identity binds to a number, never to a picture.

2. **`layout-graph.json`** — the space as a graph, **before any coordinate
   exists**. `nodes[]` are places (`{id, intent, stations?, note?, reached?}`);
   `edges[]` are connections (`walk | stair | climb | drop | carry | vision`,
   with `gating`, `one_way`, `shortcut`). **A connection the story shuts
   carries `gating`** — `{flags?, quest?, opens_from?}`, naming what opens it
   — and the world builds it shut: a bar stands in its seam until the quest
   layer opens `anchor/seam-<edge>`. A door shut by bars is a `walk` with a
   `gating`; a stair, a ladder or a shaft the story opens later is a gated
   `stair`, `climb` or `drop`. `opens_from` (`a`, `b`, default either) names
   the side a one-sided way opens from. Plus `entry`,
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
       a real broad hand-off; a stair, a climb and a sightline are not things a
       front can be (`DW0876`), and a front cannot carry a `gating`: a bar has no
       opening to stand in.
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

## One area, and anchors nobody authored

**The campaign has one area, `area/site`.** Quests, NPCs and waves name it;
`world.json`'s `areas[]` is empty, and a plan beside a non-empty `areas[]` is
`DW0839`.

**The anchors are synthesized**, plus whatever a node's `stations[]` declares —
there are no prefabs to read anchor names out of:

| Anchor | Where |
| --- | --- |
| `spawn` | the entry node |
| `anchor/node-<place>` | the floor centre of each reached place — where NPCs, waves and `reach-anchor` objectives go |
| `anchor/seam-<edge>` | the gate region over a gated connection's bar: what `open-gate`, a `shortcut` or a `timed-gate` names. The bar is the opening in a wall (iron bars), the hole through a floor for a drop or a stair (a closed iron trapdoor in the hole's upper half), the course above the hole for a ladder through a floor (a closed iron trapdoor lying on the floor over the hole). A trapdoor opens on a redstone signal, so keep every lever, button, plate or other source off its six neighbours and off the solid blocks beside it |
| `anchor/unlock-<edge>` | the far-side affordance of a one-sided gated connection, where a `shortcut`'s `unlock` stands. Present only when `gating.opens_from` is `a` or `b` |
| a `stations[]` entry's `anchor` | a station a layout-graph node declares; it may not take a derived name |

`<place>` and `<edge>` are the part of the id after the `/` — `node/near-hall`
becomes `anchor/node-near-hall`. A `gating` names a flag or a quest — one that
names neither is passable from world load and is refused (`DW0818`) — and its
quest layer opens the way with an `open-gate` at the beat the gating names, a
`shortcut`, or a `timed-gate`, naming its own `anchor/seam-<edge>`; a gated way
nothing opens is refused as a wall carrying a claim about order; a
sealed door a player can push on owes an answer (`DW0429`) — a `use` trigger
anchored on the gate.

## The numbers are provisional

**The numbers the whole thing is built to are provisional** until the metrics
gym has been walked, and every build says so (`DW0813`). That is the gym's
second job: `delvec metrics --gym <dir>` builds a site-plan campaign out of the
table itself — a spine of bays chained by one seam per standard opening, two
climbs to one rise whose hosts differ only in the run they afford (so one takes
the gentlest standard pitch and the other the steepest), and a designed fall
with a stair back out of it. It reports what the table defines that it could
not instantiate (`DW0840`) — read that line, not just the green.

## A place's own sky

**A place's own sky** (spec-0080). A campaign declares its skies once, in
`world.atmospheres[]`; a place carries one from the first tick
(`boxes[].atmosphere`); a beat repaints with
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
