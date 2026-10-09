# `delvec::compiler::skinparts`

The reference page for `crates/delvec/src/compiler/skinparts.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0978/DW0979/DW0980 — a sheet is drawn to its own model's boxes (`compiler::skinparts` + `dsl::npc`; error)



| Code | Meaning |
|---|---|
| `DW0978` | **A sheet paints a pixel no box of its model samples.** Any pixel of non-zero alpha off the footprint — a template's leftover corner, or a sheet drawn to another model's layout (The Stranding's first `drowned_outer_layer`, painted at the player's jacket and pants positions on a model that samples the base positions). The message names the model, the count, the first pixels, every box the model builds, and every other model of the same size whose boxes hold all the stray paint. Prescription: clear those pixels, or draw them onto a box the model has (`python -m delve_skin parts <model>` prints any model's boxes). |
| `DW0979` | **Every opaque pixel lands on a face a body standing level with the model cannot see.** A face is unseen when it is the top of an unposed box in the `head` or `body` subtree above a standing player's eye (`EntityType.PLAYER`'s eye height, 25.92 model pixels, read into the table) or the underside of one below it; a limb or a posed box is never unseen, and the judgement is at the model's own scale. The shape is a sheet that paints only the crown of a grown hat. Prescription: paint the faces that face the player. |
