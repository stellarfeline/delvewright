# spec-0017 — The map editor (LLM world editing, layers 2+3)

- **Status**: Draft (planner, 2026-08-01; owner's 3-layer frame ruling and
  "L2/L3 polished in M5" mandate, pulled forward on her 2026-08-01 request;
  consumes spec-0015's read half — snapshot / blocking-chart / manifest)
- **Name**: the owner has named this component **the map editor** (地图编辑器).
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
- `plant` — structural flora via the canopy rules (#121 lean-or-grow).
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

## Acceptance (M3-island polish is the first real workload)

1. **De-wall the greenfield corridor**: the 3-high bounding berm replaced
   by natural boundary treatment (slope/treeline/outcrop) via edit script
   only — no generator code change; full ladder + invariants green.
2. **Island exterior polish**: mountain silhouette + shoreline reworked
   through the editor, reviewed via the snapshot loop, owner-accepted.
3. Determinism proof over the edit stage (byte-identical double replay).
4. Editor session transcript (edit script + snapshots) demonstrates the
   closed loop end-to-end without any hand-authored blocks.

## Non-goals

L1 siting (M6); free-form per-voxel painting; runtime (in-game) editing;
any editor mutation outside the script.
