# `delvewright_dsl::trap`

The reference page for `crates/dsl/src/trap.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW01xx — validation (`dsl`; severity error; exit 1)

This module's rows of a section whose prose is on the [`delvewright_dsl::diagnostic` page](diagnostic.md#dw01xx--validation-dsl-severity-error-exit-1).

| Code | Meaning |
|------|---------|
| `DW0340` | Trap declaration structurally invalid (spec-0011): a malformed/duplicate `trap/<id>`, an `at`/`disarm.via` that no area's prefab provides, or a `disarm.via` that collides with the trap's own trigger anchor. The `at` remedy names what a trap actually needs of a piece — **one point anchor, under any name** — and says that a spec-0022 command `payload` needs nothing beyond that cell, because the compiler emits the detection; the `dispenser` socket belongs to a legacy `dispense` effect and the `trigger_block` to a flag-gated trap (`DW0363`). Sending an author to carve trap hardware for a payload trap sends them to build something no build reads. |
| `DW0341` | A trap `dispense` payload item id is not in the pinned 1.21.11 registry (spec-0011; mirrors `DW0143`). |

### DW044x — command-driven trap payloads (spec-0022)

This module's rows of a section whose prose is on the [`delvec::compiler::nav` page](../delvec/compiler/nav.md#dw044x--command-driven-trap-payloads-spec-0022).

| Code | Meaning |
|------|---------|
| `DW0440` | A trap declares **no consequence at all** — neither the legacy redstone `effect` (spec-0011 `dispense`) nor a spec-0022 command `payload`. A trigger with nothing downstream of it is scenery, but the completability proofs still model its cell as a hazard, so it is a content mistake rather than a deliberate no-op. Validation-tier (exit 1), `dsl::trap`. Prescription: give the trap a `payload` (`volley`, `collapse`, `damage-players`, `play-sound`, `narrate`, `set-flag`, `spawn-wave`, …). |
| `DW0441` | A payload verb's vanilla id is not in the pinned 1.21.11 registry, or is of the wrong kind: a `volley` `projectile` must be an **entity** id, a `collapse` `falling_block` / `then_floor` a **block** id. Validation-tier (exit 1), `dsl::trap`; mirrors `DW0143`/`DW0341`. |
| `DW0443` | A `volley`'s `salvos` (1..=16) or `interval` (1..=200 ticks) is out of range. A volley fires its whole kill zone every salvo, so the entity count is `salvos x standable cells`; past the cap that is a server hazard rather than a trap, and salvos spread wider than the interval cap stop reading as one event. Validation-tier (exit 1), `dsl::trap`. |
