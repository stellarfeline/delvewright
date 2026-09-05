# ADR Index

Architecture Decision Records. Sequential, immutable once Accepted — supersede, don't
edit. Template at the bottom.

| # | Title | Status |
|---|-------|--------|
| [0001](0001-dsl-compiler-datapack.md) | Campaign DSL → deterministic compiler → datapack | Accepted |
| [0002](0002-staged-dsl.md) | Staged, dependency-driven DSL | Accepted |
| [0003](0003-vanilla-first.md) | Vanilla-first gameplay; mods in tooling layer only | Accepted |
| [0004](0004-prefab-jigsaw.md) | Prefab library assembled via vanilla jigsaw | Accepted |
| [0005](0005-two-layer-validation.md) | Two-layer validation (static + dynamic) | Accepted |
| [0006](0006-determinism.md) | Determinism as a hard invariant | Accepted |
| [0007](0007-monorepo-licensing.md) | Monorepo; GPL code, separately-licensed content | Accepted |
| [0008](0008-ci-as-arbiter.md) | Spec-driven development; CI as sole arbiter | Accepted |
| [0009](0009-pinned-mc-version.md) | Pinned Minecraft version: 1.21.11 | Accepted |
| [0010](0010-oci-packaging.md) | Delves ship as versioned OCI images | Accepted |
| [0011](0011-compiler-foundation.md) | Compiler foundation: Rust-native + mecha CI cross-check | Accepted |
| [0012](0012-product-form-claude-code-skill.md) | Product form: Claude Code skill front-end | Accepted |
| [0013](0013-prefab-license-allowlist.md) | Expanded prefab license allowlist (+MIT/Apache/GPL) | Accepted |
| [0014](0014-creator-distribution.md) | Creator distribution: plugin install, content repo as workdir | Accepted (M4) |
| [0015](0015-schema-promotion-policy.md) | Schema promotion: composition first; native only via second-campaign or machine-proof gate | Accepted |
| [0016](0016-three-layer-versioning.md) | Three-layer versioning: format, engine, skill | Accepted |
| [0017](0017-toolchain-distribution.md) | Toolchain distribution: `cargo install delvec`, release shelf, CI-only publishing | Accepted (§3 and the musl targets superseded by ADR-0023; install default superseded by ADR-0023) |
| [0018](0018-creator-toolchain-and-the-ir-hatch.md) | Creator toolchain: cargo as a prerequisite, one authoring crate, the escape hatch at the grammar IR | Accepted (§2–§3 prerequisite posture superseded by ADR-0023) |
| [0019](0019-java-edition-bedrock-shelved.md) | Java edition stays; a Bedrock backend is shelved | Accepted |
| [0020](0020-map-design-pipeline.md) | The spatial contract — declared spaces and edges, checked against the emitted bytes | Superseded by ADR-0022 |
| [0021](0021-creator-toolchain-rederived.md) | Creator toolchain re-derived: one distributed binary, registry Nucleation, off-the-shelf viewer core | Superseded by ADR-0023 |
| [0022](0022-the-map-is-planned-before-it-is-built.md) | The map is planned before it is built — the whole-first pipeline | Accepted |
| [0023](0023-creator-toolchain-as-decided.md) | The creator toolchain as decided: one `delvec` is the delve creator, archive-first acquisition, source-build floor, lazy-loaded externals | Proposed |

## Template

```markdown
# ADR-NNNN: Title

- **Status**: Proposed | Accepted | Superseded by ADR-XXXX
- **Date**: YYYY-MM-DD
- **Source**: who/what decided this

## Context
## Decision
## Consequences
## Revisit triggers   (optional: concrete conditions under which to reopen)
```
