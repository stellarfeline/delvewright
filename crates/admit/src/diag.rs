//! Diagnostics for `delve-admit`, mirroring the compiler / schem `DWxxxx`,
//! one-JSON-object-per-line convention (spec-0002). spec-0007 owns the `DW07xx`
//! range; schem holds `DW0700..DW0702` + `DW0710`, render holds `DW072x`, so the
//! admission half takes **`DW073x..DW076x`**:
//!
//! | Code | Meaning |
//! | --- | --- |
//! | `DW0730` | audit: a palette block is not in the allowlist. |
//! | `DW0731` | audit: a hard-forbidden code-injection vector (command/structure block, NBT-bearing spawner, or embedded `Command`). |
//! | `DW0732` | input error (unreadable/unparseable `.nbt` or metadata/JSON). |
//! | `DW0733` | audit: a palette block state does not exist in the pinned Minecraft version, in a template claiming the pin's `DataVersion` (or later) — no datafix runs, the block loads as air. |
//! | `DW0734` | audit (warning, defined in `delvewright_schem::blocks`): a pre-pin template carries a state the pin does not know; load-time datafixing is expected to migrate it. |
//! | `DW0735` | audit (defined in `delvewright_schem::blocks`): a palette entry omits a shape-carrying (multipart) property, so it places disconnected. |
//! | `DW0740` | catalog card: schema/field validation failure. |
//! | `DW0741` | catalog card: license not in the ADR-0013 allowlist (NC/ND/unknown reject). |
//! | `DW0750` | admission tooling (socket/anchor/lighting) failure. |
//! | `DW0751` | lighting probe: a `dark` interior was measured. |
//! | `DW0760` | gallery emission / curation failure. |
//!
//! Diagnostics go to **stderr** so stdout stays reserved for machine-readable
//! reports (audit report, curation report, palette dumps).

use serde::Serialize;

pub const DW_ALLOWLIST: &str = "DW0730";
pub const DW_FORBIDDEN: &str = "DW0731";
pub const DW_INPUT: &str = "DW0732";
pub const DW_UNKNOWN_BLOCK: &str = "DW0733";
pub const DW_CATALOG: &str = "DW0740";
pub const DW_LICENSE: &str = "DW0741";
pub const DW_TOOLING: &str = "DW0750";
pub const DW_DARK: &str = "DW0751";
pub const DW_GALLERY: &str = "DW0760";
/// A piece's declared spatial contract disagrees with its own blocks
/// (spec-0036 §2). The second door onto the one checker; the first is
/// `delve-grammar expand`.
pub const DW_CONTRACT: &str = "DW0782";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Warning,
    Error,
}

/// One diagnostic. `pos` is a local block position when the diagnostic is about a
/// specific cell (e.g. a forbidden block in a prefab).
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pos: Option<[i32; 3]>,
}

impl Diagnostic {
    pub fn warning(code: &'static str, message: impl Into<String>) -> Self {
        Diagnostic {
            code,
            severity: Severity::Warning,
            message: message.into(),
            pos: None,
        }
    }

    pub fn error(code: &'static str, message: impl Into<String>) -> Self {
        Diagnostic {
            code,
            severity: Severity::Error,
            message: message.into(),
            pos: None,
        }
    }

    pub fn at(mut self, pos: [i32; 3]) -> Self {
        self.pos = Some(pos);
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    /// Print to stderr, honoring `--json` (one JSON object per line).
    pub fn print(&self, json: bool) {
        if json {
            eprintln!("{}", serde_json::to_string(self).unwrap());
        } else {
            let sev = match self.severity {
                Severity::Warning => "warning",
                Severity::Error => "error",
            };
            let pos = self
                .pos
                .map(|p| format!(" at {},{},{}", p[0], p[1], p[2]))
                .unwrap_or_default();
            eprintln!("{} [{sev}] {}{pos}", self.code, self.message);
        }
    }
}
