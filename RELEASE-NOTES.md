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
