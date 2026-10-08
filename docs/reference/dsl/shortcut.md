# `delvewright_dsl::shortcut`

The reference page for `crates/dsl/src/shortcut.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW01xx — validation (`dsl`; severity error; exit 1)

This module's rows of a section whose prose is on the [`delvewright_dsl::diagnostic` page](diagnostic.md#dw01xx--validation-dsl-severity-error-exit-1).

| Code | Meaning |
|------|---------|
| `DW0371` | A `shortcut` declaration (spec-0016 §2) does not resolve: a malformed or duplicate `shortcut/<id>`, a `gate`/`unlock` anchor no area's prefab provides, or an `unlock` equal to its own `gate` — the mechanism belongs on the far side of the door it opens, which is the entire point of the pattern. Validation-tier (exit 1), `dsl::shortcut`; anchor resolution stays lenient for pool areas the compiler resolves later, and for a site-plan campaign whose `layout-graph.json` is absent (`DW0824`), where the derived vocabulary is unknowable rather than empty. The id and self-gate arms are unaffected by either — they judge what the declaration says. |
| `DW0372` | A `close-gate` effect targets a gate a `shortcut` owns (spec-0016 §2). A shortcut opens **permanently** — that is the pattern — so permanence is made structural rather than left to authoring discipline: there is simply no way to spell the re-seal. The scan descends nested effect lists, so a `close-gate` buried in a `sequence` step is caught. `close-gate` on any other gate (the point-of-no-return staging beat) is untouched. Validation-tier (exit 1), `dsl::shortcut`. |
