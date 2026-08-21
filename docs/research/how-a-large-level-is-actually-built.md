# How a large level is actually built — research record and pipeline plan

- **Status**: research record and plan of record for the map pipeline. This is
  not a spec and allocates no document number; §3 names the specs and the ADR
  it implies, which are written by their own rounds.
- **Ground**: spec-0040 (map composition), ADR-0020 (spatial contract), and the
  composition consequences of ADR-0004 are **void as decisions**, together with
  the eight zone programs of the campaign built under them. Nothing in this
  document is derived from them; §4 reads them as history and checks them
  against the research. Their formal supersession is an ADR this document
  implies but does not write.
- **Question**: how is a large, coherent, walkable game level actually built in
  professional practice, and what should this project's pipeline be?
- **Method**: web research across four areas (pipeline stages, metrics and
  modular kits, large-world assembly, procedural generation and
  Minecraft-specific practice), each finding cited with its source and an
  evidence-strength rating; the three trials and the void documents were read
  **after** the research, and only for §4. The three most load-bearing sources
  were verified by a second independent read (the Shaver GDC 2018 slides, the
  Dormans 2010 paper, the Burgess/Purkeypile GDC 2013 transcript — exact
  quoted sentences reproduced from the source bytes).

Evidence weights used throughout: **STRONG** = shipped-engine official
documentation, a published book or peer-reviewed paper, or a recorded
conference talk by a named practitioner, read directly. **MEDIUM** = a named
practitioner's own blog or a reputable industry outlet with named first-party
sources, or a strong source read only through excerpts. **WEAK** = forum, wiki
or secondhand summary. Where a claim rests on nothing found, it is marked
**judgement** at the point it occurs.

---

## 1. What the research says

The findings converge from four independent directions on one shape. Each is
stated with its evidence so a reader who has read none of the sources can
weigh it.

### 1.1 The order of work is fixed, and detail comes last

The professional sequence is: **pre-production (constraints, beats, scope) →
2D layout → blockout → playtest loop on the blockout → layout lock → art
pass → lighting → polish.** A *blockout* (also called blockmesh, greybox,
whitebox) is a playable rough draft of the level built from simple geometry,
in-engine, with real player physics.

- Naughty Dog's level pipeline, from the author's own GDC 2018 slides (David
  Shaver, "Invisible Intuition: Blockmesh and Lighting Tips", GDC 2018;
  slides at davidshaver.net/DShaver_Invisible_Intuition_GDC2018.pdf —
  **STRONG**, read directly and independently re-verified): first "establish
  the context and constraints of your level" and "decide on the gameplay and
  narrative beats"; then build "a rough version of the **entire level** so you
  can get a sense of pacing and scale"; then "get something playable quickly,
  and then watch other people play it… Repeat this iteration loop until
  you… basically run out of time. And once you run out of time, your layout
  is locked and handed off to the rest of the team to make it look amazing."
  He adds: "every studio has a different process, but this core iteration
  loop is universal."
- Epic's official documentation (**STRONG**) prescribes the same gate: the
  two things to test before art are **scale** and **layout**; "add props and
  art once you are happy with how the level plays" (Epic, "Greyboxing in
  Unreal Editor for Fortnite", dev.epicgames.com).
- The Level Design Book (Robert Yang, book.leveldesignbook.com — **MEDIUM**,
  a widely used practitioner reference) states the same sequence and the gate
  logic: "if you realize a room design isn't working, then you can modify it
  more easily when it is made of simple shapes"; "premature art pass locks-in
  early design mistakes." Its blockout page names the most common defect
  found at blockout: **scale** — "playtesting the blockout in-game is the
  best way to know if it feels too big or too small. Do NOT just fly around
  in the editor view."
- The economics that force the ordering (Michael Barclay, Naughty Dog, level
  designer on The Last of Us Part II, own blog — **MEDIUM**): blockout
  geometry is cheap to throw away, art-passed work is expensive — "if you
  feel like you've done so much work that it would pain you to throw it away,
  then you've gone too far." Greybox detailing begins only "when the team
  decides the level is sound from a design standpoint."

Two corollaries with direct sources. First, **the blockout gate is a
playtest, not a review of drawings**: "you can't playtest a design document
or a layout sketch, but you can playtest a blockout" (Level Design Book,
MEDIUM). Second, **the art pass is defined as cosmetic and is re-validated
against traversal**: environment art is "the cosmetic decoration of a level…
while preserving its core functionality and gameplay"; Riot's Valorant
pipeline inserts an intermediate "art blockout" so late design tweaks
propagate to art cheaply (Level Design Book env-art page quoting Riot's dev
material, MEDIUM); and Naughty Dog instruments the re-validation — every
failed jump-grab by a playtester is recorded and overlaid in the level as a
red sphere ("we call these 'bad jumps'"), and the fix is applied to the
*art* (e.g. foliage over a climbable-looking ledge) until the bad jumps
disappear (Shaver slides, **STRONG**).

Counter-evidence, stated rather than smoothed: the ordering is not
universal. Firewatch's developers found "greyboxing did not answer any of
the important questions" because the experience depended on art, dialogue
and mood grey geometry cannot represent (via the Level Design Book,
MEDIUM). This is the documented failure mode of the blockout gate: a game
whose core content is not traversal. A delve's machine-provable half *is*
traversal, so the exception does not describe this project — **judgement**,
but a short step from the constraint that a delve must be provably
completable by machine.

### 1.2 Metrics come before geometry, and there are two kinds

The practice of fixing player-relative dimensions before any geometry is
authored is old, documented, and specific.

- Valve documents the Source-engine dimensions exhaustively (Valve Developer
  Community, "Dimensions (Half-Life 2 and Counter-Strike: Source)" —
  **STRONG**, engine-official, read in full via archive): player collision
  32×32×72 units (36 crouched), eye 64, max walkable step 18, jump onto 20 /
  crouch-jump onto 56, minimum passage 33, "normal" door 48×108, "normal"
  corridor 64 wide × 128 high, max slope 45.57°. These are the numbers a
  blockout is authored against.
- The Level Design Book's metrics page (**MEDIUM**) makes the load-bearing
  distinction: **player metrics** are engine facts (measured, not chosen);
  **building metrics** are designer-decided standards (door sizes, corridor
  widths, storey heights) that the team fixes and then obeys. On timing: "if
  you wait too late (like after an art pass) then it will be very
  time-consuming to make big changes." It recommends "metrics zoo" test maps:
  blockout rooms at varied dimensions, playtested, until the standards are
  agreed.
- The working method for deriving them (Max Pears, level designer, CD
  Projekt Red, "Level Design for Combat", Game Developer 2019 — **MEDIUM**):
  measure movement/jump with in-level rulers and ranges of boxes, have the
  team play and agree on feel, write the numbers into a one-glance document,
  and refer to it constantly during blockout. Scott Rogers' Level Up!
  (**MEDIUM** as accessed) states the same principle in book form: the
  protagonist's body is the defining metric of the game world.

### 1.3 Parts fit by construction: the modular kit discipline

The strongest single source in this research is Bethesda's kit practice
(Joel Burgess & Nathan Purkeypile, "Skyrim's Modular Level Design", GDC
2013, author-published transcript, republished at gamedeveloper.com —
**STRONG**, read directly and independently re-verified; reaffirmed in the
Fallout 4 GDC 2016 sequel). The rules, with their exact content:

1. **The footprint is the foundation.** "The footprint is the full bounds of
   the piece, and not the traversable space of the piece." Pieces always
   exist within the footprint; a piece touches its edge only at the seam
   where it snaps to another piece.
2. **Footprints are power-of-two multiples of each other.** "A 512×512×512
   room will always tile nicely with a 256×256×256 hallway, but a
   384×384×384 room will eventually create gaps and/or overlaps." The
   editor snap grid is half the footprint; large snap sizes are preferred
   because misalignment becomes visibly obvious.
3. **Pivots are fixed forever** (ground plane, centered; changing one later
   means manual fixes to hundreds of placed instances).
4. **Connection points are first-class kit pieces.** Doorway ("Ex") pieces
   snap with matching doors of the same size class — "only matching sizes
   will fit together" (Bethesda's own Creation Kit tutorial, **STRONG**).
   Transitions between kits are planned pieces, not discoveries: "plan out
   how to blend one theme into another… note which structures may need
   capping off" (Lee Perry, Epic, "Modular Level and Component Design",
   Game Developer magazine 2002 — **STRONG**, read in full). Perry also:
   lock stair heights/depths early, print the standards, distribute them —
   "a few hours' work here will save weeks."
5. **The economics**: two full-time kit artists and seven kits supported
   eight level designers producing 400+ interiors in ~2.5 years. Kit scope
   follows expected usage (the cave kit ~50 pieces in its main sub-kit, used
   200+ times; the Ratway kit 7 pieces, used twice).
6. **The failure modes are named**: *patch-up pieces* (a half-length hallway
   to fix a loop that misses by half a footprint) are "a band-aid over a
   deeper problem" — the correct fix is reworking the offending footprint;
   *kit bloat* (saying yes to every request); *art fatigue* (players notice
   repeated clutter before repeated architecture); *cookie-cutter* rooms
   (Oblivion's copy-pasted warehouses, abolished from Fallout 3 on). Kits
   are stress-tested by looping back on themselves and stacking vertically.
7. **The ordering**: kit graybox first, then layouts, authored *in* the
   kit's grid — "we depend heavily on our kits to establish layout and flow…
   getting out the graph paper and creating detailed, proportionally-accurate
   maps is a poor use of time" (Burgess, GDC 2014 transcript, **STRONG**).
   Final art is swapped in under live layouts, which survive because the
   functional facts (footprints, pivots) do not move. Each kit is
   nonetheless born paired with a level designer who stress-tests it against
   a pilot layout from day one — kit-first, never kit-alone.

Grid discipline generalizes across engines: Epic's UDK modular-environment
documentation ("the grid is god… the grid is rigid and keeps us focused on
building things that are logical and mathematical underneath the hood",
**STRONG** via archive), Source's power-of-two Hammer grid bound to the
§1.2 dimensions (**STRONG**), Quake-lineage practice of never leaving the
grid because the map format stores integer planes (**MEDIUM**).

### 1.4 The whole owns space; parts receive it

Who owns a large world's layout, in engines and in studio practice:

- **Engines make the whole a first-class artifact.** Unreal's persistent
  level owns the sub-levels, their offsets, and the streaming logic; a
  sub-level plugs in (Epic UE5 docs, **STRONG**). UE5's World Partition goes
  further: it "removes the previous need to divide large levels into
  sublevels by storing your world in a single persistent level separated
  into grid cells" — the engine's own conclusion that making humans own zone
  boundaries at the file level was the wrong model; authorial chunks survive
  as Level Instances placed *by* the world (**STRONG**). Unity's additive
  scenes are the same shape by convention rather than engine fiat
  (**STRONG** docs, read via excerpts).
- **The macro layout is authored first, cheaply, and renegotiated while
  soft.** The Witcher 3's open world began as pins on a 2D map ("plan the
  open world" was literally the assignment), with the ruling that "the quest
  content dictated the size of the world, not the other way around" and
  regions resized while the world was still soft (Kotaku feature with named
  CD Projekt sources, **MEDIUM**). Breath of the Wild calibrated the whole
  against a real city before content existed — a map of Kyoto overlaid on
  Hyrule to set travel-time and landmark-encounter rhythm — then governed
  placement by the "triangle rule" at three scales, and corrected the plan
  from playtest telemetry heatmaps (GDC 2017 / CEDEC 2017 Nintendo talks,
  **STRONG**).
- **Coherence while parts move is a machine's job at scale.** Assassin's
  Creed Origins built "a system of daily automated tests and reports to
  monitor, validate and visually communicate the state of world game data"
  because validating the world "using traditional methods" was "practically
  impossible" (Nicholas Routhier, Ubisoft, GDC 2018, **STRONG** abstract).
  Nintendo's answer was one live shared editor with real-time sync, with
  task management embedded in the world itself (CEDEC 2017, **STRONG** via
  named translator).
- **No published example was found of a hard per-zone spatial refusal**
  ("your zone gets X hectares, cut to fit"). Documented practice is soft
  renegotiation *before the world hardens* — the whole's owner arbitrates,
  parts and plan both move, and the plan is cheap to move at that stage.
  Absence of evidence is noted as such, not claimed as proof.
- Counter-examples, honestly: **Dark Souls** — the interview record (Design
  Works, **MEDIUM** via fan transcription) attests per-area rough maps,
  per-area owners, and coherence held by one director's continuous review;
  no master map document is attested. **Hollow Knight** grew organically
  from a jam, area by area (**WEAK-MEDIUM**, excerpts). Both are real, both
  shipped masterpieces of interconnection, and both substitute a whole held
  in one or two human heads with continuous authority over every part. That
  resource does not exist in a machine pipeline that builds a delve on
  demand — the sessions that author the parts share no head. **Judgement**:
  for this project the head must be replaced by an artifact, which is
  exactly what the engine-side sources above did.

### 1.5 Procedural generation achieves coherence top-down or not at all

The generation literature answers the direction-of-generation question
unusually cleanly:

- **Mission before space.** The canonical paper (Joris Dormans, "Adventures
  in Level Design: Generating Missions and Spaces for Action Adventure
  Games", PCG Workshop 2010 — **STRONG**, read directly and independently
  re-verified): a level is two structures, the mission (a graph of tasks)
  and the space (the geometric layout); the approach that works generates
  "missions first and then generate[s] spaces to accommodate these
  missions." His diagnosis of bottom-up generators: "levels often have a
  random feel to it and tend to lack overall structure."
- **Whole backbone before rooms.** Unexplored (shipped 2017) draws the
  complete cyclic backbone of the level first — entrance and goal on one
  loop, one of ~24 cycle archetypes — then refines through staged rewrites
  down to tiles; later stages cannot contradict the whole because the whole
  existed first as their constraint (Boris the Brave's and Tommy Thompson's
  deep analyses, **MEDIUM**, corroborating each other).
- **Solution path before contents.** Spelunky's generator carves a
  guaranteed entrance-to-exit path through the room grid as its *first* act,
  then instantiates rooms from hand-authored templates — reachability by
  construction, not by post-hoc check (**MEDIUM**).
- **Constraints beat generate-then-test.** The answer-set-programming paper
  (Smith & Mateas, IEEE TCIAIG 2011 — **STRONG**, read directly): integrity
  constraints "actually prevent undesirable answer sets from ever being
  generated in the first place," versus generate-and-test which discovers
  global violations only after a complete artifact exists, and which
  accretes "accidental complexity" as generator and validator drift apart.
- **Local-constraint methods stay at the decoration layer.** Wave Function
  Collapse guarantees local adjacency everywhere and knows nothing of global
  objectives; its shipped uses (Bad North, Townscaper — Oskar Stålberg,
  **STRONG** for the author's own statements) sit under an externally
  imposed layout, generating environment, not structure.
- **AAA world generation is offline, artist-curated, and deterministic.**
  Far Cry 5 regenerated its entire world nightly from artist-authored
  splines and biome paint, deterministically per 64×64 m sector so
  independently built sections stitch (Etienne Carrier, Ubisoft, GDC 2018 —
  **STRONG** talk, read via detailed third-party notes); Ghost Recon
  Wildlands' Houdini pipeline is the same shape (**STRONG** talk, content
  via secondary writeups). Nothing generates at runtime; generation is a
  tool inside an authored plan.

Across all of it: **no source defends assembling independently designed
parts as a way to obtain a coherent whole.** Parts-first appears only where
a topology pass has already allocated every part its slot. The synthesis
sentence is this document's, the component claims are cited above.

### 1.6 Minecraft-specific practice agrees

- **Vanilla jigsaw is local-only and degrades silently.** Assembly is
  recursive piece attachment by name-matched jigsaw junctions with collision
  avoidance inside a bounding box; when no candidate fits, nothing is placed
  and a fallback fires — a hole, not an error. There is no reachability
  concept at any scale (minecraft.wiki technical pages, **MEDIUM**;
  Microsoft's official Bedrock jigsaw docs, **STRONG**). The 48-block
  structure-template cap is why every large vanilla structure is a piece
  library (**MEDIUM**).
- **Large build teams work site-plan-first.** WesterosCraft — the
  best-documented team — fixes per-region style guides (palette,
  architecture) before building; project leads lay out **plots** (footprints
  marked on the ground, typed per building) that individual builders then
  fill; terraforming is a separate discipline that runs first; a role
  hierarchy separates terrain, building and world-edit authority
  (team-published guides, **MEDIUM** — access caveat: several pages read
  only as search summaries). King's Landing: 5,201 houses, built inside
  plots. Professional studios (BlockWorks) and Hypixel's build teams show
  the same pattern: concept first, terrain first, roles separated
  (**WEAK-MEDIUM**).
- **Scale is chosen before building, from the finest detail that must
  survive** — 1:1 through 3:1 conventions in build-team practice
  (**WEAK-MEDIUM**). This project's playable-scale doctrine is the same
  decision taken in the opposite direction, deliberately, and the research
  gives no reason to revisit it.

### 1.7 What the research says about reference imagery

Concept and mood imagery legitimately precedes the blockout — Barclay's
process runs concept → mood board → blockout (**MEDIUM**); Naughty Dog
deliberately colors and lights blockmesh to carry material *intent* to
artists (**STRONG**). But in every pipeline found, imagery authored before
the blockout is **style authority, never dimensional authority**: dimensions
are owned by metrics and validated by walking the blockout, and the art pass
that realizes the imagery is cosmetic by definition (§1.1). Applied here:
the whole's multi-view reference sheet is confirmed style-and-silhouette
authority; **a per-part image authored before the whole's layout exists has
no standing over geometry** — its legitimate successor is a style anchor for
the detail pass, generated (or re-generated) once the part's allocated box
is a fact. This confirms the demotion of the existing per-zone images, with
one refinement: they are not merely postponed — as *dimensional* claims they
are void; as mood material they may inform the stage-6 style anchors.

---

## 2. The recommended pipeline

This section is the plan of record: rounds are dispatched against it as
written. Stages are numbered; each states its artifact, its machine checks,
who authors it, and what makes the ordering structural rather than prose.
The design constraint carried throughout, from §1.1's economics and from
this project's own gate structure (human judgement fires on built, walkable
artifacts): **the first end-to-end walkable thing must exist early and be
cheap to throw away** — which is stage 5, reachable without any
authored-geometry work at all.

Vocabulary used below. A **place** is a node of the campaign's layout: a
room, a courtyard, an arena, a stretch of shore — the general unit, not a
castle-specific one. A **seam** is where two places connect. The **site
plan** is the whole map's design of record. The words "zone program" do not
appear in this pipeline; §2.9 states what replaces the zone as the unit of
authoring.

### Stage 0 — the metrics standard (engine, once; then maintained)

**What**: one machine-readable metrics table in the engine, in two parts.
*Player metrics* — engine facts of pinned 1.21.11, measured not chosen: the
collision box, eye height, the step/jump rule (walk-up ≤ 9/16, jump ≤ 20/16,
the jump arc), swim and fall facts. Most of these are already measured and
live in the nav model; stage 0 collects them into the one table. *Building
metrics* — standards this project fixes: minimum corridor width and
clearance, the standard doorway opening, stair pitch standards (rise:run
patterns that are pleasant, not merely legal), storey heights, cover height
if combat design wants one, and the **kit grid**: the footprint quantum for
pieces, the rule that footprints are multiples of it (§1.3), the datum
convention (where a piece's floor sits in its box), and the standard seam
opening sizes.

**Artifact**: the metrics table, engine-owned data, exported like the schema
so tools and gates read the single authority.

**Machine checks**: existing gates and the nav model read the table instead
of hard-coding numbers; a kit piece's metadata declares its footprint class
and is refused if its bytes disagree; a seam opening that matches no
standard size is a named refusal.

**How the numbers are set**: player metrics by measurement (largely done).
Building metrics by a **metrics-gym demo level** — rooms, corridors, doors
and stairs at candidate dimensions, walked once in-game, per §1.2's "metrics
zoo" practice; the walk fixes the numbers and the demo level joins the demo
queue as the standard's living documentation. **Uncertain, marked here**:
the actual values. Nothing transfers from Source units or Unreal
centimeters; Minecraft's 1-block granularity at player scale makes our
minimum-width choices coarser than anyone else's. The gym exists because
the numbers cannot be cited, only calibrated.

### Stage 1 — fiction and mission (campaign; exists)

The staged DSL through the campaign quest plan: world/setting, NPCs,
classes, the quest graph with its beats, locks and keys. Unchanged by this
plan, and it is the right first stage: §1.5's strongest finding is that the
mission structure precedes and constrains space. **Machine checks**: the
existing quest-graph reachability validation.

### Stage 2 — the whole's reference and written geometric brief (campaign)

**What**: two artifacts about the whole, before any layout exists. (a) The
**multi-view reference sheet** of the whole — the existing practice stands:
first view confirmed for style, later views anchored on the first, style
authority, rank-only, never a gate. (b) The **written geometric brief**: the
whole's numeric facts stated as text-with-numbers — overall extent class,
proportions, standoffs, dominant vertical, ground planes, anything the
campaign's fiction fixes about the site's shape. The brief exists because a
drift is checked against text, and because §4 shows what happens when the
only statement of the whole's geometry is a picture.

**Machine checks**: none on the image (rank-only stands). The brief's facts
become identities the site plan is checked against in stage 4 — so a fact
must be stated as a number or a comparison, and a brief with no checkable
facts is a named finding at stage-4 time.

**What does not exist at this stage**: per-place concept art (§1.7).

### Stage 3 — the layout graph (campaign)

**What**: the campaign's space as a graph, before any coordinate exists.
Nodes are places, each with an intent tag (arena, hub, vista, gate,
shortcut-landing, …) and a rough size class from the metrics table. Edges
are connections, each with its class (walk / stair / drop / barred /
vision), its gating (which key, quest state or unlock opens it), and its
intended one-way-ness. The graph names the entry, the goal, the critical
path, and every loop (a shortcut is an edge that closes a loop — stated as
graph structure, not prose). Every quest beat from stage 1 binds to a node.

This is Dormans' mission→space bridge and Unexplored's backbone (§1.5): the
topology carries the global guarantees, so it is authored and checked as an
object of its own, cheaply, before geometry can make it expensive.

**Artifact**: the layout graph, a campaign design-of-record document
(schema-enforced JSON like every other stage).

**Machine checks**: every node reachable from the entry respecting gating;
the critical path exists and visits every beat-bound node in a quest-legal
order; every one-way edge leaves no softlock (from any node a body can
reach after any legal sequence of drops, some path to the critical path
exists); every shortcut edge closes a loop; every vision edge's two ends
are distinct nodes; size-class sums against the playtime target as an
advisory measurement (**uncertain**: the pacing coefficients — blocks of
route per minute of play — have no citable value; the first walked blockout
calibrates them, until then the measurement carries numbers and no
threshold).

### Stage 4 — the site plan (campaign; the whole's design of record)

**What**: the geometric embedding of the layout graph. The site plan owns,
for the entire map: the world region and its datums (ground planes, water
plane, the vertical order of things); **a box for every node** of the graph,
on the metric grid, with the node's floor datum; **a seam for every edge** —
positioned on a face shared by its two boxes, with its opening size from
the standard set and its rise stated; the volumes the whole itself owns
(massif, ground, sky clearances); and the identities that bind it to the
stage-2 brief (extent, proportion and standoff facts as guarded
comparisons).

Two rules carried from the research, stated as obligations of this stage:
**extent flows down** — the region comes from the brief, boxes partition
the region, and a part is never the authority for any total (§1.4, and §4
below is the measured cost of the reverse); and **seams are allocated, not
discovered** — a seam is placed by the site plan on a shared face with its
opening and rise, so two places connect by construction (§1.3's doorway
discipline), and the two-places-cannot-mate failure class is resolved at
allocation time, where both boxes are still free. (**Judgement**: making
seam co-axiality the site plan's job rather than adding a face-adaptation
construct — chosen because every documented kit practice standardizes the
connection and moves the pieces, never the reverse.)

**Artifact**: the site plan, campaign design of record.

**Machine checks**, all at site-plan validation, all upstream of any
geometry: boxes are disjoint, on-grid, inside the region; every graph edge
has exactly one seam and its geometric feasibility holds (opening from the
standard set; a stair edge's rise achievable in its box's run at a standard
pitch; a drop edge's fall within the survivable range from the metrics
table); every node's box fits its declared size class; the brief identities
hold, and a violation is a refusal naming both numbers; the graph and the
site plan agree exactly (a node without a box, a box without a node, an
edge without a seam, a seam without an edge — each a named refusal).

### Stage 5 — the whole-map blockout, compiled and walked (campaign)

**What**: the whole map as massing, **derived mechanically from the site
plan and the metrics table — authored by no one.** For every box: floor at
its datum, clearance to its ceiling, shell walls; every seam: its opening
cut at its allocated cells, its stair/ramp massing realizing the declared
rise at a standard pitch; the whole's ground and massif volumes as plain
mass. Deterministic (same site plan + seed → byte-identical), compiled by
the existing placement machinery into a real world, joinable.

This is the artifact the old method never had (§4): the whole, walkable,
before any detail exists, cheap to regenerate in seconds. It is deliberately
not an authored program: an author cannot introduce a defect into it, and a
site-plan revision regenerates it without any hand-edit surviving to be
lost. (**Judgement**: blockout as derived data rather than an authored
program — the research's blockouts are hand-built because their site plans
are drawings; ours is schema data, so the derivation can be total. What
would falsify it: if walked blockouts repeatedly need hand-shaped massing
to be judgeable — a vista that needs its landform before scale reads — the
derivation gains parameters, not hand edits.)

**Machine checks**, the full battery, at map scale, on every regeneration:
closure and envelope over the whole; every seam's edge proof with its
declared rise against the built bytes; per-cell reachability from the entry
through declared seams only, reaching every node's floor; the critical
path walked by the bot (mineflayer, existing harness) end to end under
quest gating; determinism (double-build byte-identity); and the site-plan
identities re-checked against the built world. Every check with a stated
binding count, zero bindings red, per the standing vacuity rules.

**Human gate — the first walkable artifact**: walk the blockout in-game;
judge scale, pacing, route legibility, and the massing silhouette from the
named exterior views (renders beside the stage-2 reference). This is §1.1's
playtest gate, and the loop is deliberately tight: a finding edits the graph
or the site plan and regenerates — minutes, not rounds. **Detail work on
any place is not dispatched until the whole's blockout has passed this
walk.** That ordering is structural, not prose: stage 6's inputs (a place's
box, seams, datums, contract) exist only as outputs of the passed site
plan, and a detail program has no compilable form without them (§2.8).

### Stage 6 — detail per place, inside the frozen allocation (campaign)

**What**: the elaboration of each place from massing to finished interior
and exterior, the unit of dispatch being the place. A detail program is
handed, by the toolchain and not by a brief's prose: its box and frame; its
datums and the whole's material palette as bound parameters; its seams
(cells, opening, class, rise) as fixed obligations; and the blockout's
traversal contract for the place as the thing it must preserve. Within
that, the program is free: kit pieces from the library, the grammar's full
vocabulary, per-place style anchors — imagery generated now, anchored on
the whole's reference style (§1.7), rank-only as ever.

**Machine checks**: everything the piece pipeline already has (the gates,
the admission procedure, render review), plus the check this stage exists
for — **traversal equivalence against the blockout**: the detailed place
keeps its seams at their allocated cells, keeps every blockout-reachable
region reachable with the same edge classes and rises, and adds no new way
out of the place (new *interior* structure is free; a new hole in the
allocation's boundary is a refusal). This is §1.1's "art pass preserves
functionality," held by a machine because no head holds it here. A detail
program that wants different traversal is asking for a site-plan revision:
the site plan changes visibly, stage 5 re-runs (regeneration is cheap; the
re-walk of the whole is the price, stated rather than hidden), and the
allocation is re-handed. The part itself has no surface by which to move
an allocation — refusal, not accommodation, in both directions.

**Human review**: interior atmosphere per place, on renders, as the
existing practice does — with the boundary now explicit: this review judges
appearance and cannot move geometry the contract freezes.

### Stage 7 — whole art pass, full validation, release (campaign; mostly exists)

Connective dressing the whole owns (the material continuity pass, wear
gradients, the silhouette's roofscape), relight, the composed render review
beside the stage-2 reference, PackTest, the full bot playthrough, the
release ladder from a frozen tree. Existing machinery; the one addition is
that the traversal-equivalence check of stage 6 runs once more over the
dressed whole, so a decoration pass cannot strand a route (§1.1's bad-jumps
practice, in our medium).

### 2.8 What makes the ordering structural

The old method's deepest process defect was an ordering that existed as
prose (§4). Here, each stage's artifact is the *input the next stage's tool
requires*, so inversion is not a discipline question:

- The site plan validates only against a layout graph (stage 4 checks are
  graph↔plan agreement); it cannot exist first.
- The blockout is derived data; there is nothing to author before the site
  plan exists.
- A detail program compiles only against a handed allocation; there is no
  authorable surface for a free-standing region. The campaign manifest
  carries no per-place region at all — regions live in the site plan alone,
  and the one number that used to be free (the map's own region) is bound
  by identity to the brief.
- CI's campaign audit walks stage artifacts in order and reds a campaign
  whose later-stage artifact exists without its earlier-stage input — the
  same event-bound shape the standing rules demand of every gate.

### 2.9 The unit of authoring, and the DSL's shape

The zone — a self-regioned, self-reviewed geometry document — ceases to be
a unit of anything. What a campaign authors, in order: a quest plan (§ stage
1), a brief (stage 2), a **layout graph** (stage 3), a **site plan** (stage
4), and **per-place detail programs** bound to allocations (stage 6). The
engine authors the blockout. The kit library remains the shared vocabulary
across campaigns, now under the stage-0 metric standard.

For the DSL this means: the campaign's spatial surface is the graph and the
site plan — two new schema stages between the quest plan and geometry — and
the current fixed-stride `areas` surface is superseded by placement from
the site plan. A "place" is the general unit (the engine test: a creator
building an open island, a village, or a cave system uses nodes, seams,
boxes and datums identically; nothing in the vocabulary is castle-shaped).
Whether the built world is one contiguous mass or several detached sites is
a site-plan fact (multiple root boxes), not an engine distinction. All of
this implies specs — the schema stages, the metrics table, the blockout
derivation, the traversal-equivalence check, the audit bindings — which
their own rounds write; this document deliberately stops at the design.

### 2.10 Order of build, for the dispatching of rounds

The first thing built is the thin vertical slice of stages 3→5: a minimal
graph schema, a minimal site-plan schema with partition/seam/identity
checks, the blockout derivation, and the walk loop — proven on a small
fixture campaign, so a walkable whole exists within the first execution
rounds and every later check deepens an already-walkable path. The metrics
gym (stage 0) lands beside it, since stage 4-5 checks read the table.
Detail-stage machinery (traversal equivalence, allocation handing) follows;
the full battery and the audit bindings close it out. Gallery elements land
with each new surface in the same PRs, per the standing rule. What must not
happen: building stage 6's machinery first — that is the old method's
ordering, arriving through the build order.

---

## 3. What it costs

Stated at full price, no softening.

**Thrown away — the campaign's geometry design, entirely.** The eight zone
programs (regions from 19×6×24 to 41×14×125), the map program that composed
them (628 rules, 275 params, a 19-space/23-edge map contract), their
fixture regions and seeds, the per-zone acceptance renders *as review
authority*, and the per-zone concept images *as geometric authority* (they
may inform stage-6 style anchors; as dimensional claims they are void). The
per-zone review hours spent on them are sunk. There is deliberately **no
migration path and no salvage pass**: a preserved finished part is a
constraint the whole must accommodate — extent flowing up, the defect
itself, re-entering as thrift. The campaign's fiction (story, beats,
quests, the tide document) and the whole-map reference views survive; its
geometry restarts at stage 2 with the reference views already in hand.

**Thrown away — decisions.** spec-0040 and ADR-0020 as decisions, and
ADR-0004's composition consequences; a supersession ADR is owed (one
document, its own round). The zone-composition test suite dies with the
zones it asserts (the checker-teeth and adversary fixtures are engine tests
and survive). `zones.json` as the manifest of self-regioned programs goes;
the audit's campaign sweep re-binds to the stage artifacts of §2.8.

**Kept, on its own merits — most of the engine.** The grammar language and
its gates, the prefab admission procedure, the render loop, the assembled
world and nav model, the placement machinery, the bot harness: all of it is
the machine-validation culture §1.4's strongest sources arrive at, and
nothing in the research argues against generating geometry from a language
— it argues about *what is generated when*, which is what the stages fix.
The spatial-contract checker specifically is kept **by this document's own
argument, not by deference to the void ADR that built it**: stage 5 and 6
obligations need exactly a declared-intent-vs-built-bytes checker, its
obligations were adversarially validated, and the research's re-validation
practice (§1.1) demands its function. The piece library survives as the
kit, with one honest unknown: **uncertain** how many existing pieces
conform to the stage-0 grid once its quantum is fixed — the admission
audit against the metrics table will number it, and a nonconforming piece
is reworked at the footprint per §1.3, not patched.

**Built new**: the metrics table and gym demo; the layout-graph stage and
checks; the site-plan stage and checks; the blockout derivation and its
map-scale battery; the traversal-equivalence check; the allocation-handing
surface for detail programs; the superseding of the `areas` stride; the
audit bindings; gallery elements for every new surface; docs and skill
updates in the same PRs. Each implies a spec; numbers are handed by the
dispatching planner, per standing practice.

**Process cost, named**: stage 5's human walk is a real gate on a real
person's time, once per site-plan revision that reaches it. The mitigation
is structural — machine checks run before the walk is requested, and
regeneration is seconds — but a campaign that churns its site plan will
spend walks. The research's answer (§1.4) is that this is the cheapest
possible place to spend them, and the old method's answer was to spend
them never, which §4 prices.

---

## 4. Where the old method went wrong, checked against the research

Read for this section, after the research was in: the three trials
(`docs/trials/`), spec-0040, ADR-0020, ADR-0004.

**The failures the trials measured are the ones the research predicts —
specifically, they are §1's named failure modes, arriving on schedule.**

- *No stage at which the whole existed cheaply.* The industry's central
  artifact — the blockout of the entire level, walked before detail
  (§1.1) — had no equivalent. The first time the whole of the composed
  campaign existed in any form was trial-0003, after all eight parts were
  finished and reviewed. The trial's own headline — a 1:5.5 site against a
  compact brief, the crown subtending 6.7° against a derived 27°,
  "computable from the manifest alone before any composition ran" — is
  §1.1's "the most common problem is scale," discovered at the most
  expensive possible moment because the cheap moment was never scheduled.
- *Per-part review before any whole existed = the art pass before the
  blockout gate.* Eight zones were detailed, rendered and accepted
  individually — finished art, in the economics of §1.1 — and the method
  then discovered the layout. The research's cost model (blockout cheap,
  art expensive, "premature art pass locks in early design mistakes") is
  exactly the bill now being paid in §3.
- *Extent flowed up.* The map's region was set to the arithmetic sum of the
  parts' pre-existing depths (trial-0003, verified on the manifest). Every
  whole-owns-space source in §1.4, and every top-down generation source in
  §1.5, forbids precisely this; Dormans' "random feel… lack of overall
  structure" is a mild description of the measured outcome.
- *Fit by measurement, not by construction.* Trial-0003's R5: of ten seams,
  **zero** aligned by construction; the three that held rested on offsets
  read out of the parts' bytes and guarded after the fact. §1.3 is an
  entire discipline built so that this number is total: matching
  footprints, standard doorways, "only matching sizes will fit together."
  The parts shared no footprint quantum, no standard opening, no datum
  convention — each was reviewed against its own reference, which is
  precisely the condition §1.3's rules exist to prevent.
- *Parts owed the whole nothing, because nothing existed to be owed.* Five
  of eight parts carried no contract; no part declared a datum as a
  bindable parameter; two fixed as arithmetic the offsets the whole needed
  to set (trial-0003 R6). In kit terms (§1.3): pieces with no footprint
  discipline, no standard transitions, pivots chosen per piece.

**What the old method got right, and the research confirms it** — this list
matters because §2 keeps all of it:

- *Determinism as a hard invariant.* Far Cry 5 required per-sector
  determinism for exactly our reason — independently generated parts must
  stitch reproducibly (§1.5). ADR-0006 is ahead of, not behind, practice.
- *The compiler as the layout authority rather than runtime jigsaw.*
  Vanilla jigsaw's silent local-only degradation (§1.6) is the strongest
  possible argument for ADR-0004's amendment; a provably completable delve
  cannot be assembled by a mechanism whose failure mode is a quiet hole.
- *The machine-gate culture.* Binding counts, vacuity rules, the contract
  checker's adversary rounds — this is AC Origins' daily automated world
  validation (§1.4) built small, and it is the half of the constitution the
  new pipeline leans on hardest.
- *Playable scale.* Build-team practice chooses scale from the finest
  detail that must survive (§1.6); choosing player scale and dropping the
  detail is the same decision made deliberately, and trial-0001's re-judged
  west front shows it can carry identity.
- *The last document had the right rule and could not hold it.* spec-0040
  §3c ("extent flows down, never up") and its ordering sentence ("the map
  program is authored before the zones, as the site plan") state half of
  §2 — written *after* eight parts existed, with the ordering living in
  prose, which the spec's own §1.9 records as binding nothing. The failure
  is instructive rather than embarrassing: a correct rule arrived at by
  trial-and-error, too late, unenforced — versus the same rule arrived at
  by the practice, first, with the ordering structural (§2.8). That
  difference is the reason this document exists: established practice is
  read before it is re-derived by trial and error.
- *The trials themselves.* Frozen briefs, claim audits, attribution
  ladders, instrument bounds — the measurement discipline is sound and
  §2's gym and walks inherit it. What the trials measured was true; the
  method they measured was the problem.

One honest disagreement with the void documents' framing: spec-0040 §2
argued that "mutual consistency needs one medium" and therefore the whole
must be a grammar program composing the parts' programs. The research does
not support the inference. Every §1.4 source separates the whole's plan
(map, graph, persistent level — a *data* artifact) from the parts' medium
(kit pieces, meshes), and couples them by contract, not by co-residence in
one language. §2 follows the research: the whole lives in the graph and
site plan; the one-derivation property that spec-0040 bought with
composition is bought instead by the blockout being derived from the plan
and the details being checked against the blockout.

---

## 5. What could not be settled, and what settles each

1. **The building-metric values** (stage 0). No citable numbers exist for
   Minecraft-scale adventure interiors; Source/Unreal values do not
   transfer. Settled by: the metrics-gym demo level, walked once; the walk's
   rulings freeze the table.
2. **Pacing coefficients** (stage 3): blocks of route per minute of play,
   for a 2–3 h delve with a 10 h ceiling. No source publishes transferable
   density budgets (BotW's rhythm was calibrated against Kyoto, not stated
   as numbers). Settled by: measurement on the first campaign's walked
   blockout and first full playtest; carried as thresholdless measurements
   until then.
3. **Whether the human walk is needed per site-plan revision or only at
   first pass and majors** (stage 5). Practice varies (Naughty Dog: "grab
   anybody", continuous; our constraint: the project's one human QA hour).
   Settled by: the first campaign's actual revision count; if walks become
   the bottleneck, the bot's route metrics take the minor revisions — a
   threshold decision taken *with that data*, not before.
4. **How much of the existing piece library conforms to the stage-0 grid**
   (§3). Settled by: the admission audit re-run against the metrics table
   once its quantum is fixed; the number lands in the round summary.
5. **Whether graph and site plan stay two artifacts or merge** (stages
   3–4). Kept separate here because the graph's checks are valuable before
   any embedding exists (§1.5), at the cost of one agreement check between
   two documents. Falsifier: if in practice every graph edit is
   immediately a site-plan edit and the agreement check never fires alone,
   merge them.
6. **Blockout massing fidelity** (stage 5): whether pure derived massing is
   judgeable for silhouette, or landform-carrying campaigns need shaped
   massing parameters. Settled by: the first campaign's stage-5 review
   against its reference views; the derivation gains parameters if and only
   if a walk says scale reads wrong without them.
7. **The general engine's edge cases for the site-plan vocabulary**: open
   terrain with no rooms, underwater places, pure-vision set pieces.
   **Judgement** that nodes/seams/boxes/datums cover them (a shore is a
   node with a sky envelope; a vista is a vision edge); falsifier: the
   first campaign brief the vocabulary cannot state without a
   castle-shaped workaround — which, per the no-hacks rule, is then a
   refused feature or a first-class surface, decided at that evidence.

---

## Appendix: sources

Primary sources, with access notes. Ratings as defined at the top.

**Pipeline and blockout**
- David Shaver (Naughty Dog), "Level Design Workshop: Invisible Intuition —
  Blockmesh and Lighting Tips to Guide Players and Set the Mood", GDC 2018.
  Slides: davidshaver.net/DShaver_Invisible_Intuition_GDC2018.pdf; vault:
  gdcvault.com/play/1025360. STRONG — slides read in full, quotes verified
  twice independently.
- Epic Games, "Greyboxing in Unreal Editor for Fortnite",
  dev.epicgames.com/documentation/fortnite/greyboxing-in-unreal-editor-for-fortnite.
  STRONG — read.
- Robert Yang, *The Level Design Book*: book.leveldesignbook.com — pages
  /process/overview, /process/blockout, /process/blockout/metrics,
  /process/env-art. MEDIUM — read.
- Michael Barclay, "Pillars of Creation: My Level Design Process",
  mikebarclay.co.uk/pillars-of-creation. MEDIUM — read.
- Max Pears, "Level Design for Combat", Game Developer, 2019:
  gamedeveloper.com/design/level-design-for-combat. MEDIUM — read.
- Valve Developer Community, "Dimensions (Half-Life 2 and Counter-Strike:
  Source)": developer.valvesoftware.com/wiki/Dimensions_(Half-Life_2_and_Counter-Strike:_Source).
  STRONG — read in full via archive copy (live wiki blocks non-browser
  fetches).
- Christopher Totten, *An Architectural Approach to Level Design* (CRC,
  2014/2019); Totten (ed.), *Level Design: Processes and Experiences* (CRC,
  2016); Scott Rogers, *Level Up!* (Wiley, 2nd ed. 2014). STRONG as
  publications; consulted at table-of-contents / detailed-review level only,
  and used only for claims stated at that level.

**Kits and grids**
- Joel Burgess & Nathan Purkeypile, "Skyrim's Modular Level Design", GDC
  2013 — author transcript: blog.joelburgess.com/2013/04/skyrims-modular-level-design-gdc-2013.html;
  reprint: gamedeveloper.com/design/skyrim-s-modular-approach-to-level-design.
  STRONG — read in full, quotes verified twice independently.
- Joel Burgess, "The Iterative Level Design Process Used to Ship Fallout 3
  and Skyrim", GDC 2014 — author transcript:
  blog.joelburgess.com/2014/07/gdc-2014-transcript-iterative-level.html.
  STRONG — read.
- Joel Burgess & Nate Purkeypile, "Fallout 4's Modular Level Design", GDC
  2016 — slides via slideshare/archive.org (GDC2016Burgess); vault:
  gdcvault.com/play/1023202. STRONG — slide text read.
- Lee Perry (Epic), "Modular Level and Component Design", Game Developer
  magazine, Nov 2002 — PDF via archived Epic UDK docs. STRONG — read in
  full.
- Bethesda, "Bethesda Tutorial Layout Part 1", Creation Kit wiki (mirror:
  ck.uesp.net). STRONG — read via archive.
- Epic, UDK "Modular Environment Creation" documentation. STRONG — read via
  archive (live page 403s).

**World assembly**
- Epic Games, "Managing Multiple Levels in Unreal Engine" and "World
  Partition in Unreal Engine", dev.epicgames.com. STRONG — read.
- Kotaku, "How The Witcher 3's Developers Ensured Their Open World Didn't
  Suck", 2015 (named CDPR sources). MEDIUM — read.
- Nintendo, "Change and Constant: Breaking Conventions with The Legend of
  Zelda: Breath of the Wild", GDC 2017 (gdcvault.com/play/1024562); CEDEC
  2017 talks via Matt Walker's translation
  (gist.github.com/idbrii/e39fe96279aa1670319bfa521d907399). STRONG —
  translation read; Kyoto quote via press (gamesradar.com). 
- Nicholas Routhier (Ubisoft), "Assassin's Creed Origins: Monitoring and
  Validation of World Design Data", GDC 2018 (gdcvault.com/play/1025452).
  STRONG — abstract read; talk not watched in full.
- Game Developer, "Designing biomes to create memorable adventures in
  Assassin's Creed Valhalla" (named Ubisoft sources). MEDIUM — read.
- "Dark Souls Design Works" interview, via darksouls.wiki.fextralife.com
  transcription. MEDIUM — read via fan transcription; exact wording treated
  cautiously.
- Starr Long (NCsoft), "Strike Teams: Cross Discipline Saviors", GDC Europe
  2005, via Game Developer postcard. MEDIUM — read.
- Philippe Bergeron (Ubisoft), "360 Approach for Open World Mission
  Design", GDC 2016. STRONG for existence/abstract; characterized from the
  vault abstract only.
- Joel Burgess, "Motivating Players in Open World Games", GDC 2011 — POI
  density definition via search excerpt of the author transcript only
  (blog unreachable at research time). MEDIUM as used.
- PC Gamer / GoNintendo interviews with Team Cherry (Hollow Knight,
  Silksong). WEAK-MEDIUM — excerpts only.

**Procedural generation**
- Joris Dormans, "Adventures in Level Design: Generating Missions and
  Spaces for Action Adventure Games", PCG Workshop at FDG 2010:
  pcgworkshop.com/archive/dormans2010adventures.pdf. STRONG — read, quote
  verified twice independently.
- Adam M. Smith & Michael Mateas, "Answer Set Programming for Procedural
  Content Generation: A Design Space Approach", IEEE TCIAIG 3(3), 2011:
  adamsmith.as/papers/tciaig-asp4pcg.pdf. STRONG — pp. 187–190 read.
- Shaker, Togelius & Nelson, *Procedural Content Generation in Games*
  (Springer 2016), free author version: pcgbook.com. STRONG — chapter
  structure consulted; interiors not read.
- Boris the Brave, "Dungeon Generation in Unexplored" (2021):
  boristhebrave.com/2021/04/10/dungeon-generation-in-unexplored; Tommy
  Thompson, "Unexplored's Secret: Cyclic Dungeon Generation", Game
  Developer. MEDIUM — both read, mutually corroborating.
- Etienne Carrier (Ubisoft), "Procedural World Generation of Far Cry 5",
  GDC 2018 (gdcvault.com/play/1025557), read via christianjmills.com
  detailed notes. STRONG talk / MEDIUM as accessed.
- Benoit Martinez (Ubisoft), "Ghost Recon Wildlands: Terrain Tools and
  Technology", GDC 2017 (gdcvault.com/play/1024029), via 80.lv/SideFX
  writeups. STRONG talk / MEDIUM as accessed.
- Oskar Stålberg on WFC in Bad North (Everything Procedural 2018,
  youtube.com/watch?v=0bcZb-SsnrA) and his own published statements.
  STRONG for the author's words; talk not watched in full.
- Spelunky level generation: practitioner writeups of Derek Yu's design
  (takenapeveryday.wordpress.com 2016); primary book (Yu, *Spelunky*, Boss
  Fight Books) not consulted. MEDIUM.

**Minecraft practice**
- minecraft.wiki, "Jigsaw structure", "Jigsaw structure/Config",
  "Structure Block". MEDIUM — read.
- Microsoft Learn, "Introduction to Jigsaw Structures" (Bedrock). STRONG —
  read.
- WesterosCraft guides (New Builders Guide, Terraforming Guide, server
  roles, project pages). MEDIUM — access caveat: several pages read only as
  search summaries (fandom 402 / site 404 at research time).
- BlockWorks (Wikipedia; Screen Rant interview with James Delaney), Hypixel
  build team (wiki/forum). WEAK-MEDIUM.
- Planet Minecraft scale guides; Build the Earth scale assumption.
  WEAK-MEDIUM.
