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
- **DSL**: `dsl_version` **0.38.0**. The site plan's `boxes[]` gain `roof`.
- **Diagnostics**: **DW0987** and **DW0988**, both used below. DW0989 is not
  consumed.
- **Research**: rooflines and facades at Minecraft scale are researched, not
  invented — `docs/reference/roof-and-facade-craft.md` carries the record with
  every rule marked cited or authored. This spec consumes two of its findings
  (§3) and invents none.
- **Non-goals**: retiring `areas[]`; the per-piece drawing medium (ADR-0030's
  question, §9); detailing a whole-owned volume (§10); jigsaw connectors.

## 1. The principle, in one sentence

**A place owns everything a body can see of it from outside, and the whole owns
only what two places would otherwise both write.** The frame a piece fills is
therefore the place's **shell** — its floor course, its play space, the one-cell
ring its walls stand in, and, for a roofed place, its ceiling course and the
roof zone the plan declares above it — minus the cells the ownership rule (§2)
awards elsewhere.

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
   d. else it is the **whole's** — two places that stand side by side with no
      connection, or whose connections disagree about which comes first, meet
      at a wall neither designed, and the whole's massing is the honest
      statement that nothing crosses there.
4. A cell no claim covers is the whole's.

**The frame** of a place is the bounding box of the cells it owns. Its piece is
exactly that size (`DW0843`, unchanged). Cells inside the frame the place does
not own are its **void cells**: the allocation hands them, the piece carries
`minecraft:structure_void` (or no block) at each of them, and `delvec detail`
writes them so after expansion — a derivation, never typed. A cell the game
does not place is a cell the whole's block shows through, which is vanilla's
own semantics for `structure_void`, and the assembled-world model reads it the
same way (§7).

**The whole writes** exactly what it wrote at stage 5, everywhere outside the
cells bound pieces own — `Mass::holes` is now the owned-cell set of every bound
place rather than its frame. A party wall between two bound buildings, a seam
opening cut in such a wall, its frame ring and its bar, are written by the
whole as before; a seam opening in a wall the piece owns is the piece's to cut,
and the piece ships a `barred` seam's shut state there, which is spec-0050 §3's
floor-course row generalised to every plane the piece owns.

**What stays whole-owned, enumerated**: every party plane between two
roofed places or two open places that no connection joins, or whose connections
disagree about `a`; every seam opening, frame ring and bar in such a plane; every derived stair in an unbound
host; every whole-owned volume; the region and its surround; the synthesized
vocabulary — `spawn`, `anchor/node-…`, `anchor/seam-…` gate regions,
`anchor/unlock-…`, every station stand-in — which the derivation places off
the plan whether or not a piece stands there; and every proof: the seam
battery, the reach proof, the unallocated-crossing sweep, the sightlines, the
identities, the pacing, the exposure ledger (`DW0885`), the light measurement,
the danger rule (`DW0891`) and the stranding proof (`DW0921`). Not one of
them changes, because every one of them reads bytes against the plan and never
asks who wrote the bytes.

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

**Two roofed places side by side** share the column over their party wall
through both roof zones; it goes to the place their connection names `a`, or
to the whole in the whole's block where nothing connects them (§2 rule 3c–d).
A building whose roof should run unbroken over several rooms is **one place**
whose piece declares several spaces — which is also how §5 says a route-A site
is written.

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
- **Between the scenes** the whole's volumes stand: the hillside, the sea bed,
  the rock. They are massing until stage 7 dresses them, and §10 names a
  dressed volume as the first falsifier this spec expects to meet.

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

## 7. The piece never writes the whole's cell

| Code | Rule |
|---|---|
| `DW0987` | **A piece paints a cell it does not own.** A bound piece's template holds a block other than `minecraft:structure_void` at a void cell of its frame — a party wall it was told is the whole's, a neighbour's facade, a clipped eave. Read off the piece's own `.nbt` at validation, where `DW0888` already opens it, and named per cell with the owner the plan awards it to. Air counts as painting: the game places a template's air, so an air cell over the whole's wall would carve it. Validation tier (exit 1). **Binding: bound pieces opened, void cells examined, painted.** |
| `DW0988` | **A roof the plan has no room for.** Two shapes of one claim, both read off the plan before any geometry: `roof` on a box whose `ceiling` is `open` — an open place has no lid to put a roof on; and a roof zone's course over the shell footprint lying in another place's play space or floor course — the stacked case, named with both places and the colliding courses. Eaves are not this refusal: they stop at a neighbour (§3). Validation tier (exit 1). **Binding: roofs declared, courses examined against places.** |

**The model reads `structure_void` as the game does.** `assembled::placed_blocks`
does not write a template cell that holds `structure_void`, exactly as it does
not write a cell the template omits — before this spec it wrote it as a block,
so a voided cell would have been modelled as passable while the server kept the
wall under it. Direction: a model more passable than the world can only let a
proof pass that the world would fail, so the correction can only turn a proof
red; it is held by a test.

## 8. What the derivation builds now

`compiler.md`'s blockout table gains one row and amends one:

- **a roof**: for a box declaring `roof`, the roof zone massed solid in the
  roof palette block, written after the shell and before the interiors are
  cleared, so a neighbour's play space is never filled by it (and could not be,
  by `DW0988`).
- **a box**: the shell as before; the derived piece's AABB (what forceload and
  relight read) is the claim.

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
(roofed places with whole-owned party walls) identically. The ownership rule
has three clauses and no table of kinds.

Falsifiers, each a brief this spec cannot state without a workaround and each
the trigger for a first-class surface rather than a hack:

- **The whole's own ground wants dressing.** Between the places the whole's
  volumes are massing in the derivation's palette, and the only surface that
  dresses them is stage 7's edit script. The first campaign whose hillside or
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
3. **The whole writes nothing a bound piece owns, and everything else.** On the
   blockout fixture with one place bound, the derivation's fills cover no
   owned cell of that place and every cell they covered when nothing was
   bound outside it; the party plane with a roofed neighbour is still written.
   Vacuous if the bound place owns zero cells: the test asserts the owned
   count. Instrument: `tests/blockout.rs`.
4. **A detailed building carries its own outside.** The gallery's annex program
   paints its two free walls and a gabled roof over a declared `roof`; the
   assembled bytes at those cells are the program's blocks, not the blockout
   palette's; its two party planes with the halls are the whole's wall; and the
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
   then docs, the baseline, the version.
