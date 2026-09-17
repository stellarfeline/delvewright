# ADR-0030: A designed place is drawn — the detail medium is an ordered list of solids the engine executes

- **Status**: Proposed (draft — a direction check; finalised against what is built)
- **Date**: 2026-09-17
- **Source**: the work products of two creator runs through `/new-delve` (one released campaign on the content repository's `main`, one unreleased), read against ADR-0018's third revisit trigger; `docs/research/what-a-creator-agent-writes-instead-of-the-grammar.md` carries the measurements and the outside sources.
- **Refines**: ADR-0018 §1 (the escape hatch at the grammar IR) — narrowed, not withdrawn. ADR-0022 stage 6 (detail per place) — its medium is named.
- **Constrained by**: ADR-0001, ADR-0003, ADR-0004 (the piece library and placement stand), ADR-0006, ADR-0012, the general-engine rule, the no-hacks rule.

## Context

### What was observed

Both campaigns that set out to build a *designed* castle — a specific building with a gatehouse, a hall, a chapel, wall walks — reached the same authoring method independently:

| | released campaign (content `7318202`) | unreleased campaign (creator's report) |
|---|---|---|
| hand-written generator | 2207 lines of Python | about 2100 lines of Python |
| grammar `Program` checked in | 10.7 MB | about 11 MB |
| `rules` / `params` in that program | 1 / 0 | not measured |
| nodes (`fill` / `void` / `split` / `mark`) | 16836 / 7471 / 4271 / 31 | not measured |

The generator of the first computes a column of courses for every `(x, z)` and merges equal neighbours; the second paints roles into a voxel grid with a drawing vocabulary of its own (`box`, `clear`, `room`, `crenels`, `flight`, `gable`, `pyramid`, `lancet`, `rose`, `machicolate`, `tower`) and then serialises the grid into a split tree. The measured program uses no rule call, no parameter, no `bind`, and `repeat` on 0 of its 4271 splits; the second was not parsed, and its generator's own description is the same encoding. The grammar is being used as a transport encoding for voxels.

Around that encoding the creator re-implemented, privately, things the engine owns: stair-shape derivation (the engine demands it, `DW0801`, and derives nothing); a walkability search, a fall-and-return search, a shortcut-length measure and a reach check, each a private copy of a judgement the compiler makes much later (`DW0525`, `DW0881`, `DW0374`); and a post-expansion step that writes gate *region* anchors into the prefab manifest by hand, because `mark` declares points only. The second campaign also abandoned its site plan and shipped the whole site as one piece, because the site-plan path could not produce a designed exterior — so ADR-0022's stages 4 to 6 were bypassed with it.

### This is sanctioned, and the sanction's own trigger has fired

ADR-0018 §1 permits exactly this: *a creator may compute a `Program` by any means at authoring time — Rust, Python, an LLM — provided the emitted `Program` is checked in as the artifact of record*; §6 makes the data normative and the generator provenance. Nothing above breaks a rule.

But ADR-0018 argued the hatch for *a genuinely one-off complex thing*, preferred the IR hatch over a frozen prefab because a `Program` can be seed-varied, composed, and seen by ADR-0015's promotion detectors, and named as a revisit trigger: **"creators come to need *computed* programs rather than written ones."** Two of two designed buildings are computed. A flattened partition of 28 000 literal leaves can be neither seed-varied nor composed, and shows a promotion detector nothing: every property for which the IR hatch was preferred is void in the way it is actually used. What remains is the cost side. The normative artifact is unreadable, so review happens against a script that §6 says has no standing; a diagnostic addresses a JSON path no author wrote; and the hatch has become the road.

### Why the grammar is not what a designer reaches for

spec-0027 adopted box-split grammars for a different job — a *library of seed-varied typologies* (the temple sweep, curated from a contact sheet) — on a stated bet: models are *semantically right and geometrically weak, so the model authors rules and the expander does geometry*. Both halves have moved. The job is now a bespoke site, where variation is unwanted. And the creator agent wrote two thousand lines of parametric geometry without being asked, which is not what geometrically weak looks like.

The mismatch is one of model, not of syntax. A box-split program **partitions**: every cell belongs to exactly one leaf and nothing paints over anything, so a rose window in a gable in a wall must be reached by one split tree that anticipates all three. A designer — human or model — **paints**: raise the wall, cut the window, lay the tracery over it; later work overwrites earlier work. Both generators are painters that compile to a partition at the end.

**Cited.** The mature form of this family agrees. CityEngine's CGA, the production descendant of split grammars, did not stay a pure split language: beside `split` and `comp` it carries geometry-creation operations (`roofGable`, `roofHip`, `roofPyramid`, `roofShed`, `primitiveCylinder`, `primitiveSphere`, `primitiveCone`, `insert`), 3D booleans (`union`, `subtract`, `intersect`) and occlusion queries (`inside`, `overlaps`, `touches`) — solids and overwriting, added to a splitter (Esri, *CGA reference*, operation index). Imperative, overwriting solid placement is also the idiom of the Minecraft generation framework the GDMC competition standardised on (GDPC, MIT: `editor.placeBlock`, `geometry.placeCuboid`, read from its README; ideas only, nothing taken). **From memory, unverified**: WorldEdit's region and shape commands are the same model. **Cited from an abstract only**: a 2026 benchmark of models writing voxel-building code reports that *producing executable code is far easier than producing spatially correct output, with geometric construction and multi-object composition particularly hard* (VoxelCodeBench) — which argues for the engine's spatial checks being answerable while the author draws, not for taking the drawing away. **Authored**: everything in the Decision.

## Decision

### 1. A place's detail is a **drawing**: data, ordered, executed by the engine

A drawing is a JSON document, schema-checked like every stage: a palette of roles, and an ordered list of operations over the place's box in its local frame. **A later operation overwrites an earlier one.** `delvec` executes it deterministically into the same `VoxelModel` the grammar expander produces, and from there the existing prefab path is unchanged — so every gate that reads delivered blocks (ADR-0018's own observation) binds to a drawing exactly as it binds today.

The creator's arguments are judgements — *which solid, where, how big, of what role*. Nothing else is typed.

### 2. The operations are solids, not building parts

`box` (solid, hollow, or faces), `clear`, `replace` (role for role inside a box), `cylinder` and `disc` (axis-aligned), `sphere` and `dome`, `prism` (a box tapering along one axis to a ridge: the gable), `pyramid` (tapering to a point or a cap: the spire, the batter), `flight` (a stair from one floor to another, of a declared pitch), `line`. Integer arithmetic only; the rasterisation rule of every curved solid is stated once and tested for byte-identity.

There is no `tower`, `crenels` or `machicolate` in the engine. Those are design decisions about what solids are *for*, and the general-engine rule puts them with the creator. They are written with §3.

### 3. Repetition and reuse are in the document, so no host language is needed

- `define` / `use`: a named, parameterised list of operations with its own local box, instantiated at a position, facing and size. Parameters are integers over the expression algebra the IR already has (`+ - * / % max min`); this ADR does not add a second one.
- `repeat`: an operation or `use` stamped along an axis at a stride, with the remainder rule stated by the caller.
- `mirror`: across a declared plane of the place's box.

This is the whole of what the two generators used Python *for*, once the painting is taken out: their loops are strides and mirrors, and their functions are parameterised stamps. Neither computes anything the algebra cannot.

### 4. What the engine knows, the creator does not type

- **Derived block state is the engine's.** Stair shape, and the connection state of fences, walls, panes and bars, follow from neighbours by the pinned game's own placement rule; the executor derives them after the last operation, from the pinned block-state registry. A drawing names `stairs` of a material and a facing; `DW0801` becomes unreachable from a drawing rather than a thing to satisfy by hand.
- **Anchors are operations.** `mark` (a point, a facing), `region` (a box: the gate a story opens or closes, with its closed and open fills), and `way` take their place in the list and land in the prefab metadata through the path that writes it today. Nothing edits a manifest after the fact.

### 5. The drawing lives inside ADR-0022, and the whole owns a shell

Stage 6 already hands a detail program its box, datums, palette and seams. A drawing is that detail program: `delvec allocation` hands it and `delvec detail` (spec-0058) binds, executes, gates and freezes it exactly as it does a grammar program today. An operation that leaves the box is a refusal at the operation that does it. What stage 6 could not give — the reason a site plan was abandoned — is the exterior that *crosses* places: the curtain wall, the roofline, the silhouette from the approach. That is the whole's, under stage 7's "connective dressing the whole owns": **one shell drawing per site, over the site's region, executed before the places' drawings, which overwrite it inside their own boxes.** Open question, to be settled by the first port: whether the shell may be overwritten *only* inside place boxes, or whether a place may declare a face it leaves to the shell.

### 6. The grammar stays for what it was adopted for

A drawing operation `grammar` expands a named rule into a box (a colonnade, a run of bays, a weathered ruin under a seed). The grammar does not call drawings. Seed variation remains the grammar's property; a drawing has no seed and is the same every time.

### 7. The spatial judgements answer during authoring

A creator agent wrote five private checkers because the engine's answers arrive after the whole campaign compiles. `delvec` answers the same questions on one drawing, with the engine's one rule for what a body can pass through (spec-0056) and no second implementation: which anchors are reached from an entry with a stated set of regions open; floor that can be fallen to and not walked back from; a sliced view of any sub-box. The surface is a spec under this ADR. The compiler's campaign-level judgements are unchanged and remain the authority; the query is the same code asked early.

### 8. ADR-0018 §1 is narrowed

The IR and prefab hatches remain, for what §1 argued them for: the thing that is genuinely one-off *and* that the drawing cannot say. The skill stops presenting a computed `Program` as a way to build a place, and no released campaign is owed anything by this (it is built only by the engine it pins). Whether the engine should *refuse* a partition-shaped voxel dump is **not decided here**: a gate that names a remedy owes a check that the remedy is reachable, and it is not reachable until §§1–4 exist.

## Alternatives considered

- **Run creator code inside the build (sandboxed WASM — ADR-0018 §1's deferred form).** Declined again, on new evidence: what the generators contain is vocabulary and strides, not computation. A sandbox would host a private drawing library per campaign — the defect, made deterministic.
- **Extend the grammar the way CGA was extended** — creation primitives and booleans at the leaves of the split tree. The honest competitor, and the cited precedent. Declined as the *primary* medium because partition-first is the part designers route around: both generators discard the split tree as a way of thinking and keep it only as an encoding. §6 keeps the door open in the direction that costs nothing.
- **Leave it: the hatch works and both castles shipped or built.** The cost is per campaign and recurring: two thousand lines of unreviewed script, five private checkers whose verdicts can disagree with the compiler's, an artifact of record nobody can read, and ADR-0022's pipeline bypassed. It also teaches: the second campaign cites the first as its method.

## Consequences

- A new document class and a `dsl_version` move; every operation is a schema unit and owes a gallery element, and the mechanic owes a demo-level row (`docs/demo-levels.md`).
- `docs/reference/` gains the drawing's live record; the `/new-delve` page's place-detail step is rewritten around it in the change that makes it work, and the `grammar.md` idiom index stops being the route to a designed building.
- **The experiment that is allowed to fail** (ADR-0018's precedent): port one part of the released castle — the gatehouse — from its generator to a drawing. Pass means: the executed drawing's blocks equal the generator's blocks cell for cell inside the part's box; the document is shorter than the generator's share of lines and readable in review; no script is needed to produce it; stair shapes and gate regions are typed nowhere. If the drawing is longer or needs a host language, that is the answer, and this ADR is withdrawn rather than Accepted.

## Revisit triggers

- A creator needs a solid that is neither in §2 nor a composition of §2 under §3 — decide it against that instance (ADR-0015).
- A place's drawing exceeds what one reader can review — the signal that §3 is too weak, not that a host language is needed.
- The first designed place built through a drawing still arrives with a generator script beside it.
