# ADR-0020: The spatial contract — declared spaces and edges, checked against the emitted bytes

- **Status**: Proposed
- **Date**: 2026-08-12 (amended three times same day against the prototype's
  evidence rounds; step 1 — the declaration surface — dispatched after round 3)
- **Source**: the map-design pipeline needs restructuring, or gameplay and scenery
  cannot be unified; trial-0001 (Notre-Dame), both runs; the tree/graph analysis,
  amended below; the
  `tools/spike-spatial-contract` prototype (branch
  `feat/spatial-contract-prototype`, re-validated at `d3ce851`,
  `./run-evidence.sh` reproduces)
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
   on this artifact) and does not need to know the shape in advance.
   (`grammar.md` §2c idiom 5 documents the same class one scale down: a
   rounded split's unwritten thirteenth course, 27 cells of daylight, both
   gates green.)
3. **Duplication grows with scale.** 26 % of run 0's rules and 30 % of run 1's
   are copies of another rule once role names and call targets are erased
   (re-measured: 29 of 113, 44 of 145). Real, recorded, and
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

### What the two prototype rounds settled (2026-08-12, before any IR work)

**Round 1** (against the first draft):

- **Cost is bounded and reviewable** — an honest cathedral contract is tens
  of declarations, not hundreds; Z7 is 29 lines and green.
- **Topology obligations alone do not catch the bell drifts.** Of Z7's four
  recorded drifts, one is refused upstream and produces no bytes; the other
  three reach geometry and were green on every topology obligation — a
  one-course seam moves the flight's head landing, `connected` steps ±1, so
  the walk holds. The contract therefore carries a **declared level relation
  (`rise`) on every traversal edge**; with it, all three building drifts red
  with both numbers named.
- **Per-space reachability and exterior-as-node are unsound** (one line from
  vacuous; mutual reachability through the outside). Reachability is
  per-cell; `exterior` is a face, never a node.

**Round 2** (against the amended draft) is the structural finding of this
ADR: **the opt-outs were mechanically defeatable.** A 90-line script that
read the checker's own red cell-lists bought a full pass on the broken
artifact by declaring every unreached cell `sealed` and downgrading every
breached space to `open` — because `sealed`'s proof (unreachable) was
*entailed by the defect it existed to catch*, and `open` was an
unconditional exemption. Two more of the same character: an unconstrained
`via` was a closure exemption anywhere, and a union-merged space hid a seam
by having no internal edge to carry a `rise`.

The repair is one principle, now §0 of the spec and binding on every future
surface here: **an opt-out must be secured by a property the defect cannot
supply.** Concretely — `sealed` demands its own closure (walled off, not
merely unreached); `open` demands sky (a roofed room cannot be downgraded
out of closure); the new `posted` kind demands an anchor (the case
`rafter_hall`'s intentionally-unreachable perches prove necessary, which the
two-kind taxonomy could not classify at all); `via` demands its endpoints'
own shared boundary; and **a space is one floor** (standable span ≤ 2
y-levels), so a merge that would hide a seam is refused and every level
change crosses an edge that owes a `rise`. Both adversary scripts are kept
verbatim as permanent red fixtures.

On cost, stated against this ADR's earlier framing rather than for it:
union-of-boxes spaces **moved** the decomposition work rather than dissolving
it (spaces 28→17 and the nave's 936 phantom breaches → 0, but total boxes
43→58 and revision rounds 4→6). Unions are the fix for a case that had no
fix — a non-box room — not a cost reduction, and the ADR claims only that.

**Round 3** (sign-off round): §0 held — no third total mechanical defeat, and
the one-floor rule measured cheap (3 of 25 spaces, every repair one the spec
already names; on Z7 it *improved* the contract, the treads becoming the
stair edge's transit volume). Three findings, all folded in:

- **The author must never pick which demand applies** (§0's corollary). A
  15-line loop that tried every `no_body` kind and kept whichever passed
  bypassed `posted`'s anchor demand with one word. Kinds are now computed by
  the checker, strongest applicable; the declaration carries only a region
  and a reason.
- **Transit-volume standables are reachability targets**, or unreached cells
  can be re-hung on a stair edge as 1×1×1 vias.
- **The three-kind taxonomy was the real cost driver** — this ADR's own
  "falsifying kinds" revisit trigger fired, on ordinary stonework: 296
  cathedral cells (wall-heads, inter-buttress recesses, gable courses,
  apse-yard, cornices) fit no kind, and declaring them spaces manufactured
  four false closure breaches per true one (signal 533-of-533 → 338-of-1718;
  a gate read like an over-nagging linter loses its reader). Answered by a
  **fourth demand, not a loosening**: `facade` — every standable cell
  touched by the flood of exterior air — which strictly subsumes and retires
  `open`, is unavailable to any region nested inside a space (an enclosed
  interior can never reach it: its own closure proof guarantees no exterior
  air path), and which an interior stranding cannot supply. The stranded
  gallery still classifies as nothing and reds.

## Decision

### 1. The artifact: a **spatial contract**, owned by the prefab object class

A prefab — grammar-generated, hand-built, or ingested — may carry a spatial
contract: named **spaces** (unions of scope-claimed boxes, each one floor),
typed **edges** between them (`walk` / `stair` / `drop` / `barred` /
`vision`, each traversal edge carrying its declared **`rise`**), an
**entry** with an exterior traversal edge, per-space **envelope**
declarations that demand sky when they claim openness, and **`no_body`**
regions, each carrying a required reason and a **checker-computed** kind —
`sealed` (walled), `posted` (anchored), or `facade` (exterior air) — never
an author-picked one. The contract is *declared intent*; a checker proves the
delivered blocks agree with it. Nothing is ever inferred from the block
pattern — that direction is the folklore the no-hacks rule forbids.

The capability sits on the prefab metadata type (`delvewright_schem::prefab`),
not on the grammar verb: the grammar program is one authoring surface
(scope-bound declaration nodes, resolved per expansion exactly as `mark`
resolves anchors), `delve-admit` is the surface for hand-built and ingested
pieces, and the exported metadata always carries the **resolved** contract of
the expansion that produced the bytes. One checker, two doors.

A `barred` edge's declared bar region is, deliberately, the **region anchor**
that §7 of `grammar.md` records as inexpressible: one declaration serves the
proof and the campaign binding — the capability lands on the object (the
edge), not on the verb that first needed it.

### 2. The obligations (full statement and per-class detail in the spec)

Coverage (spaces, `no_body`, or a traversal edge's transit volume); closure
with sky-secured envelopes; edge proof per class plus the declared `rise`;
**per-cell, graph-confined reachability** (declared edges are the only doors
the walk may use — the physical-walk reading was rejected because it makes
edges decoration); the three-kind `no_body` obligation; anchors resolving to
closed extents or edge regions; vacuity reds (zero bindings on
closure/edge/reachability red; `no_body` majority red unless acknowledged,
and the acknowledgement never weakens the per-region proofs). The verdict
block enumerates every opt-out instance by name — envelopes, vision vias,
posted regions, opened-bar sets — the per-instance form a blind script
cannot satisfy and a reviewer actually reads.

**On the drift the checker cannot see**: one of Z7's four drifts is refused
at expansion and produces no bytes. That is not a checker gap; a refusal is
the *stronger* channel — loud, upstream, artifact-free — and a checker over
delivered blocks is correctly silent about artifacts that do not exist. The
spec pins both halves, so a later change that lets the drift build cannot
pass unnoticed. (Conceded by the prototype in re-validation.)

**Residual risk, named rather than smoothed** — three now, all of the same
character (the remaining discriminator is intent, surfaced by enumeration
rather than proved): `posted` decoy anchors are visible in every downstream
surface but visibility is review pressure, not a proof; an **exterior**
stranded terrace — a sky-open tower deck players were meant to reach — can
be waved off as `facade`, because to every mechanical fact available it *is*
a wall-head, and only the declared reason and the render review can
disagree; and a sub-body visual breach into an abutting `facade` region is
render-review territory. Any of the three recurring as an owner finding gets
a machine form then (finding-to-diagnostic rule).

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
  approach-binding miscount that counts any standable cell on a face, and so
  reports 47 approaches where 3 are doors).

### 4. What this ADR does NOT decide

- **No generation.** The contract constrains geometry; it never produces any.
- **No inference.** Spaces are never read out of the voxels.
- **IR ergonomics are separate.** Parameterised/cross-program `call`, the
  local-direction `mark` facing, overlay, and the positional
  index are real R4 findings on a different layer (authoring cost, not
  checked truth). Each proceeds on its own spec under the ADR-0018 §7 fence.
- **Sightline, density, and craft claims stay out.** The bell zones' Rust
  gates assert more than topology and levels; the contract does not replace
  those and must not try.

## Consequences

- The `Program` IR takes the contract surface behind a required version bump
  (ADR-0018 §7 discipline). Old programs compile unchanged; block bytes of a
  program that merely adds declarations do not move (wrapper transparency,
  asserted as `mark`'s is).
- Active-campaign adoption (CLAUDE.md version-adoption discipline): the eight
  bell zone programs get contracts in an adoption round scheduled within the
  same milestone; their `tests/zones.rs` topology-and-level assertions
  translate to contract data and the Rust suite's role narrows to
  sightline/tell/density claims plus checker-teeth fixtures. The round also
  carries the one library change the contract forced: **`rafter_hall`
  anchors every perch** (its alternation left five standable cells no
  campaign could address — unfinished surface by the contract's own
  worldview, and interior, so rightly not `facade`), with the perch
  renumbering and the Z5/Z7 gate counts it moves named in the round summary.
  Island tileset pieces are upgrade-on-next-touch.
- Authoring order changes: procedure §1's scene description gains its
  machine-checkable half — the contract is written before the rules. Docs and
  the skill update in the same PR (tooling-sync rule).
- The prototype, its evidence script, and **both adversary scripts** are kept
  (`tools/spike-spatial-contract/`) as the fixture seed; the `rise` check and
  the closed-`sealed` rule originated there and are credited as such.

## Revisit triggers

- The second authoring trial (Stormveil-class wall section) ships a green
  artifact in which an independent probe still finds a stranded declared-space
  cell or an undeclared opening — the obligations are unsound; reopen.
- A third mechanical-defeat script survives §0 — the principle itself is
  insufficient, not merely an instance; reopen rather than patch.
- Honest contracts prove writable only by falsifying declarations or by
  `posted`/`facade` blankets — the cost model or a kind's proof is wrong;
  reopen. (Fired once, round 3, on the three-kind taxonomy; answered by the
  fourth demand rather than a loosening. A second firing questions the
  taxonomy approach itself, not the missing kind.)
- The one-floor rule forces a genuinely terraced single room into an
  edge-chain that authors demonstrably fight — the merge rule needs the
  within-space rise alternative that was rejected here; reopen with that
  evidence.
- True overlap of two different spaces turns out to be required — the
  union-of-boxes model needs a semantics this ADR deferred.
- Either named residual (`posted` decoys, sub-body visual breaches) recurs
  as an owner finding — it then needs a machine form, per the
  finding-to-diagnostic rule.
