# ADR-0038: The plugin is released on its own version — the marketplace serves the plugin root at the plugin's tag, and the page pins the engine it is proven on

- **Status**: Proposed
- **Date**: 2026-11-07
- **Source**: the requirement that the `delvewright` plugin is releasable
  independently of `delvec`; the engine tree at `f2b106d09` (the `main` this
  record lands on) and the tags of `stellarfeline/delvewright` read with
  `git tag -l` and `git show <tag>:<path>`; Claude Code's documentation read
  at the pages themselves and quoted in `docs/reference/front-end-standard.md`
  §2c — *Plugin loading* ("Versions and updates", "How Claude Code computes the
  version"), *Host a marketplace* ("Keep users up to date", "Release a new
  version", "Hold users on one version"), *Marketplace reference* ("Plugin
  sources"), *Plugin manifest* (`version`); the measurements ADR-0029 §4
  records on Claude Code 2.1.265, which this record reuses and does not
  repeat; ADR-0033's Context, whose defect this record must not reopen.
- **Supersedes**: ADR-0033, whole. ADR-0029 §1 (the page ships at the engine
  release it pins), §2 (one name in the pin and in the entry) and §5 (what an
  existing install receives); the rest of ADR-0029 stands — §3 and §4 for the
  engine's pin, §6 and §6a for the gate's two states over that pin.
- **Refines**: ADR-0028 §2's plugin row (the identity, the notes and the two
  platforms of the plugin release are restated in §4 below) and §4 (a release
  is a human's dispatch: now with a commit, as the other two lines).

## Context

### The case that raised it

`main` carries one fix since `delvec--v1.15.0`: a skill script reads every
cast shape the format accepts, where it read some. No engine or format byte
changed. Under ADR-0033 the plugin's version is the engine release it ships
at, so delivering that fix to a creator who runs `/plugin update` needs an
engine release 1.15.1 whose binary is byte-identical to 1.15.0 — a shelf of
five archives and a crates.io version burned to move one Markdown file. That
coupling is rejected: the plugin is released on its own.

### What ADR-0033 fixed, and must stay fixed

Before ADR-0033 the plugin's version never moved — eight engine tags all
delivered `plugin.json` 1.5.0 — so `/plugin update` reported every installed
creator up to date through five engine releases, and six of the eight tags
pinned an engine older than themselves. ADR-0033 closed both by deriving the
version from the pin. Whatever replaces that derivation keeps three properties:

- a creator who runs `/plugin update` receives every page change that was
  released;
- the page always names an engine release it is proven on;
- the marketplace delivers exactly the released plugin tree.

### What the documentation says, read now

**The version is the update event.** "Claude Code computes a version for every
plugin it installs, and that version is how it detects an update. `claude
plugin update` and background auto-update compute the version again and don't
replace the cached copy when it matches what `installed_plugins.json`
records." The version is "the `version` field in the plugin's manifest" first,
then the entry's, then — for a `git-subdir` source with neither — "the commit
SHA of the source, shortened to 12 characters", with "a hash of the
subdirectory path". "A manifest that pins `"version": "1.0.0"` keeps every user
on the cached copy until its author changes the string, however many commits
they push." And, under *Release a new version*: "To release a new version to
users, change the plugin's `version`. Users get a new copy only when the
plugin's computed version differs from the one they have."

**The entry holds users on a ref.** "`ref` and `sha` on the plugin entry: `ref`
names a branch or tag and `sha` names a commit for a `github`, `url`, or
`git-subdir` source." A `git-subdir` source takes `url`, `path`, `ref` ("a
branch or tag. Defaults to the repository's default branch") and `sha` ("When
you set both `ref` and `sha`, Claude Code checks out `sha`"). "One marketplace
serves one version of each plugin at a time", and "a ref that moves without a
version change leaves users on the cached copy."

**The manifest's `version`** is "a version string, not checked against semver.
Setting it pins the plugin to that version until you change it." `claude plugin
validate` reports a missing `version` as a warning, and `--strict` — which
`ci.yml` runs over the plugin root — turns warnings into failures.

So the mechanics are unchanged from ADR-0028 and ADR-0029: a creator receives
new bytes when the entry's `ref` resolves to a tree whose `plugin.json`
`version` differs from the one they hold, and nothing else moves them. ADR-0029
§4 measured exactly the transitions this record relies on, on the pinned
Claude Code: an entry whose `ref` moved to a NEW tag carrying a HIGHER version
updates an existing install (`updated from 0.2.0 to 0.3.0`); a `ref` naming a
tag the remote lacks refuses a fresh install and leaves an existing one
untouched; a new tag under an unchanged version moves nothing. The tag's LINE
does not enter into it — Claude Code reads a ref name and a version string —
so what ADR-0029 measured for an engine tag holds for a plugin tag.

### What ADR-0029 §1 bought, and what it costs now

ADR-0029 made the entry's `ref` the engine tag so that "a creator never holds a
page and an engine from different revisions", by construction. Under
ADR-0033 that bound the plugin's version to the engine's. The property was
real: at `227e7504` the page named two engine paths the pinned tree lacked, and
nothing caught it. It is now caught by a gate rather than by construction —
`check-skill-page.py` rule 21 holds every `$DELVEWRIGHT_ENGINE/<path>` the page
names to the tree at the pin, with its own binding count, and rules 3–5, 17,
18 and 20 hold the page's command, document, code and in-game vocabularies to
the engine at the pin. A page judged green against the engine at `[engine].ref`
is a page proven on it, whichever commit the page itself ships from. That is
the property this record keeps; the one-commit property is given up, named
here as the cost.

### The alternative the documentation offers, and why not

Leaving `version` out of both the manifest and the entry makes every tag move
an update, with no number to bump (*Plugin loading*: "To let users track
commits instead, leave `version` out of both the manifest and the entry").
Rejected: the page records the plugin version a run used in `GENERATION.md`
and a later round under another version says so, which needs a number a person
can read; ADR-0016 gives the product a version line; and `plugin validate
--strict` fails the manifest without one.

A moving branch as the entry's `ref` (the release workflow fast-forwarding a
`release/plugin` branch to each tagged commit, so that `main` never names an
unborn ref) is rejected too: it puts a second mutable ref beside the tag,
writable by anyone with push access and held equal to the newest tag only by a
gate, where a tag under the ruleset of ADR-0028 §9 cannot move at all. The
interval ADR-0029 §4 measured is the price of an immutable name, and it is one
release run long.

## Decision

### 1. The plugin's version is its own line

`plugin.json` `version` is the plugin's number, a strict `major.minor.patch` in
the grammar of `tools/lib/release_tags.py` (so that `delvewright--v<version>`
is always a tag), continuing from 1.15.0, the version the last release under
ADR-0033 delivered. It moves only in the pull request that prepares a plugin
release, forward from the newest `delvewright--v*` tag: the next patch when
the release only fixes what the page already does, the next minor when it
changes what a creator can do — a new step, a new ask, an engine capability
the page reaches for the first time, a pin moved to a new engine minor. A
page edit merged between releases leaves the version alone and reaches
creators with the next plugin release; it is not refused, because the entry
(§3) keeps every creator on the released tree until then.

`tools/ci/check-skill-page.py` rule 11 holds the version to the line's
releases on every run: where this checkout carries the tag the version
derives, that tag is the newest release of the line and the gate prints how
many files this tree's plugin root is ahead of it by; where it does not, the
version is above every released one, which is the state a release-preparing
pull request leaves `main` in. A released version older than the newest, or an
unreleased version below it, is a red. A checkout carrying no tag of the line
cannot judge and refuses, naming the fetch; `.github/actions/skill-page-objects`
performs it.

### 2. The page pins the engine it is proven on, and the plugin ships only at a published one

`[engine].ref` beside the page stays what ADR-0029 §2 made it: one name, a
`delvec` release tag, the engine every gate materialises and judges the page
against, the engine Init I2 detaches at and Init I3a downloads. It is
independent of the plugin's version: a plugin release may pin an engine
released a month earlier, and an engine release may ship with no plugin
release. The unborn state of ADR-0029 §3 stands for the pull request that
prepares an engine release, and only there: `check-skill-page.py` and
`check-pins.py --online` accept the pin naming this tree's own next engine tag.

A plugin release is refused unless the engine the page pins is published:
the tag exists, and its Release is neither absent nor a draft
(`tools/ci/check-plugin-release.py --online`, §4). Rule 13 of
`check-skill-page.py --online` additionally refuses a draft engine Release
wherever it runs, since a draft is served to nobody.

### 3. The marketplace serves the plugin root at the plugin's own tag

The entry in `.claude-plugin/marketplace.json` stays a `git-subdir` source
(`url` the engine repository, `path` the plugin root, no `sha`, no `version`),
and its `ref` is `delvewright--v<plugin.json version>`: derived from the
version, and held equal to it by rule 12. The tree a creator receives is the
plugin root at that tag, which the ruleset of ADR-0028 §9 makes immutable, and
the Release's archive is `git archive` of the same tree at the same commit —
so the marketplace delivers exactly the released plugin tree, by construction.

The commit names its own tag, as an engine release's does (ADR-0029 §3): the
pull request that moves the version moves the entry with it, merges, and the
release dispatched on its merge commit writes the tag there. Between that
merge and that release the entry names a tag the remote lacks, and the
measured interval of ADR-0029 §4 applies as measured: a fresh install is
refused at the fetch of the ref, an existing install keeps working, and the
release that closes it is dispatched as the next act after the merge. After
the release, `main`'s page may move ahead of the tag; the entry still names
the tag, so no creator receives an unreleased tree.

`--online`, rule 13 asks the remote about the plugin's tag as it asks about
the engine's: unborn on both sides is the preparing state and passes; present
on both sides at the same commit with a published Release passes; a tag on one
side only, at another commit, or with no Release is a red.

### 4. The plugin release

`plugin-release.yml` is dispatched with a commit on `main` (default: the tip),
as the engine and the format crate are; nothing is typed but the commit. It
writes to no branch. In order:

1. **Identity**, `tools/ci/check-plugin-release.py --commit <sha> --online`,
   refused before anything is written: the commit is on `main`;
   `plugin.json` `version` there is strict and `delvewright--v<version>` is
   unborn in the checkout and on the remote; the version is above every
   `delvewright--v*` tag; the entry at the commit names that tag; the page's
   pin is a `delvec` tag the checkout carries whose Release the remote serves
   as published.
2. **The page at that commit**, `tools/ci/check-skill-page.py --online`, run
   with the tools the commit carries: every offline rule, the engine's shelf
   whole, the plugin's tag confirmed unborn.
3. **Archive, checksums and notes**: the plugin root as a zip stamped with the
   commit's time (ADR-0028 §2), `SHA256SUMS`, and `tools/ci/release-notes.py
   delvewright`, which refuses a tree whose `RELEASE-NOTES.md` has no
   `## delvewright <version>` entry (ADR-0036).
4. **The tag**, through `.github/actions/write-release-tag` at the commit,
   then **the Release** with `--latest=false` (ADR-0028 §8), read back.

No engine release is required, implied or checked for beyond §2's "published".
The dispatch is the approval, as before: the workflow starts only by a human,
touches no registry, and `tools/ci/check-release-publish-gate.py` keys the
class on the file.

### 5. What reaches whom

An existing default-branch install updates on `/plugin marketplace update
delvewright` then `/plugin update`, because the entry's `ref` now resolves to a
tree whose version is above the one held (measured, ADR-0029 §4). A creator who
holds 1.15.0 — delivered from `delvec--v1.15.0` under ADR-0033 — receives
1.15.1 that way at the first release under this record, with the same engine
pinned. A fixed page is held by adding the marketplace at the plugin's own tag:
`/plugin marketplace add stellarfeline/delvewright#delvewright--v<version>`,
whose catalog row names itself; the Release's notes say so.

### 6. The notes file, per line

`RELEASE-NOTES.md` holds at most one entry per line, each line prepared on its
own clock: the pull request that prepares a line's release writes that line's
entry and drops the entries of releases already published. An entry whose tag
is written and whose version the tree still states is that line's last
release and passes until the line moves (`tools/ci/check-release-notes.py`,
unchanged in what it judges). ADR-0036 stands; its "replaced wholesale" is
read per line.

## Consequences

- **The order for a plugin-only release**: one pull request moves
  `plugin.json` `version`, the entry's `ref` and the notes entry; it merges on
  the terms of any page change; `plugin-release.yml` is dispatched on its merge
  commit as the next act. For an engine release whose page must reach creators,
  the same pull request also moves `[engine].ref` and `[engine].version` and
  writes the engine's entry; the engine release is dispatched first, the plugin
  release after it on the same or a later `main` commit.
- **The first release under this record** is `delvewright 1.15.1` at
  `delvec--v1.15.0`, prepared in this record's own pull request: the fix that
  raised the case, with no engine release.
- **Sites that change**: `plugin-release.yml` (a commit input; identity through
  the tool); `tools/ci/check-plugin-release.py` and its tests (new);
  `tools/ci/check-skill-page.py` rules 11, 12 and 13 and
  `tools/tests/test_check_skill_page.py`; `.github/actions/skill-page-objects`
  (fetches the plugin's tags); `.github/pins.toml` (`skill-page-engine` has one
  site, the pin beside the page; the entry is no longer a site of it);
  `tools/ci/check-pins.py`'s note on the plugin line; `tools/ci/release-notes.py`'s
  plugin opening; `tools/lib/release_notes.py` and `tools/ci/check-release-notes.py`'s
  statement of the file (per line); `tools/lib/release_tags.py`'s docstring;
  `tools/tests/test_release_tags.py` (every release takes a commit); the
  release-publish-gate fixture; the skill's `versions.toml` header, `init.md`
  I2 and `fetch-delvec.py`'s docstrings (the pin is the engine the page is
  proven on); `docs/reference/skill-workflow.md`, `tools.md` and
  `front-end-standard.md` §2c. No required status context is added or renamed.
- **Costs, named**: the page and the engine a creator holds are two commits,
  held compatible by the gate rather than by construction; the interval of
  ADR-0029 §4, now once per plugin release as well as per engine release; the
  entry on `main` naming an unborn tag between a preparing merge and its
  release.
- ADR-0029's second revisit trigger ("the plugin release proves inert as a
  delivery") has fired and is closed here: the version's home is `plugin.json`
  and the plugin release is a delivery on its own.

## Revisit triggers

- Claude Code changes how it detects an update, or how a `git-subdir` source
  reads a `ref` the remote lacks: §3's interval and §5's transition are
  re-measured.
- A plugin release is wanted at an engine the page has not been walked against:
  the pin moves in a pull request that walks the page, as ADR-0029 §3 step 1
  says; nothing here changes that.
- A second plugin joins the marketplace: §3's entry shape is applied per
  plugin, each on its own line.
- The fresh-install refusal inside the interval is ever met by a creator, or
  the interval is longer than one release run: the moving-branch alternative
  in Context is re-costed against the gate it would need.
