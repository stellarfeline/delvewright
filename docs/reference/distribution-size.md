# What shipping `/new-delve` actually costs — a measured inventory

Live record of the **size** of every artifact a creator must obtain to run the
`/new-delve` skill (ADR-0012), and of what makes `delvec` the size it is.
Companion to [`tools.md`](tools.md) (*what* the tools are) and ADR-0023 (*how*
they are distributed and what is a declared prerequisite).

Every number below carries the command that produced it, so a later session
**re-measures instead of re-litigating**. Unless stated otherwise, measurements
are from this tree on macOS 26.6 / `aarch64-apple-darwin` / rustc 1.97.1 (the
pin in `rust-toolchain.toml`), with the engine at `versions.toml
[engine].version` 1.1.0.

---

## 1. The answer in one table

| | download | on disk |
|---|---|---|
| **the authoring loop** (skill + `delvec` + prefabs) | **~9.4 MB** | **~25 MB** |
| \+ a Rust toolchain, if installing via `cargo install` or building from source | ~0.4 GB | **~2.1 GB** (with the five shelf targets) |
| \+ Python venv, only if the campaign declares custom skins | — | ~94 MB |
| **the validation ladder** (docker images, jars, renderer) | ~0.4 GB | ~2.0 GB |

**`delvec` itself is ~9 MB to download and ~24 MB installed**, and it is the
whole creator: compiler, prefab authoring and admission, schematic conversion,
playtest harvesting, and both render arms — the CPU surface and the GPU stack
(Nucleation, wgpu). Nothing the authoring loop needs is large. The two big
numbers belong to things that are either *a way of obtaining* the binary (a
Rust toolchain) or *a different tier* (validation), and both are avoidable —
see §2.

---

## 2. What the skill's authoring loop genuinely needs

Established by reading the `/new-delve` page (the loop, §"The loop", and
§"Authoring tools") against [`tools.md`](tools.md), not from assumption. The
page lives in the campaigns repository, at
[`.claude/skills/new-delve/SKILL.md`](https://github.com/stellarfeline/delvewright-campaigns/blob/main/.claude/skills/new-delve/SKILL.md).

### Required

| item | download | on disk | how it is obtained |
|---|---|---|---|
| `SKILL.md` | 88,834 B | same | the skill itself |
| `delvec` binary | 9,256,047 B | 24,489,840 B | release shelf (default), `cargo install`, or a checkout (ADR-0023 §1–§2) |
| `LICENSE` (travels in every archive) | — | 35,149 B | inside the archive |
| prefab library (`campaigns/prefabs`) | 95,355 B | 479,232 B, 74 files | the content repo |

```
bash tools/build-release-binaries.sh --target aarch64-apple-darwin
ls -l dist/delvec-v1.1.0-aarch64-apple-darwin.tar.gz        → 9,256,047 B
ls -l target/aarch64-apple-darwin/release/delvec             → 24,489,840 B
wc -c < target/aarch64-apple-darwin/release/delvec           → 24,489,840 (second reader, same answer)
```

The **harvested game-registry data is not a separate download**: 684,698 B of
`crates/{compiler,dsl}/data/*.json` is `include_str!`-ed into the binary and is
already inside the numbers above (§4).

### Required, but only as a *way of getting* `delvec`

**A Rust toolchain**, if and only if the creator installs by `cargo install
delvec` or builds from a checkout. ADR-0023 §1 makes the release archive the
default acquisition and cargo not a prerequisite; the source build is the floor
that guarantees every capability on every machine (§2 there). This is the 2.1 GB
(§3) and it is the entire "why is it so big" question.

### Required conditionally

- **Python 3 + a `delve_skin` venv** — only when a campaign declares custom NPC
  skins (ADR-0018 §3; a missing skin is `DW0309`, deliberately not a silent
  skip). 94 MB.
- **`tools/i18n-translate.py`** — only for a campaign declaring non-English
  languages. Python stdlib only.
- **The 1.21.11 client jar** (31,152,600 B, `ls -l ~/.chunky/resources/minecraft.jar`)
  — for every render arm, CPU or GPU: the textures come from the creator's own
  jar, which is never downloaded, bundled or redistributed (ADR-0023 §9).
- **A GPU adapter** — for `delvec render` only. The three GPU arms are inside
  the binary on every platform; what they need at run time is an adapter (on
  Linux, a Vulkan loader the binary opens when asked). `delvec snapshot`,
  `blocking-chart`, `viewer` and the rest of the CPU surface need none.

### NOT needed by the authoring loop

- **Docker, a JDK, a Minecraft server jar, Chunky.** All validation-tier (§5).

---

## 3. The toolchain: what the ~2 GB actually is

A default-profile stable toolchain is ~1.4 GB, 62% of it `rust-docs`;
`rustup toolchain install --profile minimal` drops that. The pinned `1.97.1`
toolchain on this machine, with the five shelf targets' `rust-std` installed
beside the host's:

```
du -sk ~/.rustup/toolchains/1.97.1-aarch64-apple-darwin
→ 2144496 KB = 2.04 GiB

du -sk ~/.rustup/toolchains/1.97.1-aarch64-apple-darwin/lib/rustlib/*/
→ aarch64-apple-darwin      279,180 KB   (host)
  aarch64-unknown-linux-gnu 166,652 KB
  x86_64-unknown-linux-gnu  165,132 KB
  x86_64-apple-darwin       142,684 KB
  x86_64-pc-windows-msvc    114,040 KB
  src                        78,328 KB
```

A creator who builds from source needs only the host's `rust-std`; the four
other targets are what `tools/build-release-binaries.sh --check-only` adds to
cross-check the shelf.

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

### `cargo install`'s own disk cost

A fresh `CARGO_HOME` after installing the published crate held 35,212 KB of
registry index plus crate sources when the binary resolved 50 packages; it now
resolves 202 (§4) and that figure is **not re-measured** in this round — it is
a one-time, shared-across-all-Rust-work cost either way, not a per-delve one.

---

## 4. What makes `delvec` ~24 MB

```
cargo tree -p delvec --edges normal --prefix none | sed 's/ (\*)$//' | sort -u | wc -l
→ 202 packages
```

### 4.1 Section map of the shelf binary

```
size -m target/aarch64-apple-darwin/release/delvec      # stripped, 24,489,840 B
```

| section | bytes | note |
|---|---|---|
| `__TEXT.__text` | 16,578,844 | code |
| `__TEXT.__const` | 2,687,688 | mostly the embedded JSON (§4.3) and the render stack's tables |
| `__TEXT.__eh_frame` | 1,855,192 | unwind tables |
| `__DATA_CONST.__const` | 833,816 | |
| `__TEXT.__gcc_except_tab` | 821,208 | landing pads |
| `__TEXT.__cstring` | 397,026 | |
| `__TEXT.__unwind_info` | 305,256 | |

Panic/unwind machinery (`__eh_frame` + `__gcc_except_tab` + `__unwind_info`) is
2,981,656 B — 12% of the binary.

### 4.2 The GPU render stack is the growth, and it is deliberate

The same script on the same host produced an 8,053,424 B binary (3,251,645 B
archived) when the shelf carried the compiler and the CPU render surface alone
and the GPU arms were a second binary built from a checkout. Carrying the whole
creator in one binary (ADR-0023 §3) costs **+16,436,416 B on disk and
+6,004,402 B to download**: Nucleation's mesher and block tables, wgpu and its
per-platform backends (Metal here; Vulkan and GL on Linux; DX12 on Windows),
naga, and the image and hashing crates they bring. That is a **record, not a
budget**: binary size under 100 MB is not a decision input (ADR-0023 §1), and
the alternative it replaces was a second binary a creator had to find, build
and keep in step.

### 4.3 Embedded harvested registry data: 684,698 B

`crates/compiler/data/*.json` + `crates/dsl/data/*.json`, pulled in by 21
`include_str!`/`include_bytes!` sites:

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

Embedding it is correct — it is what makes `delvec` a single self-contained
download with no data directory to lose (ADR-0006 reproducibility).

### 4.4 The two install paths hand out different bytes

`-C strip=symbols` lives at the shelf **call site**
(`tools/build-release-binaries.sh`), not in `[profile.release]`, so the archive
is stripped and a `cargo install` / `cargo build --release` binary is not:

```
cargo build --release -p delvec
ls -l target/release/delvec                                  → 30,945,760 B
ls -l target/aarch64-apple-darwin/release/delvec             → 24,489,840 B (shelf recipe)
```

Both report `delvec 1.1.0, dsl 0.19.0, mc 1.21.11`. A repo-wide `strip` would
take symbols off every developer's `cargo build --release`, which is why the
flag sits where it does; the 26% gap is recorded, not hidden.

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
| Chunky + core, plus a JVM | beauty tier | ~55 MB | `du -sh ~/.chunky`; pins in `versions.toml [render]` |

---

## Re-measuring

```bash
# the shelf, as a user fetches it
gh release download v<version> -R stellarfeline/delvewright -p '*.tar.gz' -p SHA256SUMS
shasum -a 256 -c SHA256SUMS
tar -xzf delvec-v<version>-<target>.tar.gz && ls -l delvec

# one target, locally, exactly as CI builds it
bash tools/build-release-binaries.sh --target <triple>
ls -l dist/delvec-v<version>-<triple>.tar.gz target/<triple>/release/delvec

# the unstripped path
cargo build --release -p delvec && ls -l target/release/delvec

# what is in the binary
cargo tree -p delvec --edges normal --prefix none | sed 's/ (\*)$//' | sort -u | wc -l
size -m target/<triple>/release/delvec

# the toolchain
du -sk ~/.rustup/toolchains/<channel>-<host>
du -sk ~/.rustup/toolchains/<channel>-<host>/lib/rustlib/*/
```
