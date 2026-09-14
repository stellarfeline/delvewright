# ADR-0028: Three things, each released by its name and its version — `delvec`, `delvewright-dsl` and the `delvewright` plugin

- **Status**: Accepted — implemented; §2, §4, §6, §9 and *Consequences* amended to what was built
- **Date**: 2026-09-13
- **Source**: the rule that the engine repository publishes three
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
| `delvewright-dsl` | `versions.toml [engine].dsl_crate_version` | crates.io, resolved by `delvec`'s `=` requirement | `.github/workflows/dsl-crate-publish.yml`, on a push to `main` that moves the number; no tag, no GitHub Release; crates.io held the tree's number as its newest |
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
| identity, refused before anything builds | tag == `delvec--v` + `[engine].version` at the tagged commit, under the strict grammar (`tools/lib/release_tags.py`); ancestor of `main` (as before) | the commit is on `main`; a tag already written must name a `main` commit stating the version, and is never moved | tag == `delvewright--v` + `plugin.json` `version` at the tagged commit; the commit is itself a first-parent `main` commit carrying that version; §3's tree-identity over every such commit (`tools/check-plugin-release-identity.py`); no Release already exists at the tag |
| assets | one archive per `[engine].targets` plus `SHA256SUMS` (as before; the archive grammar `delvec-v<version>-<target>.tar.gz` is unchanged) | `delvewright-dsl-<version>.crate` — the registry's own tarball, downloaded and refused unless it hashes to the index sha256 (a local re-packaging from any later commit differs in `.cargo_vcs_info.json`) — plus `SHA256SUMS` | `delvewright-plugin-<version>.zip`, `git archive --format=zip --mtime=<the commit's committer time> <commit>:.claude/skills/delvewright`, plus `SHA256SUMS`; `--mtime` is what makes it reproducible from the tag, because git stamps the current time on every entry when archiving a tree (measured: two archives of the same tree three seconds apart hashed differently; with `--mtime`, runs a minute apart hashed identically) |
| notes | generated from the previous `delvec--v*` tag (for the first, the newest legacy `v<semver>` release), prefixed with the `dsl_version` the binary speaks and the crates.io link, read from the tree | generated from the previous `delvewright-dsl--v*` tag, prefixed with the crates.io link | generated from the previous `delvewright--v*` tag, prefixed with the engine release the page pins and its `ref`, the `requires_delvec` window, the `dsl_version` that engine speaks, and the plugin root's tree hash — every value read from the tree at the tag, none typed |
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
at that commit and derive the tag; run §3's identity and tree-identity; refuse
if a Release already exists at the tag (published, or a draft nothing in the
workflow created); prove the page's engine shelf (§3); archive, checksum and
write the notes; then, in the environment-gated job and after the approval,
create the annotated tag at that commit through the API (the job's
`contents: write`), which is the one act the tag-push path does by hand, and
the Release. The tag is written after the approval rather than before the
archive, so a run nobody approves leaves no tag behind. A tag that already
exists without a Release — a run that died between the two writes — is
accepted when it names the commit the run judged, so re-running either entry
point completes the release; a tag written with the job's own token starts no
second run. The first release is this arm run against the commit `main` carries
when the workflow lands; nothing before it is tagged retroactively, so every
earlier plugin version is delivered history with no release, and the census of
plugin releases starts at the first tag.

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
finished and the last moment anyone will ask about it. It is
`tools/check-plugin-version-released.py`, a step of the existing required job
`dsl crate version (crates.io)`; a Release counts only when it is published,
carries the zip and `SHA256SUMS`, and its tag resolves to a commit stating X.

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
with the index serving the bytes, downloads the registry's own `.crate`
(checked against the index sha256), creates `delvewright-dsl--v<version>` at
the `main` commit the run judged (or confirms an existing tag names a `main`
commit stating the version), and creates the Release with the `.crate` and
`SHA256SUMS`, `--latest=false`, reading both back. The irreversible act is first
and the residual window is the two writes after it, the same argument as
ADR-0026 §3. The approval is asked for when the registry OR the Release lacks
the version, so a re-run — or the manual arm on a later `main` commit carrying
the version — finds it on the registry, skips the upload, and writes whichever
of the tag and Release is missing. That path exposed a latent defect in
`tools/crates-io-publish.sh`: after a same-crate skip its post-condition waited
for the index to serve OUR sha256, which a later commit's packaging never has;
it now waits for the sha256 its plan decided. This is ADR-0026 §4's third
option — "a tag and an assetless release per format bump" — taken, with the
shelf not empty: the asset is the tarball the registry holds, and the checksum
beside it is what lets anyone prove the two are one.

The record is kept complete the same way as the plugin's (§5):
`tools/check-dsl-version-published.py` now also refuses a change that moves the
dsl version away from X unless `delvewright-dsl--vX` is a published Release
carrying the `.crate` and a `SHA256SUMS` whose line is the index sha256, on a
tag naming a commit stating X. This is what reds a publish whose tag-and-Release
step was skipped. The versions published before this workflow wrote Releases
have none; the first dsl bump after it lands therefore waits on the manual arm
releasing the current number.

With that, the engine release's treatment of the format crate changes from
"no-op re-check, or supply it if the hook never ran" to **refuse**: the
`delvec` release runs `tools/crates-io-publish.sh --publish --only delvec`
after a preflight (in `crates-preflight`, before any approval) that the
registry already serves `[engine].dsl_crate_version` as the same crate, and if
it does not, the remedy it names is the format crate's own workflow (its
dispatch arm). No path remains by which a `delvewright-dsl` version reaches
crates.io without its tag and Release being owed and checked.

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

ADR-0017 §5 made a filled release immutable "in practice". A tag ruleset makes
it so by construction, in the way the branch ruleset already protects
`refs/heads/feature/**`: target `tag`, include `refs/tags/*--v*` and
`refs/tags/v*`, rules `deletion` and `update`, an EMPTY bypass list, enforcement
`active`. Nobody updates or deletes a release tag, the repository owner
included.

Creation is NOT restricted, which departs from this section as proposed
("restricting creation to the repository's owner"). Two of the three release
workflows create their tag with the job's own token (§4, §6), and a `creation`
rule admits only its bypass list; GitHub's documented bypass actors are
repository roles, teams, GitHub Apps and Dependabot, and GitHub Actions is not
one of them (cited, not measured here: a public report quotes the rulesets API
refusing the Actions integration as a bypass actor). Restricting creation would
therefore stop §4 and §6 at their tag write, or need a second credential this
repository does not hold. What judges a created tag is the release workflow it
starts or that writes it: the grammar, the version at the commit, ancestry of
`main`, and for the plugin the tree-identity. Creating a tag requires write
access, which in this repository is the owner and the workflows' tokens.

It is a GitHub setting outside the tree, written by the planner from the
read-back state and proved in both directions: a probe tag under the pattern
is created (creation not blocked), then its deletion and its force-update are
refused, over both `git push` and the REST API; the probe is removed by
disabling the ruleset, deleting the probe and re-enabling it as the next act,
with the enforcement read back. The token direction is proved by the first
workflow run that writes a tag (§4 or §6): its tag step reads the written tag
back from the API.

## Consequences

- **The order the implementation lands in**, made structural. A tag-triggered
  run executes the workflow file at the tagged commit, and `engine-release.yml`
  refuses a tagged commit that is not an ancestor of `main`, so the first
  `delvec--v*` release cannot be published from an unmerged commit; and a pin
  checker that accepts only `delvec--v*` cannot land while the page still pins
  `v1.4.0` (the page's pin on `main`), because the required `content pin` job
  runs it online. No checker accepts both grammars (§7), so the work lands in
  two pull requests with a published release between them:
  1. **PR A** (the planner merges): the three workflows, the tag writer, the
     outgoing plugin-version gate and the dsl gate's Release half, the
     `crates-io-publish.sh` post-condition fix, fixtures, docs, this record.
     Nothing under the plugin root and no pin checker changes; the page stays
     on `v1.4.0`. Green on its own.
  2. The planner creates the `plugin-release` environment (owner as required
     reviewer, no secret) and the tag ruleset of §9, each proved.
  3. The planner runs `plugin-release.yml`'s manual arm on the `main` commit
     carrying the current plugin version; the owner approves. From PR A's merge
     until this Release exists, every pull request that moves the plugin
     version reds in `dsl crate version (crates.io)`, so this comes before any
     plugin bump merges. Likewise the planner runs `dsl-crate-publish.yml`'s
     manual arm on `main` before any dsl bump merges; the owner approves
     `crates-io` (the upload is skipped, the tag and Release are written).
  4. A release commit on `main` moves `[engine].version` to `1.6.0` (release
     plumbing only).
  5. A human pushes `delvec--v1.6.0` at that commit; `engine-release.yml` runs
     from it; the owner approves `crates-io`.
  6. **PR B** (a fresh branch): `tools/check-pins.py`, the `release` policy text
     in `.github/pins.toml`, `tools/check-skill-page.py` and `fetch-delvec.py`
     read `delvec--v<semver>`; spec-0063 §8 and its criterion 4 are re-stated
     against the grammar; the page re-pins to `delvec--v1.6.0` and its commit;
     the plugin version moves. Its `content pin` job is red until step 5's
     Release carries its shelf, and its plugin gate is red until step 3's
     Release exists.
  7. The plugin release for PR B's version: a pushed `delvewright--v*` tag or
     the manual arm; the owner approves.
  8. The content repository's pull request re-pins `engine-release` to
     `delvec--v1.6.0` and changes its checker and policy text in the same
     change; red until step 5.
- **Checks that change in PR A**: `engine-release.yml`'s trigger, strict
  identity, dispatch input, Release title, notes, the format-crate preflight,
  `--only delvec`, and an explicit `--latest`; `dsl-crate-publish.yml`'s `plan`
  (on-`main` refusal, the Release state in the approval decision) and gated
  job (the registry `.crate`, the tag, the Release, the read-back); the new
  `plugin-release.yml`; `tools/check-plugin-version-released.py` (new, a step
  of `dsl crate version (crates.io)`); `tools/check-dsl-version-published.py`
  (the Release half); `tools/crates-io-publish.sh`'s post-condition; the
  release-publish-gate fixtures' trigger line; `.github/pins.toml` registers
  `plugin-release.yml` as a site of the three actions it uses. Every one states
  its binding count. No required status context is added or renamed.
- **Checks that change in PR B**: `tools/check-pins.py` (both repositories)
  matches `<thing>--v<semver>` for the thing the registry entry names, and the
  `release` policy's text in both `.github/pins.toml` files says so;
  `tools/check-skill-page.py` derives the version from a `delvec--v` pin once
  and the archive name from the version, offline and `--online`;
  `fetch-delvec.py` does the same derivation.
- **Checks that do not change**: `tools/check-release-publish-gate.py` and
  `tools/check-approval-guard.py` (the plugin job is one more environment-gated
  job under a rule stated by object class); `tools/build-release-binaries.sh`
  and the archive grammar; the content repository's `release.yml`, which checks
  the engine out by commit and never by tag.
- **The docs**: `docs/reference/tools.md` rows for every new and changed tool
  (PR A) and for the pin checkers and the fetch script (PR B);
  `docs/reference/skill-workflow.md`'s account of how a newer page arrives gains
  one sentence — it arrives when the version moves on `main`, and the release is
  the record of it (PR A); spec-0063 §8 and its criterion 4 (PR B);
  `ACKNOWLEDGEMENTS.md` gains nothing (no library is adopted).
- **What the owner is committed to** the first time each can bind: a second
  GitHub environment (`plugin-release`) whose reviewer rule must be saved and
  whose binding is proved by `assert-run-approved.sh` the way the first one's
  is; a tag ruleset (§9) under which nobody, the owner included, can move or
  delete a release tag; one approval click per plugin release and one per
  format-crate Release; and the knowledge that the plugin versions and format
  versions delivered before their first Release have none.
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
