# `delvewright_dsl::race`

The reference page for `crates/dsl/src/race.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW1117 — a race that cannot be run as declared (`dsl::race`; spec-0139; error)

A race is a course of checkpoints a racer is credited in order; the surface and its emission are in [`compiler.md`](../compiler.md) (the stage-5 row `races[]`, the verbs `start-race` and `end-race`, the objective's `course` and `move-actor`'s `along`). A reference to a race, a racer or a board nobody declared is `DW0112`; a checkpoint's anchor no prefab provides is `DW0142`.

| Code | Meaning |
|------|---------|
| `DW1117` | **A race that cannot be run as declared.** Raised at the field that says it: `checkpoints` empty (a race is a course, and its last checkpoint is its finish line); `laps` of 0; `grace_ticks` of 0 (the race would be over in the tick it is won); several laps round one checkpoint (the checkpoint after it is the one the racer stands in); an actor listed twice in `racers`, or in two races (a `move-actor` walks it round one course); an actor with no `name` racing for a place on a `board` (its row would read as an id); a `move-actor` `along` a race that does not list the actor (it would pass every checkpoint and be credited none); and an objective that declares `course` and no `ride` (the build proves a course ridden, and proves none walked on foot — the race still credits a racer on foot). Validation tier (exit 1). |
