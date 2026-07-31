//! Diagnostics + exit codes for `delve-render`, mirroring the compiler / schem
//! `DWxxxx` + one-JSON-object-per-line convention (spec-0002). spec-0007 owns the
//! `DW07xx` range; schem holds `DW0700..DW0702`/`DW0710`, so render takes the
//! `DW072x` block. Diagnostics go to stderr; the only stdout output is a
//! machine-readable JSON summary (`--json`) or nothing.

use serde::Serialize;

/// A missing-texture (magenta) placeholder was detected in a render — the
/// fidelity gate's hard failure.
pub const DW_MISSING_TEXTURE: &str = "DW0720";
/// The input (`.nbt`, prefab metadata, or `render-plan.json`) could not be read
/// or parsed.
pub const DW_INPUT: &str = "DW0721";
/// An output file could not be written.
pub const DW_OUTPUT: &str = "DW0722";
/// The GPU renderer failed (device init, mesh, or frame) or textures are absent.
pub const DW_RENDER: &str = "DW0723";

/// Process exit codes (mirrors schem/compiler: 0 ok · 2 input/usage · 3 output ·
/// ≥10 internal). Render adds `4` for a fidelity-gate failure (a real
/// missing-texture finding — distinct from `5`, an infra/GPU/textures error, so
/// CI can tell "the fixture is broken" from "the renderer could not run").
pub mod exit {
    /// Success.
    pub const OK: u8 = 0;
    /// Input error or bad usage (unreadable/unparseable input).
    pub const INPUT: u8 = 2;
    /// Output error (cannot write).
    pub const OUTPUT: u8 = 3;
    /// Fidelity-gate failure: a missing-texture placeholder was detected.
    pub const FIDELITY: u8 = 4;
    /// Renderer/GPU/textures error (could not run the render at all).
    pub const RENDER: u8 = 5;
    /// Internal error.
    pub const INTERNAL: u8 = 10;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Warning,
    Error,
}

/// One diagnostic.
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
}

impl Diagnostic {
    pub fn error(code: &'static str, message: impl Into<String>) -> Self {
        Diagnostic {
            code,
            severity: Severity::Error,
            message: message.into(),
        }
    }

    pub fn warning(code: &'static str, message: impl Into<String>) -> Self {
        Diagnostic {
            code,
            severity: Severity::Warning,
            message: message.into(),
        }
    }

    /// Print to stderr, honoring `--json` (one JSON object per line).
    pub fn print(&self, json: bool) {
        if json {
            eprintln!("{}", serde_json::to_string(self).expect("serialize diag"));
        } else {
            let sev = match self.severity {
                Severity::Warning => "warning",
                Severity::Error => "error",
            };
            eprintln!("{} [{sev}] {}", self.code, self.message);
        }
    }
}
