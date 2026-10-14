# `delvec::compiler::approach`

The reference page for `crates/delvec/src/compiler/approach.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW1025 — a body the party talks to faces them (`compiler::approach`; error; exit 3)

A still body keeps the yaw it was given: its summon writes its home anchor's declared `facing` (`watching::npc_home`), and a walk leaves it on the walk's arrival turn (`nav::apply_arrival_yaw`). Neither knows which way the party comes from, so unless the body watches (spec-0101) the yaw it holds when a `talk-to` is owed is the yaw the party walks up to.

Judged per configuration — the exported critical path and every realised branch's own path, each with the legs its route proof walked — and per `talk-to` step on it, after the branch routes. **The approach**: the leg delivering the party to the step (the last leg whose `to_step` is at or before it and whose last cell completes the step), read back from its last cell to the first cell one block or more horizontally from the body's feet; the bearing from the body's feet to that cell is the direction the party arrives from. **The body's yaw at that beat**: the home summon's, when the step's station is the body's home cell, and the arrival yaw of every `move-npc` whose destination mark's own cell is the station (`nav::MovePlan::mark_cell`); each is judged. A yaw within 90° of the approach bearing faces the party (the front half-plane, closed; the threshold is authored, as the least reading of "not with its back to them"). A body watching `nearest` whose `within` reaches the approach cell from every stance is counted as drawn and not judged; a class watch turns only to that class, so it is judged by its yaw. One refusal per body, objective, stance and yaw, at the first configuration that shows it.

Every build that walks prints `approach binding (DW1025): <talks> talk beat(s) over <n> configuration(s): <j> judged by facing (<y> yaw(s), <r> refused), <w> drawn by a watch, <u> reached by no walked leg, <p> with no resolved body`, zeroes included; `u` counts talks a ride or no leg delivers the party to, and `p` talks whose station neither the home cell nor any walk's destination resolves.

| Code | Meaning |
|------|---------|
| `DW1025` | **A body the party talks to has its back to them.** At a `talk-to` step of some configuration, the bearing from the body's feet to the party's last walked cell before the talk is more than 90° off the yaw the body holds at that beat, and no `nearest` watch reaches the approach. Names the body, the objective, the configuration, the body's cell and yaw with where the yaw comes from (the home anchor's facing, or the walk whose arrival turn it is), the approach cell and its bearing, and the angle between them. Prescription: declare `watch: { "who": "nearest", "within": N }` on the npc, `N` the approach distance's ceiling and never less than the interaction reach (3), so the body turns to the party as they arrive. Build tier (exit 3). |
