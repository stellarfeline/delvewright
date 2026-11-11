# Release notes

The notes of the release being prepared: what the next release of `delvec`, `delvewright-dsl` or the `delvewright` plugin changes for the people who use it. One entry per line the release names, headed by the release's title (`## delvec 1.15.0`); the release workflow publishes the entry as that Release's notes, below the opening it reads from the tree, with a link to everything merged since the previous release of the same line. Every change ever merged is in that link; an entry holds the ones worth telling.

Each of the three lines is released on its own clock — a plugin release needs no engine release — so the pull request that prepares a line's release writes that line's entry from the changes merged since the line's previous release, and drops the entries of releases already published. The published GitHub Release keeps each version's text; nothing accumulates here, and a release whose version has no entry here is refused.

An entry is a summary, and `tools/ci/check-release-notes.py` holds it to that: a short `### Highlights` section, then the changes as bullets under at least one `### <group>` — the Keep a Changelog kinds (`Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, `Security`) or the areas of the engine they touch — within the ceilings `tools/lib/release_notes.py` states, every line written for the reader: a line that is a merged change's own title is refused, and so is an entry for a version the tree has moved past. A closing paragraph may say what the entry leaves out.

## delvec 1.16.0

### Highlights

- A body can enter the way its kind does: a warden digs up out of the ground, a wither charges up, and then it hunts the party.
- The bot proof walks a route beside a body it never fights, so a level whose party survives by moving can be proven completable.
- Every quest line states what the player does on it, and the build counts and prices those acts against the delve's target length.
- A wide town builds in about six minutes, with identical output.
- Several playtest bugs are fixed: a compass that jumped lines, walks that cut across lawns, and a flag a puzzle never paid.

### Added

- An actor can declare `entrance: "native"` and `outrun`: the body arrives the way its kind does in vanilla, and the ladder walks every step it is loose beside it, with nothing staged away or healed (`DW1081`, `DW1082`, `DW1083`). That the body is free to act is proven by the compiler's tests, not by a check at run time.
- `unleash-actor` now arrives a body whether or not a puppet stands, and only a click's own bundle can lock the new body onto its striker.
- Each planned quest states its activity; `delvec validate` prints the activity census, every build writes `validation/activity-census.json`, and a goal that names an act nobody performs on its line is refused (`DW1079`).
- The exported critical path names the bodies each step is walked beside, and the gallery gains a warden that digs up out of the long hall's porch.
- The playtest server ops its creator and hands every client the resource pack.
- `DELVEC_PHASE_TIMING` stamps the build log with wall time, CPU and threads for each phase.

### Changed

- A build of the 95-place town takes about six minutes: the branch worlds, the review worlds' light strips and the write sequence run in parallel.
- A cast row that says `unchanged` claims nothing, and `DW0858` judges the clause that wins.
- A staged walk between places keeps to the declared ways, at the width of the body that walks it.
- A registered body's click box and reach are sized to the body its registry row names, so a large friendly mob can be clicked anywhere it is drawn.

### Fixed

- A flag set behind a numeric gate is credited only once the presses that open it are on the path (`DW1080`); before, a puzzle could ship whose bot run stuck.
- A followed objective that completes hands the compass to the next step on its own line.
- The bot judges a watcher's facing after its watch line has run.
- A generated PackTest wounds a body to half of its own maximum health, so a small body's census no longer reads full health and reds.
- The load world and each arrival hold no block that only a pressed button writes, so `delvec written-world` matches the server.
- The staging gate's equipment row applies only to a campaign that declares equipment; a campaign whose actors wear nothing is no longer refused as unbound.
- The required Rust partition checks report on a pull request that changes no Rust.

The entry leaves out documentation, tests and release machinery, which change nothing in what a creator does.

## delvewright-dsl 0.71.0

### Highlights

- The DSL format number moves with this release; documents declare it and `delvec fmt` writes it.
- Actors can declare how they arrive and whether the party outruns them, and every planned quest states its activity.

### Added

- Actors gain `entrance`, read off a kind-action registry (`minecraft:warden`, `minecraft:wither`), and `outrun`.
- The quest plan gains `activity` on each line, and the metrics table prices each act in minutes.
- The ambush sugar keeps one continuation site and no story note.

### Changed

- A goal is held to the acts of its line, so a fixture or campaign that named an act nobody performs is re-headed.

## delvewright 1.16.0

### Highlights

- The page now sends a fresh reader through an invented script before the build, so a story that only its author understands is caught first.
- Every `/new-delve` page is one paragraph per line, and `SKILL.md` points at its step pages.
- The page pins `delvec` 1.16.0 and describes the entrance, the pursuers and the activity census.

### Added

- A new step 5b has a fresh reader review the invented script before anything is built, with a script for the reader's hand-off.
- The pages describe an actor's native entrance, `outrun` and the pursuit leg of the exported path.
- The pages describe each quest line's activity and the `DW1079` refusal.

### Changed

- Every page in `references/` is written one paragraph per line, and `SKILL.md` points at the step page that carries each step's body.
- The page is proven against `delvec` 1.16.0.

### Fixed

- The place handout reads every cast shape the DSL accepts.

The entry leaves out the release machinery itself, which changes nothing in what a creator does.

## delvewright-dsl 0.74.0

### Highlights

- An objective can carry a time limit: the task is done in time or lost, and the story goes on either way.
- A boat, a minecart or a saddled mount can be placed for players to ride, and a crossing nobody can walk can be ridden.

### Added

- Every objective kind takes `limit {ticks, from, on_expire}`: a window in game ticks, started at the objective's activation, its quest's start or an earlier objective's activation, whose expiry fires `on_expire` in place of the completion.
- The `despawn-wave` verb removes a wave's standing bodies unseen.
- A wave mob's `count` takes `{base, per_player}`: one player meets `base`, and each further player present at the spawn adds `per_player`.
- A `loot` fill can wait for its beat: `filled: "by-effect"` with the `fill-loot` verb.
- A firework rocket stack in a `loot` fill, a `give-item` or a class kit carries its stars as `fireworks`.
- An effect's `audience` can name a wave's or an actor's bodies (`{"wave": id}`, `{"actor": id}`) on `give-effect`, `clear-effect` and `damage`; `damage` takes `by: "actor"` to credit the acting player.
- `vehicles[]` on stage 5: `{id, entity, at, facing?, bounds, name?, equipment?, attributes?}`, the kind (boat, minecart, mount), seats and steering item read from the pinned game; the `seat-vehicle` and `despawn-vehicle` verbs.

### Changed

- The `damage-players` verb is named `damage`.
