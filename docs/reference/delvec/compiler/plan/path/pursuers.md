# `delvec::compiler::plan::path::pursuers`

The reference page for `crates/delvec/src/compiler/plan/path/pursuers.rs`: the bodies a path's steps are walked beside, and the diagnostics-catalog row of every DW code this module declares. The catalog's shared rules are in [`compiler.md` §5](../../../../compiler.md#5-diagnostics-catalog).

**Dating a lifecycle line.** A line is dated by its root at the path's own step maps (`PathFiring`): an objective's bundle at its step, a quest's `on_complete` at the step of its last objective on the path, a trigger's or a loop's bundle at the step that performs it. A line in a trigger the path does not perform arrives no earlier than the step whose walk first springs it (`waypoints::sprung_steps`: the first leg with a route cell within the `approach` trigger's range of its point, widened by two blocks), or from the first step when no walk is known to spring it; any other line the path cannot date (a trap, a reaction bundle, an `on_kill`, a shop) may fire at any step. A despawn counts only where the path certainly fires it (`PathFiring::fires`).

## Diagnostics

### DW1140 — a despawn before its actor can stand (`compiler::plan::path::pursuers`; error; exit 3)

| Code | Meaning |
|------|---------|
| `DW1140` | **A `despawn-actor` a proven path certainly fires before any line that places its actor can have fired on that path.** The placing lines are every `spawn-actor` and `unleash-actor` of the actor (an `ambush` desugars to both), dated as above; one dated at the despawn's step counts as before it. On that play order the despawn removes nothing, its beat never happens, and a body that arrives later stays. Judged on the exported path and on every story world's, at emission, where the routes date an `approach`. A despawn that tidies a body an optional act may have brought in is legal: any placing line the path cannot date, or that a walk can spring by the despawn's step, may precede it. The message names the actor, the line, the step and the first step a placing line can fire. Prescription: move the despawn to a beat after the actor's arrival, or bring the actor in before it. **Binding.** A build that walks prints `despawn binding (DW1140): D despawn-actor line(s) the exported path certainly fires judged, A with an arrival of their actor able to come before them, R refused`. |
