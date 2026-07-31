# Specs

One spec per feature, owner-approved before implementation (ADR-0008: no spec, no
feature). Status: **Skeleton** (structure agreed, details unwritten) → **Draft** →
**Approved** → **Implemented**.

Every spec must contain an **Acceptance criteria** section phrased as
machine-checkable assertions — each criterion maps to a CI check.

| Spec | Title | Status |
|------|-------|--------|
| [spec-0001](spec-0001-dsl-schemas.md) | Campaign DSL schemas (staged) | Approved |
| [spec-0002](spec-0002-compiler-cli.md) | Compiler CLI contract | Approved |
| [spec-0003](spec-0003-validation-harness.md) | Validation harness contract | Draft |
| [spec-0004](spec-0004-ci-workflow.md) | CI workflow | Draft |
| [spec-0005](spec-0005-infra-images.md) | Infrastructure images & version manifest | Draft |
| [spec-0006](spec-0006-playtest-loop.md) | Creator playtest loop | Draft |
| [spec-0007](spec-0007-asset-pipeline.md) | External asset pipeline (two-track) | Approved |
| [spec-0008](spec-0008-dsl-v0.4.md) | DSL v0.4 — expressiveness (dialogue state, props, narration, live threats, presentation) | Approved |
| [spec-0009](spec-0009-npc-skins.md) | NPC skin pipeline — creation-first, resource-pack delivery | Approved |
| [spec-0010](spec-0010-assembled-relight.md) | Assembled-world lighting, deterministic relight, declared time & weather | Implemented |
