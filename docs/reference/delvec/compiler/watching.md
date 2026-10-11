# `delvec::compiler::watching`

The reference page for `crates/delvec/src/compiler/watching.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW099x — a body watches (spec-0101; `dsl::validate` + `compiler::watching`; error)

This module's rows of a section whose prose is on the [`delvewright_dsl::body` page](../../dsl/body.md#dw099x--a-body-watches-spec-0101-dslvalidate--compilerwatching-error).

| Code | Meaning |
|------|---------|
| `DW0997` | **A watch nobody can draw** (spec-0101 §5.2). No cell of the walked population `P` (`lethal::walked_population`, the population `DW0938` reads) has its standing point within the body's `within` blocks of its feet, so no player can ever stand where the body would turn to them: the declaration is inert, and an inert declaration is refused, not ignored. Build tier (exit 3), judged after the declaration proofs, so a body whose mark leaves its piece is sent to `DW0897` first. Names the body, the declaration's pointer, `within`, the body's cell, and the nearest walked cell with its distance — whose ceiling is the `within` that passes. **Binding**: every build prints `watch binding: W watcher(s) declared, over P walked cell(s), D drawable, R refused, U unobservable`, zeroes included; `U` counts drawable watchers no drawable cell outside a killing volume turns by 10° or more, which get no generated turn template. `validation/watchers.json` carries the same counts and one row per watcher. Prescription: raise `within`, or move the body to where the party walks. |
