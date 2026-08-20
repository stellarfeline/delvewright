# spec-0047: The kind belongs to the cell — the out-of-walk classification re-bound to the blocks' own partition

- **Status**: Proposed
- **Question**: a zone program in the bell campaign (bell-r2, Z3 drowned ward)
  cannot declare its spatial contract because one out-of-walk region — the
  ruined west arcade, 160 standable cells — qualifies for **no** computed kind:
  the exterior flood reaches 159 of its cells but not the one standing inside a
  void the weighted ruin mix sealed at the shipping seed, and the region's own
  boundary opens to the sky, so `sealed` refuses it too. Both demands are
  right; the region is the wrong unit to ask them of. Every spec-0036 §2.6
  kind's demand already quantifies **per cell**; only the verdict was keyed to
  the declared region. This spec moves the classification to the cell and adds
  no surface at all.
- **ADRs**: 0020 (the contract is the checker), 0006 (determinism), 0018 §7
  (why no fence rides this — no document surface changes)
- **Specs**: 0036 (extended — §0 and all three demands verbatim; only §2.6's
  classification unit moves), 0043 (the sibling re-binding: a per-cell computed
  fact was bound to a per-space declaration there, to a per-region declaration
  here), 0042 and 0041 (untouched)
- **Non-goals**: any new kind, field, node, or document surface; the `facade`
  kind's own demand (spec-0043 §3's named residual, unchanged here); the
  `posted` anchor economics (spec-0036's stated soft spot, unchanged); the
  walkability floor predicate and the exporter's absent waterline (§5 — named
  interactions, fixed elsewhere); relaxing any demand; any campaign content.

## 1. The measured ground

Every claim below was demonstrated at engine `c9e1aaa4`. The instruments: the
checker itself over a distilled fixture (both contracts, same bytes), the
checker's own source (the classification is region-granular by construction in
`no_body_kinds`), and the zone's generation record in the campaign tree — three
readings whose failure modes are unrelated. The distilled fixture is a 25x8x9
world: an entry hall with a doorway, and a free-standing ruined pier — 55 deck
cells standable in exterior air, plus a two-cell void inside the masonry whose
lower cell is standable and which the piece's own blocks enclose on every side.

1. **One region over the pier is red.** `contract-no-body`: *"qualifies for
   NOTHING: its own boundary is not closed (not `sealed`) … the air outside
   the piece does not reach it (not `facade`)"*. This is the zone's refusal at
   fixture scale. On the zone itself the void is the mix's per-seed choice — a
   16-seed sweep is green at 15 and red at the shipping seed — so the geometry
   is right and the classification unit is what fails.
2. **The identical bytes, split along the computed facts, are green.** The
   deck as one region classifies `facade` (55 standable cells); the void as
   another classifies `sealed` (its own boundary is closed). Same blocks, same
   cells covered, opposite verdict: the verdict depends on where the author
   drew the boxes, which is a fact about authorship, not about blocks.
3. **The split is unauthorable where it matters.** A grammar region is claimed
   by rule scope boxes and its name is a literal — a program cannot carve a
   box around cells the weighted mix chooses per seed, because no rule owns
   such a box. And a contract carved from the checker's own red list is the
   90-line adversary's move even where it is expressible.
4. **`sealed` at region granularity is buyable for exterior scenery.** A probe
   region blanketing everything around the pier out to the model's edge — sky
   included — classifies `sealed` (1008 cells, every gate green), because a
   boundary at the model's edge counts as non-passable. The defect does not
   supply this (stranding buys nothing here, and the cells genuinely are out
   of play); it is a mislabeling hatch: flood-reached cells escape the
   `facade` cell-share enumeration §2.9 owes the reviewer, and their "closure"
   is supplied by the edge of the world rather than by the piece's blocks.
5. **The compatibility set is concrete.** Every committed contract-bearing
   zone program in the campaign trees classifies its out-of-walk regions
   `posted` or `facade` only; interior `sealed` regions exist in the engine's
   own fixtures (the walled recess) and are flood-disjoint. No committed
   contract carries a flood-touching `sealed` region.

## 2. The re-binding — the kind moves to the cell

The **region stays the unit of declaration**: its name, the author's `reason`,
the nesting license, coverage, reachability exclusion, and per-region
reporting all keep it. What moves is classification: the checker computes a
kind **per standable cell**, strongest applicable in the same order —
`sealed`, `posted`, `facade` — the author picking nothing, exactly as before
(spec-0036 §0's corollary). The demands are verbatim, each stated on the unit
it was always a fact about:

- **`sealed` (cell)**: the maximal passable component containing the cell lies
  wholly inside the union of declared out-of-walk cells **and touches no cell
  of the model's outer layer** — enclosed by the piece's own blocks on every
  side, never by the edge of the world. This is §2.6's own loop ("drop the
  guilty until the survivors' union is closed") run at cell granularity, plus
  the outer-layer clause §1.4 forced. A component holding any undeclared
  passable cell — a space's air, a transit volume, sky, the flood — fails it,
  cell by cell, cascade and all.
- **`posted` (cell)**: verbatim and already per-cell — within Chebyshev 2 of
  an anchor declared inside the cell's own region.
- **`facade` (cell)**: verbatim — the exterior flood reaches the cell, and the
  cell lies inside no declared space.

A region is green when **every one of its standable cells earns a kind**; a
red names the kindless cells and which demand refused each. The enumeration
reports the per-region breakdown by cell count ("55 standable cell(s)
`facade`, 1 `sealed`"), which preserves §2.9's obligations and sharpens one:
cells the flood reaches can no longer be counted under `sealed`, so the
`facade` cell share a reviewer reads is complete. Two consumers re-key with
it: the anchors gate's expectation (§2.7) becomes *the anchor stands among
`posted` cells*, and the majority gate counts the `posted` share over cells as
classified — its demand, including the acknowledgement's inability to buy a
`posted` majority, unchanged.

Under this, the zone's west arcade is one region with one honest reason. The
checker finds the deck in the exterior air and the void inside the masonry,
and says so, at every seed, without the author redrawing anything.

## 3. Why this is not the sixth vacuity mode

Each kind is an opt-out, so each owes §0's two questions: **what does this
kind demand, and could the defect itself produce that demand?**

- `sealed` demands enclosure of the cell's whole air component by the piece's
  own blocks, wholly within the declared out-of-walk cells. Stranding cannot
  produce it: a stranded cell is stranded *with respect to something that
  reaches its air* — the sky, the exterior, or play air through the breach —
  and each of those places a failing cell in the component. The observed
  defect — a mix sealing a void — **does** produce it, and that verdict is
  correct rather than leaked: a component no air path reaches is
  play-equivalent to solid rock, and what the mix cannot produce is a green
  over a cell a body can occupy in play, because occupiable means
  air-connected to play or exterior, which the demand excludes.
- `posted` and `facade` keep their demands and their existing security
  arguments unchanged; nesting still bars `facade`, and a decoy anchor still
  posts nothing beyond its radius.
- The effective per-cell obligation is the **disjunction** of the three, so it
  is as strong as its weakest member — which is why the kind stays computed,
  strongest-first, with no field to pick and no kind to shop. The weakest
  disjunct remains `posted`, whose cost (anchors are the campaign's exported
  namespace) is spec-0036's honestly-stated soft spot, not widened here.
- The author's one remaining lever was **box-drawing**, and §1.2 measured it
  flipping a verdict. Per-cell classification is box-drawing-invariant over
  the same covered cells: split or merged, the same cells earn the same
  kinds. The lever is dead in both directions — a split buys nothing, and a
  merge hides nothing.

## 4. Why this is not a weakening

No demand is relaxed; each is verbatim, evaluated on the unit it quantifies
over. Every kind-uniform region green today keeps its verdict and its kind:
`posted` and `facade` already demanded their fact of every standable cell, and
an interior `sealed` region's components lie wholly inside its declared cells.
The new greens are exactly the verdicts a legal split already buys (§1.2) —
granted now to the regions whose split no rule can author (§1.3). The one
relabel class is §1.4's flood-touching `sealed` blankets: their gate state
stays green and their flood-reached cells move into the `facade` enumeration,
which is a strengthening of what the reviewer is shown, not of what passes.
No committed contract is in that class (§1.5). All of this is asserted by
re-verdict, never argued (AC5); a divergence found there stops the PR.

## 5. Interactions with two open findings, named and not fixed

- **The floor predicate calls any non-air cell a floor**, so open water reads
  as standable ground. This spec neither depends on nor changes it: the kinds
  quantify over the same `standable`/`passable` answers at either granularity,
  and when that finding's fix moves the floor definition, every kind's
  population moves identically under region- or cell-binding. The fixtures
  here are dry stone precisely so this spec's measurements do not stand on
  that predicate's answer for water.
- **The exporter writes no waterline**, so the tide-plane diagnostic binds to
  zero over grammar-built zones. No interaction: the kinds read blocks, never
  that metadata field, and this spec neither narrows nor widens that zero.

## 6. No version fence, and what it costs

ADR-0018 §7 fences ride document surfaces; this spec adds none — no field, no
node, no keyword, no kind name. The version ledgers are untouched and this
spec claims no number in either. It obliges **no adoption round**: no
committed contract must change and none stops passing (AC5 asserts this by
re-verdict over the corpus and both campaign trees). What changes shape is
enumeration text (per-region kind breakdown), never a gate's state on the
compatibility set. What it unblocks is content: a zone whose out-of-walk
scenery mixes exterior cells with mix-sealed voids declares one region and
lands its contract — campaign work proceeding on its own branch, not an
adoption obligation.

## 7. Acceptance criteria — each stating what would make it vacuous

1. **The pier pair.** The distilled fixture under ONE region: red today
   naming "qualifies for NOTHING" (kept as the red fixture); green under the
   re-binding, the enumeration naming the breakdown (55 standable cells
   `facade`, 1 `sealed`). *Vacuous if* the void holds no standable cell or
   the deck none — either way one kind would never bind.
2. **Box-drawing invariance, both directions.** The same bytes under the
   one-region contract and the split contract yield identical gate states and
   identical per-kind standable counts. *Vacuous if* the two contracts cover
   different cell sets — the invariance would compare different populations.
3. **The stranded gallery stays dead at cell granularity.** The gallery slice
   declared out-of-walk still reds: its component holds undeclared passable
   cells (not `sealed`), no anchor posts it, the flood does not reach it and
   it nests (not `facade`); the red names the refusals per cell. *Vacuous if*
   the fixture's component lies wholly inside the declared cells — it would
   then witness a walled void, not a stranding.
4. **Sealed by blocks, never by the edge of the world.** The §1.4 blanket:
   no flood-reached cell classifies `sealed`; its standable flood-reached
   cells classify `facade` and appear in the `facade` cell-share enumeration;
   the gate state stays green. *Vacuous if* the region touches no
   outer-layer cell — the clause under test would never fire.
5. **Compatibility is re-verdicted, not asserted.** Every corpus program,
   contract fixture, and committed contract-bearing program in both campaign
   trees keeps every gate state, and every kind-uniform region keeps its kind
   on every cell. *Vacuous if* the population lacks any of the three kinds —
   assert at least one region of each bound.
6. **Determinism.** The double-expand suite extends over a mixed-kind region;
   the verdict is pure over (grid, resolved contract). *Vacuous if* the pair
   carries no mixed region — the new partition would never serialise.
7. **One checker, two doors.** `delve-admit audit` agrees with `expand` on a
   piece whose contract carries a mixed-kind region — same bytes, same
   resolved contract, same verdict and enumeration. *Vacuous if* the
   admit-side fixture's regions are all kind-uniform.
8. **Every new refusal arm is test-asserted** per the DW-coverage convention
   where it surfaces as a diagnostic; this spec claims no code numbers in
   prose. *Vacuous if* an arm ships allowlisted instead of asserted.

## 8. Order of work

1. Checker: the per-cell classification, the enumeration breakdown, the
   anchors- and majority-gate re-keys; the fixtures of §7 in the same PR.
2. `delve-admit` door parity (no metadata change — the contract block is
   carried whole).
3. Docs in the same PR: `grammar.md`'s `contract-no-body` row and the
   out-of-walk prose state the per-cell binding.
4. The motivating zone declares its contract on its campaign branch, at its
   shipping seed, with the region's breakdown named in that round's summary.
