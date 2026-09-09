# spec-0062: Danger is visible, or the engine refuses it

- **Status**: Proposed
- **Ground**: written against engine `c90da006` (`origin/main`) and the branch
  `fix/a-body-has-a-width` at `8756b398`, read only. The branch widens a
  killing volume's impassable set by the body that walks into it
  (`dsl::metrics::keep_out_box`, with `cell_can_meet_volume` as its refusing
  reading); that arithmetic is right and nothing here changes it. What this
  spec changes is who consumes it. Every cell count below is re-derived by
  arithmetic on that formula and on the prefab documents it names, and is
  marked as such; nothing here was measured by building.
- **Diagnostics**: **DW0891** is allocated to this spec — verified free across
  all 75 engine remote refs (file names and text) at writing; `DW0890` is on 9
  of them, which is the cross-check that the scan reads refs. No second number
  is taken; §4 says why one code carries every shape.
- **Research**: `docs/reference/reach-and-hazard-volumes.md` — §2 there is
  the practice this spec's ruling agrees with: a killing volume is placed
  under the playable area, never level with the footing beside it.
- **Non-goals**: the size of the keep-out (a fact about a body and the
  selector); the harness's `keep_out`; a DSL offset for a volume's region
  (§9); the tie-break of `World::snap` for a route endpoint (§9, named); any
  content work; any edit to the ledger.

## 1. The ruling, and the defect it re-frames

**Authored, on the ruling in the operating practice.**

A killing volume is a selector: whatever hitbox intersects its box dies. A
player is 0.6 wide and 1.8 tall and stands anywhere within a cell, so the set
of feet-cells from which a body can be caught is the volume grown by half a
width horizontally and by a body's height downward — one cell out on every
side, one course down, for every body up to two blocks wide. The branch
computes that set and removes it from the walk graph. The fixture
`crates/delvec/tests/fixtures/lethal-volume` then shows what removal does: its
volume is the one cell of `anchor/exit`, on a stone floor that looks like every
other stone floor in the room, and the removal takes the floor around it out of
the party's footing. The route to the reach objective is closed (`DW0510`), the
reach rules disagree about `radius` (`DW0881` at 2, `DW0850` at 1), and the
fixture is red on the branch. Nothing in the room says it kills.

The ruling: **avoiding a lethal volume's collateral damage is never done by
marking ground that looks walkable as unwalkable — the player does not know.**
The constraint belongs on the volume. It sits at the bottom of the pit, or far
enough from the mouth that a body standing on solid ground cannot be caught.
**Danger is visible, or the engine refuses it**, and the repair for an
invisible hazard is to move the hazard.

So the same set answers a different question, asked earlier: **does this
volume, as declared, reach a cell the player would read as safe floor?** If it
does, the declaration is refused and the creator moves the volume. Everything
the earlier draft of this spec repaired downstream — the sealed doorway, the
two reach rules pulling on one number — is a symptom of a hazard nobody could
see, and it is refused at its cause.

## 2. What "reads as safe floor" is, in cells

**Authored**, from the engine's own predicates; the engine cannot see, so
every term below is a set it already computes.

For one volume `V` (its inclusive cell box, `Plan::zone_box`):

| term | definition | instrument |
|---|---|---|
| the keep-out `K` | every cell a player body can be caught from | `metrics::keep_out_box(Body::PLAYER, V)` |
| the population `P` | every standable cell the party can walk to from a cell it is **put** at — the entry spawn, every `set-checkpoint` and `bonfire` seat, every transit-teleport destination — over the assembled world **with lethality removed** | `World::reachable_walkable` on `World::without_lethal` |
| the caught cells | `K ∩ P` | set intersection |
| a caught cell **shows** the hazard | the block **under** it (the floor it stands on) or the block **in** it (what it stands in) is one of the volume's declared `shown_by` blocks (§3) | `Assembled.blocks` at `c − ŷ` and at `c` |
| **reads as safe floor** | a caught cell that shows nothing | — |

Four decisions in that table, each with its reason:

1. **The population, not the whole standable set.** A cell reached only by
   falling — the bottom of a shaft deeper than a step — is a cell a body
   enters deliberately, and the hole is what it read. A cell nobody can walk
   to (a roof, a sealed cavity) is floor nobody believes in. The walk model's
   own step rule (`World::neighbors`) is what "can walk to" means, so the
   answer is the same one every route proof gives. The population is rooted at
   every cell the party is put at, not at the entry alone as `DW0881`'s is: a
   party teleported into an area stands on that area's floor. A cell the party
   reaches only after a runtime write is outside the population and so outside
   this rule — the residual `DW0881` names, bounded by the same write.
2. **Lethality removed.** On the branch the walk model already refuses `K`,
   so a population taken from the lethal-applied world never contains a caught
   cell and the check is green over every volume — the unbound shape. The
   check reads the counterfactual `DW0510` already builds (*the identical
   world with lethality removed*), and §10.4 perturbs toward the vacuous shape.
3. **Under or in, never beside.** A shore cell next to lava stands on stone
   and holds air; the lava beside it is not what the body is on. That is the
   ruling's own sentence — a body on solid ground is not caught — stated as a
   cell rule.
4. **The whole keep-out, the volume's own cells included.** A volume laid over
   a stone floor at walking height catches the floor inside its box as much as
   the ring around it, and both read as safe.

**What separates a lava lake from a kill zone over stone.** A lake's surface
cells hold fluid; the occupancy model classes water and lava alike as
`flooded` — impassable and never floor — so no cell of the lake's surface is
ever standable, ever in `P`, ever caught. A volume drawn over the interior of a
lake catches nothing, and the rule is silent. A volume drawn to the lake's
shore catches the shore, which stands on stone and shows nothing, and the rule
refuses it: the remedy is to draw the volume one cell in from the shore, or one
course under the surface, so that a body on the bank cannot be caught. The
stone kill zone is the same test with the opposite answer: its caught cells are
the stone floor itself. What separates them is never the volume; it is whether
the caught cells are floor at all, and if they are, what they stand on.

## 3. A hazard that is meant to be flush declares what shows it

**Authored, on spec-0060 §4's shape: a declaration is a claim about the
bytes, and something reads the bytes.**

A lava surface, a magma floor, a bed of spikes, a burning strip: the danger is
level with the floor because the block **is** the signal. Such a volume is
declared with the blocks that show it:

```json
{
  "id": "lethal/the-burn",
  "region": { "anchor": "anchor/exit", "extent": [0, 0, 0] },
  "shown_by": ["minecraft:magma_block"],
  "message": "The floor here is molten stone.",
  "damage_type": "fire"
}
```

`shown_by` is an optional list of block ids on `lethal_volumes[]`. It is the
third field of its kind after `waterline_y` and `shown_faces`, and it is held
to their standard, so that it cannot become a word that switches the rule off:

1. **It is checked per caught cell against the assembled bytes.** A caught
   cell whose floor is stone reads as safe whatever the list says; naming
   `magma_block` clears exactly the cells that stand on magma. The check reads
   the same settled block map every other world proof reads.
2. **It must be borne out somewhere.** A listed block that stands under or in
   no caught cell is a fiction — the shape `DW0887` refuses on a waterline —
   and is refused, whether the list is wrong or the volume catches nothing at
   all. Every `shown_by` a built campaign carries therefore binds at least one
   cell, and the ledger says how many.
3. **It names a block vanilla hurts a body with.** The engine holds the set of
   blocks that damage a player in pinned Minecraft Java 1.21.11 as data in
   `crates/dsl`, beside the collision-class table and with `VanillaRule`
   provenance: `lava`, `fire`, `soul_fire`, `magma_block`, `cactus`,
   `sweet_berry_bush`, `wither_rose`, `pointed_dripstone`, `campfire`,
   `soul_campfire`, `powder_snow`. That list is authored here from memory and
   is pinned from the wiki by the test that lands it. A `shown_by` naming a
   block outside the set is refused at validation: `stone` over stone is
   borne out by the bytes and shows nothing, and this is the arm that says so.
   An unknown block id is `DW0193`, the code every block id in the DSL
   validates under, and not this spec's.

A `shown_by` block reaches a caught cell in one of two ways, and the model
already knows which: a block with a collision top (`magma_block`, `cactus`,
`pointed_dripstone`, a `campfire`) is the **floor** under the cell; a block
with an empty collision shape (`fire`, `soul_fire`, `sweet_berry_bush`,
`wither_rose`) is **in** the cell, passable to the walker. `lava` and
`powder_snow` never meet a caught cell — a body stands in neither — and are in
the set because a volume drawn one course under a lava surface is the ordinary
lava lake, and its declaration may say so.

What the field does **not** do: it does not exempt a cell from the walk graph.
A caught cell that shows magma is still a cell the router refuses (§6), because
a visible hazard is still a hazard. The field says what the player sees; the
keep-out says where the party may be proven to walk.

## 4. DW0891 — a killing volume the player cannot see

**Authored.** One code, on `DW0344`'s shape: a document arm and a world arm,
one rule — *a killing volume and what shows it agree*.

**The world arm** (`compiler::lethal`, build tier, exit 3). Raised once per
volume, over the final assembled world, in two shapes:

- **caught floor that shows nothing** — the message names the volume, its
  keep-out box, the caught cells by floor as `DW0881` prints them (six per
  floor, then a count), and the declared signals if any. The remedy is stated
  in the geometry's own terms: *no cell of the keep-out may be floor the party
  walks unless the block under or in it is one of `shown_by` — lower the volume
  so its keep-out's top course lies under the floor, draw its `extent` in so
  the keep-out stops one cell short of the floor, or author one of the blocks
  vanilla hurts with under those cells and declare it.*
- **a declared signal the bytes do not hold** — a `shown_by` block under or in
  no caught cell. The message names the block and the count of caught cells it
  was checked against (a count of zero says the volume catches nothing, and
  the declaration is what is wrong). The remedy: *delete the declaration, or
  author the block where the volume catches floor.*

**The document arm** (`dsl::validate`, validation tier, exit 1): a `shown_by`
id that is a known block and not one vanilla hurts a body with. Nothing has to
be placed to know it. The remedy: *name the block that shows the danger, from
the set the message prints.*

**Why build tier for the world arm.** Whether a cell is floor is a fact about
the assembled world after the edit script has replayed; whether it stands on
magma is a fact about the settled block map; where the volume is at all is a
fact about the solved layout. None is answerable from documents.

**Why one code.** The three shapes are one disagreement between a volume and
its signal, and every remedy each names is admitted by the others: signalling
the floor satisfies the first shape and creates no fiction; deleting a fiction
never uncovers floor; naming a hurting block never makes a floor read as safe.

**Where it stands in the order.** `DW0511` (a posted body inside a volume — the
branch asks it first, and that stays), **then `DW0891`**, then `DW0510`, then
`DW0850` and `DW0881`. A volume that catches walked floor usually closes a
route as well and often sits under a reach; asked first, the refusal names the
cause, and every later rule then judges a volume the player can see. `DW0891`
reads the plan's volumes, the lethality-free population and the block map,
never a route, so nothing is lost by asking it before the route proofs.

**Every remedy is reachable, as a check.** `DW0891`'s three moves each end
green or at a named successor with its own row in
`crates/delvec/tests/remedy_reachability.rs` (§10.8): signalling the floor
ends green on the fixture; deleting a fiction ends green, or at the first shape
when the volume also catches floor, whose move ends green; naming a hurting
block ends green or at the second shape. No code is met twice on any chain.

## 5. The ledger

**Authored.** `validation/lethal-gate.json` gains, per resolved volume, its
`keep_out` box, `caught` (cells of `K ∩ P`), `shown` (of those, cells that
show), and `shown_by` as declared; and, over the campaign, the population's
size, the totals, and the count of declarations examined against the count
borne out. The build prints one line:

```
danger-visibility binding: 2 volume(s) examined against a walked population of 412 cell(s);
9 cell(s) caught, 9 shown, 0 read as safe floor; 1 declaration(s) of 1 borne out by the bytes.
```

A campaign with no volume emits no file, as today. A volume that catches
nothing prints its zero with the population beside it, so a pit whose keep-out
lies wholly under the floor reads as *checked and clear*, never as *unbound*.

## 6. What becomes of the walk graph

**Authored.** Nothing. The router still refuses every cell of the keep-out
(`World::standable_fp` on the branch, through `World::meets_lethal_fp`), the
recovery stake still chooses its near lip outside it (`DeathRegion::keep_out`),
and `death-plan.json` still carries it to the bot as each volume's `keep_out`.
All three are answers to *where may a body be proven to be*, and a body may not
be proven onto a cell that kills it whether or not the player can see why.

What changes is what the removal can reach. Under `DW0891` every caught cell of
a legal volume shows its hazard, so **the walk graph never loses a cell the
player reads as floor**: a pit's keep-out lies under the rim and removes no
walked cell at all; a magma floor's keep-out removes the magma, which no route
should cross. The sealed doorway of §1 cannot recur, because the floor it stood
on would have been refused first.

The two navigation tests the branch adds for the removal
(`a_walker_may_not_stand_on_the_cell_beside_a_volumes_face`,
`a_corridor_one_cell_wide_beside_a_volume_is_not_a_way_through`) are unit tests
over synthetic worlds and pin a rule this spec keeps; they stand unchanged. The
gallery probe the branch adds (`a-lane-beside-a-killing-volume`, the west pit
moved onto the chest at `anchor/case`) declares an invisible kill zone on the
stone around a chest, and under the order of §4 the engine refuses it with
`DW0891` before `DW0510` is asked. It becomes `DW0891`'s refusal probe by
re-declaring its code (§10.10); `DW0510` owes a probe of its own whose caught
cells all show.

## 7. What becomes of the reach cycle

**Authored**, re-deriving the geometry of `campaigns/prefabs/hello-room.json`:
11×6×11, floor at local y=0, walk plane y=1, a dividing wall at z=6 with one
doorway at x=4..5 (`anchor/door`), `anchor/exit` at `[5, 1, 8]`.

The cycle arose because the ring swallowed the rim. For the one-cell volume at
`[5, 1, 8]` the keep-out is `[4, 0, 7]..=[6, 1, 9]`; the completion cube at
`radius: 1` is that box plus a course of air, so it holds no footing
(`DW0850`); at `radius: 2` it reaches the doorway `[4, 1, 6]`, `[5, 1, 6]` and
the two lips `[3, 1, 7..9]`, `[7, 1, 7..9]`, and `DW0881` roots its walk at
`[3, 1, 8]` — the `(distance², cell)` tie-break of `World::snap` among three
cells at distance² 4 — so the doorway cannot reach it inside the cube.

**The ruling dissolves the cycle for every hazard it moves.** A volume at the
bottom of a pit has a keep-out that never rises above its own top, so the rim
is footing at `radius: 1` and forms a ring the walk connects; `DW0850` and
`DW0881` have nothing to disagree about. A volume drawn in from a shore leaves
the bank as footing the same way.

**Two things from the earlier draft are still true, and one of them is still
needed by a case this spec creates.** A flush, signalled hazard — the fixture
of §8, a magma floor with a `reach` on its centre — has a keep-out that is
legitimately not footing, so the reach's nearest footing lies two cells out on
three sides, and `DW0881`'s reference is again one cell chosen by tie-break.
The verdict then depends on which side the tie fell, which is not a rule about
the world. So:

1. **`DW0881`'s reference is the anchor's footing set**: the anchor's own cell
   when it is standable; otherwise every standable cell of the footprint at
   the minimum distance² from the anchor. The demand is that every footprint
   cell the party can stand on can walk, inside the footprint, to **some** cell
   of the set. For an anchor that is footing the set is one cell and nothing
   changes; for one that is not, the rule stops refusing whichever lips the
   tie-break did not pick. That is a re-specification of the reference and is
   declared as a loosening in those words, bounded exactly: floor farther from
   the anchor than its footing is refused as before.
2. **The smallest radius whose cube holds footing, `r_min`**, is computed from
   the same standable set and is the floor under every radius any message
   names: `DW0850` names `radius: r_min` as a verified move (the judgement is
   taken again at `r_min` before the number is printed) and its *never widen
   the volume* leaves the message; `DW0881`'s *lower `radius` until…* names
   `r_min` or says that no radius from 1 to the authored one answers, in which
   case the remedy is the anchor's placement and the code is `DW0850`. A
   radius above the authored one is never proposed.

The earlier draft's single judgement over one `World`, with the two checks as
its callers and no geometry of their own, is how (1) and (2) are stated once;
it is kept as the implementation shape and not restated here.

## 8. What the fixture becomes

**Authored.** The fixture moves; the rule does not bend around it. Two
fixtures declare this volume — `crates/delvec/tests/fixtures/lethal-volume`
and `crates/delvec/tests/fixtures/economy`, both the one cell of `anchor/exit`
with `obj/exit` on the same anchor at `radius: 2` — and both move the same way.

- **The floor shows the hazard.** The fixture's `world-edits.json` gains a
  batch that replaces the stone of the floor course under the keep-out,
  piece-local `[4, 0, 7]..=[6, 0, 9]`, with `minecraft:magma_block`. Nine
  cells of floor, exactly the caught set.
- **The volume declares it.** `lethal/the-drop` becomes `lethal/the-burn`:
  `shown_by: ["minecraft:magma_block"]`, `damage_type: fire`, and a message
  and story to match (*the road ends at a floor of molten stone; they reach
  its edge*). The extent stays `[0, 0, 0]` on `anchor/exit`.
- **The side door stays**, and the reason is named: the route proof snaps its
  endpoint to the nearest standable cell within `SNAP_RADIUS` by
  `(distance², cell)`, which is the west lip `[3, 1, 8]`, with no regard for
  whether the party can walk there. That is a property of `World::snap`, not
  of this rule, and it is out of scope (§9).

Under this contract the fixture is: green at `radius: 2` — nine caught cells,
nine shown, the footing set `{[3, 1, 8], [5, 1, 6], [7, 1, 8]}`, the doorway
walking to `[5, 1, 6]` and the west lip to `[3, 1, 8]`; `DW0850` naming
`radius: 2` at `radius: 1`; `DW0891` with nine cells at y=1 when the
`shown_by` line is deleted; `DW0891` again when the magma batch is deleted;
and `DW0891`'s second shape when `shown_by` names `minecraft:cactus`. Every
test on the branch that expects `DW0510` or `DW0881` from an unsignalled
volume over stone now meets `DW0891` first and is re-declared as `DW0891`'s
test, or given a signalled floor; the branch reports eleven red tests on that
fixture, a count this spec states at the strength of that report and the
implementing round verifies by running them.

## 9. Scope, and what is named out of it

**Authored.**

- The magnitude of the keep-out: a body's width and the selector's rule.
- A region offset for `lethal_volumes[]` — a second placement vocabulary for
  one object class, refused on the ground spec-0060 §4 refuses a per-piece
  `horizon`. A volume that must sit at a pit's bottom sits on an anchor at the
  pit's bottom, which is prefab metadata.
- `World::snap`'s tie-break for a route endpoint, which prefers a lip the
  party cannot reach over a doorway it can (§8). A resolve over a scope where
  distance is not unique yields a candidate, not a match; it is a defect of the
  snap and is recorded against it, not repaired here.
- The harness's `keep_out`, wave seats, content, and the ledger.

## 10. Acceptance criteria

Machine-checkable; each names its instrument, and each was checked against the
tree at `c90da006` and the branch at `8756b398` before being written. Where the
tree cannot yet satisfy a criterion the verdict is recorded as a debt.

1. **The code.** `DW0891` exists in `compiler::lethal` (world arm) and
   `dsl::validate` (document arm), with one test per shape — caught floor that
   shows nothing; a declared signal the bytes do not hold, both with and
   without caught cells; a `shown_by` naming a block vanilla does not hurt —
   and zero new allowlist entries in `tools/check-dw-codes.py`. *Tree: debt —
   the code is on none of 75 refs.*
2. **The hurting-block set.** One function in `crates/dsl` answers whether a
   block id damages a body in 1.21.11, with `VanillaRule` provenance citing
   the wiki page it was pinned from; a test asserts every block §3 names is in
   it and `minecraft:stone` is not. *Tree: debt — no such table.*
3. **The surface.** `delvec schema --stage all` exports `shown_by` on
   `lethal_volumes[]`, under the `dsl_version` the implementing round is
   handed (this spec takes none); `docs/reference/compiler.md`'s
   `lethal_volumes[]` row carries it. *Tree: debt.*
4. **The population is the lethality-free one.** A test computes the check
   over the lethal-applied world and asserts it reports zero caught cells on
   the fixture, then over `World::without_lethal` and asserts nine; the check
   is bound to the second. *Tree: debt.*
5. **The fixture.** `lethal-volume` and `economy` as §8 declares them build
   green at `radius: 2` under the keep-out; at `radius: 1` they are `DW0850`
   naming `radius: 2`; with the `shown_by` line removed they are `DW0891`
   naming nine cells at y=1; with the magma batch removed, `DW0891`; with
   `shown_by` set to `minecraft:cactus`, `DW0891`'s second shape; two builds of
   each are byte-identical (ADR-0006). *Tree: debt — on the branch the fixture
   is red at `DW0881`; on `main` no keep-out exists and nothing binds.*
6. **The order.** A test declares a volume that both catches walked floor and
   closes the only route and asserts `DW0891`, not `DW0510`; the same volume
   with its floor signalled asserts `DW0510`. *Tree: debt.*
7. **The footing set and `r_min`.** A test builds the wall-between-two-rooms
   world (an anchor in a one-thick wall, footing on both sides inside the
   cube) and asserts `DW0881` reports zero off-floor cells; the mezzanine test
   (`a_raised_anchor_whose_volume_reaches_the_floor_below_is_refused`) still
   reports more than zero and fewer than the footprint; the
   raised-anchor-over-a-hall world at `radius` 1 and 3 asserts `DW0850` both
   times with *no radius from 1 to 3* and no *set `radius`* in either;
   `check_reach_completion` and `check_reach_footprint` derive no standable
   set, footing or walk of their own. *Tree: debt — the reference is one cell
   from `World::snap`'s tie-break; `DW0850` names no radius and forbids
   widening; the mezzanine half passes today and is the pin.*
8. **Remedies reachable.** `remedy_reachability.rs` holds a row for every
   move `DW0891`, `DW0850` and `DW0881` name; every row's terminal is exit 0 or
   a named successor with its own row, and no code is met twice on a chain;
   `tools/check-dw-codes.py`'s move subject widens from bases and documents to
   a backticked field or a named object, prints the count it binds, and every
   code it newly binds without a row is a red — the number newly bound is to
   be verified, and a row that cannot land is a ledger row per code, never an
   allowlist entry. *Tree: debt — the cross-check binds 7 codes; 0 rows for any
   of the three.*
9. **The ledger.** `validation/lethal-gate.json` carries §5's per-volume and
   campaign-level fields, the build prints the binding line, and a volume
   that catches nothing prints its zero beside the population. *Tree: debt —
   the ledger holds six fields, none of these.*
10. **The gallery.** Both pits adopt: one becomes a pit whose keep-out lies
    under the rim with a visible bottom, the other a flush strip that shows
    its hazard and declares it, so `shown_by` is bound; perturbing the
    declaration to a block not under its cells moves a byte in
    `lethal-gate.json` and reds `DW0891`; `a-lane-beside-a-killing-volume`
    is re-declared as a `DW0891` probe; `DW0510` keeps a committed probe whose
    caught cells all show; a `reach` anchored on the pit's own anchor
    completes from its rim at the `r_min` the engine names, and its radius
    perturbed to `r_min − 1` is `DW0850` naming `r_min`;
    `a-reach-that-completes-from-the-floor-below` is unchanged and green;
    `tools/check-gallery-coverage.py` reports 0 units in neither state.
    *Tree: debt — both pits are volumes over the hall's stone floor (`[2, 1, 3]`
    and `[20, 1, 2]`, extent `[1, 2, 1]`), each with about twenty walked cells
    in its keep-out at the walk plane, a number the implementing round measures;
    no gallery `reach` is anchored at a pit; the DW0881 probe passes today.*
11. **The record.** `docs/reference/compiler.md` carries the `DW0891` row, the
    re-stated `DW0510`, `DW0511`, `DW0850` and `DW0881` rows, and the lethal
    section's account of §2 and §6, in the PR that lands the code. *Tree:
    debt.*
12. A row for the player-visible half — a floor that shows it kills, and a
    pit the party can look into — is queued in `docs/demo-levels.md` when the
    code lands. *Tree: not yet due.*
