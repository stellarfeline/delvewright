# ADR-PENDING: The spatial contract — declared spaces and edges, checked against the emitted bytes

- **Status**: Proposed
- **Date**: 2026-08-12
- **Source**: owner ruling 2026-08-12 ("the map-design pipeline needs restructuring,
  or our gameplay and our scenery cannot be unified"); trial-0001 (Notre-Dame),
  both runs; the planner's tree/graph analysis, amended below
- **Refines**: ADR-0004 (extends its "layout validation reduces to graph
  properties" consequence down into pieces; does **not** supersede it),
  spec-0027 (adds a gate family), ADR-0018 §7 (rides the `Program` version fence)
- **Constrained by**: ADR-0003, ADR-0006, ADR-0001, the general-engine rule,
  the no-hacks rule

## Context

Trial-0001 built a cathedral twice from the reader-facing docs alone. Both runs
passed every machine gate with non-zero bindings and both shipped structurally
broken artifacts. Verified on the saved artifacts (probe: `reach.py`, re-run for
this ADR):

1. **Stranded storeys.** Run 1: 4 982 standable cells, 2 113 reachable on foot
   from the ground entrance, **0 % reachable at every height above y = 3** —
   1 034 cells at gallery level, 216 on the tower decks. Run 0 identical in
   shape. `traversable` was green both times, correctly: it proves a
   ground-level walk from approach face to exit face, which is a claim about a
   *kind* of piece, and nothing obliged anyone to claim more.
2. **Silent seam disagreement.** The saved run-1 artifact carries a 2-cell
   overshoot on **both** transept arms — the side-wall stubs run 2 cells past
   the arm's end wall, leaving open-topped exterior notches no rule intended —
   with every gate green. (The trial record's asymmetric 7×12×9 sky hole does
   **not** reproduce on the saved artifact; it evidently described an
   intermediate program state. The defect *class* — two rules that must agree on
   a shared plane, agreeing only by hand-computed literals nothing reads — is
   in the shipped bytes regardless, and `grammar.md` §2c idiom 5 documents the
   same class one scale down: a rounded split's unwritten thirteenth course,
   27 cells of daylight, both gates green.)
3. **Duplication grows with scale.** 26 % of run 0's rules and 30 % of run 1's
   are copies of another rule once role names and call targets are erased
   (re-measured: 29 of 113, 44 of 145). Real, already filed (task #107), and
   **not what this ADR fixes** — see "What this ADR does not decide".

**The correction that bounds the design space**: run 1 was a single program
producing a single zone with no assembly step. Merging prefabs into one larger
generator fixes nothing; the defects live inside one derivation.

### The tree/graph reading, amended

The planner's reading: a `split` derivation is a tree, a level is a graph, and
the graph's edges are exactly where two mutually-invisible rules must agree via
hand-computed literals nothing verifies; the missing layer is the industry's
blockout as a machine-checkable artifact.

The failure-site half is right. The expressibility half is wrong, and the
counter-evidence is in this repo: the bell zones already build graph edges
inside the tree — Z7 climbs eight blocks to its own lift landing, four zones
carry one-way drops, three carry barred shortcut branches. The tree can state a
graph. What the bell zones paid for it is the tell: every edge that crosses a
scope boundary is guarded by a hand-maintained arithmetic identity
(`climb == treads()`, `shaft/sill == climb`, plinth `= drop − 1`) **plus a
per-zone Rust gate in `tests/zones.rs` that watched it drift**. That second
channel — bespoke Rust assertions per composition — is the middle layer, and it
already exists. It is just not *authorable*: the trial agent had `--traversable`
and nothing else, because the obligations live in a language and a repo the
authoring loop cannot reach.

So the missing thing is not a layer that never existed, and not expressive
power. It is **the zone-gate discipline as data**: the traversal intent stated
once, in the artifact of record, and checked mechanically against the emitted
bytes — for every author, not only for one that writes engine tests.

The reading also under-covers one defect family. The sky slot and the transept
lip are not graph edges; they are **envelope** failures — an unwritten or
misplaced boundary cell that no traversal claim would ever visit. A blockout
graph alone passes them. The contract therefore carries two kinds of
obligation, not one: connectivity (spaces and edges) and **closure** (a
declared space's boundary is solid except at declared openings).

## Decision

### 1. The artifact: a **spatial contract**, owned by the prefab object class

A prefab — grammar-generated, hand-built, or ingested — may carry a spatial
contract: a set of named **spaces** (boxes a body occupies), typed **edges**
between them (how a body moves between two spaces, or between a space and the
exterior), an **entry** designation, and per-space **envelope** declarations.
The contract is *declared intent*; a checker proves the delivered blocks agree
with it. Nothing is ever inferred from the block pattern — that direction is
the folklore the no-hacks rule forbids.

The capability sits on the prefab metadata type (`delvewright_schem::prefab`),
not on the grammar verb: the grammar program is **one authoring surface** for
it (declaration nodes, expansion collects them exactly as `mark` collects
anchors), `delve-admit` is the surface for hand-built and ingested pieces (the
same pattern as `socket`/`anchor`/`lighting`), and the checker runs on
delivered blocks so both routes get the same proof.

Edge classes are mechanisms, mapped to nav predicates that already exist:
`walk` (bidirectional `connected`), `stair` (bidirectional + measured rise),
`drop` (forward under `reachable_with_fall`, **not** backward under the plain
step — `drop_shaft`'s own gate pair, as data), `barred` (not connected while
the declared bar region stands; connected through exactly that region with it
voided — `far_side_bar`'s gate pair, as data), `vision` (no traversal claim; a
window is a hole the closure rule must not flag). The genre that wants one-way
drops and barred shortcuts is content; the classes are body-movement facts any
game configures.

A `barred` edge's declared bar region is, deliberately, the **region anchor**
that §7 of `grammar.md` records as inexpressible: the cells a campaign's
`shortcut`/`close-gate`/`lift` fill addresses. One declaration serves the
proof and the content binding — the capability lands on the object (the edge),
not on the verb that first needed it.

### 2. The obligations (full statement in the spec)

Coverage — every standable cell lies in a declared space or a declared
`no_body` region; closure — every boundary cell of an `enclosed` space is
non-passable except inside a declared edge opening or a face shared with an
abutting space; edge proof — every declared edge holds in the voxel model
under its class predicate; reachability — every space is reachable from
`entry` over the declared graph, with the barred-closed/openable stratification
reported per space; anchors — every anchor lies in a covered space. Every
obligation reports its binding count; a contract of one all-region `open`
space is *green with a named finding*, never a silent pass.

Coverage is what makes run 1's failure unshippable: 1 034 gallery cells are
either declared (then edge-proved reachable, or `no_body`) or undeclared (red).
Closure is what makes the sky slot and the lip loud. Neither obligation asks
the machine to judge intent; both ask the author to state it once and let the
bytes disagree audibly.

### 3. What the contract feeds

- **Content binding**: campaign content attaches to declared spaces (anchors
  carry their containing space in exported metadata). The campaign-side
  diagnostic — content in no declared space, or in a space no unlock reaches —
  is a **follow-up fence**, not this ADR.
- **Assembly**: an edge whose far side is `exterior` is the piece's face
  contract — the thing §7 calls "convention rather than contract" — and is the
  natural carrier for jigsaw connector emission (queued work, unblocked, not
  delivered here). This restores ADR-0004's "layout validation reduces to graph
  properties over placed pieces" at both scales with one vocabulary, which is
  why ADR-0004 is extended and not superseded.
- **The `traversable` gate** becomes a derived claim over declared exterior
  edges rather than a face heuristic (also retiring the 47-of-which-3-are-doors
  approach-binding miscount, task #108).

### 4. What this ADR does NOT decide

- **No generation.** The contract constrains geometry; it never produces any.
  A blockout tool that emits graybox boxes would be a second geometry path —
  the fourth-mechanism shape the review doctrine names.
- **No inference.** Spaces are never read out of the voxels.
- **IR ergonomics are separate.** Parameterised/cross-program `call`
  (task #107), the local-direction `mark` facing, overlay, and the positional
  index are real R4 findings on a different layer (authoring cost, not
  checked truth). Each proceeds on its own spec under the ADR-0018 §7 fence.
  Bundling them here would be the escalate-to-a-bigger-mechanism reflex the
  trial itself falsified twice.
- **Sightline, density, and craft claims stay out.** The bell zones' Rust
  gates assert more than topology (blindness, exactly-one-tell, perch
  density); the contract does not replace those and must not try.

## Consequences

- The `Program` IR takes the contract surface behind a required version bump
  (ADR-0018 §7 discipline: fenced constructs, refuse-unknown). Old programs
  compile unchanged; the block bytes of a program that merely *adds*
  declarations do not move (wrapper transparency, asserted as `mark`'s is).
- Active-campaign adoption (CLAUDE.md version-adoption discipline): the eight
  bell zone programs get contracts in an adoption round scheduled within the
  same milestone; their `tests/zones.rs` topology assertions translate to
  contract data and the Rust suite's role narrows to proving the checker
  catches the same four recorded drifts. Island tileset pieces (hand-built,
  socketed, campaign-proof-covered) are upgrade-on-next-touch.
- Authoring order changes: procedure §1's scene description gains its
  machine-checkable half — the contract is written before the rules. Docs and
  the skill update in the same PR (tooling-sync rule).
- Cheapest falsifier first: **before any IR work**, a prototype checker runs
  the hand-written contract for run 1's saved artifact and for bell Z7. If it
  cannot red the stranded gallery, the notches, and Z7's four drifts — or if an
  honest cathedral contract needs a pathological number of boxes — the
  obligations are wrong at the cost of a day, not a milestone.

## Revisit triggers

- The second authoring trial (Stormveil-class wall section) ships a green
  artifact in which an independent probe still finds a stranded declared space
  or an undeclared opening — the obligations are unsound; reopen.
- Honest contracts prove writable only by falsifying classes or by blanketing
  `open` envelopes (the vacuity finding recurring in every trial) — the cost
  model is wrong; reopen.
- Overlapping (not merely abutting) spaces turn out to be required by real
  builds — the box model needs a semantics this ADR deliberately deferred.
