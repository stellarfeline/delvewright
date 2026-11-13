# `delvec::compiler::view::frameset`

The reference page for `crates/delvec/src/compiler/view/frameset.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW07xx — asset, render and authoring tooling (spec-0007; `delvec` subcommands)

This module's rows of a section whose prose is on the [`delvec::admit::diag` page](../../admit/diag.md#dw07xx--asset-render-and-authoring-tooling-spec-0007-delvec-subcommands).

| Code | Tool | Meaning |
|------|------|---------|
| `DW1039` | `delvec frames` | **A frame posts a body the renderer did not draw** (spec-0112 §8.1; a report, never a refusal, exit unchanged). A review frame whose `shows` holds a body (kind `body`, from the placements ledger) of an entity kind the renderer's own statistics for that frame put in `skipped` or `fallback_boxes`, or a body that holds items (`frames::HOLDS_ITEMS`) in a frame whose `items_skipped` is not empty (the renderer counts items by item id, not by holder). Printed once per such body by `delvec frames` — the shot, the body's id, kind, cell, distance, the reason (the item ids the renderer could not draw, or no model in the renderer) and `judge_by` (how to judge it instead: in game, or by the ledger row) — and written into the shot index entry as `undrawn`; the set's binding line counts them (`<u> not drawn by the renderer (DW1039) in <f> frame(s)`), a set with none printing `0 not drawn`. What it protects is a reviewer reading empty floor as an absent keeper, or a bare wall as a missing sign. |
| `DW1100` | `delvec cameras` | **A showcase picture frames a body the renderer draws nothing of.** A body of the world a showcase camera stands in whose kind the renderer's own statistics for that frame put among the kinds it skipped (no model), standing inside the frame (its feet or its head projects inside the frame), nearer than the frame's fog and in sight of the lens over the world's blocks (`frameset::undrawn_in_frame`). Printed once per such frame — the camera, each body's kind, tag and position, and the remedies (move the camera so none stands in frame, give the body a kind the renderer draws, or wait for a renderer that draws it). A path-traced frame (`--hq`, `--draft`), the picture an author approves, is not written, and the run exits 5 once every frame is drawn; a quick frame (no flag) prints the same line and is written, so a camera can be moved while it is placed. The run's binding line counts them (`undrawn: <u> of <n> frame(s) frame a body the renderer draws nothing of (DW1100)`). What it protects is an approved picture of the world without a body the world holds. |
