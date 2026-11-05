# Measuring the test suite

How the engine's Rust test suite is measured — what runs it, what it covers, what each test proves, and what it costs — and what the established practice says about shrinking a suite without losing what it proves. Agent-facing: the reader is an engineer adding a test, auditing the suite or reading the coverage gate. The decisions taken on this record are spec-0126; this page is the research behind them and the instrument definitions the spec names.

Every rule is marked **[cited]** with its source, or **[authored]** — derived here from the tree's own measured shape, or reasoned from a cited rule without a source of its own. §7 lists the sources once. §8 records what could not be established.

## 1. The suite as it stands **[authored, measured]**

The figures below are read from the tree and from one CI log, by the commands in the commit that lands this page. Each names its denominator.

| Reading | Value | Where read |
|---|---|---|
| `#[test]` functions | 5,065 in 558 `.rs` files (`dsl`: 207 unit + 623 integration in 82 files; `delvec`: 1,277 unit + 2,958 integration in 283 files) | `grep -rE '^\s*#\[test\]' crates` on `main` |
| `#[ignore]` tests | 14 | the same tree |
| Test binaries run by `cargo test --workspace` | 373 | the `rust` job log of the integrated tree, one `Running …` line per binary |
| Tests the log passed | 5,127 passed, 0 failed, 12 ignored | the same log's `test result:` lines |
| Sum of the binaries' own `finished in` times | 10,222 s | the same lines, summed |
| Share of that sum in the five slowest binaries | 5,196 s (50.9 %); ten slowest 7,210 s (70.6 %); twenty slowest 8,489 s (83.1 %) | the same lines, sorted |
| Binaries by test count | 22 run one test or none; 147 run 2–5; 168 run 6–20; 36 run more than 20 | the same lines |
| Integration files that spawn the built `delvec` as a process | 57 of 283 (`delvec`) | `grep -lE 'cargo_bin\(|Command::new\(.*delvec|assert_cmd'` |
| Integration files that build a campaign through a `common::` helper | 93 of 283 | `grep -lE 'materialize|campaign_at|campaign_bound_to'` |
| Shared test helpers | 3,948 lines in 12 files under `crates/*/tests/{common,support}/`; `crates/delvec/tests/common/mod.rs` alone is 1,610 lines and 48 `pub fn` | `wc -l` |
| Files holding in-process shared state (`OnceLock`, `LazyLock`, `lazy_static`, a `static Mutex`) | 53 | `grep -l` over `crates/*/tests` |
| Wall time of the `rust` job | about 56 min on `main`; more than 3 h on the integrated tree | the Actions run pages |
| `timeout-minutes` on the `rust` job | none | `.github/workflows/ci.yml` |

What the shape says: the suite's cost is concentrated. Five binaries hold half the test time and run 65 tests between them; the slowest, `starter_program`, runs one test for 1,834 s. Those five all exercise one feature line (place authoring and its check) on the integrated tree; their engine-side cost is a separate round's work. The other 368 binaries hold the remaining half, spread over 5,000 tests whose overlap nobody has measured.

### Re-read on the post-audit tree **[authored, measured]**

The same readings on the tree spec-0126's audit leaves (`feat/spec-0126` at `14924f389`), beside the runs that measured it. The suite runs as one nextest run per package's two crates (the library's unit tests and `tests/it/`); every run is on the remote build machine (16 cores) unless it names the Actions runner.

| Reading | Value | Where read |
|---|---|---|
| `#[test]` functions | 5,213 in 585 `.rs` files (`dsl`: 227 unit + 653 integration in 82 modules; `delvec`: 1,351 unit + 2,982 integration in 292 modules) | `grep -rE '^\s*#\[test\]' crates` |
| `#[ignore]` tests | 12 | the same tree |
| Test binaries | 5 (`delvec` library, `delvec` binary, `delvec::it`, `delvewright-dsl` library, `delvewright-dsl::it`) | nextest's `Starting … across 5 binaries` |
| Tests run | 5,201 run, 5,201 passed, 12 skipped; and 10 doc tests (`cargo test --doc`) | uninstrumented nextest run at `74167bd9b` |
| Uninstrumented run (`cargo nextest run --workspace --profile ci`) | 1,581 s wall, 2 m 09 s of it the build; per-test JUnit times sum to 20,479 s; the longest test 1,190 s | the same run |
| The old test step at the branch point (`cargo-test-concurrent.py --jobs 2`, 375 executables) | 1,791 s wall, 3 m 18 s of it the build; the executables' own times sum to 3,022 s; 5,207 passed + 10 doc tests | `80fdb68d2` |
| Instrumented run, the `rust` job's step (`cargo llvm-cov --no-report nextest …`) | 5,294 s wall, 3 m 03 s of it the build; JUnit sum 64,982 s; the longest test, `detail_check_agrees::check_and_commit_agree_over_every_gallery_place_and_three_refusals`, 3,917 s; 54 tests over 120 s, 127 over 60 s | run D at `5111f07a0` |
| The same instrumented run before consolidation (374 binaries) | 5,200 s wall, 5 m 02 s build; JUnit sum 63,926 s | run C at `d262886ea` |
| Region coverage of production source | 231,890 of 246,511 (94.07 %); lines 93.51 %; functions 94.27 %; 51,823 regions in 206 files excluded as `cfg(test)` items | `.github/coverage-baseline.json` |
| Per-test coverage map | 5,201 tests over 426,217 production region entries; two passes differ on 56 tests by 11,005 regions; 0 tests whose region set is a subset of another test's | `crates/delvec/tests/measured/suite-audit.json` `measured` |
| Integration modules that spawn the built `delvec` | 103 of 292 (`delvec`; the regex above with `CARGO_BIN_EXE_delvec` added, 103 at the branch point too) | `grep -lE` over `crates/delvec/tests/it` |
| Shared test helpers | 4,171 lines in 13 files under `crates/*/tests/{common,support}/` | `wc -l` |
| Files holding in-process shared state | 54 (the 53 above and `common/built.rs`) | `grep -l` over `crates/*/tests` |
| Instrumented run on the Actions runner (`ubuntu-latest`, the `rust` job) | three runs: JUnit sum 59,439 s, 54,852 s, 60,859 s; nextest wall 15,799 s, 15,092 s, 16,208 s (3.76, 3.63, 3.75 tests running on average); test step 4 h 30 m 43 s, 4 h 22 m 03 s, 4 h 38 m 20 s, of it the build 7 m 23 s, 10 m 30 s, 8 m 10 s; job 4 h 36 m 34 s, 4 h 33 m 15 s, 4 h 46 m 41 s | three runs of the `rust` job on Actions, at `a54b886ed`, `e39566796` and `515bc86f1`: the JUnit artifact and the step times |
| Slow tests on the Actions runner | over 60 s: 133, 129, 136; over 120 s: 71, 65, 71 (73 distinct over the three runs); the longest, `detail_check_agrees::check_and_commit_agree_over_every_gallery_place_and_three_refusals`, 4,512 s, 3,600 s, 4,504 s; four tests over 25 min | the same JUnit files |
| Tests of 60 s or less on the Actions runner | JUnit sum 22,261 s, 20,474 s, 22,465 s | the same JUnit files |
| The same run with `main` merged in (5,208 tests) | at `4720f9fca`: JUnit sum 54,917 s, nextest wall 15,080 s (3.64 at once), test step 4 h 22 m 17 s, job 4 h 34 m 05 s; 130 tests over 60 s, 68 over 120 s, the longest 3,612 s. At `635327346`: the test run was cancelled by GitHub's 6-hour job limit after 20,540 s, with no JUnit written | two runs of the `rust` job on Actions at those revisions |

What the re-read says: instrumentation is the cost. The same tree runs in 1,581 s uninstrumented and 5,294 s instrumented, and the instrumented run's critical path is one test of 3,917 s, so no arrangement of the suite brings the instrumented run under spec-0126's 25-minute bound on this tree; consolidation into five binaries took the build from 5 m 02 s to 3 m 03 s and left the run's wall clock where it was, because the run is bound by its longest tests, not by linking.

On the Actions runner the bound is the sum, not the longest test: 3.63–3.76 tests run at once, so the run takes the JUnit sum divided by that, and the tests of 60 s or less alone take 20,474–22,465 s of it, 94–100 min of wall clock before any slow test runs. No fixture, split or table reaches spec-0126 §5's 25-minute bound on this runner while the suite runs instrumented in one job.

What was **not** measured anywhere in the tree read first: code coverage by line, region or branch (the re-read tree measures region, line and function coverage; branch coverage stays nightly-only). What exists is surface coverage — gallery units (spec-0039, `tools/ci/check-gallery-coverage.py`), DW-diagnostic coverage (`tools/ci/check-dw-codes.py`) and grammar demonstration (`delvec grammar coverage`). Each proves a *surface* is exercised; none says which source regions a test executes, so none can say whether two tests execute the same ones. Verified by `grep -rilE 'llvm-cov|instrument-coverage|grcov|tarpaulin|cargo-mutants|nextest'` over the tree: one hit, a sentence in ADR-0031 that names `cargo-nextest` as a step not decided there.

### Re-read partitioned, at one engine thread **[authored, measured]**

The suite as `.github/workflows/ci.yml` runs it at `aaff1f312`: one instrumented archive, the `rust fixtures` job, and 13 `rust partition` jobs at `DELVEC_THREADS=1` (spec-0126 §5). Every figure is read from the Actions runs named, through `tools/ci/check-partitions.py` (the census) and each partition's JUnit report. The JUnit reports are the runner's clock; the job durations are GitHub's.

| Reading | Value | Where read |
|---|---|---|
| Tests | 5,249 listed (12 ignored), 5,249 run, each in exactly one of 13 partitions; 381–434 per partition | the census, runs 42001300305 and 42010396034 |
| JUnit sum, whole suite | 50,061 s and 47,130 s; at `DELVEC_THREADS` unset, 57,287 s (run 41939107132, ba7a0ad99, N = 30) | the partitions' JUnit reports |
| The same tree with only the engine's thread count changed | JUnit sum 57,287 s → 37,005 s; tests over 120 s 92 → 48; the longest test, `compiler::surround::tests::a_carved_staircase_up_the_inner_slope_is_dw0369`, 1,010 s → 1,303 s (a unit test, so in-process at one thread) | run 41939107132 against run 41979829454 (acf099ea1), both N = 30 |
| Partitions' nextest walls, run 42001300305 | 474–973 s for 10 of 13; p10 1,159 s; p9 2,113 s and p13 2,343 s on runners 2.44 and 2.41 times as slow as the median | the JUnit `<testsuites time>`; the ratio is the partition's median per-test time against the mean of runs 41979829454 and 41992688769 |
| Partitions' nextest walls, run 42010396034 | 528–1,224 s for 12 of 13; p8 1,896 s on a runner 2.44 times as slow | the same |
| Partition jobs | 8.8–20.1 min, p9 36.2 and p13 39.8 (run 42001300305); 9.6–21.2 min, p8 32.6 (run 42010396034) | the Actions job durations |
| Build job, fixtures job | 10.4 and 13.3 min; 10.6 and 10.8 min | the same |
| Run start to the `rust` job's end | 64.6 min and 55.2 min | the same |
| One test's time between two runs | up to 4.44 times (68 → 300 s); one partition's median, up to 2.75 times | runs 41979829454 and 41992688769, tests above 30 s in both (217) |
| Region coverage of production source | 232,027 of 246,664 (94.07 %); lines 141,080 of 150,881 (93.50 %); functions 14,393 of 15,267 (94.28 %); the floor 231,874 of 246,511 | `tools/ci/coverage-floor.py` over run 42001300305's export; run 42010396034's is equal |

What the re-read says: one engine thread per test cut the suite's CPU time by a third, not by the two thirds a 16-core estimate gave, and made the longest in-process test slower. The partitions' walls are set by the runner more than by the partition: every run measured put one to three of 13 partitions on a runner more than twice as slow as the median. On a median runner every partition ran within 25 minutes.

## 2. Code coverage in Rust

### 2.1 The mechanism **[cited: rustc book, *Instrumentation-based code coverage*]**

`rustc -C instrument-coverage` makes the compiler insert calls to the LLVM intrinsic `llvm.instrprof.increment` *"at functions and branches in compiled code, to increment counters when conditional sections of code are executed"*, and embeds a coverage map (LLVM Code Coverage Mapping Format) that *"define[s] the code regions (start and end positions in the source code) being counted"*. An instrumented program writes its counters to a `.profraw` file at exit; the path comes from `LLVM_PROFILE_FILE`, whose pattern takes `%p` (process id), `%h` (host), `%t` (`TMPDIR`), `%Nm` (the binary's signature, pooled for online merging) and `%c` (continuous sync). The raw files are indexed by `llvm-profdata merge` and read by `llvm-cov report | show | export`, with `--object <binary>` naming the instrumented binary. The tools ship as the rustup component `llvm-tools` (named `llvm-tools-preview` in the book) and must match the compiler's LLVM.

The book's own testing recipe produces *"one `.profraw` for each test binary"* and merges them; it makes no claim of per-test attribution. Attribution finer than the binary is therefore the caller's construction (§2.4).

### 2.2 The metrics **[cited: rustc book, the same chapter]**

- **Region coverage** — *"the percentage of code regions that have been executed at least once. A code region may span multiple lines"*; the book calls it *"the most granular"*. `return x || (y && z)` is three regions.
- **Line coverage** — executable lines within function bodies executed at least once.
- **Function coverage** — a function counts as covered if any instantiation ran.
- **Branch coverage** and **MC/DC** exist behind `-Z coverage-options`, which the book marks unstable (nightly).

**Authored on the above.** The engine is built on the pinned stable toolchain `1.97.1` (`rust-toolchain.toml`, pin `rust-toolchain`, policy immutable). Branch coverage is therefore not available to this project without a second toolchain; region coverage is the finest stable metric and is the one spec-0126 adopts. Line coverage is reported beside it because it is what a reader of a source file can see; function coverage is reported because a function with zero instantiations executed is the shape §4.2 calls a pseudo-tested candidate's neighbour.

### 2.3 The wrapper: `cargo-llvm-cov` **[cited: taiki-e/cargo-llvm-cov README]**

`cargo llvm-cov` is *"a wrapper around rustc's `-C instrument-coverage`"* that builds into its own target directory, sets the environment, runs the tests and reports. What the README commits to, by flag name:

- `cargo llvm-cov` runs `cargo test`; `cargo llvm-cov nextest` runs `cargo nextest run`; `cargo llvm-cov run` runs a binary; `--no-run` reports without running.
- `--no-report` runs without reporting, so several runs merge; `cargo llvm-cov report --json|--lcov|--cobertura|--codecov|--html` reads what the runs left. `cargo llvm-cov clean --workspace` empties the profile directory first.
- `--json` exports LLVM's own format with per-file `segments` (line, column, count, has-count, is-region-entry, is-gap); `--summary-only` keeps the totals.
- `--remap-path-prefix` writes workspace-relative paths; `--ignore-filename-regex <re>` drops files from the report; code under a directory named `tests` is excluded by default.
- `--fail-under-lines|regions|functions <pct>` and `--fail-uncovered-lines|regions|functions <max>` exit 1 against a threshold; `--show-missing-lines` lists what is not covered.
- `--branch` and `--mcdc` are marked unstable and need nightly; `--doctests` likewise.
- `cargo llvm-cov show-env --sh` prints the environment (`RUSTFLAGS`, `LLVM_PROFILE_FILE`, `CARGO_TARGET_DIR`, …) so an external build and run can be reported: `source <(cargo llvm-cov show-env --sh); cargo llvm-cov clean --workspace; cargo build …; <run>; cargo llvm-cov report`.
- `LLVM_PROFILE_FILE_NAME` overrides the file-name part of the profile path.
- It sets `RUSTC_WRAPPER` by default (`--no-rustc-wrapper` disables); a host that already uses a wrapper (`sccache` on the remote build machine) runs it with `RUSTC_WRAPPER=` empty.
- Licence: Apache-2.0 OR MIT. Install floor rustc 1.87. Current stable release on crates.io: `0.9.1`.

### 2.4 Per-test attribution **[authored on §2.1 and §2.3; trial in §6]**

Nothing in the rustc book or the wrapper's README attributes coverage to one test. Two constructions give it; both are built from cited parts.

1. **One process per test, one profile per process.** `LLVM_PROFILE_FILE` with `%p` writes one `.profraw` per process. A runner that runs each test in its own process (cargo-nextest, §5.1) therefore leaves one file per test. Mapping file to test needs the pid, which the runner does not print; a wrapper around the test command can write it. Experimental on the runner's side.
2. **One `--exact` invocation per test.** Build the instrumented test binaries once under `show-env`; list each binary's tests with `--list`; run each test as `LLVM_PROFILE_FILE=<dir>/<binary>/<test>.profraw <binary> --exact <name>`; `llvm-profdata merge` and `llvm-cov export --object <binary>` per file. Every part is a documented flag of libtest (`--list`, `--exact`; rustc book, *Tests*) or of LLVM. Cost is one process start per test plus the test's own time; §6 measures it.

Spec-0126 takes construction 2 for the audit instrument because every step is a stable, documented flag and the mapping from file to test is by construction, not by pid.

## 3. Shrinking a suite: what the literature establishes

### 3.1 Suites grow until they are too expensive to run whole **[cited: Yoo & Harman 2012]**

The survey of regression-testing minimisation, selection and prioritisation states the premise: test suites tend to grow as software evolves, often making it too costly to execute whole, and divides the remedies into three — **minimisation** removes redundant tests permanently, **selection** picks the tests relevant to a change, **prioritisation** orders tests so faults surface sooner.

### 3.2 Coverage-based minimisation: the representative set **[cited: Harrold, Gupta & Soffa 1993]**

The HGS method selects a *representative set* of a suite that satisfies the same testing requirements as the whole, by identifying and eliminating redundant and obsolete test cases; it is independent of the testing methodology and *"only requires an association between a testing requirement and the test cases that satisfy the requirement"*. Under region coverage the requirement is a region and the association is the per-test region set. A test whose set is a subset of another's is redundant *under that requirement*.

### 3.3 Why the requirement is not enough **[cited: Rothermel, Harrold, von Ronne & Hong 2002; Inozemtseva & Holmes 2014; Vera-Pérez, Danglot, Monperrus & Baudry 2019]**

- Rothermel et al. measured reduced suites against the originals on C programs with seeded faults and report that fault-detection capability was significantly compromised by coverage-adequate reduction. Later studies on JUnit suites found smaller losses; the finding stands that coverage-equivalent is not fault-detection-equivalent.
- Inozemtseva & Holmes, over 31,000 generated suites of five Java systems scored by mutation, found a *"low to moderate correlation between coverage and effectiveness when the number of test cases in the suite is controlled for"*, and conclude coverage should not be used as a quality target.
- Pseudo-tested methods — methods that are *covered* yet whose body can be removed without any test failing — appear in every subject studied. A test can execute a region and assert nothing about it.

**Authored on the above.** Two tests with equal region sets may assert different things; a test whose region set is subsumed may be the only one that asserts a value in that region. Coverage subsumption finds *candidates*; it proves nothing about assertions. The proof that a removal loses nothing needs a second instrument that observes assertions, which is what mutation testing measures (§4).

### 3.4 Test-impact selection **[cited: Gligoric, Eloussi & Marinov 2015; Memon et al. 2017]**

Ekstazi records, per test class, the files it loads, and reruns only tests whose recorded files changed; across 615 revisions of 32 projects it cut end-to-end testing time by 32 % on average. Google's continuous-testing study reports that most tests rarely fail and that failing tests sit close to the code they exercise, and that code recently edited by more than three developers broke more often.

**Authored on the above.** Selection is a loosening by construction: it skips a test on the claim that the change cannot reach it, and the claim is only as good as the dependency record. This repository already selects at job granularity by path (`.github/ci-reach.toml`, the `changes` job), with every input enumerated and its reason beside it. Spec-0126 does not select tests below that: every test runs on every PR that reaches the `rust` job, and the per-test coverage map serves the audit and a developer's local loop, never a gate.

## 4. Mutation testing as the second instrument

### 4.1 What it observes **[cited: cargo-mutants README and book]**

`cargo-mutants` *"finds places where bugs could be inserted without causing any tests to fail"*: it copies the tree, applies one mutation at a time (replacing a function's body with a constant, flipping an operator, …), builds, runs the tests, and classifies each mutant as **caught** (a test failed), **missed** (every test passed), **unviable** (did not compile) or **timeout**. Its own framing is the one §3.3 needs: coverage shows which code tests reach; mutation testing shows whether tests check the code's behaviour.

### 4.2 The flags that matter here **[cited: mutants.rs chapters]**

- Scope: `-f <file>` restricts mutants to a file; `--in-diff <file>` restricts to code a diff touches (the book's CI chapter: PR-scoped runs are typically much faster than full ones and *"can miss some problems that would be found by running mutants on the whole codebase"*).
- Tests: arguments after `--` go to `cargo test`, so `-- --test <binary>` restricts the suite that judges each mutant; `--test-tool nextest` uses nextest.
- Parallelism: `--jobs`; `--shard k/n` with `--sharding slice|round-robin`, each shard discovering all mutants and taking its slice; the book suggests at least ten mutants per shard and 8–32 shards as a start; `--baseline=skip` skips the unmutated run on each shard when an earlier job proved the suite green.
- Time: the default test timeout is five times the baseline test time with a 20 s floor; `--timeout <s>`, `--timeout-multiplier`, `--minimum-test-timeout`; `--build-timeout` off by default; with `--baseline=skip` or `--in-place` and no explicit timeout the test timeout is 300 s.
- Output: `mutants.out/` holds `mutants.json` (every mutant, written before testing), `outcomes.json` (every result with summary counts), `caught.txt`, `missed.txt`, `timeout.txt`, `unviable.txt`, `diff/` and `logs/`.
- Licence MIT. Current stable release on crates.io: `27.1.0`.

### 4.3 How a kill set proves a removal **[authored on §3.3 and §4.1]**

For a mutant m and a suite S, *killed(m, S)* is whether S catches m. A test t is **held** by the kept suite K = S − {t} over a mutant set M when killed(m, K) = killed(m, S) for every m in M. The mutants that matter for t are those in the regions t executes, because a mutant elsewhere is reached by no assertion of t. So the proof for one candidate is: M = the mutants cargo-mutants generates in the files of R(t) (restricted with `-f`), judged twice — by the binaries that cover those files, with and without t (`-- --test <binaries>` and the runner's skip filter) — and equal `outcomes.json` kill sets. A difference names the assertion t alone held, which is then the reason to keep t or to move its assertion into a kept test.

An opt-out an author can write (`#[mutants::skip]`, `#[coverage(off)]`) is a property the defect can supply: it hides exactly the region a weak test fails to check. The audit counts them over production source and states the count; spec-0126 sets it to zero.

## 5. How large Rust projects keep CI time bounded

### 5.1 The runner **[cited: cargo-nextest book; rust-analyzer `ci.yaml`]**

`cargo test` runs each test binary in turn, each binary runs its own tests in threads, and *"the only interface between cargo test and the binary is the exit code"*: no per-test status or time reaches the caller, a failing binary stops the later ones unless `--no-fail-fast`, and a slow test holds its binary. libtest's `--report-time` is unstable (`-Z unstable-options`). cargo-nextest lists every test, then *"executes each individual test in a separate process, in parallel"*, and reports per test: a JUnit file (`[profile.<name>.junit] path = …`) with one `<testsuite>` per binary and one `<testcase>` per test carrying `time`; `slow-timeout = { period, terminate-after, grace-period }` marks and then terminates a slow test, with `[[profile.<name>.overrides]] filter = 'test(…)'` per-test exceptions and `global-timeout` for the whole run; `--partition slice:m/n | hash:m/n` splits one run across jobs per test (counted partitioning is deprecated). Licence Apache-2.0 OR MIT; current stable release `0.9.150`. rust-analyzer's CI runs `cargo nextest run --no-fail-fast --hide-progress-bar` on three OSes with `CARGO_INCREMENTAL: 0`; cargo's own CI runs `cargo test` on five OSes with `CARGO_INCREMENTAL: 0` and no `timeout-minutes` on any job.

**Authored on the above.** Process-per-test changes one thing a suite may depend on: in-process state shared between tests of one binary (`OnceLock`, `LazyLock`, a `static Mutex`) is built once per *process*, so under nextest it is built once per *test*. 53 files in this tree hold such state (§1). A fixture that is expensive must therefore live on disk, keyed by the hash of its inputs, if it is to be shared — which is also the only shape that lets two binaries share it.

### 5.2 One binary, not one per file **[cited: matklad, *Delete Cargo Integration Tests*]**

Each file under `tests/` *"is compiled into a separate binary"*, linking the library again; cargo runs the binaries one after another, so *"the critical path is the sum of longest tests in each binary"*. Cargo's own consolidation into one `testsuite` crate: *"The time to compile the test suite decreased 3x"*, *"The size of on-disk artifacts decreased 5x"*. The recommendation is one integration crate (`tests/it/main.rs`) with a module per topic. The repository's 283 `delvec` test files are 283 link steps today.

### 5.3 PR gate versus full suite **[cited: rustc-dev-guide, *Testing with CI*; GitHub Actions limits]**

rustc's PR CI is *"relatively short (~40 minutes)"* because *"it would take too many resources to run the full test suite for each commit on every PR"*; the full suite — *"tens of CI jobs"*, several hours — runs once per merge on the bors queue. GitHub caps a job at *"6 hours of execution time"*, a matrix at 256 jobs, and a free plan at 20 concurrent jobs.

**Authored on the above.** This project's rule is stronger than rustc's (every CI job is a required check on every PR; a scheduled workflow is never a gate), so the whole suite is the PR gate and must fit a PR's patience. The one lever rustc and cargo both pull — a wall-clock bound on the job — is one this repository's `rust` job does not have.

## 6. Trial: the instruments on this tree **[authored, measured]**

Run on the remote build machine (16 cores) against `main` at `7d4a1a80d`, toolchain `1.97.1`, `cargo-llvm-cov 0.9.1` installed with `cargo install --locked`, `RUSTC_WRAPPER=` (empty, so sccache does not chain) and `CARGO_INCREMENTAL=0`, over the `delvewright-dsl` crate only. The commands are the ones §2.3 and §2.4 name; the readings are in the body of the commit that lands this page.

| Reading | Value |
|---|---|
| `cargo llvm-cov -p delvewright-dsl --remap-path-prefix --json`, run twice | 37 s and 34 s wall, instrumented build included |
| Files in the export | 85, all under `crates/dsl` (the `tests` directories excluded by the tool's default) |
| Region coverage | 29,968 of 37,122 regions (80.73 %) |
| Line coverage | 20,893 of 26,701 lines (78.25 %) |
| Function coverage | 1,940 of 2,446 functions (79.31 %) |
| Branch, MC/DC | 0 of 0 — not measured on stable, as §2.2 says |
| Determinism of the export | the two runs' JSON files hash differently (`5bbc0cbe…`, `8570ec4a…`); their covered-region sets — every `(path, line, column, covered)` entry — are identical: 36,520 entries, symmetric difference 0 |
| Per-test attribution (§2.4 construction 2), `roundtrip` binary | 2 tests; sets of 461 and 502 executed region entries of the binary's 31,218; neither a subset of the other |
| Per-test attribution, the `delvewright_dsl` unit-test binary | 207 tests, run serially one process each in 1 s; `llvm-profdata merge` plus `llvm-cov export` for all 207 in 30 s; 721 MB of profiles; union of executed entries 12,936; set sizes min 9, median 110, max 704; 0 of 207 tests subsumed by another |
| Instrumented target directory | 1.2 GB |

What the trial settles, each a rule the spec takes:

- **The hash of a coverage measurement is over the covered-region set, not the export's bytes.** The raw export carries execution counts that vary with scheduling; the set of regions executed at least once did not vary across two runs of 830 tests. **[authored, measured]**
- **The export is cheap; the per-test export is the cost.** About 0.15 s per test for merge plus export, so the whole suite's map is of the order of a quarter of an hour of export on top of one serial run of the tests, with profiles of a few MB per test that are deleted once exported. **[authored, measured]**
- **The `#[cfg(test)]` filter is a precondition of subsumption, not a refinement.** The unit-test trial's sets include each test's own body, because inline test modules live in `src/` and the tool's default exclusion sees only `tests` directories; every test therefore executes regions no other test does, and the zero subsumed of 207 is a property of the unfiltered measurement, not of the suite. Subsumption is judged over production regions only (spec-0126 §1.2), and the filter is held equal to `source_scan`'s rule. **[authored, measured]**
- **The instrument runs on the pinned stable toolchain with the `llvm-tools` component already installed on the remote**; nothing nightly was needed for region, line and function coverage. **[authored, measured]**

## 7. Sources

- rustc book, *Instrumentation-based code coverage*: https://doc.rust-lang.org/rustc/instrument-coverage.html
- rustc book, *Tests* (libtest flags: `--list`, `--exact`, `--test-threads`, `--report-time` unstable): https://doc.rust-lang.org/rustc/tests/index.html
- taiki-e/cargo-llvm-cov, README: https://github.com/taiki-e/cargo-llvm-cov
- cargo-nextest book — *How it works*, *Partitioning*, *Slow tests and timeouts*, *JUnit support*, *Test coverage*: https://nexte.st/
- sourcefrog/cargo-mutants, README and book (*Continuous integration*, *Incremental tests of pull requests*, *Sharding*, *Timeouts*, *The mutants.out directory*): https://github.com/sourcefrog/cargo-mutants, https://mutants.rs/
- rustc-dev-guide, *Testing with CI*: https://rustc-dev-guide.rust-lang.org/tests/ci.html
- rust-analyzer, `.github/workflows/ci.yaml`; cargo, `.github/workflows/main.yml` (read at their default branches)
- GitHub Docs, *Usage limits for GitHub Actions*: https://docs.github.com/en/actions/reference/limits
- A. Matklad, *Delete Cargo Integration Tests*: https://matklad.github.io/2021/02/27/delete-cargo-integration-tests.html
- S. Yoo, M. Harman, *Regression testing minimization, selection and prioritization: a survey*, STVR 22(2), 2012.
- M. J. Harrold, R. Gupta, M. L. Soffa, *A methodology for controlling the size of a test suite*, ACM TOSEM 2(3), 1993.
- G. Rothermel, M. J. Harrold, J. von Ronne, C. Hong, *Empirical studies of test-suite reduction*, STVR 12(4), 2002.
- L. Inozemtseva, R. Holmes, *Coverage is not strongly correlated with test suite effectiveness*, ICSE 2014.
- O. Vera-Pérez, B. Danglot, M. Monperrus, B. Baudry, *A comprehensive study of pseudo-tested methods*, EMSE 24(3), 2019.
- M. Gligoric, L. Eloussi, D. Marinov, *Practical regression test selection with dynamic file dependencies*, ISSTA 2015.
- A. Memon et al., *Taming Google-scale continuous testing*, ICSE-SEIP 2017.

## 8. Not established

- The exact fault-detection loss Rothermel et al. report per subject: the article's full text was not retrieved; the finding is cited at the strength of its abstract and of the studies that cite it.
- Whether cargo-nextest's experimental wrapper scripts can set a per-test `LLVM_PROFILE_FILE` (§2.4 construction 1): not tried; construction 2 is the one measured.
- Whether the instrumented run's overhead (5,294 s instrumented against 1,581 s uninstrumented on one tree, §1's re-read) is the same on the Actions runner: measured on the build machine only.
