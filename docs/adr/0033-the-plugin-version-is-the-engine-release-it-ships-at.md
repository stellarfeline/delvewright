# ADR-0033: The plugin's version is the engine release it ships at

- **Status**: Accepted
- **Date**: 2026-10-09
- **Source**: the tags of `stellarfeline/delvewright` read with `git show <tag>:<path>`
  for every `delvec--v*` tag; the logs of `plugin-release.yml` runs 35152655434
  and 37555623479; `main`'s branch protection and the check suites of the two
  release commits, read through the API; Claude Code's documented update rule,
  quoted in `docs/reference/front-end-standard.md` §2c.
- **Supersedes, in part**: ADR-0028 §5, whole. The rest of ADR-0028 stands.
- **Refines**: ADR-0029 §1 and §3 (the pin names the unborn tag before the
  release; that order is now enforced, not only documented).

## Context

The marketplace entry delivers the plugin root at the engine tag the page pins
(ADR-0029 §1). Claude Code skips an update when "the resolved version matches
what a user already has". So an installed creator receives a newer page only
when the pin moves to a tag whose tree carries a different `plugin.json`
`version`.

ADR-0028 §5 made a separate plugin release the only thing that moves that
version, by committing the bump and fast-forwarding `main`. Measured:

- **Every engine tag delivers 1.5.0.** All eight `delvec--v*` tags
  (`1.6.0` … `1.10.0`) carry `plugin.json` `version` 1.5.0. The pin moved
  through five engine releases under one version, and `/plugin update` reports
  every creator who installed from any of them up to date.
- **Six of the eight tags pin an older engine than their own.** Only
  `delvec--v1.6.0` and `delvec--v1.8.2` pin themselves; `delvec--v1.10.0` pins
  `delvec--v1.9.0`. The pin was moved onto each tag after its release instead of
  naming it before, so the page a creator installs tells their Init to install
  an older engine. That breaks ADR-0029 §1's property that the page and the
  engine are one commit. It also means no edit on `main` after a release can
  change what that release delivers, whatever moves the version.
- **The plugin release never completed.** Both runs were refused at the
  fast-forward of `main`: GH006, "17 of 17 required status checks are expected"
  (1.5.0) and "21 of 21 required status checks are expected" (1.6.0), on
  commits carrying every required check green from the app the protection names,
  with `main` still at the base. The protection read through the API explains
  nothing. 1.5.0 reached `main` by hand; 1.6.0 never did and left
  `release/plugin-1.6.0` behind, which blocked a re-dispatch.

## Decision

1. **The plugin's version is derived: it equals the version of the engine
   release the pin names.** `tools/ci/check-skill-page.py` rule 11 refuses any
   other value, in this tree and, where the tag exists, in the `plugin.json` of
   the tree the tag names. There is one right value and no base revision, so the
   rule binds on every run. A creator's installed plugin version names the
   engine release whose page they hold and whose `delvec` their Init installs.
2. **The pin names the unborn tag before the release.** The pull request that
   moves the pin to `delvec--v<version>` moves `plugin.json` to `<version>` in
   the same tree, and `engine-release.yml` tags that merge commit. A pin onto a
   tag already written is refused unless that tag's tree states its own version,
   which only a tree that named the tag before its release does. The one pin
   from before this record (`delvec--v1.10.0`, delivering 1.5.0) is recorded by
   its exact pair and must be removed by the pull request that moves the pin.
3. **The plugin release writes no branch.** `plugin-release.yml`, dispatched
   with a version, tags `delvewright--v<version>` at the commit
   `delvec--v<version>` names and publishes the plugin root as a Release, after
   refusing an engine tag that is absent, off `main`, or whose tree pins another
   tag or states another version. It delivers nothing the engine release has not
   already delivered.

## Consequences

- Every pin move is a version move, so every engine release that moves the page
  reaches every installed creator.
- The GH006 refusal cannot recur: no workflow pushes to `main`.
  `tools/ci/wait-required-checks.py` and the `--base` input of
  `check-skill-page.py` are removed.
- Creators who hold 1.5.0 stay on it until the first engine release made under
  this record.
- An engine release that does not move the page has no plugin release.

## Revisit triggers

- Claude Code changes how it detects an update (for example, comparing the
  resolved commit when a version is declared).
- The marketplace entry stops delivering the plugin root at the engine tag.
