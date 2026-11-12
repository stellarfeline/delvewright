# `delvec::compiler::recording`

The reference page for `crates/delvec/src/compiler/recording.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW1104 — a way no story reaches (`compiler::recording`; spec-0137; error; exit 3)

| Code | Meaning |
|------|---------|
| `DW1104` | **A `record-outcome` whose line no story world can show** (spec-0137 §8). In every coherent story world `compiler::flow` enumerates (expiry witnesses included), either the site cannot fire or the objective it names never settles (done or expired). A site can fire in a world when the world's walk fires it, when it stands at an ambient root the walk drives out of order (a forced act's bundle, a death, a crossing, a landing, a respawn), when it is the bundle of an act the world performs whose `when` flags the world holds, or when it is a dialogue option whose `requires_flags` the world holds. Build-tier (exit 3), bound in the validation funnel every subcommand runs. The message names the objective and counts the worlds that fire the site and the worlds that settle the objective. Remedy: put the line on the event that decides that way, or make that event reachable. It does not ask that every way have a line. Asserted by `crates/delvec/tests/it/journal_records.rs::dw1104_a_way_no_story_reaches` and the gallery probe `a-way-the-story-never-takes`. |

**Binding.** A campaign that records an objective prints `records binding: O recorded objective(s), S record-outcome site(s) judged over W story world(s), R refused (DW1104)` with its validation. A campaign that records nothing prints no line.
