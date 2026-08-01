//! The mechanical **NBT palette audit** — the CI gate for community prefabs
//! (spec-0007 community contract).
//!
//! Two independent checks over a converted `.nbt`:
//!
//! 1. **Hard-forbid** (`DW0731`, always an error): the code-injection vectors the
//!    task/spec name — **command blocks**, **structure blocks**, and
//!    **NBT-bearing spawners**, plus any block entity carrying an embedded
//!    `Command`. The recursive `Command` / spawn-NBT scan is the **exact** one the
//!    `delve-schem` conversion strip uses (reused from that crate, no drift).
//! 2. **Palette allowlist** (`DW0730`, an error): every palette block name must be
//!    in the (configurable) allowlist, so a reviewer sees any surprising block.
//!
//! ### Jigsaw is deliberately NOT hard-forbidden here (flagged policy)
//!
//! The conversion strip forbids `jigsaw` because a *raw community schematic* has no
//! business carrying sockets (we carve those during admission). But the admission
//! audit runs on **library prefabs**, whose jigsaw blocks are the legitimate
//! sockets the compiler's solver mates — and a jigsaw block entity cannot carry a
//! `Command`, so it is not an injection vector. Jigsaw is therefore treated as a
//! normal allowlisted block here; only command/structure blocks and NBT-bearing
//! spawners hard-fail. Bare `spawner`/`vault` blocks (no spawn NBT) are caught by
//! the allowlist (`DW0730`), not the hard-forbid, matching "NBT-bearing spawners".
//!
//! The audit is pure and deterministic. It emits a machine-readable
//! [`AuditReport`] (stdout JSON) plus per-finding [`Diagnostic`]s (stderr).

use std::collections::BTreeMap;

use delvewright_schem::convert::{forbidden_nbt, strip_ns};
use delvewright_schem::nbt::Nbt;
use serde::Serialize;

/// Block **names** (namespace-stripped) that hard-fail the admission audit: the
/// command blocks and the structure block. (Jigsaw is a legitimate socket; bare
/// spawners are caught by the allowlist — see the module docs.)
const HARD_FORBID_BLOCKS: &[&str] = &[
    "command_block",
    "chain_command_block",
    "repeating_command_block",
    "structure_block",
];

/// Block-**entity** ids that hard-fail regardless of payload (command/structure
/// blocks). Spawner-family block entities fail only when they carry spawn NBT,
/// which the shared `forbidden_nbt` scan detects.
const HARD_FORBID_BE: &[&str] = &[
    "command_block",
    "chain_command_block",
    "repeating_command_block",
    "structure_block",
];

use crate::allowlist::Allowlist;
use crate::diag::{DW_ALLOWLIST, DW_FORBIDDEN, Diagnostic};
use crate::structure::Structure;

/// One machine-readable audit finding (mirrors a [`Diagnostic`] in JSON form).
#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub code: &'static str,
    pub severity: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pos: Option<[i32; 3]>,
}

/// The audit report: a machine-readable verdict over one asset.
#[derive(Debug, Clone, Serialize)]
pub struct AuditReport {
    /// `"pass"` when no error-severity finding fired, else `"fail"`.
    pub verdict: &'static str,
    /// The asset id / path label (set by the caller).
    pub asset: String,
    pub size: [i32; 3],
    pub block_count: usize,
    /// Every distinct palette block name (sorted), for reviewer visibility.
    pub palette: Vec<String>,
    /// Count of hard-forbidden hits.
    pub forbidden: usize,
    /// Count of not-allowlisted palette blocks.
    pub not_allowlisted: usize,
    pub findings: Vec<Finding>,
}

impl AuditReport {
    pub fn is_pass(&self) -> bool {
        self.verdict == "pass"
    }

    /// Canonical pretty JSON + trailing newline (the machine-readable report).
    pub fn to_json(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).expect("report serializes");
        s.push('\n');
        s
    }
}

fn to_finding(d: &Diagnostic) -> Finding {
    Finding {
        code: d.code,
        severity: if d.is_error() { "error" } else { "warning" },
        message: d.message.clone(),
        pos: d.pos,
    }
}

/// Audit a parsed structure against `allow`. Returns the report and the raw
/// diagnostics (for stderr printing).
pub fn audit(asset: &str, s: &Structure, allow: &Allowlist) -> (AuditReport, Vec<Diagnostic>) {
    let mut diags: Vec<Diagnostic> = Vec::new();
    let mut forbidden = 0usize;
    let mut not_allowlisted = 0usize;

    // --- Palette allowlist: report each offending palette entry once, at the
    // first cell that uses it (deterministic: blocks are in file order). ---
    let mut allow_reported = vec![false; s.palette.len()];
    // --- Hard-forbid: block-level (command/structure/spawner blocks). ---
    let mut forbid_block_reported = vec![false; s.palette.len()];

    for b in &s.blocks {
        let entry = &s.palette[b.state as usize];
        let local = strip_ns(&entry.name);

        // 1. hard-forbidden block name.
        if HARD_FORBID_BLOCKS.contains(&local) && !forbid_block_reported[b.state as usize] {
            forbid_block_reported[b.state as usize] = true;
            forbidden += 1;
            diags.push(
                Diagnostic::error(
                    DW_FORBIDDEN,
                    format!(
                        "forbidden block `{}` — a code-injection vector (command/structure/jigsaw \
                         block). Remove it from the prefab; this is hard-forbidden with no \
                         allowlist path (ADR-0003 vanilla-first)",
                        entry.name
                    ),
                )
                .at(b.pos),
            );
        }

        // 2. block-entity checks (id + recursive NBT scan). These are per-cell,
        //    not per-palette, since the payload varies by position.
        if let Some(Nbt::Compound(data)) = &b.nbt
            && let Some(reason) = forbidden_be(data)
        {
            forbidden += 1;
            diags.push(
                Diagnostic::error(
                    DW_FORBIDDEN,
                    format!(
                        "forbidden block entity: {reason} — a code-injection vector (NBT-bearing \
                         spawner or embedded `Command`). Strip it from the prefab; hard-forbidden \
                         with no allowlist path"
                    ),
                )
                .at(b.pos),
            );
        }

        // 3. palette allowlist.
        if !allow.permits_entry(entry) && !allow_reported[b.state as usize] {
            allow_reported[b.state as usize] = true;
            not_allowlisted += 1;
            diags.push(
                Diagnostic::error(
                    DW_ALLOWLIST,
                    format!(
                        "block `{}` is not in the palette allowlist — swap it for an allowlisted \
                         block, or, if the prefab genuinely needs it, propose adding it to the \
                         allowlist under review. Do NOT bypass the allowlist to admit the asset",
                        entry.name
                    ),
                )
                .at(b.pos),
            );
        }
    }

    let verdict = if diags.iter().any(|d| d.is_error()) {
        "fail"
    } else {
        "pass"
    };
    let report = AuditReport {
        verdict,
        asset: asset.to_string(),
        size: s.size,
        block_count: s.blocks.len(),
        palette: s.block_names().into_iter().collect(),
        forbidden,
        not_allowlisted,
        findings: diags.iter().map(to_finding).collect(),
    };
    (report, diags)
}

/// A block entity's own forbidden reason: a forbidden `id`, or an embedded
/// command / spawner definition anywhere in its NBT. Reuses the schem crate's
/// recursive scan so the audit and the conversion strip agree.
fn forbidden_be(data: &BTreeMap<String, Nbt>) -> Option<String> {
    if let Some(id) = data.get("id").and_then(Nbt::as_str)
        && HARD_FORBID_BE.contains(&strip_ns(id))
    {
        return Some(format!("id `{id}`"));
    }
    forbidden_nbt(data).map(|reason| reason.to_string())
}
