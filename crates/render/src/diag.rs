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
// Note the gap: `DW0724` belongs to the COMPILER's visual tier (player-POV
// camera eye cell), not to this crate — the `DW072x` block is shared with it, so
// take the next unused number from the catalog in `docs/reference/compiler.md`
// rather than from the highest constant here.
/// The contact sheet's ordering is not a total order over the candidates: the
/// score RANKS the page and never gates it (spec-0028 §3, owner ruling), so an
/// ordering that drops, duplicates or overruns a candidate is refused.
pub const DW_RANK_ORDER: &str = "DW0725";
/// A contact sheet's score set bound to fewer candidates than the sheet holds —
/// zero is an error (nothing was ranked), a partial binding a warning. A gate
/// that binds to nothing is vacuous, not a pass (CLAUDE.md).
pub const DW_BINDING: &str = "DW0726";
/// A blockstate in a prefab has no definition in the asset source it was
/// coloured from — the block does not exist at the pinned version, or the model
/// or its textures are absent. Reported with a cell count rather than drawn
/// silently, because a block that cannot be resolved is a finding about the
/// prefab or the pinned version, not a cosmetic detail: `minecraft:chain` is
/// `minecraft:iron_chain` in 1.21.11, and the prefabs that still name the old id
/// place a block the server does not have.
pub const DW_UNRESOLVED_BLOCK: &str = "DW0727";

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

    /// True for the error tier. A caller collecting a mixed list needs to tell
    /// "a finding worth printing" from "a finding worth stopping for".
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
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
