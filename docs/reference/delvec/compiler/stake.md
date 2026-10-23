# `delvec::compiler::stake`

The reference page for `crates/delvec/src/compiler/stake.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0520–DW0527 — trade and the recovery stake (`dsl::validate` / `compiler::stake`; spec-0032)

This module's rows of a section whose prose is on the [`delvewright_dsl::economy` page](../../dsl/economy.md#dw0520dw0527--trade-and-the-recovery-stake-dslvalidate--compilerstake-spec-0032).

| Code | Meaning |
|------|---------|
| `DW0525` | **No walkable route back.** From some respawn seat, under every quest state that can hold while it is in force, there is no reachable cell a stake could stand on for deaths in some region — or there are cells a player can walk to and die on **while that seat is in force** that the seat cannot reach at all (the one-way drop). Those cells are the walk regions of the areas the party stands in while the seat holds (`stake::death_cells_per_seat`): a seat holds from the step it can be in force until a checkpoint a beat sets replaces it (the step that sets it counts unless the carry onto it completes it on landing), and a bonfire or a trigger's checkpoint holds again wherever the route returns to its own area; an area's walk region is the entry spawn's for the area the delve starts in and, for an area the route is carried into, the walk from where the crossing puts it down. A route that stays in one area judges every seat against the entry spawn's walk. What a seat reaches is its walk in each configuration with every shortcut it walks to opened, in rounds (`nav::seat_reach`, `World::in_shortcut_rounds`): a player at the seat who reaches a shortcut's lever pulls it, so the far side of that door counts as somewhere the seat gets back to, although the completability model holds the door shut; the door opens in the configuration's own terms (its cells leave the state's fills). So a shortcut whose lever the seat's side holds is a way back, as the prescription says. Build-tier (exit 3), `compiler::stake`. The message names the death region, the seat and how many quest states were examined. Prescription: give the drop a way back (a shortcut, a ladder), or declare the place a `lethal_volume` so the stake is projected to its near lip instead — never delete the stake to silence it. |
| `DW0526` | **No safe footing.** Every cell reachable from the seat that a stake could be projected onto for some death region stands on a block the runtime removes — a lift car, a sealed gate region, a collapsed floor — so a marker left there would be destroyed by the next ride. Build-tier (exit 3), `compiler::stake`. Distinguished from `DW0525` because the prescription is the opposite: there IS a route back, and the ground it ends on is the problem. |
