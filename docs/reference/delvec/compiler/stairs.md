# `delvec::compiler::stairs`

The reference page for `crates/delvec/src/compiler/stairs.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW043x — geometry & container proofs (stair orientation; spec-0021 loot; `collect` container adoption)

This module's rows of a section whose prose is on the [`delvewright_dsl::loot` page](../../dsl/loot.md#dw043x--geometry--container-proofs-stair-orientation-spec-0021-loot-collect-container-adoption).

| Code | Meaning |
|------|---------|
| `DW0430` | A stair block on a **proven route** whose `facing` contradicts the climb it carries. Build-tier (exit 3), `compiler::stairs`. A vanilla stair's full-height half sits on its `facing` side (verified against the 1.21.11 collision shapes: `facing=north` puts the upper box at `z ∈ [0.0, 0.5]`, the north half), so `facing` **is** the direction you ascend. Nav models a stair as a full cube (`collision_top_16` returns 16), which means a reversed stair reads as a legal one-block *jump* and every other proof passes — the delve ships green with a staircase the player must hop up tread by tread. Scope is deliberately narrow to stay free of false positives: only stairs that are the floor of the **higher** cell of a ±1-elevation step on a proven route are inspected, then widened laterally across the run's width, each lateral cell gated on its own approach-side riser test (so a spiral's turn cannot bleed into the flight at right angles to it). Keying on the higher cell is what makes a turning staircase safe — the tread you arrived on still legitimately points the old way. Decoration is never inspected: a stepped gable, a corbel or a chair has no climb semantics. The message groups defects **per prefab piece** (the fix list — one wrong literal in a generator produces a whole run) and then names individual cells. Prescription: fix the piece that authors the blocks and re-export its `.nbt`. Do NOT reroute the critical path around the staircase and do NOT widen the nav step rule — the route is correct, the geometry is not. |
