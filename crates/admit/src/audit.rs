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

use std::collections::{BTreeMap, BTreeSet};

use delvewright_schem::blocks::{DW_SHAPE_OMITTED, DW_STATE_PRE_PIN, LoadedId, StateJudgement};
use delvewright_schem::convert::{forbidden_nbt, strip_ns};
use delvewright_schem::nbt::Nbt;
use delvewright_schem::split::TilePart;
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
use crate::diag::{DW_ALLOWLIST, DW_FORBIDDEN, DW_UNKNOWN_BLOCK, Diagnostic};
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
    /// Count of palette block states the pinned Minecraft version does not
    /// have **in a template that claims the pin** (no datafix will run, so
    /// they load as air).
    pub unknown_blocks: usize,
    /// Count of palette block states the pin does not have in a **pre-pin**
    /// template: the game's DataFixerUpper is expected to migrate them on
    /// load (`DW0734`, a warning — see `delvewright_schem::blocks`).
    pub pre_pin_unknown: usize,
    /// Count of palette entries omitting a shape-carrying property (`DW0735`):
    /// a wall/fence/pane/vine whose connection properties are unwritten loads
    /// as an isolated post, silently.
    pub underspecified: usize,
    /// **Stairs examined** by the `stair-shape` rule (`DW0801`) — its binding
    /// count. Zero means this piece has no stairs and the rule said nothing
    /// about it, which is a different fact from "the rule holds here".
    pub stairs_examined: usize,
    /// **Fluid cells examined** by the `fluid-contained` rule (`DW0800`) — its
    /// binding count, over the fluid that runs (`water`/`lava` blocks).
    pub fluid_cells_examined: usize,
    /// Cells written `waterlogged=true`: wet, measured not to spread, and so
    /// under no containment obligation.
    pub fluid_held_cells: usize,
    /// Run directions that leave the piece's own outer face, where these bytes
    /// decide nothing. Counted, never judged.
    pub fluid_at_edge: usize,
    pub findings: Vec<Finding>,
    /// For a zone that ships as a tile set: what was audited, tile by tile.
    ///
    /// Absent — and omitted from the JSON entirely — for a single structure
    /// template, so an ordinary report is exactly the report it always was.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tiles: Option<Vec<TileAudit>>,
}

/// One tile's contribution to a zone-level audit.
///
/// The bytes are audited per tile because per tile is where the bytes are; the
/// verdict is the zone's because a zone is what a player walks into. Listing
/// the tiles is what keeps that honest: a reader can see how many files the
/// verdict covers, and a zone audited from two tiles when its manifest declares
/// three is visible rather than implied.
#[derive(Debug, Clone, Serialize)]
pub struct TileAudit {
    /// The tile's `.nbt` filename.
    pub file: String,
    /// Its origin in zone coordinates.
    pub offset: [i32; 3],
    /// Its extent.
    pub size: [i32; 3],
    /// How many blocks it carries.
    pub block_count: usize,
    /// Its own verdict.
    pub verdict: &'static str,
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

/// **Audit a parsed structure**: its palette, and what the world will settle
/// its blocks into.
///
/// The two halves are separate functions because a TILE SET runs them at
/// different scales — the palette per tile, the settling rules over the
/// assembled zone. Both settling rules read a cell's neighbours, and a seam
/// between two tiles is packaging rather than geometry. Everything holding a
/// `Structure` gets both halves by calling this.
pub fn audit(asset: &str, s: &Structure, allow: &Allowlist) -> (AuditReport, Vec<Diagnostic>) {
    let (mut report, mut diags) = audit_palette(asset, s, allow);
    fold_in(
        &mut report,
        &mut diags,
        crate::settling::judge(&crate::spatial::grid(s)),
    );
    (report, diags)
}

/// Fold a settling verdict into a report: its counts always, its diagnostics
/// when it has any.
fn fold_in(
    report: &mut AuditReport,
    diags: &mut Vec<Diagnostic>,
    settling: crate::settling::Settling,
) {
    report.stairs_examined = settling.stairs_examined;
    report.fluid_cells_examined = settling.fluid_cells_examined;
    report.fluid_held_cells = settling.fluid_held_cells;
    report.fluid_at_edge = settling.fluid_at_edge;
    diags.extend(settling.diagnostics);
    if diags.iter().any(|d| d.is_error()) {
        report.verdict = "fail";
    }
    report.findings = diags.iter().map(to_finding).collect();
}

/// The palette half: every rule that judges one block state at a time.
fn audit_palette(asset: &str, s: &Structure, allow: &Allowlist) -> (AuditReport, Vec<Diagnostic>) {
    let mut diags: Vec<Diagnostic> = Vec::new();
    let mut forbidden = 0usize;
    let mut not_allowlisted = 0usize;
    let mut unknown_blocks = 0usize;
    let mut pre_pin_unknown = 0usize;
    let mut underspecified = 0usize;
    let registry = delvewright_schem::blocks::BlockRegistry::v1_21_11();

    // --- Palette allowlist: report each offending palette entry once, at the
    // first cell that uses it (deterministic: blocks are in file order). ---
    let mut allow_reported = vec![false; s.palette.len()];
    // --- Hard-forbid: block-level (command/structure/spawner blocks). ---
    let mut forbid_block_reported = vec![false; s.palette.len()];
    // --- Spelling: a palette entry the pinned game does not have. ---
    let mut unknown_reported = vec![false; s.palette.len()];
    // --- Shape: a palette entry omitting a multipart property. ---
    let mut shape_reported = vec![false; s.palette.len()];

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

        // 3. the block has to EXIST — judged against the pin AND the file's
        //    own DataVersion. An allowlist answers "should this block be
        //    here"; it cannot answer "is this a block at all", and the two
        //    questions fail in opposite directions — an allowlist is a curated
        //    list of names, so a name the game dropped stays in it forever and
        //    permits itself. `minecraft:chain`, renamed `iron_chain` in 1.21.11,
        //    was in this crate's own default allowlist and is in a shipped
        //    prefab; a structure template loads it as AIR, so the piece admits
        //    clean, ships, and is quietly missing whatever the block was for.
        //    But the game DATAFIXES every structure it loads against the
        //    file's DataVersion: the same `minecraft:chain` in a template that
        //    pre-dates the pin is renamed on load and is NOT a defect —
        //    `hero-temple-ruin-arch.nbt` (DataVersion 2975) is the shipped
        //    proof, and refusing it was a measured false positive. The rule
        //    lives in `BlockRegistry::judge_at`, not here.
        if !unknown_reported[b.state as usize] {
            match registry.judge_at(&entry.name, &entry.properties, s.data_version) {
                StateJudgement::Valid => {}
                StateJudgement::InvalidAtPin(e) => {
                    unknown_reported[b.state as usize] = true;
                    unknown_blocks += 1;
                    diags.push(
                        Diagnostic::error(
                            DW_UNKNOWN_BLOCK,
                            format!(
                                "{e} — the template claims DataVersion {}, so no datafix \
                                 will run on it",
                                s.data_version
                            ),
                        )
                        .at(b.pos),
                    );
                }
                StateJudgement::PrePin(e) => {
                    unknown_reported[b.state as usize] = true;
                    pre_pin_unknown += 1;
                    // WHICH id the fixer produces is derived, not left to the
                    // reader: the same resolution the allowlist judges on, said
                    // out loud, so the warning names the block that will be
                    // there rather than only the one that will not.
                    let resolution = match registry.loaded_id_at(&entry.name, s.data_version) {
                        LoadedId::Renamed { to, valid_through } => format!(
                            ", and the derived rename table maps it to `{to}` (the old id's \
                             last DataVersion is {valid_through})"
                        ),
                        LoadedId::AsWritten | LoadedId::Unresolved => String::new(),
                    };
                    diags.push(
                        Diagnostic::warning(
                            DW_STATE_PRE_PIN,
                            format!(
                                "{e}; the template's DataVersion {} pre-dates the pin \
                                 ({}){resolution}, so load-time datafixing is expected to \
                                 migrate this state — verify in-game that it does, because \
                                 an id no fixer maps (a typo) still loads as air",
                                s.data_version,
                                delvewright_schem::blocks::PIN_DATA_VERSION
                            ),
                        )
                        .at(b.pos),
                    );
                }
            }
        }

        // 3b. a valid state must WRITE its shape-carrying properties. A
        //     `variants` property it omits renders the block's complete
        //     default model — benign, the default is what the author meant.
        //     A `multipart` property it omits removes assembled geometry:
        //     `cobblestone_wall` with none written is an isolated post where
        //     the author drew a wall, and nothing downstream can tell. The
        //     class is derived from the game's own blockstate definitions
        //     (`BlockRegistry::shape_carrying`), never a hand-kept id list.
        if !shape_reported[b.state as usize] && !unknown_reported[b.state as usize] {
            let omitted = registry.omitted_shape_carrying(&entry.name, &entry.properties);
            if !omitted.is_empty() {
                shape_reported[b.state as usize] = true;
                underspecified += 1;
                diags.push(
                    Diagnostic::error(
                        DW_SHAPE_OMITTED,
                        format!(
                            "`{}` omits its shape-carrying propert{} {} — these assemble \
                             the block's model (multipart), so the omitted default drops \
                             geometry: the block places as an isolated post/patch instead \
                             of connecting. Write the connection state the design means",
                            entry.name,
                            if omitted.len() == 1 { "y" } else { "ies" },
                            omitted.join(", "),
                        ),
                    )
                    .at(b.pos),
                );
            }
        }

        // 4. palette allowlist — over the id a 1.21.11 SERVER will hold, not
        //    the id the bytes spell. The allowlist is a list of names at the
        //    pin, and a pre-pin template's palette is written in an older
        //    vocabulary that the game renames on load. Judging it as written
        //    made this tool contradict itself inside one run: rule 3 above
        //    passes `minecraft:chain` at DataVersion 2975 as a warning, because
        //    the fixer migrates it, and this rule refused the same cell as
        //    not-allowlisted. The resolution belongs to the registry
        //    (`loaded_id_at`), the permission to the list, and the composition
        //    is `judge_entry` so no caller can do only half of it.
        let judged = allow.judge_entry(entry, registry, s.data_version);
        if !judged.permitted && !allow_reported[b.state as usize] {
            allow_reported[b.state as usize] = true;
            not_allowlisted += 1;
            let via = match judged.renamed_from {
                Some(written) => format!(
                    " (written `{written}`, which this template's DataVersion {} \
                     datafixes to `{}`)",
                    s.data_version, judged.judged
                ),
                None => String::new(),
            };
            diags.push(
                Diagnostic::error(
                    DW_ALLOWLIST,
                    format!(
                        "block `{}`{via} is not in the palette allowlist — swap it for an \
                         allowlisted block, or, if the prefab genuinely needs it, propose adding \
                         it to the allowlist under review. Do NOT bypass the allowlist to admit \
                         the asset",
                        judged.judged
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
        unknown_blocks,
        pre_pin_unknown,
        underspecified,
        stairs_examined: 0,
        fluid_cells_examined: 0,
        fluid_held_cells: 0,
        fluid_at_edge: 0,
        findings: diags.iter().map(to_finding).collect(),
        tiles: None,
    };
    (report, diags)
}

/// Audit a zone that ships as a tile set: every tile's bytes, one zone verdict.
///
/// `tiles` are the manifest's parts paired with the structures they name, in
/// manifest order. Every finding's position is rebased into **zone**
/// coordinates before it is reported — a forbidden block at `3,2,1` of tile
/// `x0y0z1` is a cell no author can find in a design they wrote in zone
/// coordinates, and a diagnostic nobody can act on is a diagnostic that does
/// not exist.
///
/// The verdict is the zone's: it passes only if every tile passes. A tile is a
/// packaging unit and never a unit of judgement, so there is deliberately no
/// way to admit a zone whose second tile failed.
pub fn audit_tile_set(
    asset: &str,
    zone_size: [i32; 3],
    tiles: &[(TilePart, Structure)],
    allow: &Allowlist,
) -> (AuditReport, Vec<Diagnostic>) {
    let mut all_diags: Vec<Diagnostic> = Vec::new();
    let mut audits: Vec<TileAudit> = Vec::with_capacity(tiles.len());
    let mut palette: BTreeSet<String> = BTreeSet::new();
    let (mut block_count, mut forbidden, mut not_allowlisted, mut unknown_blocks) = (0, 0, 0, 0);
    let (mut pre_pin_unknown, mut underspecified) = (0, 0);

    for (part, structure) in tiles {
        let (rep, diags) = audit_palette(&part.file, structure, allow);
        block_count += rep.block_count;
        forbidden += rep.forbidden;
        not_allowlisted += rep.not_allowlisted;
        unknown_blocks += rep.unknown_blocks;
        pre_pin_unknown += rep.pre_pin_unknown;
        underspecified += rep.underspecified;
        palette.extend(rep.palette.iter().cloned());
        audits.push(TileAudit {
            file: part.file.clone(),
            offset: part.offset,
            size: part.size,
            block_count: rep.block_count,
            verdict: rep.verdict,
        });
        all_diags.extend(diags.into_iter().map(|mut d| {
            if let Some(p) = d.pos {
                d.pos = Some([
                    p[0] + part.offset[0],
                    p[1] + part.offset[1],
                    p[2] + part.offset[2],
                ]);
            }
            d
        }));
    }

    // The settling rules read neighbours, so they judge the ASSEMBLED zone and
    // never a tile: a channel or a stair run that crosses a seam is one piece
    // of geometry that happens to be packaged in two files.
    let settling = crate::settling::judge(&crate::settling::zone_grid(zone_size, tiles));
    all_diags.extend(settling.diagnostics);

    let verdict = if all_diags.iter().any(|d| d.is_error()) {
        "fail"
    } else {
        "pass"
    };
    let report = AuditReport {
        verdict,
        asset: asset.to_string(),
        size: zone_size,
        block_count,
        palette: palette.into_iter().collect(),
        forbidden,
        not_allowlisted,
        unknown_blocks,
        pre_pin_unknown,
        underspecified,
        stairs_examined: settling.stairs_examined,
        fluid_cells_examined: settling.fluid_cells_examined,
        fluid_held_cells: settling.fluid_held_cells,
        fluid_at_edge: settling.fluid_at_edge,
        findings: all_diags.iter().map(to_finding).collect(),
        tiles: Some(audits),
    };
    (report, all_diags)
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
