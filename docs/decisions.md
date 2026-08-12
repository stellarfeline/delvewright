# Owner decision ledger

Every decision the owner has made that is written down anywhere in this repo,
and — for each — **where in the tree it actually landed**.

`tools/check-decisions.py` binds this file in both directions and is a required
status check. A `landed` row whose binding stops resolving reds the PR that
broke it. A decision written into an ADR, an owner-approved spec's Status line,
or a dated `(owner …)` annotation in `CLAUDE.md` with no row here also reds —
that direction is what stops the ledger from measuring only what someone
remembered to add.

**`open` rows are the point of this file.** They are decisions that are recorded
and not built. They are printed with their age on every CI run so that "your
decision never landed" is a red on a pull request instead of a discovery in
conversation months later.

| id | date | decision | source | status | binding |
|---|---|---|---|---|---|
| DEC-0001 | 2026-07-29 | Campaign DSL → deterministic compiler → datapack | `adr/0001` | constitutive | `docs/adr/0001-dsl-compiler-datapack.md::Status\*\*: Accepted` |
| DEC-0002 | 2026-07-29 | Staged, dependency-driven DSL | `adr/0002` | constitutive | `docs/adr/0002-staged-dsl.md::Status\*\*: Accepted` |
| DEC-0003 | 2026-07-29 | Vanilla-first gameplay; mods in tooling layer only | `adr/0003` | constitutive | `docs/adr/0003-vanilla-first.md::Status\*\*: Accepted` |
| DEC-0004 | 2026-07-29 | Prefab library assembled via vanilla jigsaw | `adr/0004` | constitutive | `docs/adr/0004-prefab-jigsaw.md::Status\*\*: Accepted` |
| DEC-0005 | 2026-07-29 | Two-layer validation (static + dynamic) | `adr/0005` | constitutive | `docs/adr/0005-two-layer-validation.md::Status\*\*: Accepted` |
| DEC-0006 | 2026-07-29 | Determinism as a hard invariant | `adr/0006` | constitutive | `docs/adr/0006-determinism.md::Status\*\*: Accepted` |
| DEC-0007 | 2026-07-29 | Monorepo; GPL code, separately-licensed content | `adr/0007` | constitutive | `docs/adr/0007-monorepo-licensing.md::Status\*\*: Accepted` |
| DEC-0008 | 2026-07-29 | Spec-driven development; CI as sole arbiter | `adr/0008` | constitutive | `docs/adr/0008-ci-as-arbiter.md::Status\*\*: Accepted` |
| DEC-0009 | 2026-07-29 | Pinned Minecraft version — 1.21.11 | `adr/0009` | constitutive | `docs/adr/0009-pinned-mc-version.md::Status\*\*: Accepted` |
| DEC-0010 | 2026-07-29 | Delves ship as versioned OCI images | `adr/0010` | constitutive | `docs/adr/0010-oci-packaging.md::Status\*\*: Accepted` |
| DEC-0011 | 2026-07-29 | Compiler foundation — Rust-native, with mecha as CI cross-check | `adr/0011` | constitutive | `docs/adr/0011-compiler-foundation.md::Status\*\*: Accepted` |
| DEC-0012 | 2026-07-30 | Product form — Claude Code skill as the generation front-end | `adr/0012` | constitutive | `docs/adr/0012-product-form-claude-code-skill.md::Status\*\*: Accepted` |
| DEC-0013 | 2026-07-31 | Expanded prefab license allowlist | `adr/0013` | constitutive | `docs/adr/0013-prefab-license-allowlist.md::Status\*\*: Accepted` |
| DEC-0014 | 2026-07-31 | Creator distribution — plugin install, content repo as workdir | `adr/0014` | constitutive | `docs/adr/0014-creator-distribution.md::Status\*\*: Accepted` |
| DEC-0015 | 2026-08-06 | Toolchain distribution — `cargo install delvec`, a checksum-verified release shelf, and CI as the only publisher | `adr/0017` | constitutive | `docs/adr/0017-toolchain-distribution.md::Status\*\*: Accepted` |
| DEC-0016 | 2026-08-07 | The creator toolchain — cargo as a prerequisite, one authoring crate, and the escape hatch at the grammar IR | `adr/0018` | constitutive | `docs/adr/0018-creator-toolchain-and-the-ir-hatch.md::Status\*\*: Accepted` |
| DEC-0017 | 2026-08-02 | Java edition stays; a Bedrock backend is shelved | `adr/0019` | constitutive | `docs/adr/0019-java-edition-bedrock-shelved.md::Status\*\*: Accepted` |
| DEC-0018 | 2026-08-01 | Campaign DSL schemas (staged) | `spec/0001` | landed | `docs/reference/compiler.md::DW0120` |
| DEC-0019 | 2026-07-29 | Compiler CLI contract | `spec/0002` | landed | `docs/reference/compiler.md::DW0180` |
| DEC-0020 | 2026-08-01 | Validation harness contract | `spec/0003` | landed | `docs/reference/compiler.md::DW0724` |
| DEC-0021 | 2026-08-01 | CI workflow | `spec/0004` | landed | `.github/workflows/ci.yml` |
| DEC-0022 | 2026-07-30 | Infrastructure images & version manifest | `spec/0005` | landed | `validation/compose.yaml` |
| DEC-0023 | 2026-07-30 | Creator playtest loop | `spec/0006` | landed | `tools/playtest-server.sh` |
| DEC-0024 | 2026-07-30 | External asset pipeline (two-track) | `spec/0007` | landed | `crates/render/src/lib.rs` |
| DEC-0025 | 2026-07-31 | DSL v0.4 — expressiveness (dialogue state, props, narration, live threats, presentation) | `spec/0008` | landed | `docs/reference/compiler.md::DW0132` |
| DEC-0026 | 2026-07-31 | NPC skin pipeline — creation-first, resource-pack delivery | `spec/0009` | landed | `tools/skin` |
| DEC-0027 | 2026-07-31 | spec-0011 — Traps (lethal & non-lethal environmental hazards) | `spec/0011` | landed | `docs/reference/compiler.md::DW0141` |
| DEC-0028 | 2026-07-31 | spec-0012 — Checkpoints (respawn anchors) | `spec/0012` | landed | `docs/reference/compiler.md::DW0311` |
| DEC-0029 | 2026-08-01 | spec-0013 — Playable region & ocean horizon (pseudo-open-world staging) | `spec/0013` | landed | `docs/reference/compiler.md::DW0320` |
| DEC-0030 | 2026-08-01 | spec-0014 — Scripted actors & staging verbs (v0.6) | `spec/0014` | landed | `docs/reference/compiler.md::DW0100` |
| DEC-0031 | 2026-08-01 | spec-0015 — The visual authoring loop (agentic viewport) | `spec/0015` | landed | `crates/render/src/shots.rs` |
| DEC-0032 | 2026-08-01 | spec-0016 — Souls-mode mechanics (M4) | `spec/0016` | landed | `docs/reference/compiler.md::DW0315` |
| DEC-0033 | 2026-08-01 | spec-0017 — The map editor (LLM world editing, layers 2+3) | `spec/0017` | landed | `docs/reference/compiler.md::DW0311` |
| DEC-0034 | 2026-08-02 | spec-0018 — Party-shared progression (co-op division of labor) | `spec/0018` | landed | `crates/compiler/src/stake.rs` |
| DEC-0035 | 2026-08-02 | Cutscene rehearsal + in-game shot calibration | `spec/0019` | landed | `docs/reference/compiler.md::DW0308` |
| DEC-0036 | 2026-08-03 | The NPC scene ledger — declared presence, checked against staging | `spec/0020` | landed | `docs/reference/compiler.md::DW0195` |
| DEC-0037 | 2026-08-03 | spec-0021 — Container loot + actor equipment | `spec/0021` | landed | `docs/reference/compiler.md::DW0141` |
| DEC-0038 | 2026-08-03 | Traps v2 — physical triggers, command-driven consequences | `spec/0022` | landed | `docs/reference/compiler.md::DW0363` |
| DEC-0039 | 2026-08-03 | Combat verification semantics — the machine proves the loop, not the win | `spec/0023` | landed | `docs/reference/compiler.md::DW0450` |
| DEC-0040 | 2026-08-03 | Release pipeline — from green campaign branch to joinable delve | `spec/0024` | landed | `.github/workflows/release.yml` |
| DEC-0041 | 2026-08-03 | Branch-complete narrative verification — every branch is played, not just declared | `spec/0025` | landed | `crates/compiler/src/branch.rs` |
| DEC-0042 | 2026-08-04 | Horizon library — five pseudo-open-world bases | `spec/0026` | landed | `docs/reference/compiler.md::DW0210` |
| DEC-0043 | 2026-08-04 | Box-split grammar prefab back end | `spec/0027` | open | — |
| DEC-0044 | 2026-08-01 | Reference-image intent alignment (optional prefab-chain step) | `spec/0028` | landed | `docs/reference/compiler.md::DW0725` |
| DEC-0045 | 2026-08-08 | Runtime state, and the verbs that need it | `spec/0031` | landed | `docs/reference/compiler.md::DW0100` |
| DEC-0046 | 2026-08-08 | Currency, trade, and the recovery stake | `spec/0032` | landed | `crates/compiler/src/stake.rs` |
| DEC-0047 | 2026-07-30 | self-contained Minecraft adventure "delves" on demand | `claude/self-contained-minecraft-adventure-delves-on@2026-07-30` | unenforced | — |
| DEC-0048 | 2026-07-31 | No hacks at any layer | `claude/no-hacks-at-any-layer@2026-07-31` | unenforced | — |
| DEC-0049 | 2026-08-06 | This is a general engine. Primitives are abstract, flexible and configurable, and never bound to | `claude/this-is-a-general-engine-primitives-are-abst@2026-08-06` | unenforced | — |
| DEC-0050 | 2026-07-31 | Debug doctrine | `claude/debug-doctrine@2026-07-31` | unenforced | — |
| DEC-0051 | 2026-08-05 | Debug doctrine | `claude/debug-doctrine@2026-08-05` | unenforced | — |
| DEC-0052 | 2026-08-05 | CI is the sole arbiter | `claude/ci-is-the-sole-arbiter@2026-08-05` | landed | `tools/check-required-contexts.py` |
| DEC-0053 | 2026-07-30 | PR merge policy | `claude/pr-merge-policy@2026-07-30` | unenforced | — |
| DEC-0054 | 2026-07-31 | PR merge policy | `claude/pr-merge-policy@2026-07-31` | unenforced | — |
| DEC-0055 | 2026-08-04 | PR merge policy | `claude/pr-merge-policy@2026-08-04` | unenforced | — |
| DEC-0056 | 2026-08-02 | Audience separation in docs | `claude/audience-separation-in-docs@2026-08-02` | unenforced | — |
| DEC-0057 | 2026-08-04 | Version-adoption discipline | `claude/version-adoption-discipline@2026-08-04` | unenforced | — |
| DEC-0058 | 2026-08-08 | A settled ruling is never re-asked. Search the record first | `claude/a-settled-ruling-is-never-re-asked-search-th@2026-08-08` | unenforced | — |
| DEC-0059 | 2026-08-08 | A release is built from a frozen approved tree, never from a moving branch | `claude/a-release-is-built-from-a-frozen-approved-tr@2026-08-08` | unenforced | — |
| DEC-0060 | 2026-08-02 | Tooling sync | `claude/tooling-sync@2026-08-02` | landed | `docs/reference/tools.md` |
| DEC-0061 | 2026-08-03 | Every new mechanic owes a demo level | `claude/every-new-mechanic-owes-a-demo-level@2026-08-03` | landed | `docs/demo-levels.md` |
| DEC-0062 | 2026-08-05 | Every dispatched worker runs in its own git worktree | `claude/every-dispatched-worker-runs-in-its-own-git-@2026-08-05` | unenforced | — |
| DEC-0063 | 2026-08-11 | A worktree is created by the dispatch and destroyed by the MERGE | `claude/a-worktree-is-created-by-the-dispatch-and-de@2026-08-11` | unenforced | — |
| DEC-0064 | 2026-08-02 | Privacy in repo artifacts | `claude/privacy-in-repo-artifacts@2026-08-02` | unenforced | — |
| DEC-0065 | 2026-07-31 | Attribution ledger | `claude/attribution-ledger@2026-07-31` | landed | `docs/ACKNOWLEDGEMENTS.md` |
| DEC-0066 | 2026-07-31 | DW-diagnostic coverage | `claude/dw-diagnostic-coverage@2026-07-31` | landed | `tools/check-dw-codes.py` |
| DEC-0067 | 2026-08-11 | A reader-facing document is written in the present tense of the current version | `claude/a-reader-facing-document-is-written-in-the-p@2026-08-11` | unenforced | — |
| DEC-0068 | 2026-08-11 | An approved design becomes a campaign file, and every later stage reads it from the campaign, never from a chat artifact | `skill/new-delve-design-persistence` | landed | `.claude/skills/new-delve/SKILL.md::design/concept` |
| DEC-0070 | 2026-08-12 | When an approved concept and the back end disagree, grow the back end — never cut the concept down to what the tooling happens to say | `spec/0033` | open | — |
