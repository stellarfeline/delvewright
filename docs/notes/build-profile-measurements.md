# `delvec build` wall clock — where it actually goes (2026-08-06)

Measured because `delvec build` on a real campaign was "too slow to develop
against". It was not: it was being run unoptimized. Recorded here so the numbers
survive the conclusion, and so the next person who suspects a slow compiler
starts from data.

**~25s is acceptable, optimisation is shelved; we are not optimising
prematurely.** The engine work below is therefore *not* scheduled. It is written down only so that a future
round that does need it starts from the profile rather than from a guess.

## Method

Machine: M2 Pro (Mac14,9), 10 cores, 16 GB, macOS 26.6. Campaign:
`nobodys-cave-island` (157 stage JSON files, 10 world-edit batches) from
`delvewright-campaigns@87a8ade`. Output compared with `diff -r` over all 594
emitted files.

- Wall clock: `/usr/bin/time -p`, **3 runs per cell**, median reported, spread
  given where it exceeds 1%.
- Phase attribution: `/usr/bin/sample` (macOS, 1 ms) against a release binary
  built with `CARGO_PROFILE_RELEASE_DEBUG=1`, 24 485 samples on the main thread;
  plus temporary `Instant` probes around each `run_build` stage and around
  `LightModel::flood`. **The probes were reverted** — they are not in the tree.
- Error bars: sampling attribution is ±1 sample-period per frame and undercounts
  the process's first ~1 s (the sampler attaches after launch); the `Instant`
  probes agree with it to within 3%, which is the honest precision here. A
  concurrent unrelated `cargo` job ran during the debug measurement; `delvec` is
  single-threaded (97% CPU of one core) on a 10-core box, so contention is
  within the spread.

## Profile is the whole story

| Profile | `island` build | Compile (clean) | Compile (incremental) |
|---|---|---|---|
| `dev`, `opt-level = 0` (was the default) | **771 s** (12m51) | 24.6 s | 1.9 s |
| `dev`, `opt-level = 1` (now the default) | **46 s** | 32.4 s | 1.8 s |
| `release`, `opt-level = 3` | **25.4 s** (25.2 / 25.4 / 25.8) | 44.4 s | 20.1 s |

Debug → release is **30x**. All three emit byte-identical output over 594 files,
so ADR-0006 does not constrain the choice of profile — it is measured, not
assumed.

Small campaigns never showed the problem, which is why it survived: the CI
fixtures are 0.08 s (`hello-world`) and 0.37 s (`keep-trial`) even at
`opt-level = 0`. The pitfall only bites a campaign with a real assembled world.

## The other half of the dev profile: `debug`

The profile's other key is `debug`, and cargo's default for it is full DWARF.
That default is expensive here for a reason particular to this workspace: `cargo
test --workspace` builds **216 test executables**, and complete debuginfo — every
local variable of every function — is emitted and linked once for each of them.

### Method

Workload: `cargo test --locked --workspace --no-run` from a removed `target/` —
141 crates, 407 build units, 216 test executables, identical under every
condition. Toolchain **1.97.1 (`8bab26f4f`)**, resolved from
`rust-toolchain.toml` by running every build with the repo root as CWD; no
`.cargo/config.toml` exists between the worktree and `/` or in `CARGO_HOME`, and
no `CARGO_*`/`RUST*` variable is set, so the manifest is the only thing
configuring the profile.

- **CPU (user + sys), not wall clock**, from `/usr/bin/time -p` over the process
  tree. Repeats of one condition spread 1–2% in CPU and 7–20% in wall clock on a
  machine carrying other work, so nothing below rests on wall.
- Conditions applied by **editing the manifest**, never by a `CARGO_PROFILE_*`
  environment override: cargo validates profile keys and values, so a mistake is
  an error rather than the silent no-op an environment variable would give.
- **Two runs per condition, alternated `A B C D D C B A`**, so load drift cannot
  land on one condition.
- Free space via `df` on `/System/Volumes/Data`; `du` on `target/` only to
  apportion between conditions. The two agree to 0.13% while the machine is
  otherwise quiet and diverge by up to 19% while it is not, which is the other
  writer rather than the filesystem.

### What each setting costs

| `[profile.dev]` | CPU (n=2) | vs base | `target/` | vs base |
|---|---|---|---|---|
| `debug` at cargo's default, incremental on | 862.8 s | — | 5.58 GiB | — |
| `debug = "line-tables-only"` | 714.5 s | **−17.2%** | 4.57 GiB | −18% |
| `incremental = false` | 820.5 s | −4.9% | 3.16 GiB | −43% |
| both | 685.0 s | −20.6% | 2.63 GiB | −53% |

The headline is cross-checked on a second, separate set of runs by two further
instruments whose failure mode is not the kernel's CPU accounting: cargo's own
per-unit timers put the saving at −18.4%, and `target/` — the DWARF that was
never written — at −18.3%, against −17.6% from rusage on those same runs.
`debug = 0` is measured too and **refused**: it saves a further 7.0% and pays for
it with the `file:line` on a panicking test's backtrace, which is the only thing
this debuginfo is for.

Where the saving falls is what matters for CI, because `Swatinem/rust-cache`
restores dependency artifacts and never workspace crates: workspace units drop
18.9% (570.5 → 462.6 unit-s) against 16.6% for dependency units, so **82% of the
saving is in the part every CI job recompiles from scratch**.

### The render workspace answers the same way

`crates/render` carries a `[workspace]` table of its own and therefore inherits
nothing from the root profile, so it is measured on its own terms rather than
assumed to follow: cold `cargo test --locked --offline --manifest-path
crates/render/Cargo.toml --no-run`, same instrument, alternated `A B B A`.

| `crates/render` `[profile.dev]` | CPU (n=2) | `target/` |
|---|---|---|
| `debug` at cargo's default | 731.0 s | 3.10 GiB |
| `debug = "line-tables-only"` | 597.4 s | 2.16 GiB |

That is −18.3% CPU and −30% disk, the same answer the root workspace gives, and
the two CI jobs that build this crate under a dev profile — the `rust` job's
render steps and `gpu-probe` — are where it lands.

The pair also shows why CPU is the instrument rather than wall clock: the
baseline condition measured 754.8 CPU s at 157.0 s wall while the machine was
busy and 730.2 CPU s at 93.8 s wall once it was not — 3.4% apart in CPU, 67%
apart in wall.

### What `line-tables-only` costs, measured

A deliberately panicking test under `RUST_BACKTRACE=full` prints **46 frames of
which 45 carry a `file:line`, under both settings, and those 45 source locations
are byte-identical**. Five frames read differently, all of them this workspace's
own, and only in qualification — the bare function name where full DWARF gives
the crate-qualified one. The `at <file>:<line>:<col>` line under each frame is
unchanged, and so is the panic's own location line.

So what is actually given up is inspecting local variables under a debugger, and
nothing here does that: `lldb`, `gdb`, `dwarfdump`, `addr2line` and `dsymutil`
appear in no script or job, and `std::backtrace`, `#[track_caller]` and
`RUST_BACKTRACE` appear zero times across all seven crates' `src/` and `tests/`.
A debugging session that wants them gets them back with
`CARGO_PROFILE_DEV_DEBUG=full cargo build`, which overrides the manifest.

The saving is on the FULL build and is neutral on the loop: a rebuild after one
edit costs the same either way (60.9 s at full debuginfo, 60.8 s at
line-tables-only), because what it removes is codegen and linking across all 216
executables rather than anything on the incremental path.

### Why `incremental` is not set

A cold build is not the population. `incremental = false` cuts the cold build and
grows every rebuild after an edit, and the second effect is larger than the first
by more than an order of magnitude — one function added to `crates/compiler`,
then `cargo test --workspace --no-run` again:

| | cold (n=2) | rebuild after one edit (n=4) |
|---|---|---|
| `debug = "line-tables-only"` | 715.1 s | **60.8 s** |
| the same, `incremental = false` | 676.6 s | **265.7 s** |

The cold build saves 38.5 s; each rebuild costs 204.9 s more, so the setting
breaks even only on a tree rebuilt **fewer than 0.2 times**. The same comparison
at full debuginfo agrees: 47.3 s saved cold against roughly 266 s added per
rebuild.

It would also reach nothing where the cold builds actually are. Every job in this
repository that compiles under a dev profile — 12 of the 27 jobs across the five
workflows, none excepted — runs `Swatinem/rust-cache` before its first build, and
that action exports `CARGO_INCREMENTAL=0` unconditionally; the environment
variable overrides the profile key in both directions, which is measured rather
than quoted. So the key would take effect only on a developer's or a worker's own
machine, which is exactly the population where it loses.

## Phase breakdown (release, island, median of 3)

| Phase | Time | Share |
|---|---|---|
| validate (load + parse + stage validation) | 0.015 s | 0.1% |
| analyze (quest-graph reachability) | 0.001 s | <0.1% |
| `Plan::build` | 0.001 s | <0.1% |
| prefab `.nbt` / structure loading | 0.001 s | <0.1% |
| skin loading | 0.001 s | <0.1% |
| **`emit::build_with_warnings`** | **25.34 s** | **99.6%** |
| datapack / world write to disk | 0.047 s | 0.2% |
| TOTAL | 25.44 s | |

Everything outside emission is noise. Inside emission (sampled):

| Inside `emit` | Share of build |
|---|---|
| `edit::replay_with` → `light::relight_over` (10 world-edit batches) | 88.4% |
| `light::relight_over` (the final, post-replay relight) | 9.9% |
| `nav::pacing_lints` | 0.8% |
| everything else (nav, command validation, all emitters) | 0.9% |

`LightModel::flood` accounts for **23.7 s of the 25.4 s** — 22 calls, 1.08 s
each. Half of that (11.8 s) is the seeding scan alone.

## Why light dominates, if it is ever worth fixing

Three multiplying factors, all in `crates/compiler/src/light.rs`:

1. **`relight_area` re-floods the entire world AABB once per fixture placed**
   (the greedy loop: flood, find darkest deficient cell, place one fixture,
   repeat).
2. **`relight_over` runs once per world-edit batch**, because `edit::replay`
   re-proves every invariant after each batch. The island has 10 batches, so the
   whole light field is recomputed 11 times. This is a correctness property, not
   an accident — a batch may be what makes a room dark.
3. **`LightModel::sky_open` is O(height) per cell**, called once per AABB cell
   during seeding, making seeding O(volume x height). That is the 11.8 s. It is
   also the one factor that is pure waste: a single top-down sweep per (x, z)
   column computes the same answer for the whole column in O(volume).

Ranked by win-per-risk, if a future round needs it:

| Fix | Est. win | Determinism risk |
|---|---|---|
| Column-sweep `sky_open` (one top-down pass, memoized per (x,z)) | ~45% of build | none — by construction, same values in the same order |
| Incremental reflood: reseed only cells within light radius of a batch's writes | most of the rest | by construction if the dirty set is a superset of what changed; needs a proof, not a test |
| Dense `Vec` + interned block ids instead of `BTreeMap<[i32;3], String>` | constant factor | none — iteration order is explicit (y, z, x), not map order |
| Threading the per-area loop | ≤ the area count | by test only; areas share `&mut model`, so this is the *last* resort, not the first |

The first row is the one to do if the subject reopens: it is a local change to
one function, it is byte-identical by construction, and it is nearly half the
build.
