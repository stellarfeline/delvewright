# ADR-0036: A Release's notes are a summary written before the release, never the list of merged pull requests

- **Status**: Accepted
- **Date**: 2026-10-22
- **Source**: the `delvec 1.14.0` Release body read through the API, twice
  (first as published by `engine-release.yml`, then as rewritten by hand); the
  commit range `delvec--v1.13.0..delvec--v1.14.0` of `stellarfeline/delvewright`
  (`git log`); the engine tree at `e51f8bbef8c3a3b58caea451b5f9898b15b00509`
  (the 1.14.0 commit); Keep a Changelog 1.1.0; the changelogs of cargo (the
  Cargo Book), ripgrep and rustup, read at their current heads; GitHub's pages
  on automatically generated release notes and on the create-release REST
  parameter `generate_release_notes`; GitHub's "GitHub Models" page, the README
  of `actions/ai-inference`, the README and LICENSE of `github/copilot-cli`, and
  GitHub's Copilot plans and premium-request billing pages, all read on the
  date above.
- **Supersedes, in part**: ADR-0028 §2 — the `notes` row of its table and the
  paragraph after the table on generated notes between consecutive tags of the
  same line — and nothing else; ADR-0028 stands for everything else. Refines
  ADR-0028 §4 (the identity job now also refuses a commit with no notes) and
  the release order in `docs/reference/skill-workflow.md` (step 1 writes the
  notes).

## Context

### What a Release published

ADR-0028 §2 had every release workflow write an opening from the tree
(`tools/ci/release-notes.py`: which of the three things the Release is, the
`dsl_version` it speaks, its registry page) and then pass `--generate-notes
--notes-start-tag <previous tag of the line>` to `gh release create`. GitHub's
generated notes "include a list of merged pull requests, a list of contributors
to the release, and a link to a full changelog"; when a body is also given, "the
body will be pre-pended to the automatically generated notes".

Measured on `delvec 1.14.0`: the range `delvec--v1.13.0..delvec--v1.14.0`
holds 59 first-parent commits (399 commits in all), and the Release as
published was the opening followed by a `What's Changed` list of one line per
merged pull request, in merge order, each line the pull request's title with
its author and number — 85 lines. A reader opening the notes to learn what the
release does was handed the merge log.

The same body has since been rewritten by hand into a shape this record makes
machine-held: the opening; `## Highlights` (6 bullets); `## Changes` grouped
by area of the engine (authoring surface and diagnostics, rendering and review
frames, validation and playtesting, skill workflow, distribution and release —
24 bullets); a closing paragraph naming what the list leaves out; a link to the
full list of merged pull requests. That rewrite is the type specimen of a
summary: it reads what the merged changes did for a creator and says it in the
creator's terms.

### Established practice

Keep a Changelog, the convention this record follows, states (its words):
"Changelogs are for humans, not machines"; "There should be an entry for every
single version"; "The same types of changes should be grouped"; "The latest
version comes first"; and, under what makes a changelog bad, "Using commit log
diffs as changelogs is a bad idea: they're full of noise" — "The purpose of a
changelog entry is to document the noteworthy difference, often across multiple
commits, to communicate them clearly to end users." Its change kinds are
`Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, `Security`; each
version's compare link is a footnote at the end of the file.

The mature Rust tools do this with a committed, hand-written `CHANGELOG.md`:

| tool | where | per release |
|---|---|---|
| cargo | `CHANGELOG.md` in the Cargo Book | heading with version and date; a compare link to the release range; sections `Added`, `Changed`, `Fixed`, `Nightly only`, `Documentation`, `Internal`; one user-facing sentence per bullet, each ending in its pull-request links |
| ripgrep | `CHANGELOG.md` at the root | heading with version and date; a two-sentence prose summary of the release; sections `Platform support`, `Performance improvements`, `Feature enhancements`, `Bug fixes`; one user-facing sentence per bullet, each opening on its issue link |
| rustup | `CHANGELOG.md` at the root | heading with version and date; a prose opening and a headline feature; grouped bullets with inline pull-request references; a `Detailed changes` list of every merged change below |

None of the three publishes the merged list as its notes. Each writes the
summary into the tree before the release, and each keeps the full list one
link away. The three link every bullet to its pull request; this repository
does not, because a pull-request number is not a sanctioned identifier in a
repository artifact (`CLAUDE.md`, *Privacy in repo artifacts*), and the compare
link carries every pull request of the range for the reader who wants them.

### The constraints this record works under

- **CI publishes everything** (ADR-0023): a human dispatches a release
  workflow and nothing else reaches a Release.
- **A release is built from a frozen approved tree**, only release plumbing
  added on top, each file named, and the release refuses an unnamed difference
  (`CLAUDE.md`, *Methodology*).
- **Every input to a surface is a creative judgement or a procedural
  derivation**, never typed: a summary is a creative judgement; a compare link
  and a merged list are derivations.
- **Docs are present tense; ADRs are the only place history lives**
  (`CLAUDE.md`, *Methodology*): a changelog narrates versions by construction,
  so where it lives decides whether the documentation rule is broken.
- **Nothing owes compatibility to anything already built**: a published
  Release is never edited by a workflow, and nothing is tagged retroactively
  (ADR-0028 §4).

### A summary drafted in the run by a hosted model, evaluated

Writing the summary in the release run itself — the workflow drafts it from the
merged pull requests between the previous tag of the line and the release
commit, the shape check judges it, a release with no valid summary refuses —
was evaluated against what a workflow can call on the date above.

- **GitHub Models, through the workflow token's `models: read` permission.**
  Not available. GitHub's page states: "GitHub Models has been retired." — "As
  of July 30, 2026, GitHub Models has been fully retired." — "The playground,
  model catalog, inference API, and bring your own key (BYOK) are no longer
  available to any customer." GitHub points to Azure AI Foundry for model
  access and to Copilot for "AI-powered workflows directly on GitHub".
- **The Copilot CLI in Actions.** Callable, under conditions. The README of
  `actions/ai-inference` now reads "Run AI inference in workflows using the
  GitHub Copilot CLI"; its only `provider` is `copilot`; it "invokes Copilot CLI
  with `-p <prompt> -s --no-ask-user`"; the CLI "is not pre-installed on
  GitHub-hosted runners" (the example installs `@github/copilot` from npm); and
  authentication is a `COPILOT_GITHUB_TOKEN` taken from a repository secret —
  the CLI's README: "a fine-grained PAT with the 'Copilot Requests' permission
  enabled", under "An active Copilot subscription". The workflow's own token
  carries no Copilot permission, so the credential is a person's. Cost: "Each
  time you submit a prompt to GitHub Copilot CLI, your monthly quota of premium
  requests is reduced by one"; the plans page marks the CLI "Included" on every
  plan, Copilot Free among them, and states Free's monthly allowance of AI
  credits as a figure the page read did not give. Terms: the CLI ships under
  the "GitHub Copilot CLI License" (run-only; no modification or
  redistribution), and GitHub's Terms of Service §J make the account holder
  responsible for "reviewing, testing, and validating any Output before use".

So the second route works mechanically, and it is not adopted, for reasons
that stand whatever the model's quality:

1. **It puts a person's credential into the repository's secrets**, readable
   by every workflow on `main` and spending that person's monthly allowance,
   for a text one agent already writes at release preparation.
2. **The text would publish with review only at the approval click** for
   `delvec` and the format crate, and with none for the plugin (whose dispatch
   is its approval, ADR-0028 §5); a re-run would write a different text. A
   summary in the tree is reviewed on a pull request, under CI, before any
   release exists, and a re-run publishes the same bytes.
3. **It cannot be proven before it lands**: no release workflow runs from a
   pull request, so the step would merge unexecuted, and §J's duty to review
   the output would fall on the approval page.
4. **No cited practice does it**: the three tools above write the summary by
   hand into the tree.

The decision is therefore: the agent that prepares the release writes
`RELEASE-NOTES.md` in the release pull request, and no step of a release run
drafts it. The shape the summary must have, the check that judges it, and the
refusal of a release with none are the same under either route, so a drafting
agent reconsidered later changes none of them (*Revisit triggers*).

## Decision

### 1. The summary is a `RELEASE-NOTES.md` entry, written before the release

`RELEASE-NOTES.md` at the repository root holds the notes of the release being
prepared: one entry per line that release names — at most one per line — each
headed by the Release's title in ADR-0028 §1's grammar (`## delvec 1.15.0`,
`## delvewright-dsl 0.57.0`, `## delvewright 1.15.0`), so the heading is the
title the workflow derives, not a second spelling of it. The entry is written
by the agent that prepares the release, in the pull request that names the tag
(`docs/reference/skill-workflow.md`, step 1), from `git log <previous tag of
the line>..HEAD`; that pull request replaces the file wholesale. No step of a
release run drafts it: the hosted-model route is evaluated in §Context and not
adopted. It is part of
the approved tree the release is built from: nothing is added on top of the
frozen tree, so the frozen-tree rule needs no exception and the release reads
the entry at the released commit like every other value it reads.

The file is not a changelog and keeps no history: the published GitHub Release
keeps each version's text, and the tree carries only the next release's. ADRs
stay the only place history lives, and `docs/` stays in the present tense of
the current version, which is why the file sits outside it. Keep a Changelog's
"an entry for every single version" is met on the repository's Releases page,
one Release per version, each carrying the entry it was published from; it is
not met by an accumulating file in the tree, which this record rejects as a
second history. Earlier Releases keep the notes they were published with
(ADR-0028 §4: nothing is tagged or re-written retroactively). The name is
`RELEASE-NOTES.md` rather than `CHANGELOG.md` because the file is the next
Release's notes and nothing else; the conventional name would promise the
accumulation it does not hold.

### 2. The shape of an entry, and what makes it a summary

`tools/lib/release_notes.py` is the one reader of the file. An entry is:

```
## delvec 1.15.0

### Highlights

- what this release means to the creator who installs it (a few bullets)

### <a group of changes>

- the changes worth stating, under as many groups as the entry needs

A closing paragraph may say what the entry leaves out.
```

`release_notes.shape` holds it to that, one finding per defect:

| rule | cited or authored |
|---|---|
| a `### Highlights` section with at least one bullet, at most `HIGHLIGHTS_CEILING` (7) | authored: the hand-written 1.14.0 body has 6; ripgrep and rustup open each release with a short prose summary |
| at least one `### <group>` of changes, each with at least one bullet; the group names are the author's — the Keep a Changelog kinds or the areas of the engine the changes touch | cited: Keep a Changelog, "the same types of changes should be grouped"; cargo and ripgrep group by kind, the 1.14.0 body by area |
| at most `CHANGES_CEILING` (40) change bullets in all | authored: the 1.14.0 body states 24 changes over 59 merged pull requests and says what it leaves out |
| every line inside a section: a bullet, its indented continuation, or a line of prose; nothing before the first section | authored, so that every line is read |
| no line — bullet or prose — that is a merged change's own subject, both read through `release_notes.normalize` (casefolded; a conventional-commit prefix and a trailing pull-request suffix removed; inline markup and the final period dropped), over the commits between the previous release of the line and the released commit | cited: Keep a Changelog, "Using commit log diffs as changelogs is a bad idea" — this is the machine form of the ruling |
| the heading is a Release title; no title twice; one entry per line; the entry's version is the one the tree states for its line (an entry whose tag is written while the tree states a later version is stale and refused; one whose tag is written while the tree still states it is the release being re-run) | cited for the heading: ADR-0028 §1; authored for the rest — the file holds the release being prepared and nothing older |

The ceilings are numbers in `tools/lib/release_notes.py`, printed by the
diagnostic that applies them, and restated nowhere.

### 3. What a Release publishes

`tools/ci/release-notes.py <line> --commit REV --repository OWNER/NAME` prints
the opening ADR-0028 §2 defines, with one correction to the plugin's: the
command that holds a fixed page pins the marketplace to the engine release tag
as `/plugin marketplace add stellarfeline/delvewright#<tag>` — Claude Code pins
a GitHub marketplace to a branch or tag with `#ref`, and the `@ref` form
ADR-0028 §3 wrote is not one it reads — and tells a creator who already holds
the marketplace to `/plugin marketplace update delvewright` instead, because
the same name from another source is refused. Then the entry's body as written;
then one derived line — everything merged since the previous release of the
line, as `https://github.com/<repository>/compare/<previous>...<tag>` (for a
line's first release, a sentence saying there is nothing to compare with). No
workflow passes `--generate-notes`, and `tools/lib/github_releases.py` no
longer calls the generate-notes endpoint. The same shape for all three lines.

### 4. What refuses, and where

- **A release with no summary, or a summary that is the merged list again,
  refuses at the release**: `release-notes.py` exits 1 naming the entry or its
  absence, and each workflow reaches it before anything is built, tagged or
  published — `engine-release.yml` in `identity` (whose bytes `shelf` publishes
  from an artifact, so the approval page shows what the undraft publishes),
  `plugin-release.yml` before its tag, `dsl-crate-publish.yml` in `plan`, before
  the approval. Nothing is dispatched twice: the remedy is a pull request
  writing the entry, then a dispatch on its merge commit.
- **A defective entry refuses on the pull request that writes it**:
  `tools/ci/check-release-notes.py`, a step of `docs (local link check)`, judges
  every entry of the file through the same reader: the file-level rules, the
  shape, and that every entry is the release being prepared — its version is
  the one the tree states for that line; an entry for a released version the
  tree has moved past is refused as stale. Its range for the copy check ends at
  the entry's tag where it exists, else `HEAD`. A file with no entry is a
  legitimate zero and passes saying so.
- Both are held by `tools/tests/test_release_notes.py` (each refusal perturbed
  toward the forbidden shape and named) and `tools/tests/test_release_tags.py`
  (every release workflow reads the notes from the tree before `gh release
  create`, hands `GITHUB_REPOSITORY` for the link, and passes no
  `--generate-notes`).

## Consequences

- Preparing a release gains one obligation, in the pull request that already
  names the tag: replacing `RELEASE-NOTES.md` with the entry for each line
  being released. A dispatch on a commit without it is refused in the first
  job, with the remedy named. Between releases the file carries the last
  release's entries, which the check accepts while the tree still states those
  versions and refuses as stale once a version moves — on the pull request that
  moves it, where the new entry is owed.
- `engine-release.yml` loses its `previous` output and gains the notes step and
  artifact; `plugin-release.yml` and `dsl-crate-publish.yml` lose their
  `previous` outputs and the generated-notes calls; `github_releases.py` loses
  `notes`. `release_tags.previous` stays: it decides the range and the compare
  link.
- GitHub's "New Contributors" section no longer appears; the repository has one
  author, and the compare link lists every pull request with its author.
- `RELEASE-NOTES.md` is a file that only release-preparing pull requests touch, so
  it is not the conflict class ADR-0034 removed; it is outside `docs/`, so
  `check-doc-dupes.py` (which refuses a repeated heading within a document) does
  not read it, and its repeated `### Highlights` headings are the format.
- No credential enters the repository's secrets and no release run calls a
  hosted model: the entry is reviewed as part of the release pull request, like
  every other change that pull request carries.
- Docs: `docs/reference/tools.md` rows for `release-notes.py`,
  `check-release-notes.py` and `release_notes.py`, and the removal of the `notes` row;
  `docs/reference/skill-workflow.md` step 1; `RELEASE-NOTES.md`'s header. The
  ACKNOWLEDGEMENTS ledger gains nothing: no library is adopted.

## Revisit triggers

- A drafting agent in CI is reconsidered: the admissible form writes the entry
  into the release-preparing pull request, where the same check judges it, and
  nothing in §2–§4 changes. A variant that drafts at release time and publishes
  without a pull request would replace "no entry: refuse" with "no entry:
  draft, judge, show", which is a second hatch on the gate (`CLAUDE.md`,
  *Vacuity*), and is not taken without re-deciding §1.
- The ceilings prove wrong in practice: they are constants in
  `tools/lib/release_notes.py`, an engine-internal number, moved with the entry
  that needed it and no re-decision.
- A fourth released thing: it joins `release_tags.NAMES` and the grammar of §1
  reaches it unchanged.
