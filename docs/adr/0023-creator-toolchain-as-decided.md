# ADR-0023: The creator toolchain as decided — one `delvec` is the delve creator, archive-first acquisition, a source-build floor, lazy-loaded externals

- **Status**: Proposed
- **Date**: 2026-09-05
- **Source**: the settled acquisition rules for the creator toolchain, until now
  recorded only in ADR-0021's source note and in `CLAUDE.md` while ADR-0017/0018
  continued to state the shape they replaced; and the ruling that the engine
  ships **one binary** carrying every creator-facing capability, GPU rendering
  and prefab authoring included (`CLAUDE.md`, Architecture → One binary). This
  ADR decides what the toolchain *is made of* as one artifact and how a creator
  *obtains* it.
- **Refines**: ADR-0012 (product form), ADR-0014 (creator distribution),
  ADR-0016 (three-layer versioning: the engine line now names seven crates)
- **Supersedes**: ADR-0021 (its §1 stands and is now trivially true; its §2 is
  implemented here on a current registry release; its §3 and the musl-static
  rationale are superseded by §3 and §4 below; its §4–§6 stand as the record of
  the viewer core, the client-jar mechanism and the non-Rust criterion)
- **Supersedes, in part**: ADR-0017 (its consequence naming `cargo install
  delvec` the creator-facing install, and its release shelf's Linux targets) and
  ADR-0018 §2–§3 (their up-front prerequisite posture — the language and porting
  decisions stand)

## Context

ADR-0017 and ADR-0018 fixed the creator's entry point as `cargo install delvec`
with cargo and Python as declared prerequisites, "both installed once, neither
discovered". Those premises were re-examined and replaced: a creator obtains
prebuilt archives and need not depend on cargo at all; tools that cannot be
integrated are installed at the step that needs them, not up front. The
replacement rules reached `CLAUDE.md` (the source-build floor, the `Init`
section, acquire-at-the-step) and ADR-0021's source note, but no Accepted ADR —
so the record's authoritative layer kept asserting the superseded shape, and
documents written against it inherited that shape.

Separately, the engine repository built **six binaries** — `delvec`,
`delve-grammar`, `delve-admit`, `delve-schem`, `delve-harvest` and, from a
workspace of its own, `delve-render` — of which one was distributed. ADR-0021
§3 kept the prefab-authoring tools and the GPU render arms off the shelf on
three grounds, every one a property of **static musl** Linux targets: a
`crt-static` binary cannot `dlopen` a Vulkan loader; Nucleation's `blake3`
build script found no musl cross-compiler; and the self-contained musl sysroot
resolves no `-ldl`. None of the three is a property of the code, the creator or
the capability. A creator who needed those tools had to know which of six names
to build, from which workspace, with which manifest path — and the skill had to
carry that map.

## Decision

### 1. The default acquisition is the per-platform release archive

GitHub Releases carries `delvec` as **one archive per platform target** — never
loose files, and an archive is never re-split to save the creator disk space —
plus a `SHA256SUMS` beside them. **cargo is not a creator prerequisite.**
crates.io publication continues (ADR-0017's machinery is kept and CI remains the
only publisher), so `cargo install delvec` remains a supported path; it is no
longer the entry point. Distribution size under 100 MB and build time are not
decision inputs.

### 2. The source build is the floor, and the floor is the guarantee

Every validation the pipeline needs must be runnable on the creator's own
machine, and this is not negotiable (`CLAUDE.md`, Methodology). At worst,
cloning the repository and building from source delivers the complete
toolchain. Where a platform's binary distribution fails, the skill states how
to build locally and the run builds from source — never a diminished tool, and
a distribution question never decides a capability question. Every skill owns
an explicit **`Init` section** that establishes a complete toolchain before
any work begins. cargo is required on exactly this path, which is what makes
§1's "not a prerequisite" honest rather than a hidden dependency.

### 3. The engine is one binary, `delvec`, and it is the delve creator

`cargo build --release --workspace` produces exactly one `[[bin]]`: `delvec`.
Everything a creator runs is a subcommand of it: compiling and analysing, the
CPU render arms, the GPU render arms, prefab authoring, admission, schematic
conversion and playtest harvesting. **No cargo feature gates any subcommand**
(ADR-0021 §1's rule, kept): the surface is unconditional code, and
`tools/build-release-binaries.sh` asserts per target that the built binary's
`--help` lists exactly the clap surface parsed from source.

A subcommand is named after the **object it acts on**, never after the crate the
code came from. The surface the six binaries carried mounts as:

| was | is | the object |
|---|---|---|
| `delve-grammar <verb>` | `delvec grammar list\|show\|check\|expand\|coverage\|audit` | a grammar program |
| `delve-admit <verb>` | `delvec prefab audit\|socket\|resolve-jigsaw\|anchor\|lighting\|catalog\|gallery\|curate\|curate-merge` | a prefab piece under admission |
| `delve-schem convert` | `delvec schem convert` | an outside schematic |
| `delve-harvest` | `delvec harvest <log> <manifest>` | a playtest log |
| `delve-render piece\|batch\|fidelity-gate` | `delvec render piece\|batch\|fidelity-gate` | a rendered picture of a piece, a library, the fixture |

The CPU render arms (`viewer`, `scene`, `panorama`, `contact-sheet`, `palette`,
`index`) stay flat at the top level, where ADR-0021 §1 put them and where the
skill already names them. `--json` is `delvec`'s one global diagnostics flag
and every mounted surface answers to it; a mounted group declares no second one.

Splitting the distribution into a second package is **rejected**: it spends a
user decision to buy bytes nobody is short of, and it re-creates the map of
names the creator had to carry. The library crates keep their boundaries
(`crates/compiler`, `crates/grammar`, `crates/admit`, `crates/schem`,
`crates/orchestrator`, `crates/render`); only the executable is one. The
`delvec` package is the executable and nothing else (`crates/delvec`); the
compiler library it was fused with is the crate `delvewright-compiler`, whose
library target keeps its name.

### 4. Linux release targets are gnu, and the GPU arms ship on the shelf

`versions.toml [engine].targets` names `x86_64-unknown-linux-gnu`,
`aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin` and
`x86_64-pc-windows-msvc`. A glibc floor is accepted; it is the glibc of the
runner that builds the target, and the release workflow prints the floor each
Linux artifact actually requires rather than promising one. Each Linux target
builds natively on a runner of its own architecture, so the recipe carries no
cross-linker and no cross C compiler.

The three grounds ADR-0021 §3 gave for keeping the GPU arms off the shelf —
static musl cannot `dlopen` a Vulkan loader, Nucleation's `blake3` build script
cannot cross-build to musl, and `-ldl` is unresolvable in the musl sysroot — are
all properties of musl-static and vanish with gnu targets. They are not
re-argued here. With them gone, the archive carries the whole binary, and §2's
floor is a fallback for a broken download rather than the only route to a
capability.

### 5. Nucleation comes from the registry, and `crates/render` is a workspace member

`crates/render` depends on `nucleation` by exact crates.io version
(`versions.toml [render].nucleation_version`), not by git rev. The
separate-workspace quarantine and `tools/check-workspace-git-deps.py` existed to
keep a git dependency's resolution-time clone out of every cargo command in the
repository; a registry dependency has no such reach, so both are gone and the
root workspace holds every crate under `crates/`.

### 6. Both channels publish the same eight crates

`cargo install delvec` must install the same surface the archive carries, so
every crate the binary is assembled from is published: `delvewright-dsl` on its
own version line (ADR-0017 §2's argument stands), and the seven engine crates —
`delvewright-compiler`, `delvewright-grammar`, `delvewright-schem`,
`delvewright-admit`, `delvewright-orchestrator`, `delvewright-render` and
`delvec` — on **the engine version line**, inherited from the root manifest's
`[workspace.package]`, with `=` requirements between them declared once in
`[workspace.dependencies]`. `versions.toml [engine].crates` enumerates the
engine crates; `validation/check-versions.sh` binds every manifest to it, and
`tools/check-publishable.sh` packages all eight and rebuilds `delvec` from the
packaged tarballs alone. A publish that half-succeeds is retried at the same
version (`tools/crates-io-publish.sh` skips what the registry already holds
byte-identically); a crate whose bytes must change after it landed costs a patch
bump of the whole line and a new tag, the same cost `delvec` alone carried.

### 7. Externals that cannot be integrated are lazy-loaded

A tool that cannot be bundled into the binary or the repository — Chunky is
the type specimen — is **acquired by the agent at the step that needs it**,
following the skill, not preinstalled by a setup phase. Wheels are used, never
reimplemented, and not everything is written in Rust: ADR-0021 §6's criterion
(upstream wheel, or a runtime not ours to choose) decides what stays non-Rust.
ADR-0018 §3's refusal to port `delve_skin` stands; what changes is timing —
its Python environment is established when a design first calls for a custom
skin, not at install time.

### 8. Optional tools are offered, never front-loaded

A tool that is optional for the run — an interactive viewer, a review aid —
is announced at the step where it is useful, with the choice to use it. It is
not part of `Init` and its absence never blocks the line.

### 9. The client jar is a creator prerequisite, acquired by the creator's choice

Visual validation is indispensable, so the pinned Minecraft client jar must be
present on the creator's machine; the toolchain never downloads it on the
creator's behalf without saying so, and never bundles or redistributes it
(ADR-0010). Because locating a jar means reading files outside the project,
the acquisition step **offers an explicit choice** — fetch the pinned version
from the version manifest, or point at an existing installation — and
**scanning the creator's disk is never a default**: it happens only behind an
explicit opt-in. Download is the default of the two. The concrete mechanism
(a `client_jar` pin plus a fetch-once, hash-refusing cache) is ADR-0021 §5.

### 10. Creators work in the content repository

ADR-0014 stands unchanged: creators clone only `delvewright-campaigns`, and
the skill reaches them there. Its implementation remains deferred; until it
lands, the documented flow runs from an engine checkout because the skill and
the toolchain live in this repository — which is §2's floor, stated as the
current path rather than presented as the destination.

## Consequences

- `README.md`, `docs/reference/tools.md`, `crates/delvec/README.md` and the
  opening of `docs/reference/compiler.md` introduce `delvec` as the delve
  creator; every caller of a retired binary name — CI, `tools/`, `validation/`,
  the harness, the references and the specs — names the subcommand instead.
- ADR-0017 remains the record for release/publishing machinery (CI-only
  publication, tag discipline); its creator-facing-install consequence and its
  musl targets are superseded. ADR-0018 remains the record for the IR hatch and
  the language classification; its "declared, installed once, neither
  discovered" posture is superseded by §2/§7.
- `tools/lib/clap_surface.py` reads a mounted group (a tuple variant whose
  `Args` type carries a `#[command(subcommand)]`) as one top-level subcommand
  with its nested flags folded in, so the release gate and the campaigns
  repository's skill check see the whole surface through one parser.
- The unimplemented remainder, each with its size:
  - **client-jar choice mechanism** (§9): a `versions.toml [minecraft]`
    client-jar pin, a fetch-once cache in the shape of the server-jar cache,
    and an opt-in scan flag — ADR-0021 §5's work. Until it lands, the skill
    asks the creator for the jar's location and searches nothing.
  - **the skill's `Init` downloads the archive** (§1): a content-repository
    change, held behind the first engine release built from this tree.
  - **ADR-0014's creator mode**: plugin distribution of the skill, content
    repo as workdir, dual-mode path resolution — still M4-deferred, and moving
    the skill into the content repository is a separate decision not taken
    here.

## Revisit triggers

- The skill ships from the content repository (ADR-0014 implemented): `Init`'s
  two-repository step collapses, and §2's floor is re-stated against that
  layout.
- A second distributed binary is ever proposed: §3 is the standing refusal and
  the bar it must clear.
- A Linux creator's machine is older than the printed glibc floor: the answer
  is §2's source build, and the floor is lowered only by moving the build
  runner, never by a static target.
- Nucleation's registry cadence stops carrying the API this repository needs:
  a git pin returns to `crates/render` alone, inside the workspace, as an
  ordinary reviewed edit — the quarantine is not rebuilt for it.
