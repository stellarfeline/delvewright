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
| [0009](0009-pinned-mc-version.md) | Pinned Minecraft version | Proposed |
| [0010](0010-oci-packaging.md) | Delves ship as versioned OCI images | Accepted |
| [0011](0011-compiler-foundation.md) | Compiler foundation: Rust-native + mecha CI cross-check | Accepted |

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
