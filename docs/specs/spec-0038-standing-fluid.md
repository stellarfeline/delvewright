# spec-0038: Standing fluid — declared bodies, and the flood level as runtime state

- **Status**: Accepted
- **Basis**: the engine supports a dynamic world; the
  commissioning instance is a citadel on a tidal rock whose sea level answers
  what the party has done, opening some routes and closing others. Two further
  rulings shape every surface here: **flowing liquid is
  avoided unless deliberately designed** — a water level moves as a whole
  plane, so no source ever has a lower neighbour to flow into; and **a body of
  water is saturated by construction** — every cell of its volume is a source,
  and an emission that places some water and lets vanilla spread it is
  excluded outright (it is also nondeterministic, ADR-0006).
- **Specs**: 0031 (runtime state, region fill/clear, the gate; its §9 findings
  are two of this spec's reds), 0013/0026 (the horizon owns the ambient's
  physical facts, the flood level among them — and 0026's bases decide where
  the level attaches: any base that declares a water table, never the `ocean`
  spelling), 0048 (the piece-side waterline datum; this plane's boot level is
  the world-side fact it meets), 0042 (the region-write proof family this
  spec's events join). spec-0030's compile-time `flood` verb is not landed;
  the one flood physics this spec shares is the assembled-world flood
  (`assembled::flood` — the reach `DW0318` and `DW0851` already read).
- **ADRs**: 0003 (vanilla-first: the primitive is vanilla's own fluid), 0006
  (determinism: the waterline is a function of the DSL, never of fluid ticks)
- **Evidence**: every claim below is either demonstrated against the engine
  (`docs/specs/spec-0038-probes/`, commands inline, re-measured at engine
  revision `be8eab02` — the per-probe state is §4) or measured on the pinned
  1.21.11 (`tools/spike-fluid-plane/`, raw data in `observations.json`,
  re-runnable via `EULA=TRUE tools/spike-fluid-plane/run.sh`).
- **Non-goals**: §5. Deliberately refused, each with its disproof: a
  tide/water-named verb, a region-scoped water level, flow-and-settle
  emission, wading/swim traversal, a clocked plane driver, a second fluid,
  vehicles, a rise animation verb, an author-asserted ownership of a fluid
  cell.

## 1. What the existing mechanism fails to reach, and why

The general mechanisms adjacent to "the sea level changes" are: `fill-region`
/ `clear-region` (a declared region, written at runtime, one completability
rule `plan::RegionEvent` → `nav::region_state_at`), numeric runtime state in
the shared gate, `teleport`, `lethal_volumes[]`, and the assembled-world
flood physics (`assembled::flood`, which `DW0318`/`DW0851` read at the one
fixed level). Measured against the ruling, they fail in four distinct places:

1. **The model knows a fluid from a floor — landed — and still cannot see a
   body the water or a verb DELIVERS.** A runtime write now concludes from
   the block it writes: a fluid fill is impassable and never standable
   (`DW0544`), and footing laid from a skippable root is never credited
   (`DW0546`) — the soundness fix §2.1 demanded is current behaviour. What
   remains is the other half of probe 1 (§4, `water-deck`): the fixture
   **still builds green at `be8eab02`**, because the party reaches its
   mid-air 3×3 water "deck" by `teleport`, and nav deliberately models no
   relocation — a teleport "can only add reachability", which is sound for
   route existence and silent about where the body lands. A moving plane is
   a second relocator with the same shape: rising water carries a body
   through its swim reach, falling water sets it down. Nothing examines any
   cell a body is delivered to rather than walks to — that is §2.4's proof.
2. **Nothing checks where runtime water goes or comes from.** The same probe
   compiles a fill whose sources have open lower neighbours (flow out, the
   exact state the ruling forbids); symmetrically, `nav.rs`
   (`with_cleared`) documents that a clear opening a dry region into adjacent
   water is "optimistic there" — the model says passable, the sea pours back.
   Measured (spike, `plane_lower_ticking_edge`): a 192×192 hole cleared in the
   ocean's top layer with a ticking edge is **fully re-flooded in ≤40 s**
   (creep ≈ 4–5 cells/s per side — and with **zero flowing block-states at
   any sample**: over a source layer the healing front converts directly to
   sources, so nothing even looks wrong while the level change is undone).
   The assembled world holds exactly this proof pair for *authored* water —
   escape against the horizon (`DW0318`) and the ambient sea seeping into the
   built volume (`DW0851`), both over the one flood physics
   (`assembled::flood`); the runtime verbs have no twin of it.
3. **No surface can state a level at all.** `delvec schema --stage all` yields
   no field for a fluid level anywhere (re-measured at `be8eab02`: every
   `sea_level` / `water_level` / `flood_level` / `tide` hit is prose inside a
   description; no property name states a level). The flood level is
   `plan::SEA_LEVEL`, a constant. Per the ruling this is the missing primitive — and per the edge
   physics it must be a *plane*, which no composition of bounded region fills
   can honestly express (every bounded fill has the edge the ruling names).
4. **A stage-5 region cannot name the volumes any of this needs.** A runtime
   region is `StealthZone { anchor, extent }` — a box *centred* on a prefab
   anchor with unsigned half-extents. Probe 2 (`region-offset`): adding
   `"offset": [0,-1,0]` is `DW0100`, exit 1. The general region language —
   `box {min,max}` in a `piece-local`/`anchor-relative` frame, plus
   `union`/`intersect`/`subtract` — exists one stage away in stage 7's
   `select` and stage 5 cannot see it. This is CLAUDE.md's third defect shape,
   already recorded twice by spec-0031 §9; the answer is the existing
   language's binding widened, never a fourth mechanism.

## 2. The surfaces (one new `dsl_version`; `DW0141` below it at every site)

The version number — and every diagnostic number — is allocated when this
work is scheduled, not named here: a spec's prose is not a reservation, and a
number a machine never reads is a claim nothing defends. Every refusal below
is therefore described by what it refuses; the implementation consumes the
codes.

### 2.1 A region write knows what it leaves (landed)

Current behaviour, kept here as the plane's foundation rather than proposed:
a region write concludes from the block it writes — a fluid write is
impassable and never standable (`DW0544`), and a write fired from a root the
party can skip seals but never lays footing (`DW0546`) — one rule, in the one
place that owns region writes, inherited by `open-gate`, `close-gate`,
`fill-region`, `clear-region`, `open-way` and the plane below
(`docs/reference/compiler.md`, the `fill-region` row and the
`DW0544`/`DW0546` sections, is the record). The plane's transitions enter
through the same door: a `FloodEvent` is one more member of the family, never
a second physics.

### 2.2 The still-fluid proof — a declared body of fluid (four refusals)

Keyed to the object class the second ruling names: **a body of fluid**
is any volume the engine wets or drains — a runtime `fill-region` with a
fluid block, a runtime clear that exposes cells to water, a plane transition
(§2.3), and the piece-authored water the assembled proofs already hold still
(`DW0318`/`DW0851`). One reach computation (`assembled::flood`, generalized
to take its seed and level from the caller) serves all of them. Four
obligations:

- **Saturation.** Every wetted cell of a body is emitted as a
  source; a cell of the declared volume that the emission leaves neither
  source nor solid is refused, naming the cell. The invariant binds the
  emission (today's `/fill water` is saturating by construction — measured:
  a 12×12×3 basin filled in one command has **0 flowing cells**; the same
  basin given one corner source and 90 s of vanilla physics ends at **50
  flowing / 1 source of 432** and never becomes a body of water). Vanilla
  self-heals a *single interior* gap to a source in ≤1 s (measured), but a
  healed world no longer matches the emitted bytes — ADR-0006 is why the gap
  is refused rather than left to physics.
- **Containment on fill.** A runtime fluid fill whose wetted set has, at any
  causally-possible firing state, an open downstream neighbour outside the
  body (air at the same level or below) is refused, naming the escape cell —
  the runtime twin of `DW0318`. An effect may declare the flow **deliberate**
  (explicit field on the fill; the ruling keeps deliberate flow available),
  which waives exactly this refusal and nothing else.
- **Waterloggables in the volume.** Measured on 1.21.11:
  `fill … water replace air` around an embedded `oak_stairs` leaves it
  `waterlogged=true`; a later clear (`replace water`) does **not** touch it,
  and the stranded waterlogged block then leaks — 3 flowing cells within 8 s
  of the surrounding layer draining. So a drain emission must un-waterlog
  every waterloggable in the drained volume per-cell, or the block is a leak
  the model does not know about; until that emission exists, a waterloggable
  block inside a body's volume (or the plane's change band) is refused,
  naming block and cell.
- **Re-flood on clear.** A runtime clear any cell of which has an
  upstream water neighbour outside the cleared volume (ambient sea, authored
  water, another body) is refused, naming the inlet — the measured ≤40 s
  silent heal is the world this refusal keeps unshippable. This replaces the
  documented "optimistic there" limitation in `with_cleared` with a refusal.

Binding counts: each proof reports cells examined; zero is a finding
(CLAUDE.md vacuity rule).

### 2.3 The flood level is runtime state (`set-flood-level`)

The horizon already owns the ambient's physical facts, the flood level among
them (`plan::SEA_LEVEL` and the ambient band, spec-0013/0026). The primitive
is: **that fact becomes a declared set of values instead of one**.

- **Stage 1**: a `horizon` whose base declares a water table (today `ocean`;
  under spec-0026's bases, any base with one — the declaration is the
  ambient's, never one spelling's) may declare `flood_levels: [<y>…]` —
  the world-y values the ambient plane may stand at. The generator level
  (62) must be a member; it is the boot state. Order and count are content's
  (two entries is a tide; five is a filling cistern world; the engine knows
  only "a declared set of levels").
- **Stage 5**: effect `set-flood-level { level }`, an ordinary gated effect
  (the standard `requires_flags`/`forbids_flags`/`requires_state` gate rides
  it as the next consumer — the count is re-derived from the types by the
  existing `gate_consumers` test, never adjusted to fit). "In response to what the player has done" is
  therefore already expressible — triggers, objectives, dialogue, state — and
  needs nothing new.
- **Emission per transition** (ordered pair of levels, computed at build
  time): the compiler computes the exact cell delta against the assembled
  world — rising, the reach at the target level minus cells already wet;
  falling, the current wetted set minus the reach at the target — and emits
  saturating, box-decomposed `fill` commands (≤ 32768 blocks each, the
  measured vanilla ceiling; refusal text
  `Too many blocks in the specified area (maximum 32768, …)`;
  `max_block_modifications` can raise it but a single 262 144-block command
  costs a 591 ms main-thread stall, so chunking is the default and any
  per-tick budget is stated in ticks of a `schedule` chain, deterministic).
  Because reach is computed, a sealed vault below the new level stays dry
  and an open sea-cave floods to its ceiling — the plane is *physics-shaped*,
  not a slab.
- **The extent, and why the edge never flows.** The interior of a plane fill
  is still by construction (measured: 480×480×2 layers, **0 flowing cells**,
  interior and 16-cell rim, after 15 s under fully-ticking chunks). The only
  edge is the rim where the raised plane meets the untouched generator sea,
  and the measured physics dictate the rule: a rim in a *ticking* chunk
  leaks a frozen 1-block fringe into its non-ticking neighbours (3/192
  transect samples wet beyond the rim), and a rim chunk that *later* ticks
  spills a 1–3-block fringe (21/42 samples wet after a deliberate reload).
  So the emitted extent must end **beyond every chunk that can ever tick or
  be seen**: extent ≥ the player-reachable set (the spec-0013 `boundary`,
  which the compiler already derives) dilated by max(view-distance,
  simulation-distance) plus a one-chunk margin. Both distances therefore
  become **pinned facts in the emitted `server.properties`** — today neither
  is emitted at all, so the operator's defaults decide what a player can see,
  and no extent proof can bind. That pin is part of this spec.
- **Loading.** `/fill` into an unloaded chunk refuses
  (`That position is not loaded`, measured), so a transition brackets its
  fills in `forceload add` / `forceload remove` rings. Measured end to end
  (`transition_window`): load → fill 256×256 → unload in 287 ms; 20 s later
  the rim reloads with **16/16 sources, 0 flowing** — pending fluid ticks
  freeze with the chunk, exactly the behaviour the extent rule needs.
- **Completability.** A transition is a `FloodEvent { level, fire_step }` in
  the same causal machinery as `RegionEvent` (`region_state_at`,
  latest-causal-write-wins — already non-monotone, so tide-in/tide-out/tide-in
  along the DAG needs nothing new). The leg world's `flooded` set is the
  wetted set of the level in force at that DAG point, so **every route proof
  that reads the world binds per level with no new proof code**: `DW0311`
  (critical path dry under the level in force), checkpoint no-stranding —
  with the respawn-seat intersection ranging over every level possible while
  the checkpoint is live — NPC posts, walk drivers, the ambient-seepage band
  (`DW0851`'s `floor_top < y ≤ level`, today a constant, becomes the level
  in force), and boundary safety's stranding model. A rising level only ever
  *closes* modelled ROUTES (water is impassable, §2.1); what it opens is
  carriage, and carriage is §2.4's proof, never a route.
- **A transition fires only from a forced root.** A plane is world state
  every proof keys on, and no guess about an uncertain firing is
  conservative for it: assuming a raise happened closes routes AND invents
  carriage; assuming a lower happened opens routes AND hides stranding. A
  single fill has a pointwise-worst (§2.1's skippable-root split); a level
  does not, because its two futures disagree about opposite halves of the
  proof. So a transition fired from a root the party can skip — a trap
  payload, a shortcut's far side, a shop offer, a death bundle — is refused,
  naming the root; the campaign shape that trips it is a tide pulled by an
  optional lever. The same rule keeps the emitted delta honest: each
  transition's fills are computed for an ordered pair of levels, and an
  unforced firing makes the current level unknowable at build time, so the
  emission would be wrong for one of the orders. Likewise a transition whose
  computed reach depends on a runtime region write whose state is ambiguous
  at the transition's span is refused — the delta must be a function of
  build-time-knowable state. Where a campaign one day needs an optional
  tide, the lift is a proof over every level possible at that span (the
  union of wetted sets blocks; only everywhere-dry cells stand) — priced
  then, not guessed now.
- **Players in the volume** (measured, `player_in_rising_column` /
  `solid_fill_entombment`): a player submerged by a rising plane is not
  displaced (Pos unchanged to the decimal), takes no placement damage, and
  drowns on the ordinary vanilla clock (air 300→0 in ~15 s, then ~1.25 hp/s
  at 2-s sampling)
  — a rising tide is a fair, readable hazard with vanilla's own telegraphing,
  and needs no engine mitigation. A **solid** runtime fill over a player
  suffocates at ~2 hp/s; that is `fill-region`'s existing semantics, now
  stated in its reference row rather than discovered.
- **Scale cost** (measured): one full 512×512 layer = 8 commands, 262 144
  blocks, 0.7–1.6 s wall including instrument overhead; MSPT 2.5 → 12.1 worst
  during a two-layer raise over 1024 fully-ticking chunks, settling to ~7.
  A site-scale transition is a seconds-long dramatic event, not a freeze.

### 2.4 Water is a carrier — the delivery proof

The route proofs answer "can the party WALK it", per level. A moving plane
also MOVES the party: rising water floats a body through the swim reach of
its connected volume, and falling water sets a swimming body down on
whatever lies beneath it. Both are structural reachability changes the walk
model cannot see — probe 1 measures the same blindness in miniature, where a
body reaches water no walk proof examined and the build is green.

So the water owes one more proof, and it is a **stranding proof, not a route
proof** — keyed to the object class, so it binds any campaign with water and
a relocator, plane or no plane. A water state `H` is the flooded set in force
at a causal point (the plane's level joined with every forced fill — the
same set the route proofs read). Delivery cells, per state `H`: every
standable cell a body can exit the water onto (the climb-outs of the wetted
set at `H`); and, for each state `L` the world can subsequently occupy, the
landing cell of every column wet at `H` (still swimming where wet at `L`,
otherwise the fall-arrest beneath). Entry is every way a body gets into the
water that no walk proves: stepping in from the reachable walk region (the
entry today's stranding proof already owns), a `teleport` whose destination
is wet at the state in force, and the rise itself wetting a cell a body
stands on.
**The obligation: from every delivery cell, at every state then possible, a
route back into the proven walk region exists.** A delivery cell with none
is refused, naming the cell, the carrying state and the stranding state.

Two campaign shapes trip it, one per direction. The ledge only the flood can
reach: at high water a swimmer climbs onto a perch above the walk region;
the plane falls; the perch has no way down that is not the drop the boundary
proof already refuses. And the drained basin: the party legitimately swims a
flooded ward — entry from the walk region, climb-out green at high water —
the plane falls mid-crossing, and the ward floor they land on has a rim six
blocks overhead that was only ever a climb-out from the waterline.

This is deliberately NOT a swim-traversal model: no route may ever REQUIRE
the water (water stays impassable in every route proof — §5). The carrier
proof only asks that wherever the water can put a body, the body can come
back. The two conservative directions therefore compose instead of trading
off: routes never credit water, and stranding always charges it.

### 2.5 Whose cell is it — authored water and the plane

Prefabs already author water: a shore tops its waterline (spec-0048), and a
piece can bake a whole sea — one placed zone measured 17,760 fluid cells in
8,255 columns lying outside every placed piece's AABB (`DW0318`'s census).
A world plane that moves and a piece that authors water must not fight, and
the rule that prevents it is decided by geometry, never by an author:

- **Connectivity decides ownership.** An authored fluid cell the ambient
  reach touches at any declared level is the PLANE's: it fills and drains
  with the transitions, and every delta counts it. An authored body sealed
  from the ambient at every declared level is the PIECE's — a cistern, a
  fountain, a walled moat — no transition touches it, and it answers the
  still-fluid proofs (§2.2) on its own edges. There is no field on which an
  author can assert ownership, deliberately: "this pond is mine" claimed
  over a cell the sea reaches is exactly what a leaking pond would also
  claim. The opt-out's proof obligation is sealedness, which a leak cannot
  supply.
- **At boot, the census must agree.** The plane boots at the generator
  level, and every plane-owned authored cell must lie within the boot
  level's wetted set. A piece that bakes ambient-connected water ABOVE the
  boot waterline has authored a sea the boot world cannot hold up — refused,
  naming the piece and the cells. The prescription is the half that deletes
  work rather than adding it: a drowned state needs no baked sea at all,
  because the reach computation wets every open volume when the plane rises
  — author the geometry, let the plane bring the water. The piece-side datum
  is untouched: `waterline_y` and its placement check (`DW0344`, spec-0048)
  bind against the generator level exactly as today — the boot level IS that
  level, which is why it must be a member.

### 2.6 One region language (the binding fix)

Stage 5 gains `regions[]`: `{ id, area, shape }` where `shape` is exactly the
stage-7 `RegionShape` (`box` in a `piece-local`/`anchor-relative` frame,
`union`/`intersect`/`subtract`, `surface-band`, `palette-match`) — the same
types, not a copy. Every stage-5 region consumer (`fill-region`,
`clear-region`, `teleport.from`, `give-effect.in`, `damage-players.in`,
`begin-stealth.zones`, `lethal_volumes[].region`) accepts `region/<id>`
alongside the inline anchor-centred box, which stays byte-identical. This is
the recorded spec-0031 §9 finding closed at the object class: a bounded body
of fluid (a cistern, a moat, a lock chamber — the "special case that must
justify its own edges", which it does by passing §2.2's containment and
re-flood refusals), a lift's
deck and shaft, and a lethal shaft-bottom all become nameable, and the
emission decomposes a non-box cell set into deterministic box fills (the same
decomposition the plane delta needs — one implementation).

## 3. The worked example, which is deliberately NOT a verb

The citadel: `flood_levels: [62, 65]`. A stage-5 trigger the party fires
(gated on their progress) runs `set-flood-level {level: 65}` — the causeway
ward drowns (its walk cells now in the wetted set: any route through it is
closed for every proof from that DAG point), the sea-cave gate floods shut,
and the high walk along the wall — dry geometry the whole time — is the way
onward. A later beat runs `set-flood-level {level: 62}` and the shore route
reopens. A slow flood is a `sequence` stepping through intermediate declared
levels. No verb names a tide; the campaign's fiction calls it one.

## 4. Probes and measurements (all committed, all re-runnable)

| # | what | where | result |
|---|---|---|---|
| P1 | water as a standable deck | `spec-0038-probes/water-deck` (lift fixture, 2 fill blocks → `minecraft:water`) | at `be8eab02`: **exit 0**, still emitting `fill 4 65 3 6 65 5 minecraft:water` mid-air. The fluid-not-floor half (§2.1) is landed; what keeps it green is that the deck is reached by `teleport`, which nav models nowhere — the §2.4 red |
| P2 | a region one cell below its anchor | `spec-0038-probes/region-offset` | at `be8eab02`: `DW0100`, exit 1 — the §2.6 red |
| P3 | any level surface at all | `delvec schema --stage all` grep | at `be8eab02`: no field; every level-word hit is prose in a description |
| M1 | fill ceiling + gamerule | spike `fill_ceiling` | 32768; refusal text verbatim; 262 144 blocks in one command = 591 ms stall |
| M2 | plane-raise stillness + cost | `plane_raise_512` | interior + rim **0 flowing**; 8 cmds/layer; MSPT 2.5→12.1→6.9 |
| M3 | ticking-edge lower | `plane_lower_ticking_edge` | fully healed ≤40 s, **0 flowing block-states while healing** |
| M4 | frozen-edge behaviour | `plane_raise_512.spill…`, `transition_window` | 1-block frozen fringe past a ticking rim; unloaded rim reloads as 16/16 sources, 0 flowing |
| M5 | unloaded fill | `transition_window` | `That position is not loaded` — hence forceload brackets |
| M6 | saturation vs settle vs gap | `basin_*` | 0 flowing saturated; 50 flowing / 1 source of 432 settled; single gap self-heals ≤1 s (still refused — ADR-0006) |
| M7 | waterloggables | `waterloggables` | fill waterlogs embedded stairs; clear strands them; stranded block leaks 3 flowing cells |
| M8 | players | `player_in_rising_column`, `solid_fill_entombment` | no displacement; vanilla drowning clock; solid fill suffocates ~2 hp/s |

Unmeasured, stated rather than assumed: the visual appearance of the frozen
fringe at render distance (the extent rule keeps it past view distance
instead); multi-cell interior gaps' heal behaviour; kelp/seagrass and other
in-water flora under a moving level; item entities and boats on a changing
surface; the mechanism by which 1.21.11's `fill … replace air` waterlogs an
embedded waterloggable (the fact is measured, its cause is not).

## 5. Refused, with disproofs

- **A `tide` / `sea-level` / any water-named verb.** Fails the generality
  test on its name alone. The surface is the horizon's flood level (an
  existing engine fact made writable) plus declared bodies; "tide" is a
  campaign's fiction.
- **A region-scoped water level** ("raise water to y=N inside R"). The rule:
  a level moves as a plane, and every bounded region has the edge
  where the flow appears. The bounded case already has its surface — a §2.6
  region filled/cleared under the §2.2 proofs, which make it justify its own
  edges — so a second, level-shaped spelling of it would be the
  fourth-mechanism defect (strictly weaker than the mechanism it duplicates).
- **Flow-and-settle emission.** Measured never to terminate in a body of
  water (M6: 50 flowing / 1 source after 90 s), nondeterministic (ADR-0006),
  and the exact moving water the ruling forbids.
- **Wading / swim traversal modelling.** Water is impassable, never
  standable, in every route proof. Consequence stated plainly: a risen level
  only ever closes modelled routes; anything the fiction wants passable at
  high water must be dry geometry. A swim model is a third collision class
  through every proof in `nav` for which no campaign has yet named a need —
  and §2.4's carrier proof is not one: it asks only whether a delivered body
  can come BACK, and never lets a route require the water.
- **An ownership declaration on a fluid cell** ("this water is the piece's —
  leave it dry / leave it full"). §2.5's rule: connectivity decides, and the
  only opt-out is sealedness, which is provable on the bytes and which the
  defect — a leak into or out of the plane — cannot supply. An assertable
  field would let a leaking pond claim exactly what a sealed one claims.
- **A clocked plane driver** (a tide on a timer). The ruling is event-driven
  ("in response to what the player has done"). The engine's one clocked
  world-change mechanism (`timed_gates`' ping-pong) is gate-keyed; a
  generalization with zero declared consumers would be an unbound green
  (vacuity rule), so it waits for the campaign that asks.
- **A second fluid (lava plane).** The level belongs to the horizon's
  declared ambient, which is water for every base that has one; no horizon
  declares lava, so a lava path would bind zero campaigns. Nothing in the
  §2.2/§2.3 shape hardcodes water; the day a horizon declares another fluid,
  the surfaces are already keyed to "fluid".
- **Boats / vehicles**; **a rise-animation verb** (a `sequence` through
  intermediate declared levels — §3, the lift precedent: worked example, not
  surface).

## 6. Acceptance criteria

1. **P1 inverts, and the half that stayed green is the half that reds.** At
   `be8eab02` the fluid classification is landed and
   `spec-0038-probes/water-deck` still builds exit 0, because its deck is
   reached by teleport. Under §2.4 it fails to build: the destination is wet
   at the level in force, the deck's cells deliver a body no route returns,
   and the refusal names them. Restoring the slab block builds green,
   byte-identical to today's lift output.
2. **Byte identity.** A campaign declaring no fluid fill, no `regions[]`, and
   no `flood_levels` builds byte-identical to the pre-0038 engine across all
   fixtures and both released campaigns, engine-diff style (the spec-0031
   measurement discipline: counted files, named deltas, zero expected).
3. **Saturation is proven, not asserted**: a from-the-emission test asserts
   every emitted body/transition writes sources over its entire wetted set
   with no interior gap; a generated PackTest fires each campaign transition
   on the pinned toolserver and asserts **zero flowing water block-states**
   in the wetted volume (fill-replace census, the spike's instrument), with
   its binding count stated.
4. **The delta equals the model, cell for cell**: for each transition, the
   emitted fill set == the model's wetted-set delta; rebuild byte-identity
   twice (ADR-0006).
5. **Refusals red where they must**: a fixture per refusal — the four §2.2
   obligations (saturation gap, uncontained fill, waterloggable in the
   volume, re-flooding clear), the unforced-root transition (§2.3), the
   carrier stranding pair (§2.4: the flood-only ledge and the drained
   basin), and the boot-census disagreement (§2.5) — each failing under the
   code the implementation allocates and naming the offending cell, block,
   inlet, root or piece; the deliberate-flow declaration waives exactly the
   containment refusal and nothing else; every proof states cells examined
   and a zero binding is itself a red.
6. **The extent binds**: `view-distance` and `simulation-distance` appear in
   emitted `server.properties`; a test derives the minimum extent from the
   campaign `boundary` + max(view, simulation) + 1 chunk and asserts every
   transition's fill extent covers it; every fill ≤ 32768 blocks.
7. **The gate reaches the new verb from the types**: `set-flood-level`
   appears in the `gate_consumers` enumeration without that test changing its
   derivation.
8. **The worked example exists as a fixture** (`flood_levels: [62, 65]`, one
   route closed and one opened across a transition), proven by the
   completability model choosing different legs per level — and the first
   campaign to declare `flood_levels` carries a bot-tier live proof of its
   transition (level changed, zero flowing census, route state toggled), the
   spec-0031 `on_death` precedent: machine-green admits it to the batch, the
   owner's playtest merges it.
9. **Reference updated in the same PR**: `docs/reference/compiler.md` rows
   for the new verb, the carrier proof, the ownership rule, and every code
   the implementation allocates; `check-dw-codes` coverage for each.
10. **Ownership is connectivity, cell for cell**: a fixture placing one
    sealed basin and one sea-connected pool under a two-entry `flood_levels`
    proves, across a lowering transition, that the pool's cells leave the
    wetted set and the basin's do not — asserted on the emitted delta, not
    on prose; and the same fixture with one basin wall removed flips the
    basin to plane-owned without any declaration changing.

## 7. Corrections rounds have made to their briefs and to this spec

- The dispatch named "fill/clear + state + gates" as the likely mechanism
  set; the plane ruling is what disqualified bounded fills as the
  primary surface, and measurement (M3) then showed a bounded lower is not
  merely ugly but **silently undone in 40 s** — the fill-shaped mechanism was
  wrong for the instance, not just inelegant.
- The brief's fear that "changing large volumes at runtime" might be
  prohibitively expensive did not survive measurement: the whole-site cost is
  seconds (M2), and the real constraints are elsewhere — the edge (M4), the
  loading bracket (M5), and waterloggables (M7).
- Re-measured at engine revision `be8eab02` (the amendment round): the §2.1
  model fix is landed behaviour (`DW0544`/`DW0546`), and probe 1 still
  builds green through its teleport — which moved the missing half from "a
  fluid is not floor" to "a delivered body has no proof" (§2.4). The
  compile-time `flood` this spec's first draft cited as its shared machinery
  (spec-0030, with its codes) is not landed; the citations now name the
  assembled-world flood that is (`assembled::flood`, `DW0318`/`DW0851`). The
  draft also named diagnostic codes for its own refusals — a number
  allocated by prose is not free, so every refusal is now described by what
  it refuses and the implementation consumes the numbers.
- The same round added what a campaign's measured need showed the draft had
  not decided: the carrier proof (§2.4 — reachability moves in BOTH
  directions, and the draft's "a rising level only closes routes" was true
  of routes and silent about stranding), the forced-root rule for
  transitions (§2.3 — no guess about an uncertain level is conservative),
  and cell ownership between an authored sea and the plane (§2.5 — decided
  by connectivity, with the boot census as the agreement proof).
