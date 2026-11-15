# `delvec::compiler::loss`

The reference page for `crates/delvec/src/compiler/loss.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW1112 — a loss no story world dates (`compiler::loss`; spec-0133; error; exit 3)

| Code | Meaning |
|------|---------|
| `DW1112` | **A `fail-objective` in a root no story world dates** (spec-0133 §1 as amended). The story worlds date a loss in a quest bundle (an objective's completion, a quest's completion, a limit's `on_expire`) at its step, and a loss in an elective ambient act's bundle by making the act a failure world ([`flow.md`](flow.md), *Losses*). A loss anywhere else — in the bundle of an act every world performs wherever its gate holds (a fight a mandatory `kill` slays, a trigger the path is forced through), or in a root no walk fires at a step (the campaign's `on_death`, a checkpoint's `on_respawn`, a loop's `on_cross`, a strike's `on_land`) — would settle a task at runtime that every proof holds open. Build-tier (exit 3), bound in the validation funnel every subcommand runs. The message names the root's kind. Remedy: put the loss on an event a walk performs — an objective's completion, or an act a player may choose (a credited kill of a fight no `kill` requires, a trigger off the path). Asserted by `crates/delvec/tests/it/fail_objective.rs::dw1112_a_failure_no_world_dates` and the gallery probe `a-loss-at-a-death`. |

**Binding.** A campaign with a `fail-objective` prints `failure binding (DW1112): S fail-objective site(s) — D in a quest bundle, E in A elective act(s), each a failure world — R refused` with its validation. A campaign with none prints no line.
