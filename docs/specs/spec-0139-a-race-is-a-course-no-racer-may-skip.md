# spec-0139: A race is a course no racer may skip — an ordered row of checkpoints every racer is credited in turn, started by the story, over when its racers are in or its grace has run, proven by riding it

- **Status**: Proposed
- **Ground**: written against `feature/festival-day` at `8645e2429` (`delvec 1.16.1`, `dsl 0.78.0`), read for: `Board`, `place_rows` and `board_rows` in `crates/dsl/src/board.rs` and `emit/board.rs`; `TriggerOn::Step`, `TriggerAudience::Presser` and `TriggerOn::BodyEnters` in `crates/dsl/src/trigger.rs`; `Vehicle`, its `rider` and the `riders` census in `crates/dsl/src/vehicle.rs` and `emit/census.rs`; `Verb::MoveActor` and `plan_actor_moves` in `nav/actor.rs`; `ride_of`, `RideTake` and `RideMiss` in `nav/vehicle.rs`; `ride_leg`, `Carries` and `route_with_links` in `nav/route/leg.rs`; `Step` and `CRITICAL_PATH_FORMAT_VERSION` in `plan/path/mod.rs`; `check_ridden_vehicles_stand` in `plan/path/pursuers.rs`; `pending_guard` and `told_guard` in `emit/quest.rs`; the gate module `crates/dsl/src/gate.rs`; the effect roots in `crates/dsl/src/effects.rs`; `Mark` in `crates/dsl/src/mark.rs` and `DW0897` in `compiler/mark.rs`; the harness's lockstep leg matcher in `harness/src/waypoints.ts`; spec-0039, spec-0066, spec-0135 (§4–§6, §13).
- **What it is for**: a festival town holds a pig race. Written with what the engine had — a start trigger and a finish line of `step` plates that place their presser on a board — a racer reaches the line by any way it likes, and the race is won by cutting across. A race is a mechanism of its own: a course every racer passes in order. This spec gives it a class, says what a racer is, what the player sees when a checkpoint does not count, when a race starts and when it is over, how an actor runs it, and how the build and the bot prove a course can be run in order.
- **Research**: `docs/reference/race-checkpoints.md`. Every rule below is **[cited]** from it by section or from the record named, or **[authored]** here.
- **Numbers**: spec `0139`; `dsl_version` `0.79.0`; `DW1117`, `DW1118` and `DW1119`, three used (§7). `DW1120` was handed and is not used. No `critical-path.json` format number was handed and none is used: no harness step changes (§6).
- **Non-goals**: a pig race (a primitive is a mechanism — a creator racing boats round a bay, minecarts down a line or runners up a tower configures the same class); a race's time or a best lap (§9); the course's fairness (where the checkpoints stand, and whether a shortcut between two of them exists, is the course's design, as a fight's difficulty is); a mode the map is in while a race is run (§9); any campaign's course.

## 1. The class, and why one **[authored, on cited facts]**

**[cited]** (the record §1): established racing games keep, per racer, the index of the checkpoint it must hit next; a checkpoint counts only when it is that one; one further on gives no progress and a warning; one already passed does nothing; a credit shows the racer its count. A rule that accepts a lap on anything but every checkpoint in order is skipped. **[cited]** (the record §2): the pinned game finds a body by a box its hitbox meets, whoever it is and whatever carries it, and holds a score per entity.

**[authored]** So a race is one class, **`races[]`**, and a racer is a body with a count: no player is special, and a racer on foot, on a mount, in a boat or an actor on a pig is found by the same box. It is not an arrangement of triggers: the order is a fact of the course, held per racer, a racer may be a body that is not a player, and a trigger's press is one place. It is not a datum of `state[]`: nothing gates on a racer's count but the race.

## 2. The surface **[authored]**

Stage 5 gains:

```json
"races": [
  {"id": "race/round-the-town",
   "checkpoints": [
     {"at": {"anchor": "anchor/east-wood", "offset": [0, 0, -40]}, "extent": [4, 3, 1]},
     {"at": {"anchor": "anchor/garth"}, "extent": [3, 3, 3]},
     {"at": {"anchor": "anchor/fairground", "offset": [6, 0, 0]}, "extent": [1, 3, 4]}
   ],
   "laps": 1,
   "racers": ["actor/racer-ulla"],
   "board": "board/pig-race",
   "grace_ticks": 600,
   "missed": "You missed a checkpoint — go back.",
   "passed": "Checkpoint"}
]
```

- `id` is `race/<kebab>` (`DW0110`/`DW0111`).
- `checkpoints` are the course, in the order a racer passes them; the last is the finish line. Each is the box `at ± extent`, `at` a mark (spec-0066). A mark and not a bare anchor because a course runs where the story's places do not stand, and its centre is said from the nearest anchor; a checkpoint places no body, so — as a camera's and a sound's marks are — it is outside `DW0897` and its offset may leave its anchor's piece. One checkpoint or more (`DW1117`).
- `laps` (absent: 1) runs the course again: after the last checkpoint the next is the first. Several laps need two checkpoints or more (`DW1117`).
- `racers` lists the actors that race beside the players. Every player may race: a player or listed actor is a racer once it has started — been credited the course's first checkpoint.
- `board` names the board (spec-0135 §13) each finisher takes the next place on. The quantifier is the race's — every finisher, whatever its kind — which no effect's `audience` says, so it is the race's field and not a `place-on-board` in a bundle. A listed actor then carries a `name` (`DW1117`).
- `grace_ticks`: how long the race stays open after its first finisher (§3).
- `missed` is the line a player reads while it stands in a checkpoint further on than its next; `passed` (optional) the line read at a credit, followed by the count and the total. Both are inventoried for translation (`race.<id>.missed`, `race.<id>.passed`).
- Two verbs: **`start-race {race}`** and **`end-race {race}`** (§3). Both are the race's and take any bundle.
- **An objective of any kind may carry `course: "race/<id>"`** beside `ride` (§4). The declaration lives where `ride` does and for its reason: a leg is the way to an objective.
- **`move-actor` gains `along: "race/<id>"`** (§5).

## 3. A racer's count, the start and the end **[authored, on cited primitives]**

**The count.** A racer holds two scores of the race's own: `dw.rc.<race>`, the checkpoints it has been credited, and `dw.rn.<race>`, the index of its next within the lap. While the race runs, the root `tick` runs `race_<race>_body` as every player and as every listed actor. The body function credits **at most one checkpoint a tick and only the racer's next**: for checkpoint `j` it asks whether the next is `j` and the racer's hitbox meets the box (**[cited]** the record §2, `@s[x=…,dx=…]`), and returns with the credit. A racer whose count is the total — the checkpoints, once per lap — has finished and is asked nothing more.

**What a player sees.** **[authored, on the record §1]** A player whose hitbox meets a checkpoint further on in the lap than its next is credited nothing and reads `missed` on the action bar for as long as it stands there. A checkpoint behind it is silent: riding back for the one it missed is what `missed` asks for. A credit shows `passed` and the count (`Checkpoint 2/6`). An actor is shown nothing.

**The start is the story's.** A race credits nobody until a `start-race` fires: every count is reset, and the race runs. The condition a race starts on is therefore whatever fires that line — a talk's option, a countdown's last step, a `body-enters` trigger over a `riders` census (spec-0135 §5), which is the moment the first racer mounts. A race started again starts over.

**The end is the race's.** A race is **over** at the first of three: every racer in the world has finished (asked at each finish: no player and no listed actor that has started — been credited the first checkpoint — is still on the course, so a present member who never starts is not waited on); `grace_ticks` have passed since its first finisher, player or actor (a `schedule` the first finish makes and a start clears — **[cited]** the record §2); or an `end-race` fires. Over, it credits nobody and tells nobody of a miss, what was placed on its board stays, and the latch `#rfin_<race>` holds until it is started again. A race with no `grace_ticks` waits for every racer that has started, or the story; one nobody has started stays open.

**The finish.** A racer credited the last checkpoint of the last lap takes the next place on `board` — a player by the board's own `board_place_<b>`, a listed actor by its `board_row_<b>_<a>` — so the board's rows count the party and every listed actor.

## 4. A race as a task **[authored]**

An objective with `course` says the leg to it runs that race's course. Two things follow.

**It completes only once the race is over.** Every guard the objective completes under gains the race's latch; the guard it is *told* under — its announcement, its journal page, the compass — does not, so the party reads the task before the race is run. The objective's own act is still owed: a `reach-anchor` past the line is walked into, a `talk-to` is the word with the steward afterwards. Its `on_objective_complete` is where the story says how the race ended. *Over*, and not *won by a player*: a race an actor wins while the party is still on the course ends by its grace, and the story goes on — a task that waited for a player's finish would hold a party that gave up in the race for good.

**The leg is proven through every checkpoint, in order** (§6). It is proven ridden, so `course` needs `ride` (`DW1117`); and the race must be running when the path gets there: of the `start-race` and `end-race` lines of that race the path certainly fires before the objective's step, the last is a start — dated as a ridden vehicle's lines are, so a start the path cannot date (a trigger's) starts it for every step — else `DW1119`.

## 5. An actor that races **[authored, on the cited model]**

**[cited]** (spec-0135 §13): an actor sits a vehicle by `rider`, and its `move-actor` walks the vehicle with it aboard along a route the build plans at the stacked body's footprint.

**[authored]** A `move-actor` with `along` walks its actor round the race: through one cell of each checkpoint, in order and for every lap, and on to `to`, where it pulls up. The cell is the one its footprint stands in nearest the box's centre; each stretch is the walk the verb always planned; the rendered line is smoothed between those cells and never across one, so a corner is not cut past its checkpoint. The actor is one of the race's `racers` (`DW1117`), so its body is credited by the same boxes a player's is and takes its place at the line — the engine walks it, and the count is still what places it. A stretch with no walk is the verb's `DW0325`, naming the checkpoint and the lap.

## 6. How the build and the bot prove a course **[authored, on cited facts]**

**[cited]** (spec-0135 §4, §6): a leg an objective declares ridden is one `ride` step whose `medium` is the cells the vehicle is driven through, and the bot reports the vehicle's position along exactly those cells. **[cited]** (`harness/src/waypoints.ts`): a walked leg is replayed from one proven polyline per step, matched in lockstep.

**[authored] A course is proven as the ride it is.** For a leg whose objective declares `course`, the ride is planned in stretches: from the seat the medium is searched to the nearest cell of the first checkpoint, from there to the nearest cell of the second, and so on for every lap; the ride proper — the act from the seat, or the leaving cell — is planned on from the last. The step's `medium` is the whole way. A checkpoint the medium does not reach from the one before it is the leg's `DW0311`, in the words *the leg runs a race's course*. One rule says what *in a checkpoint* means for a cell (`nav::race::cell_in_box`: the column inside the box, and the box reaching the feet cell or the one above), read by the ride, the actor's walk and `DW1118`.

**The bot's verdict is the objective's own marker.** The contract gains no step and its format does not move: the bot rides the `ride` step it always rode, and the step after it passes on the objective's completion marker — which the server withholds until the race is over, and the race is over for a lone bot only when its own count reached the total, that is, when the server credited every checkpoint in order along the ride. A course whose checkpoint the ride misses in the world is a red at that step.

**Proofs over the class** (`DW1118`), of every race whether or not an objective runs it: every checkpoint's box holds a cell a player stands in or a cell of the water's surface; and two checkpoints that follow one another — the last and the first, in a race of several laps — share no cell, because a body in both is credited the second where it was credited the first.

**Classed by what each judges** (CLAUDE.md, *A check guards only what our toolchain owns*): that a credit is given in order and refused out of order is the compiler's output, proven by Rust tests and the PackTests of §8; that this campaign's course can be ridden in order is the content's, judged by the bot; that a selector's box finds a rider at its seat is vanilla's, held by `vehicles-animals-and-targets.md` and no recurring check.

The engine states, on every build that declares a race, `race binding (DW1117–DW1119): <r> race(s), <c> checkpoint(s) judged for a cell a body stands in or floats on and for sharing none with the next, <x> refused; <a> actor racer(s); <o> objective(s) run a course; <w> walk(s) follow one`.

## 7. The rules **[authored]**

| code | module | refuses |
|---|---|---|
| `DW1117` | `dsl::race` | a race that cannot be run as declared: no checkpoint; no lap; a grace of no tick; several laps round one checkpoint; an actor listed twice, or in two races; an unnamed actor racing for a place on a board; a `move-actor` `along` a race that does not list the actor; an objective's `course` with no `ride` |
| `DW1118` | `compiler::nav::race` | a checkpoint whose box holds no cell a body stands in or floats on; two checkpoints in a row that share a cell |
| `DW1119` | `compiler::nav::race` | an objective's `course` whose race is not running when the path reaches its step |

Reused: `DW0110`/`DW0111` (id), `DW0112` (a race, a racer or a board nobody declared), `DW0142` (a checkpoint's anchor), `DW0311` (a ride its medium cannot take round the course), `DW0325` (a walk along a course with no route), `DW0942` (an `audience` on a race verb).

## 8. PackTest and the gallery **[authored, under spec-0039]**

Per race the build emits `race_<id>_in_order` — the template's dummy stood in every checkpoint of every lap in order, its count asserted after each, the race over at the last and running again with nobody credited after a start — and `race_<id>_bystander` — a second player spawned above every checkpoint and asked on every step the dummy runs the course: at the dummy's finish the race is over and the second was never credited — and, for a race of two checkpoints or more, `race_<id>_skip` — the dummy stood in each checkpoint further on than its next, the finish included: its count does not move and nobody has finished; stood in its next, it is credited. Each asks the race's own body function, the one the tick runs. The gallery primary binds the class in the yard behind the long hall, on the legs its vehicles already ride: a race for each objective kind's `course` — one of them two checkpoints run for two laps with a `passed` line and a board, one a checkpoint over water, one a cart's run — and a sixth in the far hall with a grace and a listed actor walked `along` it, ended by an `end-race`.

## 9. What it leaves out, by name **[authored]**

A course **walked on foot as an objective's leg**: the race credits a racer on foot, and the build proves no walked course — a walked leg is one polyline per step in the contract, and routing it through checkpoints is the leg proof's change, not this one's; `course` with no `ride` is `DW1117` until it lands. A **time**, a best lap, a board ordered by time. An effect bundle fired **at a checkpoint or at a finish** per racer (an effect root of the race): the board is the finish's record and the objective's bundle the story's. A **direction**: a racer may run the course's stretch between two checkpoints either way. A racer **dropped** from a race, and a race restricted to **some players**. A **wave's** bodies as racers. The **mode** a map is in while a race is run — what suspends the main quest's compass, journal and clocks and gives them back — which is a mechanism over quests and not a race's field, and is not built here.

## 10. Acceptance criteria

1. **The surface.** `delvec schema --stage quests` exports `races[]` with `id`, `checkpoints[{at, extent}]`, `laps?`, `racers?`, `board?`, `grace_ticks?`, `missed`, `passed?`, the verbs `start-race` and `end-race`, `course?` on every objective kind and `along?` on `move-actor`, at `dsl_version` 0.79.0; every document the repository holds parses. `crates/dsl/tests/it/v29_firework.rs` holds the sixty-four verbs.
2. **The refusals of the document.** `crates/dsl/tests/it/race.rs`: each arm of `DW1117` is raised at the field that says it and the same race without the fault is silent; a race, a racer, a board, a `course` and both verbs naming nothing declared are `DW0112`; a board a race places on counts the party and its listed actors among its rows.
3. **The emission.** `crates/delvec/tests/it/vehicle.rs`: the body function credits checkpoint `j` only under the racer's next being `j` and its hitbox meeting the box, and gives a player in the second checkpoint with the first unpassed `missed`; the credit calls the finish at the total; the finish places a player and a listed actor by the board's own functions, schedules the end `grace_ticks` on at the first finish, and ends the race when no player and no listed actor that has started is short — the credit, not the body's asking, puts a racer on the course; the start clears the schedule; the tick runs the body as `@a` and as each listed actor while the race runs.
4. **The course is ridden in the order written.** `vehicle.rs`: a leg declared a course of two checkpoints is one `ride` step whose `medium` passes a cell of the first before a cell of the second; the same checkpoints written the other way round are passed the other way round (the perturbation only this proof catches); two laps pass the first again after the second; the objective's completion, and nothing else in the tick, waits on the race's latch. A course the vehicle's medium does not reach is `DW0311` naming the course.
5. **The proofs over the class.** `vehicle.rs`: a checkpoint in the air over the pool is `DW1118` naming it; the same checkpoint written twice in a row is `DW1118`; a course no `start-race` precedes, and one whose last line before it is an `end-race`, are `DW1119`. `nav::race`'s unit tests hold `cell_in_box` and the meeting of two boxes.
6. **An actor runs it.** `vehicle.rs`: a `move-actor` `along` a race stands on each checkpoint's cell on its way, and the same walk with no `along` does not.
7. **PackTest.** The gallery build emits `race_<id>_in_order` and `race_<id>_bystander` for every race and `race_<id>_skip` for every race of two checkpoints or more, and they run green in tier 2.
8. **The gallery.** Every unit of §2 is bound in the gallery primary, the coverage gate reports none in neither state, and the gallery bot's run completes every objective that declares a `course`.
9. **Codes and pages.** Every code of §7 is asserted by a test and sits on its module's page; `check-dw-codes.py`, `check-diagnostic-messages.py`, `check-capability-ownership.py`, `check-effect-roots.py`, `check-gallery-coverage.py`, `check-live-commands.py` and `check-skill-page.py` are green.

## 11. Measurements this spec states

Written into the commit bodies that land the implementation: the race binding line over the gallery; each course leg's medium cell count; the two templates' names per gallery race.
