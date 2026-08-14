# spec-0038: Standing fluid — declared bodies, and the flood level as runtime state

- **Status**: Proposed
- **Basis**: owner ruling 2026-08-13 — the engine supports a dynamic world; the
  commissioning instance is a citadel on a tidal rock whose sea level answers
  what the party has done, opening some routes and closing others. Two further
  owner rulings, same day, shape every surface here: **flowing liquid is
  avoided unless deliberately designed** — a water level moves as a whole
  plane, so no source ever has a lower neighbour to flow into; and **a body of
  water is saturated by construction** — every cell of its volume is a source,
  and an emission that places some water and lets vanilla spread it is
  excluded outright (it is also nondeterministic, ADR-0006).
- **Specs**: 0030 (compile-time `flood` — the static half of the same physics;
  its reach computation is the shared machinery), 0031 (runtime state, region
  fill/clear, the gate; its §9 findings are two of this spec's reds), 0013/0026
  (the horizon owns the ambient's physical facts, `flood_level` among them)
- **ADRs**: 0003 (vanilla-first: the primitive is vanilla's own fluid), 0006
  (determinism: the waterline is a function of the DSL, never of fluid ticks)
- **Evidence**: every claim below is either demonstrated red against the
  current engine (`docs/specs/spec-0038-probes/`, commands inline) or measured
  on the pinned 1.21.11 (`tools/spike-fluid-plane/`, raw data in
  `observations.json`, re-runnable via `EULA=TRUE tools/spike-fluid-plane/run.sh`).
- **Non-goals**: §5. Deliberately refused, each with its disproof: a
  tide/water-named verb, a region-scoped water level, flow-and-settle
  emission, wading/swim traversal, a clocked plane driver, a second fluid,
  vehicles, a rise animation verb.

## 1. What the existing mechanism fails to reach, and why

The general mechanisms adjacent to "the sea level changes" are: `fill-region`
/ `clear-region` (a declared region, written at runtime, one completability
rule `plan::RegionEvent` → `nav::region_state_at`), numeric runtime state in
the shared gate, `teleport`, `lethal_volumes[]`, and spec-0030's compile-time
`flood`. Measured against the ruling, they fail in four distinct places:

1. **The model does not know a fluid from a floor.** A runtime fill's write is
   `RegionWrite::Fill` — cells become `solid`, i.e. *standable ground* — with
   no record of what block was written. Probe 1 (§4, `water-deck`): the lift
   fixture with both car-floor fills changed to `minecraft:water` **builds
   green**, and the emitted pack teleports the party onto a 3×3 water "deck"
   hanging in mid-air (`fill 4 65 3 6 65 5 minecraft:water` in
   `seq_*.mcfunction`). The proof asserts the party stands on water; live,
   they fall through it while it cascades off the edge. The correct model
   exists eleven lines away — `World::flooded`, task #45: water is impassable
   and **never standable** — and the write cannot reach it.
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
   spec-0030 proves exactly these two claims for *compile-time* water
   (`DW0394`/`DW0395`); the runtime verbs have no twin of that proof.
3. **No surface can state a level at all.** `delvec schema --stage all` yields
   no field for a fluid level anywhere (grep for
   `sea_level|water_level|flood_level|ambient|tide|plane`: four hits, all
   prose in descriptions). The flood level is `horizon::SEA_LEVEL`, a
   constant. Per the ruling this is the missing primitive — and per the edge
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

## 2. The surfaces (DSL v0.11.0; `DW0141` below it at every site)

### 2.1 A region write knows what it leaves (model fix, every version)

`RegionEvent` carries the write's occupancy class, derived from the block id:
a **fluid** write enters the leg world's `flooded` set (impassable, never
standable — the task #45 rule, and spec-0030's deliberate non-goal upheld: no
wading model, the conservative direction); a solid write stays `solid`; air
stays `Clear`. One rule, in the one place that already owns region writes
(`nav::World::with_region_state`) — `open-gate`, `close-gate`, `fill-region`,
`clear-region` and the plane below all inherit it.

This is a soundness fix to an existing proof, not new surface: it applies at
**every** `dsl_version` (a proof that lets a campaign stand its critical path
on water is wrong at 0.10.0 too), and emission is byte-identical everywhere.

### 2.2 The still-fluid proof — a declared body of fluid (`DW0550`–`DW0553`)

Keyed to the object class the second owner ruling names: **a body of fluid**
is any volume the engine wets or drains — a runtime `fill-region` with a
fluid block, a runtime clear that exposes cells to water, a plane transition
(§2.3), and (already proven, by `DW0394`/`DW0395`) a stage-7 `flood`. One
reach computation (spec-0030's `flood_reach`, generalized to take its seed
and level from the caller) serves all of them. Four obligations:

- **`DW0550` — saturation.** Every wetted cell of a body is emitted as a
  source; a cell of the declared volume that the emission leaves neither
  source nor solid is refused, naming the cell. The invariant binds the
  emission (today's `/fill water` is saturating by construction — measured:
  a 12×12×3 basin filled in one command has **0 flowing cells**; the same
  basin given one corner source and 90 s of vanilla physics ends at **50
  flowing / 1 source of 432** and never becomes a body of water). Vanilla
  self-heals a *single interior* gap to a source in ≤1 s (measured), but a
  healed world no longer matches the emitted bytes — ADR-0006 is why the gap
  is refused rather than left to physics.
- **`DW0551` — containment on fill.** A runtime fluid fill whose wetted set
  has, at any causally-possible firing state, an open downstream neighbour
  outside the body (air at the same level or below, spec-0030's
  `Tide::downstream`) is refused, naming the escape cell — the runtime twin
  of `DW0395`. An effect may declare the flow **deliberate** (explicit field
  on the fill; the ruling keeps deliberate flow available), which waives
  exactly this code and nothing else.
- **`DW0552` — waterloggables in the volume.** Measured on 1.21.11:
  `fill … water replace air` around an embedded `oak_stairs` leaves it
  `waterlogged=true`; a later clear (`replace water`) does **not** touch it,
  and the stranded waterlogged block then leaks — 3 flowing cells within 8 s
  of the surrounding layer draining. So a drain emission must un-waterlog
  every waterloggable in the drained volume per-cell, or the block is a leak
  the model does not know about; until that emission exists, a waterloggable
  block inside a body's volume (or the plane's change band) is refused,
  naming block and cell.
- **`DW0553` — re-flood on clear.** A runtime clear any cell of which has an
  upstream water neighbour outside the cleared volume (ambient sea, authored
  water, another body) is refused, naming the inlet — the measured ≤40 s
  silent heal is the world this code keeps unshippable. This replaces the
  documented "optimistic there" limitation in `with_cleared` with a refusal.

Binding counts: each proof reports cells examined; zero is a finding
(CLAUDE.md vacuity rule).

### 2.3 The flood level is runtime state (`set-flood-level`)

The horizon already owns the ambient's physical facts, `flood_level` among
them (`crate::horizon`, spec-0026). The primitive is: **that fact becomes a
declared set of values instead of one**.

- **Stage 1**: an ocean-based `horizon` may declare `flood_levels: [<y>…]` —
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
  wetted set of the level in force at that DAG point, so **every proof that
  reads the world binds per level with no new proof code**: `DW0311`
  (critical path dry under the level in force — the generalization of
  `DW0364`'s static dryness), checkpoint no-stranding, respawn-seat
  intersection (already an intersection over configurations), NPC posts,
  boundary safety's stranding model. A rising level only ever *closes*
  modelled routes (water is impassable, §2.1), so the proof direction is
  conservative by construction. A transition whose computed reach depends on
  a runtime region write whose state is ambiguous at the transition's span
  (the non-forced-trigger case) is refused (`DW0554`) — the delta must be a
  function of build-time-knowable state, or the emitted fills are wrong for
  one of the orders.
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

### 2.4 One region language (the binding fix)

Stage 5 gains `regions[]`: `{ id, area, shape }` where `shape` is exactly the
stage-7 `RegionShape` (`box` in a `piece-local`/`anchor-relative` frame,
`union`/`intersect`/`subtract`, `surface-band`, `palette-match`) — the same
types, not a copy. Every stage-5 region consumer (`fill-region`,
`clear-region`, `teleport.from`, `give-effect.in`, `damage-players.in`,
`begin-stealth.zones`, `lethal_volumes[].region`) accepts `region/<id>`
alongside the inline anchor-centred box, which stays byte-identical. This is
the recorded spec-0031 §9 finding closed at the object class: a bounded body
of fluid (a cistern, a moat, a lock chamber — the "special case that must
justify its own edges", which it does by passing `DW0551`/`DW0553`), a lift's
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
| P1 | water as a standable deck | `spec-0038-probes/water-deck` (lift fixture, 2 fill blocks → `minecraft:water`) | `delvec build` **exit 0**, emits `fill 4 65 3 6 65 5 minecraft:water` mid-air — the §2.1 red |
| P2 | a region one cell below its anchor | `spec-0038-probes/region-offset` | `DW0100`, exit 1 — the §2.4 red |
| P3 | any level surface at all | `delvec schema --stage all` grep | no field; 4 hits, all prose |
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
- **A region-scoped water level** ("raise water to y=N inside R"). The owner
  ruling: a level moves as a plane, and every bounded region has the edge
  where the flow appears. The bounded case already has its surface — a §2.4
  region filled/cleared under the §2.2 proofs, which make it justify its own
  edges — so a second, level-shaped spelling of it would be the
  fourth-mechanism defect (strictly weaker than the mechanism it duplicates).
- **Flow-and-settle emission.** Measured never to terminate in a body of
  water (M6: 50 flowing / 1 source after 90 s), nondeterministic (ADR-0006),
  and the exact moving water the ruling forbids.
- **Wading / swim traversal modelling.** spec-0030's non-goal, upheld at
  runtime: water is impassable, never standable. Consequence stated plainly:
  a risen level only ever closes modelled routes; anything the fiction wants
  passable at high water must be dry geometry. A swim model is a third
  collision class through every proof in `nav` for which no campaign has yet
  named a need.
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

1. **P1 inverts.** `spec-0038-probes/water-deck` fails to build (the fluid
   deck is not standable, so the teleport-onto-it and the finale leg red);
   restoring the slab block builds green, byte-identical to today's lift
   output.
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
5. **Refusals red where they must**: fixtures for `DW0550`–`DW0554` each
   fail naming the offending cell/block/inlet, and the deliberate-flow
   declaration waives exactly `DW0551`; every proof states cells examined and
   a zero binding is itself a red.
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
   for the two verbs, the model change, the new codes; `check-dw-codes`
   coverage for every code above.

## 7. Corrections this round made to its own brief

- The dispatch named "fill/clear + state + gates" as the likely mechanism
  set; the owner's plane ruling is what disqualified bounded fills as the
  primary surface, and measurement (M3) then showed a bounded lower is not
  merely ugly but **silently undone in 40 s** — the fill-shaped mechanism was
  wrong for the instance, not just inelegant.
- The brief's fear that "changing large volumes at runtime" might be
  prohibitively expensive did not survive measurement: the whole-site cost is
  seconds (M2), and the real constraints are elsewhere — the edge (M4), the
  loading bracket (M5), and waterloggables (M7).
