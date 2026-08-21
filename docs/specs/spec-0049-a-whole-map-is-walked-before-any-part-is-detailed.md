# spec-0049: A whole map is walked before any part is detailed — the vertical slice to a derived blockout

- **Status**: Accepted
- **Ground**: this spec executes §2 of the map-pipeline research record
  (`docs/research/how-a-large-level-is-actually-built.md`, the plan of record),
  covering pipeline stages 0, 3, 4 and 5 — the thin vertical slice its §2.10
  orders built first. spec-0040, ADR-0020 and ADR-0004's composition
  consequences are void as decisions (their supersession is ADR-0022, a
  separate document that records stage-level architecture only); nothing here
  is derived from them, and there is deliberately no migration path from the
  artifacts built under them.
- **DSL**: the implementation lands at `dsl_version` **0.13.0**. This spec
  states the number; the ledger moves in the implementation round.
- **Diagnostics**: codes **DW0812–DW0839** are allocated to this spec.
  DW0815 and DW0823 are left unassigned as slack for findings inside the
  block during implementation; they are not used below.
- **Vocabulary**: "pipeline stage N" below is the research record's numbering
  (0 metrics, 1 mission, 2 brief, 3 graph, 4 site plan, 5 blockout, 6 detail,
  7 art/release). It is unrelated to the numbering of the DSL's stage
  documents; new campaign documents are named, never renumbered into the
  existing 1–7 sequence.
- **Non-goals**: pipeline stage 6 (per-place detail programs, allocation
  handing, traversal equivalence against the blockout) and stage 7 (whole art
  pass) — the next spec's subject, not designed here; retiring `areas[]` for
  existing campaigns (both placement authorities stay legal at 0.13.0, one per
  campaign — §7); the building-metric **values** (the gym walk calibrates
  them — §3.3); the kit-piece footprint-class admission check (deferred with
  its reason — §9); any campaign content.

## 1. The shape of the slice

Three authored artifacts, one derived one, in a chain where each is the input
the next tool requires:

```
metrics table (engine)         geometry brief facts (campaign)
        \                                 |
         \        layout graph (campaign) |
          \               |               |
           \        site plan (campaign) ←┘   ← identities bind plan to brief
            \             |
             └──→  blockout (DERIVED — authored by no one)
                          |
                 assembled world → full battery → joinable server → the walk
```

The design constraint carried throughout, from the research's economics and
from this project's own gate structure: **the first end-to-end walkable thing
exists early and is cheap to throw away.** A campaign reaches a joinable,
bot-proven whole map from four JSON documents and zero authored geometry, and
a site-plan revision regenerates it in seconds. Detail work is downstream of
the walk and out of scope here.

Two rules from the research are obligations of this design, restated where
they bind: **extent flows down** (§5.4) and **seams are allocated, not
discovered** (§5.5, §6.3).

## 2. Stage 0 — the metrics standard

### 2.1 The table

One machine-readable metrics table, engine-owned data, in two halves whose
epistemic status differs and is recorded per entry:

**Player metrics** — facts of pinned Minecraft Java 1.21.11, measured, never
chosen. The entries and their values already live in the engine as the nav
model's constants: the collision box (0.6 × 0.6 × 1.8; 1.5 crouched), eye
height 1.62, the step rule (walk-up ≤ 9/16, jump rise ≤ 20/16, > 20/16
impossible), the jump arc (≈12 airborne ticks against ≈4.6 ticks/block on the
flat — the basis of the A* elevation weight), fluid impassability, and the
fall-damage onset (falls above 3 blocks damage; the survivable ceiling is a
function of health and armor). Stage 0 does not re-measure any of this; it
**collects** it into one exported table.

**Building metrics** — standards this project fixes, each carrying a
`calibrated` flag (§2.3): minimum corridor width and clearance; the **standard
seam opening set** (named openings, each `width × height` — the provisional
set includes a 1×2 door slot, a 2×3 arch, the 3×3 passage the jigsaw socket
seal already standardizes, and one broad opening for vehicles of scenery
scale); **stair pitch standards** (named rise:run patterns with their
realization — stair-block 1:1, slab-ramp 1:2 — chosen for comfort, not merely
legality); storey heights; the **size-class ladder** (named classes, each with
a min/max interior footprint range and a minimum clearance — the vocabulary
graph nodes declare); the **drop policy** (the maximum rise a designed `drop`
edge may fall — a policy cap, distinct from the physics fact beside it); the
**pacing coefficients** (blocks of route per minute of play — carried as
numbers with **no threshold** anywhere until calibrated, per §4.3); and the
**kit grid**: the footprint quantum `q` (provisional 4), the rule that box
extents are multiples of `q` on the horizontal axes, and the datum convention
— a box's floor **surface** is at its declared datum `y`; whatever stands in
the box later puts its walk plane there.

Every provisional value in this spec is a seed for the gym, not a standard.
This document deliberately fixes the table's **shape and mechanism** and not
its numbers.

### 2.2 One authority, exported

The table is one Rust module in the engine, exported as JSON by a new CLI
surface — `delvec metrics` — exactly as `delvec schema` exports the stage
schemas, with a `metrics_version` field. The single-authority obligation is
**structural, not disciplinary**: the nav model's constants are re-exported
*from* this module (one definition; the nav model imports it), so the player
half cannot drift from the model that proves routes, and every check this
spec adds reads the module — no gate below states a number of its own.
Tools outside the engine (the content repo's audit, future creator tooling)
read the JSON export, never a copy.

### 2.3 Calibration and the metrics gym

Building-metric values cannot be cited — nothing transfers from other
engines' units, and Minecraft's 1-block granularity at player scale makes
minimum-width choices coarser than any published standard — so they are
**calibrated by walking**. Each building entry carries `calibrated: bool`,
false at first landing.

The **metrics gym** is a demo level (a row in `docs/demo-levels.md`, queued by
the implementation PR that lands the table) and it is itself a site-plan
campaign: a generator emits a geometry brief, a layout graph and a site plan
that instantiate each building metric at its candidate value and the
neighbouring values (corridors at width 1/2/3, each standard opening and its
neighbours, each stair pitch, each storey height, one room per size class at
its bounds), built by the ordinary stage-5 derivation. Walking it once
in-game fixes the numbers: the walk's rulings edit the table, flip
`calibrated`, and the gym — regenerated from the table — becomes the
standard's living documentation. Generating the gym **from** the table is
what makes it incapable of drifting from what it documents.

The gym is dogfood by design: it is also the first site-plan campaign the
slice builds, so the calibration walk and the first exercise of the whole
machinery are the same hour.

### 2.4 Stage-0 checks

| Code | Rule |
|---|---|
| `DW0812` | **A document names a metrics entry the table does not define.** A graph node's `size_class`, a seam's `opening`, a stair seam's `pitch` — any reference into the table that resolves to nothing is refused at validation naming the reference and the defined set. This is what makes the table the single authority rather than a suggestion: an undefined name cannot compile, so a check downstream never meets one. Error, validation tier. Binding: references resolved, stated. |
| `DW0813` | **A verdict rests on a provisional standard.** Any check whose pass/fail reads a building metric whose `calibrated` is false states so once per run: a warning naming the metrics and the checks that read them. The checks still run and still refuse — a provisional number is a number — but a green that rests on an unwalked standard says it does, so the gym walk's absence is visible in every build rather than remembered. Warning, exit 0. Binding: provisional entries read, stated; zero provisional entries read = the line does not print, which is the calibrated end state, not a vacuity. |

## 3. Stage 3 — the layout graph

### 3.1 The document

`layout-graph.json`, a new campaign stage document (stage name
`layout-graph`) with the standard envelope, existing only at `dsl_version` ≥
0.13.0. It states the campaign's space as a graph **before any coordinate
exists**:

- **`nodes[]`** — `{id, intent, size_class, note?}`. A node is a **place**:
  a room, a courtyard, an arena, a stretch of shore, a cavern — the general
  unit. `intent` is a free non-empty label (arena, hub, vista, gate-house,
  shortcut-landing, …) that **no check keys on** — it is recorded judgement
  for the reviewer and for the stage-6 briefs, kept free-form deliberately:
  an enum of intents would be this month's genre wearing a schema's clothes.
  `size_class` names a metrics-table class (`DW0812` on an unknown name).
- **`edges[]`** — `{id, a, b, class, gating?, one_way?, shortcut?,
  opens_from?}`. `class` is `walk | stair | drop | barred | vision`.
  `gating` is `{requires_flags[]?, requires_quest?}` — the flag vocabulary is
  the existing one quests set. `one_way` declares intended directionality
  (`a_to_b` | `b_to_a`); a `drop` edge is one-way by construction and
  declares which way it falls. `barred` is a sealed connection some effect
  opens; `opens_from` (`a` | `b` | `either`, default `either`) states which
  side can open it — the one-side-openable door is spelled here, and it is a
  property of the edge, not of any campaign's fiction. `shortcut: true`
  marks an edge whose purpose is closing a loop, checked as structure
  (`DW0820`).
- **`entry`**, **`goal`** — node ids.
- **`critical_path[]`** — an authored node sequence from `entry` to `goal`.
  Authored rather than derived so that it is a claim the machine verifies
  (`DW0817`) and the walk sheet can print; a derived path would be an answer
  with no author to disagree with.
- **`beats[]`** — `{quest, objective, node}` bindings: every **place-bound**
  quest beat (a `reach-anchor`, a `talk-to` whose NPC stands somewhere, a
  `kill` whose wave seats somewhere) binds to exactly one node. The beats
  come from the existing stage-4/5 quest documents; the graph adds only the
  *where*.

This is the mission→space bridge: the topology carries the global guarantees
and is checked as an object of its own, cheaply, before geometry exists to
make it expensive.

### 3.2 Gating semantics, defined once

Graph checks judge reachability under a **monotone closure**: starting from
`entry` with nothing, alternately (a) mark every edge passable whose
`gating` is satisfied by the obtained set, (b) mark every node reachable
over passable edges respecting one-way direction, (c) add to the obtained
set every flag/quest whose granting beat is bound to a reached node. Iterate
to fixpoint. Deterministic, linear, and exactly Dormans' loop: beats grant,
edges demand.

**Marked judgement, with its falsifier**: the closure is branch-blind — a
campaign whose branch points (spec-0025) set mutually exclusive flags can
make the closure optimistic, reaching a node no single playthrough reaches.
The stage-5 battery is branch-aware on bytes (the existing per-branch nav
proofs and the bot), so the optimism cannot ship a broken world; it can only
under-report at graph time. If a campaign's graph-stage green turns into a
repeated stage-5 red on branch-gated nodes, the closure gains branch
awareness from `compiler::flow`'s existing branch enumeration — that is the
trigger, and before it fires the simple closure is the cheaper instrument.

### 3.3 Stage-3 checks

Validation tier (exit 1) unless stated; every check states its binding count.

| Code | Rule |
|---|---|
| `DW0814` | **The graph is not a graph.** A duplicate node or edge id, an edge endpoint naming no node, an edge whose two ends are one node (for every class — a self-loop states nothing a place does not already state), a missing or unknown `entry` or `goal`. Referential wellformedness, refused before any semantic check runs. |
| `DW0816` | **A node the closure never reaches.** Under §3.2's closure, a node unreachable from `entry` respecting gating and one-way direction. Analysis tier (exit 2). The message names the node and the nearest reached node, so the missing link is visible. Binding: nodes examined. |
| `DW0817` | **The critical path does not hold.** The authored `critical_path` is not a path (consecutive nodes share no edge), does not run `entry` → `goal`, traverses an edge whose gating is unsatisfied by beats bound to nodes already visited on the path (quest-legal order, checked stepwise), or fails to visit every beat-bound node whose beat is on the mandatory quest spine. Analysis tier (exit 2). Binding: path steps checked and beat-bound nodes required, both stated; **zero beat-bound nodes is stated as a zero binding** — a critical path over an unbound graph is a route through nothing and says so. |
| `DW0818` | **The graph names quest-side state that does not exist, or a place-bound beat has no place.** Three shapes of one referential rule between the graph and the quest documents: a `beats[]` entry naming an unknown quest/objective; a `gating` naming a flag no effect sets or a quest that does not exist; and the reverse direction — a place-bound beat in the quest documents bound to **no** node (every place-bound beat binds, so space and mission cannot silently disagree). Also the opener obligation: a `barred` edge that no quest-side effect opens (§6.2 names the region such an effect must target) — refused here, by name, rather than surfacing as a pathless `DW0311` whose symptom names nothing. |
| `DW0819` | **A one-way edge strands.** For every one-way edge `u → v`: from `v`, under the obtained set with which `u` first becomes reachable in §3.2's closure, some path to a critical-path node must exist over edges passable under that set (one-way respected). A body can only be at `v` having been at `u`; if it cannot rejoin the spine from there, the drop is a softlock. Analysis tier (exit 2). **Marked judgement**: the check uses the closure's obtained set at `u`, which is the maximal set available at that round — a player may arrive holding less. The residual is covered on bytes by the branch-aware stage-5 battery; if a walked blockout ever demonstrates a strand this check called green, the check moves to the gate-state lattice, and that evidence is the trigger. Binding: one-way edges examined. |
| `DW0820` | **A shortcut closes no loop.** An edge marked `shortcut: true` must lie on a cycle: its endpoints remain connected with the edge removed (direction-blind — the loop it closes is spatial). A shortcut that closes nothing is a corridor wearing a shortcut's name, and the graph is where that claim is cheap to refuse. Binding: shortcut edges examined. |
| `DW0822` | **The pacing measurement.** Per critical-path leg, the nominal traverse length from the size-class ladder; summed and multiplied by the pacing coefficients into a projected route-minutes figure, printed **with no threshold** — the coefficients are uncalibrated until the first walked blockout and first full playtest, and a threshold before calibration would be a number defending nothing. Warning, exit 0, at two call sites like the identity checks: over the graph (projection) and over the built blockout (measured A* route length along the critical path), so projection and measurement sit side by side and the coefficients can be calibrated by comparing them. Binding: legs measured, stated at both sites. |

## 4. Stage 4 — the site plan

### 4.1 The document

`site-plan.json`, a new campaign stage document (stage name `site-plan`),
`dsl_version` ≥ 0.13.0: the geometric embedding of the layout graph, and the
whole map's design of record. It owns:

- **`region`** — `{origin: [x,y,z], extent: [dx,dy,dz]}`, in world
  coordinates. The whole map's one region. There is **no way to omit it and
  no way to derive it**: the schema has no "compute from boxes" spelling, so
  extent-flows-up is unrepresentable, not merely forbidden. The water plane
  is not site-plan surface: `horizon: ocean` in the stage-1 world document
  already fixes sea level, and the plan reads that single authority rather
  than restating it.
- **`datums`** — named ground planes (`{id, y}`) the boxes reference.
- **`boxes[]`** — `{node, min: [x,y,z], extent: [dx,dy,dz], floor: <datum id
  or y>, ceiling: <blocks> | "open"}` — **exactly one box per graph node**,
  horizontal extents on the kit grid (multiples of `q`), the node's floor
  datum stated. `ceiling: "open"` is a sky-open place — a courtyard, a
  shore, a summit.
- **`seams[]`** — `{edge, face, at: [u,v], opening: <standard name>, rise,
  stair_in?}` — **exactly one seam per traversal edge** (`walk | stair |
  drop | barred`; `vision` edges carry a sightline instead — §4.4). A seam
  sits on a face the two boxes **share**, at cells `at` on that face, with
  its opening from the standard set and its `rise` (the signed floor-datum
  difference it spans) stated. `stair_in` names which of the two boxes hosts
  the stair massing when `rise` needs one.
- **`volumes[]`** — `{id, min, extent, role: massif | ground | clearance}` —
  the volumes the whole itself owns: the mountain a cave system is inside,
  the ground plane under a village, the sky a silhouette needs kept empty.
- **`identities[]`** — `{fact, measure, cmp}`: guarded comparisons binding
  the plan to the geometry brief's facts (§4.2). `measure` is from a small
  fixed vocabulary: `region.extent.x|y|z`, `box(<node>).extent.x|y|z`,
  `distance(<node>,<node>).xz`, `datum(<id>).y`, and
  `height(box(<node>))` — enough to state extent, proportion, standoff and
  dominant-vertical facts. **Marked judgement**: the vocabulary will grow;
  the falsifier is the first brief fact a campaign cannot bind with it, at
  which point the missing measure is added as first-class vocabulary — never
  worked around by binding a different fact.
- **`sightlines[]`** — `{edge, from: [x,y,z], to: [x,y,z]}`, one per
  `vision` edge (§4.4).
- **`views[]`** — `{id, eye: [x,y,z], look_at: [x,y,z], note?}`: the named
  exterior views the stage-5 walk judges the silhouette from, rendered
  beside the stage-2 reference sheet. Optional; a plan with zero views has
  that zero stated in the build output.
- **`lighting`** (optional) — `{fixture, min_light}`, one setting applied to
  every enclosed box by the existing relight pass (default torch / 7), so a
  blockout interior is walkable at night without per-box surface.

### 4.2 The brief's machine-readable facts (the stage-2 input format)

Stage 2 (the whole's reference sheet and written brief) is not designed here;
this spec fixes only the input stage 4's identity checks require:
`geometry-brief.json`, a campaign document with the standard envelope and one
section — `facts[]`: `{id, value, unit?, note}`. A fact is a number with a
name; the brief's prose stays prose, and only what is stated as a fact is
checkable. The reference imagery keeps its standing: style authority,
rank-only, never a gate (spec-0028) — identities bind to the written brief's
numbers, never to a picture.

### 4.3 Stage-4 checks

All at site-plan validation (exit 1), all upstream of any geometry, each
stating its binding count:

| Code | Rule |
|---|---|
| `DW0824` | **The graph and the plan do not agree exactly.** A node without a box, a box without a node, a traversal edge without a seam, a seam without an edge, a `vision` edge without a sightline, a sightline without a `vision` edge — each a named refusal; a site plan present with no layout graph (or no geometry brief) is the limiting case and is refused naming the missing document. This check is also the two-artifact question's instrument (§10): how often it fires **alone** — a graph edit with no plan edit or the reverse — is measurable from CI, and that number decides whether graph and plan stay two documents. |
| `DW0825` | **A box leaves the kit grid.** A horizontal extent not a multiple of `q`, per the metrics table. Named per box with both numbers. |
| `DW0826` | **A box leaves the region.** Any box cell outside `region`. The region is the brief's number flowing down; a box is never grounds to grow it — the prescription is to shrink or move the box, or change the brief's fact and re-derive, visibly. |
| `DW0827` | **Two boxes overlap.** Boxes are disjoint; shared **faces** are the only permitted contact, because a seam needs one. Named with both boxes and the intersection. |
| `DW0828` | **A seam is not on a shared face.** The seam's declared face is not shared by its edge's two boxes, or its `at` cells lie outside the shared area. Seams are **allocated on faces both boxes already have** — the two-places-cannot-mate failure class is resolved here, where both boxes are still free, not discovered later between finished parts. |
| `DW0829` | **A seam's opening is not a standard, or does not fit.** The named opening is undefined (`DW0812` covers the unknown name; this code is the geometric half) — the opening's `width × height` does not fit within the shared face at `at`, or the opening's sill is not at a height a body at the source floor can enter per the step rule. |
| `DW0830` | **A stair seam cannot be built at standard pitch.** `|rise|` needs a run longer than the hosting box (`stair_in`) affords along the seam's normal, at every standard pitch in the table, after clearance. The message names the rise, the best pitch's required run, and the available run — the numbers a plan edit needs. |
| `DW0831` | **A drop seam falls outside the drop policy.** `rise` on a `drop` edge exceeds the metrics table's designed-drop cap (or is negative toward the declared direction — a drop that rises is a mislabelled stair). Policy cap, deliberately tighter than the physics survivability fact stored beside it. |
| `DW0832` | **A box violates its node's size class.** Interior footprint or clearance outside the class's declared range. This is the one place a size class becomes geometry; the class's own playtime weight stays thresholdless (`DW0822`). |
| `DW0833` | **A brief identity does not hold.** An `identities[]` comparison is false. Raised at **two call sites** with one rule: over the plan at validation, and over the **built world** at stage 5 (the measures recomputed from assembled bytes), so a derivation defect that moved a datum cannot hide behind a plan-time green. The refusal names both numbers — the fact's and the measured one. |
| `DW0834` | **The identity gate binds nothing.** Zero `facts[]` in the brief, or zero `identities[]` in the plan: the binding that holds the whole to its written design is empty, which is the vacuity the whole stage exists to prevent. Warning naming the empty side — a finding for the round summary, per the standing rule that a zero binding is a finding, not silently a pass and not an error that would refuse a deliberately minimal fixture. |
| `DW0835` | **A whole-owned volume enters a box.** A `volumes[]` region intersecting any box's interior: the whole's mass may stand beside, under and over places, never inside them — an overlap here is two authorities writing one cell, which the derivation must never have to arbitrate. |

### 4.4 Departure, recorded: a vision edge carries a sightline, not a seam

The research's stage 4 allocates "a seam for every edge". This design gives
**traversal** edges seams and gives `vision` edges a **sightline** — two
cells, one in each box, whose connecting segment the stage-5 battery walks
with the existing exact grid traversal (the cutscene clip's DDA). Reason for
the departure, recorded at the point it is made: a seam is an opening on a
**shared face**, and a vista's two ends are routinely not adjacent — a
bell tower seen from a shore shares no face with it — so the seam construct
cannot state the one thing a vision edge asserts. The sightline is the
general form; a window between adjacent places is simply a short one. The
seam's guarantees (standard openings, rise) are about bodies passing, which a
vision edge does not claim, so nothing is lost in the substitution.

## 5. Stage 5 — the whole-map blockout

### 5.1 Derived, authored by no one

The blockout is a **pure function of the site plan and the metrics table**.
It is not an authored program; there is no document to write, so an author
cannot introduce a defect into it and a plan revision regenerates it with no
hand edit to lose. It takes no randomness: the same plan and table produce
byte-identical output regardless of seed — one derivation, no parameters, in
the slice. (**Marked judgement**, carried from the research with its
falsifier: if walked blockouts repeatedly need hand-shaped massing to be
judgeable — a vista that does not read without its landform — the derivation
gains **parameters**, never hand edits.)

For every box: a floor slab whose top face is at the box's datum, shell walls
one cell thick to the declared clearance, a ceiling unless `open`. For every
seam: its opening cut at its allocated cells; where `rise` is nonzero, stair
or ramp massing in `stair_in` realizing the declared rise at a standard
pitch. For every volume: plain mass (or kept-empty clearance). Materials are
a fixed legibility palette — floors, walls, seam frames and stair massing in
distinct blocks, one accent color per node cycled deterministically — so a
walker can see where one place ends and another begins, which is the
blockout's whole job.

The derivation's output enters the build as **placed pieces** — one synthetic
piece per box plus the whole-owned volumes — at the same join placed prefabs
enter `compiler::assembled`. Everything downstream is inherited unchanged:
gravity settling, the nav occupancy model, relight, boundary derivation,
forceload spans, emission, the bot export. That is what "compiled by the
existing placement machinery" means concretely, and it is why the slice ships
no second world pipeline.

### 5.2 Synthesized anchors and gates — how the quest layer lands on massing

A blockout world has no prefab-declared anchors, so the derivation
synthesizes the campaign's spatial vocabulary:

- **`node/<id>`** — an anchor at each box's floor center. Quests, NPCs and
  waves in a site-plan campaign anchor here (and the entry node's anchor
  carries the declared entry **role**, per spec-0046, so spawn placement is
  the existing machinery).
- **`seam/<edge>`** — a gate region over each `barred` seam's opening cells,
  which the derivation fills (the world-load seal measures it shut, as with
  any prefab-authored gate). The quest layer opens it with the existing
  verbs: an `open-gate` naming `seam/<edge>`, or a `shortcut` for an edge
  with a one-sided `opens_from` — for which the derivation also synthesizes
  **`seam/<edge>/unlock`**, an anchor on the opening side of the seam, where
  the shortcut's far-side affordance stands.

Nothing else is invented: no new gating machinery, no new verbs. The graph
declares the topology, the plan places it, the derivation builds it, and the
existing quest surface drives it — which is what lets the existing analyze
pass, gate-seal model, PackTest emission and mineflayer bot run unmodified
over a world nobody built by hand.

### 5.3 The machine battery, at map scale

Run on every regeneration, before any walk is requested. Existing checks
inherited whole: boundary safety and fluid escape (`DW0322`/`DW0318`),
gravity (`DW0313`), the branch-aware critical-path walkability proofs, the
relight gate, determinism (double-build byte-identity), the emitted-command
tree check, and the bot playthrough (existing harness, existing
`critical-path.json` export) end to end under quest gating. New checks, build
tier (exit 3) unless stated:

| Code | Rule |
|---|---|
| `DW0836` | **A built seam disagrees with its allocation.** Over the assembled bytes, per seam: the opening's cells are passable per the step rule from the source floor (for `barred`, with the gate's cells treated open), the opening is exactly the allocated cells — no wider, no elsewhere — and the realized rise equals the declared rise. The derivation is supposed to make this impossible; this check is the **independent observer** of the derivation, computed from bytes and the plan, sharing none of the derivation's arithmetic — a second method that differs where the suspicion is. Binding: seams proven. |
| `DW0837` | **A node's floor is unreached.** Per-cell reachability from the entry over the assembled world (the nav step rule, `barred` seams opened as their gating closure allows): every node's floor must contain at least one reached standable cell. The graph's `DW0816` proved this over topology; this proves the derivation preserved it in bytes. Binding: nodes proven. |
| `DW0838` | **A connection nothing allocated.** Every legal step (walk, jump, drop) between a standable cell owned by one box and a cell owned by another must lie within a declared seam's opening cells. A crossing anywhere else — over a shared wall the massing left low, through a corner the shells did not close — is a seam that was **discovered**, which is the exact failure class the allocation exists to end, caught as a named refusal instead of shipped as an accident a player finds. Binding: cross-ownership steps examined. |
| `DW0821` | **A sightline is blocked** (the `vision` edge's proof — declared at stage 4, measured here). Per `vision` edge, the DDA walk of its declared segment: every intersected cell must be non-solid in the assembled world. **Warning** in the slice, promoted to error by the detail-stage spec: derived massing has no landform shaping, so a legitimate vista over terrain the detail pass will carve can be blocked at blockout time, and refusing it now would force hand-shaped massing — exactly what §5.1's falsifier reserves for walk evidence. The warning names every blocking cell, so the walk sheet carries the fact. Binding: sightlines walked. |

`DW0833` (identities) and `DW0822` (pacing) run their second call sites here,
as stated in their rows.

### 5.4 The human walk, and its record

The blockout walked in-game is **the campaign's first gate**: scale, pacing,
route legibility, and the massing silhouette judged from the declared
`views[]` renders beside the stage-2 reference sheet. The loop is
deliberately tight — a finding edits the graph or the plan and regenerates in
seconds. How a round is staged and reported is operating practice
(`docs/reference/playtest-methodology.md`), not restated here.

What this spec fixes is the record's **form**, because the next spec's
machinery consumes it: `walk-record.json`, a campaign artifact —
`{site_plan_sha256, blockout_sha256, engine_revision, verdict: "passed" |
"findings", findings[]: {subject, note}}`. The build prints both hashes so
the record can name its instrument literally (the revision, never a version
string). In this slice **nothing enforces the record** — stated plainly
rather than implied: the record is an artifact format, and its gate
(allocation handing refuses a missing or stale record) is the detail-stage
spec's first obligation, named in §8. Binding it here to nothing and
pretending otherwise would be the UNRUN shape wearing a schema's clothes.

## 6. One campaign, one placement authority

| Code | Rule |
|---|---|
| `DW0839` | **Two placement authorities in one campaign.** A stage-1 world document declaring `areas[]` in a campaign that also carries a site plan is refused: `areas[]` places pieces on the fixed stride, the site plan places the blockout in its own region, and a world with both has two owners for one question. One or the other, per campaign. Validation tier. Both remain legal at 0.13.0; retiring `areas[]` is not this spec's subject. |

**Gallery obligation, stated for the implementation PRs**: every schema
property and enum variant of `layout-graph`, `site-plan`, `geometry-brief`
and the metrics export becomes a coverage unit the moment it lands
(`schema --stage all` is the authority). The gallery's primary campaign stays
on `areas[]`; the new surface binds through an **overlay** (the gallery's
existing mechanism for settings that cannot coexist with the primary) that
builds the gallery's quest layer over a small graph + site plan, and a
committed **probe** binds `DW0839` itself — the mutual exclusion produces
exactly the machine refusal the probe form demands. Every new DW code above
lands with a test asserting it (`tools/check-dw-codes.py`), a red fixture and
its catalog row in `docs/reference/compiler.md`, in the same PR as its check.

## 7. What makes the ordering structural

The reset this design answers was caused by an ordering that existed as
prose. Here, each inversion is **not compilable**, enumerated:

1. **Site plan before graph** (or before brief): `site-plan.json` validates
   only against a layout graph and a geometry brief — `DW0824` refuses a
   plan whose graph or brief is absent, and every box must name a graph
   node. A plan cannot reach green first.
2. **Blockout before site plan**: the blockout has **no authored form** — no
   document, no file, no schema. The only path to blockout bytes is the
   derivation, whose input is a validated site plan. There is nothing to
   author early.
3. **Detail before the walk**: in a site-plan campaign there is no
   authorable placement surface at all — no `areas[]` (`DW0839`), no
   per-place region anywhere in the campaign manifest; regions live in the
   site plan alone. A detail program's compilable form (a handed allocation)
   does not exist until the next spec builds it, and that spec's first
   obligation is that the handing refuses without a walk record whose hashes
   match the current plan (§5.4).
4. **Graph before mission**: representable — a graph with no `beats[]` and
   no gating validates against quest documents it never references. It is
   not silently green: `DW0817` states its zero beat binding, and the moment
   quests exist, `DW0818`'s reverse direction demands every place-bound beat
   bind. The ordering tooth between stages 1 and 3 is the pair of
   directions in `DW0818`, and the vacuity of an unbound graph is stated,
   which is the standing treatment for a binding of zero.

The content repo's campaign audit walks the stage artifacts in this order and
reds a campaign whose later-stage artifact exists without its earlier-stage
input — the same event-bound shape as every gate above, bound to the audit
that already runs on every push.

## 8. Out of scope, named as the next spec's subject

Pipeline stage 6 — the allocation handing (box, frame, datums, seams and
palette handed by the toolchain, not by a brief's prose), the
traversal-equivalence check (the detailed place keeps its seams, keeps every
blockout-reachable region reachable at the same edge classes and rises, adds
no new way out), the walk-record gate on the handing, and the kit-piece
footprint-class admission check — and stage 7 (the whole art pass and the
equivalence re-run over the dressed whole) are one following spec. Nothing in
this slice presumes their design beyond the walk record's form (§5.4) and the
statement that stage-6 machinery is **not** built first — building it first
is the old ordering arriving through the build order.

## 9. Departures from the plan of record

Each recorded where it is made; collected here so the list is auditable:

1. **Vision edges carry a sightline, not a seam** (§4.4) — the seam construct
   cannot state a non-adjacent vista.
2. **The sightline proof is advisory in the slice** (§5.3, `DW0821`) — an
   error would force hand-shaped massing ahead of the falsifier evidence the
   research reserves it for.
3. **Building-metric values land provisional, calibration follows** (§2.3,
   `DW0813`) — the gym is itself a site-plan campaign, so the machinery must
   exist before the walk that calibrates the table; the research's "the gym
   lands beside the slice" is sequenced within, not around. The provisional
   state is visible in every build rather than remembered.
4. **The kit-piece footprint admission check is deferred to the detail-stage
   spec** (§8) — no kit piece participates in the blockout; the check binds
   where pieces are first consumed, and a gate landed before anything it
   examines exists would be green at zero binding from birth.
5. **Graph reachability is branch-blind in the slice** (§3.2, §3.3
   `DW0819`) — the monotone closure with its stated optimism, backstopped by
   the branch-aware bytes battery; the trigger for deepening is named.

## 10. The two-artifact question, carried

The research's open item: whether graph and site plan stay two artifacts or
merge. This design keeps them separate — the graph's checks are valuable
before any embedding exists, at the cost of one agreement check — and makes
the falsifier **measurable**: `DW0824` firing alone (a graph edit without a
plan edit, or the reverse) is the event that justifies two documents. If, in
practice, every graph edit is immediately a plan edit and the agreement check
never fires alone, the two merge — a decision to be taken on that CI-visible
count, not re-argued.

## 11. The general-engine test, with falsifiers

Nothing in the vocabulary is castle-shaped, checked case by case: an **open
island** is sky-open boxes on a ground volume with walk seams at grade and an
ocean horizon; a **village** is small boxes on one datum, seams as lanes and
door slots, the church tower a tall box with a sightline to it; a **cave
system** is boxes inside a `massif` volume, stair and drop seams, no sky. A
detached-sites map is several box clusters in one region — a site-plan fact,
not an engine distinction. `intent` is uninterpreted; size classes,
openings and pitches are table entries a creator re-fixes for their own
fiction. **Falsifier, carried from the research**: the first campaign brief
this vocabulary cannot state without a workaround is the evidence — and per
the no-hacks rule the answer is a first-class surface or a refused feature,
decided on that brief, never folklore downstream.

## 12. Order of work

Dispatched as separate rounds; each lands with its tests, catalog rows,
gallery elements and doc updates in the same PR, per standing rules:

1. Metrics module + `delvec metrics` + `DW0812`/`DW0813` + provisional
   values.
2. `geometry-brief` + `layout-graph` schemas and the stage-3 checks.
3. `site-plan` schema and the stage-4 checks.
4. The derivation + synthesized anchors/gates + the stage-5 battery +
   `DW0839`.
5. The fixture campaign (a five-node graph with a barred seam, a drop, a
   shortcut loop and a vision edge, proven end to end by the bot), the
   metrics gym, and the gallery overlay + probe.

**The first walkable whole exists at the end of round 4** on the fixture —
four small documents, no authored geometry — and every later round deepens an
already-walkable path. A full restart at any point before round 5 discards
schemas and checks, never content: the throwaway cost is the design's, not a
campaign's, which is the property the whole reset was bought for.

## 13. Acceptance criteria

Machine-checkable; each names its verdict's instrument.

1. `delvec metrics` exits 0 and emits JSON with `metrics_version`, a player
   half, and a building half in which every entry carries `calibrated`; a
   unit test asserts the exported player metrics are (compile-time) the nav
   model's own constants — one definition, not two agreeing.
2. `delvec schema --stage all` includes `geometry-brief`, `layout-graph` and
   `site-plan`; `tools/check-gallery-coverage.py` is green with every new
   unit bound in the gallery domain or refusal-proven, and the `DW0839`
   probe is committed and red.
3. Every code in DW0812–DW0814, DW0816–DW0822, DW0824–DW0839 (as assigned;
   DW0815/DW0823 unassigned) has at least one test asserting it and a
   fixture the compiler refuses (or warns) with it; `tools/check-dw-codes.py`
   is green in both directions with zero new allowlist entries.
4. The fixture campaign (`layout-graph` + `site-plan` + quest layer, zero
   authored geometry) builds to a joinable world: `delvec build` exit 0; two
   builds byte-identical; changing the seed changes no blockout byte.
5. The fixture's bot playthrough passes: the mineflayer critical path walks
   entry to goal through the barred seam's opener and the shortcut's
   far-side unlock, on the same harness and export existing campaigns use.
6. On the fixture, deleting the layout graph makes `delvec validate` red
   with `DW0824`; deleting the quest documents' opener for the barred edge
   makes it red with `DW0818`; adding `areas[]` to its world document makes
   it red with `DW0839` — the three ordering teeth demonstrated red→green.
7. Every new check's output states a binding count; building a fixture with
   zero identities emits `DW0834` naming the empty side; `DW0817` on a
   graph with zero beat bindings states the zero.
8. `DW0836`, `DW0837` and `DW0838` each have a fixture red produced by a
   deliberately perturbed derivation in a test (not by hand-authored
   bytes) — proving the checks are independent observers of the derivation,
   not replays of its arithmetic.
9. The metrics gym generator emits a site-plan campaign that builds green,
   and its row is queued in `docs/demo-levels.md`.
10. `docs/reference/compiler.md` carries the new stage tables, the metrics
    export, and every new DW row; `docs/reference/tools.md` carries
    `delvec metrics`; `tools/check-doc-dupes.py`,
    `tools/check-reference-versions.py` and the docs job are green.

## 14. Not settled here

For the implementation rounds and the record, beyond the marked judgements
inline (§3.2 branch-blindness, §3.3 strand optimism, §4.1 measure
vocabulary, §5.1 derivation parameters, §10 the two-artifact question):

- **The building-metric values** — provisional until the gym walk; every
  build says so (`DW0813`).
- **The pacing coefficients** — thresholdless until the first walked
  blockout and first full playtest calibrate them (`DW0822`).
- **Walk cadence** — whether the human walk is owed per plan revision or at
  first pass and majors is decided with the first campaign's revision data,
  not before; the machine battery always runs first either way.
- **How much of the existing piece library conforms to the kit grid** once
  `q` is calibrated — numbered by the admission audit when the detail-stage
  spec lands its check; not a blocker for anything in this slice.
