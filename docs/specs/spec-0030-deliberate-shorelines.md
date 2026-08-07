# spec-0030 — Deliberate shorelines (`flood`)

Status: Proposed · Owner ruling: 2026-08-06 · DSL: world-edits stage v0.9.0

## Problem

`DW0364` (spec-0026 §2) refuses any standable cell at or below a non-void
horizon's flood line, **with no exemption** — that exemption is precisely what
made defect #149 (the drowned tide mill) invisible for a release cycle.

Applied to `nobodys-cave-island`, the flagship released campaign, it fires on
**26 standable cells at world y=62**, the ocean line, and the build stops. The
prefab is innocent (`island-greenfield.nbt` has zero standable cells at its
declared walk plane − 1). The cells come from two stage-7 edit batches,
`batch/west-bank-falls` and `east-bank-falls`, whose `morph lower` rolls the
meadow berms down past the waterline — the shoreline the campaign is *supposed*
to have.

**Owner ruling, 2026-08-06: this is intended.** A shoreline that gets its feet
wet is the design. It needs a first-class declaration.

## The trap this spec has to avoid

A declaration that merely suppresses `DW0364` is the same hole with a nicer
name. `shallows: true` on a batch, an area or a prefab reopens #149 the first
time an author typos it, copy-pastes it, or reaches for it to silence an
unrelated red.

So the declaration must make the compiler check something **different**, not
something **less**.

## Decision

Add one stage-7 edit verb:

```json
{ "verb": "flood", "region": "region/<name>" }
```

**It does not place water and it does not skip a check. It admits the horizon's
ambient sea into a declared envelope, and the compiler computes what the sea
does there.**

1. The author names a **region** — an envelope, not a cell list.
2. The compiler computes the sea's **reach** inside it: seeded from ambient
   water bordering the envelope, propagated cardinal-sideways and downward
   through waterable cells at or below the flood level, confined to cells
   **inside a placed piece** (outside them the generator's sea is already
   there, and `DW0364` does not look there either).
3. Every reached cell is **materialized** as `minecraft:water` — in the model
   *and* in the emitted `world_edits` function.
4. Everything the sea does not reach stays dry, and stays under `DW0364`
   **unchanged**.

### Why this is not an exemption

`DW0364` is untouched. A declared cell stops being a `DW0364` violation only by
**ceasing to be ground**: water cells are impassable and never standable in the
occupancy model (task #45), so there is nothing left to stand on. The verb can
only ever *remove* walkable cells, never add one — it is fail-closed by
construction, and using it costs the author the ground it takes.

Three consequences follow, and they are the whole safety argument:

- **You cannot silence a drowned room with it.** A sealed interior a block under
  the sea is not reachable by the sea, so the declaration binds nothing and is
  `DW0394`. The #149 fixture with a `flood` declared straight over it stays red;
  it merely changes which red.
- **You cannot over-declare your way out.** A wider envelope wets more of your
  own map. An envelope that swallows a corridor floods the corridor and turns
  nav red. There is no envelope that silences anything.
- **You cannot under-declare.** Water that would flow on past the envelope into
  an undeclared air cell of a placed piece is `DW0395`: the shoreline is not
  where the author said it is.

### Why the water is materialized rather than modelled

A compiler that models water it did not build commits the exact sin `DW0364`
punishes — a model that says one thing while the delivered world does another,
in the opposite direction. Today the island's shoreline is whatever vanilla
fluid ticking does on first boot; after this the waterline is a deterministic
function of the DSL (ADR-0006), 25 emitted source blocks, and the model and the
world agree cell for cell.

### Why the declaration lives on the edit script

The cells come from stage-7 world edits, not from the prefab, and the verb sits
in the same language, frame and region vocabulary that sculpted the bank.

| Alternative | What it would have cost |
|---|---|
| Flag on the **edit batch** | Batch-wide scope over a dozen verbs; the claim would cover cells no one meant it to, and batches that touch the same band later would be silently included. |
| Field on the **area** (stage 1) | Effectively campaign-wide. This is the "nicer name for an exemption" shape: one word, no locality, nothing computed. |
| Field on **prefab metadata** | Wrong owner. The prefab is innocent, is placed at different `y` in different campaigns, and "at the waterline" is a placement-and-edit-time fact, not a property of the `.nbt`. |
| **Nothing** — make the ambient sea a global flood source in `assembled` | The truthful model needs no declaration at all, and this was seriously considered. Rejected: it silently changes every ocean campaign's model and every released world's bytes with no declaration to point at, it makes authorial intent invisible in the DSL, and it converts `DW0364`'s strict default (*no ground under the sea, ever*) into a physics question by default rather than by explicit, proof-carrying exception. |
| **Nothing** — author the water with the existing `fill`/`replace` | Expressible today (`intersect` a surface band with a y=62 box, then `fill` water), and it clears `DW0364`. Rejected as the *primary* answer because it makes the author hand-compute the sea's reach — the downstream folklore CLAUDE.md forbids: the ambient sea is a vanilla primitive the DSL was making content simulate by hand, and nothing checked the result. |

### Timing

The verb applies at its position in the batch (so `delvec edit`'s per-batch
snapshots show the shoreline the author is looking at), and the **tideline
invariant** re-proves over every declaration so far after **every** batch — the
same cadence as boundary safety. A later batch that re-cuts the bank is red
where it happened.

## Diagnostics

| Code | Meaning |
|---|---|
| `DW0394` | The declaration binds nothing: the horizon has no ambient water at all, or the sea reaches no cell of the envelope. A zero binding is a finding, never a pass. Build-tier (exit 3). |
| `DW0395` | The tideline invariant: the admitted water does not stop inside the envelope, or a later batch left a cell the sea reaches with no water in the model. Build-tier (exit 3). |

`DW0364` keeps its exact behaviour and its exact domain. `DW0141` fences the
verb below world-edits `dsl_version` 0.9.0.

## Acceptance criteria

1. `nobodys-cave-island`, with a `flood` declared over its bank bands, **builds
   green** (`delvec build`, exit 0) on the spec-0026 engine + the `walk_y`
   content backfill.
2. The declaration's **binding count is non-zero and reported**: the island's
   `flood` wets exactly **25** cells, all at world y=62, and exactly 25
   `minecraft:water` cells appear in the emitted `world_edits` function.
3. **`DW0364` still fires where it should**, with the declaration in place:
   removing only the one-cell bank-pocket fill from the island's tideline batch
   — leaving the `flood` envelope covering that cell — is `DW0364` on exactly
   **1** cell. Removing only the `flood` verb is `DW0364` on **25**.
4. The spec-0026 #149 fixture with a `flood` declared over the whole drowned
   piece is **`DW0394`, never green** (`crates/compiler/tests/spec0030_flood.rs`).
5. A shoreline notch cut to the ocean line is `DW0364` without a declaration and
   **exit 0 with exactly one emitted water cell** with one — red→green over
   identical geometry.
6. A `flood` whose envelope covers part of a wider notch is **`DW0395`**.
7. A `flood` in a horizon with no ambient water is **`DW0394`**, naming that
   cause.
8. A `flood` in a world-edits stage below `dsl_version` 0.9.0 is **`DW0141`**,
   exit 1.
9. **Determinism (ADR-0006)**: a flooded campaign builds byte-identically twice,
   and the island does too.
10. A campaign that declares no `flood` builds **byte-identically** to its
    pre-spec-0030 output.

## Non-goals

- No change to `DW0364`'s rule, tier, domain or message.
- No global change to the ambient's participation in the assembled flood model.
- No modelling of *wading*: a materialized shoreline cell is water, and water is
  impassable in the occupancy model. In the delivered world the party can wade
  through a one-deep tideline; the model declines to route through it. That is
  the conservative direction (it can only lose walkable cells, never invent
  one), and player confinement is owned by the `boundary` clock (`DW0320`), not
  by nav impassability. A first-class wade cell would be a third collision class
  through every proof in `nav` and is deliberately out of scope.
