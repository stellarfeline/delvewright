# ADR-0032: One map route — a site is a plan of scenes, and a scene is drawn whole

- **Status**: Accepted
- **Source**: the owner's direction on spec-0098; the work products of the three
  campaigns that reached a designed exterior (two released on `areas[]` with one
  whole-site piece, one in flight on a site plan with 3 of 53 places detailed);
  the implementation of spec-0098 and its §14 record of departures, rulings and
  corrections; the demo level that walked the finished route (`docs/demo-levels.md`,
  the spec-0098 row).
- **Refines**: ADR-0022 (the whole-first pipeline — stands; this names what a
  stage-6 part is). ADR-0012 (the skill is the front end — stands; the skill
  offers one placement route).
- **Constrained by**: ADR-0001, ADR-0003, ADR-0004, ADR-0006, the general-engine
  rule, the no-hacks rule.

## Context

Two map routes existed when this was decided. **Route A**: a creator-written
generator painted a whole site into a voxel grid, the grid became one grammar
`Program`, the `Program` one piece, the piece one `areas[]` area. **Route B**
(ADR-0022): a layout graph, a site plan, a derived blockout, and one piece per
place inside a frame the plan handed it (spec-0050).

Every campaign that playtested well was route A, and every route-A campaign paid
the same price: two thousand lines of script per site, five private
re-implementations of judgements the compiler makes (walkability, falls, reach,
shortcut length, gate regions), an artifact of record nobody can read, and
ADR-0022's stages bypassed. Route B's first town found the opposite defect:
the frame stopped at the play space, so a detailed building read as the
blockout's box from outside and an open place stayed walled, and the one
remedy a gate named was unreachable. Neither route was the method; each was
half of it.

## Decision

1. **There is one map route: a site plan of one or many places, each place a
   piece that owns its outside** (spec-0098). A scene — a church, a cottage, a
   road, a bridge — is authored with route A's freedom: one program, the
   scene's own, drawing walls, roof, facade and ground inside the frame the
   plan hands it. The frame is the place's **claim**: a cuboid of its play
   space, its floor course, the one-cell ring its walls may stand in, the
   ground under its plot down to the terrain (a ground place) or a declared
   underside (`base: {"aloft": n}`, a hung place: a bridge, a gantry, a
   treehouse platform) and, roofed, its lid and a declared roof zone
   (`roof: {courses, eaves}`), minus the cells the ownership rule awards to a
   neighbour. The whole is the plan: which scenes touch, where, with what
   crossing, on what ground.

2. **Route A is the degenerate case, not a second route.** A site plan with one
   box and no seam hands its one piece the whole site. The `/new-delve` skill
   offers the site plan and only the site plan at step 2; `areas[]` remains the
   engine's assembly surface for library pools and is not a map route the skill
   presents. No threshold decides between one place and many: that is the
   layout graph's node count, a design judgement.

3. **A wall exists only where a design declares one; the engine never writes a
   wall, a fill or any block nothing declared — it only checks.** A site-plan
   box bounds where a design may draw; it is not a box wrapped in walls. What
   every cell no place claims becomes is declared too: the plan's required
   `fill` — `solid` (an enclosed site: a dungeon, a cave, its places carved out
   of rock) or `open` (a town, a camp: a declared terrain under sky, flat on a
   datum or a heightmap the creator drew) — overridden per region by the
   plan's `volumes[]`. On an `open` site, ground no place claims is the
   **commons**: ordinary walkable ground every place may open onto through its
   own openings, with no declaration. What the whole owns is what the plan
   declares — its terrain, its volumes, its region and surround, the
   synthesized names — and every proof: the seam battery, reach, the
   unallocated-crossing sweep, sightlines, identities, exposure, light, danger
   and stranding, over bytes, indifferent to who wrote them. A cell two scenes
   would both draw with no rule to award it is a plan refusal (`DW0827`), not
   an engine arbitration. The stage-5 blockout's shells are review stand-ins
   for pieces not yet drawn, and a stand-in never ships: the staging gate
   refuses a build whose derivation still masses any place.

4. **A connection is designed, never derived.** The layout graph's edges and
   the plan's seams are the design of how scenes meet; every seam declares its
   `form` in a few words, and both places it joins are handed that sentence. A
   contested plane between two scenes goes to the place whose floor course it
   is, else to the one roofed place among them, else to the `a` of the seams
   the plan allocates across it, else to the one hung place standing in a
   ground place's sky ring; a plane no rule awards is refused. The engine
   computes only arithmetic (corners, sills, rises) and refuses every crossing
   it did not allocate (`DW0838`), the commons excepted: two places that both
   open onto the commons are joined through it, and the binding counts them. A
   connector that is itself a structure — a bridge, a long stair — is a place
   of its own with its own piece. A way a body climbs on a ladder or a vine is
   a `climb` connection, with no treads and no sill.

5. **The per-piece medium is open.** A computed `Program`, bound by
   `delvec detail`, is the medium; a drawn medium is proposed in a draft record
   that has not landed and is decided on its own evidence. Whatever the medium,
   its output is a piece with a spatial contract, and the engine's judgements
   answer at `delvec detail` on that piece before the whole is built.

6. **A site-plan check confirms a declared intent and never restricts.** A
   box's footprint is any whole number of blocks; its extent and headroom are
   the author's declaration and the piece may not exceed them; an opening is a
   standard the metrics table names or a size the seam declares; a drop cap is
   the plan's own `max_drop`; a stair no standard pitch fits is a warning about
   the stand-in, never a refusal. The enclosed spaces of a piece are its
   author's declaration, and closure confirms exactly those — zero declared
   passes with its count stated. A place may be scenery (`reached: false`): a
   thing built to be seen and never entered, proven unreached both ways, owing
   no node anchor, no play light and no way in.

## Consequences

- spec-0098 is the implementation of §1–§4 and §6; the skill's step 2 offers
  the site plan alone and its step 9 details every campaign.
- The skill does not present `areas[]` as a way to make a map. No released
  campaign is owed anything (it is built by the engine it pins).
- Between the scenes stands only what the plan declares — the `fill`'s terrain
  under sky, or its rock, and the plan's volumes — never an engine wall.
- A site-plan build ships only fully detailed: the staging gate refuses a box
  the derivation still masses.
- Landscape first: an open site's ground is one declared heightfield the whole
  owns, judged at stage 4 beside the boxes before any piece exists. On a ground
  place the ring of ground cells around the plot is fixed by the whole, derived
  from the terrain, and no piece may write it (`DW0990`), so every plot
  stitches into the map by construction; a crack between two grounds is still
  measured over bytes. The ring fixes ground only — what stands above it is the
  owner's, and a stair or ramp lands inside a place. Where a seam crosses the
  ring at grade, the ring's ground there is the sill's course, flat across the
  opening; a seam aloft fixes no earth under it. A hung place owns no ground
  and has no ring to fix; terrain reaching into its claim is refused. No
  connector between two heights is ever engine-generated: the whole declares
  the difference and the owning piece draws what crosses it.
- The agent that designs a scene works from the handout `delvec allocation`
  emits and nobody types: what is built and in what style, the scene's own
  concept reference (the design record's row for the place, or its named
  absence before it is drawn), its position in the whole, the whole's
  reference sheet, its initial ground with the fixed ring, its seams with their
  `form` and its owed anchors. `delvec detail` reads the same object, voids
  every cell the place does not own after expansion, and ships each template
  with its void cells omitted, so the owner's block shows through in the game
  as in the model.
- Research into single-building craft is bounded: route A has demonstrated the
  building; the open craft questions are composition — how scenes meet, how a
  town's ground reads — and are researched when a campaign reaches them.

## Alternatives considered

- **Keep two routes and let the skill choose by size.** Declined: a threshold
  is a per-case judgement the constitution forbids, and the measured defect of
  each route is not its size but its ownership model.
- **One whole-site piece, always (route A alone).** Declined: linear script
  growth, no whole to walk before the parts are spent, and the five private
  checkers return.
- **Per-place pieces inside play-space frames (route B as landed).** Declined:
  the exterior is unreachable, which is the defect that opened this decision.

## Finalised against the implementation

The draft was a direction check; this text states what spec-0098 built. What
changed from the draft:

- A box is a cuboid with three declared planes (`floor`, `ceiling`, `base`);
  the draft knew no hung place. A hung place (`base: {"aloft": n}`) claims no
  ground, has no fixed ring, and the space under it is the commons or the rock.
- The draft fixed the ring's ground under every seam from the seam's two
  floors; built, only a seam at grade levels the ring to its sill, and a seam
  aloft fixes no earth.
- The commons is built: on an `open` site, unclaimed ground is walkable, two
  places may open onto it with no declaration, and `DW0838` excludes it from
  the crossing sweep. The draft had every undeclared crossing refused.
- `fill` is required with `solid` or `open`, and `open` carries the terrain as
  a tagged union, `flat` or `heightmap`. The draft said "ground at its datum
  and sky, or declared terrain" with no spelling.
- `form` is required on every seam and handed to both sides; `climb` is a
  connection class; a place may be scenery. None was in the draft.
- The checks that refused with no declaration behind them — the kit grid, the
  size and way classes, the opening floor, the metrics table's drop cap — are
  removed or turned into the author's own declaration (§6). The draft said
  nothing about them.
- The shipped template omits its void cells; the draft assumed the game would
  place `structure_void` as a hole, and the built world showed it does not.
- The draft named ADR-0030's drawing as the medium's proposed successor; that
  record has not landed, and §5 names it as a draft decided on its own
  evidence.
- "A volume detailed like a place" was the draft's first expected follow-on. It
  is not built; it stands below as a revisit trigger.

## Revisit triggers

- A campaign whose scenes cannot be tiled edge to edge, so the whole's ground
  between them is what the player mostly sees — the brief for a volume detailed
  like a place.
- A scene that must know its neighbour (a sightline composed across two scenes,
  a roof that must run over two places).
- The per-piece medium question is decided.
