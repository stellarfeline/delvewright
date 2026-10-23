# Changelog

What each release of `delvec`, `delvewright-dsl` and the `delvewright` plugin changes for the people who use it. One entry per released version, newest first, headed by the release's title (`## delvec 1.15.0`); the release workflow publishes the entry as that Release's notes, below the opening it reads from the tree, with a link to everything merged since the previous release of the same line. Every change ever merged is in that link; this file holds the ones worth telling.

An entry is a summary, and `tools/ci/check-changelog.py` holds it to that: a short `### Highlights` section, then the changes as bullets under at least one `### <group>` — the Keep a Changelog kinds (`Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, `Security`) or the areas of the engine they touch — within the ceilings `tools/lib/changelog.py` states, every line written for the reader: a line that is a merged change's own title is refused. A closing paragraph may say what the entry leaves out. The entry is written before the version is released, by whoever prepares the release, from the changes merged since the previous release of the line; a release whose version has no entry here is refused.

Entries begin with the first release made under this rule. Earlier releases keep the notes they were published with.
