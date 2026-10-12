# `delvewright_dsl::pulse`

The reference page for `crates/dsl/src/pulse.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0993–DW0995 — a sound beats over a place (`compiler::pulse` / `dsl::pulse`; spec-0102; error + one advisory)

This module's rows of a section whose prose is on the [`delvec::compiler::pulse` page](../delvec/compiler/pulse.md#dw0993dw0995--a-sound-beats-over-a-place-compilerpulse--dslpulse-spec-0102-error--one-advisory).

| Code | Meaning |
|------|---------|
| `DW0993` | **A pulse declared against itself.** An `every` of 0 (`schedule … 0t` is the same tick again, forever); a `floor` outside `[0, 1)` (at 1 the reach is infinite); a `pitch` outside the `[0, 2]` the pinned `playsound` takes; `when: {}` (a stage with no term); a `requires_state` term on a `player`-scoped datum (a pulse addresses every player in its place, so its gate is a party fact — raised beside `DW0503` in `dsl::state`, the code chosen by the consumer); a duplicate id. Validation tier (exit 1), `dsl::pulse`. Prescription: the value inside its range; leave `when` out or name a flag or a `party`-scoped datum; a distinct id. |
