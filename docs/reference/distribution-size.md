# What shipping `/new-delve` actually costs — a measured inventory

Live record of the **size** of every artifact a creator must obtain to run the
`/new-delve` skill (ADR-0012), and of what makes `delvec` the size it is.
Companion to [`tools.md`](tools.md) (*what* the tools are) and ADR-0017 /
ADR-0018 (*how* they are distributed and what is a declared prerequisite).

Every number below carries the command that produced it, so a later session
**re-measures instead of re-litigating**. Unless stated otherwise, measurements
are from `stellarfeline/delvewright` release **v1.1.0** and from repo `main` at
`374bbfb`, on macOS 26.6 / `aarch64-apple-darwin` / rustc 1.97.1 (the pin in
`rust-toolchain.toml`).

---

## 1. The answer in one table

| | download | on disk |
|---|---|---|
| **the authoring loop** (skill + compiler + prefabs) | **~3.9 MB** | **~10.7 MB** |
| \+ a Rust toolchain, if installing via `cargo install` | ~0.4 GB | **~1.4 GB** |
| \+ Python venv, only if the campaign declares custom skins | — | ~94 MB |
| **the validation ladder** (docker images, jars, renderer) | ~0.4 GB | ~2.0 GB |

**`delvec` itself is ~4 MB to download and ~11 MB installed.** Nothing the
authoring loop needs is large. The two big numbers belong to things that are
either *a way of obtaining* the compiler (a Rust toolchain) or *a different
tier* (validation), and both are avoidable — see §2.

---

## 2. What the skill's authoring loop genuinely needs

Established by reading `.claude/skills/new-delve/SKILL.md` (the loop, §"The
loop", and §"Authoring tools") against [`tools.md`](tools.md), not from
assumption.

### Required

| item | download | on disk | how it is obtained |
|---|---|---|---|
| `SKILL.md` | 88,834 B | same | the skill itself |
| `delvec` binary | 2,990,764–3,421,465 B | 7,923,608–11,300,864 B | release shelf, `cargo install`, or a checkout (ADR-0017) |
| `LICENSE` (travels in every archive) | — | 35,149 B | inside the archive |
| prefab library (`campaigns/prefabs`) | 95,355 B | 479,232 B, 74 files | the content repo |

The **harvested game-registry data is not a separate download**: 684,698 B of
`crates/{compiler,dsl}/data/*.json` is `include_str!`-ed into the binary and is
already inside the numbers above (§4).

### Required, but only as a *way of getting* `delvec`

**A Rust toolchain**, if and only if the creator installs by `cargo install
delvec` or builds from a checkout. ADR-0018 §2 makes cargo a declared creator
prerequisite; ADR-0017's five-target release shelf is the path that does **not**
need one. This is the 1.4 GB (§3) and it is the entire "why is it so big"
question.

### Required conditionally

- **Python 3 + a `delve_skin` venv** — only when a campaign declares custom NPC
  skins (ADR-0018 §3; a missing skin is `DW0309`, deliberately not a silent
  skip). 94 MB.
- **`tools/i18n-translate.py`** — only for a campaign declaring non-English
  languages. Python stdlib only.

### NOT needed by the authoring loop

- **A GPU, a Minecraft client jar, or `delve-render`.** The loop's visual channel
  is `delvec snapshot` / `delvec blocking-chart`, a CPU voxel raycaster inside
  `delvec` — its own doc comment states it runs "in one process with no GPU, no
  resource pack and no server" (`crates/compiler/src/snapshot.rs`). `delve-render`
  is deliberately **not on the release shelf** (ADR-0017 §3) precisely because it
  needs a GPU stack and an EULA-gated client jar.
- **Docker, a JDK, a Minecraft server jar, Chunky.** All validation-tier (§5).

---

## 3. The 1.5 GB figure — what it actually measured

**It is the Rust toolchain, and the recollection is accurate.**

Provenance: a planning session on **2026-08-07T16:39:15Z** argued the costs of
making creator-facing tooling all-Rust with cargo as a prerequisite — the
discussion that became **ADR-0018** (dated 2026-08-07). Its third listed cost was
*"toolchain size (~1.5 GB) and the first build's several minutes"*. It was an
unverified figure quoted in chat; no repo artifact carries it (`grep -rniE
'1\.5 ?gb|1500 ?mb' .` over the tree returns nothing relevant).

Verified on this machine — a default-profile, host-only stable toolchain:

```
du -sk ~/.rustup/toolchains/stable-aarch64-apple-darwin
→ 1477376 KB = 1.41 GiB
```

Breakdown (`du -sk ~/.rustup/toolchains/stable-aarch64-apple-darwin/{share,lib,bin,libexec}`):

| component | KB | what it is |
|---|---|---|
| `share/` | 917,036 | `rust-docs` — the offline documentation, 62% of the toolchain |
| `lib/` | 494,148 | rustc's own libs + `rust-std` for the host |
| `bin/` | 64,688 | rustc, cargo, rustdoc, clippy, rustfmt |
| `libexec/` | 1,464 | |

The pinned `1.97.1` toolchain on this machine is **2,108,396 KB = 2.01 GiB**,
because the five shelf targets' `rust-std` are installed alongside it
(`du -sh ~/.rustup/toolchains/1.97.1-aarch64-apple-darwin/lib/rustlib/*/`:
aarch64-apple-darwin 273M, x86_64-unknown-linux-musl 220M,
aarch64-unknown-linux-musl 146M, x86_64-apple-darwin 139M,
x86_64-pc-windows-msvc 111M).

`rustup toolchain install --profile minimal` drops `rust-docs`, which is where
~0.9 GB of the 1.4 GB lives.

### It is NOT the OCI image, and here is the ruling-out

A delve's OCI image (ADR-0010) is a different artifact and does not match the
figure either way:

```
docker images --format '{{.Repository}}:{{.Tag}}\t{{.Size}}'
→ delvewright/delve:local  865MB      (a built delve, uncompressed, local)
→ dw-*-bot:latest          1.04GB     (the mineflayer harness image)

docker manifest inspect ghcr.io/stellarfeline/delvewright-toolserver@sha256:bce98718…
→ 28 layers, 341,537,417 B compressed (amd64 pull)
```

A creator never obtains any of these to *author*; a player obtains only the delve
image, and that is the product, not the toolchain.

### `cargo install`'s own disk cost is small

A fresh `CARGO_HOME` after installing the published crate:

```
CARGO_HOME=<fresh> cargo install delvec --version 1.1.0 --root <fresh>
du -sk <fresh CARGO_HOME>   → 35212 KB   (registry index + 50 crates' sources)
```

So the "cargo path" is ~1.4 GB of toolchain + ~34 MB of registry cache, and the
1.4 GB is a one-time, shared-across-all-Rust-work cost — not a per-delve one.

---

## 4. Why `delvec` is ~11 MB and not ~1 MB

The premise "`delvec` shouldn't have many dependencies" is **correct** — and the
dependencies are not the cause.

```
cargo tree -p delvec --edges normal --prefix none | sed 's/ (\*)$//' | sort -u | wc -l
→ 50 packages
```

### 4.0 The CPU render surface is ~1.6 MB of it

`delvec` carries the whole CPU render surface — `viewer`, `palette`, `scene`,
`panorama`, `contact-sheet`, `index` — so a creator installs one binary and
nothing render-shaped is a second download. Measured on one host with one script,
which is the only comparison worth making (the figures below move by a megabyte
between toolchains and hosts, so a number from a different machine is not a
baseline):

```
bash tools/build-release-binaries.sh --target aarch64-apple-darwin
```

| | download | on disk |
|---|---|---|
| without the render surface | 3,383,559 B | 9,109,152 B |
| with it | 3,914,334 B | 10,718,048 B |
| difference | +530,775 B (+15.7%) | +1,608,896 B (+17.7%) |

That is a **record, not a budget**: binary size under 100 MB is not a concern,
and the alternative it replaces was a second binary a creator had to find, build
and keep in step. What the surface does NOT drag is a GPU stack — no
`nucleation`, no `wgpu`, and the two new external dependencies (`image` with
default features off, and `zip` with `deflate` only) are pure Rust, which is what
keeps this binary cross-building for all five shelf targets including the two
static-musl ones.

### 4.1 80% of the code is ours

```
cargo bloat --release -p delvec --bin delvec --crates -n 30
```

| crate | .text | share of .text |
|---|---|---|
| `delvewright_dsl` | 2.4 MiB | 40.7% |
| `delvewright_compiler` | 2.3 MiB | 40.2% |
| `std` | 454.1 KiB | 7.6% |
| `clap_builder` | 265.0 KiB | 4.5% |
| `serde_json` | 124.5 KiB | 2.1% |
| everything else (25 crates) | < 200 KiB total | ~2% |

`.text` is 5.8 MiB of a 10.5 MiB unstripped file. Every third-party crate
combined is about 1.1 MiB.

### 4.2 It is a long tail of derive-generated code, not a few fat functions

`cargo bloat -n 25` shows the largest single function at 172 KiB
(`emit::emit_functions`); the **8,586 functions below the top 25 account for
5.0 MiB**. Aggregating all 8,611 symbols by what generated them:

```
cargo bloat --release -p delvec --bin delvec -n 0 --message-format json
# then bucket `functions[].size` by name pattern
```

| category | bytes | % of .text | functions |
|---|---|---|---|
| serde `Deserialize` | 1,683,072 | 27.6% | 2,094 |
| `schemars::JsonSchema` | 538,244 | 8.8% | 748 |
| serde `Serialize` | 155,696 | 2.6% | 270 |
| **derive-generated subtotal** | **2,377,012** | **39.0%** | **3,112** |
| clap | 217,000 | 3.6% | 345 |
| `fmt`/`Debug` | 95,088 | 1.6% | 458 |

The asymmetry is the tell: `crates/dsl/src` is 23,192 lines and emits 2.4 MiB of
`.text`; `crates/compiler/src` is 65,445 lines and emits 2.3 MiB. The DSL crate
produces ~3× the machine code per source line because 119 of its types derive
`JsonSchema` on top of `Serialize`/`Deserialize`
(`grep -r 'JsonSchema' crates/dsl/src --include='*.rs' | grep derive | wc -l`).
This is monomorphisation over a large authored type surface — the direct cost of
the schema-enforced staged DSL (ADR-0002) and of `delvec schema` being served
from the same types the compiler validates with.

### 4.3 Embedded harvested registry data: 684,698 B

`crates/compiler/data/*.json` + `crates/dsl/data/*.json`, pulled in by 21
`include_str!`/`include_bytes!` sites. Verified **byte-present in the shipped
v1.1.0 binary** (each file's first 120 bytes located inside the binary image):

| file | bytes |
|---|---|
| `commands-1.21.11.json` | 474,604 |
| `sounds-1.21.11.json` | 71,582 |
| `item-stack-sizes-1.21.11.json` | 52,762 |
| `items-1.21.11.json` | 46,986 |
| `item-combat-1.21.11.json` | 19,982 |
| `entity-tags-1.21.11.json` | 8,503 |
| `damage-types-1.21.11.json` | 5,472 |
| `entities-1.21.11.json` | 4,042 |
| `crates/dsl/data/*` (4 files) | 765 |
| **total** | **684,698** |

That is 8.5% of the shipped binary, and it is pretty-printed JSON (§6.3).
Embedding it is correct — it is what makes `delvec` a single self-contained
download with no data directory to lose (ADR-0006 reproducibility).

### 4.4 Section map of the shipped binary

```
size -m <extracted delvec>          # v1.1.0, aarch64-apple-darwin, 8,053,424 B
```

| section | bytes | note |
|---|---|---|
| `__text` | 5,547,692 | code (§4.1–4.2) |
| `__const` | 1,050,196 | mostly the embedded JSON (§4.3) |
| `__eh_frame` | 551,656 | unwind tables |
| `__gcc_except_tab` | 315,612 | landing pads |
| `__cstring` | 135,708 | |
| `__unwind_info` | 92,544 | |

Panic/unwind machinery is 959,812 B — 12% of the binary (§6.2).

---

## 5. Validation tiers (never part of the authoring loop)

Listed so the split is unambiguous: none of this is needed to author, validate
statically, compile, or look at a delve.

| artifact | tier | size | command / note |
|---|---|---|---|
| `delvewright-toolserver` image | PackTest + bot ladder | 341,537,417 B pull, ~865 MB on disk | `docker manifest inspect ghcr.io/…/delvewright-toolserver@sha256:bce98718…` |
| built delve image | PackTest + bot ladder | 863–865 MB | `docker images` |
| harness bot image | bot ladder | 1.04 GB | `docker images` |
| Minecraft server jar | any server boot | 56,327,581 B | `versions.toml [minecraft].server_jar_size` — fetched at run time, never baked (EULA) |
| 1.21.11 client jar (textures) | beauty tier | 31,152,600 B | `ls -l ~/.chunky/resources/minecraft.jar` — EULA, never committed |
| Chunky + core, plus a JVM | beauty tier | ~55 MB | `du -sh ~/.chunky`; pins in `versions.toml [render]` |
| `delve-render` | beauty tier | a GPU/driver stack | not on the shelf, ADR-0017 §3 |

---

## 6. Reduction options, largest saving per unit of disruption first

Each row states whether it is **MEASURED** or an **ESTIMATE**. Nothing here was
applied; the numbers exist so a decision can be made against evidence.

All builds below: `cargo build --release --locked -p delvec --bin delvec` on
`main` with `RUSTFLAGS="-C strip=symbols"` and the named profile override, into a
separate `CARGO_TARGET_DIR`. The baseline is the current shelf recipe:
Every row below is measured against ONE baseline binary on one host, so the
savings are relative and remain valid, while the absolute figures are smaller
than §4.0's because that baseline carries no render surface. Re-measure the pair
together before quoting either.

**8,756,512 B on disk / 3,251,645 B archived** (`tools/build-release-binaries.sh
--target aarch64-apple-darwin`).

### 6.1 Strip the `cargo install` path too — 1,959,504 B, near-zero disruption · MEASURED

`-C strip=symbols` lives at the shelf **call site**
(`tools/build-release-binaries.sh`), not in `[profile.release]`, so the two
supported install paths hand out different binaries **at the same version**:

| path | v1.1.0 binary |
|---|---|
| release archive | 8,053,424 B |
| `cargo install delvec --version 1.1.0` | 10,012,928 B |

Both report `delvec 1.1.0, dsl 0.9.0, mc 1.21.11`. Adding
`[profile.release] strip = "symbols"` closes a 24% gap for the crates.io path.
Cost: every developer's own `cargo build --release` also loses symbols — which is
why the flag was placed at the call site in the first place. A dedicated
`[profile.release-shelf]` inheriting from `release` gets both.
**Download impact: none** (crates.io ships source).

### 6.2 `panic = "abort"` (+ fat LTO, 1 CGU) — 2,467,040 B on disk, 659,679 B download · MEASURED

| variant | on disk | archived | build time |
|---|---|---|---|
| baseline (shelf recipe) | 8,756,512 | 3,251,645 | 30 s |
| `lto=fat`, `codegen-units=1` | 7,760,736 (−11.4%) | 3,198,680 (−1.6%) | 152 s |
| \+ `panic=abort` | 6,289,472 (−28.2%) | 2,591,966 (−20.3%) | 87 s |

Both variants run and report the right version. Note the shape: **LTO buys a lot
of disk and almost no download** (dead code compresses well); `panic=abort` buys
both, because it deletes the unwind tables of §4.4. Disruption: `panic=abort` is
a semantic change (no unwinding, no `catch_unwind`) and would have to be a
shelf-only profile so tests keep unwinding; fat LTO costs 2–5× release build
time in CI's five-target matrix.

### 6.3 `opt-level = "z"` — 1,269,072 B on disk, 512,507 B download · MEASURED, with an unmeasured cost

7,487,440 B on disk / 2,739,138 B archived, at **no** build-time cost (25 s).
But `delvec`'s own runtime is the thing `Cargo.toml` already documents as
mattering (island campaign: 771 s at opt-level 0, 25 s at 3), and the runtime
cost of `z` **was not measured here**. Do not adopt without measuring a campaign
build.

### 6.4 Minify the embedded registry JSON — ~341,050 B on disk · MEASURED on the data, ESTIMATE on the binary

`json.dumps(…, separators=(',',':'))` over all 12 embedded files:
684,698 → 343,648 B (`commands-1.21.11.json` alone: 474,604 → 157,442). The
binary saving is an **estimate** — it assumes `__const` shrinks 1:1.
**Download saving is negligible**: `zlib.compress` of the concatenated data is
41,579 B raw vs 34,327 B minified, so the archive moves by ~7 KB. The files are
generated by `tools/extract-*.py`, so this is a harvest-time flag; the cost is
that a diff of the harvested data stops being readable, which matters for the
provenance record (`crates/compiler/data/PROVENANCE.md`).

### 6.5 Deflate the embedded JSON, inflate at startup — ~650,371 B on disk · ESTIMATE

`flate2` is already a dependency. 684,698 → 34,327 B of stored bytes. Costs a
decompress on every process start and turns a `&'static str` into an owned
allocation at every one of the 8 registry sites. Download saving again ~7 KB.

### 6.6 Shrink the derive surface — up to ~0.5 MB · ESTIMATE, most disruptive

The measured `schemars` share is 538,244 B. Moving `delvec schema` into its own
binary, or hand-writing `JsonSchema` for the largest enums, would recover part of
it. This trades directly against ADR-0002's premise that the schema the LLM
generates against and the types the compiler validates with are one source — and
against the size of the whole saving, which is smaller than §6.2's. Listed for
completeness, not recommended.

---

## 7. Findings recorded while measuring

1. **`tools/build-release-binaries.sh` understated the binary by ~2×.** Its
   comment claimed `-C strip=symbols` takes the artifact "11.7 MB -> ~4 MB per
   target". Built at `922cfb6`, the exact commit that wrote that line, on this
   host and toolchain: **9,794,464 B unstripped → 7,917,536 B stripped**. It was
   not drift — no shelf artifact has ever been ~4 MB (the smallest of the five
   v1.1.0 targets is 7,923,608 B). Corrected in the same PR as this document,
   comment-only.
2. **The two install paths differ by 24%** at the same version — §6.1.
3. **The binary grew 10.6% in five days** — 7,917,536 B (`922cfb6`, 2026-08-06) →
   8,756,512 B (`main`, today), same flags, same host. Worth a periodic
   re-measure rather than a gate; nothing here is near a threshold that matters.
4. **`skinpy-extended` pulls `twine`** into the skin venv (~25 MB of
   `pygments`/`docutils`/`rich`/`keyring` that a skin composer never calls).
   Upstream's dependency declaration, not ours; noted so a future session does
   not go looking for our mistake.

---

## Re-measuring

```bash
# the shelf, as a user fetches it
gh release download v<version> -R stellarfeline/delvewright -p '*.tar.gz' -p SHA256SUMS
shasum -a 256 -c SHA256SUMS
tar -xzf delvec-v<version>-<target>.tar.gz && ls -l delvec

# one target, locally, exactly as CI builds it
bash tools/build-release-binaries.sh --target <triple>

# what is in the binary
cargo bloat --release -p delvec --bin delvec --crates -n 30
cargo bloat --release -p delvec --bin delvec -n 0 --message-format json
size -m target/<triple>/release/delvec

# the toolchain
du -sk ~/.rustup/toolchains/<channel>-<host>
```
