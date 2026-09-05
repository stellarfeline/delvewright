# spec-0039: The gallery campaign — every declared DSL surface, bound in one artifact

- **Status**: Accepted
- **Specs**: 0033 (the corpus precedent: coverage of an authoring surface is a
  measured artifact, not a claim), 0025/0023 (the plans the exercise proof
  reads), 0015 (the render arms), 0004 (fixture-build discipline for CI)
- **ADRs**: 0006 (determinism — the baseline leans on it), 0007 (licensing —
  §2 reconciles the gallery with the no-campaigns-in-this-repo zone), 0008
  (CI as arbiter; every job required)
- **Non-goals**: grammar-IR coverage (spec-0033's corpus owns it); the
  prefab-admission and render tool surfaces (their own gates own them);
  red-path diagnostic coverage (fixtures + `check-dw-codes.py` own it — the
  gallery is the composed *green* whole and never asserts a diagnostic fires);
  teaching humans (demo levels own it, §2); shipping the gallery as a playable
  delve (never — §2).

## 1. The decision

The engine repository gains **one campaign of its own — the gallery** — holding
at least one instance of every content-visible surface the DSL declares. When
an engine change lands something a campaign author can write, the same change
adds its element to the gallery. The gallery is built in CI on every pull
request, and the job is a required status check.

Three obligations, deliberately distinct, each with its own machinery:

1. **Coverage** (§3): a surface no campaign exercises is a surface nothing has
   ever compiled end to end. The gallery build reports which of the DSL's
   declared surfaces it binds, and a zero binding is a red.
2. **Regression** (§5): every element lives in ONE campaign, so a change's
   effect on existing elements is a diff in one named artifact — a committed
   emission baseline — rather than scattered test failures.
3. **Inspection** (§7): the elements are concrete, so "what does this surface
   look like when written" has an answer that is authored JSON and a rendered
   frame, not prose.

## 2. Where it lives, and what it is not

`gallery/` at the repository root: stage documents, l10n sidecars, overlays
(§3), generator programs for its pieces (§6), render plan (§7), baseline
(§5). The implementing PR adds the row to CLAUDE.md's layout listing.

**Versus the forbidden zone / ADR-0007.** "Generated campaigns do not live in
this repo" forbids shipping player content from the GPL tree. The gallery is
not player content: it is authored engine-test source, licensed with the code,
never released, never staged. Its build outputs are CI artifacts and are never
committed — only source and the hash baseline are. The release pipeline and
the staging surface enumerate content-repo campaigns; a test asserts the
gallery id is not reachable by either (§8.13).

**Versus `crates/*/tests/fixtures`.** A fixture is a minimal per-rule input,
owned by the crate that owns the rule, and half of them exist to prove a
refusal. The gallery is the opposite on every axis: one campaign, composed,
green, exhaustive over the surface, never minimal, never asserting a red. A
rule's regression test stays a fixture. What only the gallery can catch is the
cross-feature interaction and the emission drift that no per-rule input
composes. **The gallery is singular**: a landing feature adds an element to
it; a second gallery is the two-authorities defect and is refused in review.

**Versus `docs/demo-levels.md`.** A demo teaches ONE mechanic to a human at
playable scale, lives in the content repo, and is the form an engine
capability's confirmation takes. The gallery is exhaustive, machine-first, and
nobody plays it for understanding. They do not merge, in either direction:
folding demos in would bloat them past teachability; folding the gallery out
to demos would put coverage behind the content pin and a human queue, which is
§3's same-PR property destroyed. A gallery element may seed a demo's design;
it is never one.

**Versus the content-repo campaign builds.** `tools/build-every-campaign.py`
already builds every campaign at the **pinned** content SHA — real, and not
this. What it proves is that shipped content still compiles under the changed
engine. It structurally cannot cover a surface landed in the PR under review:
the pin lags the engine by construction, and the campaigns behind it are
approved creative artifacts no engine PR may edit (unrequested change is a
rejection cause on its own). The gallery is engine-owned and same-repo, so the
element lands **in the same PR** as the surface — which is the entire point:
generality and coverage are decided at the first site, not retrofitted at the
second.

## 3. The coverage gate

**The unit.** The declared surface is enumerated from `delvec schema --stage
all` of the delvec built in the tree under test — the compiler's own export of
`crates/dsl/src/stages.rs`, the single authority. A **surface unit** is every
named schema property and every enum/tagged-union variant (objective kinds,
effect verbs, trigger kinds, edit verbs, shot styles …), recursively, across
stages 1–7 and the sidecar schema. Values of vanilla registries (potion ids,
block ids, sound ids) are data, not surface, and are never units — exhausting
them is meaningless and would make the gallery a registry dump.

**The binding.** `tools/check-gallery-coverage.py` (stdlib python, CI job,
runnable on any clone) walks the gallery's authored JSON **guided by the
schema** — never by grep — and maps every unit to its binding sites (JSON
pointers). The binding domain is the primary gallery plus its **overlays**:
some stage-1 scalars are mutually exclusive within one world (`horizon`,
`time`, `weather`, `difficulty`, `min_players`), so the gallery carries a
small enumerated set of overlay documents, each a variant of one or more stage
files rebuilt as the same campaign at a different parameter point. An overlay
is a parameter point of the one gallery, not a second gallery: each declares
the non-empty unit set it exists to bind, and the tool asserts that set is
bound by it and not already bound by the primary — a redundant overlay is a
red, which is what stops the set growing by reflex.

**The verdict.** Every unit is either **bound** somewhere in the domain, or
**refusal-proven**: a committed probe document writes the unit and the gate
asserts `delvec validate` refuses it at the gallery's `dsl_version` with the
named code (a reserved field's `DW0141`, a deprecation fence). Nothing else —
no free-text exemption, no third kind. This is the vacuity-mode-6 hardening:
the escape hatch demands a machine-produced refusal, which "nobody authored
it" — the defect this gate exists to catch — cannot supply. A justification
that is prose is not an exemption here. Anything legal-but-unbound is a red,
and the fix is an element, because any legal combination is expressible by
some overlay.

> **Amended — what "an element" includes, measured on the traversal surface.**
> The clause above, "any legal combination is expressible by some overlay",
> was read during the first gallery build as promising that every unit is
> dischargeable by a *field line*, and six units are not: `Locomotion::ground`
> / `::climber` / `::flier`, `BodyTraversal.locomotion`, `Npc.traversal`,
> `Actor.traversal`. `DW0454` refuses a `traversal` declaration the build
> cannot hold the body to, so writing one legally needs a body whose derived
> class differs from the declared one *and* a walked route that crosses a
> barrier line — a world, not a field. Measured against the engine rather
> than the doc comment (spec-0034's own fixtures, plus a standalone `delvec
> build` of each remaining shape), every one of the six binds green in a
> build whose world pays for the claim: `climber` or `flier` declared on a
> ground-derived body over a barrier crossing waives the `DW0453` advisory;
> `ground` declared on a climber-derived body over the same crossing raises
> it, as one pinned expected-warnings row. The crossing itself is ordinary
> element material — one `world-edits` fill of a wall line, no generator or
> engine change. So the premise stands, restated precisely: **any legal unit
> is writable by some element of the domain, and an element includes the
> world that pays for its claims.** A unit legal in *no* build is a fence and
> owes a probe (`DW0455`'s `aquatic` is the worked case). No third coverage
> state exists or is added: "bound" already means written in the domain and
> built green, and for a claim-shaped unit the compiler's own exercised proof
> (`DW0454`) is what holds the binding — a demand a merely-unwritten unit
> cannot meet. The required-check expiry condition is therefore reachable as
> written; the six discharge as one ordinary element, not as a gate change.

**Why this fires at the right rate.** The gate reds exactly when the schema
export gains a unit the gallery does not write — i.e. precisely on the changes
that grow authoring surface, and on no others. The nearest measured proxy in
this repo's history is the `Since`-columned surface-table signal: 27–53
firings over 416 merges (`check-demo-levels.py`'s replay). That rate killed
that signal *for the demo gate* because a demo costs a level and a human
queue; here the discharge is one field line or one small element, in-repo, in
the same PR — the discharge IS deliverable 1, not overhead on it. A gate is
routed around when firing is frequent AND discharge is expensive; this one is
frequent-and-cheap by construction, and it cannot fire on a non-surface change
at all (schema unchanged → unit set unchanged). The stronger demand — a
*mechanic*-shaped element rather than a field line — is deliberately not the
gate's business: the gate cannot tell a mechanic from a field, and the
long-flag demo gate already routes mechanics to the human queue.

**Vacuity guards on the gate itself.** Zero enumerated units is a red (the
schema export moved or emptied). The tool never parses `stages.rs` itself —
one enumeration authority, the compiler's, same doctrine as
`for_each_effect_root`.

## 4. Exercised, not present

A declared element nothing reaches compiles green and checks nothing. "Bound"
in §3 means *written*; the gallery must also prove *exercised*, and the proof
is the compiler's own ladder, not a parallel one:

1. **Compiled**: `delvec build` exit 0 on every build in the domain, in every
   declared language (§5). The build's own binding reports (`RootBinding` and
   every stated-binding proof) are carried into the coverage report; a
   machinery the gallery writes that reports zero binding at build time is a
   red — the unbound-element case caught by the layer that owns it.
2. **Reachable**: `delvec analyze` green. The quest graph, branch proofs
   (`DW0480`–`DW0485`), cast ledger, and critical-path/branch plans mean no
   gallery quest, body or branch can dangle off the playable graph; the
   gallery declares all four machine-visible structures (cast, happenings,
   branch points, tiers) from day one, so none of its gates is unbound or
   unfenced.
3. **Warning-pinned**: the emitted warning set must equal a committed
   expected-warnings ledger exactly (code + pointer + one-line reason per
   row). Judgement-tier warnings are legitimate; *drifting* warnings are not.
   A new or vanished warning is a red, so "still green" can never quietly
   absorb "warns differently now".
4. **Run**: the gallery's generated PackTest suite runs in the existing
   required `tier 2` job on every PR. The full mineflayer critical-path and
   branch runs join the release-candidate tier, per the tiered-testing rule —
   a per-PR bot run is a cost §6 refuses on purpose.

The rejected alternative: per-element ablation builds (does removing the
element change the bytes?). Attribution is airtight but the build count goes
quadratic-ish with growth; the ladder above catches the same failure at the
layer that owns each half. Recorded so the next reviewer does not re-derive
it; revisit only if a real unreached-element escape is found.

## 5. The emission baseline

**What is committed.** Never the output tree — only, per build in the domain
(primary × declared languages, plus each overlay): a copy of that build's
`manifest.json` (the compiler's SHA-256 reproducibility index over inputs and
every output file), under `gallery/baseline/`. A header records the delvec
version, `dsl_version`, the gallery source-tree hash, and the generator-input
hash (§6). The comparison **asserts** the header — a baseline taken by a
different delvec, or over different source, refuses with its own message
instead of diffing noise (this is the queued finding about a baseline that
records its own SHA, adopted).

**The check.** CI builds the domain and byte-compares each manifest to its
committed copy. A mismatch is a red naming every differing output path. Two
distinct verdicts, because they mean opposite things:

- the PR touches gallery source or `crates/` — an **emission change**, and the
  red says: regenerate the baseline in this PR or explain the drift;
- the PR touches neither — a **determinism finding** (ADR-0006), named as
  such. The baseline is thereby also a standing cross-machine determinism
  probe, for free.

**Who updates it, and what stops the rubber stamp.** Only
`tools/gallery-baseline.py --write` regenerates it; it also writes
`gallery/baseline/delta.json`: every added/removed/changed emitted path,
grouped by output class (datapack function, structure, PackTest, plan, …). CI
recomputes the delta from the two baseline versions in git (merge-base vs
head) and asserts equality — a stale or hand-edited delta is a red, so the
delta always tells the truth about the update it rode in on. The delta file is
the review artifact and opens with the question its reviewer answers: *is
every path class listed here a consequence this PR claims to have?* An
unclaimed class is a finding. Which review lane reads it — and the fact that a
PR changing player-reachable emission is never mechanical — is operating
practice, referenced here and deliberately not restated.

A baseline updated in a PR with an empty delta is a red (a noise commit); a
baseline update is never split from the change that caused it.

## 6. Growth, cost, and the pieces

**The numbers are reported from day one.** Every run's job summary states:
units total / bound / refusal-proven; elements; builds performed; wall-clock
per build and total; emitted bytes per build; PackTest count contributed to
tier 2. The deterministic counts also live in the baseline header, so growth
is a diffable number, not a complaint. **No wall-clock threshold**: a timing
red on a shared runner is an intermittent red, and the debug doctrine forbids
re-running those, so a threshold would manufacture findings that cannot be
honestly retried. If the total ever crowds the PR budget, the sanctioned moves
are job splitting and caching; **sampling the unit set is the one forbidden
move** — a sampled coverage gate is the vacuity this spec exists to close.

**Prefabs.** The gallery's pieces are **generated at build time by the
engine's own generators** (`prefabs/` programs → `delvec grammar` → `.nbt` in
the build directory), passing admission like any piece. Committed source is
generator programs, never `.nbt`. Reasons, in order: the coverage gate's
same-PR property must not wait on a content re-pin (a gate whose discharge
needs the other repo is version-adoption lag wearing a gate's clothes); the
engine repo stays free of a second `.nbt` library (the layout rule keeps the
library in the content repo); the gallery then also exercises the
generator→admission→assembly path end to end; and the fresh-clone wart the
fixtures carry (tests reaching through the `campaigns/` symlink) is not
inherited — the gallery builds from this repository alone, which is the
creator-machine floor. The alternative — reuse pinned content prefabs like the
test suite does — is cheaper on day one and rejected for exactly the same-PR
reason. A piece a generator cannot express is a grammar gap, and spec-0033's
doctrine applies: grow the back end.

**dsl_version.** The gallery always declares the tree's current `dsl_version`
(asserted against `delvec --version`); a version-bump PR updates the gallery
in the same PR. This is the one campaign for which adoption rounds are
structurally free, and keeping it current is what keeps every fence-gated
proof non-inert.

## 7. Rendering

Some surfaces are only confirmable by a picture, and CI renders nothing
today. The split is established fact: the CPU arms (`viewer`, `palette`,
`scene`, `panorama`, `contact-sheet`, `index`) run anywhere; the GPU arms
(`piece`, `batch`, `fidelity-gate`) need a real adapter. Whether a software
adapter works on CI runners is **measured in the implementing PR, not
assumed** — and either answer changes only where frames come from, never what
the engine can do; an infrastructure limit does not decide a capability
question.

The machine half, asserted in CI regardless of the answer: the gallery carries
a committed render plan naming every declared view of every visual element;
every declared view is either produced or refused loudly (`DW0721` semantics —
a set is never silently short); every produced frame passes the featureless
check (`DW0727`'s `detect::is_featureless`); every view manifest states a
non-zero binding (`DW0726` discipline). If the GPU arms prove runnable, they
join with the fidelity gate (`DW0720`); if not, the CPU set is the CI floor
and the GPU shots are produced on a dev machine as part of visual review.
**Pixels are never baselined** — renderer output is not covered by ADR-0006's
byte guarantee across drivers; the manifests are the machine truth, the frames
are for eyes. The human half — does the element *look* right — happens
wherever visual review already happens, on the CI-artifact contact sheet or a
dev-machine render, and follows rule 8 of the playtest methodology
(instrument-bound vs artifact-bound).

## 8. Acceptance criteria

1. `tools/check-gallery-coverage.py` derives the unit set solely from `delvec
   schema --stage all` of the tree's own delvec; it enumerates zero units →
   red; it contains no parser of `stages.rs`.
2. Every unit is bound in the primary+overlay domain or refusal-proven by a
   committed probe that `delvec validate` rejects with the ledgered code at
   the gallery's `dsl_version`; any other state is a red naming the unit. The
   exemption ledger admits no entry without a passing probe.
3. Each overlay declares a non-empty `binds` set; the tool asserts every
   declared unit is bound by that overlay and unbound in the primary; a
   violation either way is a red.
4. The primary builds green (`build` exit 0, `analyze` green) in every
   declared language (≥ 2, so the sidecar surface is real); every overlay
   builds green in `en`; the gallery declares cast, happenings, branch points
   and tiers from its first commit.
5. The emitted warning set equals the committed expected-warnings ledger
   exactly (code + pointer per row); any delta is a red.
6. Compiler-stated binding counts (`RootBinding` and every stated-binding
   proof) appear in the coverage report; a zero binding on machinery the
   gallery writes is a red.
7. `gallery/baseline/` holds one manifest copy per build in the domain with
   the version/source header; CI byte-compares each and refuses (distinct
   message) on a header mismatch rather than diffing.
8. A manifest mismatch reds naming every differing path, classed
   emission-change vs determinism-finding by whether the PR diff touches
   `gallery/` or `crates/`; the determinism verdict cites ADR-0006.
9. The baseline changes only together with a `delta.json` that CI recomputes
   from the merge-base and head baseline versions and asserts byte-equal; an
   empty-delta baseline change is a red.
10. The job summary states, every run: units total/bound/refusal-proven,
    elements, builds, wall-clock per build, emitted bytes, PackTest count; the
    deterministic counts are asserted against the baseline header. No
    wall-clock value gates.
11. The gallery's generated PackTests run inside the required `tier 2` job on
    PR; its critical-path and branch bot runs are wired into the
    release-candidate tier only.
12. Render: every view in the committed render plan is produced or refused
    (`DW0721`), produced frames are non-featureless (`DW0727`), view manifests
    bind non-zero (`DW0726`); no pixel file is committed or compared.
13. The release workflow and the staging surface cannot name the gallery: a
    test asserts the gallery id is absent from release campaign discovery and
    unstageable.
14. The CI job is a required status check: its name lands in
    `.github/required-status-checks.txt` per that file's documented add
    procedure, `check-required-contexts.py` green.
15. Red→green falsification on the implementing PR, kept as a test in
    `tools/tests/`: a schema fixture gaining a unit with no gallery binding
    must red the gate; adding the binding must green it; deleting an overlay's
    only binding must red criterion 3.
16. Gallery JSON is inside `delvec fmt --check` discovery (a zero-file match
    is already `DW0774`); `docs/reference/compiler.md` and
    `docs/reference/tools.md` gain their rows in the implementing PR.

## 9. Not settled here

- **Whether CI runners can drive the GPU arms** via a software adapter —
  measured in the implementing PR; §7 defines both outcomes and neither moves
  a capability boundary.
- **Whether the domain's total build time stays inside the PR budget as the
  surface grows** — the number is reported from the first run (§6); the
  sanctioned relief moves are named there, and sampling is forbidden there.
- **Schema-walk edge semantics** beyond §3's rule (properties and variants are
  units; registry values are not): any case the implementation finds genuinely
  ambiguous is decided in the tool with a comment citing this spec, and the
  decision surfaces as a unit-count change the baseline header makes visible.
- **Whether `RootBinding`-style stated bindings are uniformly machine-readable
  today** (`--json` carries diagnostics; binding statements may need a small
  compiler-side report surface). If a report surface must be added, it is an
  ordinary engine change riding the implementing PR — criterion 6 stands
  either way, and weakening it to "printed somewhere" is not an option.
