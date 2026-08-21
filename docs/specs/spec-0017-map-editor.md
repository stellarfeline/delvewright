# spec-0017 — The map editor (LLM world editing, layers 2+3)

- **Status**: Draft (polishing L2/L3 is an M5 mandate, pulled forward; consumes
  spec-0015's read half — snapshot / blocking-chart / manifest)
- **Name**: the component's official name is **the map editor** (地图编辑器).
- **Vision**: the designing LLM edits worlds the way it already inspects
  them — a closed loop of edit verb → deterministic replay → snapshot —
  instead of every visual fix being a Rust generator change.

## Layers (owner frame, settled)

- **L1 siting** (place structures on a macro world) — M6, out of scope here.
- **L2 massing**: declarative control of the jigsaw layout — the piece/pool
  graph of an area.
- **L3 detailing**: block-level finishing of generated pieces — terrain
  shaping, boundary treatment, interiors, lighting, dressing.

## Source of truth: edit scripts (ADR-0006 preserved)

The editor NEVER mutates world files as truth. The artifact of record is a
versioned, schema-enforced **edit script** per area
(`world-edits.json`, a DSL stage after world assembly), replayed
deterministically by the compiler after generation: same DSL + same edits +
same seed → byte-identical world. Editing sessions leave no state outside
the script. The LLM writes edit verbs; it never writes raw NBT or
mcfunction (no-hack doctrine: a finishing operation with no verb is
excluded until a verb exists).

## L3 verb set (initial)

All verbs operate on **named regions** (boxes/masks in piece-local or
anchor-relative frames) and reuse the generators' proven primitives
(seeded palette recipes, value-noise, scatter with keep-clear envelopes):

- `select` — define/compose regions (box, surface-band, palette-match).
- `fill` / `replace` — palette-recipe fill (seeded noise, never uniform).
- `carve` — clear to air, sealing-aware.
- `morph` — surface raise/lower/smooth within a region (berm → natural
  slope is the canonical use).
- `scatter` — seeded dressing (flora, rocks, props) honoring keep-clear.
- `plant` — structural flora via the canopy rules (lean-or-grow).
- `fragment` — stamp a prefab fragment (provenance/license recorded, as
  prefabs; ADR-0013).
- `relight` — re-run the lighting pass over a region (spec-0010 machinery).

## L2 verb set (initial)

`swap-piece`, `insert-piece`, `remove-piece`, `resize-piece`,
`rewire-socket`, `reseed-piece` — thin declarative surface over the
existing solver; every application re-runs the full assembly validation.

## The loop

`delvec edit apply` replays the script and auto-renders snapshots of every
edited region (spec-0015 read half) so the designing LLM sees each batch's
result; `delvec edit preview` renders without persisting the script entry.
Shot grammar from spec-0015 applies (declared expectation per snapshot).

## Invariants (machine-enforced after EVERY edit batch)

1. Walk-envelope preservation: corridor-clear assertions and DW0311
   walkability re-prove on the edited world; an edit that breaks the
   critical path is a compile error, not a runtime surprise.
2. Sealing + relight: carved/filled regions re-enter the sealing and
   lighting passes (spec-0010); no dark-leak or mob-leak regressions.
3. Determinism: double replay byte-identical (existing ADR-0006 test
   machinery extended over the edit stage).
4. Boundary safety: no walkable cell may border void after edits — the
   guarantee the greenfield berm currently provides physically becomes a
   checked invariant, freeing the boundary's SHAPE to be natural landform.

## Acceptance criteria (M3-island polish is the first real workload)

Each names the reading that would make it vacuous.

1. **Determinism over the edit stage**: same DSL + same edit script + same
   seed → byte-identical world across a double replay (the ADR-0006 gate
   extended over the edit stage). *Vacuous if* the replayed script is empty —
   the run states how many edits it applied, and zero applied edits is a
   finding, not a pass.
2. **Every invariant can refuse**: one red fixture per invariant — an edit
   that breaks the critical path (walk-envelope), one that opens a dark or
   mob leak (sealing + relight), and one that leaves a walkable cell
   bordering void (boundary safety) — each a compile error naming the edit
   batch that caused it. *Vacuous if* the invariants are only ever seen green:
   a validator never observed refusing has not been shown to validate.
3. **De-wall by script alone**: the greenfield berm replaced by natural
   boundary treatment through the edit script with **no generator code
   change** — asserted on the change's scope (edit script and compiler
   replay only, no `prefabs/` generator diff) — with the full ladder and all
   invariants green. *Vacuous if* the ladder ran against a world built
   without the script — the build names the edit script it replayed and the
   batch count it applied.
4. **The closed loop, end to end**: an editor session leaves an edit script
   plus an auto-rendered snapshot per edited region and zero hand-authored
   blocks or NBT. *Vacuous if* the snapshots render unedited regions — each
   snapshot names the edit batch it renders and carries the declared
   expectation the shot grammar (spec-0015) requires.

Not machine-assertable, said plainly: whether the island's reworked mountain
silhouette and shoreline *read* right is the owner's acceptance, given on the
snapshot loop. The machine proves the loop closed and the invariants held; it
cannot prove the mountain reads as a mountain.

## Non-goals

L1 siting (M6); free-form per-voxel painting; runtime (in-game) editing;
any editor mutation outside the script.
