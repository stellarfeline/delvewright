# `delvewright_dsl::perception`

The reference page for `crates/dsl/src/perception.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW050x — runtime state (`dsl::state`; spec-0031)

This module's rows of a section whose prose is on the [`delvewright_dsl::state` page](state.md#dw050x--runtime-state-dslstate-spec-0031).

| Code | Meaning |
|------|---------|
| `DW0941` | **A particle the game does not draw from a bare id** (spec-0085 §4.3). A `particle` effect names an id the pinned registry (`crates/dsl/data/particles-1.21.11.json`, 115 types) does not hold, or one of the 18 whose type takes options (`dust`, `block`, `item`, `flash`, …) — the verb carries no options, so the game would refuse the command. Validation-tier (exit 1), `dsl::validate`. Prescription: a registered id a bare name spawns. |
| `DW0942` | **An audience on a party fact** (spec-0085 §3.3). An effect states the envelope's `audience` or `in` on a verb the emitter fires once for the world (`Verb::addresses_players` is false — a flag, a gate, a block, a region, a wave, an actor, an NPC, a camera, the time, a checkpoint, a bonfire, stealth, a timeline, a teleport, a volley, a collapse, a rocket, a state write). A box has no party and a world fact has no audience; a `sequence`'s steps each state their own. Validation-tier (exit 1), `dsl::validate`, naming the verb and the field. |
| `DW0944` | **A sight effect that ends under a camera** (spec-0085 §5.3). In one timeline, a `give-effect` of a sight effect (`dsl::perception::SIGHT`: `night_vision`, wind-down 200 ticks; `blindness` and `darkness`, 20 ticks authored from memory) whose window `[at_ticks, at_ticks + 20 × seconds)` overlaps a `cutscene` step's and ends at or after its start and before its end plus the wind-down — so it starts ramping down on screen. The message names the grant, the shot, the tick the grant ends and the `seconds` that clears it. Validation-tier (exit 1), `dsl::validate`. The `give-effect` half of the findings-ledger row whose general form is that a granted sight effect outlasts any authored camera it can overlap. |
