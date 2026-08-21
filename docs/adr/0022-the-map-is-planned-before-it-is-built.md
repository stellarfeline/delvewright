# ADR-0022: The map is planned before it is built — the whole-first pipeline

- **Status**: Accepted
- **Date**: 2026-08-20
- **Source**: `docs/research/how-a-large-level-is-actually-built.md` — the
  research record and plan of record for the map pipeline; its appendix carries
  the primary sources (blockout practice, modular-kit discipline, engine world
  assembly, top-down generation, Minecraft build-team practice). Read against
  the three trials (`docs/trials/`) and the documents superseded below.
- **Supersedes**: ADR-0020 and spec-0040, in full, as decisions; the
  *composition consequences* of ADR-0004 (its decision stands — §1 below).
- **Constrained by**: ADR-0001, ADR-0003, ADR-0006, the general-engine rule,
  the no-hacks rule.

## Context

The superseded method designed a map's parts first: eight zone programs, each
self-regioned and reviewed against its own reference, composed afterwards by a
map program (spec-0040) with the spatial contract as the map-level checker
(ADR-0020). The first artifact in which the whole existed in any form was
measured after every part was finished. trial-0003 recorded the outcome: a
1:5.5 site against a compact brief, computable from the manifest alone before
any composition ran; zero of ten seams aligned by construction; the map's
region set to the arithmetic sum of the parts' pre-existing depths — extent
flowing up.

The research record establishes that these are the named failure modes of
professional practice, arriving on schedule: the industry's central artifact —
a blockout of the entire level, walked before any detail is built — had no
equivalent; per-part review before a whole existed is the art pass before the
blockout gate; parts that share no footprint quantum, standard opening or datum
convention cannot fit by construction. No source found defends assembling
independently designed parts as a way to obtain a coherent whole; parts-first
appears only where a topology pass has already allocated every part its slot.
spec-0040 §3c stated the right rule — extent flows down, the map program is
authored before the zones — but as prose, written after the parts existed, and
its own record shows the ordering bound nothing. An ordering obligation that
exists only as prose is not an ordering obligation.

## Decision

### 1. What is superseded, and what is void

- **spec-0040** (map composition) is superseded in full. Its central inference
  — that mutual consistency needs one medium, so the whole must be a grammar
  program composing the parts' programs — is not supported by the research:
  every whole-owns-space practice separates the whole's plan (a data artifact)
  from the parts' medium and couples them by contract. The one-derivation
  property it bought is bought instead by the blockout being derived from the
  site plan and the details being checked against the blockout.
- **ADR-0020** (the spatial contract) is superseded in full *as a decision*.
  The checker it built survives on this ADR's own ground (§4); its pipeline
  framing — contracts as the coherence mechanism over independently designed
  parts — does not.
- **ADR-0004's composition consequences are void**: obtaining a whole map's
  design by assembling parts, and layout validation as a property of the
  assembled result. **ADR-0004's decision stands** — the prefab `.nbt` library,
  compiler-controlled placement with compiler-owned seeds (its
  compiler-is-the-jigsaw amendment), determinism per ADR-0006. Its status is
  unchanged; this ADR narrows what it decides to the piece library and the
  placement mechanism.
- The geometry design built under these documents — the eight zone programs of
  `the-drowned-bell-r2`, its map program, the per-zone acceptance renders *as
  review authority*, and the per-zone concept images *as dimensional
  authority* — is void. The campaign's fiction, quests and whole-map reference
  views survive.

### 2. The architecture of record

The pipeline is a sequence of artifacts. Each stage's checks are specified by
their own specs; here they are named only by the property they hold. A
**place** is a node of the campaign's layout — room, courtyard, arena, shore —
the general unit. A **seam** is where two places connect.

- **Stage 0 — the metrics standard** (engine; once, then maintained). One
  machine-readable table: *player metrics*, measured facts of pinned 1.21.11;
  *building metrics*, standards this project fixes — corridor and doorway
  minima, stair pitches, storey heights, and the kit grid (footprint quantum,
  datum convention, standard seam openings). Calibrated by a walked
  metrics-gym demo level; tools and gates read the table as the single
  authority.
- **Stage 1 — fiction and mission** (campaign; exists). The staged DSL through
  the campaign quest plan, unchanged. Mission structure precedes and
  constrains space.
- **Stage 2 — the whole's reference and written geometric brief** (campaign).
  The multi-view reference sheet of the whole (style authority, rank-only) and
  a written brief stating the whole's numeric facts as checkable identities.
  No per-place concept art exists at this stage: imagery authored before the
  blockout is style authority, never dimensional authority.
- **Stage 3 — the layout graph** (campaign). The campaign's space as a graph
  before any coordinate exists: places with intent and size class; connections
  with class, gating and direction; entry, goal, critical path, loops; every
  quest beat bound to a node. Checked as a graph — reachability under gating,
  no softlock, beat coverage.
- **Stage 4 — the site plan** (campaign; the whole's design of record). The
  geometric embedding of the graph: the world region and its datums, a box for
  every node on the metric grid, a seam for every edge allocated on a shared
  face at a standard opening. Extent flows down — the region comes from the
  brief and boxes partition it; a part is never the authority for any total.
  Seams are allocated, not discovered. Checked for partition, per-seam
  geometric feasibility, the brief's identities, and exact graph agreement.
- **Stage 5 — the whole-map blockout, compiled and walked** (engine derives;
  a human walks). The whole map as massing, derived mechanically from the site
  plan and the metrics table — authored by no one — compiled by the existing
  placement machinery into a joinable world, deterministic. The full machine
  battery runs at map scale on every regeneration; then the human walk judges
  scale, pacing, route legibility and silhouette. This is the first walkable
  artifact, and the gate all detail work waits on.
- **Stage 6 — detail per place, inside the frozen allocation** (campaign; the
  unit of dispatch is the place). A detail program is handed, by the toolchain
  and not by prose: its box, datums, palette, seams, and the blockout's
  traversal contract. Within that it is free — kit pieces, the grammar's full
  vocabulary, per-place style anchors generated now against the whole's
  reference. Checked for traversal equivalence against the blockout; a program
  that wants different traversal is asking for a site-plan revision and a
  stage-5 re-walk — the part has no surface by which to move an allocation.
  Human review judges interior atmosphere on renders and cannot move geometry
  the contract freezes.
- **Stage 7 — whole art pass, full validation, release** (campaign; mostly
  exists). Connective dressing the whole owns, relight, composed render review
  beside the stage-2 reference, PackTest, the full bot playthrough, the
  release ladder from a frozen tree. The traversal-equivalence check runs once
  more over the dressed whole, so a decoration pass cannot strand a route.

### 3. The two properties the architecture exists for

- **The ordering is structural, not prose.** Each stage's artifact is the
  input the next stage's tool requires: a site plan validates only against a
  layout graph; the blockout is derived data, with nothing to author before
  the site plan exists; a detail program has no compilable form without a
  handed allocation and carries no region of its own — regions live in the
  site plan alone. The campaign audit walks the stage artifacts in order.
  Inverting the order is not a discipline failure to be caught; it is a state
  that cannot compile.
- **The first walkable artifact arrives early and is cheap to discard.** Stage
  5 is reachable with no authored-geometry work at all; regeneration is
  seconds; a finding edits the graph or the site plan and regenerates. Human
  judgement fires on a walkable whole before any detail exists to be sunk —
  the blockout economics the research documents, held by construction.

### 4. What survives, on its own merits

Kept because the research argues for each, not by deference to the superseded
documents:

- **The grammar language and its gates.** The research argues about *what is
  generated when*, never against generating geometry from a language; the
  stages fix the when.
- **The prefab admission procedure, the render loop, the assembled world and
  nav model, the placement machinery, the bot harness.** The
  machine-validation practice the strongest world-assembly sources arrive at;
  each is a tool the stages invoke.
- **The spatial-contract checker**, kept by argument: stages 5 and 6 need
  exactly a declared-intent-versus-built-bytes checker; its obligations were
  adversarially validated; and the property that the art pass preserves
  traversal must be held by a machine, because no one head persists across
  the sessions that author the parts. spec-0036 and the contract specs built
  on it are re-grounded on this ADR — that line's decision record now stands
  here, not on ADR-0020.
- **The piece library**, as the kit under the stage-0 metric standard. A piece
  that does not conform to the grid once its quantum is fixed is reworked at
  the footprint, not patched.
- **The checker-teeth and adversary fixtures**, which are engine tests. The
  zone-composition test suite dies with the zones it asserts.

### 5. The unit of authoring

The zone — a self-regioned, self-reviewed geometry document — ceases to be a
unit of anything: not of authoring, dispatch, review or acceptance. What a
campaign authors, in order: a quest plan, a brief, a layout graph, a site
plan, and per-place detail programs bound to handed allocations. The engine
authors the blockout. The place is the general unit — nodes, seams, boxes and
datums serve an open island, a village or a cave system identically; nothing
in the vocabulary is castle-shaped. Whether the built world is one contiguous
mass or several detached sites is a site-plan fact, not an engine distinction.

## Consequences

- **No migration path and no salvage pass** for the discarded geometry: a
  preserved finished part is a constraint the whole must accommodate — extent
  flowing up, the defect itself, re-entering as thrift. The campaign's
  geometry restarts at stage 2 with its reference views in hand; the per-zone
  concept images may inform stage-6 style anchors, and as dimensional claims
  they are void.
- **The stages imply specs** — the metrics table and gym, the graph and
  site-plan schema stages, the blockout derivation, the traversal-equivalence
  check, the audit bindings, and the supersession of the fixed-stride `areas`
  surface by placement from the site plan. Each is its own spec, written by
  its own round; this ADR deliberately stops at the architecture.
- **Order of build**: the thin vertical slice of stages 3→5 first, proven on a
  small fixture campaign, so a walkable whole exists within the first rounds
  and every later check deepens an already-walkable path. Detail-stage
  machinery follows. Building stage 6's machinery first is the superseded
  ordering arriving through the build order, and is refused.
- Reference docs and skills that describe the superseded surfaces update as
  the machinery replacing them lands, per the tooling-sync rule; until then
  they describe engine capabilities (program composition, the contract) whose
  role as the whole's method is what this ADR retires.

## Revisit triggers

- Walked blockouts repeatedly need hand-shaped massing to be judgeable — the
  derivation gains parameters, never hand edits; if parameters fail too,
  reopen the derived-blockout decision.
- Every layout-graph edit is immediately a site-plan edit and the agreement
  check between the two artifacts never fires alone — merge them into one.
- A campaign brief the place vocabulary cannot state without a castle-shaped
  workaround — per the no-hacks rule, a first-class surface or a refused
  feature, decided at that evidence; reopen here only if the vocabulary
  itself is wrong.
