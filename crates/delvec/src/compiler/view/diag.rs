//! Diagnostics + exit codes for `delvec render`, mirroring the compiler / schem
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
/// score RANKS the page and never gates it (spec-0028 §3), so an
/// ordering that drops, duplicates or overruns a candidate is refused.
pub const DW_RANK_ORDER: &str = "DW0725";
/// A contact sheet's score set bound to fewer candidates than the sheet holds —
/// zero is an error (nothing was ranked), a partial binding a warning. A gate
/// that binds to nothing is vacuous, not a pass (CLAUDE.md).
pub const DW_BINDING: &str = "DW0726";
/// An anchor's eye-level camera does not stand on the anchor's own cell — or
/// could not be stood up at all. A prefab is mostly solid, so an eye point taken
/// from an anchor position lands inside a block often enough that assuming it is
/// how the whole review goes blind; the resolution is reported here and in the
/// shot manifest instead, so the reviewer always knows where the body in the
/// frame is standing.
pub const DW_ANCHOR_EYE: &str = "DW0727";

// The rest of the `DW072x` block is spoken for, so the asset-resolution
// diagnostics below take their own `DW079x` block.
//
// Take the next unused number from the catalog in `docs/reference/compiler.md`
// **as it stands on `origin/main` at the moment of the merge**, never from the
// highest constant here and never from the catalog this branch started at:
// `DW078x` was free when these three were written and was taken by the spatial
// contract before they landed, so all three collided on arrival. The uniqueness
// half of `tools/check-dw-codes.py` is what says so.

/// A blockstate has no definition in the pinned asset source: the id does not
/// exist at this version, or its model or one of its textures is absent.
/// Reported with a cell count rather than drawn silently, because a block that
/// cannot be resolved is a finding about the prefab or the pin, not a cosmetic
/// detail — `minecraft:chain` is `minecraft:iron_chain` in 1.21.11, so a prefab
/// naming the old id gets the missing-texture placeholder here.
///
/// It says what the PAGE cannot draw, and nothing about what a server would
/// load. Those are different questions, and only the second depends on the
/// template's `DataVersion`: a pre-pin file is datafixed on load and places the
/// renamed block correctly, which is why
/// [`delvewright_dsl::blocks::judge_at`] calls that case a warning and not a
/// refusal. Answering the second question here would report a defect on a file
/// the game loads fine.
pub const DW_UNRESOLVED_BLOCK: &str = "DW0790";

/// A palette entry leaves shape-carrying properties unwritten. The state is
/// legal — a server fills the rest from the block's default state — but every
/// reader that is not a running server has to guess, and the guess is a full
/// cube: a `cobblestone_wall` with no `north`/`east`/`south`/`west` matches no
/// multipart case at all. Reported per state with the properties it omits and
/// the cell count, and it is what stops the page reporting a clean resolution
/// over a building whose walls are the wrong shape.
pub const DW_UNDERSPECIFIED_STATE: &str = "DW0791";

/// The resource bundle the page carries did not survive its own completeness
/// self-check: the texture atlas holds fewer cells than were packed into it, or a
/// block-entity texture id the emitter asks for resolves to nothing at the pin.
/// A finding about this toolchain, not about the prefab — both failures are
/// silent by construction, since a dropped atlas cell and a wrong id both render
/// as magenta and neither raises anything on its own.
pub const DW_VIEWER_RESOURCES: &str = "DW0792";

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
