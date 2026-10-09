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
- **DSL**: `dsl_version` **0.38.0**. The site plan's `boxes[]` gain `roof`, and
  the plan gains a required `fill` — what every cell no place claims becomes
  (§2b) — whose `open` kind carries the site's **terrain**, a declared
  heightfield (§2c).
- **Diagnostics**: **DW0987**, **DW0988** and **DW0989**, all used below;
  `DW0827`'s quantifier widens from play spaces to claims (§2 rule 3d);
  `DW0838`'s widens to ground outside every claim (§2c). **One more code is
  needed** for the stitching check (§2c, written as `DW09xx`): the handed range
  is spent, so this spec stops at the placeholder and asks for it.
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
  One flat datum makes it worse. So the whole owns a terrain and every place is
  stitched to it (§2c), and undulating ground is in this design's core, not a
  follow-on.
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
| the floor course under the shell footprint (`y = floor − 1`) | yes | yes |
| the play space | yes | yes |
| the ring: the one-cell wall position on every side, `floor ..= top of play space` | yes | yes |
| the ceiling course over the shell footprint | — | yes |
| the roof zone: the shell footprint grown by `roof.eaves` on every horizontal side, from the ceiling course up `roof.courses` courses | — | when `roof` is declared |

So the claim of a place with no `roof` is its stage-5 shell exactly, and the
claim of an open place is that shell without its lid.

**The owner of a cell** is decided by this rule, in this order, over every
box of the plan — one function, `siteplan::claim::owner`, and every reader
below calls it:

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
is exactly what it refuses, naming the pair and a witness cell. Its remedies are
each reachable inside an authored document, which is what makes it a pair and
not a dead end: the place on either side **draws its edge** on the ring it owns
(a wall, a hedge, a kerb a body cannot step over); the gap **becomes a place**
— an alley with its own seams — or part of one; or a declared `volume` fills
it. The refusal names all three. **Reachable ground is a place.** In a bounded
map every cell a body can reach is inside some place — that is what a layout
node is — so ground a body can walk between two places is either a place of its
own (an alley, a square) or kept off by the edges of the places beside it; the
proof does not change for an open site, it only has more to say.

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

**The stitching rule, read off bytes.** A place shapes its interior freely.
Along its boundary — the outer face of the ring it owns, and the plane where a
neighbour's claim begins — the two ground surfaces either side must **meet**:
the standable surface on the place's edge column and the surface on the column
beyond it differ by at most one block (a step a body walks), **or** a solid face
stands from the lower surface to the higher (a retaining element the piece
drew — a plinth's wall, a revetment), **or** a seam across that boundary
declares the crossing (a `drop` with its rise, a `stair`, a portal). What is
refused is **the crack**: along a boundary segment, air between the two surfaces
on the higher side — an edge nothing holds up, Ulrich's tile gap in blocks.
Named with the boundary segment, both heights and both owners, build tier
(exit 3) in the stage-5 battery beside `DW0836`, read from the assembled bytes
with no knowledge of who wrote them, so a stand-in shell and a piece are judged
alike. **Code: `DW09xx`** — the handed range is spent; the number is requested.
Binding: boundary columns examined, cracks.

**The faced cliff is already refused where it matters, by the proofs that read
reach.** A body can walk off a plinth's edge: onto a neighbouring place, which
is `DW0838` (a connection nothing allocated) unless a `drop` seam declares it;
or onto ground no place claims, which is a way out of the designed places.
`DW0838`'s quantifier **widens** to say so: *delete every allocated opening,
and no two places are walk-connected, and no standable cell outside every
claim is reached from any place.* Reachable ground is a place (§2); the
remedies are the same three — close the edge, make the ground a place, or
declare the drop — and each is authorable. In a `solid` site the second shape
cannot fire, and its binding line says how many cells it examined.

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
says a route-A site is written.

## 4. What the handing says now

`delvec allocation <place>` (spec-0050 §4) gains: the roof zone's extent in
piece-local cells, or its absence; every void cell as a list of piece-local
AABBs, each naming who owns it (`whole`, or the neighbouring place); every
eaves cell the plan clipped, naming the place it stopped at; and the frame's
cell count beside its owned-cell count. `delvec detail` binds the same values
under `handed/` (`handed/roof/y0`, `handed/roof/y1`, `handed/roof/eaves`) and
voids the handed cells after expansion and before the piece is frozen. The
program paints a stand-in wall at a void column so that its own contract gates
judge a closed building; the stand-in never ships.

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
  between two houses too narrow for a road is ground and sky; if a body can
  walk it from one place into another the graph does not join, that is
  `DW0838`, and a design closes it or makes it a place (§2). The declared
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

**Two seams sharing a corner cell (#1005) — dissolved by the frame.** The
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

**An all-open piece refused (#1004) — answered, with its hatch closed.** A
street or a mud field honestly encloses nothing, and `contract-closure` reds a
zero binding when no space is `enclosed` or `open_top`. Under this spec open
places are first-class pieces, so the zero must be readable. **Declared as a
loosening, in these words**: `contract-closure` stops refusing a piece whose
every space is `open`, and instead states its binding — `0 of N space(s)
declare an envelope closure examines; every space is open` — at exit 0. What
secured that refusal was the question "did a room forget to say it is
enclosed", and that question is answered where the kind is known: **the plan**.
A roofed box is a place with a lid, so a piece bound to it must carry at least
one space closure examines; a piece with none is refused at the binding
(`DW0989`), naming the place's `ceiling` and the piece's envelopes. An open box
may carry a shelter (an `enclosed` space inside an open place is a hut on a
flat) or nothing enclosed at all. The kind is the object's — the box's
`ceiling` — never the piece's own word, so the defect cannot supply the hatch.

**Also recorded there, a doc defect (finding 4)**: the allocation hands a seam
as its two corner cells, which the skill's detail reference calls "its cells";
the reference is corrected to the handing's own words with the pin bump.

## 7. The piece never writes a neighbour's cell

| Code | Rule |
|---|---|
| `DW0987` | **A piece paints a cell it does not own.** A bound piece's template holds a block other than `minecraft:structure_void` at a void cell of its frame — a neighbour's facade, the party wall a connection gave the other side, a clipped eave, a gap cell. Read off the piece's own `.nbt` at validation, where `DW0888` already opens it, and named per cell with the owner the plan awards it to. Air counts as painting: the game places a template's air, so an air cell over a neighbour's wall would carve it. Validation tier (exit 1). **Binding: bound pieces opened, void cells examined, painted.** |
| `DW0827` (widened) | **Two owners for one block, and no rule to pick by.** Its claim is unchanged and its quantifier grows from play spaces to claims: two places whose claims share a cell that none of §2's rules 3a–3c awards — exactly one cell apart with no connection across that plane, or with connections that disagree about `a`. Named with both places, the shared cells and the three remedies (connect them, stand them apart, make them one). The engine neither arbitrates nor writes. Validation tier (exit 1). **Binding: box pairs compared, contested cells awarded.** |
| `DW0989` | **A roofed place bound to a piece that encloses nothing.** The place's box has a `clearance` ceiling and the bound piece's spatial contract declares no `enclosed` or `open_top` space, so the closure gate had nothing of it to examine and the room that is the place's reason for a lid is not in the piece. Read from metadata at validation beside `DW0843`. An open box is never this refusal. Validation tier (exit 1). **Binding: roofed places bound, pieces with an enclosing space.** |
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

- **a box**: the stand-in shell in the claim the place owns — floor course,
  ring, lid — exactly where its piece will draw; every seam opening the place
  owns cut in it, its frame ring and its bar as now; nothing written in a
  neighbour's cells and nothing in a gap.
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
11. **Stage 5 is unchanged where nothing is declared.** A campaign with no
    `roof` and no detail plan derives byte-identical fills before and after this
    spec. Instrument: the existing byte-identity test in `tests/blockout.rs`,
    whose fixture declares neither, and the gallery baseline's delta naming only
    the overlay that changed.
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
15. **An all-open piece binds to an open place and not to a roofed one.** A
    piece whose every space is `open` passes `contract-closure` with the stated
    zero and binds green to an open box; the same piece bound to a box with a
    `clearance` ceiling is refused `DW0989`. Vacuous if the piece has an
    enclosing space: the test asserts the contract's envelope set is `{open}`.

16. **Two places one cell apart with no connection are refused, and each
    remedy is taken green.** A plan with two roofed boxes one cell apart and no
    seam between them is `DW0827` naming both and the shared cells; the same
    plan with a seam between them, with the boxes two apart, and with the two
    made one box each validates. Vacuous if the boxes do not share a cell: the
    test asserts the claims' intersection is non-empty. Instrument: `crates/dsl`
    site-plan tests.
17. **A walkable gap is refused and the named remedies are reachable.** On a
    fixture of two places two cells apart on an `open` site with a flat
    terrain and no seam between them, `DW0838` names the pair and a cell in the gap; binding one
    place to a piece that draws a wall on its ring clears it, and so does adding
    the gap as a third place with two seams. Vacuous if the gap is not
    walkable: the test asserts the witness cell first. Instrument:
    `tests/blockout.rs`.
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
    two above the terrain with air under its edge is refused `DW09xx` naming
    the boundary segment and both heights; the same piece with its edge faced
    down to the terrain passes; the same edge with a declared `drop` seam
    across it passes; a step of exactly one passes. Vacuous if the edge is not
    above the terrain: the test asserts the two heights first. Perturbation
    `Perturb::hollow_edge` reds it alone. Instrument: `tests/blockout.rs`.
22. **Ground outside every claim is not reached.** A piece whose ring is open
    onto the terrain is refused `DW0838` (second shape) naming the first
    unclaimed cell reached; closing the ring, or declaring that ground a place
    with a seam, passes; on a `solid` site the shape's binding line reads zero
    cells examined. Instrument: `tests/blockout.rs`.

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
   `MASSIF`/`GROUND`; the stitching check and `DW0838`'s second shape join the
   battery).
