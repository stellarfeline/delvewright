# ADR-0032: One map route — a site is a plan of scenes, and a scene is drawn whole

- **Status**: Proposed (draft — a direction check; finalised against what is built)
- **Source**: the owner's direction on spec-0098; the work products of the three
  campaigns that reached a designed exterior (two released on `areas[]` with one
  whole-site piece, one in flight on a site plan with 3 of 53 places detailed);
  the draft ADR-0030's context, read as evidence.
- **Refines**: ADR-0022 (the whole-first pipeline — stands; this names what a
  stage-6 part is). ADR-0012 (the skill is the front end — stands; the skill
  offers one placement route).
- **Constrained by**: ADR-0001, ADR-0003, ADR-0004, ADR-0006, the general-engine
  rule, the no-hacks rule.

## Context

Two map routes exist. **Route A**: a creator-written generator paints a whole
site into a voxel grid, the grid becomes one grammar `Program`, the `Program`
one piece, the piece one `areas[]` area. **Route B** (ADR-0022): a layout graph,
a site plan, a derived blockout, and one piece per place inside a frame the plan
hands it (spec-0050).

Every campaign that playtested well was route A, and every route-A campaign paid
the same price: two thousand lines of script per site, five private
re-implementations of judgements the compiler makes (walkability, falls, reach,
shortcut length, gate regions), an artifact of record nobody can read, and
ADR-0022's stages bypassed. Route B's first town found the opposite defect:
the frame stopped at the play space, so a detailed building read as the
blockout's box from outside and an open place stayed walled, and the one
remedy a gate named was unreachable. Neither route is the method; each is half
of it.

## Decision

1. **There is one map route: a site plan of one or many places, each place a
   piece that owns its outside** (spec-0098). A scene — a church, a cottage, a
   road — is authored with route A's freedom: one program, the scene's own,
   drawing walls, roof, facade and ground inside the frame the plan hands it.
   The whole is the plan: which scenes touch, where, with what crossing.

2. **Route A is the degenerate case, not a second route.** A site plan with one
   box and no seam hands its one piece the whole site. The `/new-delve` skill
   offers the site plan and only the site plan at step 2; `areas[]` remains the
   engine's assembly surface for library pools and is not a map route the skill
   presents. No threshold decides between one place and many: that is the
   layout graph's node count, a design judgement.

3. **A wall exists only where a design declares one; the engine never writes a
   wall, a fill or any block nothing declared — it only checks.** A site-plan
   box bounds where a design may draw; it is not a box wrapped in walls. What
   the whole owns is what the plan declares — its volumes, its region and
   surround, the synthesized names — and every proof: the seam battery, reach,
   the unallocated-crossing sweep, sightlines, identities, exposure, light,
   danger and stranding, over bytes, indifferent to who wrote them. A cell two
   scenes would both draw with no rule to award it is a plan refusal, not an
   engine arbitration. The stage-5 blockout's shells are review stand-ins for
   pieces not yet drawn, and a stand-in never ships.

4. **A connection is designed, never derived.** The layout graph's edges and
   the plan's seams are the design of how scenes meet; the place a connection
   names first draws the line where two scenes touch. The engine computes only
   arithmetic (corners, sills, rises) and refuses every crossing it did not
   allocate.

5. **The per-piece medium is open.** A computed `Program` is the medium today;
   ADR-0030's drawing is its proposed successor, decided on its own evidence.
   Whatever the medium, its output is a piece with a spatial contract, and the
   engine's judgements answer at `delvec detail` on that piece before the whole
   is built.

## Consequences

- spec-0098 is the implementation of §1–§4; the skill's step 2 and step 9 are
  rewritten with the release that carries it.
- The skill stops presenting `areas[]` as a way to make a map. No released
  campaign is owed anything (it is built by the engine it pins).
- Between the scenes stands only what the plan declares — ground at its datum
  and sky, or declared terrain — never an engine wall; a declared volume is
  dressed by stage 7, and a volume detailed like a place is the first expected
  follow-on (spec-0098 §10).
- A site-plan build ships only fully detailed: the staging gate refuses a box
  the derivation still masses.
- What undeclared space becomes is a declared mechanism, not the engine's
  choice: a site states `fill` — `solid` (the enclosed site: a dungeon, a cave)
  or `open` (a town: ground at a datum under sky) — required with no default,
  overridden per region by the plan's volumes. The engine fills only what was
  declared.
- Landscape first: an open site's ground is one declared heightfield the whole
  owns; the ring of ground cells around every scene is fixed by the whole,
  derived from the terrain, and no piece may write it, so every plot stitches
  into the map by construction; a crack between two grounds is still measured
  over bytes. The ring fixes ground only — what stands above it is the owner's,
  a stair or ramp lands inside a place, and at a seam the ring's ground is
  derived from the seam's two floors. No connector between two heights is ever
  engine-generated: the whole declares the difference and the owning piece
  draws what crosses it.
- The agent that designs a scene works from a handout `delvec allocation`
  emits and nobody types: what is built and in what style, the scene's own
  concept reference (generated after the walk, anchored on the whole's sheet),
  its position in the whole, the whole's sheet, its initial ground with the
  fixed ring, its seams and owed anchors.
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

## Revisit triggers

- A campaign whose scenes cannot be tiled edge to edge, so the whole's ground
  between them is what the player mostly sees.
- A scene that must know its neighbour (a sightline composed across two scenes,
  a shared roof).
- ADR-0030's medium question is decided.
