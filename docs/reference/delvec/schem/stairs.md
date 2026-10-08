# `delvec::schem::stairs`

The reference page for `crates/delvec/src/schem/stairs.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW07xx — asset, render and authoring tooling (spec-0007; `delvec` subcommands)

This module's rows of a section whose prose is on the [`delvec::admit::diag` page](../admit/diag.md#dw07xx--asset-render-and-authoring-tooling-spec-0007-delvec-subcommands).

| Code | Tool | Meaning |
|------|------|---------|
| `DW0801` | `delvec grammar` / `delvec prefab` | **A stair claims a `shape` the game does not derive at its cell.** A stair's `shape` is not a stored fact: vanilla recomputes it from the stair's own neighbours on every horizontal block update at that cell, so an authored value is a *claim about the four cells around it* — and a wrong claim is corrected by the world the first time anything is placed, broken or flooded beside it. This is the one property that can be right in every tool this project owns and wrong in the game: the render draws what the bytes say, the reviewer approves the picture, the world draws something else. The live instance is a mitred kerb pointed across its run instead of along it, which survives every render and flattens to `straight` in play. The derivation — straight unless a stair of the same `half` sits across the facing axis in front (outer corner) or behind (inner corner), suppressed when the cell beyond the turn already carries a stair of this facing and half, and *any* stair block counts, not the same one — is **measured, not read**: a field of 758 random stairs was placed, settled and read back on the pinned 1.21.11 server (`tools/spike-block-settling/ at 84f364997d24`) and `crates/delvec/tests/schem_stair_shape_measured.rs` replays every cell of it through `delvec::schem::stairs::derive_shape`. A stair that writes no `shape` at all makes no claim here, so nothing can disagree with it. Binding: stairs examined; a piece with none gets no gate and the count stands as a measurement. Emitted as a red `stair-shape` gate (no `.nbt` is written) and as an `audit` error. |
