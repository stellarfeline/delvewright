# `delvewright_dsl::timed_gate`

The reference page for `crates/dsl/src/timed_gate.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW01xx — validation (`dsl`; severity error; exit 1)

This module's rows of a section whose prose is on the [`delvewright_dsl::diagnostic` page](diagnostic.md#dw01xx--validation-dsl-severity-error-exit-1).

| Code | Meaning |
|------|---------|
| `DW0377` | A `timed-gate` declaration (spec-0016 §4) is structurally invalid: a malformed or duplicate `timed-gate/<id>`, an `open_ticks` or `closed_ticks` of 0 (a gate that never opens, or never closes — that is `open-gate`/`close-gate`, not a clock), a `phase` at or beyond the full cycle, two timed gates driving one region (two clocks race every tick and the region's state becomes emission order, not design), a gate a `shortcut` already owns (a clock would re-seal what `DW0372` exists to forbid re-sealing), or a `disarm.via` anchor no area's prefab provides / one that IS the gate anchor (the jam lever cannot stand inside the span the portcullis closes on). Validation-tier (exit 1), `dsl::timed_gate`. |
| `DW0389` | A `close-gate` effect targets the gate of a `timed-gate` that declares a `disarm` (`docs/notes/souls-design-language.md` §5.2). A disarm suppresses the clock **permanently with the gate resting OPEN** — a jammed portcullis stays up — so, exactly as for a `shortcut` (`DW0372`), permanence is structural rather than left to authoring discipline: there is no way to spell the re-arm. The scan descends nested effect lists, so a `close-gate` buried in a `sequence` step is caught. A `close-gate` on a timed gate with **no** `disarm` is untouched — that clock is still a clock and the point-of-no-return beat may seal it. Validation-tier (exit 1), `dsl::timed_gate`. |
