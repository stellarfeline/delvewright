# spec-0036: The spatial contract — spaces, edges, levels, closure, and coverage

- **Status**: Proposed (ADR-0020-map-design-pipeline; trial-0001 both runs are
  the motivating red; amended three
  times against the `tools/spike-spatial-contract` prototype — build
  `d3ce851`, its adversary scripts, and the round-3 cost measurement are the
  fixture seed; step 1, the declaration surface, is dispatched)
- **ADRs**: 0020 (decision), 0004 (extended), 0006 (determinism), 0018 §7
  (the `Program` version fence this rides)
- **Non-goals**: parameterised / cross-program `call` (its own spec);
  local-direction `mark` facing (`grammar.md` §7, own spec); overlay; the
  positional index; sightline/density/craft claims; campaign-side reachability
  diagnostics over unlock logic (follow-up fence); jigsaw connector emission
  (unblocked by §2.8, delivered separately); any geometry **generation** from
  the contract.

## 0. The governing rule for every opt-out

The re-validation defeated the first amendment round mechanically: a 90-line
script read the checker's own red cell-lists and bought a pass by declaring
every unreached cell `sealed` and downgrading every breached space to `open`.
The root cause was structural — `sealed`'s proof (unreachable) was **entailed
by the defect it existed to catch** (stranded), and `open` was an
unconditional exemption. A gate whose proof condition is implied by the defect
cannot discriminate.

So this spec has one governing rule, applied to every opt-out below and
binding on any future one: **an opt-out must be secured by a property the
defect cannot supply.** `sealed` demands its own closure; `posted` demands an
anchor; `facade` demands exterior air; an open envelope demands sky; `via`
demands the declared endpoints' own boundary; a merge demands one floor.
Anything that exempts a cell from an obligation must prove a different
positive fact about that cell.

Round 3 added the rule's corollary: **the author never picks which demand
applies.** A declared exemption names only its region and its reason; the
checker determines the kind (strongest applicable). Letting the author pick
was a one-word bypass — a 15-line loop tried every kind and kept whichever
passed, which voids the weaker kinds' role as discriminators.

## 1. Surface

### 1a. Program IR (fenced at the next `Program` version)

- `contract` block on `Program`: `{ entry: <space>, edges: [Edge],
  no_body_majority_ack?: <string> }`. **The string is not the demand** — see
  §6.1 for what secures the acknowledgement and for the recommendation to
  retire the field. **`entry` must carry at least one
  `exterior` edge of a traversal class** — the piece is enterable where it
  claims (Z4/Z6-style fall entries are an exterior `drop` edge).
- Declaration node `{ "op": "space", "space": <kebab>, "envelope":
  "enclosed" | "open_top" | "open", "body": … }` — wraps a body like `mark`,
  writes no blocks, claims **the scope's box** (never literal coordinates; a
  parametric program resolves its contract per expansion exactly as `mark`
  resolves anchors). **Multiple declarations claiming one name union**; the
  envelope is stated once. Unions exist because a non-box room (the
  cathedral nave's stepped cross-section) is otherwise a forced split whose
  upper box has no standable cells and can never go green — they are the fix
  for a case that had no fix, not a cost reduction (measured: spaces 28→17
  and nave closure breaches 936→0, but total boxes 43→58 and revision
  rounds 4→6).
- **A space is one floor** (the merge rule, §0 applied to unions): the
  standable cells of a space span at most 2 consecutive y-levels (a room may
  hold a dais; `connected` itself steps ±1). Greater relief means the author
  is describing two places and a transition, and transitions are edges. This
  is what stops a merged space from hiding a seam: fusing `stair-foot` and
  `stair-head` into one space is refused at well-formed, so the seam always
  crosses an edge and always owes a declared `rise`. (The alternatives — a
  per-space relief declaration, or named boxes with internal rises — were
  rejected: the first re-opens the hatch behind one reviewable line, the
  second adds surface that duplicates what edges already say.)
- `{ "op": "no-body", "region": <kebab>, "reason": <non-empty string>,
  "body": … }` — standable-but-out-of-play cells. **There is no `kind`
  field**: the checker determines the kind (§2.6, strongest applicable) and
  the verdict reports it per region; the author supplies only the region and
  a reason a reviewer will read. A `no_body` region may nest inside a space
  (the one licensed overlap); a region spanning hosts splits by host. Two
  different *spaces* may abut but never overlap.
- `Edge`: `{ a: <space | "exterior">, b: <space | "exterior">, class:
  "walk" | "stair" | "drop" | "barred" | "vision", rise?: <i64>,
  via?: <box | [box]>, bar?: { region: <scope-declared>, block: <role> } }`.
  - `rise` — the declared level relation, checked as `min_y(b) − min_y(a)`
    over resolved boxes: default 0 on `walk` **and `barred`** (checked when
    the bar-voided proof runs), required on `stair` (≥ 1) and `drop` (≤ −1),
    refused on `vision` and on any edge with an `exterior` endpoint
    (exterior has no resolved box; the assembly-time counterpart belongs to
    connector emission).
  - `via` — **constrained** (§0): on `walk`/`barred`/`vision`, a rectangle
    lying wholly on the shared boundary of the edge's endpoints (the piece's
    outer face for `exterior` edges); on `stair`/`drop`, a **transit
    volume** — disjoint from every space, abutting both endpoints — whose
    standable cells (treads, fall column) it covers. `via` is **required on
    `stair`** (the run's cells must belong to something; they belong to the
    edge). An unconstrained `via` was a one-line closure exemption anywhere
    on the model; this paragraph is what killed that.
  - `drop` is directed a→b. `bar` is required exactly on `barred`.
  - **`exterior` is a face, not a node**: contributes no connectivity;
    the reachability walk never routes through it.
- Collected into `Expansion::spaces` / `Expansion::no_body` /
  `Expansion::edges` (BTreeMaps), never into the `VoxelModel`.

### 1b. Prefab metadata

`spatial_contract` block in the metadata JSON: always the **resolved**
contract of the expansion that produced the bytes. Hand-built and ingested
pieces get the same block via `delvec prefab space` / `delvec prefab no-body` /
`delvec prefab edge`; their bytes never re-parameterise, so literal boxes are
the natural form there. Every declared anchor gains a `resolves_to` field
(§2.7). Absent block = legacy metadata.

### 1c. Checker

One checker over (block grid, resolved contract), callable from
`delvec grammar expand` (a red writes no `.nbt`) and from `delvec prefab audit`.
Same bytes + same resolved contract → same verdict, whichever door (AC6).
Both invocations are bound to the events they guard.

## 2. Obligations (each a gate with a verdict and a binding count)

1. **well-formed**: `entry` declared and carrying an exterior traversal edge;
   endpoints declared; `bar.region` on its endpoints' shared boundary; `via`
   as §1a per class; `rise` present/absent per class; the one-floor rule per
   space; spaces overlap nowhere; `no_body` overlaps nothing except a space
   it nests wholly inside; stair/drop transit volumes disjoint from spaces.
2. **coverage**: every standable cell lies in a declared space, a `no_body`
   region, or a traversal edge's transit volume. Uncovered > 0 → red, cells
   listed.
3. **closure**: for every `enclosed` space (and the side faces of
   `open_top`), every boundary cell is non-passable except: a declared
   edge's `via` (constrained per §1a), faces shared with an abutting declared
   space, and faces shared with an abutting `no_body` region. **An opening is
   claimed, never discovered** (§6.2): a via-less edge licenses no closure
   exception. **Envelopes demand sky** (§0):
   `open_top` and `open` are refused when any standable cell of the space
   has artifact solid above it — a roofed room cannot be downgraded out of
   closure, which was the adversary's second move and the one the three-
   decision list did not cover. Binding: boundary cells examined; any
   unexplained passable boundary cell → red, cell and writing rule named.
   **Named residual**: a sub-body visual breach into an abutting `open`
   region is render-review territory (a *walkable* breach is caught by
   §2.6); recurring as an owner finding gives it a machine form then.
4. **edge proof**, per class, over the endpoint spaces plus via, using
   `nav::connected` / `nav::reachable_with_fall`, and in every non-`vision`,
   non-exterior class the declared `rise` equals the measured value:
   - `walk`: connected both ways; rise as declared (default 0).
   - `stair`: connected both ways through its transit volume; rise ≥ 1 as
     declared.
   - `drop` (a→b): reachable under walk-and-fall; **not** b→a under the
     plain step; rise ≤ −1 as declared.
   - `barred`: **not** connected while `bar.region` stands; connected
     through exactly that opening with the region voided; rise (default 0)
     checked on the voided copy.
   - `vision`: no traversal claim; exempts exactly its `via` from closure.
5. **reachability — per cell, graph-confined**: every standable cell of
   every declared space, minus nested `no_body`, is reached from `entry` by
   the voxel walk **confined to declared spaces and transit volumes, crossing
   between them only through declared edges** — bars standing, drops forward
   only. The physical-walk reading was rejected and the choice is
   load-bearing: under it §2.5 is independent of the declared edges
   (deleting Z7's stair edge stays green) and edges decay into decoration;
   graph-confined is what makes an edge a checked claim. **A transit
   volume's standable cells are reachability targets too** — otherwise
   deleting an unreached space and re-hanging its cells on a stair edge as
   1×1×1 `via` boxes turns reachability green (found by attack, round 3).
   Unreached > 0 → red, counted per space and per transit volume. A space
   unreachable only while bars stand is re-walked with named bar sets opened
   and the required set printed per space; unreachable under every opening →
   red.
6. **the `no_body` obligation** — three kinds, **computed by the checker**
   (strongest applicable, in this order; the author never picks — §0's
   corollary), each demanding what the defect cannot supply. A region
   satisfying none is red:
   - `sealed`: **the union of all `sealed`-classified regions is itself
     closed** — every boundary cell non-passable. "Walled off", not "we
     failed to reach it": stranding is entailed by the §2.5 defect, closure
     is not. A genuinely walled recess passes as decoration; a stranded
     gallery's boundary opens onto the nave air and cannot classify here.
   - `posted`: out-of-walk standables placed bodies use. Demands an anchor:
     the region contains ≥ 1 declared anchor and **every standable cell
     lies within Chebyshev 2 of one** — per-cell deliberately, because the
     per-region-with-one-anchor form re-opens the blanket hatch (one decoy
     anchor on a thousand stranded cells) and was rejected on §0 grounds.
     Anchors are the campaign's exported namespace, so decoys are visible
     in every downstream surface — the cost that secures the kind, and the
     honestly-stated soft spot (ADR residual-risk note).
   - `facade` (round 3, replacing `open`, which it strictly subsumes —
     sky-open cells are a fortiori exterior-connected): exterior dressing a
     body never occupies in play. Demands **exterior air**: every standable
     cell of the region is touched by the flood-fill of air from outside
     the artifact's bounding box. This is the fourth demand the round-3
     cost measurement forced: 296 cathedral cells — wall-heads, buttress
     recesses, gable courses, apse-yard cells, cornices — are overhung
     ordinary stonework that no wall, sky, or anchor demand fits, and
     declaring them spaces manufactured four false closure breaches for
     every true one (signal 533-of-533 → 338-of-1718). Exterior-air
     connection is a positive fact an *interior* stranding cannot supply —
     an enclosed space's inside can never classify here, because its own
     closure proof guarantees no exterior air path. **A region nested
     inside any space can never be `facade`** (the interior of play space
     is play space's business: `sealed` or `posted` only) — this is what
     keeps a stranded shelf in an open-top shaft red rather than
     facade-green.
   Binding: regions, cells per computed kind.
7. **anchors**: every declared anchor resolves to a contract element — the
   closed extent of a covered space, a declared edge's via or bar region, or
   a `no_body` region (`posted` is the expected kind; others print a
   finding). Binding: anchor count, by element kind.
8. **exterior faces**: edges naming `exterior` are exported as the piece's
   face contract; `--traversable`'s claim is re-derived from them, retiring
   the standable-face approach-count heuristic, which counts any standable
   cell on a face and so reports 47 approaches where 3 are doors.
9. **vacuity reds**: a zero binding on closure, edge proof, or reachability
   is red. A `no_body` majority is red unless the contract carries
   `no_body_majority_ack` **and the majority is not made of `posted` cells**
   (§6.1) — the acknowledgement does not weaken §2.6, which still binds every
   region (AC8). `1 space, 0 edges` remains a printed finding. The verdict block enumerates, always: every `open`/`open_top`
   envelope, every `vision` via with its area, every `posted` region with
   its anchors, every `facade` region with its cell share, and every
   opened-bar set §2.5 used — the per-instance, named form that a blind
   script cannot satisfy and a reviewer actually reads.

## 3. Determinism and transparency

- Contract data serialises canonically; the double-expand suite extends over
  `spaces`/`no_body`/`edges` exactly as it covers anchors.
- Wrapper transparency: declarations move no block bytes (asserted as
  `mark`'s is).
- Checker verdicts are pure over (grid, resolved contract).

## 4. Order of work

1. **Prototype — done, three rounds** (`tools/spike-spatial-contract/`,
   re-validated at `d3ce851`). Round 1: cost bounded; the level relation is
   load-bearing (three of Z7's four drifts red only through `rise`; the
   fourth refuses upstream, deliberately outside the checker — AC5);
   per-space reachability and exterior-as-node unsound. Round 2: the
   opt-outs were mechanically defeatable, which is where §0 comes from; its
   two adversary scripts become permanent red fixtures (AC8). Round 3: §0
   held (no third total defeat; the one-floor rule measured cheap — 3 of 25
   spaces, every repair one the spec names), and produced the author-picked
   kind bypass (→ computed kinds), the transit-target hole (→ §2.5), and
   the taxonomy cost measurement (→ `facade`, AC14). Step 1 — the
   declaration surface, no obligations — is dispatched.
2. IR surface + checker in-engine, fenced; export + `delvec prefab` halves.
3. Docs and skill in the same PR (`grammar.md`, `prefab-procedure.md`
   §1/§3/§4, `tools.md`; `/new-delve` gains contract-before-rules).
4. Bell adoption round (same milestone): contracts for the eight zone
   programs, translated from `tests/zones.rs`'s topology **and level**
   assertions; the Rust suite keeps sightline/tell/density claims and gains
   checker-teeth fixtures. **Includes the AC9 library change** —
   `rafter_hall` anchors every perch — with the perch renumbering and the
   Z5/Z7 zone-gate counts it moves, named in the round summary.
5. Trial 2 (owner's Stormveil-class brief) against the reader-facing docs
   only, as trial-0001 was run.

## 5. Acceptance criteria

1. A `Program` at the new version with no `contract` block: refused, naming
   the field. At the old version: byte-identical compile (fence test both
   directions, red-demo per the fence rules).
2. The run-1 artifact under its honest contract: coverage green, per-cell
   reachability red, unreached count agreeing with an independent probe
   restricted to declared spaces (two implementations, not a pinned
   number). Closure red on the wall-less transept flanks, cells listed.
   Corrected twins of the distilled fixtures green.
3. Edge-class fixtures, pass + teeth: `drop`/rescue-ladder,
   `barred`/`unbarred`, `stair`/`broken_step`, `walk`/sealed doorway — plus
   rise teeth per traversal class: one course off the declared `rise` reds
   naming both numbers on an artifact where every topology obligation is
   green (the Z7 seam case, pinned).
4. Closure × mix pinned both ways: weighted air voiding an `enclosed`
   shell cell reds at that seed; the same program with the space *honestly*
   sky-open declared `open` is green — and a **roofed** space declared
   `open` is refused (envelope-sky teeth).
5. Z7 green on all obligations at the fixture region/seed; its three
   building drifts red through the checker via `rise`; the fourth asserted
   to refuse before bytes exist (both halves pinned — a refusal is the
   stronger channel, and the checker's silence over a non-artifact is
   correct).
6. One checker, two doors: `delvec prefab audit` agrees with `expand` for the
   same bytes and resolved contract; a re-parameterised expansion carries
   its own resolved boxes, never stale ones.
7. Double-expand determinism over `spaces`/`no_body`/`edges`; wrapper
   transparency.
8. **Both adversary scripts are dead and stay dead**: the 26-line
   all-`no_body` contract and the 90-line red-list-reader
   (sealed-blankets + envelope downgrades + acknowledgement), verbatim as
   fixtures, exit non-zero on the broken artifact — the blankets on §2.6
   `sealed` closure, the downgrades on §2.3 envelope-sky — and remain
   non-zero with `no_body_majority_ack` present. A vacuous-binding fixture
   (nothing enclosed, zero closure binding) reds on §2.9. **Both directions of
   the acknowledgement** (§6.1): a `sealed`/`facade` majority with the
   acknowledgement present is green, the same contract with the same
   acknowledgement over a `posted` majority is red, and the red names the
   `posted` share.
9. First-party anchor resolution with no per-rule exception, asserted over
   the library: `far_side_bar`'s `gate` (bar region), `boulder_stair`'s
   `volley-slot` (boundary cell), and `rafter_hall`'s perches classified
   **`posted`**, green — which requires the library change this AC now
   names: **`rafter_hall` declares an anchor on every perch** (all 10, via
   `index: auto`), not on alternating sides. The alternation left five
   standable corbel cells 14 blocks from any anchor — cells no campaign
   could ever address, which under this contract's own worldview is
   unfinished surface, not decoration (they are interior, so `facade` is
   rightly unavailable). The change moves exported metadata and perch
   numbering, so it lands in the bell adoption round beside the zone-gate
   counts it touches (Z5's "4 perches", Z7's "5"), and is named in that
   round's summary. The discrimination direction: the same shelves stripped
   of every anchor classify as nothing — nested, so no `facade`; unwalled,
   so no `sealed`; unanchored, so no `posted` — and red.
10. Union and one-floor rules, both directions: the run-1 nave as one
    union-of-boxes space is green where the forced two-box split produced a
    phantom forever-red space (kept as the union rule's red fixture); Z7's
    `stair-foot`+`stair-head` merged into one space is **refused** at
    well-formed (one-floor teeth — the merge that silently re-greened all
    three drifts in re-validation).
11. Exterior and graph-confinement: deleting Z7's stair edge reds
    reachability; an exterior edge never appears in a reachability
    explanation; a `via` off its endpoints' shared boundary is refused
    (the five-boxes-over-the-breaches cheat, kept as a fixture).
12. Anchors round-trip with `resolves_to` through `PrefabRegistry`
    (`crates/compiler/tests/grammar_prefab.rs` extended).
13. Trial 2 run and recorded in `docs/trials/` under the written-before
    rubric discipline, judged on: no structural defect of the verified
    classes ships green (independent probe, not the gates under test); the
    breach-walkway and one-way drop stated in edge classes without
    falsification; the agent's account does not name the contract as the
    reason a concept was cut. Any of the three failing is this spec's
    design failing, and the record says which.
14. **No closure exception is discovered** (§6.2): an `enclosed` space with a
    via-less edge to `exterior` and a hole in its wall is RED, and the same
    space with the hole claimed as that edge's `via` is green. The five-boxes
    cheat (AC11) and this fixture are the same rule from two sides.
15. **Signal restoration and the `facade` adversary, both directions**: the
    cathedral's five decoration families (296 cells: tower wall-heads,
    inter-buttress recesses, gable courses, apse-yard cells, cornices)
    declared as `no_body` classify `facade` and manufacture **zero**
    closure or reachability reds — the fixture asserts the closure red set
    equals the independently verified genuine set, restoring the
    round-2 signal that the three-kind taxonomy had diluted to 20 %. The
    adversary direction: interior unreached cells (the run-1 gallery)
    declared as free-standing `no_body` regions classify as nothing —
    no exterior air reaches them — and red; and a kind-shopping loop over
    declarations cannot exist, because there is no kind field to shop
    (§2.6 computes it).

## 6. Where this spec and the checker disagree, and which one is right

The checker is authoritative on both points below, and each is the same story:
the spec as written offered an opt-out §0 forbids, the implementation refused to
ship it, and it could not simply delete the surface — deleting a landed field is
the owner's call, not an implementer's. The spec is brought into line here.
§6.1 also carries a recommendation this round declined to take.

### 6.1 A string is not a demand — `no_body_majority_ack`

**As written**, a `no_body` majority — more than half the standable floor
declared out of play — is red unless the contract carries
`no_body_majority_ack`, a non-empty string.

**The defect supplies that for free.** Every other opt-out here demands a
positive fact about the cells: `sealed` demands its own closure, `posted` an
anchor within reach of every cell, `facade` exterior air, an open envelope sky.
The acknowledgement demands *typing*. That is not merely §0's usual shape (a
proof entailed by the failure) but something weaker still — an **unconstrained**
demand: the piece whose floor is 90 % stranded and the piece whose floor is 90 %
honest decoration write the identical field, and the reviewer reading the string
cannot tell which one is in front of them.

**What the checker demands instead.** The acknowledgement is honoured only when
the out-of-walk majority is made of the kinds whose demands are facts about the
blocks — `sealed` (a closed region) and `facade` (exterior air reaches every
cell). A majority of `posted` cells is **red with the acknowledgement present**,
because `posted` is the one kind an author secures by *placing something*: an
author free to scatter the anchors and write the excuse has authored their own
exemption twice. The kind is computed (§2.6 — the author never picks), so what
now secures the hatch is a fact the author cannot write.

**The recommendation, not taken.** The majority test is not an obligation at
all: it names no cells and proves no property. It is a *shape* measurement —
"most of this building's floor is out of play" is something a reviewer should be
told and something no author can be required to fix, because a cathedral is
legitimately mostly roof. The honest form is to **demote the majority to a
measurement in the verdict block and delete `no_body_majority_ack`**, leaving
§2.6 — which binds every region, one at a time, with a demand each — as the only
gate over out-of-walk floor. That deletes a `Program` field: a `dsl_version`
bump and an adoption round on every active campaign, so it is a versioned
decision and it is recorded here rather than taken.

### 6.2 An opening is claimed, never discovered

**As written**, §2.3 excused "the discovered opening of an edge with no `via`"
from closure.

**The defect supplies that too.** The demand is "declare an edge". A doorway
supplies it; so does a wall the author never built. Between two declared spaces
it costs nothing — an abutting space is already an excuse in its own right, and
crossing into one without a declared edge still fails the graph-confined walk of
§2.5 — but toward `exterior` it excuses a breach of **any size**: an
eleven-course hole in a nave wall and a door are the same declaration.

**What the checker demands instead.** A passable boundary cell is excused by
exactly three things: a declared edge's `via` (already constrained by §1a to lie
on the endpoints' shared boundary, or on the piece's outer face for an
`exterior` edge), the cells of an abutting declared space, and the cells of an
abutting `no_body` region. A via-less edge licenses nothing. An author who means
"there is a way out here" names the cells, and the `via` constraints then bind
to those cells — the same rule that refuses the five-boxes cheat (AC11), seen
from the other side.

What a piece leaves open at its own outer face is not a closure question at all.
It is the face contract (§2.8), which assembly consumes and `DW0780` mates.
