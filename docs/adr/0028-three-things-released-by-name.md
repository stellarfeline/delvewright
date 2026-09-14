# ADR-0028: Three things, each released by its name and its version — `delvec`, `delvewright-dsl` and the `delvewright` plugin

- **Status**: Proposed
- **Date**: 2026-09-13
- **Source**: the ruling that the engine repository publishes three
  independently versioned things — the `delvec` binary, the `delvewright-dsl`
  crate and the `delvewright` Claude Code plugin — each as a GitHub Release
  whose tag is the thing's name plus its version, each published by CI, and
  that the first plugin release is started by hand through the release
  workflow's manual arm rather than by a pushed tag. Measured against the
  engine tree at `70eea6296cfab2440054f95670729081c3d4bca1` (the `v1.5.0`
  commit), the content repository at `73182027db05e14d3a52a39e0c30fabd9025b9c0`,
  the remote's tags and releases, the Claude Code documentation pages named in
  §Context, and a throwaway marketplace exercised with Claude Code 2.1.270.
- **Refines**: ADR-0016 (three version lines: the lines stand, their release
  identities change), ADR-0025 (two crates: both now have a release), ADR-0026
  §1 (one approval, every platform: now applied to each of the three), ADR-0027
  §2 (the plugin carries the page and its pin: the pin now names a release
  under this grammar).
- **Supersedes, in part**: ADR-0016 §Decision item 2, its parenthesis
  "(`v<semver>`)" and nothing else of the item; ADR-0016 §Decision item 3, in
  that the product's version is not only "declared in the skill itself" — it
  is declared in `plugin.json` and released; ADR-0017 §4, its
  first sentence ("fires on a `v<semver>` tag"); ADR-0026 §4, whose open
  question is closed here by its third option, with the shelf filled rather
  than empty. ADR-0017 §5 stands as the record of the one-time `v1.0.0` move
  and of why a filled release never moves again.

## Context

### What the repository releases today, and how

Three things carry a version and reach a consumer by three different paths
(measured at `70eea629`; every figure below was re-derived, none taken from
the brief that commissioned this record):

| thing | its number lives in | how it reaches a consumer | its release today |
|---|---|---|---|
| `delvec` | `versions.toml [engine].version` = `1.5.0` | the archive on the GitHub Release (`fetch-delvec.py`), or `cargo install delvec` | `.github/workflows/engine-release.yml`, on a pushed tag matching `v[0-9]+.[0-9]+.[0-9]+`, with a `workflow_dispatch` arm that takes an existing `v<semver>` tag; six releases `v1.0.0`…`v1.5.0`; crates.io holds `delvec` 1.1.0, 1.3.0, 1.4.0, 1.5.0 |
| `delvewright-dsl` | `versions.toml [engine].dsl_crate_version` = `0.25.0` | crates.io, resolved by `delvec`'s `=` requirement | `.github/workflows/dsl-crate-publish.yml`, on a push to `main` that moves the number; no tag, no GitHub Release; crates.io holds 0.25.0 as its newest |
| the `delvewright` plugin | `.claude/skills/delvewright/.claude-plugin/plugin.json` `version` = `1.4.2` (`1.4.3` on `main` at `93c9802e`, merged after this tree was cut) | `/plugin marketplace add stellarfeline/delvewright` then `/plugin install delvewright@delvewright`; the marketplace is `.claude-plugin/marketplace.json` at the root, one entry, `"source": "./.claude/skills/delvewright"` | none: no tag, no Release, no workflow |

The remote carries seven tags: `v1.0.0`, `v1.1.0`, `v1.2.0`, `v1.3.0`,
`v1.4.0`, `v1.5.0` (each with a published Release named `delvec v<semver>`,
the newest carrying five archives plus `SHA256SUMS`, marked Latest) and
`archive/bell-engine-r1`, which is not a release. The GitHub Release titled
`v1.5.0` is the repository's single Latest.

### The census of the `v<semver>` shape

A regex family (`v[0-9]+\.[0-9]+\.[0-9]+`, `v<semver>`, `refs/tags/v`,
`tags/v`, `v\[0-9\]`) over every tracked file matches **45 of 1432** files in
the engine tree and **7 of 285** in the content tree. Reading each match
(the constitution's rule: a `grep -c` counts mentions, not obligations), the files in
which the shape is load-bearing — parsed, built into a URL or a name, or held
by a check — are these; everything else in the 52 is narrative, a fixture's
incidental literal, or the content repository's own campaign tag grammar
`release/<campaign>/v<semver>` (spec-0024 §1), which this record does not
touch:

| consumer | what it does with the shape | fate under §Decision |
|---|---|---|
| `.github/workflows/engine-release.yml` | trigger `tags: ["v[0-9]+.[0-9]+.[0-9]+"]`; `identity` refuses `TAG != v$VERSION`; the dispatch input names "an existing `v<semver>` tag"; the Release is titled `delvec $TAG` | trigger and identity move to `delvec--v`; the dispatch input follows |
| `.github/pins.toml` | the `release` policy is defined as "a commit a `v<semver>` tag points at" (three statements of it) | the policy names the thing whose tag must point at the commit |
| `tools/check-pins.py` (engine; the content copy is the same code) | `re.fullmatch(r"v\d+\.\d+\.\d+", tag)` over `git tag --points-at <pin>` | matches `<thing>--v<semver>` for the thing the entry names |
| `tools/check-skill-page.py` | `RELEASE_RE = ^v\d+\.\d+\.\d+$`; `ARCHIVE = "delvec-{release}-{target}.tar.gz"` built from the pin literal; `release.lstrip("v")` compared with the engine's number; `--online` resolves `git/ref/tags/{release}` and `releases/tags/{release}` | the pin names a `delvec--v` tag; the version is derived from it once; the archive name is derived from the version |
| `.claude/skills/delvewright/skills/new-delve/scripts/fetch-delvec.py` | `DOWNLOAD = …/releases/download/{release}`; `ARCHIVE = "delvec-{release}-{target}.tar.gz"`; `release.lstrip("v")` | same derivation as the gate that judges it, stated once in each |
| `.claude/skills/delvewright/skills/new-delve/versions.toml` | `release = "v1.4.0"` (the brief's observation holds: the page pins `v1.4.0` → `d8d87ef6` while `v1.5.0` → `70eea629` is published; under the `release` policy that drift is not a finding) | re-pinned to the first `delvec--v` release, in the pull request that walks the page against it |
| `tools/tests/test_fetch_delvec.py`, `tools/tests/fixtures/skill-page/SHA256SUMS-v1.4.0`, `tools/tests/fixtures/skill-page/README.md` | fixtures naming `v1.4.0` archives | follow their tools |
| `tools/tests/fixtures/release-publish-gate/*.yml` (5 files) | copies of the old trigger line; `tools/check-release-publish-gate.py` reads jobs, not `on:`, so these bind nothing | rewritten with the new trigger so a fixture is not a stale copy of a workflow |
| content `.github/pins.toml` (`engine-release`, policy `release`, value `70eea629`) and content `tools/check-pins.py` | the same policy text and the same regex | the same change, in the content repository, when it next re-pins |
| `tools/build-release-binaries.sh` | names archives `delvec-v$VERSION-$t.tar.gz` from `[engine].version`, never from the tag | unchanged: the archive grammar stays `delvec-v<version>-<target>.tar.gz` |

Two stale statements found on the way, recorded here because a census is where
they surface: `engine-release.yml` line 100 says "`rc-*` belongs to
`release.yml` (tier 3)", and no `release.yml` exists in this repository (it is
the content repository's); the content repository's `versions.toml` comment
above `[engine].ref` narrates "Engine release `v1.1.0`" over a value that is
`v1.5.0`'s commit.

### Whether a plugin release decides what a creator receives

The documentation, read at the pages themselves (`plugin-marketplaces.md`,
`discover-plugins.md`, `plugins-reference.md`, `plugin-dependencies.md` under
`code.claude.com/docs/en/`), says: a marketplace added as `owner/repo` is a
clone of the default branch, and one added with `@ref` (or `#ref` on a git
URL) "updates to the latest commit of that ref, not the repository's default
branch"; a relative-path plugin source carries no `ref` or `sha` field; the
plugin's version is resolved from `plugin.json` `version` first, then the
marketplace entry's, then the commit SHA, and "if the resolved version matches
what a user already has, `/plugin update` and auto-update skip the plugin";
setting `version` "pins the plugin … push new commits without changing that
string, existing users … keep the cached copy"; and the ecosystem's release
tag is `{plugin-name}--v{version}`, written by `claude plugin tag`, read by
dependency-range resolution, and nowhere tied to GitHub Releases.

The documentation is silent on the one question that matters here — whether a
tag on a relative-path plugin changes what a default-branch subscriber gets —
so it was settled by doing it. A throwaway marketplace (`dwtest-mkt`, one
relative-path plugin `dwtest`, under this machine's scratch directory, served
over local smart HTTP because the CLI refuses `file://` and `git://` sources
and dumb HTTP lacks the shallow capability the CLI's clone asks for; never
pushed anywhere) carried commits A (`0.1.0`), B (`0.1.0`, content changed),
C (`0.2.0`, tagged `dwtest--v0.2.0`), D (`0.2.0`, content changed, branch
tip). With Claude Code 2.1.270:

- `claude plugin marketplace add http://127.0.0.1:8765/mkt.git` then
  `claude plugin install dwtest@dwtest-mkt` installed **D** — `installed_plugins.json`
  recorded `version 0.2.0`, `gitCommitSha 3fb884a` (D), and the skill file
  carried D's marker. The tag at C decided nothing.
- E pushed (content changed, version still `0.2.0`), `marketplace update` then
  `plugin update`: "`dwtest is already at the latest version (0.2.0)`", D's
  bytes kept. F pushed (`0.3.0`): "`updated from 0.2.0 to 0.3.0`", F's bytes.
  **The version string is the delivery event; a commit without a bump is
  invisible to a subscriber.**
- The marketplace removed and re-added as `…/mkt.git#dwtest--v0.2.0`: the clone
  stood detached at C, `known_marketplaces.json` recorded `"ref":
  "dwtest--v0.2.0"`, the install was C (`e6370d9`), and after G (`0.4.0`) was
  pushed to `main`, `marketplace update` left the clone at C and `plugin
  update` reported `0.2.0` current. **A tag decides only for a subscriber who
  added the marketplace at that tag.**
- `claude plugin tag --dry-run` on the same plugin printed `Tag:
  dwtest--v0.3.0` and the two `git` commands it would run, confirming the
  convention on the real CLI.

So for the marketplace this repository ships — relative-path source, added at
the default branch — **a plugin release does not decide what a creator
receives; the merge that moves `plugin.json` `version` on `main` does.** The
release can only record that delivery, and it is worth having only if it
proves that what it records is what was delivered.

## Decision

### 1. One tag grammar for the three: `<name>--v<major>.<minor>.<patch>`

A release tag is the thing's name, the separator `--v`, and a strict semver
triple with no prerelease and no build metadata: `delvec--v1.6.0`,
`delvewright-dsl--v0.26.0`, `delvewright--v1.4.3`. The regular expression, one
per workflow trigger and one in each checker, is
`^(delvec|delvewright-dsl|delvewright)--v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$`,
with the name fixed per workflow. The names are the ones the things already
carry where a consumer resolves them: the crate names on crates.io and the
plugin's `name` in `plugin.json`, which is the namespace a creator types
(`/delvewright:new-delve`).

`--v` rather than `-v` or `/v` because the plugin's tag is dictated —
`{plugin-name}--v{version}` is what `claude plugin tag` writes and what
dependency resolution reads, and the documentation gives the reason (a
double-hyphen separator is a prefix match that survives hyphenated names, which
`delvewright` versus `delvewright-dsl` needs as much as any) — and one grammar
for three things beats two grammars for the sake of a prettier engine tag.
The Release title is the tag with `--v` read as a space (`delvec 1.6.0`,
`delvewright-dsl 0.26.0`, `delvewright 1.4.3`); the first line of every
Release's notes says which of the three it is.

### 2. What each release is, carries and proves

| | `delvec` | `delvewright-dsl` | `delvewright` plugin |
|---|---|---|---|
| starts on | a pushed `delvec--v*` tag; dispatch re-fills a draft (as today) | a push to `main` that moves `[engine].dsl_crate_version` (as today); dispatch is the remedy arm (as today) | a pushed `delvewright--v*` tag; dispatch writes the tag (§4) |
| who writes the tag | a human, the release act | the publishing job, after the registry upload (§6) | a human after the first; the manual arm for the first and as the remedy |
| identity, refused before anything builds | tag == `delvec--v` + `[engine].version` at the tagged commit; ancestor of `main` (as today) | the tag the job will write does not exist yet; the commit is on `main` | tag == `delvewright--v` + `plugin.json` `version` at the tagged commit; ancestor of `main`; §3's tree-identity over every `main` commit carrying that version |
| assets | one archive per `[engine].targets` plus `SHA256SUMS` (as today; the archive grammar `delvec-v<version>-<target>.tar.gz` is unchanged) | `delvewright-dsl-<version>.crate` — the tarball `cargo package` produced and the registry received, byte-identical, which `tools/crates-io-publish.sh` already hashes — plus `SHA256SUMS` | `delvewright-plugin-<version>.zip`, `git archive --format=zip <tag>:.claude/skills/delvewright` (reproducible by anyone from the tag), plus `SHA256SUMS` |
| notes | generated from the previous `delvec--v*` tag, prefixed with the `dsl_version` the binary speaks and the crates.io link, read from the tree | generated from the previous `delvewright-dsl--v*` tag, prefixed with the crates.io link | generated from the previous `delvewright--v*` tag, prefixed with the engine release the page pins and its `ref`, the `requires_delvec` window, the `dsl_version` that engine speaks, and the plugin root's tree hash — every value read from the tree at the tag, none typed |
| the two platforms | crates.io then the undraft, one approval (ADR-0026) | crates.io then the tag and Release, one approval (§6) | the Release, one approval (§5) |
| Latest | yes: `make_latest=true` | no: `--latest=false` | no: `--latest=false` |

Generated notes run between consecutive tags **of the same line** (`git tag -l
'<name>--v*' --sort=-v:refname`), never between the repository's last two tags,
which would interleave the three histories.

### 3. The plugin release records what `main` delivered, and proves the two are one thing

`tools/check-skill-page.py`'s version-bump rule already holds that a pull
request touching anything under the plugin root moves `plugin.json` `version`.
The consequence, stated so the release can lean on it: **every commit on
`main` that carries one plugin version carries a byte-identical plugin root.**
The plugin release enumerates `main`'s first-parent history, collects every
commit whose `plugin.json` `version` equals the tag's version, and refuses
unless the plugin root's tree hash (`git rev-parse <commit>:.claude/skills/delvewright`)
is one value across all of them — printing the count as its binding (`N
commits carry 1.4.3, 1 distinct tree`). A tag may therefore point at any of
those commits; the archive is the same bytes whichever it is, and a creator who
received `1.4.3` from the marketplace received exactly the archive's contents.

Before it publishes, the plugin release also proves what the page promises:
`tools/check-skill-page.py --online` at the tagged commit is green — the pinned
`delvec--v*` tag exists, resolves to `[engine].ref`, and its Release carries an
archive per target at that revision plus `SHA256SUMS`. A plugin release whose
engine shelf is partial is refused, because that is the state in which Init
falls to the source build on exactly the platforms nobody tested.

The alternative — making the release *decide* delivery by turning the
marketplace entry into an `archive` source (URL plus `sha256` of the Release's
zip) or a `git-subdir` source pinned by `sha` — is rejected. It needs a second
commit after every release to re-point the entry, puts a second authority for
the plugin's bytes on `main` beside the tree the developer's skills-dir load
already reads, and buys a staged delivery nobody asked for. A creator who wants
a fixed page has the measured path: add the marketplace at the tag
(`/plugin marketplace add stellarfeline/delvewright@delvewright--v1.4.3`),
which stays there through `marketplace update`.

### 4. The first plugin release is the manual arm writing the tag

The plugin workflow's `workflow_dispatch` takes one input, a commit on `main`
(default: `main`'s tip), and performs, in order: read `plugin.json` `version`
at that commit; refuse if `delvewright--v<version>` already exists; run §3's
identity and tree-identity; create the annotated tag at that commit through
the API (the job's `contents: write`), which is the one act the tag-push path
does by hand; then continue exactly as the tag-push path does — archive,
checksums, notes, the environment-gated publish. The first release is this arm
run against the commit `main` carries when the workflow lands; nothing before
it is tagged retroactively, so `1.4.2` and every earlier plugin version are
delivered history with no release, and the census of plugin releases starts
at the first tag.

### 5. Tag-driven releases after the first, and the version that must not go unreleased

After the first, a plugin release is a human pushing `delvewright--v<version>`
at a `main` commit carrying that version (the dispatch arm remains the remedy
path and does the same). Because `main` delivers on the bump and the tag lags
behind it, a version can be delivered and never released; the gate that keeps
the record complete is the one `tools/check-dsl-version-published.py` already
has the shape of: **on a pull request that moves `plugin.json` `version` from
X to Y, X has a published `delvewright--vX` Release**, or the pull request
reds and names the remedy (push the tag, or run the arm). The subject is the
outgoing number, for the reason that file gives: it is the moment X is
finished and the last moment anyone will ask about it.

The publish step of the plugin release runs in a job that declares an
environment of its own, `plugin-release`, holding no secret, with the owner as
required reviewer; its first step is `tools/assert-run-approved.sh
plugin-release`. This is not a second door for one decision: the tag push says
*which* commit, the approval says *publish it*, and it is what keeps
`tools/check-release-publish-gate.py`'s rule — a publishing act lives only in
a gated job — intact with no exemption, and `tools/check-approval-guard.py`
covering the new job by object class with no edit. The `crates-io` environment
is not reused, because a job that declares it can read the registry token and
this job has no use for one.

### 6. The format crate's release is written by the job that publishes it

`delvewright-dsl` keeps its clock (ADR-0026 §4: a format number is resolvable
the moment a document declares it, so the crate does not wait for a human
tag). What changes is that the gated `publish` job of `dsl-crate-publish.yml`,
after `tools/crates-io-publish.sh --publish --only delvewright-dsl` returns
with the index serving the bytes, creates `delvewright-dsl--v<version>` at the
commit it ran from and the Release with the `.crate` and `SHA256SUMS`,
`--latest=false`. The irreversible act is first and the residual window is
one call wide, the same argument as ADR-0026 §3; a re-run finds the version on
the registry, skips it, and writes the tag and Release if they are missing.
This is ADR-0026 §4's third option — "a tag and an assetless release per format
bump" — taken, with the shelf not empty: the asset is the tarball the registry
holds, and the checksum beside it is what lets anyone prove the two are one.

With that, the engine release's treatment of the format crate changes from
"no-op re-check, or supply it if the hook never ran" to **refuse**: the
`delvec` release runs `tools/crates-io-publish.sh --publish --only delvec`
after a preflight that the registry already serves `[engine].dsl_crate_version`
byte-identically, and if it does not, the remedy it names is the format
crate's own release (its dispatch arm). No path remains by which a
`delvewright-dsl` version reaches crates.io without its tag and Release.

### 7. The six existing tags stay as they are, and nothing else is ever tagged `v<semver>`

`v1.0.0`…`v1.5.0` and their Releases are published history with archives in
the wild (`fetch-delvec.py` at any page pinned to them builds
`releases/download/v1.4.0/…`); ADR-0017 §5 already says a filled release never
moves. They are neither deleted, renamed, re-tagged under the new grammar, nor
given twin tags. The new trigger does not match them, so no run can ever be
started against one again, which is right for a published release. No pin
checker accepts both grammars: the engine's next release is `delvec--v1.6.0`,
the page re-pins to it in the pull request that walks the page against it,
and the content repository re-pins `engine-release` to it when it next adopts
an engine — each re-pin changes the checker's expected shape and the pin in
one pull request, with no window in which `v1.5.0` must still parse.

### 8. One Latest, and it is `delvec`

GitHub marks one release per repository Latest ("the most recent
non-prerelease, non-draft release, sorted by the `created_at` attribute",
which is the commit date). With three lines in one repository that marker
would flip between a plugin release and an engine release on whichever commit
was newer. It is therefore set explicitly by every release and never left to
the default: the `delvec` release undrafts with `make_latest=true`; the other
two publish with `--latest=false`. `delvec` owns it because it is the thing a
stranger arriving at the repository page downloads; the crate is found through
the registry and the plugin through the marketplace. No tool in either
repository resolves anything through `releases/latest`, and none may start to.

### 9. Immutability by ruleset

ADR-0017 §5 made a filled release immutable "in practice". A tag ruleset on
`refs/tags/*--v*` and on the six `v*` tags — forbidding update and deletion,
restricting creation to the repository's owner — makes it so by construction,
in the way the branch ruleset already protects `refs/heads/feature/**`. It is
a GitHub setting outside the tree, so it is proposed here and written by the
planner from the read-back state, proved by attempting the forbidden act.

## Consequences

- **Checks that change**: `engine-release.yml`'s trigger, identity message,
  dispatch input and Release title; `dsl-crate-publish.yml` gains the tag and
  Release steps in its gated job; a new `plugin-release.yml` with the two
  entry points of §4–§5; `tools/check-pins.py` (both repositories) matches
  `<thing>--v<semver>` for the thing the registry entry names, and the
  `release` policy's text in both `.github/pins.toml` files says so;
  `tools/check-skill-page.py` derives the version from a `delvec--v` pin once
  and the archive name from the version, offline and `--online`;
  `fetch-delvec.py` does the same derivation; a new outgoing-plugin-version
  gate in `ci.yml` (§5) beside `check-dsl-version-published.py`; the
  release-publish-gate fixtures carry the new trigger. Every one states its
  binding count.
- **Checks that do not change**: `tools/check-release-publish-gate.py` and
  `tools/check-approval-guard.py` (the plugin job is one more environment-gated
  job under a rule stated by object class); `tools/build-release-binaries.sh`
  and the archive grammar; the content repository's `release.yml`, which checks
  the engine out by commit and never by tag.
- **The docs**: `docs/reference/tools.md` rows for the three checkers and the
  fetch script; `docs/reference/skill-workflow.md`'s account of how a newer
  page arrives gains one sentence — it arrives when the version moves on
  `main`, and the release is the record of it; spec-0063 §8 and its criterion
  4 name the pin's shape as `v<semver>` and are re-stated by the implementing
  pull request against this grammar; `ACKNOWLEDGEMENTS.md` gains nothing (no
  library is adopted).
- **The order the implementation lands in**, made structural rather than
  narrated: the engine release under the new grammar must exist before the
  page can pin it, so the implementing pull request lands the three
  workflows and checkers with the page still pinned to `v1.5.0` under a
  checker that accepts only `delvec--v` — which reds — and is not mergeable
  until `delvec--v1.6.0` is published from a tree that carries this record;
  the re-pin and the plugin's version bump ride the same pull request; the
  first plugin release (§4) follows its merge; the content repository's re-pin
  is a separate pull request there, blocked on the same engine release.
- **What the owner is committed to** the first time each can bind: a second
  GitHub environment (`plugin-release`) whose reviewer rule must be saved and
  whose binding is proved by `assert-run-approved.sh` the way the first one's
  is; a tag ruleset (§9); one approval click per plugin release; and the
  knowledge that `1.4.2` and earlier plugin versions have no release.
- ADR-0026's revisit trigger "a third platform joins the release" does not
  fire: no line gains a platform; one line (the format crate) gains a second
  outlet under the approval it already had, which is §1 of that record applied
  as written.

## Revisit triggers

- Claude Code documents a way for a relative-path plugin source, or a
  default-branch marketplace, to pin a ref: §3's rejected alternative is
  re-costed, because the second commit it needs may disappear.
- A prerelease of any of the three is wanted: the grammar in §1 gains a
  prerelease suffix by an amendment naming which consumers parse it; until
  then a prerelease is refused at the trigger.
- The content repository's campaign tag grammar (`release/<campaign>/v<semver>`,
  spec-0024 §1) is re-examined for the same "name plus version" rule: that is
  a decision of its own and is not taken here.
- A consumer starts resolving a thing through `releases/latest`: §8 is the
  standing refusal.
