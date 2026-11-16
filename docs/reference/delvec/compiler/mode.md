# `delvec::compiler::mode`

The reference page for `crates/delvec/src/compiler/mode.rs`: a mode over the path, and the mode binding. The surface is in [`compiler.md`](../../compiler.md) (the stage-5 row `modes[]`); the emission is `emit::mode`.

**A party is never left in a mode: three facts.** The emitted settlement (`mode_<m>_settle`) leaves a mode the moment none of its kept quests is started and not complete, so no mode outlasts what it keeps. The document tier refuses a kept objective that waits on a flag only held quests set (`DW1123`, [`dsl/mode.md`](../../dsl/mode.md)). And the map's mode is replayed along every path the build proves (`plan::walk_modes`), which is this page's code.

**The replay.** An `enter-mode` the path certainly fires at a step (`PathFiring::fires`, dated by its root as a race's lines are) puts the map in the mode from that step's completion; one the path cannot date — a trigger it does not perform, a reaction bundle — enters nothing on that path. The mode is left where the emitted functions leave it: at the entry itself or at the completion of a quest it keeps, when no kept quest is in progress (a quest is started when its trigger is `campaign-start` or the quest its trigger names is complete on the path); at another mode's entry; and at a `leave-mode` the path certainly fires. A `leave-mode` it cannot date leaves nothing. The exported path is replayed in `Plan::build`, and each story world's path in `emit::branch_paths`.

## What the pinned game gives

Read from the pinned server jar (`versions.toml`, `server_jar_sha1`) under Mojang's published server mappings (sha1 `5621e9253f05fd57872bbe7f8ddf5f9a7d525955`), with `javap -p -c -constants`:

- `ScheduleCommand.remove` returns the number of pending calls it removed and throws `ERROR_CANT_REMOVE` when that is none: `schedule clear <fn>` answers 1 for a clock that runs and fails for one that does not.
- `CommandResultCallback.onFailure` is `onResult(false, 0)`, and the scoreboard store of `execute store result` (`ExecuteCommand.lambda$storeValue$32`) writes the result it is handed when it asks for a result: a `schedule clear` that fails stores 0.
- `ScheduleCommand.schedule` throws `ERROR_MACRO` for a function that is a `MacroFunction`: a macro cannot be scheduled. The resumed expiry is therefore scheduled *by* a one-line macro function (`$schedule function <ns>:expire_<obj> $(t)t replace`), and the function it schedules is a plain one.
- **Unsupported claim, named as such:** that a command which throws reaches the store's callback as a failure is Brigadier's `ContextChain.runExecutable`, which was not read from the jar; the `mode_<m>_clock` PackTest asserts the stored count on the pinned server.

## Diagnostics

### DW1124 — a path that performs what a mode holds (`compiler::mode`; spec-0140; error; exit 3)

| Code | Meaning |
|------|---------|
| `DW1124` | **A path the build proves performs an objective while the map is in a mode that does not keep its quest** (spec-0140 §5). Raised at the plan for the exported path and at emission for a story world's, whose message is headed by the world. The objective is held at that step — not told, not completable — so the path stops there. The message names the objective, its quest, the step, the mode and the step the mode is entered at. Prescription: order the mode by the quest graph — the quest it keeps takes a `quest-complete` trigger on what comes before it and what follows waits on its completion — or enter the mode from a bundle of a quest it keeps, add the performed quest to `keeps`, or fire a `leave-mode` before the objective. Not judged: a trigger step that serves a held objective. |

**Binding.** A build that assembles a world and declares a mode prints `mode binding (DW1122–DW1124): M mode(s), E entered by a line; K quest(s) kept and H held over them, C clock(s) stood still; on the exported path S span(s) of a mode, L left before its end, J objective step(s) inside judged for a quest the mode keeps`. A campaign that declares no mode prints no line.
