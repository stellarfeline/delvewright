# Ordered checkpoints in a race: established practice and the pinned game's primitives

What a race's course is built from, for the reader who changes `races[]` (`docs/reference/compiler.md`, *Races*). Every rule is marked **[cited]** from a source named beside it, or **[authored]** here. The sources are tutorials and a community wiki, read on the web as summaries of their pages; none is a specification, and each is ideas-only (`docs/ACKNOWLEDGEMENTS.md` carries no entry for them because nothing of theirs is adopted but the idea).

## 1. What established racing games do

- **[cited]** A racer carries the index of the checkpoint it must hit next; a checkpoint counts only when it is that one, and the index then advances. A Verse racing tutorial keeps `NextGate` and `Laps` per racer and one handler for every gate (<https://uefncentral.com/how-to/make-racing-lap-system-uefn>); a Godot course advances `next_checkpoint = (next_checkpoint + 1) % CHECKPOINTS`, so the last checkpoint is followed by the first (<https://coddy.tech/learn/courses/godot_game_neon_circuit/checkpoints_in_order>).
- **[cited]** A checkpoint further on than the next gives no progress and a warning to the racer; a checkpoint already passed does nothing at all, "so backtracking cannot farm checkpoints" (the Verse tutorial).
- **[cited]** A credited checkpoint tells the racer its count (`Checkpoint X/N`); a finish line crossed with a checkpoint missing does nothing (the Verse tutorial, which also notes that it then shows nothing and the racer is left to guess).
- **[cited]** The course's author places the checkpoints so that every shortcut misses at least one, and makes each volume tall and wide enough to catch a racer in a vehicle (the Verse tutorial).
- **[cited]** A rule that judges a lap by anything other than every checkpoint in order is skipped. Mario Kart Wii requires its key checkpoints in order and then accepts a lap when a completion fraction changes by more than 0.95 at the line; the fraction updates only on entering a checkpoint, and the documented *ultra shortcut* takes a lap from the last key checkpoint's region and the line alone (<https://wiki.mkwtas.com/wiki/Checkpoint>).

## 2. What the pinned game gives

- **[cited]** (`docs/reference/vehicles-animals-and-targets.md` §4–§6) A rider's position on the server is its vehicle's seat, and a box selector counts a rider by the box at the seat. A racer on a pig, in a boat or in a cart is found by the same volume as one on foot.
- **[cited]** (the pinned command tree, `crates/delvec/data`) A selector's `x`,`y`,`z`,`dx`,`dy`,`dz` pick the entities whose hitbox meets the box; `@s[…]` asks it of the executor alone. A score is held per entity, a player or any other. `return run` ends a function with the command it runs. `schedule function … replace` holds one pending call of a function, and `schedule clear` drops it. `title … actionbar` shows a line that fades by itself. A score component draws a holder's score inside a text.
- **Unsupported claim, named as such:** no published Minecraft race map's own functions were read for this record; the searches returned packs that time a race or spawn checkpoint entities, none stating how order is enforced.

## 3. The rules the engine takes from this

- **[authored, on §1]** One count and one next-checkpoint index per racer, in two scoreboard objectives of the race's own. A credit is the next checkpoint and nothing else.
- **[authored, on §1]** A player in a checkpoint further on in the lap reads the race's `missed` line; a checkpoint behind the racer is silent.
- **[authored, on §1]** A credit shows the race's `passed` line and the count. The finish is the last checkpoint of the last lap, so there is no line that can be crossed without its checkpoints: the Mario Kart shape cannot be written.
- **[authored]** Two checkpoints that follow one another share no cell (`DW1118`): a body in both would be credited the second where it was credited the first.
- **[authored, on §2]** A racer is any body with the scores: every player, and each actor the race lists. Where a volume and a score are all that is needed, no player is special.

## 4. The gap this record fills

Before this record a campaign wrote a race as a start trigger and a finish line of `step` plates with `audience: presser`, each placing its presser on a board — a line a racer reaches by any way it likes. The line that should have covered order is spec-0135 §13's *A board of places*: it gives an order of finishing and says nothing of the course.
