# `delvewright_dsl::mode`

The reference page for `crates/dsl/src/mode.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

A mode is a state of the map in which every quest but the ones it keeps is held; the surface and its emission are in [`compiler.md`](../compiler.md) (the stage-5 row `modes[]` and the verbs `enter-mode` and `leave-mode`). A reference to a mode or a kept quest nobody declared is `DW0112`.

## Diagnostics

### DW1122 — a mode that cannot be run as declared (`dsl::mode`; spec-0140; error)

| Code | Meaning |
|------|---------|
| `DW1122` | **A mode that cannot be run as declared.** Raised at the field that says it: `keeps` empty (a mode is on while a quest it keeps is in progress, so it would be left the tick it is entered and hold nothing), or a quest named twice in `keeps` (a quest is kept or held). Validation tier (exit 1). |

### DW1123 — a mode that holds what it waits on (`dsl::mode`; spec-0140; error)

| Code | Meaning |
|------|---------|
| `DW1123` | **A kept objective waits on a flag only held quests set** (spec-0140 §5). Asked of every mode some `enter-mode` names, and of every objective of a quest it keeps that may be open while it is on: all of them, less the ones certainly settled before every entry fires — an `enter-mode` in a kept quest's own `on_objective_complete` bundle has that objective and its plain `after` ancestors behind it, and an entry anywhere else settles nothing first. Such an objective that `requires_flags` a flag every setter of which is a `set-flag` in a bundle of a quest the mode holds (`on_objective_complete`, `on_complete`, a limit's `on_expire`) is refused at the flag: once the mode is on nothing can set it, the kept quest never completes and the mode is never left. A flag a trigger, a trap, a shop, a disarm or a dialogue option also sets is free — no mode holds those — and a flag nothing sets is `DW0172`'s. Validation tier (exit 1). Prescription: set the flag from a quest the mode keeps or from a trigger, add the quest that sets it to `keeps`, or drop the gate. Not judged: a `requires_state` term, and an item only a held quest gives. |
