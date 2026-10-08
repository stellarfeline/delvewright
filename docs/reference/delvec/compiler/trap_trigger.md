# `delvec::compiler::trap_trigger`

The reference page for `crates/delvec/src/compiler/trap_trigger.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW043x — geometry & container proofs (stair orientation; spec-0021 loot; `collect` container adoption)

This module's rows of a section whose prose is on the [`delvewright_dsl::loot` page](../../dsl/loot.md#dw043x--geometry--container-proofs-stair-orientation-spec-0021-loot-collect-container-adoption).

| Code | Meaning |
|------|---------|
| `DW0917` | **A trap whose trigger cell does not hold its trigger block.** A `traps[]` entry's `trigger` names a kind of hardware — `pressure-plate` (any `minecraft:*_pressure_plate`), `tripwire` (`minecraft:tripwire`, the string, not the hook), `trapped-chest` (`minecraft:trapped_chest`) — and the cell of its `at` anchor in the assembled world (the edited one when a stage-7 script exists) holds something else. Build-tier (exit 3), `compiler::trap_trigger`, run in the same pass and over the same block map as `DW0431`/`DW0438`. The prefab places a trigger, the compiler only detects it: a plate or tripwire by a position test on the cell, a trapped chest by an invisible `minecraft:interaction` hitbox. Neither needs the block, so without this check a trap over empty air compiles, passes its completability proofs and fires on a cell that shows the player nothing — a false chest that is empty air. The gap runs toward shipping, never toward a red. The message names each trap, its cell, the block found, and the anchors of this world whose cell already holds the right block. A trap inside a gate region is refused too: the assembled model clears the region, as the running game does when the gate opens. Prescription: point `at` at an anchor whose cell holds the trigger block, or have the piece place it (a library change); never a runtime `set-block`. Gallery probe: `a-chest-that-is-not-there`. **The same rule for a `step` trigger** (`trap_trigger::check_step_triggers`, same pass): its anchor's cell must hold a block a step fires (`dsl::fires_on_step` — a pressure plate or the tripwire string; the hook is not one), because its detection is a player in the cell and needs no block either; a step over bare floor is refused, naming the trigger, the cell, the block found and the anchors that hold a plate. |
