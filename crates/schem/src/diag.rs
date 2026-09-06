//! Diagnostics for `delvec schem`, mirroring the compiler's `DWxxxx` /
//! one-JSON-object-per-line convention (spec-0002). spec-0007 owns the `DW07xx`
//! range. Diagnostics go to stderr so stdout stays reserved for the
//! `--palette-report` audit output.

use serde::Serialize;

/// Strip audit hook (community contract): a forbidden block/entity was removed.
pub const DW_STRIP: &str = "DW0700";
/// An oversize schematic was tiled into structure parts.
pub const DW_SPLIT: &str = "DW0701";
/// The source `DataVersion` differs from the pinned MC 1.21.11 target.
pub const DW_DATAVERSION: &str = "DW0702";
/// The input could not be read or parsed as a Sponge schematic.
pub const DW_INPUT: &str = "DW0710";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Warning,
    Error,
}

/// One diagnostic. `pos` is a local/source block position when the diagnostic is
/// about a specific block (e.g. a strip).
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

    /// Print to stderr, honoring `--json` (one JSON object per line).
    pub fn print(&self, json: bool) {
        if json {
            // Serialization of a fixed struct cannot fail.
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
