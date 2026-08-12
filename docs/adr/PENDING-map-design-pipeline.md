# ADR-PENDING: The spatial contract — declared spaces and edges, checked against the emitted bytes

- **Status**: Proposed
- **Date**: 2026-08-12 (amended same day against the prototype's evidence)
- **Source**: owner ruling 2026-08-12 ("the map-design pipeline needs restructuring,
  or our gameplay and our scenery cannot be unified"); trial-0001 (Notre-Dame),
  both runs; the planner's tree/graph analysis, amended below; the
  `tools/spike-spatial-contract` prototype (branch
  `feat/spatial-contract-prototype`, `./run-evidence.sh` reproduces)
- **Refines**: ADR-0004 (extends its "layout validation reduces to graph
  properties" consequence down into pieces; does **not** supersede it),
  spec-0027 (adds a gate family), ADR-0018 §7 (rides the `Program` version fence)
- **Constrained by**: ADR-0003, ADR-0006, ADR-0001, the general-engine rule,
  the no-hacks rule

## Context

Trial-0001 built a cathedral twice from the reader-facing docs alone. Both runs
passed every machine gate with non-zero bindings and both shipped structurally
broken artifacts. Verified on the saved artifacts:

1. **Stranded storeys.** Run 1: 4 982 standable cells, 2 113 reachable on foot
   from the ground entrance, **0 % reachable at every height above y = 3** —
   1 034 cells at gallery level, 216 on the tower decks. Run 0 identical in
   shape. `traversable` was green both times, correctly: it proves a
   ground-level walk from approach face to exit face, which is a claim about a
   *kind* of piece, and nothing obliged anyone to claim more.
2. **Silent envelope failure, larger than any early reading of it.** The
   design round's first probe read the run-1 transepts as a 2-cell seam
   overshoot; the prototype's closure obligation then measured the truth: the
   transept arms have **no side wall at all for eleven courses** (floor at
   y0 running x 2–28, nothing above until the roof caps at y12, both flanks,
   symmetric) — verified independently on the bytes. Two hand probes
   under-measured the same defect because both searched for a specific shape;
   the closure obligation binds to every boundary cell (1 668 over 8 spaces
   on this artifact) and does not need to know the shape in advance. That is
   the argument for the obligation stated as evidence. (`grammar.md` §2c
   idiom 5 documents the same class one scale down: a rounded split's
   unwritten thirteenth course, 27 cells of daylight, both gates green.)
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

The reading also under-covers one defect family. The transept flanks are not
graph edges; they are **envelope** failures — an unwritten or misplaced
boundary cell that no traversal claim would ever visit. A blockout graph alone
passes them. The contract therefore carries two kinds of obligation, not one:
connectivity (spaces, edges, reachability) and **closure**.

### What the prototype settled (2026-08-12, before any IR work)

The spec's own order of work put a prototype checker plus hand-written
contracts for run 1 and bell Z7 ahead of implementation. Results, all folded
into the Decision below:

- **Cost is bounded and reviewable**: 28 spaces / 21 edges / 84 lines,
  ~45 minutes, for an honest cathedral; 8 / 9 / 29 lines, ~15 minutes and
  fully green, for Z7. The real cost was not box count but the strict
  partition rule (every box face a wall or an abutting space's face), which
  drove four revision rounds — answered by union-of-boxes spaces (§1 below).
- **Topology obligations alone do not catch the bell drifts.** Of Z7's four
  recorded drifts, one is refused upstream and produces no bytes; the other
  three reach geometry and were green on every original obligation — a
  one-course seam moves the flight's head landing, `connected` steps ±1, so
  the walk holds. What `zones.rs` catches it with is a *height* assertion.
  The contract therefore carries a **declared level relation across an edge**
  (§2.4); with it, the prototype reds all three geometry-reaching drifts with
  both numbers named.
- **The cheap escape hatch is `no_body`, not the `open` envelope.** An
  all-`no_body` contract passed the original obligations in 26 lines on the
  broken artifact, with findings printed that gate nothing. Findings that do
  not gate are findings nobody reads: the vacuity rules now **red** (§2.7),
  and `no_body` carries an obligation of its own (§2.6).
- **Per-space reachability is one line away from vacuous** ("declare it all
  one space"); reachability is therefore **per-cell** within declared spaces
  (§2.5), which is also what run 1's honest contract actually reds on —
  coverage is green when the author honestly declares the gallery; the red is
  that its cells are unreached.

## Decision

### 1. The artifact: a **spatial contract**, owned by the prefab object class

A prefab — grammar-generated, hand-built, or ingested — may carry a spatial
contract: named **spaces** (each the union of one or more boxes claimed by
scope-bound declarations), typed **edges** between them, an **entry**
designation, per-space **envelope** declarations, and **`no_body`** regions
(standable cells deliberately outside play). The contract is *declared
intent*; a checker proves the delivered blocks agree with it. Nothing is ever
inferred from the block pattern — that direction is the folklore the no-hacks
rule forbids.

The capability sits on the prefab metadata type (`delvewright_schem::prefab`),
not on the grammar verb: the grammar program is **one authoring surface** for
it — declaration nodes claim *the scope's box*, never literal coordinates, so
a parametric program resolves its contract per expansion exactly as `mark`
resolves anchors — `delve-admit` is the surface for hand-built and ingested
pieces, and the exported metadata always carries the **resolved** contract of
the expansion that produced the bytes. The checker runs on delivered blocks,
so both routes get the same proof.

**A space is a union of boxes.** Multiple declarations claiming one name merge.
This is what lets a non-box room (the cathedral nave's stepped cross-section —
the ordinary case, per the prototype) be one space instead of a forced split
whose upper box has no standable cells and reds forever, and it is what
dissolved most of the prototype's decomposition cost. True overlap of two
*different* spaces remains refused; `no_body` regions may nest inside spaces
(required by §2.5 — `rafter_hall`'s intentionally unreachable perches are the
proving case).

**`exterior` is a face, not a node.** An edge naming `exterior` declares a
face contract (and feeds connector emission); it contributes **no
connectivity** — the reachability walk never routes through it. The prototype
showed the alternative is unsound: exterior-as-node made any two
exterior-doored spaces mutually reachable, so deleting Z7's stair edge stayed
green and `barred` gating was defeated.

Edge classes are mechanisms mapped to existing nav predicates: `walk`, `stair`,
`drop` (directed), `barred` (sealed now; connected through exactly the declared
bar region with it voided), `vision`. Every traversal edge additionally
declares its **level relation** — `rise`, checked as `min_y(b) − min_y(a)` in
the resolved boxes: 0 by default on `walk` (two rooms meet on one surface —
the seam claim the bell zones are built on), required and exact on `stair` and
`drop`. This is the amendment that makes the bell drift family expressible;
without it the contract is topology-blind to a one-course seam error, which
the prototype demonstrated on Z7.

A `barred` edge's declared bar region is, deliberately, the **region anchor**
that §7 of `grammar.md` records as inexpressible: one declaration serves the
proof and the campaign binding — the capability lands on the object (the
edge), not on the verb that first needed it.

### 2. The obligations (full statement in the spec)

Coverage (every standable cell in a space or `no_body`); closure (an
`enclosed` space's boundary is non-passable except declared openings and
abutting declared regions — with one named residual, spec §2.3); edge proof
per class **plus the declared `rise`**; **per-cell reachability** (every
standable cell of every space, minus nested `no_body`, reached from entry
with bars closed and drops directed); the **`no_body` obligation** (each
region declares `sealed` — its cells provably unreachable from entry — or
`open` — exterior decoration — with a per-region reason); anchors resolve to
the closed extent of a space or to a declared edge region; **vacuity reds** —
a zero binding on closure, edge proof or reachability is red, and a `no_body`
majority is red unless acknowledged per-region.

Per-cell reachability is what makes run 1's failure unshippable — the honest
contract is coverage-green and reachability-red, unreached cells counted.
Closure is what found the wall-less transept flanks. Neither obligation asks
the machine to judge intent; both ask the author to state it once and let the
bytes disagree audibly.

**On the drift the checker cannot see**: one of Z7's four drifts is refused at
expansion and produces no bytes. The prototype counts this against the
checker; this ADR does not. A refusal is the *stronger* channel — loud,
upstream, artifact-free — and a checker over delivered blocks is correctly
silent about artifacts that do not exist. The spec asserts the refusal stays a
refusal; and if a later change ever lets that drift build, the declared `rise`
is positioned to catch what the refusal no longer does.

### 3. What the contract feeds

- **Content binding**: campaign content attaches to declared spaces (anchors
  carry their resolved contract element in exported metadata). The
  campaign-side diagnostic — content in no declared space, or in a space no
  unlock reaches — is a **follow-up fence**, not this ADR.
- **Assembly**: `exterior` edges are the piece's face contract — the thing
  §7 calls "convention rather than contract" — and the natural carrier for
  jigsaw connector emission (queued work, unblocked, not delivered here).
  This restores ADR-0004's "layout validation reduces to graph properties
  over placed pieces" at both scales with one vocabulary, which is why
  ADR-0004 is extended and not superseded.
- **The `traversable` gate** becomes a derived claim over declared exterior
  edges rather than a face heuristic (also retiring the
  47-of-which-3-are-doors approach-binding miscount, task #108).

### 4. What this ADR does NOT decide

- **No generation.** The contract constrains geometry; it never produces any.
- **No inference.** Spaces are never read out of the voxels.
- **IR ergonomics are separate.** Parameterised/cross-program `call`
  (task #107), the local-direction `mark` facing, overlay, and the positional
  index are real R4 findings on a different layer (authoring cost, not
  checked truth). Each proceeds on its own spec under the ADR-0018 §7 fence.
- **Sightline, density, and craft claims stay out.** The bell zones' Rust
  gates assert more than topology and levels (blindness, exactly-one-tell,
  perch density); the contract does not replace those and must not try.

## Consequences

- The `Program` IR takes the contract surface behind a required version bump
  (ADR-0018 §7 discipline). Old programs compile unchanged; block bytes of a
  program that merely adds declarations do not move (wrapper transparency,
  asserted as `mark`'s is).
- Active-campaign adoption (CLAUDE.md version-adoption discipline): the eight
  bell zone programs get contracts in an adoption round scheduled within the
  same milestone; their `tests/zones.rs` topology-and-level assertions
  translate to contract data and the Rust suite's role narrows to
  sightline/tell/density claims plus checker-teeth fixtures. Island tileset
  pieces are upgrade-on-next-touch.
- Authoring order changes: procedure §1's scene description gains its
  machine-checkable half — the contract is written before the rules. Docs and
  the skill update in the same PR (tooling-sync rule).
- The prototype and its evidence script are kept as the fixture seed
  (`tools/spike-spatial-contract/`); its `[BEYOND SPEC]` rise check is
  §2.4's origin and is credited as such.

## Revisit triggers

- The second authoring trial (Stormveil-class wall section) ships a green
  artifact in which an independent probe still finds a stranded declared-space
  cell or an undeclared opening — the obligations are unsound; reopen.
- Honest contracts prove writable only by falsifying classes or by `sealed`
  `no_body` blankets that the unreachability proof happens to bless — the
  cost model or the `no_body` obligation is wrong; reopen.
- True overlap of two different spaces turns out to be required by real
  builds — the union-of-boxes model needs a semantics this ADR deferred.
- The closure residual (spec §2.3: sub-body visual breaches into declared
  `open` regions are render-review territory) recurs as an owner finding —
  it then needs a machine form, per the finding-to-diagnostic rule.
