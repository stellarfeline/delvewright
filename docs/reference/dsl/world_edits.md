# `delvewright_dsl::world_edits`

The reference page for `crates/dsl/src/world_edits.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW01xx — validation (`dsl`; severity error; exit 1)

This module's rows of a section whose prose is on the [`delvewright_dsl::diagnostic` page](diagnostic.md#dw01xx--validation-dsl-severity-error-exit-1).

| Code | Meaning |
|------|---------|
| `DW0162` | Stage-7 edit script structurally invalid (spec-0017): an edit names a region no earlier `select` in its batch defined (region refs are strictly backward within a batch), a `union`/`intersect` lists < 2 regions / a `subtract` removes nothing, a box `min` > `max` on an axis, a surface band `from` > `to`, a palette recipe is empty or carries a non-positive/non-finite `weight`/`scale`, a `matching` list is empty, or a morph `by`/`passes` is 0. (Unknown recipe/matching block ids reuse `DW0193`; id syntax `DW0110`; duplicate batch/region names `DW0111`.) |

### DW1094 — a batch and its beat (spec-0134 §6; severity error; exit 1)

| Code | Meaning |
|------|---------|
| `DW1094` | **A batch and its beat disagree** (spec-0134 §6). Validation tier (exit 1), `world_edits::apply_edits_checks`, over every `apply-edits` at every effect root and nesting depth: a `batches[]` entry declaring `applied: "by-effect"` that no `apply-edits` names (raised at its `applied` — declared and never written into the world), or an `apply-edits` naming a batch applied at build (raised at its `batch` — a second write of cells the build already holds). An `apply-edits` of an undeclared batch is `DW0112`. Prescription: add the `apply-edits` to the beat that applies the batch, or remove `applied`; or declare the batch `by-effect`, or remove the `apply-edits`. |
