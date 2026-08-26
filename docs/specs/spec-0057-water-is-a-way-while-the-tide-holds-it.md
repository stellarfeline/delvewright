# spec-0057: Water is a way while the tide holds it — the swim credit, bound to the level in force

- **Status**: Proposed
- **Basis**: the commissioning campaign's design requires the tide to open ways
  in both directions: rising water carries a body to heights it cannot
  otherwise reach, falling water opens the low places it was hiding, and the
  level moves because the story reached a stage, never because a clock ticked.
  Today every route proof answers that such a way is not a route. spec-0038 §5
  refused swim traversal on the stated premise that *no campaign has yet named
  a need*; that premise has expired, and this spec supersedes exactly the
  clauses that rest on it (§1). The survey of what refuses this and which
  general mechanisms nearly reach it is `docs/notes/water-as-a-way-scoping.md`
  (code citations at engine revision
  `d3e7709ff927269b11a45fa471b6b5075a99b74d`; the load-bearing ones re-verified
  here at the same revision).
- **Specs**: 0038 (three clauses superseded, everything else stands and most
  of it is prerequisite — §1), 0056 (the block-shape authority, untouched —
  §3), 0034 (`DW0455`, whose message this spec falsifies as written — §4.4),
  0013/0026 (the horizon owns the flood level), 0051 (an adversary-direction
  consumer: optionality is a negative claim).
- **ADRs**: 0003 (vanilla-first: the swim is vanilla's own physics), 0006
  (determinism: verdicts are functions of the DSL, never of fluid ticks).
- **Numbers**: this spec consumes none. The `dsl_version`, every diagnostic
  code, and the harness allowlist entry (`harness/src/critical-path.ts`, the
  version ledger's second consumer) are allocated by the planner at
  implementation; every refusal below is described by what it refuses.
- **Non-goals**: §6.

## 1. What falls, and what stands

spec-0038 is superseded in exactly **three clauses** — never edited; its file
stands as the historical record and this section is the authority on which of
its rulings still bind:

1. **§2.3's closing rule** — "A rising level only ever *closes* modelled
   ROUTES … what it opens is carriage, and carriage is §2.4's proof, never a
   route." A rising level now also opens routes: swim legs wet at the level in
   force.
2. **§2.4's scope clause** — "This is deliberately NOT a swim-traversal model:
   no route may ever REQUIRE the water." A route may now require the water,
   under the certain relation (§4.1). The same section's closing claim — "the
   two conservative directions compose … routes never credit water, and
   stranding always charges it" — falls in its first half only; the second
   half survives as this spec's AC3, and §2.4's carrier proof itself is
   promoted from companion to load-bearing prerequisite (AC1).
3. **§5's "Wading / swim traversal modelling" refusal**, whose disproof rests
   on the expired premise.

**Standing, and prerequisite** — the supersession touches none of this:

- **§2.1** (landed as `DW0544`/`DW0546`): a fluid write is impassable and
  never lays footing. The credit is an *edge class*, not footing; these stand.
- **§2.2** still-fluid proofs (saturation, containment, waterloggables,
  re-flood): precondition, not an extra — a swim credit through water vanilla
  may drain or that is still moving is a proof about a world that stops
  existing.
- **§2.3's entire surface** — `flood_levels`, `set-flood-level`, `FloodEvent`,
  the forced-root rule, extent, loading, emission: **adopted unchanged** (§2).
- **§2.4's carrier proof**: stranding charged at every subsequently possible
  state, both tide directions (AC1, AC3).
- **§2.5** ownership-by-connectivity, **§2.6** `regions[]`, every measurement
  (M1–M8), every §6 criterion, and every *other* §5 refusal — the tide verb,
  the region-scoped level, flow-and-settle, the clocked driver, the second
  fluid, vehicles, the ownership field — stand as written.

## 2. The tide as world state: spec-0038 §2.3, adopted unchanged

Established at the pinned revision: **no `FloodEvent`, `set-flood-level` or
`flood_levels` symbol exists anywhere in `crates/`** (zero hits), and the
flood level is the constant `plan::SEA_LEVEL = 62`
(`crates/compiler/src/plan.rs:48`). The level this spec credits cannot even be
stated yet.

Decision: **adopt §2.3 unchanged.** The argument: (a) it is an Accepted design
that already places the tide in the exact causal machinery the credit needs —
`FloodEvent` beside `RegionEvent` in `region_state_at`, with the forced-root
rule making the level in force build-time-knowable; (b) re-deciding it here
would stand two authorities over one rule; (c) nothing here asks anything of
the plane that §2.3 does not already provide — the credit reads the wetted set
of the level in force, nothing more. The ordering is therefore structural, not
prose: a swim credit's admission rule takes its level from a surface with zero
symbols today, so **no credit can be implemented before §2.3 lands**.

## 3. Where the "only at stage X" quantifier lives — and where the rule may not go

The quantifier over *when* is already a landed general mechanism:
`World::region_state_at` (`crates/compiler/src/nav.rs:3746`) computes the
world in force at a quest-DAG point, latest-causal-write-wins, already
non-monotone, and route proofs already bind per causal state per leg. It can
host this rule because a swim edge is a fact about the leg world — admissible
iff its cells are wet, with water, at the level in force at that DAG point —
and per-leg worlds are exactly what this function produces.
Tide-in/tide-out/tide-in closes and reopens the way with no new proof shape;
the brief obligation "creditable at high water must not silently survive to
low water" falls out of the binding rather than needing its own check (AC2
demonstrates it anyway, because "falls out of" is a claim an implementation
can get wrong).

Where the rule may **not** go, and why those sites stay byte-identical:

- **The grammar/schem piece walk** (`Voxels` and everything the contract gates
  prove): static piece geometry, a monotone fixpoint with no notion of causal
  state. The tide rule is entirely about *when*, so the site cannot express
  its quantifier — hosting it there is wrong, not merely risky. The
  piece-level refusal is also correct on its own terms: a piece cannot know
  the world's level.
- **`Collision::Fluid`** (spec-0056) stays no/no to both questions: swim is a
  distinct edge class, never a passability reclassification. Flipping
  `passes_body` would hand water-as-air to every proof at once — the unsound
  direction nothing downstream catches.

## 4. The two relations, fluid identity, and the surface

### 4.1 Two calibrations of one geometry

Credit and refusal cannot use one relation: a generous swim relation is sound
for refusing a negative claim and unsound for crediting a route. The tree
already holds this pattern with its directions documented —
`reachable_with_fall` (`crates/schem/src/nav.rs`): "Deliberately *more*
permissive than `connected`, which is why the two are used in opposite
directions and never interchangeably … The negative direction … is asserted
under the plain walk instead, because … proving a negative under the generous
model would be circular." Swim joins that discipline as a pair:

- **The certain relation** (under-approximation) — **route credit only.**
  Entry from a proven standable cell by the boundary proof's entering rule;
  3D reach through the wetted set of the level in force; exit only through the
  climb-out band — feet at the level or level + 1 onto a proven standable
  cell, the conservative rules already written for the stranding proof at
  `crates/compiler/src/nav.rs:7066-7086`, rebound; every fully-submerged
  segment bounded by the measured air clock (spec-0038 M8: air 300→0 in
  ~15 s; a submerged body is not auto-lifted, so the route genuinely requires
  swimming input). Swim speeds are unmeasured; they are a measurement
  obligation on the spike template (`tools/spike-fluid-plane/`), never an
  invented constant — until measured, the certain relation credits nothing
  whose air cost it cannot bound.
- **The generous relation** (over-approximation) — **adversary only**, for any
  claim that something is *not* reachable: uniqueness, optionality
  (spec-0051), containment. Surface connectivity may be over-counted and the
  climb-out band widened, because under the adversary an error can only
  refuse. This half is a soundness improvement independent of the tide: a
  player can physically swim in vanilla today, so a negative claim over
  water-adjacent geometry that ignores swimming is already optimistic.

Each relation's doc names which direction of claim may use it, and AC7
separates the two by fixture, not by prose.

### 4.2 Fluid identity

`World.flooded` is one bag holding water and lava alike
(`crates/compiler/src/nav.rs:871`; `crates/compiler/tests/lava_floor.rs`
lands prefab lava there precisely so it is never floor). Refusal needed no
identity; credit does — a swim credit through lava is the pass that cannot be
survived. **The World must carry the identity of the fluid that wets each
cell.** The undistinguished union keeps serving every refusal-direction proof
unchanged; only cells whose fluid is water are eligible to either swim
relation — the generous relation included, because a body vanilla's own
physics kills is not a way even to an adversary.

### 4.3 The authored surface: nothing

The creator writes exactly spec-0038's accepted surface — `flood_levels`,
`set-flood-level`, `regions[]` — and no more. The credit is not authored:
wherever the engine can prove a swim leg sound it credits it, exactly as it
credits a walk leg without a field saying "floor may be walked on". Water
that stands is climbable — a fact about the world; an opt-in field would be a
design decision wearing a mechanism's clothes, and would quietly opt the
campaign out of the adversary direction. A campaign wanting a purely-closing
tide simply designs one; extra provable routes cost it nothing.

What the build owes instead is **legibility**: every swim-credited leg is
reported with the level it credits, the body of water it swims, and its
climb-in/climb-out cells; a credit binding zero cells is a finding; and a
route refusal over a tide-gated way must be able to say *walkable only at
level L, which is not provably in force here* — naming the level.

### 4.4 Scripted bodies stay ground-routed

The swim edge class is **player-only**. Every scripted body keeps ground
routing (`crates/compiler/src/traversal.rs`: a declaration "deliberately does
not change how a route is computed"), the same shape as use-gate edges, which
exist for the player and are withheld from NPC walkers. Consequence for
`DW0455`: its refusal of `locomotion: aquatic` may stand — scripted bodies
still cannot be held to a swim — but its message's universal claim
("water-flooded cells are impassable and never floor for EVERY body, so there
is nothing for an aquatic claim to feed") becomes false as written and is
rewritten at implementation (AC10).

## 5. Acceptance criteria

Machine-checkable assertions, each marked **[proof]** — an element that
answers differently if the implementation is wrong — or **[coverage]** — it
proves a surface is authored or a survey ran, never that the thing is right.
Every proof states its binding count (cells, legs, fixtures examined); a zero
binding is itself a red. Diagnostic codes below are the implementation's to
allocate; each owes a test and a `docs/reference/compiler.md` row, and the
new surface owes its gallery element in the same PR and its
`docs/demo-levels.md` row.

1. **The carrier proof binds before any credit exists.** In any tree where a
   route proof credits a swim leg, spec-0038 §2.4's two carrier fixtures —
   the flood-only ledge and the drained basin — exist and fail red under the
   carrier proof's allocated code, naming cell, carrying state and stranding
   state. The ordering is structural: the credit cannot merge green without
   them. **[proof]** — an implementation that credits without the carrier
   proof builds both fixtures green.
2. **The credit binds to the level in force.** A fixture campaign declaring
   two levels, with a way walkable only through water wet at the higher
   level: (a) the completability model chooses the swim leg exactly at DAG
   points where the higher level is provably in force, and different legs at
   the boot level (spec-0038 §6.8's worked-example fixture, extended); (b)
   the same campaign with the raising transition removed is refused, the
   refusal naming the level not provably in force. **[proof]** — a
   level-blind credit answers (b) green; a credit that fails to close on
   tide-out answers (a) with the same leg at both levels.
3. **Stranding is still charged at both levels.** The flood-only-ledge
   fixture stays red *after* the credit lands: at high water the perch now
   has a swim route back, and the red is entirely about the subsequently
   possible low state. Symmetrically the drained basin. **[proof]** — an
   implementation that quantifies delivery only over the carrying state
   greens the fixture the moment it learns to credit the swim back.
4. **The climb-out band bounds where a swim can put a body.** A fixture whose
   only exit from the wetted set is a standable cell with feet at
   level + 2 is refused, naming the band and the cell; lowering that cell one
   block builds green. **[proof]** — an exit rule looser than the band greens
   the refused variant.
5. **Fluid identity separates water from lava.** AC2's fixture geometry with
   the body's fluid replaced by lava is refused — no credit — and
   `crates/compiler/tests/lava_floor.rs` stays red, unmodified. **[proof]**
   — an identity-blind credit through the `flooded` bag greens both.
6. **The air clock bounds the credit.** A fixture whose only swim leg
   contains a fully-submerged segment the measured air budget cannot cover is
   refused, naming the segment and the budget. **[proof]** — an unbounded
   credit greens it. Companion: every speed and air constant the bound uses
   is read from the committed spike record (`tools/spike-fluid-plane/`
   observations), never a hand-written literal, asserted by a from-the-record
   test. **[coverage]** — traceability proves the constant's provenance, not
   the measurement; the measurement's own check is the re-runnable spike.
7. **The two calibrations are separated by fixture, not prose.** (a) A
   negative claim — an optionality claim (spec-0051) or a route-uniqueness
   claim — over geometry a generous swimmer defeats and a certain swimmer
   does not, is refused: the adversary uses the generous relation. (b) AC4
   and AC6 are the mirror: the credit uses the certain one. **[proof]** — a
   single shared relation, however calibrated, answers (a) or (b)
   differently.
8. **Isolation and determinism.** A campaign declaring no `flood_levels` and
   routing over no water builds byte-identical, engine-diff style: counted
   files, named deltas, zero expected; and any build rebuilt twice is
   byte-identical (ADR-0006). **[proof of isolation only]** — it answers
   differently if the implementation perturbs what it must not touch, and
   says nothing about whether the credit is right; marked so.
9. **The live tier swims a credited leg.** Whether the harness can swim is an
   unestablished capability question and is the implementing round's *first*
   spike. The first campaign routing over a swim credit carries a bot-tier
   live proof: the bot traverses the credited leg on the pinned server at the
   level in force. A credit the bot tier cannot exercise leaves the machine
   gate unable to admit the campaign that uses it — that **blocks, not
   waives**. **[proof]** — a credit unsound in vanilla (wrong band, wrong air
   arithmetic) fails on the real physics; conditional on the spike, and the
   conditionality is stated rather than silently assumed.
10. **The false universal dies with its carrier.** The `DW0455` message and
    every doc line asserting that water is impassable and never floor *for
    every body / every proof* is rewritten to the surviving truth (piece walk
    and scripted bodies keep the refusal; the player's route proofs do not),
    and the test asserting the message text
    (`crates/dsl/tests/v11_body_traversal.rs`) moves with it.
    **[coverage]** — it proves the text was edited, not that the new text is
    true.
11. **The standing refusals stay executable, unmodified.** These pass with
    zero diff in the implementing PR:
    `crates/grammar/src/nav.rs` `a_body_stands_on_stone_and_not_on_water`,
    `water_is_never_occupied_and_never_a_floor`, the flooded-ward/dry-spine
    test; `crates/dsl/src/blockshape.rs` fluids-answer-no/no;
    the `DW0544`/`DW0546` fixtures (a fluid fill still lays no footing —
    credit is an edge class, not footing). **[proof]** for §3's containment
    claim — an implementation that reclassified fluid passability or piece
    standability could not leave them green.
12. **Every moved verdict is named.** Full suite `--no-fail-fast` before and
    after; every fixture whose verdict moved is enumerated in the implementing
    PR, none silently absorbed. **[coverage]** — it proves the survey ran,
    not that each move is right; the moves themselves are judged by AC1–AC11.

## 6. Refused here

New refusals only; every spec-0038 §5 refusal not named in §1 stands.

- **Swim for scripted bodies.** Ground routing for every non-player body is a
  documented property (§4.4); a scripted swimmer is a walk-driver capability
  with its own cost, unpriced and unasked-for.
- **A passability reclassification.** Flipping `Collision::Fluid` or the
  piece walk's answers reaches every proof at once (§3); the credit is an
  edge class in the one place that owns causal state.
- **An authored credit field** ("swimmable: true"): §4.3 — a design decision
  wearing a mechanism's clothes, and a quiet opt-out from the adversary
  direction.
- **A lava credit**, and any per-fluid survivability model (fire resistance,
  potions): the identity rule (§4.2) makes water the one creditable fluid;
  the day a campaign names a need, the surface is already keyed to fluid
  identity.
- **Vertical carriage as a credit** — a route that requires the *transition
  itself* to lift a standing body (fire the raise while the party waits, then
  swim out). The route proofs bind per causal state, not across a
  transition's moment; spec-0038 M8 measured that a rising plane does not
  displace a body, so mid-transition carriage credits a physics the world
  does not have. What is creditable is the world after the transition, which
  AC2 covers.
