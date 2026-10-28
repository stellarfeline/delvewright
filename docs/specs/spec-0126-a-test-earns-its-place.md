# spec-0126: A test earns its place — the suite is measured per test, a removal is proven by two instruments, and the run has a budget

- **Status**: Proposed
- **Ground**: written against engine `7d4a1a80d` (`origin/main`), read only; the integrated tree `9c6d1ca1a` (`origin/integrate/0117-0123-reach`) and its `rust` job log, read only, as the measurement subject. Files read: `.github/workflows/ci.yml` (the `rust` job, the `docs` job), `.github/required-status-checks.txt`, `.github/ci-reach.toml`, `.github/pins.toml`, `versions.toml` `[ci]`, `rust-toolchain.toml`, `Cargo.toml` `[profile.dev]`, `crates/delvec/tests/common/{mod,pinned,source_scan}.rs`, `crates/delvec/tests/{codes,cli,admit_fluid_predicates_agree}.rs`, `tools/ci/{check-dw-codes,check-gallery-coverage,check-source-dupes,check-doc-dupes,gallery-baseline}.py`, ADR-0006, ADR-0008, ADR-0023, ADR-0031 §6, spec-0039, spec-0122 (criteria 5, 7, 18 name the binaries that dominate the integrated tree's run).
- **What it is for**: the suite only grows. On `main` it is 5,065 `#[test]` functions in 558 files; on the integrated tree the `rust` job runs 373 test binaries whose own times sum to 10,222 s, five of them 9–30 min each, and the job takes more than three hours where `main`'s takes about 56 min. Nobody has measured what any test covers that another does not, and no line, region or branch coverage is measured anywhere in the tree (verified: `docs/reference/test-suite-measurement.md` §1). The constitution forbids weakening a test to get green; removing a test proven redundant is not weakening one, and until something can prove it, nothing can be removed. This spec gives the suite its instruments — a runner that times each test, a coverage measurement with a floor that refuses, a per-test coverage map, and mutation testing as the second instrument — states the rule under which a test is merged or removed, the shape tests are consolidated into, a time budget the run must meet, and one audit over the whole suite that applies them.
- **Research**: `docs/reference/test-suite-measurement.md` is the record; every rule below is marked **[cited]** with the section of that record or the constitution rule it rests on, or **[authored]** (this spec chooses). The trial of the instruments on this tree is its §6.
- **Numbers**: spec `0126`. No DW code: nothing here changes what the compiler says. `dsl_version` does not move: nothing here changes what a document says. Every tool this spec names is written without a number.
- **Non-goals**: making the engine cheaper to run (the reach-graph cost is a separate round's work; this spec judges the suite, not the engine); test-impact selection as a gate (§3.4 of the record: declined, with the reason); branch or MC/DC coverage (nightly-only, and the toolchain pin is immutable); a mutation gate on every PR (§1.3 says where mutation runs); a coverage *target* above the measured floor (coverage is a floor that cannot fall, never a number to chase — record §3.3); changing what any test asserts.

## 1. The instruments

### 1.1 The runner is cargo-nextest **[cited: record §5.1; authored where stated]**

`cargo test` reports nothing per test but an exit code per binary; libtest's own `--report-time` is unstable. The `rust` job's test step runs `cargo nextest run --workspace --locked --no-fail-fast --profile ci` instead. The `ci` profile in `.config/nextest.toml` writes a JUnit report (`[profile.ci.junit] path = "junit.xml"`) with one `<testcase>` per test carrying its `time`; the job uploads it as an artifact on every run, pass or fail. **Authored**: the runner is pinned by literal in `versions.toml` `[ci]` (`nextest_version`) and registered in `.github/pins.toml` with its policy, installed by the same action the job already uses for pinned instruments, and entered in `docs/ACKNOWLEDGEMENTS.md` (Apache-2.0 OR MIT). Process-per-test is a change of semantics for the 53 test files that hold in-process shared state (record §1): a fixture built once per binary today is built once per test under nextest, which is why §4.2 moves expensive fixtures to disk before the runner changes, and why the runner lands in the same PR as that move.

### 1.2 Coverage is region coverage, measured by the one test run **[cited: record §2.2, §2.3; authored where stated]**

The metric is **region coverage** of production source — the finest metric the pinned stable toolchain yields — with line and function coverage reported beside it. The instrument is `cargo-llvm-cov`, pinned by literal (`llvm_cov_version = "0.9.1"` in `versions.toml` `[ci]`, a `pins.toml` row, an acknowledgements entry; Apache-2.0 OR MIT) over the pinned toolchain's `llvm-tools` component, which is the only LLVM the counters may be read with. **Authored**: coverage is a by-product of the one test run, never a second run of the suite: the test step is `cargo llvm-cov --no-report nextest --workspace --locked --no-fail-fast --profile ci`, and the report step is `cargo llvm-cov report --json --remap-path-prefix` into the job's artifacts. A second, uninstrumented run of the suite is forbidden by this spec, because it would double the time §5 budgets.

**What is production source** is one question with one answer, and that answer already lives in `crates/delvec/tests/common/source_scan.rs`: a file's production text is the file with every item a column-zero `#[cfg(test)]` applies to removed, an out-of-line `#[cfg(test)] mod` taking its file and directory with it. `cargo-llvm-cov` excludes directories named `tests` by itself but cannot see a `#[cfg(test)]` item on stable (`coverage(off)` is nightly), so the coverage reader applies that rule to the export: every region whose span lies inside a `#[cfg(test)]` item is dropped from numerator and denominator alike, and the reader prints what it dropped — regions, files — as the size of what it skipped. The reader's copy of the rule is held equal to `source_scan`'s by an agreement test over generated cases (one question, one implementation; a second copy without the agreement test is the defect this constitution names).

### 1.3 Mutation testing is the second instrument, and runs where a removal is proven **[cited: record §4; authored where stated]**

`cargo-mutants` (pinned by literal, `mutants_version = "27.1.0"`, a `pins.toml` row, an acknowledgements entry; MIT) answers the question coverage cannot: whether a test *checks* the code it reaches. **Authored**: it is not a gate on every PR — a full run over this tree is hours, and a PR-scoped `--in-diff` run judges the diff's mutants against the whole suite, which is not the question a removal asks. It runs in two places: the audit (§6), over the mutants in the files the candidate tests cover; and every later PR that removes a test, under the rule of §3.3, with its `outcomes.json` pair committed as the proof. An opt-out an author can write — `#[mutants::skip]`, `#[cfg_attr(…, coverage(off))]`, `#[coverage(off)]` — is a property the defect can supply, so the coverage reader counts them over production source and refuses a count above zero.

## 2. The coverage baseline

### 2.1 The record **[cited: CLAUDE.md measurement doctrine; authored where stated]**

The baseline is a measurement, so it names its instrument by exact revision and carries a canonical hash of what was measured. It lives at `.github/coverage-baseline.json`, beside the other files the gates read, and is written only by `tools/ci/coverage-floor.py --write` from a report the instrument of §1.2 produced. Its fields:

| field | what it holds |
|---|---|
| `tree` | the full sha of the commit the suite was run at |
| `instrument` | `{ "rustc": "1.97.1", "cargo_llvm_cov": "0.9.1", "llvm_tools": <the version `llvm-cov --version` prints>, "nextest": <pinned> }` — each equal to the pin, held equal by `validation/check-versions.sh` |
| `command` | the two command lines of §1.2, verbatim |
| `regions`, `lines`, `functions` | `{ "covered": n, "count": n }` after the §1.2 filter — integers only; a percentage is derived, never stored |
| `excluded` | `{ "files": n, "regions": n }` — what the filter dropped |
| `unstable_regions` | the size of the symmetric difference between the covered-region sets of two runs of the suite at `tree` (§2.3) |
| `sha256` | over the canonical covered-region set: the sorted list of `(path, line, column, covered)` for every region entry, paths workspace-relative, one entry per line, LF — the one hash the trial showed stable across runs where the raw export's bytes are not (record §6) |
| `lowered_from` | absent, or the previous record's `regions` and `tree` when a PR lowered the floor (§2.2) |

### 2.2 What refuses, and what a PR that lowers it must carry **[authored]**

The gate is `tools/ci/coverage-floor.py`, run as the last step of the `rust` job over the report that job produced; the `rust` job is a required status check already, so the gate is bound to the event it guards — a pull request's admission to `main` — with no new context to register. It prints one binding line, `regions judged: N production regions in F files; excluded: E regions in X files (tests directories, cfg(test) items); covered: C (P %); floor: C₀/N₀ (P₀ %); headroom: +Δ points`, and refuses when:

- `P < P₀ − tolerance`, where `tolerance = unstable_regions / N₀` in points — a drop a flake could not have produced; or
- `N = 0` — an unbound run; or
- the record's `sha256` does not match the record's own `regions` block as recomputed by `--verify` from the stored region set of the run that wrote it (the set is not committed; the writer stores it as a CI artifact of the writing run, named in the record by run id — so a hand-edited number is caught at the next `--write`, and a record whose `tree` is not an ancestor of the PR head is refused outright: the floor is never set on a branch); or
- any `#[mutants::skip]` or `coverage(off)` attribute stands in production source.

A PR that lowers the floor rewrites the record with `--write` on its own tree and the record carries `lowered_from`; the gate prints `FLOOR LOWERED from P₀ to P by this pull request` on every run while that field is present, and the pull request is of the class that weakens a check. The next `--write` that raises the floor removes the field. Raising the floor is mechanical: a PR may run `--write` whenever headroom exists; a headroom above 1.0 point on `main` is a finding queued as the standing idle work, so the floor never drifts far below what the suite does. Nothing in the gate reads the per-file figures: they are information in the artifact, not a refusal, because a file's count moves when code moves.

### 2.3 The first record **[authored, on the trial]**

The floor is taken on the tree the audit PR branches from, *before* any test is merged or removed — so the audit's own removals are judged by it, and a removal that lowers coverage is refused by the gate it installed. Two runs are made; their covered-region sets' symmetric difference is `unstable_regions`. The trial on the `delvewright-dsl` crate gave 36,520 region entries with a symmetric difference of 0 between two runs and raw exports whose bytes differed (record §6), which is why the hash is over the set and the tolerance is measured rather than assumed. The whole-workspace figures are the audit's to measure; the record's first `regions` block is written down in the commit body that lands it.

## 3. Overlap: how a candidate is found, and what proves a removal

### 3.1 The per-test coverage map **[cited: record §2.4 construction 2, §6]**

`tools/ci/test-coverage-map.sh` builds the instrumented test binaries once under `cargo llvm-cov show-env --sh`, lists each binary's tests with `--list`, runs each test as its own process with `LLVM_PROFILE_FILE=<dir>/<binary>/<n>.profraw <binary> --exact <name>`, indexes each profile with `llvm-profdata merge` and exports it with `llvm-cov export --object <binary>`, and writes one JSON per test: the set of executed region entries keyed by `(path, line, column)` after the §1.2 filter, the test's wall time, and its outcome. The key is binary-independent, so sets from different binaries compare. A test whose process did not exit 0 has no set: its coverage is unknown, not small. The trial ran it on a two-test binary (sets of 461 and 502 entries, neither a subset of the other) and on the `dsl` unit binary; the map over the whole suite is the audit's first artifact and is stored as a CI artifact of the audit run, not committed.

### 3.2 Candidates **[cited: record §3.2; authored where stated]**

A test *t* is a **candidate** when any of these holds; the audit table (§6) names which:

- **subsumed**: R(t) ⊆ R(u) for some kept test *u*, where R is the executed-region set — the HGS representative-set condition with region as the requirement;
- **shared fixture**: *t* and *u* materialise the same campaign or world (same inputs by hash) and assert over it in separate processes — a merge candidate, never a removal;
- **near-duplicate scenario**: *t* and *u* differ only in a parameter (a block id, a DW code, a count) over the same code path — a table candidate.

A test with an empty set is not subsumed by everything; it is a finding of its own (it executes no production code) and is kept or removed on its assertions alone, stated in the table.

### 3.3 The proof **[cited: record §3.3, §4.3; authored]**

Coverage names candidates and proves nothing about assertions: two tests with equal region sets may assert different values, and a covered region may be asserted by neither. A test is removed or merged only under one of these, and the table names which:

1. **Merge, assertions preserved.** The test's assertion sites move whole into a kept test (a table row, or a second block over a shared build). The proof is textual and machine-checked: the count of assertion macros (`assert!`, `assert_eq!`, `assert_ne!`, `.expect(`-on-a-result-under-test is not one) and the multiset of DW codes asserted, over the files the merge touches, are equal before and after; `tools/ci/check-dw-codes.py` holds DW coverage over the merged tree as it does today.
2. **Remove, held by a kept test's assertions.** A kept test *u* asserts the same observable — the same DW code on the same input, the same emitted bytes, the same diagnostic text — and the table names *u* and the observable in words a reader can check against *u*'s source.
3. **Remove, held by the kill set.** For the files of R(t), `cargo mutants -f <file>… -- --test <every binary covering those files>` is run twice: with *t* present and with *t* skipped by the runner's filter; the two `outcomes.json` have equal caught sets. A mutant caught only with *t* names an assertion only *t* holds, and *t* is kept or its assertion moved under rule 1. Both `outcomes.json` are committed beside the table, named by the row.

Rules 2 and 3 both require rule 3.2's subsumption first: a test with regions no kept test executes is never removed, whatever its assertions.

### 3.4 The vacuity modes a removal must not hit **[cited: CLAUDE.md, *Vacuity*; authored]**

- **unbound**: a candidate judged on an empty or unknown set (a failing or killed process) — refused by §3.1's rule that only an exit-0 run has a set;
- **untraversed**: a per-test run that stopped early — every test runs alone, with `--no-fail-fast` meaningless and so not relied on; the map's row count is asserted equal to the `--list` count per binary;
- **unemitted**: a region hidden by the §1.2 filter or by an `--ignore-filename-regex` — the filter prints its size; the audit uses no regex beyond the default;
- **the opt-out the defect can supply**: `mutants::skip`, `coverage(off)` — counted and refused at zero (§1.3, §2.2);
- **a kill set judged by the wrong suite**: rule 3's run restricted to a subset of the covering binaries — the binaries are taken from the map, not chosen, and the command line is committed with the outcomes;
- **a merge that drops an assertion**: rule 1's count is over the touched files, not the file pair — an assertion moved to a third file counts, one deleted does not.

## 4. The shape tests are consolidated into

### 4.1 One integration crate per package **[cited: record §5.2]**

Each file under `tests/` is a binary that links the engine again, and cargo runs them one after another. `crates/delvec/tests/` becomes one crate, `tests/it/main.rs`, with one module per former file (`mod grammar_marks;` …) and `common` as a module of it; `crates/dsl/tests/` likewise. Test names keep their module path, so a nextest filter `test(grammar_marks::)` still names what the file named. `env!("CARGO_BIN_EXE_delvec")` keeps working inside the one crate. Cargo's own consolidation measured a 3× faster test-suite compile; the audit measures this tree's.

### 4.2 One build per world, on disk **[authored on record §5.1; ADR-0006]**

An expensive fixture — a materialised campaign, a built world, an assembled gallery point — is built by one helper, `common::built(<inputs>)`, that keys a cache directory under the target directory by the SHA-256 of its inputs and of the `delvec` binary that builds it, builds on a miss, and returns the path on a hit. Determinism (ADR-0006) is what makes the cache correct: the same inputs and the same binary give the same bytes, so a second test, in a second process or a second binary, reads what the first built. In-process `OnceLock`/`LazyLock` fixtures are replaced by this helper where the fixture is expensive, and left alone where it is cheap; the 53 files that hold such state are each named in the audit table with which. The cache is under `target/`, is never committed, and is cleared by `cargo clean`.

### 4.3 Table-driven cases **[authored]**

Where tests differ only in a parameter over one code path, the cases become one table. Two forms, chosen by what the rows share: when each row needs its own build or its own process, a `cases!` macro expands one `#[test]` per row so nextest times and isolates each; when the rows share one built world and are cheap, one `#[test]` iterates the table and names the failing row in its panic message. A row is never silently skipped: a table's row count is part of the test's output.

### 4.4 Helpers: one question, one implementation **[cited: CLAUDE.md, *One question, one implementation*]**

A helper that answers a question about the tree — what is production source (`source_scan`), where the pinned content is (`pinned`), how a campaign is materialised (`materialize`) — has one home in `common` and no second copy in a test module; a second copy is a `tools/ci/check-source-dupes.py` finding, which the audit extends to the test tree if it does not already reach it. Every consumer of a question the gates also ask (the §1.2 filter and `source_scan`) is held to it by an agreement test.

### 4.5 What stays intact **[cited: CLAUDE.md, *DW-diagnostic coverage*; *agreement tests*; ADR-0006]**

Agreement tests (`*_agree*.rs`, `detail_check_agrees`) are kept by class: their two consumers are their denominator and no coverage argument reaches them. DW-diagnostic coverage is held by `check-dw-codes.py` over the merged tree, so a removal that strips a code's only test reds there, which is the pair this spec relies on. The determinism double-build gate inside `cargo test` (`tests/cli.rs`) is one test under nextest as it is one test today. `codes.rs`, grammar coverage and gallery coverage are unchanged.

## 5. The time budget **[cited: record §5.3, §5.1; authored where stated]**

The whole suite is the PR gate here, so it must fit a pull request's patience; the established figure for a PR suite is rustc's, about 40 minutes. **Authored**, three numbers, each a hard bound in the place that can enforce it:

| bound | value | enforced by |
|---|---|---|
| the `rust` job | 40 min | `timeout-minutes: 40` on the job — a run that exceeds it is cancelled and red |
| the test run | 25 min | `global-timeout = "25m"` in `[profile.ci]` of `.config/nextest.toml` |
| one test | 120 s | `slow-timeout = { period = "60s", terminate-after = 2 }` in `[profile.ci]`; a test over 60 s is printed `SLOW`, over 120 s is terminated and fails |

A test that needs more than 120 s declares it in `[[profile.ci.overrides]]` with `filter = 'test(=<name>)'`, its own `slow-timeout`, and a comment carrying its measured time and why its scenario cannot be smaller; the override list is enumerated by the acceptance criterion with its count, so it cannot grow unseen. The two measurements that judge the budget share no configuration: the JUnit `time` per test (the runner's own clock, summed per binary and whole) and the job's step durations from the Actions run (GitHub's clock); the audit records both, and a budget PR carries both. When a PR exceeds a bound the job is red; the remedy is a cheaper fixture (§4.2), a split, or a table (§4.3), never a raised number; raising any of the three is a PR of the class that weakens a check and carries the two measurements that say why. The budget is armed by the audit PR, which is the first PR that meets it; until a tree meets it, the criterion is a recorded debt, never a pass.

## 6. Rollout: one audit, then the standing gate **[authored]**

One feature branch, one pull request into `main`, in this order on the branch — each step leaves the branch green under the checks that exist at that step:

1. **Instruments.** Pins and acknowledgements for nextest, cargo-llvm-cov, cargo-mutants; `.config/nextest.toml` with the `ci` profile (JUnit on, no timeouts yet); the `rust` job's test step as §1.2; `coverage-floor.py` with the first record taken on the branch point by two runs (§2.3); `test-coverage-map.sh`; the agreement test of §1.2.
2. **The map.** `test-coverage-map.sh` over the whole suite on the remote build machine, twice; the map and the JUnit times are the audit's inputs, stored as the run's artifacts and named by run id in the table.
3. **The audit table**, `crates/delvec/tests/measured/suite-audit.json`: one row per test file of the pre-audit tree (enumerated by `git ls-tree` at the branch point — the denominator is that listing, 365 files on the integrated tree), with `file`, `tests`, `seconds` (JUnit sum), `unique_regions` (regions no other file executes), `shared_state` (§4.2: `disk`, `cheap`, or `none`), `verdict` ∈ {`keep`, `merge into <file>`, `remove: held by <test>`, `remove: held by kill set <path>`}, and `proof` (the rule of §3.3 and its artifact). Verdicts of `keep` need no proof; every other verdict needs its artifact to exist in the tree.
4. **The refactor**: §4.1 one crate per package, §4.2 fixtures to disk, §4.3 tables, in the order the table's merges name.
5. **The budget**: the three numbers of §5 into the job and the profile; the override list, enumerated.
6. **The second record**: `--write` after the audit; the gate prints the headroom the audit bought.

The standing gate after the PR is §2.2 on every pull request, and a removal row in the table for every later removal, judged by `tools/ci/check-suite-audit.py`: every row's `file` existed at the tree it names, every verdict is in the set, every named holder and artifact exists, and the table's row count equals the denominator it names.

## 7. Open questions, and the constitution's wording

**Does any CLAUDE.md line need an owner-confirmed amendment for removal-with-proof to be legal?** Read as written, no. The debug doctrine forbids weakening *to get green*; a removal under §3.3 is not made to get green, and the floor of §2.2 makes a coverage-lowering removal red, not green. *Nothing owes compatibility to anything already built* permits the deletion of a test whose assertions are held elsewhere. The DW-coverage line is held by its own gate. What the constitution lacks is not permission but the name of the proof, and this spec is where the proof is named; the constitution says a rule is stated once. If the owner wants the removal class visible in the constitution, the line proposed is one bullet under *Methodology*, and it is offered, not required:

> **A test is removed only against its proof**: its executed regions are held by kept tests and its mutant kill set is unchanged without it, or its assertions moved whole into a kept test; the removal is a row in the suite's audit table, and the coverage floor never falls to make room for it.

**Other open points**, each decided here unless the owner rules otherwise: nextest's semantics change (process per test) is taken with the disk-cache move in one PR, not staged; the floor's tolerance is the measured `unstable_regions`, which may turn out to be 0 on the whole suite as it was on `dsl`; the audit's mutation runs are scoped by `-f` to the files a candidate covers, never the whole tree.

## Acceptance criteria

Each criterion names what it counts and over what.

1. `versions.toml` `[ci]` carries `nextest_version`, `llvm_cov_version` and `mutants_version` as literals; `.github/pins.toml` carries a row for each with `policy`, `bound_by` and `why`; `validation/check-versions.sh` holds every consumer in `.github/workflows/ci.yml` equal to them; `docs/ACKNOWLEDGEMENTS.md` carries one entry each with the verified licence. Count: 3 pins, 3 rows, 3 entries, 0 unbound consumers.
2. The `rust` job's test step is `cargo llvm-cov --no-report nextest --workspace --locked --no-fail-fast --profile ci` and no step in any workflow runs `cargo test` over the workspace; `.config/nextest.toml` `[profile.ci.junit]` names a path and the job uploads that file and the coverage JSON as artifacts with `if: always()`. Count: 1 test step, 0 `cargo test` invocations over `grep -nE 'cargo test' .github/workflows/*.yml` (the determinism gate's own invocations inside `tests/cli.rs` are not workflow steps).
3. `tools/ci/coverage-floor.py` prints the binding line of §2.2 with every field, exits 1 on each of the four refusals, and `tools/tests/test_coverage_floor.py` asserts each refusal by perturbation: a report with `P` one region below the tolerated floor; a report with zero judged regions; a record whose `regions` block is hand-edited against its `sha256`; a production file carrying `#[mutants::skip]`. Count: 4 refusals, 4 tests, each red on its perturbation and green on the unperturbed input.
4. `.github/coverage-baseline.json` exists with every field of §2.1; its `instrument` values equal the pins; its `tree` is an ancestor of `main`; its `regions.covered`, `regions.count` and `unstable_regions` are stated in the commit body that lands it with the two run ids; `lowered_from` is absent.
5. The `#[cfg(test)]` filter inside `coverage-floor.py` and `crates/delvec/tests/common/source_scan.rs` are held equal by an agreement test: `crates/delvec/tests/it/source_scan_agrees.rs` (or the module of that name) generates at least 50 synthetic files mixing inline and out-of-line `#[cfg(test)]` items and asserts both rules keep the same production spans. Count: ≥ 50 cases, 0 disagreements.
6. `tools/ci/test-coverage-map.sh` exists, is listed in `docs/reference/tools.md` with class `agent`, and over the audit tree writes one JSON per test with `regions`, `seconds`, `exit`; the per-binary row count equals that binary's `--list` count for every binary, and the number of tests with `exit != 0` is stated. Denominator: the `--list` total.
7. `crates/delvec/tests/measured/suite-audit.json` has one row per `.rs` file under `crates/*/tests/` at the branch point (`git ls-tree -r --name-only <tree> -- 'crates/*/tests/*.rs'`, excluding `common/`, `support/`, `fixtures/`, `measured/`), with the fields of §6 step 3; `tools/ci/check-suite-audit.py` asserts the row count equals that listing, every verdict is in the closed set, every `merge into` target and `held by` test exists in the post-audit tree, and every `kill set` path exists. Count: rows = files; 0 rows with a missing holder or artifact.
8. For every `remove: held by kill set` row, the committed pair of `outcomes.json` has equal caught sets, and the committed command line names every binary the map says covers the row's files; `check-suite-audit.py` asserts the equality and the binary list. Count: pairs = such rows; 0 unequal.
9. For every `merge into` row, the assertion-macro count and the DW-code multiset over the touched files are equal before (at the branch point) and after; `check-suite-audit.py --merges` computes both and prints them per row. Count: 0 rows with a dropped assertion or code.
10. `crates/delvec/tests/it/main.rs` and `crates/dsl/tests/it/main.rs` exist; `crates/delvec/tests/*.rs` and `crates/dsl/tests/*.rs` hold no other file; `cargo nextest list --workspace` reports a test count equal to the pre-audit total minus the removed rows plus the table rows added, stated in the commit body with the three numbers.
11. `common::built` exists, keys its cache by the SHA-256 of inputs and of the `delvec` binary, and a test asserts: two calls with equal inputs in two processes return one directory and build once (a build-count file beside the cache); a changed input builds again. Every `shared_state = disk` row's file calls it; the audit table's `shared_state` column has 53 non-`none` rows at most and the count of each value is stated.
12. `tools/ci/check-dw-codes.py`, `tools/ci/check-gallery-coverage.py`, `delvec grammar coverage` and every `*_agree*` test are green on the post-audit tree with the same binding counts as at the branch point, stated side by side in the commit body.
13. The `rust` job carries `timeout-minutes: 40`; `[profile.ci]` carries `global-timeout = "25m"` and `slow-timeout = { period = "60s", terminate-after = 2 }`; every `[[profile.ci.overrides]]` entry carries `filter = 'test(=…)'`, a `slow-timeout`, and a comment with a measured time; the override count is stated in the commit body and `check-suite-audit.py --overrides` asserts it equals the entries present. The audit PR's `rust` job completes within 40 min on the merge candidate with the JUnit sum and the Actions step duration both stated; if it does not, the criterion is recorded as debt in the PR body in those words and the three bounds are not armed.
14. `docs/reference/tools.md` lists `coverage-floor.py` (class CI), `check-suite-audit.py` (class CI) and `test-coverage-map.sh` (class agent); `docs/reference/test-suite-measurement.md` §1's figures are re-read on the post-audit tree and the new readings added beside the old with the tree named; `.github/ci-reach.toml` gains the inputs of the new gate (`.config/nextest.toml`, `.github/coverage-baseline.json`) with a `why` each.
