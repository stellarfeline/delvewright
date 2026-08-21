# spec-0050: A place is detailed inside the box the whole gave it — stage 6 of the map pipeline

- **Status**: Accepted
- **Ground**: this spec is the successor spec-0049 names in its non-goals: it
  designs pipeline stage 6 (per-place detail programs, allocation handing,
  traversal equivalence against the blockout) and records the disposition of
  stage 7, executing ADR-0022 §2's stage-6 row against the stage-5 machinery
  as it actually landed (`crates/compiler/src/blockout.rs`, `crates/dsl/src/
  siteplan.rs`, the map-scale battery, the five-place fixture the bot walks
  end to end). Nothing here revisits ADR-0022; where this design departs from
  the research record's stage-6 prose, the departure is recorded in §10.
- **DSL**: stage 6 adds one campaign stage document, at `dsl_version`
  **0.15.0** — per-stage fenced in the settled shape (§13).
- **Grammar**: no new grammar-program surface. The detail unit uses the
  contract vocabulary as it stands (`1.7.0`); the reserved `1.6.0` surface is
  not consumed. No grammar-ledger movement.
- **Diagnostics**: codes **DW0841–DW0845 and DW0848** are allocated to this
  spec, all used below. DW0821 changes severity under §7 and keeps its code.
- **Non-goals**: retiring `areas[]` (unchanged from spec-0049); the walk's
  cadence and staging (operating practice); any campaign content; a
  whole-owned dressing surface beyond what exists (§9); jigsaw connectors.

## 1. The shape: a place is detailed by a piece, and the binding is not a placement

The unit that details a place is a **piece** — a prefab: frozen bytes plus
metadata carrying a resolved spatial contract, faces and anchors. A detail
*program* is a grammar program whose export produces one; a kit-library piece
admitted from other tooling is the same object. The engine consumes the
object class, never the tool that made it: `delvec` does not depend on the
grammar back end (generation-time tooling, ADR-0003), so the program→piece
step is the existing export/admission loop, and every gate below reads piece
metadata and bytes, indifferent to provenance.

The binding is a new campaign stage document, **`detail-plan.json`** (stage
name `detail-plan`), standard envelope, existing only at `dsl_version` ≥
0.15.0:

- **`palette`** (optional) — role → paint: the whole's material vocabulary,
  handed into every allocation (§4). Style surface; no gate reads it (§10.4).
- **`details[]`** — `{place, piece, anchors}`: `place` names a layout-graph
  node, `piece` names a prefab, `anchors` maps each synthesized anchor name
  the place owes (§6) to an anchor of the piece.

**That is the whole document.** It has no coordinate, no region, no extent,
no datum, no seam, no offset — not optional fields, absent fields. A detail
document is *structurally unable* to move its box, its datum or its seams
because the schema has no spelling for any of them; the only path from a
`details[]` row to placed bytes runs through the compiler computing the frame
from the site plan (§3), inside `Plan::build`, which is the only constructor
every world-reaching verb goes through. This is the same tooth as the
blockout's: inversion is not forbidden, it is uncompilable.

Detail is **per-place and partial by construction**: the derivation masses
every unbound box exactly as at stage 5, so a campaign with one detailed
place builds, walks, renders and reds like any other — the broken
intermediate is a real, lookable object at every point between "no detail"
and "fully detailed".

## 2. What the walk record gates, and what it cannot

`walk-record.json` keeps the form spec-0049 §5.4 fixed:
`{site_plan_sha256, blockout_sha256, engine_revision, verdict: "passed" |
"findings", findings[]}`. This spec lands its gate and the hashes it needs:
every build of a site-plan campaign prints `site_plan_sha256` (over the
plan's canonical bytes, so a reformat is not a re-walk) and
`blockout_sha256` (over the massing the walk judged — the derivation with
**nothing bound**, in deterministic order: a hash over the massing as
written would move on the first binding, and the drift warning below would
fire on every detailed campaign), naming the engine revision beside them.
The revision is stamped into the binary at compile time
(`DELVEC_ENGINE_REVISION`); an unstamped binary prints `unstamped` rather
than claiming one, because a run-time reading would need a `.git` a
published crate does not carry, and what the engine must never do is claim
a revision it does not have.

| Code | Rule |
|---|---|
| `DW0841` | **Detail without a passed walk of this plan.** A campaign carrying a `detail-plan` document refuses at validation unless `walk-record.json` exists, its `verdict` is `"passed"`, and its `site_plan_sha256` equals the current plan's. Missing, unparseable, `"findings"`, and stale are each named — a stale record's refusal prints both hashes. The same refusal guards `delvec allocation` (§4), so the two events that begin detail work — obtaining an allocation, compiling a binding — are both bound; there is no third entry point, because no other verb reads a `detail-plan`. Binding: the record checked and the hash compared, stated. |

A `blockout_sha256` mismatch alone — same plan, different massing bytes — is
a **warning naming both hashes and both engine revisions**, not a refusal.
The hatch question, answered: the defect this gate exists to catch is
detailing a plan the whole's walk never passed, and that defect moves the
*plan* hash — a campaign author has no edit that moves the blockout hash
without moving the plan hash, because the derivation is a pure function of
plan, metrics and engine. The warning path is reachable only by toolchain
movement (an engine or metrics change), which is a re-walk *decision* for
the round summary, not a defect the author could launder through it.

Stated plainly, as spec-0049 stated it for the record itself: the machine
half of this gate is **freshness and an explicit verdict**. That a human
actually walked is the record author's assertion, held by operating
practice; no engine check can prove a walk happened, and this spec does not
pretend one can.

## 3. The frame: what the piece owns, and what stays the whole's

Stage 5 fixed the fabric geometry: a box is **play space** (`PlacedBox::
space()`), adjacent boxes are separated by a one-cell party plane owned by
neither, seams are openings cut in that plane, and floors, walls, ceilings,
frames, stairs and bars are derivation-written fabric around the boxes.
Stage 6 splits that fabric on one convention:

- **The piece's frame is the box grown one course downward**: the play
  space plus the floor course its walk plane stands on. A floor's material
  is the place's own voice, and the datum convention already says the walk
  plane is the plan's — the handing states the datum in piece-local
  coordinates, and the seam-rise proofs hold it (§7). Where boxes stack, the
  horizontal party plane *is* the upper box's floor course and belongs to
  the upper piece — a seam frame lying in that course goes with it, like the
  rest of its floor; the derivation writes it only while the upper box is
  unbound.
- **Every vertical party plane, every unshared shell face, every seam
  frame in a vertical plane, every derived stair in an unbound host, and
  every bar in a vertical-plane seam stays whole-owned**, derived exactly
  as at stage 5,
  whether or not the boxes beside it are detailed. Interior wall treatment
  is lining inside the box — the piece dresses its side of the wall from
  within its own frame; the party plane is structure, and the whole's.
- For a `stair` seam whose `stair_in` box is bound, the derivation writes
  no stair massing there: the climb is the piece's to build, and the bytes
  battery proves it was built (§7).
- The plan's blockout `lighting` fixture pass applies to derived interiors
  only; a bound place lights itself — its cells go to the undeclared-darkness
  measurement, so a dark detailed place is a finding, never a silence.

In the derivation the split is one rule rather than a list: **a bound frame
is a hole in what the whole writes**. The floor accent, the interior clear,
the ceiling of the box stacked underneath, a hosted stair and a bar in the
box's own floor course are all writes that land inside that frame, so all
five stop by the same subtraction — and everything outside the frame is
written exactly as before. A list of exemptions is a list the sixth escapes.

The piece is placed at the frame, exactly:

| Code | Rule |
|---|---|
| `DW0842` | **The binding does not bind.** A `detail-plan` in a campaign with no site plan (the limiting case, naming the missing document); a `place` naming no layout-graph node; two `details[]` rows for one place; a `piece` the prefab library does not hold; an `anchors` key that is not a name this place owes (§6); an `anchors` value naming no anchor of the piece. Validation tier. Binding: details resolved, stated against the plan's box count. |
| `DW0843` | **The piece is not the shape of its allocation.** The piece's structure size differs on any axis from the handed frame — the refusal prints both extents, and *undersize refuses exactly as oversize does*: the box is the footprint, and a smaller building means a smaller box, which is a site-plan edit and a re-walk, taken visibly. Also under this code: a bound piece declaring no spatial contract — the equivalence instrument (§7) would have nothing to read, so a contractless piece cannot be a detail piece. Validation tier, metadata only. Binding: pieces measured. |
| `DW0844` | **The piece's openings are not the plan's seams.** Both directions, from metadata, before any bytes assemble: a seam this box must answer with no aligned face opening of a compatible class, and a face opening of the piece answering no seam — the discovered-seam refusal at the earliest tier. Alignment means the face's opening cells answer the seam's allocated cells across the plane (for a seam in the piece's own floor course, *at* those cells); compatibility is determined by the object, per the table below. Binding: seams required and faces examined, both stated. |
| `DW0845` | **An owed anchor has no standing.** An owed name (§6) left unbound by `anchors`; bound to a piece anchor that declares no cell (a region answers a gate, and a gate region is never owed by a place); or bound to one the piece's own contract resolves anywhere but play space — a `no_body` region, a bar, a transit or way volume. Validation tier. Binding: owed names per bound place, stated. |

The class-compatibility table `DW0844` judges with, keyed to the geometry
rather than chosen by anyone:

| plan seam | the piece's answering face |
|---|---|
| `walk` | `walk` |
| `stair`, this box hosts | `stair` (the treads are the piece's) or `walk` where the piece meets the opening at grade |
| `stair`, other box hosts | `walk` |
| `drop` | `drop` leaving, `walk` landing |
| `barred`, opening in a vertical party plane | `walk` — the bar stands in the whole's plane beyond the piece |
| `barred`, opening in this piece's floor course | `barred`, with the bar at exactly the allocated cells — the piece ships the gate's shut state |

`DW0844` is deliberately redundant with `DW0836`/`DW0838` and is not their
replacement: it reads declarations and names the piece and the seam at
validation; they read bytes at build and remain the independent observers.
A piece that lies in its metadata passes `DW0844` and reds on bytes.

## 4. The handing: computed on demand, proved by recomputation, never copied

**`delvec allocation <place>`** (and `--all`) emits the handed allocation as
JSON: the frame's extents; the datum in piece-local coordinates; every seam
of the box in piece-local coordinates with face, cells, class, rise and the
answering-face class the table above requires; the owed anchor names; and
the detail plan's `palette`. It refuses without a passed, fresh walk record
(`DW0841`) — this is spec-0049 §8's "the handing refuses a missing or stale
record", bound to the event that starts detail work.

The output is derived from the site plan on every invocation and is **not an
input to anything**: no gate, no build step and no check ever reads an
allocation file. The proof of the hand-off is that `DW0842`–`DW0845` and the
bytes battery recompute every obligation from the plan itself at every
validation — a committed allocation file is a copy with no consumer, so its
staleness has no vector into the build. The exported file exists for the
authoring loop only: a program is written against it, and the export/render
loop iterates against it, on the creator's own machine.

The `palette` is handed the same way and **gated by nothing**: materials are
style, style review is rank-only (spec-0028), and a piece exported against a
stale palette is a render finding, not a machine one. The provenance row
already freezes what the piece was actually built from.

## 5. The library half: a piece states its class, and the statement is checked

spec-0049 §9.4 deferred the kit-piece footprint check to the stage that
first consumes pieces. This is that stage:

| Code | Rule |
|---|---|
| `DW0848` | **A piece's declared footprint class disagrees with its bytes.** Prefab metadata gains optional `footprint_class`, naming a metrics-table size class (`DW0812` refuses an unknown name, as for any document naming a table entry). A piece declaring one is refused when its structure size could serve no box of that class: horizontal extents off the class's range or off the kit grid (`q`), height under the class clearance plus the floor course. Raised at `delve-admit audit` — the admission event, where the library's integrity lives — and again when a `detail-plan` consumes the piece, so a pre-check-era piece cannot be consumed unjudged. The field stays optional for the library at large; a piece bound by a `details[]` row is checked whether or not it declares (frame equality in `DW0843` is the consumer's exact check; `DW0848`'s declared-class half binds only where declared). Binding: pieces declaring a class, stated against pieces examined. |

## 6. The owed anchors: the campaign's names survive detailing

A site-plan campaign's quest layer names `synthesized_anchors` — and those
names were bound to nodes at stage 3, before any detail existed, so
detailing a place must never force a quest edit. The names a place owes are
exactly the synthesized names whose bearer is its box: its `anchor/node-…`;
the entry anchor (`spawn`) when it is the entry node; each `anchor/unlock-…` whose
`opens_from` side it is. Gate regions (`anchor/seam-…`) over vertical party
planes are never owed — they are whole fabric — and one over a bound upper
box's floor course resolves to the seam's allocated cells as ever, which
`DW0844`'s barred row has just required the piece to bar.

The `anchors` map re-binds each owed name to a piece anchor, so a kit piece
keeps its own vocabulary and a campaign keeps its own; `delvewright_dsl::
siteplan::synthesized_anchors` stays the one authority for what is owed, and
resolution places the campaign name at the piece anchor's resolved position.
`DW0842` refuses a map that names wrongly; `DW0845` refuses an owed name
with no standing.

## 7. Traversal equivalence, precisely

**The property**: detailing a place must not change whether the map is
walkable. **The instrument**: the stage-5 battery plus the piece's own
contract gates — not a comparison against the blockout's bytes. `DW0836`,
`DW0837` and `DW0838` were built as observers of *bytes against the plan*,
sharing no arithmetic with the derivation; they run unchanged over a world
with pieces standing where massing stood, and they are the equivalence
check. What "equivalent" means, exhaustively:

**Preserved — a violation refuses:**

1. Every seam's opening is passable at exactly its allocated cells, at its
   declared rise, and nowhere else on the shared wall (`DW0836`, bytes;
   `DW0844`, metadata, earlier and named).
2. No crossing between places exists outside an allocated seam (`DW0838`) —
   a piece cannot add a way out, and a hole it opens in its own boundary
   toward a wall is a refusal, not a discovery.
3. Every place's floor is reached under the gating closure (`DW0837`), and
   within a bound place, every standable cell of the piece is reached per
   its own contract (`contract-reachability` and the other contract gates,
   run where they already run — export and admission — which `DW0843`'s
   contract-presence refusal makes unskippable for a detail piece).
4. The brief's identities hold over the built bytes (`DW0833`, second call
   site, unchanged).
5. The bot walks the critical path end to end on the same harness and
   export.
6. Sightlines: `DW0821` stays an advisory while any box is unbound and
   becomes a **refusal when `details[]` binds every graph node** — the
   promotion spec-0049 assigned to this spec, keyed to a computed fact
   about the artifact rather than a flag an author sets. A fully detailed
   map asserting a vista owes the vista; the fix is a plan edit, a piece
   edit or the whole's own carving, all authorable in the same campaign.

**Free to change — deliberately, and no check may bind them:**

- The interior standable set and route structure within a place: partitions,
  stairs, lofts, pits — the blockout's floor plan is massing, not a
  template, and equivalence binds to the *allocation*, never to the
  scaffold's cells (§10.1).
- Materials, light (relight re-runs), and everything rank-only review
  judges.
- Anchor positions within their box (§6).
- The pacing measurement: `DW0822` re-prints at both sites, thresholdless
  as before — a detailed route measuring longer than its massing is the
  coefficients' calibration data, not a finding.

## 8. What makes the ordering structural, enumerated

1. **Detail before the walk**: `DW0841`, at both entry points (§2, §4). The
   record's hash is the site plan's, so a plan edit re-opens the gate by
   itself.
2. **Detail before the plan**: a `detail-plan` validates only against a site
   plan (`DW0842`'s limiting case), and a `details[]` row without a box has
   no frame to be checked against — there is nothing to author early,
   because the document cannot state where anything goes.
3. **A part moving its allocation**: unrepresentable (§1). The escalation
   path a part that *wants* different traversal takes is the one ADR-0022
   names: a site-plan revision, which moves the plan hash, which re-opens
   `DW0841`, which re-runs the whole's walk. The cost is stated, not hidden.
4. **The content repo's campaign audit** owes the same gate on every push:
   a `detail-plan` present without a passing walk record for the current
   plan reds the campaign — the same event-bound shape as spec-0049 §7's
   audit row. **That binding lives in the content repository and is still
   owed**: it lands with the engine-pin bump that first lets a campaign
   there carry a `detail-plan` (§13's adoption rule), and until it does,
   the two engine-side entry points above are the bound gates.

## 9. Stage 7, discharged — a finding, not a further spec

spec-0049 §8 held stages 6 and 7 for one following spec. On the tree as it
stands, stage 7 does not warrant a spec, and this section records why rather
than deferring it again:

- Its one novel obligation — *the traversal-equivalence check runs once more
  over the dressed whole, so a decoration pass cannot strand a route* — is
  already held **by construction**: the battery runs inside emission over
  the same world model every other proof uses, "edits and relight included",
  and dressing has no path into a shipped world that does not pass through
  that function. There is nothing to bind that is not bound.
- Its surfaces exist: whole-owned dressing enters through the existing
  world-edits stage against the site area, relight is the existing pass,
  the composed render review uses the plan's `views[]` beside the stage-2
  reference as at stage 5, and PackTest, the bot and the release ladder are
  unchanged. Where the edit surface proves unable to reach something the
  whole's dressing legitimately owns, that is an ordinary
  capability-gap finding against an existing surface, triaged by the
  standing rule — not new architecture.

## 10. Departures from spec-0049's stage-6 prose, recorded

1. **Equivalence is proved against the plan, not against the blockout's
   bytes.** "Keeps every blockout-reachable region reachable" is realized
   as "the plan's own proofs hold over the substituted bytes" (§7): a
   cell-for-cell comparison with the massing would freeze scaffolding
   accidents into obligations and forbid exactly the interior freedom stage
   6 exists to grant. The blockout remains what the walk judged; the plan
   remains what everything is checked against.
2. **The fabric split (§3).** The research's kit practice puts walls inside
   pieces; stage 5 landed party planes owned by neither box, so this design
   keeps structural fabric whole-owned and gives the piece its play space
   plus its floor course. Interior wall treatment is lining; exterior and
   connective fabric is the whole's, which is where ADR-0022 put the
   silhouette's authority anyway.
3. **The detail unit arrives as a frozen piece, not a program the engine
   expands.** Forced by the settled dependency architecture: the engine
   never depends on the grammar back end. The program is the authoring
   medium; the piece is the interchange object; provenance rows keep the
   two honestly joined.
4. **The palette is handed, not gated** (§4): materials are style, and
   style authority is rank-only by standing decision.
5. **`DW0821`'s promotion is keyed to full binding** (§7.6), not to a
   stage marker or an author flag — the severity is computed from the
   artifact, so there is nothing to set and nothing to forget.

## 11. The hatch question, answered for the whole design

Opt-outs this spec creates: **none.** A place is bound or unbound — the kind
is determined by whether a `details[]` row exists, not chosen among demands;
an unbound place gets the full derivation and the full battery, a bound one
gets the full handing checks and the full battery. There is no
acknowledgement field, no exemption list, no severity an author selects.
The two soft edges, each secured by a property the defect cannot supply:
the walk record's freshness hash (§2 — the defect moves the plan hash, which
is the one thing a fabricated-but-fresh record cannot survive a plan edit
with) and the blockout-drift warning (§2 — reachable only by toolchain
movement, not by any campaign edit). The piece's own contract hatches
(`no_body` kinds, the majority ack) keep their existing securements and gain
no new ones.

## 12. The general-engine test

Nothing here is delve-shaped or castle-shaped. A **piece filling a handed
box** serves a village house on a lane grid, a cave chamber in a massif and
a ship's deck on an open sea identically; the owed-anchor rule is "the
campaign's names survive detailing", which any quest-bearing game wants; the
walk-record gate is "a whole is judged before its parts are spent", which is
the industry practice the research documents, engine-agnostic. `palette`,
`footprint_class` and the compatibility table are all metrics-table and
contract vocabulary a creator re-fixes for their own fiction. Falsifier,
carried forward: the first campaign brief a bound-piece-per-place cannot
state without a workaround — several pieces wanting one box, or a place
wanting to be half-derived — is the evidence, and per the no-hacks rule the
answer is a first-class surface or a refused feature, decided on that brief.

## 13. Version discipline

- **`dsl_version`**: one bump, to **0.15.0**. Per-stage fence in the settled
  shape: a campaign below it cannot carry a `detail-plan`, and no document
  below it moves by a byte.
- **Adoption**: no released campaign carries a site plan, so no released
  artifact is touched. Every in-development site-plan campaign adopts per
  the standing rule — for the campaign in flight, the obligation lands
  exactly when it begins stage-6 work, which is the work it was waiting on.
- **Grammar**: no ledger movement (§ front matter).
- **Prefab metadata**: `footprint_class` is additive and optional; absent
  means what absence means everywhere in that file — the claim is not made.

## 14. Gallery, demos, docs

- Every schema property and enum variant of `detail-plan`, and
  `footprint_class`, become coverage units on landing (`schema --stage all`
  authority). The site-plan overlay binds them: one place bound to a small
  piece generated at build time (the gallery's piece is generated, never
  committed), its walk record committed beside the overlay and regenerated
  with it. Committed probes bind the refusals in the probe form: a
  `detail-plan` with no walk record (`DW0841`) and a piece one cell too
  deep (`DW0843`) each produce exactly the machine refusal the hatch
  demands.
- The mechanic's demo row: a two-place site plan, one place bound, walked
  beside its massing twin — queued in `docs/demo-levels.md` by the
  implementation PR, per the standing rule.
- `docs/reference/compiler.md` carries the `detail-plan` stage table, the six
  DW rows and the `DW0821` severity change; `docs/reference/tools.md` carries
  `delvec allocation`; the prefab-procedure and skill workflows carry the
  handing step — same PRs as the surfaces, per the tooling-sync rule.

## 15. Order of work

1. Hashes and the walk record: build prints both hashes; the record schema;
   `DW0841` at validation; the audit binding (§8.4).
2. The `detail-plan` stage, the frame computation, placement, and
   `DW0842`/`DW0843`; the derivation learns to skip what a binding owns
   (§3).
3. Faces against seams (`DW0844`), owed anchors (`DW0845`), and the
   `DW0821` promotion.
4. `delvec allocation`, the palette hand-through, `footprint_class` and
   `DW0848` at both events.
5. Fixture: the blockout fixture gains a bound place and its walk record;
   the gallery overlay, probes, demo row and docs close it out.

Each round lands with its tests, catalog rows, gallery elements and doc
updates in the same PR. The first bound place builds at the end of round 2;
every later round deepens a map that already substitutes.

## 16. Acceptance criteria

Machine-checkable; each names its verdict's instrument.

1. Every build of a site-plan campaign prints `site_plan_sha256` and
   `blockout_sha256` with the engine revision; two builds print identical
   hashes; a plan edit moves both, and edits outside the plan document —
   stage 1, the layout graph — move neither: the plan hash is a function of
   the plan document alone, which is what §2's hatch argument rests on. A
   massing that moved under an unchanged plan is §2's warning, never a
   refusal — demonstrated against a record naming a different blockout hash.
2. `delvec schema --stage all` includes `detail-plan`;
   `tools/check-gallery-coverage.py` is green with every new unit bound in
   the gallery domain or refusal-proven; the `DW0841` and `DW0843` probes
   are committed and red.
3. Every code in DW0841–DW0845, DW0848 has at least one test asserting it
   and a fixture the compiler (or `delve-admit`) refuses with it;
   `tools/check-dw-codes.py` is green in both directions with zero new
   allowlist entries.
4. On the blockout fixture with one place bound and a fresh passed record:
   `delvec build` exit 0; double-build byte-identity holds; the battery's
   binding line states its counts; and the export the bot is driven by
   (`critical-path.json`) equals the unbound fixture's in every step field
   but one `pos`, moved inside the bound place — traversal equivalence
   demonstrated as a passing battery, not asserted. The playthrough itself
   is the bot tier's, which runs on release candidates: **the bot walk of a
   detailed build is owed on the first release ladder that carries one**,
   and is not claimed here.
5. Deleting the record, or editing the site plan without re-recording, reds
   `DW0841` naming the hashes; `delvec allocation` refuses identically —
   both entry points demonstrated.
6. A piece whose structure size exceeds its frame by one cell on any axis
   is refused `DW0843` naming both extents; one cell smaller is refused
   the same way.
7. A piece with its seam-side opening sealed in metadata reds `DW0844` at
   validation; the same defect forced past metadata (a perturbed piece
   whose metadata lies) reds `DW0836`/`DW0838` on bytes — the two
   observers proven independent, in the spec-0049 §13.8 manner.
8. An owed anchor left unbound reds `DW0845`; a graph node bound twice
   reds `DW0842`.
9. With every node bound, a blocked sightline is exit-red; with one node
   unbound, the same world is a warning — `DW0821`'s promotion
   demonstrated in both directions.
10. `delve-admit audit` refuses a piece whose `footprint_class` its bytes
    contradict (`DW0848`); the same piece bound in a `detail-plan` is
    refused at validation.
11. Every new check's output states a binding count with its denominator;
    `tools/check-stated-counts.py` and the docs job are green;
    `docs/reference/compiler.md` and `tools.md` carry the new rows and the
    new verb.

## 17. Not settled here

- **Walk cadence** — unchanged from spec-0049 §14, still decided with the
  first campaign's revision data.
- **Several pieces per place, or a half-derived place** — excluded until a
  campaign brief demands it (§12); the falsifier is the brief.
- **A whole-owned dressing surface richer than the existing edit verbs** —
  stage 7's only open question, an ordinary capability-gap finding when a
  campaign hits it (§9).
- **Whether `footprint_class` becomes mandatory for library admission**
  once the kit grid is calibrated and the library audit (spec-0049 §14)
  has numbered conformance — decided on that count, not before.
