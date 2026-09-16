# spec-0065: Furniture is not floor

- **Status**: Accepted
- **Ground**: written against engine `93c9802e` (`origin/main`), read only, and
  against the released content revision `73182027` — `prefabs/doune-castle.json`
  (sha-256 `4857f90c69912802ea62c0abdddf1dcafa453c3eb75e12795a393a52c70069f9`)
  and `campaigns/doune-castle-tour/quests.json` (sha-256
  `c86c22a25aeab486d759f8e45a5ae3927ccc39ffec942ce27a11471ace69d99f`), fetched
  at that revision. Every engine fact below is a line read in the tree; every
  content fact is a value read from those two documents; nothing was measured
  by building. The shape of this document is spec-0062's.
- **What it is for**: a routed body — a guide the party follows, a puppet a
  scene walks — never takes a route over a laid table, an altar, a counter or a
  bed, because the piece that built the furniture said it is furniture and the
  walk model refuses to stand a body on it. The finding this answers is a
  released campaign's guide climbing onto a dining table and down the other
  side, on a route the build proved and nothing named.
- **Research**: no craft question is open. What a player reads as furniture is
  not inferred by the engine from block shapes (§2.3) — it is declared by the
  producer that built the piece — so there is no rule about materials to
  research. The record this spec rests on is the engine's own: spec-0060 §4 (a
  declaration is a claim about the bytes, and something reads the bytes),
  spec-0056 (one rule for what a body can pass through), spec-0036 / spec-0047
  (the spatial contract's out-of-walk regions and why they are computed per
  cell), spec-0062 and `docs/reference/reach-and-hazard-volumes.md` §2 (the
  ruling on ground that looks walkable). Every rule below is marked **cited**
  or **authored**.
- **Numbers**: no spec, ADR or DW code beyond this one. **Zero new DW codes**:
  `DW0888` gains a claim key and `DW0510` gains a second shape (§4.3, §5), each
  with its own tests. **`dsl_version` moves**: the prefab document type lives
  in the DSL crate and gains a vocabulary term a reader must know (§3).
- **Non-goals**: furniture a campaign lays itself, through `world-edits.json`
  or a runtime `set-block` / `fill-region` (§8.1 — recorded as a gap, with the
  surface it belongs to named); a runtime barrier that stops a *player* from
  jumping onto a table (nothing here touches the shipped world); any inference
  of furniture from block ids or shapes; the released campaign's own repair,
  which is content; the harness.

## 1. The defect, in the engine's own predicates

**Finding, from reading the tree and the content.**

The walk model has one standability rule, `nav::World::standable_fp`
(`crates/delvec/src/compiler/nav.rs`): a body of footprint `fp` may stand with
its feet in cell `c` when, for every column of the footprint, the cell below is
solid and the `height` cells from the feet up are unoccupied — and when no
declared lethal volume's selector can reach a hitbox standing there
(`World::meets_lethal_fp`). That predicate is what every route proof, snap,
flood and waypoint export funnels through, and the lethal clause is the only
term in it that is not geometry: it is the one place the model already says *a
body could stand here and may not be proven to*.

A laid table is geometry that satisfies the rule. In `doune-castle`'s bytes a
`minecraft:oak_slab[type=bottom]` stands on a `minecraft:oak_fence`; the slab
cell is solid, the two cells above it are air, so the cell over the slab is
standable, and the step rule (`metrics::step_allowed`) admits the one-block
rise onto it. A `move-npc` from `anchor/stop-lords-hall` (zone-local
`[72, 16, 40]`) whose shortest path crosses the table therefore climbs it:
the emitted per-tick `tp` rises a block, crosses, and comes down. The build
exits 0.

The piece document says nothing that could have stopped it, and could not have:
`prefabs/doune-castle.json` declares `walk_y: 4`, four `shown_faces`, a
`lighting` block, 31 anchors (one with `role: entry`, none with a `region`) and
no spatial contract. No key in the document's vocabulary means *furniture*, and
`DW0888`'s claim class — `structure.data_version`, `walk_y`, an anchor's `pos`,
`region`, `dispenser` and `trigger_block`, a connector's opening and socket, and
the jigsaw converse (`claims::ClaimKey::ALL`, nine keys) — has nothing to hold
such a claim to.

The released campaign no longer exhibits the instance: its route was moved.
The general form reproduces on an engine-side minimal case (the ledger row's
own reproduction), and it is the predicate that is wrong, not the table.

## 2. The row's general form, kept and corrected

**Authored**, against `CLAUDE.md`'s *This is a general engine* paragraph.

The row is right about four things, and this spec keeps each: the fact is *a
cell a body would not walk*, which is different from *cannot*; it is asked at
`standable_fp` and nowhere else; the declaration belongs to the object that
owns the blocks; and the declaration is a claim about bytes, held by the
byte-claim rule so that a declaration naming no furniture is refused rather
than believed. It is wrong about three things.

### 2.1 Not a new list — the piece already has an object class for a named place

The row proposes a piece-level list of furniture volumes *beside* `anchors`,
`walk_y`, `lighting` and `shown_faces`. But the piece document already has the
object class this is an instance of: an **anchor** is *a named place in a
piece* — one type covering a point, a gate region and a trap's hardware, each
writing only the keys it means (`prefab::Anchor`, `prefab-procedure.md` §9) —
and it already carries a `region` (an inclusive local box) and a `role` (what
the place is for, from a closed vocabulary the compiler owns, so the compiler
can find it without being told its name). A table is a named place with a
region and a purpose. A second list holding `{region}` objects would be
`check-capability-ownership.py`'s check C committed on purpose — a structural
twin of the anchor's own region — and it would leave the table nameless: a
campaign could not put a cup on it with `set-block`, aim a camera at it, or
play a sound from it. So the declaration is an anchor with `role: furniture`
and a `region` (§3).

### 2.2 No campaign-authored twin shaped like `lethal_volumes[]`

The row proposes a second, campaign-authored volume in the shape of
`lethal_volumes[]` (`{anchor, extent}` in `quests.json`) for furniture the
campaign rather than the piece puts down. That is refused here on the ground
spec-0062 §9 already ruled on for volumes: a placement vocabulary for blocks
belongs to whatever puts the blocks down, and a campaign puts blocks down
through exactly three surfaces — a `world-edits.json` batch (build time), and
the runtime verbs `set-block` and `fill-region` — each of which already carries
the region it writes. A fourth object in `quests.json` naming an anchor-centred
box over blocks some other surface placed is a second placement vocabulary for
one class of blocks, and it is also the opt-out shape: a box anybody can draw
over any floor, with no producer's bytes to hold it to. What a campaign-laid
table owes is the same claim on the surface that laid it, and that is recorded
as the gap it is (§8.1) rather than designed here without an instance.

### 2.3 Not a scan after the proof — the predicate itself

The row's check "quantifies over every cell of every route this build proves
and every waypoint it exports, and refuses where one stands inside such a
volume". That is a second authority for the standable set: a scan that agrees
with the predicate is redundant, and one that disagrees is the defect. With
the exclusion inside `standable_fp` no route, snap, flood, seat or export can
stand on furniture in the first place, and what the creator sees when the only
route crossed a table is the counterfactual the model already builds for the
lethal case — *the route exists only when this exclusion is lifted* — naming
the anchor (§4.3). The quantifier the row wanted is then the predicate's own,
and it is total by construction.

## 3. The declaration

**Authored**, on `prefab::Anchor`'s shape.

```json
"anchors": {
  "anchor/high-table": {
    "role": "furniture",
    "region": { "from": [26, 16, 39], "to": [30, 17, 41] },
    "resolves_to": "space:lords-hall",
    "note": "the laid dining table down the middle of the hall"
  }
}
```

1. **`furniture` is a term of `AnchorRole`**, beside `entry`. It says what the
   place is for: *blocks a body stands beside and never on*. It is closed
   vocabulary, so a misspelling is refused by name where the document is read
   (`DW0346`), as `entry` is.
2. **The role requires a `region`.** A furniture anchor names cells, never a
   point; an anchor carrying the role and no region is a declaration with no
   subject and is refused as the byte-claim's own shape (§5) — nothing is
   guessed from `pos`.
3. **The region is the furniture's blocks**, not the air over them: the table's
   legs and top, the altar's stone, the bed. What the exclusion removes is
   derived from those cells (§4.1), so a region drawn one course too high
   removes nothing and is refused (§5).
4. **It is a named place.** A campaign addresses it like any anchor: a
   `set-block` puts a candle on it, a camera subject frames it, a `play-sound`
   plays from it. Nothing about the role forbids a reference; what it forbids
   is a *walk*.
5. **Written by the producer that built the piece.** A generator that lays a
   table writes the anchor beside it (`delvec prefab anchor --role furniture`
   for a hand-built piece); a census derivable from the object is never typed
   by hand.

The declaration lives in the prefab document, which is a library asset and not
a stage document — `delvec schema --stage prefab-metadata` exports it, `all`
deliberately does not — so no coverage unit is enumerated for it (§9.1 says
what the gallery owes instead).

## 4. What the walk model does with it

**Authored.**

### 4.1 The exclusion

`World` carries, beside `lethal_regions`, the placed furniture regions as
`(anchor id, world box)`, resolved through the same anchor table every placed
piece's regions already resolve through. `standable_fp(c, fp)` then refuses
`c` when, for any column `[dx, dz]` of the footprint, the **support cell**
`[c.x + dx, c.y − 1, c.z + dz]` lies in a furniture region **and is solid**. In
words: a body may not be proven to stand *on* furniture. Air cells inside the
region contribute nothing; a body standing beside a table, feet on the floor,
is untouched, because the rule is membership of the support cell and not a
selector's reach. The lethal clause stays exactly as it is; the two are asked
in one predicate, one after the other.

### 4.2 What is not judged

- **A declared post on furniture stands.** An NPC anchored on a tabletop — a
  cat on the table, a crow on the fence rail — is staging, and the compiler
  posts it where it was declared. Posts are judged by what judges them today
  (`DW0450` for solid geometry, `DW0511` for a killing volume); the furniture
  exclusion judges **walks**: route proofs, `move-npc` / `move-actor` legs and
  their snaps, floods, wave seats and waypoint exports. A body posted on a
  table that is later walked somewhere starts from the nearest standable cell
  its snap finds, as a body posted inside an affordance does today.
- **The danger-visibility population is not reduced.** `DW0891`'s population
  `P` (spec-0062 §2) is taken over the world with **every** semantic exclusion
  lifted — the counterfactual `World::without_lethal` becomes *without
  exclusions*, one function — so a furniture declaration cannot hide a killing
  volume's caught floor. That is the hatch this surface would otherwise open:
  mark the stone round a pit as furniture and the pit reads clear. A tabletop
  beside a lava lake that the keep-out catches is refused as caught floor,
  which is right — the body that climbs it dies.
- **The spatial contract's demands stay geometric.** Coverage (every standable
  cell lies in a space or a `no_body` region) and reachability (every space is
  reached from `entry`) are asked of the admission walk over the piece's own
  bytes, and a table stands *inside* a space: it is in play, not out of it,
  which is why it is not a `no_body` kind and why spec-0047's computed kinds
  are untouched. The admission standable rule (`schem::nav::standable_cells`)
  is unchanged, on the precedent the lethal clause set: the compiler's
  predicate asks *may a body be proven to walk here*, the admission's asks *can
  a body stand here*, and the two questions have two answers on purpose.

### 4.3 What the creator sees

- **A route that exists only over furniture** is `DW0510`'s second shape: the
  proof re-routes with the exclusion lifted, finds the way, and refuses naming
  the furniture anchor it crossed — *the only route runs over `anchor/high-table`*
  — with the same remedy family the lethal shape has: move the mark, open a way
  round, or move the furniture in the piece; never delete the declaration to
  silence the proof. One code, because it is one rule: *a route may not depend
  on a cell a body may not be proven to stand in*; the message names the kind.
- **A `move-npc` / `move-actor` destination on furniture** snaps to the
  nearest standable cell within `SNAP_RADIUS`, as a destination inside an
  altar does today, and the leg is proved to that cell.
- **Every build prints** `furniture binding: F region(s) over S solid cell(s),
  W standable cell(s) withheld from walking; L leg(s) proved, 0 standing on
  furniture` — zeroes included, so a campaign whose pieces declare none reads
  as *checked, nothing declared* and never as unbound. `W` is the count that
  says the declarations bit; a positive `F` with `W = 0` cannot occur, because
  §5 refuses it in the library.

## 5. The byte claim

**Authored, on `DW0888`'s class**: *a prefab document's declaration that its
own bytes deny*. `claims::ClaimKey` gains **`AnchorFurniture`**
(`anchors.*.furniture`), examined for every anchor carrying the role, in the
same read as the other nine keys and printed in the same census. Three
shapes, one key:

1. **The role with no region** — a subject-less declaration (§3.2).
2. **A region holding no solid cell** — furniture drawn over air; the same
   fiction `DW0887` refuses on a waterline with no water.
3. **A region that withholds nothing** — no geometrically standable cell of
   the piece has its support in the region. The declaration changes no
   verdict, and a declaration that changes no verdict is refused rather than
   carried (`DW0454`'s shape for a body's traversal claim). It is the
   opposite of an opt-out: a furniture declaration can only make a proof
   fail, never pass (§4.2), so the one thing left to hold it to is that it
   does *something*.

The region's cells lying inside the piece is already `AnchorRegion`'s key and
is not restated. The check runs wherever `check_piece` runs: `delvec prefab
audit` over a file, a tile set and a library, `delvec prefab seating`, and the
campaign's own validation when it seats the piece — validation tier, exit 1,
before a block is placed. The remedy each shape names is reachable and is one
edit in the piece document: give the anchor its region; draw the region over
the blocks; or delete a declaration over something no body could stand on.

## 6. The pair with the danger ruling

**Authored.** Spec-0062 transcribes the rule that ground which reads as safe
floor is never marked unwalkable, because the compiler knows and the player
does not. This surface marks cells unwalkable, and it is not that defect,
for a reason that should be written where the two will be read together: a
furniture declaration withholds cells the player *reads as not floor* — the
declaration and the player agree, which is the exact opposite of a kill zone
over stone. What keeps it from becoming that defect is §4.2: the one check
whose verdict marking floor could improve, `DW0891`, does not read the
exclusion. Everything else the exclusion touches can only turn red.

## 7. What the gallery, the record and the skill owe

**Authored.**

- **The gallery element** (spec-0039). The gallery generator lays a table in
  the hall — a row of `oak_fence` under `oak_slab[type=bottom]`, the released
  campaign's own construction — across the straight line between a mark a
  `move-actor` starts from and its destination, so that the shortest route
  crosses it, and the piece declares `anchor/high-table` with the role. Bound
  by perturbation: removing the role from the generated document moves the
  per-tick `tp` lines of that actor's `ma_tick_*` function (the route climbs
  the table) and the leg's rows in `validation/critical-path-waypoints.json`;
  the binding line's `W` moves from a positive count to zero. No coverage unit
  is enumerated for a prefab key (§3), so the element is the emission, and the
  gate reports the binding count from the build. `DW0888`'s new key is tested
  in `crates/delvec/tests` over a synthetic piece, each of the three shapes
  once; it needs no gallery probe because a probe perturbs stage documents and
  the claim is about a library document.
- **The record.** `docs/reference/compiler.md`: the `DW0888` row gains the
  key; the `DW0510` row gains the shape; the nav section states §4.1 and §4.2
  in the pull request that lands the code. `docs/reference/prefab-procedure.md`
  §9 lists the second role term beside `entry`. `docs/reference/grammar.md`
  gains the mark spelling once a generator writes it.
- **The skill.** A creator reads it from
  `references/new-pieces.md` (the body-has-to-walk-up-it section: a table is
  declared, not left to the walker) and from `references/quest-capabilities.md`
  under *Bodies*: a body is posted where it is declared and is never walked
  onto furniture; a route that has nowhere else to go is refused naming the
  table.
- **The demo level.** A row in `docs/demo-levels.md` when the code lands: a
  hall where a guide leads the party round a laid table, and the same hall
  with the table's declaration removed is the refusal.

## 8. Scope, and what is named out of it

**Authored.**

### 8.1 Furniture a campaign lays

A `world-edits.json` batch that builds a table, or a runtime `set-block` /
`fill-region` that does, owes the same declaration on the surface that laid
it — a `furniture` claim riding the edit's own named region, held to the
assembled bytes after the batch replays, at build tier. No campaign in the
record lays furniture that way (the gallery's four batches dress a floor, lay a
hearth, thin a vault and rough a lane; the released castle's table is piece
bytes), so the surface is not designed here: it is a ledger row against the
line in `WorldEdit` that would carry it, and the row names this section.

### 8.2 The rest

- A barrier in the shipped world. The exclusion is a fact about proofs; a
  player who jumps onto a table has done what players do.
- Inferring furniture from block shape, id or height. A bottom slab is a step
  on a stair and a tabletop on a fence; the difference is intent, and intent is
  declared.
- The content repair of the released campaign.
- The harness and the bot's own walk.

## 9. Acceptance criteria

Machine-checkable; each names its instrument, and each was checked against the
tree at `93c9802e` before being written. Where the tree cannot yet satisfy a
criterion the verdict is recorded as a debt.

1. **The role.** `AnchorRole` carries `furniture`; `AnchorRole::ALL` has two
   terms; `delvec prefab anchor --role furniture` writes it; a misspelled role
   is `DW0346` naming both terms. *Tree: debt — one term.*
2. **The claim.** `ClaimKey::ALL` has ten keys; `check_piece` examines every
   furniture anchor and refuses each of §5's three shapes with `DW0888`, one
   test per shape in `crates/delvec/tests` over a synthetic piece; the byte-
   claim census prints `anchors.*.furniture` examined and denied on every
   run; `delvec prefab audit` over the pinned content library reports
   0 furniture declarations examined and 0 denied. *Tree: debt — nine keys.*
3. **The predicate.** A unit test over a synthetic world places a solid cell
   inside a furniture region under an otherwise standable cell and asserts
   `standable_fp` is false for that cell, true for the cell beside it, and true
   for a cell over an *air* cell of the region; the same test with the region
   removed asserts all three true. *Tree: debt — no such term in the
   predicate.*
4. **Walks, not posts.** A test declares an NPC on a furniture cell and
   asserts the build posts it there (the summon line carries that cell) and no
   diagnostic names the anchor; a `move-npc` to the same cell asserts the leg
   ends on the snapped floor cell. *Tree: debt.*
5. **The counterfactual.** A test builds a room whose only route crosses a
   declared table and asserts `DW0510` naming the furniture anchor and no
   lethal volume; the same room with a route round the table builds green and
   its critical path carries no waypoint whose support cell lies in the
   region. *Tree: debt.*
6. **The hazard hatch is shut.** A test takes spec-0062's fixture, declares
   the floor round the volume as furniture, and asserts `DW0891` still reports
   the same caught-cell count as without the declaration. *Tree: debt.*
7. **Admission unchanged.** `schem::nav::standable_cells` over the gallery
   hall's own bytes returns the same set before and after the generator adds
   the table's declaration (the table's blocks change the set; the role does
   not), asserted by a test that runs the count twice on one grid. *Tree:
   debt.*
8. **The binding line.** Every build prints §4.3's line; a campaign declaring
   no furniture prints `0 region(s)`; a test asserts the line on the gallery
   primary has `F ≥ 1` and `W ≥ 1`. *Tree: debt.*
9. **The gallery.** The generator lays the table and writes the anchor; the
   primary builds green; perturbing the declaration away moves a byte in the
   actor's tick function — the bearer's per-tick `tp` samples change in count
   and their y rises over the table; `tools/check-gallery-
   coverage.py` reports 0 units in neither state; two builds are byte-identical
   (ADR-0006). *Tree: debt — no table in the hall; the generator's anchors are
   all points.* Loosening: this criterion asserts no byte in `validation/critical-path-waypoints.json`, which holds only the player's route.
10. **Determinism.** Furniture regions enter `World` in placed-piece order and
    anchor-name order, never hash order; the double-build gate over the gallery
    is green. *Tree: debt.*
11. **The record.** `compiler.md` rows `DW0888` and `DW0510`, the nav section,
    `prefab-procedure.md` §9, `tools.md` where `prefab audit`'s census is
    described, and the two skill pages of §7 — in the pull request that lands
    the code. *Tree: debt.*
12. **The ledger row for §8.1** exists in `docs/playtest-findings.json` with a
    non-zero binding computed by `tools/staging-gate.py`'s `probe()` over the
    edit verbs that write blocks, or the round records that the binding is zero
    because no campaign lays furniture. *Tree: not yet due.* *Implementation:
    recorded debt — `probe()` over `set-block` / `fill-region` in the gallery
    primary's `quests.json` measures a binding of 2, and the row cannot land
    yet: with no general-form carrier it is `NO-GENERAL-FORM` at
    `tools/staging-gate.py` (exit 1), and `tools/check-gallery-stageable.py`
    refuses a gallery point carrying a capability-gap row with no general-form
    carrier. No row is added.*
13. A demo-level row is queued when the code lands. *Tree: not yet due.*

## 10. Decisions for the owner

- A piece declares furniture as an **anchor with `role: furniture` and a
  `region`** — the alternative is a separate `furniture[]` list in the prefab
  document, refused in §2.1 as a structural twin of the anchor's region.
- **No campaign-side furniture list** in `quests.json` — the alternative is
  the ledger row's `lethal_volumes[]`-shaped surface, refused in §2.2; a
  campaign that lays a table through an edit owes the claim on the edit, and
  that is recorded as a gap rather than built now.
- A body may be **posted** on furniture (a cat on the table) and is never
  **walked** onto it — the alternative is refusing the post too, which would
  forbid staging the engine has no reason to forbid.
- A route that has nowhere to go but over a table is **refused** naming the
  table (`DW0510`'s shape) — the alternative is an advisory, which would let
  the released campaign's guide ship again.
- The declaration **moves `dsl_version`**; no ADR, no new DW code.
