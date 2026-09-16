# ADR-0029: The page ships at a tag — the marketplace serves the plugin from the engine release the page pins

- **Status**: Accepted
- **Date**: 2026-09-16
- **Source**: the rule that the page and the engine a creator clones move
  together, with no window in which a fresh `/new-delve` is broken.
  Measured against the engine tree at
  `227e750446a058c70277198d1ee0afbec6bf3323`, the pinned tree
  `70eea6296cfab2440054f95670729081c3d4bca1`, the remote's tags, and the Claude
  Code documentation pages `plugin-marketplaces.md`, `plugins-reference.md`,
  `plugin-dependencies.md` and `errors.md` under `code.claude.com/docs/en/`,
  read at the pages themselves; every quotation below is from those pages. The
  two behaviours the documentation does not state (§4) were measured on Claude
  Code 2.1.265 — the version `versions.toml [ci].claude_code_version` pins and
  `ci.yml` runs `plugin validate --strict` with — over a throwaway marketplace
  served on local smart HTTP, and again against this repository at the one tag
  that exists; every sentence attributed to the tool below is its own output.
- **Refines**: ADR-0027 §2 (the plugin carries the page and its pin: the pin
  now names the release the page ships at), ADR-0028 §1 (the tag grammar,
  applied), §2 (the plugin Release's notes read the pin), §4 (a release is a
  human's dispatch; every interval below is measured from that dispatch), §5
  (the plugin's version still moves only in its release; what that release
  delivers is restated here), §7 (the first `delvec--v*` tag is the first pin
  under this record).
- **Supersedes, in part**: ADR-0028 §3, whole, and nothing else of that
  record. Its first revisit trigger fires here — not because Claude Code
  documented anything new, but because the rejection rested on a `sha`, and a
  `git-subdir` source takes a `ref`, whose name a commit can know before its
  object exists.

## Context

### What a creator holds today: two revisions

A creator holds two things. The page: the marketplace entry is a relative-path
`source`, the marketplace is added at the default branch, so a fresh install
receives the plugin root as `main`'s tip carries it at that moment. The
engine: Init I2 clones `stellarfeline/delvewright` and detaches at
`[engine].ref`, read from `versions.toml` beside the page — `70eea629`, the
`v1.5.0` commit, registered in `.github/pins.toml` as `skill-page-engine`
under the `release` policy. Nothing makes those two revisions one.

The gap is measured, not theoretical. `tools/check-skill-page.py` rule 21
holds every `$DELVEWRIGHT_ENGINE/<path>` the page names to the tree the gate
runs in — at `227e7504`, 37 of 41 engine paths bound, the gate green. Its own
preamble says which half of the pair it holds: the tree the page ships from,
not the tree `[engine].ref` names. `validation/chunky.sh` and
`validation/chunky-install.sh` are in the tree the page ships from and are not
in the tree at `70eea629` (`git ls-tree 70eea629 validation/` lists neither).
A creator who installs today and follows the page to the Chunky render step
clones an engine without them. That is the window the ruling closes.

### What the documentation says about where a plugin's bytes come from

A relative-path source is a string: "For plugins in the same repository, use a
path starting with `./`" — it "resolve[s] relative to the marketplace root",
and it carries no field, so it cannot pin. A `git-subdir` source "point[s] to a
plugin that lives inside a subdirectory of a git repository" through "a sparse,
partial clone", and takes exactly four fields: `url` ("Required. Git repository
URL, GitHub `owner/repo` shorthand, or SSH URL"), `path` ("Required.
Subdirectory path within the repo containing the plugin"), `ref` ("Optional.
Git branch or tag (defaults to repository default branch)") and `sha`
("Optional. Full 40-character git commit SHA to pin to an exact version").
"When both `ref` and `sha` are set … the `sha` is the effective pin."

Version resolution is unchanged from ADR-0028's reading: "Claude Code resolves
the version from the first of these that is set: 1. The `version` field in the
plugin's `plugin.json` 2. The `version` field in the plugin's marketplace entry
3. The git commit SHA of the plugin's source, for `github`, `url`,
`git-subdir`, and relative-path sources …"; with an explicit version, "Users
get updates only when you bump this field. Pushing new commits without bumping
it has no effect, and `/plugin update` reports 'already at the latest
version'." A marketplace "added with a branch or tag `ref` updates to the
latest commit of that ref, not the repository's default branch."

The documentation is silent on two things this record needs, and neither is
invented here: what a fresh install receives while a `git-subdir` entry's `ref`
names a tag the remote does not yet have, and whether `plugin update`
re-fetches a `git-subdir` source when the entry's `ref` moves to a tag whose
`plugin.json` carries a higher version. Both are settled the way ADR-0028
settled its own silence — a throwaway marketplace over local smart HTTP,
exercised on the pinned Claude Code — by the pull request that implements
this record, before it merges, and this record is finalised with the result.

### What the release machinery already provides

`engine-release.yml` starts only by `workflow_dispatch` on a commit that is on
`main`. Its `identity` job reads `[engine].version` at that commit, derives
`delvec--v<version>` through `tools/lib/release_tags.py` (the grammar of
ADR-0028 §1, stated once), refuses when that tag already exists, and the
gated job writes the tag at that same commit after the registry upload. It
adds no commit. So the tag a `main` commit will carry is a function of the
commit's own tree — `delvec--v` plus the `[engine].version` it states — and a
file inside that commit can name it before it exists. The plugin's own tag
cannot serve this way: `plugin-release.yml` writes `delvewright--v<version>`
on a version-bump commit it creates, and a `sha` is never knowable by the
commit it names.

The remote carries `v1.0.0` … `v1.5.0` and `archive/bell-engine-r1`, and no
`delvec--v*` tag: the first engine release under ADR-0028's grammar is ahead,
and it is the first pin under this record.

### ADR-0028 §3, re-read

§3 rejected "making the release *decide* delivery by turning the marketplace
entry into an `archive` source … or a `git-subdir` source pinned by `sha`" for
three reasons. Each in turn.

**"It needs a second commit after every release to re-point the entry."** True
of a `sha`, which is born with the release commit and can be written only
after it, and true of the plugin's own tag for the same reason. It is not true
of the engine tag: the commit that moves the page's pin writes
`delvec--v<version>` into both the pin and the entry, the release is dispatched
on that commit, and the release creates the tag there. The ordering is
therefore *name, then release, then nothing*: the commit names its tag, the
release gives the name its object, and no commit follows. §1–§3 of the
Decision say this step by step, with what stands between the naming and the
release.

**"It puts a second authority for the plugin's bytes on `main` beside the tree
the developer's skills-dir load already reads."** This stands, and this record
accepts it as a cost, named: after this decision `main`'s tip carries a page a
developer's skills-dir load reads and no creator receives, while the entry on
that same tip names a tag whose tree is what every creator receives. The two
differ by exactly the commits since the pin last moved. The cost is bounded by
the gate (§6): the tip's page is judged against the engine at the tag, so what
the developer reads is at worst ahead of what the creator holds, never
incompatible with it, and it becomes the creator's page at the next pin move
with no other step.

**"It buys a staged delivery nobody asked for."** It is asked for, in these
words: the page and the engine a creator clones move together, with no window
in which a fresh `/new-delve` is broken. A page that ships from one revision and
clones another cannot promise that; a page that ships at the tag it pins can.

What §3 measured stands and is used here: a page edit under an unchanged
`plugin.json` version reaches no creator who already holds that version, and
the fixed-page path — adding the marketplace at a tag — stays as measured.

## Decision

### 1. The page ships at the engine release it pins

The marketplace entry in `.claude-plugin/marketplace.json` is a `git-subdir`
source: `url` is the engine repository, `path` is the plugin root
`.claude/skills/delvewright`, `ref` is the engine release tag the page pins,
and `sha` is not set (it would be the effective pin, and no commit can name
its own). The bytes a creator receives are the plugin root at the commit that
tag points at, and `[engine].ref` inside those bytes names the same tag, so
the engine Init I2 detaches at is that same commit. **The property this buys,
stated once: a creator never holds a page and an engine from different
revisions.** It holds by construction at every tag, not by a gate that
compares two trees.

### 2. One name, in the pin and in the entry

`versions.toml [engine]` beside the page carries `repo` and `ref`, and `ref` is
the tag — `delvec--v1.6.0` for the first pin — matching the `delvec` arm of
`tools/lib/release_tags.py`'s grammar and nothing else. `[engine].release` is
removed: it named the same tag under the old grammar, and one name written
twice in one file is the restatement the file's own header forbids. Every
reader that derived something from `release` (the archive URL, the archive
name, the version the binary must answer) derives it from `ref`.

The marketplace entry's `ref` is the same name in a second file, because
`marketplace.json` is read by Claude Code, which reads nothing else. That is
two files stating one name, and the gate holds them equal (§6): the pin is the
authority, the entry is its copy, and a difference is a red. `tools/check-pins.py`
registers the pin under the `release` policy with the tag name as its value
and discovers it in the site the way it discovers a revision today.

### 3. The order

In the present tense, for every engine release the page moves to:

1. **A pull request names the tag.** It writes `[engine].ref` and the entry's
   `ref` to `delvec--v<version>`, where `<version>` is `[engine].version` at
   the root of the same tree. It is the pull request that walks the page
   against the engine — which is now this tree — and it merges on the same
   terms as any page change. The gate (§6) refuses a name that is not this
   tree's own tag, and refuses a name whose tag already exists, because an
   existing tag names another commit's tree.
2. **The release is dispatched on the merge commit.** `engine-release.yml`
   `identity` derives the same name from the same tree and finds it unwritten;
   the shelf builds; the gated job uploads to crates.io, writes the tag at
   that commit and undrafts (ADR-0026 §3: registry first, tag and Release
   after). From this moment the entry on `main` names a tag with an object, a
   fresh install receives the plugin root of that commit, and its pin names
   the commit it came from.
3. **Nothing follows.** No commit re-points the entry; no pull request waits
   on the release (ADR-0028 §4 stands). `main` moves on; its tip's page is
   judged against the engine at the tag until the next pull request names the
   next tag.

A `main` commit merged after step 1 and before step 2 also names the unborn
tag, and the release is dispatched on whichever such commit the human chooses
(default: the tip); the tag names that commit's tree, and the earlier commit's
page was never anyone's.

### 4. The interval, and what a creator receives in it

Between step 1's merge and step 2's tag write, the entry on `main` names a tag
the remote does not have. Both silences are measured, and both answers are
refusals rather than breakage. That is the difference between this interval and
the window the ruling closes: one refuses, the other broke.

**A fresh install inside the interval.** `claude plugin marketplace add` still
succeeds — the marketplace is the repository's default branch and the entry is
only a catalog row — and then the install exits 1, printing:

```
Installing plugin "delvewright@delvewright"...✘ Failed to install plugin
"delvewright@delvewright": Failed to clone repository for git-subdir source:
Cloning into '<cache>/temp_subdir_<n>.clone'...
fatal: Remote branch delvec--v1.6.0 not found in upstream origin
```

Nothing is installed: the plugin cache stays empty and `installed_plugins.json`
is never written. Those are the real tag this record names and the real
repository — the entry of §1 with `ref` set to `delvec--v1.6.0`, put to the
pinned CLI. With `ref` set to `v1.5.0`, the one tag that exists today, the same
entry installs: `✔ Successfully installed plugin`, `installed_plugins.json`
recording `version 1.4.2` and `gitCommitSha
70eea6296cfab2440054f95670729081c3d4bca1`, and the `versions.toml` in the
delivered bytes is the one that commit carries. So the entry shape is proven by
an install and not by a schema reading, and the refusal is the interval's and
not the shape's.

**An existing install inside the interval.** `claude plugin marketplace update`
succeeds (`✔ Successfully updated marketplace`), and `claude plugin update`
exits 1:

```
Checking for updates for plugin "dwprobe" at user scope…
✘ Failed to update plugin "dwprobe": Failed to clone repository for git-subdir
source: Cloning into '<cache>/temp_subdir_<n>.clone'...
fatal: Remote branch dwprobe--v0.2.0 not found in upstream origin
```

The install is untouched — same `installPath`, same `version`, same
`gitCommitSha`, same bytes on disk. A creator who already has the page keeps
working through the interval; only a NEW install is refused, and a refusal is
what they can act on.

**And what happens when the release closes the interval**, measured on the same
rig, so the sentences §5 rests on are not assumed. The tag written at a commit
whose `plugin.json` states a higher version: `✔ Plugin "dwprobe" updated from
0.2.0 to 0.3.0 for scope user. Restart to apply changes.` — `installPath` moves,
`gitCommitSha` becomes the tagged commit, and the bytes on disk are that
commit's. The entry's `ref` moved to a NEW tag at a NEW commit whose version is
unchanged: `✔ dwprobe is already at the latest version (0.2.0).`, exit 0, and
nothing moves. And the transition §5 describes — an existing RELATIVE-PATH
install, the entry then becoming a `git-subdir` source at a tag carrying a
higher version — moves on one `marketplace update` plus one `plugin update`:
`✔ Plugin "dwt" updated from 1.0.0 to 1.1.0`, with the tagged commit's bytes on
disk.

The interval is a cost this record accepts and bounds. The merge of a
pull request that names a tag is followed by the dispatch of the release on
its merge commit as the next act; the interval is the release's own duration.
ADR-0028 §4 stands as written — the release is a human's dispatch, no merge
publishes, no pull request waits on a release — and this is the one merge
after which `main` names something the release has not yet made, said here
rather than discovered.

The alternative that removes the interval is the plugin release's own shape
(ADR-0028 §5): the engine release gains a `prepare` job that writes the
pin-moving commit itself, fast-forwards `main` to it, and tags it seconds
later. It is not taken now, because the walk of the page against the engine is
a pull request's work and a workflow cannot do it; it is the first revisit
trigger.

### 5. What an install that already exists receives

An existing install holds a page from a `main` commit under the relative-path
entry, at `plugin.json` version `1.4.3`, pinning `70eea629` and the `v1.5.0`
shelf — a consistent pair, and it stays one: nothing here touches what a
creator already holds.

Under this record, a page reaches an existing install when the entry's `ref`
moves to a tag whose `plugin.json` `version` is above the one held; an
unchanged version is not an update, as measured. So a page that is meant to
reach existing creators is tagged from a `main` that already carries the
version bump, and ADR-0028 §5's plugin release is what supplies it: the plugin
release is dispatched, then the pull request of §3 step 1, then the engine
release — or the pull request first and the plugin release second, as long as
both precede the dispatch. A plugin release with no engine release after it
reaches nobody on its own; that is what ADR-0028 §3's first paragraph asserted
of a page edit under an unchanged version, now true of the version bump too,
and it is written in the Consequences as a cost.

For the transition itself: the first tagged page carries a version above
`1.4.3`, so the first `marketplace update` followed by `plugin update` after
the first engine release moves every existing default-branch install from the
relative-path page at some `main` commit to the page at `delvec--v1.6.0`. A
creator who added the marketplace at a tag (`@delvewright--v<version>`) stays
there through `marketplace update`, as ADR-0028 measured; under this record
the tag to add for a fixed page is the engine's, `@delvec--v<version>`, and
the entry inside it names itself.

### 6. The gate: one pair, one tree

`tools/check-skill-page.py` judges the page against the engine at
`[engine].ref`, as it does today; what changes is what the ref is and what the
page's own tree is to it:

- **Offline**, `[engine].ref` matches the `delvec` arm of the release-tag
  grammar; the entry is a `git-subdir` source whose `url` names `[engine].repo`,
  whose `path` is the plugin root, whose `ref` equals `[engine].ref`, with no
  `sha` and no `version`; the release's number is the tag's version and equals
  `[workspace.package] version` at the tree the tag names.
- **Rule 21 holds one tree**: every engine path the page names exists in the
  tree at `[engine].ref` — the tree the page ships with. In §3's step 1 that is
  this tree, and the two halves the preamble names today are one rule. After
  the release, `main`'s tip is judged against the tagged tree, and a page that
  names a path the tagged engine lacks reds until the pull request that names
  the next tag carries it — which is the pull request in which that path
  belongs.
- **`--online`** has two states, decided by the object. The tag does not exist:
  the name equals this tree's own tag, which is the state in which the release
  is what creates it, and the shelf half cannot be asked yet. The tag exists:
  it points at a `main` commit whose `[engine].version` is the tag's version,
  whose Release carries one archive per `[engine].targets` at that commit plus
  `SHA256SUMS`, and whose plugin root is the page that ships; the gate prints
  how many files this tree's page differs from it by, as information. A tag
  that exists and points at a commit not on `main`, or whose Release is
  missing or partial, is a red as today.

`ci.yml`'s `manifest consistency (versions.toml)` job runs the pinned Claude
Code's `plugin validate --strict` over `marketplace.json` already; with the
entry a `git-subdir` source, that is the real implementation of the format
reading the shape this gate reads, the pair the constitution asks for, with no
change to the step.

### 6a. How CI gets the pinned engine, in both states

Three jobs materialise the pinned engine, and each reaches it through the one
composite action `.github/actions/skill-page-objects`: `manifest consistency
(versions.toml)`, `i18n translation tool (pytest)` and `content pin (drift +
zone-program audit)`. The action reads `[engine].ref` out of the page's
`versions.toml` at run time — never a literal — and fetches it.

**The tag exists.** The action fetches it by an explicit refspec,
`git fetch --no-tags --quiet origin "refs/tags/<tag>:refs/tags/<tag>"`, and
prints the commit. Measured on git 2.54.0: `--no-tags` turns off automatic tag
FOLLOWING and does not suppress a tag a refspec names, so the ref lands in
`refs/tags/` and `git archive <tag>` resolves afterwards; exit 0, silent. The
bare spelling `git fetch --no-tags origin <tag>` also exits 0 and leaves the
object in `FETCH_HEAD`, but writes no local tag, so the name does not resolve
later — which is why the refspec is explicit.

**The tag does not exist.** The fetch exits 128 with `fatal: couldn't find
remote ref refs/tags/<tag>` and writes nothing, in either spelling. That is not
a failure of the job: the action reports the state and continues, because the
engine the gate judges is then this tree, which is the tree the release
dispatched on this merge commit would tag — the same answer
`tools/check-skill-page.py`'s `resolve_ref` gives, from the same reading of the
same object. Without that arm the pull request that NAMES a tag could never be
green, since the tag it names is created by the release dispatched on its own
merge commit: the ordering of §3 would be unsatisfiable inside CI. So the
interval of §4 has a second face, stated here rather than discovered — a fresh
install is refused, and CI judges the tree that is about to become the tag.

### 7. Every consumer the change moves

Each verified in the tree at `227e7504`; a consumer found later is this
record's defect.

| consumer | what it does with the ref or the entry today | what moves |
|---|---|---|
| `.claude/skills/delvewright/skills/new-delve/versions.toml` | `release = "v1.5.0"`, `ref = <40-hex>` | `ref` is the tag; `release` is removed |
| `.claude-plugin/marketplace.json` | `"source": "./.claude/skills/delvewright"` | a `git-subdir` source: `url`, `path`, `ref`; no `sha`, no `version` |
| `tools/check-skill-page.py` | `REV_RE` (40-hex) and `RELEASE_RE` (`v<semver>`) in rule 2; rule 3 takes the number from `release`; rule 12 demands a relative-path string `source`; rule 13 `--online` resolves `release` to `ref`; rule 18 fetches the shelf by `release`; rule 21's instrument is the tree it runs in, and its preamble says so; `materialise()` serves `rev^{commit}`; rule 11's docstring says the marketplace delivers what `main` carries | one name, the grammar of `release_tags.py` imported and not copied; rule 12 reads the four fields; the two-state `--online`; rule 21 over the tree at the tag; the docstrings restated |
| `tools/tests/test_check_skill_page.py` | fixtures asserting "not a full 40-hex revision" and the `["engine"]["ref"]` extraction | follow the rules above |
| `.github/actions/skill-page-objects/action.yml` | `git fetch --no-tags --quiet origin "$REF"` by sha | a tag is fetched as `refs/tags/<tag>:refs/tags/<tag>`; when the tag is unborn the engine is this tree |
| `.github/pins.toml` | `skill-page-engine` `value` is the revision; the `release` policy's text says "a commit a `v<semver>` tag points at" in three places | the value is the tag name; the policy's text says a release tag of the thing the entry names, existing or this tree's own unborn one |
| `tools/check-pins.py` | discovery finds a 40-hex literal in the site (`RE_REV`); `--online` `cat-file -e value^{commit}`, then `tag --points-at value` filtered by `v<semver>` | the entry moves to the registry's `bound_by` arm, with `check-skill-page.py` as the binder and `engine.ref` as the key, and `--online` judges a release tag: the tag exists and its commit's tree states the tag's version, or it is absent and equals this tree's own tag, through `release_tags.py`'s grammar. **Corrected at finalisation**: the draft said discovery would find the literal by that grammar. Measured against this tree's fetch sites, twelve distinct `<name>--v<semver>` literals stand in them and eleven are fixtures of `tools/tests/test_release_tags.py` and examples in `tools/lib/release_tags.py`'s own docstring, so a shape scan would report eleven pins nobody fetches. A tag name therefore carries no shape the scan can separate from data, which is the exact condition the `bound_by` arm exists for — and it is the stronger outcome, because the binder is what holds the pin and the marketplace entry to one name |
| `.claude/skills/delvewright/skills/new-delve/scripts/fetch-delvec.py` | `DOWNLOAD` and `ARCHIVE` are formatted from `release`; `expected = release.lstrip("v")`; `engine_targets()` runs `git show <ref>:versions.toml` | URL from the tag, archive name `delvec-v<version>-<target>.tar.gz` from the tag's version (the archive grammar is unchanged, ADR-0028 §2); `git show` at a tag name once I2 has fetched it |
| `.claude/skills/delvewright/skills/new-delve/scripts/check-toolchain.py` | compares `rev-parse HEAD` to `ref` as strings; `want = release.lstrip("v")` | compares to `rev-parse <ref>^{commit}`; the version from the tag |
| `.claude/skills/delvewright/skills/new-delve/references/init.md` (I2) and `SKILL.md` (I1b, I2 rows) | `checkout --detach "$ENGINE_REF"` then `[ "$(rev-parse HEAD)" = "$ENGINE_REF" ]`; the rows name `[engine].release` | the fetch names the tag; the equality is against `rev-parse "$ENGINE_REF^{commit}"`; the rows name one key |
| `tools/release-notes.py` (`delvewright` arm) | prints `pin['release']` and `pin['ref']`, and reads the engine table at `ref` | prints the one name; an unborn tag is printed as unborn, not raised, so a plugin release dispatched inside §4's interval still writes its notes |
| `docs/reference/skill-workflow.md` | "A newer page reaches a creator when `plugin.json` `version` moves on `main` — the marketplace serves the default branch" | the marketplace serves the tag the entry names; a newer page reaches a creator when the entry moves to a tag carrying a higher version |
| `docs/reference/tools.md` | the rows for `check-skill-page.py`, `check-pins.py`, `fetch-delvec.py`, `check-toolchain.py` | restated |
| `docs/specs/spec-0063-the-front-end-as-a-product.md` §8 and its criteria | "`release` (`v<semver>`), `ref` (40-hex)"; the rule table's pin row; criterion 4 | restated to one name under the tag grammar; criterion 4's rewrite IS a loosening and is declared in those words, with the narrower online assertion that replaces it in the unborn state |
| `docs/reference/front-end-standard.md` §3a | `git-subdir` listed as taking `(url, path)` | the four documented fields, quoted, and the two silences this record's measurement closed — **found after the draft, and therefore this record's own defect**, named here rather than left to be discovered |
| `tools/tests/test_fetch_delvec.py`, `tools/tests/test_check_toolchain.py`, `tools/tests/test_check_pins.py`, `tools/tests/test_release_tags.py` | fixtures building a pin from `release` + a 40-hex `ref`; no `release`-policy online test | tag-shaped pins, a real tag written into the rig's own engine checkout, the `release` policy's two arms perturbed, and the shipped copy of the grammar held to the module's answer — **found after the draft**, for the same reason as the row above: §7 named one test file where five hold these rules |

Not moved: `plugin-release.yml` and rule 11 (the version still moves only in
the plugin release); `engine-release.yml` (it already tags an existing `main`
commit); `tools/build-release-binaries.sh` and the archive grammar; the content
repository, whose `engine-release` pin names a commit and is not this page's.

## Consequences

- **The order, made structural**: this record's implementation is one pull
  request, and it is also the first pull request of §3 step 1 — it names
  `delvec--v1.6.0`, because a pin checker that accepts the tag grammar cannot
  land while the pin names `v1.5.0`, and none accepts both grammars (ADR-0028
  §7). It therefore carries what ADR-0028's Consequences called PR B, for this
  repository: `check-pins.py`, the `release` policy text, `check-skill-page.py`
  and `fetch-delvec.py` read the new grammar; spec-0063 §8 and criterion 4 are
  re-stated. Its `--online` runs in the unborn state. The engine release is
  dispatched on its merge commit as the next act. A plugin release precedes
  that dispatch (§5), so that the first tagged page is an update to every
  existing install.
- **What this record commits the project to**, the first time each can bind:
  one engine release dispatched directly after the merge that names it, every
  time the page's pin moves; one plugin release before any engine release whose
  page must reach existing creators; and the interval of §4, in which a fresh
  install is refused rather than served.
- **Costs accepted, named**: the second authority on `main` (ADR-0028 §3's
  reason two), bounded by the gate; the interval of §4; one name in two files,
  held equal by the gate; a plugin release that delivers nothing until an
  engine release follows it.
- **Checks that change**: `tools/check-skill-page.py` rules 2, 3, 12, 13, 18
  and 21 and their tests; `tools/check-pins.py`'s `release` policy and the arm
  the entry sits on; `.github/actions/skill-page-objects/action.yml`; the two
  creator-side scripts; `tools/release-notes.py`. No required status context is
  added or renamed.
- **Rule 21 holds the pair whole**, one arm per tree, each with its own binding
  count and the same denominator: 37 of 41 engine paths in the tree the page
  ships from, and 37 of 41 in the tree the pin names. The arm is not merely
  true: removing `validation/chunky.sh` from the tree the pin names reds the PIN
  arm alone (32 of 37 present) and leaves the shipping arm at 33 of 37, and the
  same holds for `validation/chunky-install.sh`. Put to the tree the OLD pin
  named, `70eea629`, the new arm reds on exactly those two paths — the defect it
  exists for, reproduced rather than described.
- **Checks that do not change**: the `plugin validate --strict` step over
  `marketplace.json` in `ci.yml`'s `manifest consistency (versions.toml)` job;
  rule 11; the release workflows; the tag ruleset of
  ADR-0028 §9, which already covers `refs/tags/*--v*`.
- **The docs**: `docs/reference/skill-workflow.md`, `docs/reference/tools.md`,
  spec-0063 §8, the `why` of `skill-page-engine` in `.github/pins.toml`, and
  rule 21's preamble, in the same pull request. `ACKNOWLEDGEMENTS.md` gains
  nothing.
- **The measurements this record owed at finalisation are in §4**, each in the
  tool's own words and each on Claude Code 2.1.265: the refusal a fresh install
  meets inside the interval; what `plugin update` does inside it; that
  `plugin update` moves an existing install when the entry's `ref` moves to a
  tag carrying a higher `plugin.json` version and does not when the version is
  unchanged; and the relative-path-to-`git-subdir` transition §5 describes. None
  contradicts a sentence of §4 or §5, so the last revisit trigger below did not
  fire. Nothing was published to make them: a throwaway marketplace on local
  smart HTTP, plus the real entry shape put to the real repository at `v1.5.0`.
- ADR-0028's first revisit trigger has fired and is closed by this record; its
  other four stand.

## Revisit triggers

- A creator is refused inside §4's interval, or the interval is ever longer
  than one release's run: the engine release gains ADR-0028 §5's `prepare`
  job and writes the pin-moving commit itself, and the walk of the page moves
  to the pull request before it.
- The plugin release proves inert as a delivery in practice — a version bump
  that waits on an engine release nobody has reason to dispatch: the version's
  home is re-decided, with the documented commit-SHA versioning (no `version`
  in `plugin.json` or the entry, every tag move an update) costed against
  ADR-0016 item 3 and ADR-0028 §5.
- Claude Code documents what a `git-subdir` entry does with a `ref` the remote
  lacks, or gains a way for the entry to name "the tag this commit will carry"
  without a literal: the second file of §2 is re-costed.
- A second plugin joins the marketplace: §1's entry shape is applied per
  plugin, and whether each pins its own engine tag is decided then.
- The version this record's own pin names stops being the version the tree
  states. The pin is `delvec--v1.6.0` and the engine version moves to `1.6.0`
  in its own round; until that round lands, both gates red naming the
  disagreement, which is the ordering made structural rather than a defect.
