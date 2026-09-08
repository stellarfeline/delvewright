# ADR-0026: One approval authorises both platforms — an engine release reaches crates.io and the GitHub Release together, or reaches neither

- **Status**: Accepted
- **Date**: 2026-09-08
- **Source**: the rule that an engine release is one act with two outlets, so
  landing on either platform without the other is the defect; measured against
  `.github/workflows/engine-release.yml`, `.github/workflows/dsl-crate-publish.yml`
  and the tools they call, at the revision this ADR was written from and on the
  release change that implements it.
- **Refines**: ADR-0016 (the engine version line: one tag, one number), ADR-0025
  (two crates, published in one order).
- **Supersedes, in part**: ADR-0017 §4 — its mechanism half. Two of its
  statements are replaced: that exactly one job in the repository declares the
  `crates-io` environment, and that the shelf is published before, and
  independently of, the registry. ADR-0017's remaining sections stand as
  already amended, and its *What happened in practice* section stays the
  record of the incident and of the tooling it produced.

## Context

ADR-0017 §4 made CI the only publisher and put the registry behind the
`crates-io` environment. The release line it described had two outlets that
could be reached separately: the GitHub Release was created and its shelf
uploaded with nobody asked, on the argument that assets are reversible, and only
the registry half waited for a reviewer. Either half could therefore land alone.
One direction was observed — the release whose shelf filled while nothing reached
crates.io is recorded in ADR-0025 §4 — and the mirror image, a public release
beside a registry that never received the crates, was reachable by the same
structure. One approval was being asked for, and two independent outcomes were
possible.

§4 was also wrong in a second way that predates the release change. It said that
exactly one job declares the `crates-io` environment and is therefore the only
job able to obtain the token. Two do: the release workflow's gated job, and the
`publish` job of `dsl-crate-publish.yml`, which uploads `delvewright-dsl` off
`main` when its number moves. The statement was a count over the repository,
the count changed with no edit to §4, and nothing in the record said so.
ADR-0017's own *What happened in practice* section foresaw the shape — a second
environment-gated job inherits nothing — and named the checker as the work to
redo; that checker exists (`tools/check-approval-guard.py`) and is part of what
this record relies on.

The general point, which §4 already demonstrated once, is that an environment
approval gates access to a token, and what it authorises is whatever the gated
job then does. A record that describes the approval as protecting a platform is
describing the job, and a job can be restructured without the record noticing.

## Decision

### 1. One approval authorises both publishes, and neither happens without it

An engine release publishes to two platforms — the crates.io versions and the
GitHub Release — under one approval, on the `crates-io` environment, granted to
one job that performs both acts. A run that is never approved publishes to
neither. The synchrony is logical, not temporal: the two need not land at the
same instant, only under the same decision, and no path exists by which one
platform is reached under a decision the other was not.

### 2. A draft is not a publication, so the shelf owes no approval

GitHub's release API defines a draft as an unpublished release (GitHub REST API,
*Create a release*, the `draft` field). A draft is served only to accounts with
write access on the repository; it is absent from the public releases listing,
from the releases feed and from every unauthenticated asset download; creating
one announces nothing. Filling a draft therefore publishes nothing, and that is
the whole reason the shelf can be built, checksum-verified after its round trip
through artifact storage, counted against the target list and read back from
the API before a human is asked anything. The reviewer approves a shelf that is
already proven complete, and an incomplete shelf reds before the prompt exists.

The one act the shelf job must never perform is a write to a release that is
already published, because uploading an asset to a public release is itself a
publish. It reads the release's draft state before writing and refuses a
published one, which is also what ADR-0017 §5 requires: once its shelf is
filled, a released tag is as immutable in practice as a crates.io version.

### 3. The irreversible act goes first, and the residual window is one call wide

The gated job publishes to crates.io first and undrafts the GitHub Release
second. The order follows from reversibility. A crates.io version can never be
reused or deleted (ADR-0017, Context); undrafting is one idempotent API call.
A failure at the registry leaves the release a draft and nothing published
anywhere — the "neither" state the approval is meant to be able to produce. The
reverse order would let a registry failure leave a public release describing a
version the registry never received, and no later act could take that back.

The failure mode this design keeps is the window between a successful registry
upload and a successful undraft. Its width is one API call. Inside the job the
undraft is retried with the release state read back each time — a zero exit
from the tool is not evidence that the release is public — and if every attempt
fails the run reds, states that the registry holds the version and the release
is still a draft, and names the single remaining act. Re-running the workflow
performs the same act with no new decision needed: the registry step finds every
version already present with identical bytes and skips it (ADR-0017 §4's
checksum idempotency, on which this ADR depends), and the undraft runs again.
The tag is never re-cut and the version is never bumped to escape this window.

So the only state the design can leave that is neither "both" nor "neither" is
"registry published, release still a draft", from which "both" is one repeatable
call away and "neither" is unreachable. That asymmetry is chosen, and it is the
reason the order is fixed rather than a matter of tidiness.

### 4. The format crate publishes on its own clock, and that path is outside this rule's letter

`delvewright-dsl`'s package version is the `dsl_version`: the crate defines the
format, so its number is the format's number (`crates/dsl/Cargo.toml`;
`versions.toml [engine].dsl_crate_version`; ADR-0024). A number that names a
format has to be resolvable the moment a document declares it, and the engine's
release line moves on a different clock, so the crate does not wait for a tag:
`.github/workflows/dsl-crate-publish.yml` publishes it when its number moves on
`main`, under its own approval on the same `crates-io` environment. Plainly
stated: **this is the one path by which something reaches crates.io with no
GitHub release beside it**, and it is the second environment-gated job.

The rule in §1 binds an event that publishes to two platforms. The format crate's
publish has one platform and one product — a library crate, with no archive to
shelve; a GitHub release for it would carry no assets, which is the empty shelf
ADR-0017's Context names as the defect it set out to close. "Both or neither"
over one outlet reduces to one approval per publish, which that path already
has. So the letter of §1 does not reach it, and this ADR does not fold it in.

Whether the rule's intent reaches it — that nothing should arrive on crates.io
except under an engine release — is not settled by the record and is left open
here rather than decided in either direction. What each answer costs:

- **Fold the format crate into the release.** The format's number becomes
  resolvable only at the next engine tag, so a document declaring it in between
  names a version the registry does not hold. `ci.yml`'s `dsl crate version
  (crates.io)` check, which refuses a change to `crates/dsl` under a number the
  registry already serves on every pull request, loses its premise: the number
  could move on `main` without publishing, several format changes could ride one
  number until release day, and a number would stop naming exactly one format
  (ADR-0024). The release path's no-op re-check of the format crate becomes a
  real publish of two crates in sequence, reintroducing the half-success
  (format crate lands, engine fails) that ADR-0017 §2 separated the version
  lines to contain.
- **Leave it as it is.** Two gated jobs share one environment, so two approval
  prompts of different weight look alike and the reviewer reads the workflow
  name to know which door is being opened; and "a crates.io version with nothing
  on GitHub beside it" is a normal, intended outcome of one path while being the
  failure state of the other.
- **Give the format publish a release of its own.** A tag and an assetless
  release per format bump: the empty shelf, on purpose, once per `dsl_version`.

### 5. What holds by construction, what holds by a check, and what is known only after the fact

An ADR that claims a setting it cannot see is the defect ADR-0017 §4 already
demonstrated, so each claim above is filed by how it is held.

**By construction** — properties of the workflow's own structure, which no
comment can misdescribe: the `needs:` graph, under which the gated job is
reachable only through a shelf job that built, verified, counted and read back
the whole shelf into a draft; the `environment:` declaration, which is the only
place the registry token can be read from; and the write scope on repository
contents, which the undraft needs and which only the shelf job (bounded to
drafts by its own read-back) and the gated job carry.

**By a check, on every push** (`ci.yml`): that only a gated job publishes —
`tools/check-release-publish-gate.py` refuses a publishing act (the registry
token, a registry upload, a release created other than as a draft, an undraft)
in any job that declares no environment, refuses the release verbs in scripts
outright so a job cannot hide one behind a call, requires an ungated job that
writes to a release to read its draft state back in the same job, and reds on a
zero binding; and that every gated job asserts its own approval first —
`tools/check-approval-guard.py`, keyed to `environment:` as the object class
rather than to any job by name. Both prove that the shape is authored. Neither
can see GitHub's configuration.

**After the fact only**: whether the run actually paused. The reviewer
requirement lives in GitHub's environment settings, outside the repository, and
nothing in the tree can read it. `tools/assert-run-approved.sh`, the first
`run:` step of every gated job, reads the run's own approval record and refuses
when no approval names the environment; a run that was never held records none.
This ADR therefore claims exactly this about the pause: a run nobody approved
cannot publish, because the only job that can publish refuses first. It claims
nothing about the environment's settings. The run also states the state it left
without a reader opening a log: the shelf job writes the draft state into the
run summary, and the gated job stands at "waiting for review" for as long as it
is never approved.

## Consequences

- A release run nobody approves ends with the tag holding a draft release that
  carries the whole shelf and nothing new on crates.io. Approving it later
  completes both halves; nothing has to be rebuilt or re-tagged.
- The manual re-run input re-fills a shelf only while its release is a draft; a
  published release is never re-filled, by refusal rather than by convention
  (ADR-0017 §5).
- ADR-0017 §4's "the registry step runs only after the whole shelf has built"
  now reads: after the whole shelf has been built, verified, counted and read
  back from the draft release. Its remaining mechanisms — the tag names the
  engine version, the tagged commit is an ancestor of `main`, the upload is
  idempotent by checksum — stand, and §3 of this ADR depends on the last.
- ADR-0017 §4's "exactly one job" is retired as a count. The rule is stated by
  object class: every job that declares an environment proves its own approval,
  and no job without one can publish. A third environment-gated job inherits
  both checks with no edit to either.
- ADR-0025 §4's re-alignment of the two channels at the next engine version
  happens under §1: one approval, both channels.
- `docs/reference/tools.md` holds the two checkers and the guard; the release
  workflow's header carries the ordering argument beside the steps it orders.

## Revisit triggers

- The format's number and the engine's number stop moving on separate clocks
  (ADR-0025's own trigger). The format crate's path then folds into the release
  and §4's open question closes without a further decision.
- A ruling on §4's open question in either direction; it pays one of the three
  costs listed there, knowingly.
- A third platform joins the release — an OCI image per ADR-0010 is the likely
  one. §1 generalises unchanged: one approval, every platform, acts ordered from
  least reversible to most, and the residual window is the one after the last
  irreversible act.
