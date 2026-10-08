# `delvec::compiler::wrongside`

The reference page for `crates/delvec/src/compiler/wrongside.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW03xx — build / solver / nav (`compiler`; error; exit 3, `stage:"build"`)

This module's rows of a section whose prose is on the [`delvec::compiler::nav` page](nav.md#dw03xx--build--solver--nav-compiler-error-exit-3-stagebuild).

| Code | Meaning |
|------|---------|
| `DW0425` | **The compiler cannot tell which side of a `shortcut`'s gate is the sealed one**. A shortcut door's clickable body is placed in the open air on the *sealed* side only, and that placement IS the side test — so the side has to be derivable or nothing may be placed. It is derived from the gate slab's thin axis plus which side of it the `unlock` cell lies on, and it fails when the region has no unique thinnest axis (a cube is not a doorway) or the `unlock` is level with the doorway on that axis rather than beyond it. **It binds to every `shortcut` in the campaign**, not to the ones that declared something: every door gets a clickable body — a door with no answer is still a door a player walks up to and pushes — and every body has to stand on a side, so there is nothing to opt into and nothing to forget. Withhold, never invent: bodies placed on a guess put the author's "this will not open" answer exactly where the door DOES open, and a false player-facing statement is worse than silence — silence teaches nothing, a lie teaches something wrong. `compiler::wrongside::derive` + `emit::check_shortcut_sides`, build-tier (exit 3), raised **before** the route proofs so an undecidable doorway is not reported under `DW0374`'s name. Prescription: put the `unlock` clear of the gate's span on the axis the door is thin on — which is where a far-side bar belongs anyway — or use a gate anchor whose region is a doorway slab rather than a volume. |
