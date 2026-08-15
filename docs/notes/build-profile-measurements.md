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
