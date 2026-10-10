# `delvec::compiler::pulse`

The reference page for `crates/delvec/src/compiler/pulse.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0993–DW0995 — a sound beats over a place (`compiler::pulse` / `dsl::pulse`; spec-0102; error + one advisory)

A pulse (`pulses[]`, stage 5) is a vanilla sound event sounding from a mark on a fixed interval to every player standing in a place, while its gate holds. Its reach is derived from the declared `floor` over the standable cells of its box (`compiler.md`, the `pulses[]` row); the neither/both of `region` / `place` is `DW0929`, an unknown sound `DW0326`, a source outside its piece `DW0897` — no second code for a rule that exists.

| Code | Meaning |
|------|---------|
| `DW0994` | **A pulse nobody can stand in hearing of.** The box the pulse is heard in (its `region`, or the bounds of the `place` it names) holds no standable cell — the cells every walk proof stands a body on — so its farthest ear does not exist, its reach cannot be derived from `floor`, and the binding would be a zero. Also raised for a pulse in a campaign that assembles no world. Build tier (exit 3), `compiler::pulse::measure`, over the assembled world. The message names the box and the nearest standable cell to its centre. Prescription: the box — move or resize the `region`, or name the `place` the party walks. |
| `DW0995` | **A pulse the forced route never hears** (advisory). The forced route's proven legs pass no arrival at which the pulse is live while a route cell lies in its box: the ladder cannot exercise it and the bot reports it `not_heard: no station`. The message names the pulse and why — no route cell lies in its box, or the term of its gate that never holds (`lethal::never_held_term`, the reading `DW0954` takes). The shape of `DW0954`. Warning tier; none required: a beat the party need never hear is a design. |
