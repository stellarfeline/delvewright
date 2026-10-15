# `delvec::compiler::view::camera`

The reference page for `crates/delvec/src/compiler/view/camera.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../../compiler.md#5-diagnostics-catalog).

## A frame's exposure is the engine's

Every scene is written at `scene::EXPOSURE`, 1.0: the one value at which the sky's radiances are the pinned client's lightmap colours, the tone curve holds the mid-grey where the game's transfer does, and the lantern scale makes a lantern-lit surface read as the game draws it ([`compiler.md` §4](../../../compiler.md)). The camera record (`design/cameras.json`) has no `exposure` field, and neither does a review shot or a panorama.

## Diagnostics

### DW1010 — a camera record states an exposure (`compiler::view::camera`; error; exit 3)

| Code | Meaning |
|------|---------|
| `DW1010` | **A camera in `design/cameras.json` states `exposure`.** Refused by the record's one reader (`camera::parse_sheet`) at any value, the derived 1.0 included, so `delvec cameras`, `delvec place-camera` (exit 2) and `delvec build` (build tier, exit 3, before anything is placed) refuse it alike, naming the first camera that carries the field and the row it answers. Remedy: remove the field. A room that reads too dark at the derived exposure is short of light: light it where it is designed — add or re-arrange its lights until `delvec prefab lighting` calls it lit (the skill's `new-pieces.md`, *Light inside a cave or a body*) — never the picture. There is no override. Proven by the gallery probe `a-crypt-exposed-instead-of-lit` (refused at `delvec build`), `design_record.rs` (the build, by this code and no other) and `view_cli.rs` (`delvec cameras`, exit 2). |
