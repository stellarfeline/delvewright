# Release notes

The notes of the release being prepared: what the next release of `delvec`, `delvewright-dsl` or the `delvewright` plugin changes for the people who use it. One entry per line the release names, headed by the release's title (`## delvec 1.15.0`); the release workflow publishes the entry as that Release's notes, below the opening it reads from the tree, with a link to everything merged since the previous release of the same line. Every change ever merged is in that link; an entry holds the ones worth telling.

Each of the three lines is released on its own clock — a plugin release needs no engine release — so the pull request that prepares a line's release writes that line's entry from the changes merged since the line's previous release, and drops the entries of releases already published. The published GitHub Release keeps each version's text; nothing accumulates here, and a release whose version has no entry here is refused.

An entry is a summary, and `tools/ci/check-release-notes.py` holds it to that: a short `### Highlights` section, then the changes as bullets under at least one `### <group>` — the Keep a Changelog kinds (`Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, `Security`) or the areas of the engine they touch — within the ceilings `tools/lib/release_notes.py` states, every line written for the reader: a line that is a merged change's own title is refused, and so is an entry for a version the tree has moved past. A closing paragraph may say what the entry leaves out.

## delvec 1.16.1

### Highlights

- A stealth leg is now walked crouched for the whole leg, so a guard or warden that listens for a standing body is proven against the walk the player makes.
- A showcase picture shows the story's moment: the world the path has built by then, its cast where the story stations them, facing the lens, with the horizon behind.

### Fixed

- A declared stealth leg was walked upright by the bot; the crouch now holds on every tick, yielding only for the moves a crouched body cannot make (a climb down, a step down, a jump).
- A trigger performed in front of a stealth objective is walked sneaking, so a long approach to a sneaking goal no longer wakes what it should not.
- A death on a pursued step is recorded as a measurement of the run, not lost.
- A build that plans story worlds is admitted to a playtest only with every planned world passed on that very build; a branch no world realizes is refused.
- A showcase moment pictures the blocks, cast and sky left by one play of its path, and `DW0721` refuses only a moment identical to the load.
- A body that watches the nearest player turns to the lens, and a frame looking past the build shows ground out to where its fog closes.

### Added

- `DW1100` names a showcase frame that frames a body the renderer draws nothing of; its hq picture is not written.

The entry leaves out the tests and documents that moved with these fixes.

## delvewright 1.16.1

### Highlights

- The page pins `delvec` 1.16.1, and its hand-over steps carry the branch-run record a story-world build now owes.

### Changed

- The hand-over and ladder pages name the branch-run record the staging gate reads before it admits a build with story worlds.
- The page is proven against `delvec` 1.16.1.

The entry leaves out the release machinery itself, which changes nothing in what a creator does.

## delvewright-dsl 0.78.0

### Highlights

- An objective can carry a time limit: the task is done in time or lost, and the story goes on either way.
- A boat, a minecart or a saddled mount can be placed for players to ride, and a crossing nobody can walk can be ridden.
- A character standing in the world can hop, crouch, turn, hold something and change size, alone or as a crowd in a rolling wave.

### Added

- A place's side takes `sheer` beside `wall` and `open`: no wall stands there and the ground beyond is left as the terrain has it. Every other unwalled side of a ground place is met by the land in steps, at the place's `grade: {rise, run}` (one block per two columns when absent).
- Every objective kind takes `limit {ticks, from, on_expire}`: a window in game ticks, started at the objective's activation, its quest's start or an earlier objective's activation, whose expiry fires `on_expire` in place of the completion.
- The `despawn-wave` verb removes a wave's standing bodies unseen.
- The `fail-objective` verb settles a task as failed from any effect bundle — a goat killed fails the goats task, and the day goes on.
- Every objective kind takes `record`, what the task was, and the `record-outcome` verb — in any effect bundle and on a dialogue option — writes a way the task went; the journal keeps a finished task with every line its play wrote.
- A wave mob's `count` takes `{base, per_player}`: one player meets `base`, and each further player present at the spawn adds `per_player`.
- A `loot` fill can wait for its beat: `filled: "by-effect"` with the `fill-loot` verb.
- A firework rocket stack in a `loot` fill, a `give-item` or a class kit carries its stars as `fireworks`.
- An effect's `audience` can name a wave's or an actor's bodies (`{"wave": id}`, `{"actor": id}`) on `give-effect`, `clear-effect` and `damage`; `damage` takes `by: "actor"` to credit the acting player.
- `vehicles[]` on stage 5: `{id, entity, at, facing?, bounds, name?, equipment?, attributes?}`, the kind (boat, minecart, mount), seats and steering item read from the pinned game; the `seat-vehicle` and `despawn-vehicle` verbs.
- Every objective kind takes `ride`: the leg to it is ridden on the named vehicle.
- A ridden objective whose act a rider can do — a container opened, a lever or button pressed, a hitbox or an NPC clicked — is done from the seat of a boat, a raft or a pig, and the next objective ridden on the same vehicle goes on without getting off.
- A census counts a vehicle's riders: `state[].counts.of: {"riders": id}`.
- A vehicle takes `seated` (`at-load` or `by-story`: first placed by a `seat-vehicle`) and `left` (`at-its-mark` or `where-left`: a riderless body stays where its rider left it).
- Six verbs move a staged body: `hop {height?}`, `pose {pose}`, `turn {by}`, `face {to}`, `hold {slot, item?}`, `resize {scale}`. The body is the effect's `audience`, which can name an NPC (`{"npc": id}`) or a cast (`{"bodies": [...]}`).
- A `particle` and a `play-sound` can happen at staged bodies: `at: {"bodies": [...], "up"?}` and `at: {"at": "bodies", "bodies": [...]}`.
- A body's move, a `particle` and a `play-sound` take `repeat {times, every_ticks}` and `stagger_ticks`.

### Changed

- The `damage-players` verb is named `damage`.
- `play-sound` `at: {"at": "actor"}`, which every build refused, is replaced by `at: {"at": "bodies"}`.
- A limited task's clock settles it when it runs out whatever the task's own gate says, so a task whose gate a lost body holds shut no longer hangs.
