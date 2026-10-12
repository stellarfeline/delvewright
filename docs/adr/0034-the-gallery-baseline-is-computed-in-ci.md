# ADR-0034: The gallery emission baseline is computed in CI, never committed

- **Status**: Accepted
- **Source**: a replay of the member merges of the five integration pull
  requests that landed on `main` before this decision, with `git merge-tree
  --write-tree --name-only` (git 2.54.0) over each member merge's two parents;
  `git log` over `main` and the integration branches; the GitHub Actions jobs
  API for the `main` run of the last of those integrations. Refines spec-0039
  §5 and its acceptance criteria 7–10; keeps ADR-0006's cross-host probe.

## Context

spec-0039 §5 committed the gallery's emission baseline: per build of the domain
a copy of the compiler's `manifest.json` (`gallery/baseline/manifests.json`), a
header (`header.json`), a review delta naming the commit it was measured from
(`delta.json`), and the expected-warnings ledger (`warnings.json`). CI rebuilt
the domain and refused any tree whose measurement differed from the committed
copy, so every change reaching `crates/` or `gallery/` had to commit a
regenerated baseline.

Measured:

- **The baseline is the largest merge-conflict class.** 28 member merges
  entered the five integration branches (four merges of `main` into an
  integration branch excluded). Replayed, they conflict in 159 files: 58 under
  `gallery/baseline/`, 36 in append-only documentation registries
  (`docs/reference/*`, `docs/specs/README.md`, `docs/demo-levels.md`), 28 in
  Rust or TypeScript source, 37 elsewhere. Counting content conflicts alone:
  124, of which 58 are the baseline. A baseline conflict is a conflict between
  SHA-256 values: it has no hand resolution, only a rebuild and a regeneration.
- **Every fix on an integration branch re-pays it.** On the last integration
  the four member merges took six minutes; twelve follow-up commits then ran
  over 1 h 33 min, two of them regenerating the baseline and one correcting the
  per-row attribution of a regeneration. Another integration branch carried
  nine non-merge commits regenerating the baseline.
- **It is a standing share of all work.** Of the last 300 non-merge commits
  reachable from `main`, 58 change `gallery/baseline/` (46 of them
  `manifests.json`).
- **The job was not the bottleneck.** On the `main` run of the last
  integration, `gallery baseline (emission, warnings, served points)` took
  7 m 29 s and the critical path was `rust (fmt, clippy, test)` at 30 m 22 s.

The cause is structural: a committed measurement of a build is a file every
engine change must rewrite, so any two engine changes conflict on it, and the
conflict can only be resolved by re-measuring.

## Decision

1. **The baseline is computed in the run.** `tools/ci/gallery-baseline.py`
   builds the domain from the comparison tree — `merge-base(origin/main, HEAD)`
   on a pull request, `HEAD^1` on a push to `main` — and from the head, both
   from source, and publishes the delta between them (job summary and CI
   artifact). The comparison is built entirely from its own tree: its own
   `delvec`, its own `gallery-prefabs.py`, its own gallery source, its own
   `gallery_domain.py`.
2. **Red conditions.** A build failure on either side (never a fallback); the
   head's warning set differing from the committed ledger (spec-0039 criterion
   5); a non-empty delta on a change that reaches no emission input — a
   determinism finding under ADR-0006. A non-empty delta on a change that
   reaches an emission input is a report for the reviewer, not a red.
3. **Only the ledger is committed.** `gallery/baseline/warnings.json` stays:
   it is an authored expectation, not a measurement. `header.json`,
   `manifests.json` and `delta.json` leave git and are ignored.
4. **The consumers read the run's measurement.** `gallery-build.py` proves the
   tree it writes against the head manifests the same run measured and refuses
   manifests measured at another commit; `check-gallery-stageable.py`
   enumerates its points from the gallery's own languages and overlays.
5. **The cross-host probe moves to the cross-OS job.** The committed baseline,
   written on one host and checked on another, was also a macOS-vs-Linux probe
   of the gallery build. `tools/ci/determinism-subject.sh --subject gallery`
   builds the gallery primary — generated pieces, materialised campaign, build —
   on both hosts, and `cross-OS determinism (macOS vs Linux bytes)` compares the
   digests beside the metrics gym's.

## Consequences

- Two engine pull requests no longer conflict on the gallery. Merging an
  integration branch needs no regeneration step, and a fix on it needs none.
- An emission change is no longer acknowledged by a committed file; it is read
  in the job summary. The determinism verdict still reds by itself.
- The `gallery baseline` job builds a second engine and a second prefab
  directory from source; the comparison's cargo target is cached. The `gallery
  bot` job waits for it, because it reads the manifests that job measured.
- The warning ledger is the one gallery document a merge can still conflict on;
  it is small, authored and readable, so a conflict in it is resolved by hand.
- The cross-host probe now runs on every change the `rust` group reaches, not
  only on the developer machine that regenerated the baseline.
