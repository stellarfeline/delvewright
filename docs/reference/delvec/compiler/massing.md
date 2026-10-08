# `delvec::compiler::massing`

The reference page for `crates/delvec/src/compiler/massing.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW03xx — build / solver / nav (`compiler`; error; exit 3, `stage:"build"`)

This module's rows of a section whose prose is on the [`delvec::compiler::nav` page](nav.md#dw03xx--build--solver--nav-compiler-error-exit-3-stagebuild).

| Code | Meaning |
|------|---------|
| `DW0324` | An L2 massing verb cannot apply to the solved layout (spec-0017): the target area binds a single `prefab` (no jigsaw layout to mass), a `piece` index / `prefab` guard mismatches the placement (layout drift), a `swap-piece`/`reseed-piece` candidate cannot re-mate every mated socket without overlap (or the pool has no compatible variant), an `insert-piece` socket is already mated or nothing attaches without overlap, a `remove-piece` targets the entry piece or a non-leaf, or a `rewire-socket` names an out-of-range connector / seals an already-sealed (opens an already-open) socket. `compiler::massing`, build-tier (exit 3); every message names the batch and prescribes re-inspecting the layout with `delvec snapshot` — never deleting the drift guard or the sockets. |
