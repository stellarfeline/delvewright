//! Diagnostics: the `--json` shape from spec-0002 and the stable `DW01xx` codes.

use serde::Serialize;

/// Diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// A hard rejection.
    Error,
    /// Advisory only (unused in v0, reserved for future rules).
    Warning,
}

/// One diagnostic, serialized as one JSON object per line by `delvec --json`.
///
/// Field order matches spec-0002: `code`, `severity`, `stage`, `path`, `message`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    /// Stable machine code, e.g. `DW0101`.
    pub code: String,
    /// Severity.
    pub severity: Severity,
    /// The stage this diagnostic concerns (`world`, `npcs`, …), or empty.
    pub stage: String,
    /// JSON-pointer-ish location within the stage document.
    pub path: String,
    /// Human-readable explanation.
    pub message: String,
}

impl Diagnostic {
    /// Build an error diagnostic.
    pub fn error(
        code: &str,
        stage: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Diagnostic {
            code: code.to_string(),
            severity: Severity::Error,
            stage: stage.into(),
            path: path.into(),
            message: message.into(),
        }
    }
}

/// The stable validation diagnostic codes (see `crates/dsl/README.md`).
pub mod codes {
    /// Document does not conform to its stage schema (unknown field / wrong type).
    pub const SCHEMA: &str = "DW0100";
    /// Envelope `stage` does not match the document's slot.
    pub const STAGE_MISMATCH: &str = "DW0101";
    /// Unsupported `dsl_version`.
    pub const DSL_VERSION: &str = "DW0102";
    /// Inconsistent `campaign_id` across stages.
    pub const CAMPAIGN_ID_MISMATCH: &str = "DW0103";
    /// Malformed id syntax (kebab-case / prefix).
    pub const ID_SYNTAX: &str = "DW0110";
    /// Duplicate id within its namespace.
    pub const ID_DUPLICATE: &str = "DW0111";
    /// Dangling reference: an id ref does not resolve.
    pub const DANGLING_REF: &str = "DW0112";
    /// Stage-6 dialogue node unreachable from `root`.
    pub const DIALOGUE_UNREACHABLE: &str = "DW0120";
    /// Stage-6 dialogue `root`/`next` references an unknown node.
    pub const DIALOGUE_BAD_REF: &str = "DW0121";
    /// Stage-6 dialogue effect references an objective that is unknown, not a
    /// `talk-to`, or a `talk-to` on a different NPC (foreign effect).
    pub const DIALOGUE_BAD_OBJECTIVE: &str = "DW0122";
    /// A stage-5 `talk-to` objective has no reachable completing dialogue option
    /// (the static half of the compiler's `DW0203` deadlock guarantee).
    pub const DIALOGUE_UNCOVERED: &str = "DW0123";
    /// Quest dependency cycle.
    pub const PLAN_CYCLE: &str = "DW0130";
    /// `finale` is not a declared quest.
    pub const FINALE_UNKNOWN: &str = "DW0131";
    /// `finale` is not the convergent sink of the plan.
    pub const FINALE_UNREACHABLE: &str = "DW0132";
    /// Non-mandatory quest (reserved in v0).
    pub const NON_MANDATORY: &str = "DW0133";
    /// Objective `after` cycle.
    pub const AFTER_CYCLE: &str = "DW0140";
    /// Reserved feature used (reserved enum value or reserved field).
    pub const RESERVED: &str = "DW0141";
    /// Anchor not provided by the area's bound prefab.
    pub const ANCHOR_UNRESOLVED: &str = "DW0142";
    /// Item id not in the pinned 1.21.11 registry.
    pub const ITEM_UNKNOWN: &str = "DW0143";
    /// Planned quest (stage 4) has no expansion in stage 5.
    pub const QUEST_NOT_EXPANDED: &str = "DW0150";
    /// Stage-5 quest is not planned in stage 4.
    pub const QUEST_NOT_PLANNED: &str = "DW0151";
    /// Stage-2 NPC has no stage-6 dialogue tree.
    pub const NPC_WITHOUT_TREE: &str = "DW0152";
    /// Stage-6 dialogue tree references an NPC not declared in stage 2.
    pub const TREE_WITHOUT_NPC: &str = "DW0153";
    /// Area binds neither or both of `prefab` / `prefab_pool` (exactly one
    /// required).
    pub const PREFAB_BINDING: &str = "DW0160";
    /// Area `prefab_pool` references a pool absent from `prefabs/` metadata.
    pub const POOL_UNKNOWN: &str = "DW0161";
    /// (v0.3) A `kill` objective or `spawn-wave` effect references a `wave/<id>`
    /// not declared in the stage-5 `waves` section (dangling wave reference).
    pub const WAVE_UNKNOWN: &str = "DW0170";
    /// (v0.3) A declared wave is referenced by a `kill` objective but is never
    /// spawned by any `spawn-wave` effect (referenced-but-never-spawned). A wave
    /// must be spawned by some effect before its kill objective is reachable.
    pub const WAVE_NEVER_SPAWNED: &str = "DW0171";
    /// (v0.3) A `requires_flags` entry references a `flag/<id>` that no `set-flag`
    /// effect ever produces (dangling flag reference).
    pub const FLAG_UNKNOWN: &str = "DW0172";
    /// (v0.3) A wave mob `entity` is not a known vanilla entity id. (Item-id
    /// checks for `collect.item`, `interact.requires_item` and `give-item.item`
    /// reuse [`ITEM_UNKNOWN`] / `DW0143`.)
    pub const ENTITY_UNKNOWN: &str = "DW0173";
}
