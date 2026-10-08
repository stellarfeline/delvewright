# `delvec::compiler::view::sight`

The reference page for `crates/delvec/src/compiler/view/sight.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW07xx — asset, render and authoring tooling (spec-0007; `delvec` subcommands)

This module's rows of a section whose prose is on the [`delvec::admit::diag` page](../../admit/diag.md#dw07xx--asset-render-and-authoring-tooling-spec-0007-delvec-subcommands).

| Code | Tool | Meaning |
|------|------|---------|
| `DW0893` | `delvec viewer` / `delvec render piece` / `delvec render batch` | **An eye-level frame is blind** (warning; `compiler::view::sight`): more than half of it is a surface turned toward the camera within arm's reach. Measured off the piece's own bytes by casting 32×32 rays through the frame's pixel centres with the draft renderer's voxel walk and counting first hits on a face whose normal is within 45° of the view axis, no further than 4.5 blocks (the vanilla default of `minecraft:block_interaction_range`); a ray starting inside a block counts. Distance alone is not the rule — floor, ceiling and a corridor's side walls are near in every low room and recede — and the centre ray alone is not either: a parapet at chest height fills the frame over a long clearance. The walk counts every placed block as a full cube, so a pane or a fence within reach reads as a surface; that error can only call a cluttered frame blind. A **report and never a refusal**: a surface can be the subject of an eye-level shot (an altar, a hearth, a hanging), and the engine cannot tell pressed-against from looking-at. The message names the room camera for the same anchor — the same facing, stood back to the far side of the space (`room-<anchor>` in `render piece`, `room:<anchor>` in `viewer`) — with its fraction, or says standing back does not clear it and why the walk stopped. Measured over `render piece`'s `eye-*`, `room-*` and `stand=` view shots, and over `viewer`'s `pov:`/`room:` presets; every run prints a `sight:` binding line with its zeroes, and `<stem>-shots.json` carries a `sight` block per eye-level shot and for the run. |
