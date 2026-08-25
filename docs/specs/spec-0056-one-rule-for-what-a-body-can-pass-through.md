# spec-0056: One rule for what a body can pass through

- **Status**: Proposed
- **Ground**: engine `4ee0c0e1`. Every count below was measured at that
  revision by a scratch harness driving `delvewright-grammar`'s public API —
  not quoted from the round that first reported the shape. The sibling repair
  this spec completes — one step rule, `delvewright_dsl::metrics::step_allowed`
  — is on `fix/one-rule-for-whether-a-body-can-get-there`, **unmerged at the
  time of writing**; §5 states what this spec takes from it.
- **DSL / Diagnostics**: this spec allocates **no** `dsl_version` and **no** DW
  code. No schema surface moves; the change is fidelity inside admission
  proofs, and its refusals arrive through the contract gates that already
  exist (`contract-well-formed`, `contract-closure`, `contract-edge-proof`,
  reachability). Released campaigns are unreachable: they reproduce through
  their pinned engines.
- **Non-goals**: swim/underwater traversal (deliberately unmodelled —
  spec-0038's two conservative directions stand); splitting the body question
  from the sightline question for partial-height blocks (§3.3); the compiler's
  assembled-world machinery (occupancy, flood, settle, waterlogging — those
  are facts about a *world*, not about a block state, and they stay in
  `delvec`); any use-gate waypoint tagging in grammar exports.

## 1. Two authorities, measured disagreeing in both directions

The grammar walk answers "can a body occupy this cell" in
`crates/grammar/src/nav.rs` (`impl Voxels for VoxelModel`): a cell is passable
iff its block `is_air()` or its name ends in `_skull`. Everything else is a
full solid cube.

The compiler carries a real collision model in
`crates/compiler/src/assembled.rs`: `collision_top_16` (a block state's
collision-box top face in sixteenths, from Minecraft Java 1.21.11 block
shapes), and the class predicates built on it — `is_no_collision_plant`,
`is_thin_decoration`, `is_partial_floor`, `is_tall_barrier`, `is_fence_gate`,
`is_fluid`. It knows glow lichen has an **empty** collision shape.

Same predicate, two implementations, and the divergence is live in both
directions (12×12 room, floor + two air courses + ceiling, measured at the
ground revision):

- **Rejects-valid.** A four-column glow-lichen band across the floor: the
  lichen cell fails `passable`, the cell above it loses its second course of
  headroom, so *no floor level survives in the whole column* — `standable`
  drops from 144 cells to 96, and a walk seeded at one wall reaches 48. The
  band severs the room. Consequence: **no grammar-built zone can hold any
  zero-collision block in walkable space** — glow lichen, carpets, grass,
  pressure plates, one-layer snow. A prior round measured the same shape as
  three contract gates red at once over one lichen bed.
- **Accepts-invalid.** A basin of `water[level=0]` under open air:
  `standable` directly above the surface answers **true** — the walk proves a
  body standing on open water, which is precisely the route credit spec-0038
  forbids. The library already leans on the wrong model as folklore:
  `library/causeway.rs` argues its flood is unstandable *because water is not
  air* — an assertion about the defect, not about water.

## 2. The one rule, and where it lives

**A block state's collision shape is one fact with one home:
`delvewright-dsl`** — a module beside `metrics` (proposed:
`crates/dsl/src/blockshape.rs`), holding `collision_top_16`, its constants,
and the class predicates, **moved from `assembled.rs`, not redesigned**.
`delvec` keeps its current names as delegating re-exports (no call-site
churn); the grammar's `Voxels` impl derives its answers from the same table.

The placement argument is `step_allowed`'s, unchanged: `delvec` is published
and may depend only on published crates; `delvewright-schem` is not published;
`delvewright-dsl` is the one crate both already reach. And it is the right
home by object class, not only by reachability: the table is a fact about
**a vanilla block state under the pinned game version** — the same kind of
pinned-physics fact, in the same sixteenths, as the auto-step and jump-apex
budgets that already live in `metrics` (`MAX_AUTO_STEP_16 = 9`,
`MAX_JUMP_RISE_16 = 20`); `collision_top_16` is what a correct rise
measurement is made of.

The authority's input stays the vanilla string form
(`minecraft:oak_slab[type=top]`) — one parser, already written and
state-sensitive. Grammar's `BlockState` prints exactly that form, and
`VoxelModel` is palette-indexed, so classification is once per distinct state,
never per cell.

## 3. Review shape 3, second step: what the wider site can and cannot express

The general mechanism whose binding is too narrow is the shared walk,
`delvewright_schem::nav`, and its `Voxels` trait.

**3.1 The walk itself is the wrong home, not merely a risky one.** Its
contract — "what counts as passable is content, the walk is mechanism" — is
still right for the part that *is* content (the rule library's floor-skull
convention). And the publish fence means a table in `schem` would still be
unreachable from `delvec`, so moving the vocabulary into the walk would
recreate today's private copy one crate over. The walk keeps taking its
answers per cell; the table is what implementors consult to give them.

**3.2 The trait can express everything the defect needs.** With the step-rule
branch's `Voxels::floor_top_16` (§5), the surface is `passable` / `floor` /
`floor_top_16`, and every collision class maps onto it:

| class (per the authority) | `passable` | `floor` | `floor_top_16` |
|---|---|---|---|
| empty / thin (top < 8/16) | true | false | — (body rests on the block below; sub-slab tops are noise under the 9/16 auto-step, the compiler's own THIN rule) |
| partial floor (8..16) | false | true | the top |
| full cube (default) | false | true | 16 |
| tall barrier (fence/wall) | false | false | — (no walking body stands on a 1.5-block top) |
| fence gate | true | false | — (the player model: adventure-mode use opens it; and for closure a gate is never a seal, so reading it as a hole is the sound direction) |
| fluid | false | false | — (spec-0038: routes never credit water, and nothing stands on it) |

`floor`'s default (`!passable` inside the box) is overridden by exactly the
three classes whose two answers split: thin decorations, tall barriers,
fluids.

**3.3 What one boolean cannot carry, named.** `passable` is documented as
answering for a body *and a sightline*. For the class this spec changes —
empty collision — the two agree (glow lichen neither collides nor occludes).
Partial-height blocks keep the full-cube reading for the eye, which refuses
sightline claims it should sometimes grant; splitting the questions would need
a second per-cell answer (`occludes`) with its own conservative direction per
claim sign. That surface is not proposed here; until it exists, the narrowing
is the refusing direction and is recorded, not hidden.

## 4. What the grammar layer needs that the compiler's model does not give it

1. **Reach.** The table lives in `delvec`, and no dependency path from
   `delvewright-grammar` to it exists or should (grammar is upstream;
   `delvec`'s published dependency set is closed). The move in §2 *is* the
   repair for this gap.
2. **An entry point for its own vocabulary type.** The table is keyed on the
   string form; grammar holds structured `BlockState`s. Rendering
   `to_string()` once per palette entry closes it — cheap by construction
   (palette ≤ 65,536 states) and keeps one parser. No structured API is owed.
3. **Nothing else.** The rest of the compiler's model (occupancy, flood,
   settle) is about an assembled world and answers questions the admission
   proofs do not ask. The authority is per-block-state facts only; asking
   more of it would move world machinery into a document crate.

## 5. Ordering against the step-rule branch

The partial-floor row of §3.2 flows through `Voxels::floor_top_16`, which the
step-rule branch introduces with a full-cube default. This spec **builds on
that hook**; if that branch does not land, the implementation of this spec
carries the identical hook itself — it is the measurement half of the same
rule, and there is exactly one of it either way.

## 6. Acceptance criteria

Each is a test in this repository; none needs a second engine build. A
criterion the implementation cannot yet satisfy is a debt row here, never a
pass.

1. **One authority, tripwired.** `delvec` exports its collision names by
   delegation to the dsl module, and a `crates/compiler` test asserts
   byte-equal answers between the exported names and the dsl authority over a
   probe set covering every class and every state-sensitive case
   (`oak_slab` default vs `[type=top]`, `snow[layers=1]` vs `[layers=8]`,
   `pale_moss_carpet` both forms, a fence, a gate, `water`, bare vs
   namespaced ids). Green-by-delegation today; it exists to red the day a
   second private copy diverges — that is its stated binding.
2. **The motivating scenario, red first.** A grammar unit test builds the
   12×12 lichen-band room of §1: `standable_cells` is **byte-identical as a
   set** to the bare room's 144, and `reachable_from` one seed covers all of
   it. (At the ground revision this is 96 standable / 48 reachable — the
   test is written red before the fix.)
3. **The fluid probe, red first.** The §1 basin: no cell above the water
   surface is standable, and no water cell is. (Currently the surface
   answers standable — red before the fix.)
4. **Partial floors measure, not lie.** `floor_top_16` for
   `oak_slab[type=bottom]` answers 8 through the grammar's `Voxels` impl,
   and a walk steps from that slab onto an adjacent full block as a
   `Rise::Walk` (8/16 ≤ 9/16), with no headroom demanded. **Debt until §5's
   hook is on `main`**; recorded here, not promoted.
5. **The folklore fossil is retired in the same PR.** `causeway.rs`'s flood
   assertion holds under the new model for the stated reason (fluid never
   floors), and its doc comment says that reason; the zone's existing tests
   stay green. `docs/reference/grammar.md`'s nav section and
   `docs/reference/compiler.md`'s collision-model section are updated in the
   same PR (tooling-sync).
6. **No verdict weakened.** The full grammar suite and the compiler suite are
   green with no test threshold or contract gate loosened; any verdict that
   *changes* is enumerated in the implementation PR body with its direction
   (a refusal that stops firing on a zero-collision block is the point; a new
   refusal is a finding to explain).

## 7. What a coverage gate would and would not prove here

The engine coverage gate enumerates schema units, and this change adds none —
**coverage is structurally silent about this defect**, which is exactly where
it lived for as long as it did. A gallery element scattering every
zero-collision id would prove the surface *authored* and nothing more: it
compiles green under the wrong model too.

The elements that answer differently when the implementation is wrong are the
**set-moving assertions** — criteria 2 and 3 compare whole standable and
reachable sets, and a wrong table moves those sets; it cannot leave them
byte-identical — and the **cross-instrument tripwire** of criterion 1, which
reds when two copies of the rule exist and disagree. What is measured, and
where: standable/reachable sets over the whole fixture model, compared as
sets, never as counts.
