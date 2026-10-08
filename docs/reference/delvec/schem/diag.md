# `delvec::schem::diag`

The reference page for `crates/delvec/src/schem/diag.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW07xx — asset, render and authoring tooling (spec-0007; `delvec` subcommands)

This module's rows of a section whose prose is on the [`delvec::admit::diag` page](../admit/diag.md#dw07xx--asset-render-and-authoring-tooling-spec-0007-delvec-subcommands).

| Code | Tool | Meaning |
|------|------|---------|
| `DW0700` | `delvec schem` | Strip hook: a forbidden block/entity was removed. |
| `DW0701` | `delvec schem` | Oversize schematic tiled into structure parts. |
| `DW0702` | `delvec schem` | Source `DataVersion` ≠ pinned MC 1.21.11. |
| `DW0710` | `delvec schem` | Input unreadable / not a Sponge schematic. |
