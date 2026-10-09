# `delvec::compiler::blockout`

The reference page for `crates/delvec/src/compiler/blockout.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0821/DW0836–DW0839 — the derived blockout (`compiler::blockout`; error + two advisories)

Stage 5 of the map pipeline: the whole map's mass, derived from the site plan and
the metrics table, and then **judged against the plan it was derived from**.

**There is no authored form.** No document, no schema, no file. The only path to
blockout bytes is `compiler::blockout::derive`, whose input is a validated site
plan, and the only caller is `Plan::build` — which is the only constructor
`build`, `snapshot`, `blocking-chart`, `edit`, `detail` and `cameras --preview`
can reach a world through. That is what makes *blockout before site plan* uncompilable rather
than forbidden: there is nothing to author early.

**The mass is region writes, not a structure template.** A blockout box is a
shell — six faces of one uniform block around a volume of air — so a `.nbt`
packaging of it would be tens of thousands of mostly-air cells split across tiles
past vanilla's 48-per-axis cap, and would need a template writer this crate
cannot reach. A derived piece is therefore a `PiecePlacement` with **no
templates** whose blocks live in `AreaPlacement::mass`, applied in
`assembled::placed_blocks` one step ahead of the socket seals that already had
that shape. Nothing downstream special-cases it: `bbox()` answers for forceload
and relight off `pos`/`size` exactly as before, and a piece the prefab registry
has never heard of contributes no face contract and no anchors — which is
correct, because a derived box makes no claim about mating with anything. Each
write is split at the point of writing so none exceeds what one vanilla `fill`
will accept; a `fill` the server refuses fails in a function nobody reads.

**What the derivation writes, and what it never writes** (spec-0098). A wall
exists only where a design declares one, so the derivation writes, in order:
the site's declared `fill` over the whole region (a `solid` site's block, or an
`open` site's terrain — its `surface` block at the terrain's height and `below`
under it, merged into rectangles of equal height); the plan's `volumes[]`, each
of the block its kind takes from the fill unless it names its own; a
**stand-in** — floor course, ring wall, lid, and a declared roof zone massed
solid in the roof block — for every place no piece is bound to, written only in
the cells that place owns (`siteplan::Site::ownership`); the play spaces of
those places cleared; the **ring's fixed ground** of every place, bound or not,
as the terrain continued to the plot's edge; and each seam's frame and opening
only where the place owning the seam's plane is a stand-in. A bound place's
owned cells are written by its piece and by nothing else, and a stand-in never
ships: the build records every place still massed in `validation/blockout.json`,
which the staging gate refuses.

**No seed reaches the derivation.** It takes the plan and the table and nothing
else — no RNG, no clock, no hash-order iteration — so the same plan derives the
same mass, and changing `world.seed` changes no blockout byte.

**The synthesized vocabulary** (what makes the unchanged quest layer land on
massing nobody authored):

| Name | What it is |
|---|---|
| `spawn` | The entry place's footing. The derivation **declares** it, exactly as a prefab does: the anchor arrives carrying `"role": "entry"`, so the graph's own `entry` node is what decides, and the spelling is a name content may address rather than the thing resolution reads. |
| `anchor/node-<place>` | A place's own footing. `node/near-hall` becomes `anchor/node-near-hall`, because a campaign reaches an anchor through `anchor/<kebab>` and `node/<id>` is not a name any document could write. |
| `anchor/seam-<edge>` | A `barred` seam's gate region, filled at world load with the bar and measured shut by the same gate-seal model a prefab-authored gate is. `open-gate` and `shortcut` address it. Synthesized only while the place owning the seam's plane is a stand-in; once a piece fills that place, the name is the piece's gate anchor, bound through its `details[]` row. |
| `anchor/unlock-<edge>` | The far-side affordance's footing, on the side a one-sided `barred` seam opens from. Absent on an `either` seam, which needs none. |

Each is read off the mass **after** it is laid, not computed from the plan: a
stair the plan hosts in a box legitimately stands on that box's centre, so an
anchor placed by arithmetic alone lands inside the massing and `summon` does no
snapping. `dsl::siteplan::synthesized_anchors` is the one authority for the
names — validation resolves a campaign's anchor references against it and the
derivation places exactly it, so a name that validates cannot fail to exist.

**What invokes the battery**: `emit::build_with_warnings`, the one function that
turns a `Plan` into a datapack. A campaign with no site plan gets nothing and its
output does not move. There is no flag, no subcommand and no line in a document
to remember, and someone building a site-plan world without it would have had to
emit a datapack without `build_with_warnings`.

**Why the battery is an observer and not a replay.** Every verdict compares what
the plan declares with what the assembled bytes are. It does not know where the
derivation put a floor course, how it chose a pitch, or which cells it cleared —
it knows the plan, resolved by the same `dsl::siteplan` code stage 4 judged, and
it knows the world. That claim is demonstrated rather than asserted: each of
`DW0833`, `DW0836`, `DW0837`, `DW0838`, `DW0877` and `DW0986` is reddened in test by a **deliberately
perturbed derivation** (`blockout::Perturb`), never by hand-authored bytes, and the
production path passes `Perturb::none()` as a literal with a test asserting it.
`low_ceiling` closes one place a course under its plan's ceiling and moves nothing
else — no datum, no opening, no footing — so the whole error list under it is
`DW0833` alone, which is what makes it a defect only the headroom measure can see.
`open_stairwells` cuts the floor over every course of every through-floor run,
measured need or not, and is what shows `DW0836`'s stairwell admission refuses
something: the cells over courses no head reaches are a leak, and on the blockout
fixture the whole error list under it is `DW0836` alone.
`bury_barred` walls the cells flush behind every barred portal on its `b` side,
over the opening's own span — the shape a stair's treads laid across a doorway
take — and leaves the opening clear, so neither `DW0836` nor `DW0838` can see it;
on the blockout fixture and on the gallery's site-plan point `DW0986` is the
only refusal under it.

**And the demonstration is reachable from the command line.** `delvec build
<dir> --perturb <knob>` asks the derivation for one named defect and runs the
observer over the result, so the claim above is something a creator watches
happen on their own campaign rather than something they take from a test
transcript. The knobs are every field of `Perturb`, offered under
`--help` from `blockout::Knob` — `slide-openings`, `sink`, `short-walls`,
`brick-up`, `low-ceiling`, `wall-contacts`, `open-stairwells`, `bury-barred` — and the three that damage one place
(`sink`, `brick-up`, `low-ceiling`) take `--perturb-place <place>`, checked
against the site plan's own boxes before anything derives. One knob per run: two
at once and the code that fires says nothing about which defect it saw.

**A perturbed build writes nothing, and cannot.** `--out` and `--perturb` are
declared as conflicting arguments, so the invocation carrying both is refused by
the parser; a perturbed run has no output path at all, rather than a decision not
to use one. No tree means no `manifest.json`, which is the file
`tools/creator/staging-gate.py` hashes to give a build the identity an admission token
binds to — so a perturbed tree is unadmittable by construction. The exit is the
build tier whatever happens, and a perturbed derivation that NOTHING refuses is
itself a refusal, in those words: that outcome is precisely the failure the
facility exists to be able to see, and it also occurs honestly when a campaign
has nothing for that defect to damage (the metrics gym allocates no contact, so
`wall-contacts` reaches nothing there).

**A build stops at its first refusal and the battery states all of them.**
`BuildFailure` carries one code and one message, as every check in this compiler
does; one derivation defect is routinely seen by two of these rules, so the
battery prints a refusal line counting every code it raised and the messages
after the first, capped at twenty. Walls a course tall are the standing example:
they open every wall above its allocation (`DW0836`, which stops the build) and
join two places nothing connected (`DW0838`).

The step rule is the compiler's own (`nav::World::neighbors`), whose visibility
was widened for this rather than copied — a second step rule would make this the
one proof in the compiler taken under different physics.

| Code | Rule |
|---|---|
| `DW0836` | **A built seam disagrees with its allocation.** Three claims over the bytes. *Every allocated cell is passable* — a hole the derivation failed to cut is a connection the graph declares and the world does not have. *No other cell of the shared wall is passable*, asked per **wall** rather than per seam, because two connections may legitimately pierce one wall and the union of their openings is what it is allowed to have — **with one admission, a loosening of this claim**: a stair's stairwell. An open cell outside every allocation is admitted when a body on the stair uses it — a body standing in it, putting its head in it, or, from a raised tread two under it (above the host's planned walk plane), sweeping it in a jump — **and** it joins, across the wall through cells that are also so used, the hole of a seam in this wall the plan hosts a stair for (`blockout::stairwell_of`). It reads the plan and the bytes, never the derivation's own record of what it cut; a floor opened over courses no head reaches stays a leak, which the `open-stairwells` perturbation demonstrates. *The realized rise equals the declared rise*, measured as the lowest cell a body can stand on inside each place — so a floor course laid at the wrong height disagrees with the plan that put the two places at those datums. Build tier (exit 3). **Binding: seams proven, shared walls examined, and unallocated open cells admitted as a stair's stairwell.** |
| `DW0837` | **A node's floor is unreached.** Per-cell reachability from the campaign's own spawn over the assembled world, with every way the graph's monotone gating closure never opens sealed as the plan sealed it — the base assembled model holds gate regions open, so a proof taken over it would walk through a door nothing unlocks. A declared `drop` is **seeded**, not walked: the step rule models no free fall (a router that could fall would prove routes a body cannot come back from), so the far side of a fall whose near side is already stood in is handed a starting cell and the closure iterates. That is the graph's own declaration carried into the bytes, never a widening of the step rule. A declared `carry` (spec-0083 §7) is seeded the same way, in each direction it allows and only where the graph's closure grants its gating: a carry has no geometry for this battery to judge — the link that realises it is the route proof's (`DW0932`), and that the graph and the links agree is `DW0934`'s. The graph's `DW0816` proved this over topology; this proves the derivation preserved it in blocks. Build tier (exit 3). **Binding: places proven.** |
| `DW0877` | **A contact nothing can cross** (spec-0053). The measured crossing profile of a contact's span — the columns of it a body crosses over the assembled bytes, under `nav::World::neighbors`, the same step rule every route proof in this compiler is taken under and unwidened — holds no unbroken run of body width. The author allocated a front and the massing walled it, so the graph declares a hand-off the world does not have. It is the **contact's half of `DW0836`'s first claim, and a different claim rather than that one widened**: a portal is a hole and *every* cell the plan allocated must be clear, while a contact is continuous ground and massing standing on part of it is content — a rim with a boulder on it is still a rim. Asking a portal's question of a front would refuse correct content. **The one asymmetry**: a `walk` contact's column must let a body step out on both sides, because walking ground is two-way; a `drop` contact's only on the high side, because the far side of a fall is precisely what the step rule does not model and `DW0837` already seeds rather than walks it — one policy about drops in this engine, not two. Build tier (exit 3). **Binding: contacts examined, and crossable columns measured**, both stated beside the seam count, because zero columns over zero contacts and zero over three are different facts and only the pair separates them. Reddened in test by a **perturbed derivation** (`Perturb::wall_contacts`), never by hand-authored bytes. |
| `DW0986` | **A portal nothing crosses.** Every cell of the opening is clear (`DW0836`'s first claim holds) and the hole still leads nowhere: massing stands flush behind it, so a body that opens the bar meets a wall and, where another way joins the two places, every route proof goes round by it and stays green. **The quantifier**: some body standing in the opening steps, under `nav::World::neighbors` at the player's footprint, onto standable ground on **both** sides of the wall — the opening's standable cells taken as one floor a body may walk across first, with the stairwell `DW0836` admits beside a stair's hole in the same wall (a stair through a floor arrives in its hole on one tread and leaves from the next, so no single cell of it need step both ways) — measured over the world with every bar open. A `drop` owes only its **high** side, the policy `DW0877` and `DW0837` hold: a body walks into the opening from the higher place or, through a floor, to the brink, where the cell over the hole has room for a body and a body stands level beside it. A portal whose opening `DW0836` found solid is left to `DW0836` and counted apart. **Remedy**: move the seam along its face (`at`, with `meets` where the far box should stay put), or move what stands flush behind it — a stair laid in either place moves with its own edge's `at`/`meets`, a detailed place's piece is re-detailed. The remedy is taken in test (`tests/blockout.rs`: the same door at `at` 7 refused and at `at` 2 green). The `bury-barred` perturbation reddens it alone. Build tier (exit 3). **Binding: portals measured, standable opening cells, portals left to `DW0836` as solid** (`blockout battery binding:`). |
| `DW0838` | **A place leaks.** Delete every allocated opening from the world — and, on an `open` site, the **commons** — and no two places may still be walk-connected: a body reaching another place's space that way crosses a wall nothing allocated (a hole in a party wall, a wall the massing left low, a corner two shells did not close). **The commons** (spec-0098 §14, a ruling): on a `fill: open` site every standable cell of the region outside every claim is ordinary walkable ground, an implicit commons every place may open onto through its own openings with no declaration — a narrow gap between two houses is simply ground. Two places that both open onto it are joined through it, and the binding counts the pairs. It is **not** a node of the layout graph: the graph holds the connections the design states, and the route proofs over the assembled world (`DW0311` reach, `DW0306` gate-aware reach, `DW0921` pockets, the critical path's legs) read commons cells as the ordinary world cells they are — none of them reads a claim, so a route through the commons is a route they walk and a pocket in the commons is a pocket. Two shapes stay refused beside the crossing: a **stand-in** — the engine's closed shell, whose only openings are its seams — reaching the commons (the derivation's own leak: detail the place, or find what broke the shell); and, on a `solid` site, which has no commons, any place reaching ground outside every claim (close the edge on the ring the place owns, make the ground a place with seams, or declare the fill `open`). **Departure from spec-0049 §5.3, recorded**: the spec states the first shape as *every legal step between a cell owned by one box and a cell owned by another*, and read literally that rule is vacuous by construction — the plan's own `DW0828` puts exactly one cell between any two boxes, so no cell of one is ever a cardinal neighbour of a cell of the other and the rule quantifies over an empty set forever. The claim is therefore made over **paths**, which is the same claim and can fail: it catches the crossings the step form was reaching for and could not see — a wall the massing left low, a corner two shells did not close, a roof one open place lets a body onto — none of which is a single step between two owned cells either. Build tier (exit 3). **Binding: standable cells classified, place pairs tested, standable cells outside every claim (the commons on an open site), and place pairs joined through the commons.** |
| `DW0990` | **The plot does not stitch** (spec-0098 §2c, §7). Two shapes of one claim — that a place meets the ground the whole gave it. *A piece writes a fixed ring cell*: its template holds a block (air included) at a cell of the ring's fixed ground, read off the piece's own `.nbt` at validation beside `DW0987` (`compiler::detail::check_voids`, on `DW0886`'s precedent), named per cell with the terrain block it displaced. *A crack*: along a plot's edge — each column just inside the footprint beside the ring column just outside it — the two ground surfaces (each column's lowest standable cell from the claim's bottom to the top of the play space) differ by more than one block with air under the higher edge, and no seam crosses the ring there; measured over the assembled bytes, so a stand-in and a piece are judged alike, and named with both columns and both heights. A step of one passes; a higher edge standing on a solid face down to the lower surface (a plinth's face, a retaining wall, the fixed ground under a sunken yard) passes. The `hollow-edge` perturbation reddens it alone over a sloped terrain. Declared build tier; the first shape is raised at validation. **Binding: fixed ring ground cells handed and examined; plot-edge columns examined, cracks** (`void binding:` and `blockout battery binding:`). |
| `DW0821` | **A sightline is blocked.** The DDA walk of a declared `vision` edge's segment — `nav::walk_cells`, the same exact grid traversal the cutscene clip is proven with — naming **every** blocking cell rather than the first, because a walk sheet that names one cell of a wall has not said where the wall is. **Warning (exit 0) while any box is unbound; an error (exit 3) once `details[]` binds every graph node.** Derived massing has no landform, so a vista the detail pass will carve a ridge for is blocked here by the shells themselves, and refusing it then would force hand-shaped massing into a derivation whose whole property is that nobody shapes it. A fully detailed map has nothing left to carve, so the same world becomes a refusal. The severity is **computed from the artifact** — `compiler::detail::fully_detailed` — rather than set by a stage marker or an author flag, so there is nothing to set, nothing to forget, and no author who can choose the lenient reading. **Binding: sightlines walked.** |

`DW0833` and `DW0822` run their **second call sites** here, and the pair is the
point of each:

- `DW0833` re-measures the brief's identities off the assembled bytes, so a
  derivation defect that moved a datum cannot hide behind a plan-time green — a
  floor laid one block low satisfies every stage-4 rule, because stage 4 never
  saw a block. **`box-height` is measured over the whole footprint**: the tallest
  stack of clear cells standing over the place's realized walk plane at any
  column, capped at the declared clearance. The maximum, because every place this
  derivation builds has a flat ceiling — an unmassed column answers the height,
  and massing the plan itself put here can only ever answer less, so it cannot
  inflate the reading. `box-extent` is measured at the **top course** of the play
  space for the same reason on the horizontal axes: a stair the plan hosts here
  legitimately stands on the floor, and a measurement taken there reports the
  room as smaller than it is. **The prescription names both repairs**: a
  disagreement between plan and bytes is a disagreement about the MASS, and only
  one of the two things that put mass in a place is a defect — the derivation may
  have built it wrong, or the plan may have given the place a run of treads it was
  never given the room for, in which case the repair is to move the seam, host the
  stair in the other place, or widen the host. **Departure, recorded**: four of the five measures have a
  byte-side referent (a box's built footprint, its built headroom, the distance
  between two built places, a datum's realized walk plane) and `region-extent`
  does not. A region is a declaration the plan's contents must fit inside
  (`DW0826`); nothing is required to reach its edges and the derivation builds no
  object whose extent it is, so re-measuring it as *the extent of whatever got
  built* would refuse every plan that leaves a margin. Such an identity is
  evaluated once and counted as **declaration-only** in the binding line rather
  than passed over in silence.
- `DW0822` prints the **measured** A* route along the layout graph's own critical
  path, beside the projection stage 3 made of it. Neither carries a threshold;
  they exist to be set side by side, which is the only way the pacing coefficient
  gets calibrated at all.

**Binding line.** Every build of a site-plan campaign prints two: what the
derivation massed (places, how many detailed and how many massed by the
derivation, roof zones massed, fixed ring ground cells laid, seams with their
stair and barred counts, whole-owned volumes, anchors synthesized, region writes
and the cells they cover) and what the battery examined (seams, walls, places,
standable cells, place pairs, sightlines, identities with the declaration-only
count, critical-path legs, fixed ring cells handed, plot-edge columns and
cracks, standable cells outside every claim — the commons on an open site — and the place pairs joined through it).
