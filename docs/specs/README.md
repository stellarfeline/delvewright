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
| [spec-0011](spec-0011-traps.md) | Traps — lethal & non-lethal environmental hazards | Approved |
| [spec-0012](spec-0012-checkpoints.md) | Checkpoints (respawn anchors) | Proposed |
| [spec-0013](spec-0013-playable-region.md) | Playable region & ocean horizon (pseudo-open-world) | Approved |
| [spec-0014](spec-0014-actors-staging.md) | Scripted actors & staging verbs (v0.6) | Approved |
| [spec-0015](spec-0015-visual-authoring-loop.md) | The visual authoring loop (agentic viewport) | Approved |
| [spec-0016](spec-0016-souls-mode.md) | Souls-mode mechanics (M4) | Draft |
| [spec-0017](spec-0017-map-editor.md) | The map editor (LLM world editing, layers 2+3) | Draft |
| [spec-0018](spec-0018-party-progression.md) | Party-shared progression (co-op division of labor) | Draft |
| [spec-0019](spec-0019-cutscene-rehearsal.md) | Cutscene rehearsal + in-game shot calibration | Draft |
| [spec-0020](spec-0020-npc-scene-ledger.md) | The NPC scene ledger — declared presence, checked against staging | Draft |
| [spec-0021](spec-0021-container-loot.md) | Container loot + actor equipment | Draft |
| [spec-0022](spec-0022-traps-v2-command-driven.md) | Traps v2 — physical triggers, command-driven consequences | Draft |
| [spec-0023](spec-0023-combat-verification-semantics.md) | Combat verification semantics — the machine proves the loop, not the win | Accepted |
| [spec-0024](spec-0024-release-pipeline.md) | Release pipeline — from green campaign branch to joinable delve | Approved |
| [spec-0025](spec-0025-branch-complete-verification.md) | Branch-complete narrative verification | Approved |
| [spec-0026](spec-0026-horizon-library.md) | Horizon library — five pseudo-open-world bases | Proposed |
| [spec-0027](spec-0027-grammar-prefab-backend.md) | Box-split grammar prefab back end | Proposed |
| [spec-0028](spec-0028-ref-image-intent-alignment.md) | Reference-image intent alignment (optional prefab-chain step) | Proposed |
| [spec-0029](spec-0029-i18n-v2-client-selected-language.md) | i18n v2 — the client picks the language | Implemented |
| [spec-0031](spec-0031-runtime-state-and-interactive-verbs.md) | Runtime state, and the verbs that need it | Draft |
| [spec-0032](spec-0032-economy-and-recovery-stake.md) | Currency, trade, and the recovery stake | Draft |
| [spec-0038](spec-0038-standing-fluid.md) | Standing fluid — declared bodies, and the flood level as runtime state | Proposed |
| [spec-0040](spec-0040-map-composition.md) | Map composition — how a whole map gets its appearance | Proposed |
| [spec-0042](spec-0042-a-way-that-content-opens.md) | A way that content opens — contingent edges, both signs, and the effect that opens them | Proposed |
| [spec-0043](spec-0043-an-open-space-carries-its-shadow.md) | An open space carries its shadow — the sky demand re-bound to the computed partition | Proposed |
