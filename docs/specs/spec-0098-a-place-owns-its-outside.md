# spec-0098: A place owns its outside — the frame is the shell, the roof is declared, and one place can be the whole site

- **Status**: Proposed
- **Ground**: spec-0050 §3 fixed the detail frame as the play space plus its
  floor course, and put every wall, every unshared shell face and every ceiling
  course with the whole. On the first campaign to detail a town the consequence
  arrived exactly as that sentence predicts: a detailed building still reads
  from outside as the stage-5 flat-topped stone-brick box, and an open place —
  a mud flat, a road, a breakwater — is still walled to its headroom by a shell
  no document can touch. `DW0821`'s remedy ("the detail pass will carve the
  vista") is unreachable, which makes that gate and this frame a pair defect.
  The draft ADR-0030 records the same root cause on two earlier campaigns: the
  site-plan path could not produce a designed exterior, so a creator abandoned
  the plan and shipped the site as one piece.
- **Refines**: spec-0050 §3 (the frame), §4 (the handing), §7.6 (`DW0821`'s
  pair). Nothing here revisits spec-0049's stage 5: the blockout stays derived
  and authored by no one.
- **DSL**: the next minor `dsl_version` (this spec states no version literal). The
  site plan's `boxes[]` gain `roof`, its `seams[]` gain `form` (§2c), and
  the plan gains a required `fill` — what every cell no place claims becomes
  (§2b) — whose `open` kind carries the site's **terrain**, a declared
  heightfield (§2c).
- **Diagnostics**: **DW0987**, **DW0988** and **DW0990**, all used below
  (DW0989 was allocated and is unused: §14, ruling 3); `DW0827`'s
  quantifier widens from play spaces to claims (§2 rule 3d); `DW0838` refuses
  a place leaking, and on an `open` site ground outside every claim is the
  commons (§2c, §14 ruling 2).
- **Decision record**: ADR-0032 (one map route).
- **Research**: rooflines and facades at Minecraft scale are researched, not
  invented — `docs/reference/roof-and-facade-craft.md` carries the record with
  every rule marked cited or authored. This spec consumes two of its findings
  (§3) and invents none.
- **The ruling this executes**: a wall exists only where something declares
  one; a site-plan box bounds where a design may draw; the engine never writes
  a wall, a fill or any block nothing declared — it only checks. Where this
  spec's first form let the engine keep a wall between two unconnected places,
  that is gone (§2 rule 3d).
- **The finding this answers**: two neighbouring places may each shape their
  own ground — a plinth beside a slope — and where they meet, a step or an open
  gap appears along the boundary that no check sees unless a route crosses it.
  One flat datum makes it worse. So the whole owns a terrain, **the ring of
  ground around every place is fixed by the whole and may not be written by the
  piece**, every plot is stitched into the map by construction (§2c), and
  undulating ground is in this design's core, not a follow-on.
- **Non-goals**: retiring `areas[]`; the per-piece drawing medium (ADR-0030's
  question, §9); dressing the declared terrain (trees, paths — stage 7, §10);
  jigsaw connectors.

## 1. The principle, in one sentence

**A wall exists only where a design declares one; the engine never writes a
wall, a fill or any block that nothing declared — it only checks.** A site-plan
box is a region that bounds where a design may draw, not a box wrapped in walls.
So a place owns everything a body can see of it from outside — the frame a
piece fills is the place's **claim**: its floor course, its play space, the
one-cell ring its walls *may* stand in, and, for a roofed place, its lid and the
roof zone the plan declares above it — minus the cells the ownership rule (§2)
awards to a neighbour. Nothing in the final world is the engine's: what a piece
does not draw inside its claim is air; what no place claims holds the ground or
terrain the plan's `volumes[]` declare there, else air; and a cell two places
would both draw with no rule to award it is a plan refusal, never an engine
arbitration and never an engine wall.

Everything spec-0050 bought is kept: the frame is still computed from the site
plan inside `Plan::build` and authored by no one; the piece must still be
exactly its shape (`DW0843`); the seams are still allocated by the plan and
proved over bytes (`DW0836`–`DW0838`, `DW0877`, `DW0986`); detail is still
per-place and partial. What moves is the boundary between the piece and the
whole, and it moves by one cell outward and a declared number of courses up.

## 2. The claim and the owner — the whole of the fabric split, as one rule

**A place's claim** is the set of cells it would write if nothing stood beside
it, computed from its resolved box (`PlacedBox`) alone:

| Cells | Open place | Roofed place |
|---|---|---|
| the ground under the plot, from the claim's **bottom** — `min(lowest terrain under the footprint and ring, floor) − 1` — up to the floor course (`y = floor − 1`); the whole hands it terrain-shaped, and the piece reshapes it inside the ring | yes | yes |
| the play space | yes | yes |
| the ring: the one-cell position on every side, from the claim's bottom to the top of the play space — its **ground cells are fixed** (§2 rule 0) and only what stands above them is the place's | yes | yes |
| the ceiling course over the shell footprint | — | yes |
| the roof zone: the shell footprint grown by `roof.eaves` on every horizontal side, from the ceiling course up `roof.courses` courses | — | when `roof` is declared |

So the claim of a place with no `roof` is its stage-5 shell exactly, and the
claim of an open place is that shell without its lid.

**The owner of a cell** is decided by this rule, in this order, over every
box of the plan — one function, `siteplan::claim::owner`, and every reader
below calls it:

0. A cell of the **fixed ground** is the whole's, written as the declared
   terrain and written by no piece (`DW0990`). The fixed ground is every ring
   column's cells at or below its ground height `G`: on an `open` site the
   terrain's height there; on a `solid` site the floor course, `floor − 1`.
   Where a seam crosses the ring at grade — its sill one course over that
   ground or less — `G` at the seam's columns is the sill minus one, flat
   across the opening's width, so neither side has to edit the ground a
   doorway, a stair's landing or a drop's brink stands on (§2c). A seam aloft
   fixes no earth under it (§14, correction 1).
1. A cell inside a place's **play space** is that place's. Nothing else may
   claim it (`DW0988`, `DW0827`).
2. A cell exactly **one** claim covers is that place's.
3. A cell several claims cover:
   a. if it lies in exactly one of those places' **floor course**, it is that
      place's — where boxes stack, the plane between them is the upper box's
      floor, which is spec-0050 §3's stacked-box sentence kept;
   b. else, if exactly **one** of those places is **roofed**, it is that
      place's — the wall between a house and the street is the house's
      facade, and the street has no wall of its own;
   c. else, if every seam the plan allocates between those two places across
      that plane names the same `a`, it is **`a`'s** — the connection between
      two yards, or between two rooms, is a designed thing, and the place the
      designer put first on it draws the line where they meet: the kerb, the
      hedge, the doorway's reveal, with the allocated crossing left open
      (`DW0836`, `DW0877`) and nothing else (`DW0838`);
   d. else the plan is **refused** (`DW0827`, whose claim this already is:
      *two owners for one block, and no rule to pick by*): two places standing
      exactly one cell apart with no connection between them, or whose
      connections disagree about which comes first, have both asked to draw
      the same wall. The remedy is the plan's — give them a connection (whose
      `a` draws the wall), stand them two or more cells apart (each then owns
      its own ring and the gap between is ground), or make them one place.
4. A cell no claim covers is **nobody's**: it holds what a `volumes[]` entry
   declares there if one covers it, and otherwise what the site's `fill`
   declares undeclared space to be (§2b) — rock in an enclosed site, ground and
   sky in an open one. The gap between two houses in a town is ground and sky,
   never a wall and never massing nobody declared.

**The frame** of a place is the bounding box of the cells it owns. Its piece is
exactly that size (`DW0843`, unchanged). Cells inside the frame the place does
not own are its **void cells**: the allocation hands them, the piece carries
`minecraft:structure_void` (or no block) at each of them, and `delvec detail`
writes them so after expansion — a derivation, never typed. A cell the game
does not place is a cell the whole's block shows through, which is vanilla's
own semantics for `structure_void`, and the assembled-world model reads it the
same way (§7).

**Every seam lies in a plane some place owns**, because rule 3 awards every
contested plane or refuses the plan. So every seam opening is the owning
piece's to cut, and a `barred` seam's shut state is the owning piece's to ship —
spec-0050 §3's floor-course row generalised to every plane. The gate region
`anchor/seam-<edge>` is therefore **owed** by the place that owns the plane
(spec-0050 §6's exception becomes the rule): its `anchors` binds it to a gate
anchor of the piece whose region is exactly the allocated cells and whose
declared `block` is what the campaign's `close-gate` writes back — the piece's
own bar, never the derivation's constant. The derivation writes no opening, no
frame ring and no bar in the final world.

**What the whole owns, enumerated** — and none of it is a block nothing
declared: the plan's `volumes[]`, filled as declared; the region and its
surround (declared by `horizon`); the synthesized vocabulary — `spawn`,
`anchor/node-…`, `anchor/unlock-…`, every station stand-in — placed off the
plan whether or not a piece stands there; the review stand-ins of §8, which
never ship; and every proof: the seam battery, the reach proof, the
unallocated-crossing sweep, the sightlines, the identities, the pacing, the
exposure ledger (`DW0885`), the light measurement, the danger rule (`DW0891`)
and the stranding proof (`DW0921`). Not one proof changes, because every one of
them reads bytes against the plan and never asks who wrote the bytes.

**Unplanned crossings stay refused without any engine wall.** `DW0838` deletes
every allocated opening and asks whether two places are still walk-connected
over the bytes; a gap of ground that joins two places the graph does not connect
is exactly what it refuses when the walk does not pass through the commons,
naming the pair and a witness cell. Its remedies are each reachable inside an
authored document: the place **closes its wall** on the ring it owns, or the
plan **allocates a seam** between the two. **On an `open` site, ground no place
claims is the commons** (§14, ruling 2): ordinary walkable ground every
place may open onto through its own openings, with no declaration — a narrow
gap between two houses is simply ground, and two places that both open onto it
are joined through it.

## 2b. What undeclared space becomes is declared, not picked by the engine

Route B was first built for a site a player is always inside — a dungeon, a
cave — where only the interior matters and everything undeclared is rock. A town
is the other kind: most of it is air, and space no design declares is open
ground under sky. Both are legitimate, and which one a site is cannot be the
engine's guess (the general-engine rule). So the site plan **declares** it, in
one required field with no default:

| `fill` | What every cell no place claims and no volume covers holds |
|---|---|
| `{"kind": "solid", "block": <state>}` | the declared block — the enclosed-site case, which a region-wide `massif` gives today |
| `{"kind": "open", "terrain": <terrain>, "surface": <state>, "below": <state>}` | at the terrain's height the `surface` block, under it `below`, above it air — a natural ground surface under sky; `terrain` is §2c's heightfield |

**Required, no default — because either default is a per-case judgement the
engine would be making.** A cave defaulted to open ground stands its rooms in
daylight with air between them; a town defaulted to solid buries its streets in
rock. Neither is the "safe" path: each is wrong for half the sites, and the
author knows which half this is. A plan without `fill` does not parse
(`DW0100`), which is the refusal where entered; a plan with it has said, once,
what kind of site it is. `fill` is **per site**; **per region** it is overridden
by `volumes[]` exactly as today — a `massif` on an open site is the solid hill
under the streets, a `clearance` in a solid site is a sky well, a `ground`
volume is a ground plane at another datum. A town by the sea is `open` over a
terrain that rises from the sea to its terraces, with a `massif` where the rock
shows. Every block is declared: a
volume takes the block of its kind from `fill` (`massif` → `solid.block`,
`ground` → `surface`/`below`) unless it names its own.

The derivation's `MASSIF` and `GROUND` palette constants stop being written
into the world; what remains of the blockout palette is the stand-in shell
(§8), which never ships.

## 2c. The whole owns the terrain, and every place is stitched to it

**The terrain is a declared heightfield**, not one flat datum:
`fill.open.terrain` is `{"flat": <datum id>}` — one height everywhere — or
`{"heightmap": <png in the campaign>, "base_y": <y>, "range": <blocks>}` — a
greyscale image exactly the region's `x × z` pixels, each pixel the ground's
top `y` as `base_y + value × range / 255`, the same input every terrain tool
takes and the creator's own artifact (drawn, or generated by a tool whose seed
is written down). Research: `docs/reference/roof-and-facade-craft.md` §7.
Volumes still shape solid masses on and under it.

**Landscape first.** The terrain exists at stage 4 beside the boxes, so the
plan is judged against it before any piece exists: the allocation hands each
place the terrain's height along every cell of its perimeter — the list, its
minimum and its maximum — and a box's `floor` is the designer's choice against
those numbers (a plinth stands above them, a sunken yard below, a cottage on a
slope at one and steps down at the other).

**Stitching is by construction: the ring of ground is fixed.** Every ring
column's cells at or below its ground height (§2 rule 0) are the whole's — the
terrain continued to the plot's edge, block for block, in the `surface` and
`below` blocks the site declared — and the allocation hands them to the piece
as fixed: their 3D coordinates and their material. The piece writes
`structure_void` there (as at every cell it does not own) and may not write a
block; a piece that does is refused at validation, where the piece's own bytes
are read beside `DW0987` (**`DW0990`**, first shape, naming the cell and the
terrain block it displaced). Inside the ring the place shapes its ground
freely from the handed initial ground — a plinth above it, a sunken yard below,
a slope that follows it — and its edge meets the fixed ring on the place's own
terms: within a block, or faced by a wall the piece drew. So two neighbouring
plots meet the same terrain along their shared ring, and the map's ground runs
unbroken through every plot edge whether or not either plot is detailed. That
is stronger than refusing a crack after the fact, and the crack check is kept
as the **measurement over bytes** that proves it held.

**The crack check, read off bytes.** Along every claim boundary — the fixed
ring against the plot inside it, and every seam opening against the ground it
stands on — the two standable surfaces either side differ by at most one block
(a step a body walks), **or** a solid face stands from the lower surface to the
higher (a retaining element the piece drew), **or** a seam across that boundary
declares the crossing. What is refused is **the crack**: air between the two
surfaces under the higher edge — an edge nothing holds up, Ulrich's tile gap in
blocks (**`DW0990`**, second shape, build tier in the stage-5 battery beside
`DW0836`, naming the boundary segment, both heights and both owners, read from
the assembled bytes with no knowledge of who wrote them, so a stand-in shell and
a piece are judged alike). Binding: fixed ring cells handed and examined,
boundary columns examined, cracks.

**Crossings over the ring.** The ring fixes ground cells only; everything above
the ground course — a doorway, a wall, a plinth's face, a gate's bar — belongs
to the owner §2 rule 3 names. A stair or a ramp that spans a height difference
is laid **inside** a place by its piece and arrives at the seam's opening; it
never edits the ring. Where a seam crosses the ring the whole derives the ring's
ground there from the seam's two floors rather than from the terrain — the
sill's course, flat across the opening — so a doorway stands on ground neither
side has to cut, a stair's landing meets its opening at grade, and a `drop`'s
brink is level; the fall itself is answered by the **higher** place's own face
inside its claim, which is the one policy about drops this engine has
(`DW0877`, `DW0837`, `DW0986`).

**Every seam declares its form, and both sides read it.** A seam carries
`form`, a short declared description of the crossing — "a wooden arch bridge, 3
wide", "a stone stair, down 4", "rope ladder, up 6". It is a creative judgement
the plan states once, on the seam (the site plan already owns the seam's kind
and cells), never player-facing and therefore never translated, and required:
a crossing whose two sides are designed apart is designed by both against the
same sentence. The handout (§4) gives it to **both** places the seam joins, so
each designs its side of the interface knowing what meets it.

**A connector that is itself a structure is a place.** A bridge across a gap, a
long stair down a cliff, a rope walk between two trees: each is a way-classed
place of its own (spec-0053) with its own piece, and its neighbours provide only
a landing or an opening at each seam. No new mechanism is needed: the bridge's
piece owns its deck, rails and supports like any place owns its outside, and the
seams at either end are ordinary seams.

**The faced cliff is already refused where it matters, by the proofs that read
reach.** A body can walk off a plinth's edge: onto a neighbouring place, which
is `DW0838` (a crossing nothing allocated) unless a seam declares it; or, on an
`open` site, onto the commons, which is ground like any other — the route
proofs over the assembled world walk it, and a pocket in it is `DW0921`'s.
`DW0838` refuses a place **leaking** (§14, ruling 2): *delete every
allocated opening and the commons, and no two places are walk-connected; no
stand-in — the engine's closed shell — reaches the commons; and on a `solid`
site, which has no commons, no place reaches ground outside every claim.*

**Her question, answered: there is no engine-generated connector between two
heights.** The whole declares the height difference — a seam's rise is
`floor(b) − floor(a)`, a terrain's slope is the heightmap — and the owning
piece draws what crosses it: the stair's treads, the ramp, the steps down the
plinth, the opening. The stage-5 stair (`blockout::tread`) is a review stand-in
laid in an unbound host and never ships, like every other stand-in (§8). Where a
piece draws no crossing, there is none, and the proofs say whether that is a
defect: a declared seam the bytes do not honour is `DW0836`/`DW0837`; a
crossing the bytes have and the graph does not is `DW0838`.

## 3. The roof is declared by the whole and drawn by the piece

`boxes[].roof: {courses, eaves}`, optional, refused on an open box
(`DW0988`). `courses` is how many courses the roof rises above the ceiling
course; `eaves` is how far the roof zone overhangs the shell footprint on each
horizontal side. Both are creative judgements the plan states; neither is
inferred. A roofed box with no `roof` has a flat lid one course thick, which is
the stage-5 ceiling course the piece now owns.

**Why the plan and not the piece.** The silhouette is the whole's authority
(ADR-0022), and the roof is most of a building's silhouette: a roof declared by
the part would let the part decide the skyline, and would reach the region, the
neighbours and the sightlines after the whole had been walked. Declared on the
box, the roof zone is massed at stage 5 — solid, in its own palette block, so a
walker sees the volume every building will occupy — and judged by the plan's
own rules: it must stand inside the region (`DW0826`, whose quantifier is "what
the plan places", and the roof zone is placed), and it may not rise into another
place (`DW0988`).

**The two cited findings this spec consumes**
(`docs/reference/roof-and-facade-craft.md`):

- *A roof that cuts straight to the wall looks unfinished; most roofs carry an
  eave or overhang.* Hence `eaves`, and hence the roof zone begins at the
  ceiling course rather than above it: the eaves course sits level with the
  wall's top and one cell beyond it, which is where the idiom puts it.
- *The three standard pitches are 63.4° (one course per column), 45° (stairs)
  and 22.5° (alternating slabs).* Hence `courses` is the plan's choice against
  a span the plan knows: a 45° gable over a shell `W` wide, eaves included,
  rises `⌈(W − 1) / 2⌉` courses; the research page tabulates it. The engine
  does not compute `courses` from a pitch, because the pitch is a judgement
  and the number is what the whole has to reserve.

**Eaves stop at a neighbour.** An eaves cell that lies in another place's play
space is not claimed — the overhang stops where the neighbour's wall begins,
as a real eave does — and the allocation prints where it stopped. An eaves cell
in another place's claim outside its play space is contested and goes to §2's
rule. The roof proper (the courses over the shell footprint) is never clipped:
a roof that rises into another place's floor course or play space has no room,
and is refused where it is declared (`DW0988`), naming both places and the
courses that collide. The remedy is the plan's — fewer courses, a taller
neighbour, or one place where there were two.

**Two roofed places exactly one cell apart** share the column over their party
wall through both roof zones; it goes to the place their connection names `a`
(§2 rule 3c), and with no connection the plan is refused (rule 3d). A row of
houses a gap apart share nothing: each owns its own ring and roof, and the gap
is ground and sky. A building whose roof should run unbroken over several rooms
is **one place** whose piece declares several spaces — which is also how §5
says a route-A site is written. Under every wall the ring's ground is the
whole's (§2 rule 0): a facade stands on the terrain, a party wall on the sill's
course where a door crosses it, and the piece's ownership begins one block up.

## 4. The handout: what the whole hands a place, produced by a tool and never typed

`delvec allocation <place>` (spec-0050 §4) becomes the **handout** the agent
that designs a place works from — one JSON document per place, derived from the
plan on every invocation, an input to nothing (`DW0842`–`DW0845`, `DW0987`,
`DW0990` and the battery recompute every obligation from the plan). It carries:

- **what is built here, and in what style**: the layout-graph node's `intent`,
  `note`, size or way class and stations; the detail plan's `palette`; the
  design record's style lines that bind the campaign (`design.json`).
- **the place's own concept reference**: the path of the approved image for
  this place under `design/places/<stem>/` and its `design.json` row. It is
  **generated at step 9, after the walk**, anchored on the whole's reference
  sheet and on this handout's position and ground — never before the blockout,
  because per-place imagery authored ahead of the whole is style authority only
  (ADR-0022), and a place's concept drawn against the plot it really has is the
  one that can be built.
- **where it stands in the whole**: the frame's world position; each neighbour
  by face with its kind, floor and roof; the terrain's height along the
  perimeter; the plan's `views[]` that see it, each with the `delvec snapshot
  --shot` that frames the plot in the stage-5 whole.
- **the whole's concept reference sheet**: the stage-2 sheet's approved images
  by path, from `design.json`.
- **its initial ground**: every fixed ring cell — `[x, y, z]` and block — and
  the terrain-shaped ground inside the ring as columns `{x, z, top, blocks[]}`
  the piece may reshape; the claim's bottom; the floor's piece-local `y`.
- **its seams and owed anchors**: every seam piece-local with its declared
  `form` (§2c), the place on its other side, face, cells,
  class, rise and the answering face class (spec-0050 §3's table), the ring's
  derived ground at each seam; every owed name, the seam gate regions now among
  them (§2); the frame's extent and its void cells, each with its owner.

`delvec detail` reads the same object and binds the same values under
`handed/` (`handed/datum-y`, `handed/seam/…`, `handed/roof/…`,
`handed/ground/min-y`, `handed/ground/max-y`), voids the fixed ring and every
other cell the place does not own after expansion, and runs every gate before
a file is written. The skill's step 9 opens with the handout and the concept
image, and types nothing from either.

## 5. Pure route A is the degenerate case, with no switch

A site plan with **one box** and **no seam**, whose claim is the region or
inside it, hands its one piece the whole site: walls, roof, ground and all. Its
layout graph has one node, which is also its entry, its goal and its whole
critical path; the places inside the building are the piece's **spaces**, as a
route-A piece already declares them (spec-0036), and the spots the quest layer
names are the node's **stations** (spec-0052), which the piece answers with its
own anchors through `anchors`. The walk record (`DW0841`) is still required
and judges a whole of one place and zero connections — its binding line says
so, and that zero is the honest count of a question a one-place whole does not
ask rather than a hatch.

**No threshold exists and none is computed**, because the question "one place
or many" is the layout graph's node count, which is the creator's design and
nothing else. What the engine states, per place, is what it already states:
the frame's extent and cell count (allocation), the templates the frame needs
(tiling at the vanilla 48-per-axis cap is packaging, `compiler.md`), and the
share of the world-build chain the piece costs (`DW0984`'s binding). A piece
too large for one template is tiled by the export; a world too large for one
tick is stepped by the emitter; neither is a threshold anyone chooses.

Demonstrated, not asserted (§11.6): a one-box campaign whose piece carries
three spaces and two stations builds green and its bytes are the piece's at
every cell of the region the piece covers.

## 5b. A town is scenes that meet by design

The direction this spec executes: **a scene keeps route A's freedom, and a map
is scenes composed the route-B way, joined by connections somebody designed.**
Route A's cost is a script that grows with the map; here each scene's program
is the size of that scene, and the whole is the plan. In this spec's terms:

- **A scene is a place and a piece.** A church is one roofed place, its program
  the church's own — nave, tower, porch and roof inside the frame the plan
  hands it, several spaces in one contract, every station it owes answered by
  its own anchors. Nothing in the program names another scene; what couples it
  to the whole is the handing (§4), and only that.
- **The same building twice is the same program twice.** A row of identical
  cottages is one program, included (`grammar.md`, document-level composition)
  by one stub per place that declares the handed names; each expansion differs
  only by what its place is handed — the door's cells, the void columns, the
  eaves the neighbour clipped — so the cottages are identical where the design
  says so and different exactly where their situation differs. A piece already
  in the library may also be bound to several places by several rows, with the
  same facing, and the whole proves each binding on its own seams.
- **A road is a scene too.** It is an open, way-classed place (spec-0053), and
  under this spec it owns its ring and its floor: its program draws the
  carriageway, the kerb, the verge and the lamp posts, and leaves its edge open
  where the design opens it. A building's facade onto the road is the
  building's (§2 rule 3b); the road's own edges toward other open ground are
  the road's where it is the connection's `a` (rule 3c). Where two roads or a
  road and a square run together, the connection is a **contact** — continuous
  ground along a span the designer allocated — and the whole proves a body
  really crosses it (`DW0877`) and crosses nowhere else (`DW0838`).
- **The designed connection is the layout graph, placed by the plan.** Which
  scenes touch, where the door is on the facade, how wide the mouth of the
  square is, which way a drop falls — these are the graph's edges and the
  plan's seams, authored as judgements. The engine never invents a connection:
  a way that exists because a wall came out low is refused, and a connection
  the graph declares that the pieces do not honour is refused. What the engine
  does mechanically is only the arithmetic — the corner of a box one cell
  beyond the face it hangs off, the sill, the rise.
- **Between the scenes** stands only what the plan declares: on an `open`
  site the terrain — one heightfield the whole owns, rising and falling as the
  design drew it — with every scene stitched to it along its boundary (§2c),
  and `volumes[]` where rock shows or sky is kept; on a `solid` site, rock. Never a wall and never a fill nothing declared. A gap
  between two houses too narrow for a road is ground and sky — the commons,
  which both houses may open onto (§14, ruling 2); a body crossing from
  one place into another through a wall, not through a seam or the commons,
  is `DW0838`. The declared
  ground is one surface until stage 7 dresses it, and §10 names a dressed
  volume as the follow-on.

## 6. The checks answer while the creator draws

Per piece, at `delvec detail` (and `delvec grammar expand` for the loop before
it), the engine already runs the contract gates — reachability of every
standable cell, the edge proofs per class, nothing out of walk, the light probe,
the admission audit — then the binding check (`DW0842`–`DW0845`, `DW0848`,
`DW0987`) on the row it would write, and then **builds the whole** and runs
the stage-5 battery over it, before any file is written. So every proof the
campaign will run on the piece's geometry runs at the verb, on the one body
rule (spec-0056), and a creator who re-implements walkability privately is
re-implementing an answer the verb gives. What the verb does **not** run is
enumerated so nobody expects it: the stranding proof (`DW0921`) and the danger
rule (`DW0891`) run at `build` over the campaign, because both need the
campaign's lethal volumes and gates; the exposure ledger (`DW0885`) runs at
`build`, because burial is a fact about neighbours. Bringing those three to the
verb is a follow-on, not a condition of this spec.

## 6b. Two defects of the paused campaign round, and what the frame does to them

Both are recorded against the content repository's `campaign/stranding` at
`6ddb2ca2` (GENERATION.md, round 7, engine findings 2 and 3).

**Two seams sharing a corner cell — dissolved by the frame.** The
Narrows is a strip with a contact along its whole west face and a contact across
its north end; under spec-0050's frame both answering layers lay *inside* the
play space (the piece's boundary cells, `x = x0` and `z = z0`), so the two
openings shared the room's corner cell, `contract-well-formed` refused a cell in
two openings, and conceding the corner to the room made `DW0844` refuse both
openings as not the plan's. Under this spec every seam is answered **at its own
party plane** — the west opening at `x = x0 − 1`, the north at `z = z0 − 1` —
and two party planes meet only at the ring's corner column, `(x0 − 1, z0 − 1)`,
which lies in neither seam's shared face (a shared face is the overlap of the two
play spaces' spans and never reaches a corner). The openings are disjoint by
construction; nothing is conceded and nothing re-refused. Both planes are owned
by a place (rule 3c: each contact names an `a`), so no engine wall stands in
either. Criterion 14 pins it.

**An all-open piece refused — answered by the author's declaration.** A
street, a mud field, a pavilion or a covered market honestly encloses nothing,
and `contract-closure` used to red a zero binding when no space is `enclosed`
or `open_top`. **The enclosed spaces are the author's declaration, and the
check confirms them** (§14, ruling 3): a piece declares zero or more
box-shaped `enclosed` (or `open_top`) spaces in its spatial contract, and
closure verifies exactly those. Zero declared passes and states `0 of N
space(s) declare an envelope closure examines; every space is open`. Nothing
infers an enclosure from the plan's `ceiling`, and a space declared `open`
under the piece's own blocks is a covered space, taken as declared.

**Also recorded there, a doc defect (finding 4)**: the allocation hands a seam
as its two corner cells, which the skill's detail reference calls "its cells";
the reference is corrected to the handing's own words with the pin bump.

## 7. The piece never writes a neighbour's cell

| Code | Rule |
|---|---|
| `DW0987` | **A piece paints a cell it does not own.** A bound piece's template holds a block other than `minecraft:structure_void` at a void cell of its frame — a neighbour's facade, the party wall a connection gave the other side, a clipped eave, a gap cell. Read off the piece's own `.nbt` at validation, where `DW0888` already opens it, and named per cell with the owner the plan awards it to. Air counts as painting: the game places a template's air, so an air cell over a neighbour's wall would carve it. Validation tier (exit 1). **Binding: bound pieces opened, void cells examined, painted.** |
| `DW0990` | **The plot does not stitch.** Two shapes of one claim — that a place meets the ground the whole gave it. *A piece writes a fixed ring cell*: its template holds a block (air included) at a cell of the fixed ground (§2 rule 0), read off the piece's own `.nbt` at validation beside `DW0987`, named per cell with the terrain block it displaced. *A crack*: along a claim boundary the two standable surfaces differ by more than one block with air under the higher edge and no seam declaring the crossing — measured over the assembled bytes in the stage-5 battery, naming the boundary segment, both heights and both owners. Declared build tier (the battery's); the first shape is raised at validation on `DW0886`'s precedent. **Binding: fixed ring cells handed and examined; boundary columns examined, cracks.** |
| `DW0827` (widened) | **Two owners for one block, and no rule to pick by.** Its claim is unchanged and its quantifier grows from play spaces to claims: two places whose claims share a cell that none of §2's rules 3a–3c awards — exactly one cell apart with no connection across that plane, or with connections that disagree about `a`. Named with both places, the shared cells and the three remedies (connect them, stand them apart, make them one). The engine neither arbitrates nor writes. Validation tier (exit 1). **Binding: box pairs compared, contested cells awarded.** |
| `DW0988` | **A roof the plan has no room for.** Two shapes of one claim, both read off the plan before any geometry: `roof` on a box whose `ceiling` is `open` — an open place has no lid to put a roof on; and a roof zone's course over the shell footprint lying in another place's play space or floor course — the stacked case, named with both places and the colliding courses. Eaves are not this refusal: they stop at a neighbour (§3). Validation tier (exit 1). **Binding: roofs declared, courses examined against places.** |

**The model reads `structure_void` as the game does.** `assembled::placed_blocks`
does not write a template cell that holds `structure_void`, exactly as it does
not write a cell the template omits — before this spec it wrote it as a block,
so a voided cell would have been modelled as passable while the server kept the
wall under it. Direction: a model more passable than the world can only let a
proof pass that the world would fail, so the correction can only turn a proof
red; it is held by a test.

## 8. The stage-5 blockout is a review stand-in, and a stand-in never ships

The whole must be walkable and reviewable before any piece exists (ADR-0022;
spec-0049 §5), and a half-detailed campaign must build and walk at every point
in between (spec-0050 §1). Both stand, under one reading the record already
carries: **the derivation's shells are stand-ins for pieces not yet drawn.**
They are written in the blockout's legibility palette — concrete floors, stone
bricks, the roof block — so that a walker sees massing and not a building, and
they stand only in the claim of an **unbound** place, where a piece will stand.
A bound place's claim is written by its piece and by nothing else.

- **a box**: the stand-in shell in the claim the place owns — a flat floor at
  the walk plane over the handed ground, the ring above its fixed ground cells,
  the lid — exactly where its piece will draw; every seam opening the place
  owns cut in it, its frame ring and its bar as now; nothing written in the
  fixed ground, in a neighbour's cells or in a gap. The fixed ring is written
  by the terrain pass, bound or not, so the plot's edge is the same ground at
  stage 5 as in the shipped world.
- **a roof**: for an unbound box declaring `roof`, the roof zone massed solid
  in the roof palette block, so the skyline is walked before it is drawn.
- **the gaps**: what `fill` and `volumes[]` declare, at stage 5 exactly as in
  the shipped world — a `solid` site is rock with its places carved out of it,
  which is how a cave reads today; an `open` site is its declared terrain —
  the heightfield laid in its surface and below blocks — with the stand-in
  shells standing on it under sky, each reading as a plinth or a pit against
  the ground around it, the roof massing showing the skyline. Both are walkable
  and both read as the kind of site they are before any piece exists, and the
  stitching check (§2c) runs over the stand-ins too: a solid ring is a faced
  edge, so the walk sees every box's relation to the ground and no crack.

**A stand-in never ships.** The derivation already counts them —
`blockout binding: … N box(es) (D detailed, U massed by the derivation)` —
and the fact `DW0821`'s promotion is keyed to (`detail::fully_detailed`) is
the same number reaching zero. The staging gate (`tools/creator/staging-gate.py`),
the one event between a build and a player, refuses a site-plan build whose
binding line counts any box massed by the derivation, naming each; its clause
today admits "any place detailed", and that is the loosened reading this spec
closes. A development build is unaffected: it walks and renders with stand-ins
at every stage, which is what they are for.

Nothing else in the derivation moves: openings are still cut last, stairwells
still measured over them, the stations still read off the laid mass.

## 9. The per-piece medium is not decided here

The frame is handed to a **piece** — frozen bytes with a contract — and the
engine consumes the object class, never the tool that made it (spec-0050 §1).
A grammar program bound by `delvec detail`, a computed program a generator
emits, a hand-admitted `.nbt` and ADR-0030's proposed drawing are four
producers of one object, and every one of them owns its outside under this
spec identically. Whether the drawing should replace the computed program as
the authored medium is ADR-0030's revisit question, decided on its own
evidence; this spec removes the one reason both earlier campaigns gave for
leaving the site plan, and takes no position on the medium beyond requiring
that its output carry a spatial contract.

## 10. The general-engine test, and the falsifiers carried forward

Nothing here is a house or a town: a *shell owned by what it encloses, a lid
declared by the whole, a party plane awarded by which side has walls* serves a
cave chamber in a massif (no roof, walls the piece lines), a ship's deck on an
open sea (an open place whose ring is its gunwale) and a terrace of shops
(roofed places whose connections say who draws each party wall) identically. The ownership rule
has three clauses and no table of kinds.

Falsifiers, each a brief this spec cannot state without a workaround and each
the trigger for a first-class surface rather than a hack:

- **The terrain wants dressing.** An `open` site's terrain has its shape
  (the heightfield) and one surface block; trees, paths, outcrops and the
  texture of a hillside are stage 7's edit script. The brief this cannot state
  is a dressed landscape between the scenes; The first campaign whose hillside or
  sea floor cannot be dressed that way is the brief for *a volume is detailed
  like a place* — the same `details[]` row naming a `volume`.
- **A roof that must run over two places.** The first brief that cannot make
  them one place is the brief for a shared roof zone, declared on the plan.
- **Eaves on one side only.** Clipping answers the neighbour case; a style
  that wants deep eaves at the front and none at the rear on open ground is
  the brief for per-face eaves.

## 11. Acceptance criteria

Machine-checkable. Each names its instrument and **what would make it vacuous**.

1. **The ownership rule is exhaustive and one-owner.** Over the blockout
   fixture, the gallery's site-plan overlay and a hand-built stacked pair, every
   cell of every claim has exactly one owner under `siteplan::claim::owner`,
   and the owned-cell sets of distinct places are disjoint; each of rules 3a–3d
   is reached by a named cell (a stacked floor, a facade onto an open place, a
   plane two yards' contact names `a` on, a plane nothing connects). Vacuous if
   the test enumerates fewer cells than the claims hold: it asserts the
   enumerated count equals the union's size. Instrument: `crates/dsl` tests.
2. **The frame is the shell.** For a roofed box with no `roof`, the frame is
   the play space grown one cell on every side; for an open box, the same minus
   the top course; for a box with `roof: {courses: c, eaves: e}`, the top rises
   `c` and the horizontal extent grows `e` beyond the shell. Vacuous if the
   assertion compares the frame to itself: the expected extents are literals
   derived in the test from the box's numbers. Instrument: `detailplan` tests.
3. **The derivation writes only stand-ins, only in unbound claims.** On the
   blockout fixture with one place bound, the derivation's fills cover no cell
   of that place's owned set and no cell no place claims (a gap cell holds its
   declared volume or nothing); with nothing bound, every fill lies in some
   place's owned set or in a declared volume. Vacuous if the bound place owns
   zero cells or the fixture has no gap: the test asserts both counts.
   Instrument: `tests/blockout.rs`.
4. **A detailed building carries its own outside.** The gallery's annex program
   paints its two free walls and a gabled roof over a declared `roof`; the
   assembled bytes at those cells are the program's blocks, not the blockout
   palette's; its two party planes with the halls are drawn by the side the gallery's connections name `a`; and the
   render from eye height outside shows the roofline (the proof renders, §12).
   Vacuous if the perturbation moves no byte: `tools/ci/check-gallery-coverage.py`
   binds `PlanBox.roof`, `Roof.courses` and `Roof.eaves`, and the gallery's
   perturbation of `courses` moves an emitted byte.
5. **An open place is open.** The gallery's yard owns its ring: its piece
   leaves the ring air on its free sides, its party cells toward the far hall
   are void, and a body on the yard sees sky and the far hall's wall — the
   `DW0885` ledger names the yard's free sides as shown. Vacuous if the ring is
   not in the frame: `DW0843` refuses the old 8×4×8 piece against the new
   frame, which the probe `a-yard-the-size-of-its-floor` pins.
6. **Route A is one box.** A one-box, zero-seam campaign built in test, whose
   piece carries three spaces and two stations, validates and builds green, its
   battery binding reading `1 box, 0 seams`, and every cell of the region the
   piece covers holds the piece's block. Vacuous if the piece is small: the test
   asserts the piece's extent equals the region's extent. Instrument:
   `tests/one_place_site.rs`.
7. **`DW0987` is reachable and bound.** A bound piece painting one void cell
   (stone, and separately air) is refused naming the cell and its owner; the
   same piece with `structure_void` there passes; the binding line states
   pieces opened and void cells examined. Vacuous if no void cell exists:
   the test asserts the examined count is non-zero. Gallery: the probe
   `a-wall-the-whole-already-owns` binds a generated piece that paints its
   party plane.
8. **`DW0988` is reachable in both shapes.** `roof` on an open box is refused;
   a roof over a box with another box stacked on it is refused naming both;
   a roof over a free-standing box passes; eaves reaching a neighbour's play
   space are clipped and printed, not refused. Gallery probes:
   `a-roof-over-the-open-sky` and `a-roof-under-the-hall`.
9. **The pair is whole.** On the blockout fixture, a sightline the loft's shell
   wall blocks at stage 5 (`DW0821` warning) clears when the loft — an open
   place — is bound to a piece whose ring is air. The remedy `DW0821` names is
   taken in test. Vacuous if the sightline was never blocked: the test asserts
   the warning first.
10. **The model skips `structure_void`.** A template with `structure_void` over
    a mass block leaves the mass block in the assembled model; the same template
    with air removes it. Instrument: `assembled` unit test.
11. **Binding nothing moves no byte.** A campaign whose detail plan binds no
    place derives the same mass as the same campaign with no detail plan, and
    the derivation is deterministic and seedless. Instruments: `tests/detail.rs`
    and `tests/blockout.rs`.
12. **Every proof still holds over a bound outside.** The stage-5 battery,
    `DW0885`, `DW0210`, `DW0891` and `DW0921` run on the gallery's site-plan
    overlay with the yard and the annex bound, exit 0, with their binding lines
    non-zero where they were non-zero before. Instrument: the gallery build.
13. **The catalog, the skill and the queue.** `tools/ci/check-dw-codes.py` green
    with zero new allowlist entries; `compiler.md`, `blockout.md`, `detail.md`,
    `siteplan.md` and `tools.md` carry the rows and the handing; the skill's
    step-9 text for the pin bump is in the scratch file the implementation names;
    `docs/demo-levels.md` carries the demo row.
14. **Two seams at a corner bind.** A fixture place with a contact along one
    face and a contact across the adjacent face, meeting at the place's corner
    (the Narrows' shape), is bound to a piece answering both at their planes:
    `contract-well-formed` and `DW0844` both green, the two declared openings
    disjoint. Vacuous if the openings do not meet at a corner: the test asserts
    the two shared faces' in-plane spans both reach the place's corner cell.
15. **The enclosed spaces are the author's declaration, and closure confirms
    exactly those.** A piece whose every space is `open` passes
    `contract-closure` with the stated zero (`0 of N`) and binds green to an
    open box and to a roofed one alike; a covered space declared `open` is taken
    as declared, its covered cells stated; the same space declared `enclosed`
    is examined (the perturbation: the declaration is what binds the gate), and
    unclosed it is refused. Vacuous if the piece has an enclosing space: the
    test asserts the contract's envelope set is `{open}`.

16. **Two places one cell apart with no connection are refused, and each
    remedy is taken green.** A plan with two roofed boxes one cell apart and no
    seam between them is `DW0827` naming both and the shared cells; the same
    plan with a seam between them, with the boxes two apart, and with the two
    made one box each validates. Vacuous if the boxes do not share a cell: the
    test asserts the claims' intersection is non-empty. Instrument: `crates/dsl`
    site-plan tests.
17. **A narrow walkable gap between two places on an open site is ground.**
    On the blockout fixture (`open`, flat terrain), the landing and the exit —
    two places with no seam between them — each open their side onto the narrow
    gap between them: the build is green, the binding states the commons, and
    the count of place pairs joined through the commons rises by exactly one.
    Perturbation: the exit keeps its side closed and the count falls back; over
    a `solid` fill the binding states no commons and joins no pair.
18. **A stand-in never ships.** The staging gate refuses a site-plan build whose
    blockout binding counts one box massed by the derivation, naming it, and
    admits the same build with that box bound; the gallery's site-plan overlay,
    fully bound, passes. Vacuous if the gate reads a flag rather than the
    binding line: the test perturbs the line's count. Instrument:
    `tools/creator/staging-gate.py` tests.

19. **`fill` is required, and both kinds build and bind.** A site plan without
    `fill` is `DW0100` naming the field; the blockout fixture builds under
    `solid` and under `open` with every proof green, and the two derivations
    differ exactly in the unclaimed cells — the test asserts that no claimed
    cell differs and that some unclaimed cell does. The gallery binds both
    kinds: its site-plan overlay declares `open` (a ground plane under the
    halls' free sides) and a second overlay point declares `solid` (the same
    places carved in rock), and perturbing either moves an emitted byte
    (`tools/ci/check-gallery-coverage.py` binds `Fill::solid`, `Fill::open`
    and each kind's fields). Vacuous if the two points share a fill: the gate's
    index names the kind each binds.

20. **The terrain is declared and placed.** A `heightmap` whose pixel count is
    not the region's `x × z` is refused naming both sizes; a heightmap whose
    height exceeds the region's top is `DW0826`; the blockout fixture under a
    sloped heightmap derives a surface whose top at every unclaimed column is
    the heightmap's value, and changing one pixel moves exactly that column
    (the test asserts the moved cells). The gallery's `open` overlay carries a
    generated heightmap and `tools/ci/check-gallery-coverage.py` binds
    `Terrain::flat` and `Terrain::heightmap` with their fields.
21. **A crack is refused and a faced plinth is not.** On the blockout fixture
    under a sloped terrain, a place bound to a piece whose edge ground stands
    two above the terrain with air under its edge is refused `DW0990` (second
    shape) naming the boundary segment and both heights; the same piece with its edge faced
    down to the terrain passes; the same edge with a declared `drop` seam
    across it passes; a step of exactly one passes. Vacuous if the edge is not
    above the terrain: the test asserts the two heights first. Perturbation
    `Perturb::hollow_edge` reds it alone. Instrument: `tests/blockout.rs`.
22. **A place that leaks is refused.** A hole in a place's own wall that lets
    a body into another place where nothing allocated an opening — the
    landing's party wall with the hall, two cells beside the allocated opening
    — is `DW0838` naming both places; the same piece without the hole is green
    (the perturbation). A stand-in — the engine's closed shell — whose walls
    come out short lets a body onto the commons, and `DW0838` names it as a
    stand-in leak (`--perturb short-walls`).

23. **The ring is fixed, and the ground runs through every plot edge.** On the
    blockout fixture under a sloped terrain, every fixed ring cell the handout
    names holds the terrain's block in the assembled bytes with every place
    bound and with none; a bound piece that writes stone, and separately air,
    at one fixed cell is refused `DW0990` (first shape) naming the cell and the
    displaced block; at a door seam the ring's ground is the sill's course, flat
    across the opening, and a stair host's treads lie inside its play space and
    write no ring cell. Vacuous if the fixture's terrain is flat at the floor
    (every fixed cell would be the floor course anyway): the test asserts the
    terrain's range across the plot exceeds one. Instrument: `tests/blockout.rs`
    and `tests/detail.rs`.
24. **The handout is complete and typed by nobody.** `delvec allocation` on the
    gallery's site-plan overlay emits, for every place, each field of §4 —
    intent and style, the concept reference path (or its named absence before
    step 9), position and neighbours, the whole's sheet, the initial ground
    with its fixed cells, seams and owed anchors — and `delvec detail` binds
    the `handed/ground/*` values from it; the output is byte-identical across
    two invocations. Vacuous if a field is optional and absent: the test
    asserts every field present on a place with a roof, a seam and a station.
    Instrument: `tests/detail_verb.rs`.

25. **The handout carries every seam's form to both sides.** `delvec
    allocation --all` on the gallery's site-plan overlay hands each seam, on
    each of the two places it joins, the same non-empty `form`. Vacuous if the
    string is absent from either side: the test compares every seam side and
    asserts the count of sides compared. Instrument: `tests/detail_verb.rs`.

## 12. Proof renders

Two frames of the gallery's annex from one eye position on the yard side, at
eye height, outside: the base engine (the stage-5 box) and this one (the roof
and the free wall the program drew). Paths in the implementation report.

## 13. Order of work

1. `dsl`: `roof` on the box; `siteplan::claim` (claim, owner, frame, voids);
   `Frame::of` over all boxes; `DW0988`; `DW0826` over the claim.
2. Compiler: `Mass::holes` from owned cells; the roof massing; the piece AABB;
   `detail::check` answering layer and `DW0987`; the allocation's new fields;
   the model's `structure_void`.
3. `delvec detail`: the handed roof values; voiding after expansion.
4. Tests, then the gallery (annex program, yard piece, probes, overlay binds),
   then docs, the baseline, the version. `fill` and `terrain` land in step 1
   (schema, `DW0100`, the heightmap read through the `image` crate the engine
   already carries) and step 2 (the derivation lays the terrain in place of
   `MASSIF`/`GROUND`, the fixed ring as rule 0, the handout's ground; `DW0990`
   at validation and in the battery, and `DW0838`'s second shape).

## 14. Departures, and where each criterion is demonstrated

Recorded at implementation, each with its reason. A departure that reduces
what a criterion asserts says so in the word *loosening*.

1. **What a terrain's height is.** Every terrain height is the `y` of the
   **surface block**; a `flat` terrain on a datum stands its surface block at
   the datum's `y − 1`, so the datum is the terrain's walk plane, exactly as it
   is a box's. The claim's bottom (§2) is therefore the lower of the lowest
   surface `y` under the footprint and ring and the floor course — §2's formula
   with "terrain" read as the terrain's walk plane. Without the reading, a place
   on terrain at its floor would claim two courses of ground and §2's sentence
   "the claim of a place with no `roof` is its stage-5 shell exactly" would be
   false.
2. **The ground under a plot stops above a place stacked under it.** A claim's
   bottom is raised to one course over the highest claim of any place lying
   wholly below its floor under its shell footprint, then capped at its own
   floor course. Otherwise an upper place over open, low terrain claims a column
   of ground straight through the place beneath it, and rule 3a's stacked plane
   never arises.
3. **Rule 3c at a corner three places share.** The seams the plan allocates
   between any two of the claimants across a plane through the cell are read
   together: where the seams between one pair disagree about `a`, the cell is
   refused (rule 3d, as written); otherwise it is the `a` of the first of them in
   the plan's seam order. With two claimants this is §2's rule exactly. A
   T-junction — a room beside two rooms that are beside each other, joined
   pairwise — always meets connections led by different places at one corner
   column, and §2 as written refuses every one; the designer's own order decides
   instead, never the engine.
4. **An eave stops at a neighbour's shell**, not only at its play space (§3).
   An eaves cell over a taller neighbour's ring or lid is in both claims with no
   rule to award it, so every eave beside a taller neighbour would be refused.
5. **The terrain's spelling is a tagged union**: `{"kind": "flat", "datum"}`
   and `{"kind": "heightmap", "heightmap", "base_y", "range"}`. §2c's literal
   spellings are not tagged variants, so the export would enumerate no
   `Terrain::flat` or `Terrain::heightmap` unit and criterion 20's coverage
   binding would bind nothing.
6. **A volume may name its own block** (`volumes[].block`, §2b's "unless it
   names its own"). Where the fill has no block of the volume's kind, the fill's
   own mass block is taken: a `massif` on an `open` site is the fill's `below`, a
   `ground` on a `solid` site its `block`. A `clearance` naming a block is
   `DW0193`.
7. **A heightmap's refusals are `DW0826`**: unread (a campaign handed to the
   checks without its directory), unreadable, not exactly the region's `x × z`
   pixels (naming both sizes), or a surface outside the region's `y` span.
8. **Criterion 11 is restated as what it asserts** (ruling 1, below).
9. **`DW0848`, a loosening.** A frame is a claim, never smaller than its box and
   wider by whichever ring cells and eaves the place owns, which no piece's
   bytes know; `footprint_class` is now held only to the class's narrowest box
   and its shallowest frame, with no upper bound and no kit-grid test.
10. **Where a seam is answered.** The place that owns the plane answers at the
    plane; the other place at its own first layer beside the plane. A declared
    via lying wholly in one plane inside a frame exports a face pointing away
    from its space, so a frame reaching past a plane it does not own (its eaves,
    its ring beyond a shorter neighbour) can still answer the seam.
11. **The seam gate is the plane owner's.** `owed_anchors` owes
    `anchor/seam-<edge>` to the place owning the plane; the row binds it to a
    gate anchor whose region is exactly the allocated cells (`DW0842` refuses
    another region), and `Plan::build` seats it with the piece's own `block`.
    The static `close-gate` check (`DW0343`) still answers the derivation's bar
    for a synthesized name; the runtime fill is the piece's.
12. **Criterion 9, retargeted.** A vista into an open place crosses only the
    plane it shares with its neighbour, which rule 3b gives to the roofed
    neighbour, so an open place's own wall never blocks one; and the fixture's
    vista is blocked by the hall's stair treads and the door sill's fixed ground.
    The pair is demonstrated where it is reachable: a vista from the landing to
    the exit, blocked at stage 5 by both stand-ins' facing walls, clears when both
    places are bound to pieces carving a window in the ring each owns.
13. **Criterion 14 at the unit.** No fixture carries the Narrows' shape; the
    test builds it from resolved boxes and seams and judges the piece with the
    grammar's own contract check and face export.
14. **Criterion 16's remedies at the rule.** On a fixture whose graph reaches
    every place, removing the seam between two adjacent places changes the
    graph; so the refusal is shown at the campaign (the exit hung one cell from
    the landing) and each remedy — connect, stand apart, make one — at the
    ownership rule (`crates/dsl` claim tests).
15. **Criteria 17 and 22 are rewritten** (ruling 2, below). **Criterion
    21, a loosening**: the crossing exemption applies to any seam's columns and
    is not separately shown with a `drop`.
16. **Criterion 1's instrument** is `tests/blockout.rs`: the gallery overlay's
    terrain is a heightmap the loader reads, which the `dsl` crate's tests do not.
17. **The model places template air** (§7): every cell a template names is
    written, air included, except `structure_void`. Before this, the model
    dropped a template's air, so it was more solid than the world; the
    correction can only turn a false red green, and is held by criterion 10.
18. **The concept reference and the sheet** are the design record's own rows:
    a place's image is the row `concept/<place stem>` (the record admits only
    `concept/` and `reference/` names), the whole's sheet its `reference/` rows,
    and their `shows` sentences the style lines §4 asks for (the record carries
    no other).
19. **Views that see a place** are those whose eye has the place's frame centre
    within 35° of its line to `look_at`, each handed as `delvec snapshot
    --camera <eye, yaw, pitch>`: a plan view is not a render-plan shot `--shot`
    can name.
20. **The contract gates read `structure_void` as the whole's.** A voided cell
    is not floor the contract owes (`contract-coverage`), and a voided boundary
    cell is counted rather than refused (`contract-closure`): the whole's
    proofs over the assembled world judge it.
21. **The light probe stands a piece on its handed ground**: the fixed ring
    cells hold the whole's block, so a doorway over its sill has a floor.
22. **The staging gate reads a third instrument**, the build's
    `validation/blockout.json`, and fails closed on a site-plan build without one.
23. **The overview camera stands over the declared fill** (the region's top on
    a `solid` site, the terrain at its column on an `open` one), as it already
    stood over a horizon's ground.
24. **`DW0838` examines** the standable cells of the region outside every
    claim: on an `open` site they are the commons (ruling 2); on a
    `solid` site that keeps a sky volume, the rock's top under the sky is such
    ground, and a place reaching it is refused — shown red in test, with its
    `fill: open` remedy taken green. Binding the solid point found that the
    walk looking for that ground excluded exactly those cells on a solid site,
    so the shape could never fire; fixed with the test.
25. **The gallery's annex** declares nine roof courses with one cell of eaves:
    the eaves course sits level with the lid, so a stepped 45° gable over twenty
    columns peaks nine courses above it. Its free walls are three (north, west,
    south), not two. It carries a station, `anchor/annex-bench`, so one place
    holds a roof, seams and a station for criterion 24.
26. **`form` is required**, never defaulted.
27. **Every place of both site-plan points is detailed** (criterion 18 as
    written): a stand-in never ships, so a stageable point binds every place.
    The open point's halls, loft, undercroft and causeway, and every place of
    the solid point, are plain programs under each overlay's `programs/`
    (a stone shell, a lantern-lit lid, an opening at every seam, the barred
    doors and the reliquary grille as gates, the stairs as flights). The
    solid point is its own campaign, `gallery-rock`, so the verb's piece names
    (`<campaign id>-<place stem>`) never collide with the open point's; the
    vista from the near hall to the loft is raised to head height in both and
    passes through a window in each wall, and on the solid point through a
    one-cell clearance in the rock between them.
28. **What the program path gained, found by binding the gallery**: a bar is
    exported as the gate anchor of its region, so a program can ship a gate a
    campaign names (spec-0058 §2.6 binds by stem); a `stair`, `drop` or
    `barred` edge's face is every via cell on the plane, treads and bars
    included, and a `barred` edge with no via faces out through its bar, so a
    stair rising through a neighbour's hole answers the seam (`DW0844`); and
    the light probe of `delvec detail` walks in through the place's seams, so
    a cellar entered only from above is measured (`DW0752`).
29. **`DW0833`'s `distance-xz` measures a place's floor centre**: the midpoint
    of the extent the place's standable cells span at its realized walk plane,
    over its own play space. One probe column through the box's centre read a
    stand-in's ladder pillar as the room's edge and rounded to the integer
    centre where the probe cell held a block, so an exact identity reddened on
    a correctly built map. A room built a course narrower still moves the
    reading. Shown in `blockout.rs`
    `the_centre_measure_reads_the_floor_and_not_one_column`.
30. **The rung in a floor's hole is the hole owner's.** A stand-in that cut the
    hole of a `climb` through a floor hangs the rung in it whatever the lower
    place's binding; a bound lower piece hangs its ladder up to its own
    ceiling. Shown in `blockout.rs`
    `the_rung_in_the_hole_is_the_hole_owners_whatever_the_lower_binding`.
31. **The overview camera stands over a stack** (departure 23 extended): an
    `interior` overview's eye also clears every placement and site-plan place
    standing over its column, ceiling course and roof out to its eaves. Shown in
    `emit.rs` `an_interior_overview_stands_over_a_place_stacked_over_its_eye`.
32. **Scenery with no floor owes no floor** (ruling 4 carried into the
    contract). A sealed piece — its place `reached: false` — none of whose cells
    is stood in states that zero with its count in `contract-coverage`,
    `contract-reachability`, `contract-no-body` and
    `contract-no-body-majority`, as `contract-closure` states an all-open
    piece's, so it is not forced to grow a floor that would then owe light. A
    reached place with nowhere to stand is still refused. Shown in
    `grammar_contract_check.rs`
    `a_sealed_scenery_piece_states_its_zero_standable_cells`.
33. **A climb inside one place is a contract edge.** A space is one floor, so
    a two-level interior — a treehouse's two decks, a lookout over a road — is
    two spaces, and the way between them when it is a ladder is a `climb`
    edge of the piece's spatial contract: `rise` and a `via` (the climbable
    cells and the hole they rise through) required, an optional `way`, proved
    connected both ways through the cells of its volume a body holds on in,
    over the body's own walk and climb moves (spec-0099) built from the
    piece's blocks; the reachability walk crosses it the same way. It is
    program document version `1.10.0`'s surface, refused below it where
    written (`grammar_contract.rs`
    `a_climb_is_refused_below_its_version_and_the_same_edge_as_a_stair_is_not`). The gallery
    binds it: the open point's causeway carries a lookout gallery on a stone
    stand, reached by a ladder. Shown in `grammar_contract_check.rs`
    `a_ladder_between_two_floors_is_a_climb_edge_proved_both_ways` and
    `a_climb_without_its_volume_or_its_rise_is_refused`.

| Criterion | Demonstrated in |
|---|---|
| 1 | `tests/blockout.rs` `the_ownership_rule_is_exhaustive_and_one_owner`; `crates/dsl` `siteplan::claim` tests |
| 2 | `crates/dsl` `detailplan` tests |
| 3 | `tests/blockout.rs` `the_derivation_writes_only_stand_ins_and_only_in_unbound_claims` |
| 4 | the gallery's site-plan overlay (`gallery/overlays/site-plan/programs/annex.json`), its build, the proof renders, and the `courses` perturbation measured in the commit that regenerates the baseline |
| 5 | the gallery overlay's yard piece; probe `a-yard-the-size-of-its-floor` |
| 6 | `tests/one_place_site.rs` `a_one_box_site_is_route_a` |
| 7 | `tests/detail.rs` `dw0987_refuses_a_piece_painting_a_neighbours_cell`; probe `a-wall-the-whole-already-owns` |
| 8 | `crates/dsl/tests/v14_site_plan.rs` `dw0988_refuses_…`; probes `a-roof-over-the-open-sky`, `a-roof-under-the-hall` |
| 9 | `tests/detail.rs` `dw0821_clears_when_the_places_own_walls_are_carved` (departure 12) |
| 10 | `tests/one_place_site.rs` `the_model_reads_structure_void_as_the_game_does` |
| 11 | `tests/detail.rs` `a_campaign_that_binds_nothing_is_massed_byte_identically`, `the_seed_moves_no_detailed_byte` |
| 12 | the gallery build over the overlay (the battery, `DW0885`, `DW0210`, `DW0891`, `DW0921`) |
| 13 | `tools/ci/check-dw-codes.py`; the reference pages; `docs/demo-levels.md`; the skill text in the round's scratch file |
| 14 | `compiler::detail` test `two_contacts_at_a_corner_answer_at_their_own_planes` (departure 13) |
| 15 | `tests/grammar_contract_check.rs` `an_all_open_piece_states_its_zero_and_passes`, `a_covered_space_declared_open_is_taken_as_declared`; `tests/detail.rs` `an_all_open_piece_binds_to_a_roofed_place_and_an_open_one` |
| 16 | `tests/blockout.rs` `dw0827_refuses_two_places_one_cell_apart_with_nothing_joining_them`; `siteplan::claim` `two_roofed_places_one_apart_are_contested_and_each_remedy_parts_them` |
| 17 | `tests/detail.rs` `the_commons_is_walkable_ground_and_a_solid_site_has_none` |
| 22 | `tests/detail.rs` `dw0838_refuses_a_hole_between_two_places_nothing_allocated`; `tests/cli.rs` `perturb_short_walls_reddens_dw0838_beside_dw0836` (the stand-in leak) |
| 18 | `tools/tests/test_staging_gate.py` `test_a_stand_in_never_ships`; `tools/ci/check-gallery-stageable.py` over both site-plan points, every place bound |
| 19 | `crates/dsl/tests/v14_site_plan.rs` `a_plan_without_a_fill_is_refused_naming_the_field`; `tests/blockout.rs` `both_fills_derive_and_differ_only_where_no_place_owns`; overlays `site-plan` and `site-plan-solid` |
| 20 | `tests/blockout.rs` `the_terrain_is_the_heightmap_column_for_column`; probe `a-terrain-on-a-datum-the-plan-does-not-name` |
| 21 | `tests/detail.rs` `dw0990_refuses_a_crack_and_passes_a_faced_edge_and_a_step`; `tests/blockout.rs` `a_hollow_edge_reddens_dw0990_alone` |
| 23 | `tests/blockout.rs` `the_ring_is_the_terrain_and_a_door_stands_on_its_sill`; `tests/detail.rs` `dw0990_refuses_a_piece_writing_the_rings_ground` |
| 24, 25 | `tests/detail_verb.rs` `the_handout_is_complete_and_hands_each_seams_form_to_both_sides` |

### Rulings

Four rulings on this spec, applied as stated.

1. **Compatibility is not a loosening.** Nothing owes compatibility to
   anything already built (CLAUDE.md), so a criterion asserting that stage 5 is
   unchanged from the engine before this spec asserts nothing the engine owes.
   Criterion 11 is restated as what it now asserts — binding nothing moves no
   byte, and the derivation is deterministic and seedless — and no longer
   appears as a loosening.
2. **Undeclared ground on an open site is ordinary walkable ground.** On a
   `fill: open` site, ground no place claims is an implicit **commons** that
   every place may open onto through its own openings, with no declaration
   needed: a narrow gap between two houses is simply ground. What stays refused
   is a place leaking (`DW0838`): a body crossing between two places other than
   through an allocated opening, with the commons excluded from the walk; a
   stand-in — the engine's closed shell, whose only openings are its seams —
   reaching the commons; and, on a `solid` site, which has no commons, any place
   reaching ground outside every claim. Two places that both open onto the
   commons are joined through it, and the binding counts those pairs. **The
   commons is not a node of the layout graph**: the graph holds the connections
   the design states, and an implicit node would make every open-site place
   adjacent to every other in it. The route proofs over the assembled world
   read commons cells as the ordinary world cells they are: the critical
   path's legs, `DW0311`'s reach, the gate-aware reach of `DW0306` and
   `DW0921`'s pocket flood all walk the assembled world, and none of them
   reads a claim, so a route through the commons is a route they walk and a
   pocket in it is a pocket. The remedies stay reachable: close the
   wall or allocate a seam (a crossing); detail the place (a stand-in leak);
   close the edge, make the ground a place or declare the fill `open` (a
   solid site). Criteria 17 and 22 test this.
3. **Enclosed spaces are the author's declaration, and the check confirms
   intent rather than restricting.** The principle, as the ruling states it: a check exists to
   help the author confirm, deterministically, that the design intent was
   achieved, never to limit creative freedom. A place's author declares zero
   or more box-shaped enclosed spaces in their piece, with coordinates, through
   the spatial contract's existing space regions; the closure check verifies
   exactly those; zero declared is legitimate and passes with its count stated
   (`0 of N`). `DW0989` is removed — no inference from the plan's `ceiling`,
   and no refusal of a roofed place with nothing enclosed: a pavilion, a
   covered market or a covered bridge is fine — and the number goes unused.
   Executing the ruling also takes out the closure gate's refusal of a space
   declared `open` or `open_top` under the piece's own blocks: without that, a
   pavilion had no kind it could be declared as. Such a space is a covered
   space, taken as declared, with its covered cells stated in the enumeration.

4. **A place may be scenery.** A box built to be seen and never entered —
   a tree's crown over a treehouse — is a layout node declaring
   `reached: false`, named from the closure's own word for a place a body
   gets to. A check confirms the declared intent both ways: a reached node's
   floor must be reached (`DW0816` over the graph, `DW0837` over the built
   world, as before), and a node declared not reached must NOT be — the
   closure reaching it is `DW0816`, and a body reaching a cell of its play
   space in the built world is `DW0837`, because a body getting into scenery
   is the design failing. Scenery still owns its outside (§2), still
   stitches its ground ring (`DW0990`), and is still detailed — a stand-in
   never ships. It is exempt from exactly what a place owes because a body
   visits it: the node anchor (`anchor/node-<x>`, the place a body stands —
   nothing stands there), the light a body plays by (the light probe of
   `delvec detail` binds no cell a body stands in, and none is owed), and the
   way in its piece would otherwise claim (the contract is judged sealed: no
   exterior traversal edge, declaring one refused, zero faces stated). The
   kind is the place's, read from the layout graph, never the piece's own
   word. The gallery's open site-plan point binds it with a sealed stone
   beacon south-west of the yard; the probe `scenery-the-graph-reaches`
   declares the loft scenery while the far hall's stair leads into it.
   Shown in `blockout.rs`
   `scenery_is_proven_not_reached_and_reachable_scenery_is_refused`.

### Corrections

Defects found after the rulings, corrected on the spec where they were made.

1. **An aloft seam stood on a wall of earth.** Rule 0 as first written
   levelled every seam's ring columns to the sill minus one whatever the
   terrain beneath, and fixed that ground as the whole's (`DW0990`): a bridge
   seam sixteen courses over a forest floor got a one-cell earth column from
   the floor to the deck, as wide as the opening, and no document could remove
   it. The fixed ground is the terrain's ground. Only a seam at grade — its
   sill one course over the terrain or less, or below it — levels the ring to
   its sill; a seam aloft fixes no earth, and the column between the terrain
   and the sill is the owner's by §2's rules. The handout's `ground_y` under a
   seam reports that ground. Shown in the `siteplan::claim` unit test
   `an_aloft_seam_fixes_no_earth_under_it_and_a_grade_seam_levels`, and the
   blockout test of criterion 23 now checks both cases on its slope.
2. **A ladder had no seam kind.** A vertical link that is a ladder could be
   declared only as a `walk` — which through a floor asked no sill, got no
   stage-5 stand-in, and left the lower place unreached at stage 5 (`DW0837`,
   `DW0986`, reproduced on the blockout fixture with its cell-to-undercroft
   link as a five-deep `walk`) — or as a `stair`, which masses treads the
   design does not have. The layout graph gains a `climb` class: a way a body
   climbs on a ladder or a vine the lower place hangs (spec-0099), through a
   floor or to a door high in a wall, with no treads and no sill; a climb
   between two places on one plane is `DW0992`. At stage 5 the derivation
   hangs a ladder in the lower place when it is a stand-in, and the climb
   moves prove it (`DW0837` reaches over it, `DW0986` crosses the opening on
   it); a bound lower place's piece hangs its own. A `walk` or `barred` seam
   in a floor between two floors further apart than a jump is now refused
   (`DW0829`), naming `climb`, `stair` and `drop`. The gallery binds the class
   on both site-plan points: the undercroft's crawl up into the near hall is
   a ladder its program hangs, topped in the hall's hole. Shown in
   `v14_site_plan.rs` `a_climb_carries_a_rise_and_a_climb_on_one_level_is_dw0992`
   and `blockout.rs` `a_climb_is_laddered_at_stage_five_and_a_deep_walk_is_refused`.

