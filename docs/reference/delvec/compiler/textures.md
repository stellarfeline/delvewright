# `delvec::compiler::textures`

The reference page for `crates/delvec/src/compiler/textures.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW03xx — build / solver / nav (`compiler`; error; exit 3, `stage:"build"`)

This module's rows of a section whose prose is on the [`delvec::compiler::nav` page](nav.md#dw03xx--build--solver--nav-compiler-error-exit-3-stagebuild).

| Code | Meaning |
|------|---------|
| `DW0309` | An image a campaign declares has no file (spec-0084 §6.4): a staged **body** — a stage-2 npc or a stage-5 actor alike — declares `skin.texture_id` but the campaign ships no `skins/<id>.png` to bake (build-tier), or a `world.textures[]` row has no `textures/<id>.png` (raised by `compiler::textures::resolve` at `delvec validate`, exit 1, naming `world.textures[n]` and the path). Declared once, `compiler::textures::DW_IMAGE_MISSING`. The message names the declaring body, its stage and its JSON pointer. Enumerated from `dsl::body_skin_sites`, a filter over `dsl::body_sites` (`BodyRef`'s closed set), so both classes are baked and both refused by the same rule; a skin declared by a class outside that set is red in `crates/dsl/tests/body_skin_sites.rs`, which takes the population from the schema export. One texture is read once however many bodies name it. |
