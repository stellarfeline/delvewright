# `delvec::compiler::view::frameset`

The reference page for `crates/delvec/src/compiler/view/frameset.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW07xx — asset, render and authoring tooling (spec-0007; `delvec` subcommands)

This module's rows of a section whose prose is on the [`delvec::admit::diag` page](../../admit/diag.md#dw07xx--asset-render-and-authoring-tooling-spec-0007-delvec-subcommands).

| Code | Tool | Meaning |
|------|------|---------|
| `DW1039` | `delvec frames` | **A frame posts a body the renderer did not draw** (spec-0112 §8.1; a report, never a refusal, exit unchanged). A review frame whose `shows` holds a body (kind `body`, from the placements ledger) of an entity kind the renderer's own statistics for that frame put in `skipped` or `fallback_boxes`, or a body that holds items (`frames::HOLDS_ITEMS`) in a frame whose `items_skipped` is not empty (the renderer counts items by item id, not by holder). Printed once per such body by `delvec frames` — the shot, the body's id, kind, cell, distance, the reason (the item ids the renderer could not draw, or no model in the renderer) and `judge_by` (how to judge it instead: in game, or by the ledger row) — and written into the shot index entry as `undrawn`; the set's binding line counts them (`<u> not drawn by the renderer (DW1039) in <f> frame(s)`), a set with none printing `0 not drawn`. What it protects is a reviewer reading empty floor as an absent keeper, or a bare wall as a missing sign. |
