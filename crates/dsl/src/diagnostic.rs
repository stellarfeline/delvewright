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
    /// Dialogue node unreachable from `root`.
    pub const DIALOGUE_UNREACHABLE: &str = "DW0120";
    /// Dialogue `root`/`next` references an unknown node.
    pub const DIALOGUE_BAD_REF: &str = "DW0121";
    /// Dialogue effect references an unknown objective.
    pub const DIALOGUE_BAD_OBJECTIVE: &str = "DW0122";
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
}
