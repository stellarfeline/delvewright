# `delvec::compiler::nav::race`

The reference page for `crates/delvec/src/compiler/nav/race.rs`: a race's course over the assembled world. The surface is in [`compiler.md`](../../../compiler.md) (the stage-5 row `races[]`); the practice it follows is in [`race-checkpoints.md`](../../../race-checkpoints.md).

**One question, one answer.** A checkpoint is the box `at ± extent` (`checkpoint_box`), and a body whose feet are in a cell meets it when the cell's column is inside the box and the box reaches the feet cell or the one above (`cell_in_box`). The emitted selector's box, the ridden leg (`nav::vehicle::ride_of`'s `course`) and a `move-actor`'s walk `along` the race (`nav::actor`) all read these two.

**A ridden course.** A leg whose objective declares `course` (`Plan::declared_courses`) is one ride: from the seat the medium is searched to the nearest cell of the first checkpoint, from there to the nearest cell of the second, and so on for every lap; the ride proper — its act from the seat, or its leaving cell — is planned on from the last. A checkpoint the medium does not reach from the one before is the leg's `DW0311`, in the words *the leg runs a race's course*. The exported `ride` step's `medium` is the whole way, which the bot drives cell by cell.

**A walked course.** A `move-actor` with `along` is routed through `World::course_cell` of each checkpoint — the cell its footprint stands in nearest the box's centre — and its rendered line is smoothed between those cells and never across one. A stretch with no walk is `DW0325`, naming the checkpoint and the lap.

## Diagnostics

### DW1118 — a checkpoint nobody can pass on its own (`compiler::nav::race`; spec-0139; error; exit 3)

| Code | Meaning |
|------|---------|
| `DW1118` | **A checkpoint no body can be in, or two in a row that share a cell** (spec-0139 §6). Build-tier (exit 3), asked over the assembled world of every checkpoint of every race. A box that holds no cell a player stands in and no cell of the water's surface is refused: it is in a wall, over a drop or above the ground. Two checkpoints that follow one another — and, in a race of several laps, the last and the first — whose boxes share a cell are refused: a body there is in both, and is credited the second where it was credited the first. The message names the race, the checkpoint by its place in the course, its mark and its box. The first finding is the build's; the rest print beside it. Prescription: centre the checkpoint on the course's floor, or move or shrink one of the two until the boxes are apart. Not judged: whether a racer can cut from one checkpoint to the next by a shorter way than the author meant — where the checkpoints stand is the course's design. |

### DW1119 — a course whose race is not running (`compiler::nav::race`; spec-0139; error; exit 3)

| Code | Meaning |
|------|---------|
| `DW1119` | **An objective's `course` names a race that is not running when the path reaches it** (spec-0139 §4). Raised at the plan (`plan::path::check_courses_started`), dated as a ridden vehicle's lines are: of the `start-race` and `end-race` lines of that race the path certainly fires before the objective's step, none is a start, or the last is an end. A `start-race` the path cannot date — a trigger's, a dialogue option's — may fire at any step and starts the race for every step; an `end-race` it cannot date ends nothing. The message names the objective, the race and the step. Prescription: fire a `start-race` at a beat before the objective, or drop its `course`. |

**Binding.** A build that assembles a world and declares a race prints `race binding (DW1117–DW1119): R race(s), C checkpoint(s) judged for a cell a body stands in or floats on and for sharing none with the next, X refused; A actor racer(s); O objective(s) run a course; W walk(s) follow one` before the verdict is taken. A campaign that declares no race prints no line.
