# `delvewright_dsl::dialogue`

The reference page for `crates/dsl/src/dialogue.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW01xx — validation (`dsl`; severity error; exit 1)

This module's rows of a section whose prose is on the [`delvewright_dsl::diagnostic` page](diagnostic.md#dw01xx--validation-dsl-severity-error-exit-1).

| Code | Meaning |
|------|---------|
| `DW0120` | Dialogue node unreachable from `root`. |
| `DW0121` | Dialogue `root`/`next` references unknown node. |
| `DW0122` | Dialogue effect targets an objective unknown / not `talk-to` / on a different NPC. |
| `DW0123` | A `talk-to` has no reachable completing option in its tree, measured from the stage-6 `root` and every `cast` ledger root at once (static half of `DW0203`). Whether that option is what right-click opens during the beat that needs it is `DW0858`. |
| `DW0191` | A `talk-to` has no **ungated** completing option (all `requires_flags`-gated → deadlock risk). |
