# Release notes

The notes of the release being prepared: what the next release of `delvec`, `delvewright-dsl` or the `delvewright` plugin changes for the people who use it. One entry per line the release names, headed by the release's title (`## delvec 1.15.0`); the release workflow publishes the entry as that Release's notes, below the opening it reads from the tree, with a link to everything merged since the previous release of the same line. Every change ever merged is in that link; an entry holds the ones worth telling.

The pull request that prepares a release replaces this file wholesale with that release's entries, written from the changes merged since the previous release of each line. The published GitHub Release keeps each version's text; nothing accumulates here, and a release whose version has no entry here is refused.

An entry is a summary, and `tools/ci/check-release-notes.py` holds it to that: a short `### Highlights` section, then the changes as bullets under at least one `### <group>` — the Keep a Changelog kinds (`Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, `Security`) or the areas of the engine they touch — within the ceilings `tools/lib/release_notes.py` states, every line written for the reader: a line that is a merged change's own title is refused, and so is an entry for a version the tree has moved past. A closing paragraph may say what the entry leaves out.


## delvec 1.15.0

### Highlights

- A build's peak memory no longer grows with the map: the 95-place town that needed about 13 GiB now builds in about 2.4 GiB, with identical output.
- `delvec build` and `delvec detail` print the resource cost before the heavy stages and `delvec build` records it, so a host can size itself first.
- New proofs refuse maps a player could leave by a jump, shut ways learned too late, narrowed stairs and doors, and silent clickable props.
- Places meet seamlessly on living ground, and the checks run much faster on large worlds.

### Added

- A resource cost line is printed before the whole-map stages, and `validation/resource-cost.json` states the computed cost; the engine never refuses a map for its size.
- One reachable-surface graph, simulated under the pinned physics, answers every question about where a body can be; a declared jump is proven makeable (`DW1041`, `DW1042`, `DW1043`).
- A press target must be a real block that answers the player, and a prop that would be silent when pressed is refused (`DW1051`, `DW1052`, `DW1059`).
- A story-shut way must say how the player learns it is shut, and the engine confirms it against what the world shows (`DW1053`, `DW1054`, `DW1060`).
- A harmless monster-kind body within reach of a walked route must carry a line explaining itself (`DW1055`, `DW1056`).
- A fixed structure that blocks part of a door, stair or road is refused (`DW1057`).
- Place boxes can declare each side, the fill takes surface rules and grows trees, and a place that is too small or unreadable for its trade is flagged (`DW1063`, `DW1064`, `DW1065`).
- A sign whose text its sign cannot show is refused (`DW1075`).
- A quest whose trigger disagrees with its plan's `depends_on` is refused (`DW1077`).
- `delvec detail --check` judges a place without writing anything, so places can be checked in parallel.

### Changed

- A side with no declaration now stands no wall, so squares, streets and plots meet as one paved ground.
- A respawn seat is derived beside its rest object, so a campfire can be a rest point.
- The clock in the world stage is stated in ticks, and `test_level` replaces `demo_level`.
- `--stage-anyway` overrides every refusal of the staging gate and prints each one it overrode.
- Reach, occupancy, light and per-place checks are much faster: one graph per world, each block state classified once, and a door edit re-flies only what read the door.

### Fixed

- The terrain's ground inside the sea's band is no longer treated as water.
- What a reach node banks no longer depends on what the flight memo holds.
- The death plan reads the exported path the emitter writes, and the bot learns the boundary a crush death is walked at.
- PackTest templates: the dummy enters with no score on its name, and the wave compass template holds every other body of its wave out.

The entry leaves out CI, planner and documentation work that changes nothing for a creator.

## delvewright-dsl 0.69.0

### Highlights

- The DSL format number is now 0.69.0; documents declare it and `delvec fmt` writes it.
- Place sides default to open, and fill strata let a place's ground read as a hillside.
- Stages 2 and 5 gain the declarations that make harmless bodies and shut ways honest.

### Added

- The world stage gains `jumps[]`, a declared jump the engine proves makeable.
- Boxes gain `sides`, and a node can state how a shut way is learned.
- Fill gains `strata[]` surface rules and tree growth.
- NPCs and actors can carry the line a harmless monster-kind body shows when struck or pressed.
- The stage-1 `time` takes the clock in ticks, and `test_level` replaces `demo_level`.
- A quest trigger is checked against its plan's `depends_on`.

### Changed

- `boxes[].sides` defaults to no wall when a side says nothing.
- Float views and clear widths are read through the same shared metrics every proof uses.

### Fixed

- A sea-band ground cell no longer reads as water in the navigation model.

## delvewright 1.15.0

### Highlights

- The `/new-delve` creator asks only design questions; technical choices become stated defaults, and debug mode reports them.
- Places are detailed in parallel, each from a starter that already passes.
- The skill gains a web-narrative page.

### Added

- Debug mode (`DELVEWRIGHT_DEBUG=1`) records each technical decision and engine limit in the run's final report without stopping; the released default is off.
- Step 9 details places in parallel, each place checked with `delvec detail --check` and committed serially.
- A per-place starter draws the building as cells and names its parts, so a place agent begins from a program that already passes.
- Guide text for building a place big enough and legible as its trade, with one style per building.
- A web-narrative skill page.

### Changed

- The skill's asks are a closed list of two asks, one hand-over and two offers; every other question is a default or an engine fix.
- The skill pages name the new refusals (jumps, shut ways, narrowed ways, silent props) so the agent repairs them where they are entered.

### Fixed

- A compass-wave template error in the skill's PackTest material.
