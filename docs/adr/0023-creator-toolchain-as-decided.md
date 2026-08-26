# ADR-0023: The creator toolchain as decided — archive-first acquisition, a source-build floor, lazy-loaded externals

- **Status**: Accepted
- **Date**: 2026-08-25
- **Source**: the settled acquisition rules for the creator toolchain, until now
  recorded only in ADR-0021's source note and in `CLAUDE.md` (Methodology) while
  ADR-0017/0018 continued to state the shape they replaced. This ADR is the
  acquisition story of record; it decides how a creator *obtains* the toolchain,
  not what the toolchain is made of.
- **Refines**: ADR-0012 (product form), ADR-0014 (creator distribution),
  ADR-0021 (the toolchain re-derivation this ADR's rules commissioned)
- **Supersedes, in part**: ADR-0017 (its consequence naming `cargo install
  delvec` the creator-facing install) and ADR-0018 §2–§3 (their up-front
  prerequisite posture — the language and porting decisions stand)

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

## Decision

### 1. The default acquisition is the per-platform release archive

GitHub Releases carries `delvec` as **one archive per platform target** — never
loose files, and an archive is never re-split to save the creator disk space —
plus a `SHA256SUMS` beside them. **cargo is not a creator prerequisite.**
crates.io publication continues (ADR-0017's machinery is unchanged and CI
remains the only publisher), so `cargo install delvec` remains a supported
path; it is no longer the entry point. Distribution size under 100 MB and
build time are not decision inputs.

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

### 3. The distributed surface is one binary

One binary with subcommands (ADR-0021 §1, implemented). Splitting the
distribution into a second package to save space is **rejected**: it spends a
user decision to buy bytes nobody is short of. Binaries that only a checkout
can honestly carry — the prefab-authoring and admission tools, the GPU render
arms — are not distributed at all; §2's floor is how a creator reaches them,
and the skill's `Init` builds them at the step that needs them.

### 4. Externals that cannot be integrated are lazy-loaded

A tool that cannot be bundled into the binary or the repository — Chunky is
the type specimen — is **acquired by the agent at the step that needs it**,
following the skill, not preinstalled by a setup phase. Wheels are used, never
reimplemented, and not everything is written in Rust: ADR-0021 §6's criterion
(upstream wheel, or a runtime not ours to choose) decides what stays non-Rust.
ADR-0018 §3's refusal to port `delve_skin` stands; what changes is timing —
its Python environment is established when a design first calls for a custom
skin, not at install time.

### 5. Optional tools are offered, never front-loaded

A tool that is optional for the run — an interactive viewer, a review aid —
is announced at the step where it is useful, with the choice to use it. It is
not part of `Init` and its absence never blocks the line.

### 6. The client jar is a creator prerequisite, acquired by the creator's choice

Visual validation is indispensable, so the pinned Minecraft client jar must be
present on the creator's machine; the toolchain never downloads it on the
creator's behalf without saying so, and never bundles or redistributes it
(ADR-0010). Because locating a jar means reading files outside the project,
the acquisition step **offers an explicit choice** — fetch the pinned version
from the version manifest, or point at an existing installation — and
**scanning the creator's disk is never a default**: it happens only behind an
explicit opt-in. Download is the default of the two. The concrete mechanism
(a `client_jar` pin plus a fetch-once, hash-refusing cache) is ADR-0021 §5.

### 7. Creators work in the content repository

ADR-0014 stands unchanged: creators clone only `delvewright-campaigns`, and
the skill reaches them there. Its implementation remains deferred; until it
lands, the documented flow runs from an engine checkout because the skill and
the toolchain live in this repository — which is §2's floor, stated as the
current path rather than presented as the destination.

## Consequences

- `README.md` leads with the release archive and offers `cargo install` as the
  alternative, in the same change as this ADR.
- ADR-0017 remains the record for release/publishing machinery (shelf targets,
  CI-only publication, tag discipline); only its creator-facing-install
  consequence is superseded. ADR-0018 remains the record for the IR hatch and
  the language classification; its "declared, installed once, neither
  discovered" posture is superseded by §2/§4.
- The unimplemented remainder, each with its size:
  - **client-jar choice mechanism** (§6): a `versions.toml [minecraft]`
    client-jar pin, a fetch-once cache in the shape of the server-jar cache,
    and an opt-in scan flag — ADR-0021 §5's work. Until it lands, the skill
    asks the creator for the jar's location and searches nothing.
  - **ADR-0014's creator mode**: plugin distribution of the skill, content
    repo as workdir, dual-mode path resolution — still M4-deferred, and moving
    the skill into the content repository is a separate decision not taken
    here.
  - **ADR-0021 §2** (registry Nucleation, render re-entering the root
    workspace): an engine change with its own round; the render crate today
    still carries the git pin inside its own workspace.

## Revisit triggers

- The skill ships from the content repository (ADR-0014 implemented): `Init`'s
  two-repository step collapses, and §2's floor is re-stated against that
  layout.
- A second distributed binary is ever proposed: §3 is the standing refusal and
  the bar it must clear.
