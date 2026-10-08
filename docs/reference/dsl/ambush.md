# `delvewright_dsl::ambush`

The reference page for `crates/dsl/src/ambush.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW01xx — validation (`dsl`; severity error; exit 1)

This module's rows of a section whose prose is on the [`delvewright_dsl::diagnostic` page](diagnostic.md#dw01xx--validation-dsl-severity-error-exit-1).

| Code | Meaning |
|------|---------|
| `DW0375` | An `ambush` declaration (spec-0016 §3) is structurally invalid: a malformed or duplicate `ambush/<id>`, an empty `actors` list (an ambush that springs nothing), or the same actor listed twice — `spawn-actor` is idempotent, so the second one is a silent no-op and the ambush is half the size it reads as. Validation-tier (exit 1), `dsl::ambush`. Deliberately does **not** require a `telegraph`: the un-telegraphed ambush is core souls vocabulary. Everything else about an ambush is checked as the trigger it desugars to (`DW0194`, the anchor seals, `DW0350`). |
