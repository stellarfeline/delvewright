# Water as a way, while the tide holds it — scoping

Design requirement: rising water opens ways to high places a body cannot
otherwise reach; falling water opens the low places it was hiding; the level
moves because the story reached a stage, never because a clock ticked. The
first half — a route that exists only at high water — is unprovable today:
every reachability proof answers that it is not a route. This note scopes the
repair. It implements nothing and consumes no number; every code citation is
at engine revision `d3e7709ff927269b11a45fa471b6b5075a99b74d`.

**The verdict up front: a general mechanism bound too narrowly — in three of
the four places the feature needs — plus one genuinely missing member, which
is a widening of an existing vocabulary rather than a parallel mechanism.**
The evidence is in §2. And one place the rule must NOT go, because the site
cannot express its quantifier (§2.4).

## 1. What refuses this today

Four authorities refuse it, at four layers, each deliberate, each with the
universal quantifier ("never", "every body"), and each backed by executable
text (§5 enumerates the tests).

1. **The block-shape table** (spec-0056, the one answer for the whole
   workspace): `Collision::Fluid` answers no to both questions — a body
   neither passes through a fluid cell nor stands on it
   (`crates/dsl/src/blockshape.rs:43` table row, `:461` the variant, `:502`
   "a fluid is a fluid whatever else it looks like", `:558` the truth row).
   Water and lava are one class here (`is_fluid`, `:157`).
2. **The shared piece walk** (`delvewright_schem::nav`, consumed by the
   grammar's contract gates and admission): standability requires a floor,
   and a fluid is never a floor. The grammar's own `Voxels` impl states the
   quantifier: "a fluid (spec-0038 — a route never credits water, and
   nothing stands on a surface)" (`crates/grammar/src/nav.rs:88-91`).
3. **`delvec`'s routing world**: `flooded` cells are impassable and never
   floor for every route, every walker, every proof —
   `World::is_occupied` (`crates/compiler/src/nav.rs:1958-1963`) counts
   `flooded` beside `solid`, `tall` and `lethal`; `standable_fp`
   (`:1977-1983`) demands solid ground below; the world-model doc states it
   as the rule (`:832-838`). Runtime writes inherit it: a fluid fill is
   impassable and never lays footing (`DW0544`, `:98-112`; `DW0546`), and
   "flood beats them all" in per-DAG-state world views (`:1432`).
4. **The DSL surface itself refuses the authoring direction by name**:
   `DW0455` rejects `locomotion: aquatic` with the quantifier written into
   the message — "routing has ONE reachability model, standable ground, and
   water-flooded cells are impassable and never floor for EVERY body, so
   there is nothing for an aquatic claim to feed"
   (`crates/dsl/src/validate.rs:2309-2340`).

Above the code, the decision record: spec-0038 (Accepted) §2.3 rules "A
rising level only ever *closes* modelled ROUTES … what it opens is carriage,
and carriage is §2.4's proof, never a route"; §2.4 declares its carrier
proof "deliberately NOT a swim-traversal model: no route may ever REQUIRE
the water"; and §5 refuses wading/swim traversal with its disproof resting
on the premise that "no campaign has yet named a need". **That premise has
expired**: the commissioning campaign's design names exactly this need. A
spec is a historical decision record, so the repair is a new spec
superseding those three clauses — everything else in spec-0038 stands, and
most of it turns out to be this feature's prerequisite (§3).

Quantifier check, because it decides where the repair goes: the refusal
really is "never", not "not for this question". No proof path credits water
in the positive direction. The only water-aware positive machinery the
compiler has runs in the *charging* direction — stranding, escape, seepage
(next section).

## 2. The general mechanisms that nearly reach it, and where each stops

### 2.1 "Open only at story stage X" — already general, already landed

The quantifier the feature needs over *when* is the region-event machinery:
`World::region_state_at` computes the world in force at a quest-DAG point,
latest-causal-write-wins, already non-monotone
(`crates/compiler/src/nav.rs:3746`, applied per leg at `:4012`, `:4599`,
`:6486`). Route proofs already bind per causal state. spec-0038 §2.3 already
designs the tide into exactly this family (`FloodEvent` beside
`RegionEvent`), with the forced-root rule making the level in force
build-time-knowable. Nothing about "only at high water" needs a new
mechanism — **but none of §2.3 is landed**: no `flood_levels`,
`set-flood-level` or `FloodEvent` symbol exists anywhere in `crates/`, and
the flood level is still the constant `SEA_LEVEL: i32 = 62`
(`crates/compiler/src/plan.rs:48`). The level cannot even be stated yet.

### 2.2 Swim geometry — exists, bound to the charging direction only

The compiler already models a swimming body — in boundary safety. The ocean
arm of `DW0322` (`crates/compiler/src/nav.rs:7062-7108`, `swimmable` at
`:7273-7280`) has the entering rule (a walkable cell adjacent to an open
column down to ambient water — "whether the player walks in, wades in or
falls from a cliff, they end up afloat"), surface-connectivity into bodies,
and the **climb-out band** — a swimmer exits onto a proven reachable
walkable cell whose feet are at `sea.level` or `sea.level + 1`; "a ledge
higher than that is a wall to a swimmer" (`:7081-7086`). The fluid-reach
computation exists too (`crate::assembled::flood`, used at `:7838`,
serving `DW0318`/`DW0851`). Every one of these binds only to "can a body
that ended up in the water get back / where does escaping water go" —
never to "may a route credit this". Same geometry, narrower binding: the
third review shape.

Note the sound part of the reuse: the climb-out band and the entering rule
are under-approximations of what vanilla certainly allows, which is the
same conservative direction a route credit needs. The surface-only
connectivity restriction ("a diver might swim under a land bridge … this
model deliberately does not count on", `:7078-7080`) is also safe in both
uses — under-counting connectivity can only refuse. What it cannot express
is the feature's core case, ascent through a flooded volume, so the credit
relation needs 3D reach through the wetted set with an air bound (§4).

### 2.3 Two calibrations of one walk — precedented

"A route may credit X" and "a uniqueness/negative claim must survive X" need
opposite approximations, and the codebase already holds that pattern with
its directions documented: `reachable_with_fall` is "deliberately *more*
permissive … which is why the two are used in opposite directions and never
interchangeably: forward, where a piece is entered by stepping off a ledge …
and as the adversary, when a piece claims a route is the only route … The
negative direction … is asserted under the plain walk instead, because …
proving a negative under the generous model would be circular"
(`crates/schem/src/nav.rs:229-241`). Swim joins this pattern: a *certain*
relation (under-approximation) for route credit, a *generous* relation
(over-approximation) as the adversary for any claim that something is not
reachable. The adversary half is a soundness improvement independent of the
tide — a player can physically swim in vanilla today, so a non-reachability
claim over water-adjacent geometry is already optimistic; the engine's
boundary model already refuses to treat the sea as containment ("the
question is never 'can the player fall out' but 'can the player get back'",
`crates/compiler/src/nav.rs:7062-7064`).

### 2.4 Where the rule may NOT go

The grammar/schem piece walk (the `Voxels` trait and everything the
contract gates prove) is static piece geometry with no notion of causal
state — a monotone fixpoint with no "when". The tide rule is entirely about
when, so hosting it there would be wrong, not merely risky. The piece-level
refusal is also *correct*: a piece cannot know the world's level. So the
grammar-side predicates and their tests stay exactly as they are, and the
rebinding happens only in `delvec`'s `World`, which already owns causal
state. Likewise `Collision::Fluid` stays no/no for the walk: swim is a
distinct edge class, not a passability reclassification — flipping
`passes_body` would silently hand water-as-air to every proof at once,
which is the unsound direction nothing catches.

### 2.5 The genuinely missing members

Two, and both are widenings of existing vocabularies, not parallel proofs:

- **A swim edge class in the route graph.** The player's route vocabulary is
  already heterogeneous — use-gate edges exist for the player and are
  withheld from NPC walkers (`crates/compiler/src/nav.rs:844-847`,
  `without_gate_use`), and fall edges exist with documented directionality.
  Swim is one more edge class with its own admission rule, player-only
  (every scripted body stays ground-routed, `crates/compiler/src/traversal.rs:91-93`).
- **Fluid identity in the world model.** `World.flooded` is one bag holding
  water and lava alike (`crates/compiler/src/nav.rs:871`;
  `crates/compiler/tests/lava_floor.rs` header — prefab lava lands there
  precisely so it is never floor). Refusal needed no identity; credit does:
  a swim credit through lava is the pass-that-cannot-be-walked. The set
  must carry (or split by) the fluid's identity.

## 3. The minimum honest surface

**Nothing new for the creator to write.** The authored surface is exactly
spec-0038's accepted one, unlanded today: `flood_levels: [<y>…]` on a
horizon that declares a water table (stage 1), the gated
`set-flood-level { level }` effect (stage 5), and `regions[]` (§2.6 there).
The credit itself is not authored: wherever the engine can prove a swim leg
sound, it credits it, exactly as it credits a walk leg without a field
saying "floor may be walked on". Water that stands is climbable — that is a
fact about the world, and an opt-in field would be a design decision
wearing a mechanism's clothes; it would also quietly opt the campaign out
of the adversary direction. A campaign that wants a purely-closing tide
simply designs one — extra provable routes cost it nothing.

What the build owes instead of a field is legibility: each swim-credited
leg is reported with the level it credits, the body it swims, and its
climb-in/climb-out cells; a credit binding zero cells is a finding; and a
route refusal over a tide-gated way must be able to say "walkable only at
level L, which is not provably in force here."

## 4. What must be provable

1. **Level-in-force.** A swim edge wet only at level L exists only at DAG
   points where L is provably in force — spec-0038 §2.3's forced-root rule
   supplies knowability, `region_state_at` supplies the per-leg world. The
   machinery is non-monotone already, so tide-in/out/in closes and reopens
   the way with no new proof shape. This is the brief's "must not silently
   become creditable at low water" obligation, and it falls out of the
   binding rather than needing its own check.
2. **Body integrity.** Credit only through cells of a body that is water
   (not lava), saturated, contained and still — spec-0038 §2.2's proofs are
   the precondition, not an optional extra: swim credit through water
   vanilla may drain or is still moving is a proof about a world that
   stops existing.
3. **Entry and exit.** Entry from a proven standable cell by the boundary
   proof's entering rule; exit only through the climb-out band (feet at
   level or level + 1 onto a standable cell) — the conservative rules
   already written at `crates/compiler/src/nav.rs:7066-7086`, rebound.
4. **Bounded underwater exposure.** In-water movement is 3D through the
   wetted set, but an underwater segment must fit inside vanilla's measured
   air clock (spec-0038 measured drowning: air 300→0 in ~15 s; a submerged
   body is NOT auto-lifted by a rising plane — its measurement M8 — so the
   route genuinely requires swimming input, like any route requires walking
   input). Ascent/descent/horizontal swim speeds are unmeasured; they are a
   measurement obligation on the spike template
   (`tools/spike-fluid-plane/`), never an invented constant. Until
   measured, the certain relation credits nothing whose air cost it cannot
   bound.
5. **Stranding still charged in both states.** spec-0038 §2.4's carrier
   proof (delivery cells, per carrying state, per subsequent state, each
   owing a route back) lands **with or before** the credit, and the
   ordering is structural, not prose — one spec, and an implementation
   sequence in which the credit cannot merge without the carrier proof
   binding. §2.4's closing claim that "the two conservative directions
   compose" was true only while routes never credited water; once they do,
   the carrier proof is what keeps the composition sound, in both tide
   directions (the flood-only ledge; the drained basin).
6. **Two calibrations, directionally documented** (§2.3 above): certain
   for credit, generous for the adversary, each function naming which
   direction of claim may use it — the `reachable_with_fall` discipline.
7. **Determinism and byte-identity.** A campaign declaring no
   `flood_levels` and routing over no water builds byte-identical.
   Water-adjacent campaigns may change *verdicts* (routes newly provable);
   nothing is owed compatibility, and the change is named, not smuggled.
8. **The live tier walks it.** The critical-path bot must actually swim a
   credited leg on the pinned server; whether the harness's navigation can
   swim is an unestablished capability question and belongs in the
   implementation round's first spike, since a credit the bot tier cannot
   exercise would leave the machine gate unable to admit the campaign that
   uses it.

## 5. The cost

**Prerequisite (largest single item): spec-0038 §§2.2, 2.3, 2.6 are
unimplemented.** No level surface, no `FloodEvent`, no still-fluid proofs;
`SEA_LEVEL` is a constant (`crates/compiler/src/plan.rs:48`). The tide
cannot be credited before the tide exists.

Then, per crate:

- `crates/compiler`: fluid identity in `World` (§2.5); the swim edge class
  in player routing; the two swim relations; the boundary/seepage bands
  generalised from the constant level to the level in force (spec-0038
  §2.3 already lists this); the carrier proof (§2.4 there); refusal
  messages that name level and body. New diagnostics — codes allocated by
  the planner, never here — each owing a test and a `compiler.md` row.
- `crates/dsl`: the `DW0455` message's universal claim becomes false as
  written and is rewritten (the refusal itself may well stand — scripted
  bodies stay ground-routed; the reason changes). Possibly nothing else.
- `crates/grammar`, `crates/schem`, `crates/admit`: nothing. The piece walk
  keeps refusing, correctly (§2.4 above).
- Harness: swim navigation capability (unestablished, §4.8), plus the
  version-ledger's second consumer (`harness/src/critical-path.ts`
  allowlist) whenever the implementing round is handed its `dsl_version`.
- Docs: a new spec superseding three clauses of spec-0038 (§2.3's
  "rising … only ever closes routes", §2.4's "no route may ever REQUIRE
  the water", §5's wading refusal with its expired premise); reference
  rows; gallery element(s) for the new surface in the same PR; a demo
  level row queued for the mechanic.

**Tests that assert the current refusal as executable text** — the green
tests a naive implementation would read as regressions:

| test | fate |
|---|---|
| `crates/grammar/src/nav.rs:313` `a_body_stands_on_stone_and_not_on_water` | stays — piece scope is correct |
| `crates/grammar/src/nav.rs:367` `water_is_never_occupied_and_never_a_floor` (its own comment: "Wading is a claim in the opposite direction and is deliberately not made here", `:379-380`) | stays |
| `crates/grammar/src/nav.rs:568` flooded-ward/dry-spine | stays |
| `crates/dsl/src/blockshape.rs:637` fluids answer no/no | stays — the walk's answers do not change |
| `crates/dsl/tests/v11_body_traversal.rs:131-153` asserts `DW0455`'s message text | changes — the message's universal claim dies |
| `crates/compiler/tests/lava_floor.rs` | stays red (lava is never creditable) — and is the fixture proving fluid identity matters |
| `crates/compiler/tests/v10_region_write.rs`, `optional_footing.rs`, `laid_footing_root.rs` (`DW0544`/`DW0546`) | stay — a fluid fill still lays no footing; credit is an edge class, not footing |
| every fixture whose red rests on water blocking the only route | enumerated at implementation time by running the full suite `--no-fail-fast` and reading which verdicts moved; each move is named, none is silently absorbed |

## 6. What was established versus not contradicted

**Established** (each by reading the cited code or grepping the tree at the
pinned revision): the four refusing authorities and their quantifiers (§1);
that no spec-0038 §2.3/§2.4/§2.6 symbol is landed while §2.1's `DW0544`/
`DW0546` are; that the swim/climb-out/fluid-reach geometry exists bound
only to charging-direction proofs; that the two-calibration pattern is
precedented with documented directionality; that `flooded` holds water and
lava undistinguished; that scripted bodies are routed on ground rules.

**Not contradicted, not established**: that no proof anywhere makes a
non-reachability claim across water (none was found; the search was not
exhaustive — the adversary calibration is required regardless); that the
harness can swim (named as an open capability question); vanilla swim
speeds and the air-budget arithmetic (named as measurement obligations);
that mineflayer or PackTest can observe a level transition mid-run
(spec-0038 acceptance 8 assumes it; unverified here).
