//! What runs `delvec prefab` — the prefab admission pipeline (spec-0007, M3);
//! the command line's type is [`crate::admit::cli`].
//!
//! Exit codes: `0` ok · `1` audit/validation failure · `2` input error · `3`
//! output error · `≥10` internal. Diagnostics (`DW073x..DW076x`) go to stderr,
//! one JSON object per line under `--json`; machine-readable reports go to stdout
//! (or a `--report`/`-o` file).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::admit::allowlist::Allowlist;
use crate::admit::audit::{self, audit};
use crate::admit::catalog::CatalogCard;
use crate::admit::cli::{CatalogCmd, PrefabArgs, PrefabCommand};
use crate::admit::diag::{
    DW_DARK, DW_FRAGMENT, DW_GALLERY, DW_INPUT, DW_NO_PROVENANCE, DW_TOOLING, DW_UNBOUND,
    Diagnostic,
};
use crate::admit::gallery::{self, Candidate};
use crate::admit::light::{self, Zone};
use crate::admit::meta::{self, AnchorEdit, AnchorRole, License, PrefabMeta, Region};
use crate::admit::settling;
use crate::admit::socket::{self, SocketDecl};
use crate::admit::spatial::Door;
use crate::admit::structure::Structure;
use crate::schem::split::{TilePart, TileSet, fragment_refusal, tile_evidence};

const EXIT_FAIL: u8 = 1;
const EXIT_INPUT: u8 = 2;
const EXIT_OUTPUT: u8 = 3;

/// Run `delvec prefab`. `json` is `delvec`'s global diagnostics flag, and
/// `prefabs_dir` its global `--prefabs`, which is the library a command that
/// takes a whole library rather than one file reads.
pub fn run(args: PrefabArgs, prefabs_dir: &Path, json: bool) -> ExitCode {
    match args.command {
        PrefabCommand::Seating { horizon } => run_seating(&horizon, prefabs_dir, json),
        PrefabCommand::Anchors { pool } => run_anchors(pool.as_deref(), prefabs_dir, json),
        PrefabCommand::Audit {
            nbt,
            allowlist,
            report,
        } => run_audit(&nbt, allowlist.as_deref(), report.as_deref(), json),
        PrefabCommand::Socket {
            nbt,
            pos,
            facing,
            opening,
            name,
            target,
            pool,
        } => run_socket(
            &nbt,
            SocketArgs {
                pos,
                facing,
                opening,
                name,
                target,
                pool,
            },
            json,
        ),
        PrefabCommand::ResolveJigsaw { nbt } => run_resolve_jigsaw(&nbt, json),
        PrefabCommand::Anchor {
            nbt,
            name,
            pos,
            facing,
            region,
            block,
            role,
            no_role,
        } => run_anchor(
            &nbt,
            &name,
            AnchorArgs {
                pos,
                facing,
                region,
                block,
                role,
                no_role,
            },
            json,
        ),
        PrefabCommand::Planes { nbt, write } => run_planes(&nbt, write, json),
        PrefabCommand::Lighting {
            nbt,
            write,
            dark_threshold,
        } => run_lighting(&nbt, write, dark_threshold, json),
        PrefabCommand::Catalog { cmd } => match cmd {
            CatalogCmd::Validate { files } => run_catalog_validate(&files, json),
        },
        PrefabCommand::Gallery { dir, out, id, cols } => run_gallery(&dir, &out, id, cols, json),
        PrefabCommand::Curate { log, layout, out } => {
            run_curate(&log, &layout, out.as_deref(), json)
        }
        PrefabCommand::CurateMerge { report, catalog } => run_curate_merge(&report, &catalog, json),
    }
}

// -------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// The pairing (spec-0060 §6)
// ---------------------------------------------------------------------------

/// **`delvec prefab seating --horizon <base>`**: can this library stand on this
/// base, per pool, with the reason and the numbers.
///
/// Three properties are load-bearing and each is asserted rather than described:
///
/// 1. **It opens the bytes.** A verdict computed from declarations alone would
///    report a pool of waterline fictions as seatable, which is exactly the
///    green this command exists to end; the `.nbt`-opened count is printed
///    beside the document count and a run where the first is smaller than the
///    second exits non-zero.
/// 2. **It states a numerator and a denominator at every level** — pools of
///    pools, members of members, declarations borne out of declarations
///    examined — because a seating report with no denominator is the vacuity
///    `CLAUDE.md` forbids, and because a library that has lost a field must read
///    as a red rather than as a small number.
/// 3. **It is the same code the compiler runs.** One implementation, two entry
///    points, so a library can never be seatable according to the tool and
///    refused by the build.
///
/// Property 3 is the one this command was measured FAILING, and the failure is
/// worth stating because "same code" was true of the member checks and false of
/// the only question a pool has. Every member of `pool/island` and every member
/// of `pool/cave-shore` answered every per-piece question, so both printed
/// `SEATABLE` — and both were refused at build by `DW0886`, because the members'
/// declared walk planes disagree with one another and one origin cannot be
/// derived from two. That rule lived in `compiler::plan::area_base_y` alone.
/// [`crate::compiler::seating::set_walk_plane`] holds it now, and this command,
/// the validation check and the build derivation all read it.
///
/// Under `--json` the whole verdict is one object per line on stdout: the
/// per-pool verdicts, the reasons with their codes and shapes, and the binding
/// counts. The flag was accepted and ignored, which is worse than refusing it —
/// a caller that asked for machine output got a table it could not parse and no
/// signal that it had not been heard.
fn run_seating(horizon: &str, dir: &Path, json: bool) -> ExitCode {
    let base = match horizon {
        "void" => delvewright_dsl::HorizonBase::Void,
        "ocean" => delvewright_dsl::HorizonBase::Ocean,
        "valley" => delvewright_dsl::HorizonBase::Valley,
        other => {
            return input_err(
                &format!(
                    "unknown horizon base `{other}`. The bases this engine declares are the \
                     ones `delvec schema --stage world` exports: `void`, `ocean`, `valley`"
                ),
                json,
            );
        }
    };
    let registry = match crate::compiler::registry::PrefabRegistry::load_dir(dir) {
        Ok(r) => r,
        Err(e) => {
            return input_err(
                &format!("cannot read prefabs dir {}: {e}", dir.display()),
                json,
            );
        }
    };
    // A document this engine cannot parse is ABSENT from the registry, so a
    // sweep that did not say so would report a smaller library as a clean one.
    let unparsed = registry
        .load_diagnostics()
        .iter()
        .filter(|d| d.severity == delvewright_dsl::Severity::Error)
        .count();
    for d in registry.load_diagnostics() {
        if d.severity == delvewright_dsl::Severity::Error {
            eprintln!("{} [error] {}", d.code, d.message);
        }
    }
    let library = crate::compiler::seating::Library::read(&registry, dir);
    for line in &library.unreadable {
        Diagnostic::error(DW_INPUT, line.clone()).print(json);
    }

    let pools = registry.pool_ids();
    let mut members_total = 0usize;
    let mut pools_seatable = 0usize;
    let mut lines: Vec<String> = Vec::new();
    let mut pool_json: Vec<serde_json::Value> = Vec::new();
    for pool in &pools {
        let mut ids: Vec<String> = registry
            .pool(pool)
            .map(|m| m.iter().map(|m| m.prefab.clone()).collect())
            .unwrap_or_default();
        ids.sort();
        ids.dedup();
        members_total += ids.len();
        let mut seatable = 0usize;
        let mut reasons: Vec<String> = Vec::new();
        let mut reason_json: Vec<serde_json::Value> = Vec::new();
        let record = |about: &str, r: &crate::compiler::seating::Reason| {
            serde_json::json!({
                "about": about,
                "code": r.code.id(),
                "shape": format!("{:?}", r.shape),
                "short": r.short,
                "detail": r.full,
            })
        };
        for id in &ids {
            let Some(f) = library.pieces.get(id) else {
                reasons.push(format!(
                    "  {id:<28} no document or no readable `.nbt` in this library"
                ));
                reason_json.push(serde_json::json!({
                    "about": id,
                    "code": DW_INPUT,
                    "shape": "Unreadable",
                    "short": "no document or no readable `.nbt` in this library",
                    "detail": "no document or no readable `.nbt` in this library",
                }));
                continue;
            };
            let mut why = crate::compiler::seating::seating_reasons(base, f);
            if let Some(w) = crate::compiler::seating::waterline_reason(f) {
                why.push(w);
            }
            if why.is_empty() {
                seatable += 1;
                continue;
            }
            for r in why {
                let short = id.strip_prefix("prefab/").unwrap_or(id);
                reasons.push(format!("  {short:<28} {} ({})", r.short, r.code.id()));
                reason_json.push(record(id, &r));
            }
        }
        // **The question the pool has and no member of it can answer**: one
        // origin per area, derived from one walk plane. A pool every member of
        // which is individually perfect is refused here when they disagree, and
        // this is where the command stopped agreeing with the build.
        let declared: Vec<(String, Option<i32>)> = ids
            .iter()
            .map(|id| (id.clone(), library.pieces.get(id).and_then(|f| f.walk_y)))
            .collect();
        let mut set_refused = false;
        if let crate::compiler::seating::SetPlane::Refused(rs) =
            crate::compiler::seating::set_walk_plane(base, pool, &declared)
        {
            set_refused = true;
            for r in &rs {
                reasons.push(format!(
                    "  {:<28} {} ({})",
                    "(the pool)",
                    r.short,
                    r.code.id()
                ));
                reason_json.push(record(pool, r));
            }
        }
        let verdict = if seatable == ids.len() && !ids.is_empty() && !set_refused {
            pools_seatable += 1;
            "SEATABLE"
        } else {
            "REFUSED "
        };
        lines.push(format!(
            "{pool:<24} {verdict} {seatable} of {} member(s) seatable",
            ids.len()
        ));
        lines.extend(reasons);
        pool_json.push(serde_json::json!({
            "pool": pool,
            "verdict": verdict.trim(),
            "members": ids.len(),
            "members_seatable": seatable,
            "reasons": reason_json,
        }));
    }

    let (declared, borne_out) = library.waterline_census();
    let binding = format!(
        "seating binding: horizon base `{horizon}`; {pools_seatable} pool(s) seatable of \
         {pool_count} in this library, examined over {members_total} member(s); \
         {documents} document(s) read, {opened} `.nbt` opened; {declared} waterline \
         declaration(s) examined, {borne_out} borne out by the bytes.",
        pool_count = pools.len(),
        documents = library.documents,
        opened = library.nbt_opened,
    );
    if json {
        // One object, on stdout, carrying exactly what the table carries — the
        // per-pool verdicts, every reason with its code and its shape, and every
        // binding count. A `--shape` string rather than a substring of the
        // message, so a caller narrowing this set says which member of it it
        // means.
        let doc = serde_json::json!({
            "check": "seating",
            "horizon_base": horizon,
            "pools": pool_json,
            "binding": {
                "pools": pools.len(),
                "pools_seatable": pools_seatable,
                "members": members_total,
                "documents_read": library.documents,
                "nbt_opened": library.nbt_opened,
                "waterlines_declared": declared,
                "waterlines_borne_out": borne_out,
                "line": binding,
            },
        });
        println!(
            "{}",
            serde_json::to_string(&doc).expect("the verdict serializes")
        );
    } else {
        for line in &lines {
            println!("{line}");
        }
        println!("{binding}");
    }

    // The vacuity guard, stated over the objects rather than over an intention:
    // a run that examined no pool has judged nothing, and one that read fewer
    // `.nbt` than it read documents has judged a library it could not open.
    if pools.is_empty() {
        Diagnostic::error(
            DW_UNBOUND,
            format!(
                "the seating verdict examined ZERO pools in {} — a green verdict over an empty \
                 population is the unbound vacuity mode, not a pass. A prefab library declares \
                 its pools in `pools.json`",
                dir.display()
            ),
        )
        .print(json);
        return ExitCode::from(EXIT_FAIL);
    }
    if library.nbt_opened < library.documents || !library.unreadable.is_empty() || unparsed > 0 {
        Diagnostic::error(
            DW_UNBOUND,
            format!(
                "{} document(s) read ({unparsed} unparseable) but only {} `.nbt` opened: this \
                 verdict was computed from declarations for at least one piece, which is exactly \
                 the reading that reports a library of fictions as seatable",
                library.documents, library.nbt_opened
            ),
        )
        .print(json);
        return ExitCode::from(EXIT_FAIL);
    }
    if pools_seatable == pools.len() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(EXIT_FAIL)
    }
}

/// **`delvec prefab anchors`**: which anchors does a pool guarantee, answered
/// from the library alone.
///
/// The question is asked at the third authoring step, where the campaign that
/// would carry it does not exist yet, so this reads no campaign at all. What it
/// prints per pool is the guarantee and its denominator: the members, the
/// `entry` member every draw seats, the anchor names that member declares, and
/// every other name in the pool's vocabulary with the carrier it would have to
/// arrive on and the role that decides when the layout seats it. The verdict
/// comes from [`crate::compiler::guarantee`], which is the same implementation
/// the compiler's own `DW0889` reports from, so this command and the build
/// cannot describe different pools.
///
/// It is a REPORT, not a gate: a pool with a small guarantee is an ordinary
/// pool, and exiting non-zero over one would make the answer unaskable. The one
/// thing it does refuse is a question about nothing — a library with no pools,
/// or a `--pool` this library does not declare.
fn run_anchors(pool: Option<&str>, dir: &Path, json: bool) -> ExitCode {
    let registry = match crate::compiler::registry::PrefabRegistry::load_dir(dir) {
        Ok(r) => r,
        Err(e) => {
            return input_err(
                &format!("cannot read prefabs dir {}: {e}", dir.display()),
                json,
            );
        }
    };
    // A document this engine cannot parse is ABSENT from the registry, so a
    // report that did not say so would describe a smaller library as a whole one.
    let unparsed = registry
        .load_diagnostics()
        .iter()
        .filter(|d| d.severity == delvewright_dsl::Severity::Error)
        .count();
    for d in registry.load_diagnostics() {
        if d.severity == delvewright_dsl::Severity::Error {
            eprintln!("{} [error] {}", d.code, d.message);
        }
    }

    let all = registry.pool_ids();
    let pools: Vec<String> = match pool {
        Some(p) => {
            if !all.iter().any(|q| q == p) {
                return input_err(
                    &format!(
                        "this library declares no pool `{p}`. It declares {}: {}",
                        all.len(),
                        if all.is_empty() {
                            "none".to_string()
                        } else {
                            all.join(", ")
                        }
                    ),
                    json,
                );
            }
            vec![p.to_string()]
        }
        None => all.clone(),
    };

    let mut lines: Vec<String> = Vec::new();
    let mut pool_json: Vec<serde_json::Value> = Vec::new();
    let mut members_total = 0usize;
    let mut vocab_total = 0usize;
    let mut guaranteed_total = 0usize;
    for id in &pools {
        let Some(g) = crate::compiler::guarantee::pool_guarantee(&registry, id) else {
            continue;
        };
        members_total += g.members.len();
        vocab_total += g.vocabulary.len();
        guaranteed_total += g.unconditional.len();
        lines.extend(crate::compiler::guarantee::report_lines(&g));
        pool_json.push(crate::compiler::guarantee::report_json(&g));
    }

    let binding = format!(
        "anchor-guarantee binding: {n} of {total} pool(s) in this library reported, examined \
         over {members} member(s) declaring {vocab} anchor name(s), of which {guar} are \
         guaranteed by the layout; {unparsed} document(s) in this library did not parse.",
        n = pool_json.len(),
        total = all.len(),
        members = members_total,
        vocab = vocab_total,
        guar = guaranteed_total,
    );

    if json {
        let doc = serde_json::json!({
            "check": "anchors",
            "pools": pool_json,
            "binding": {
                "pools_in_library": all.len(),
                "pools_reported": pool_json.len(),
                "members": members_total,
                "vocabulary": vocab_total,
                "guaranteed": guaranteed_total,
                "documents_unparsed": unparsed,
                "line": binding,
            },
        });
        println!(
            "{}",
            serde_json::to_string(&doc).expect("the report serializes")
        );
    } else {
        for line in &lines {
            println!("{line}");
        }
        println!("{binding}");
    }

    // The vacuity guard, over the objects rather than over an intention: a run
    // that reported no pool has answered nothing.
    if pool_json.is_empty() {
        Diagnostic::error(
            DW_UNBOUND,
            format!(
                "the anchor-guarantee report examined ZERO pools in {} — an empty report is the \
                 unbound vacuity mode, not an answer. A prefab library declares its pools in \
                 `pools.json`",
                dir.display()
            ),
        )
        .print(json);
        return ExitCode::from(EXIT_FAIL);
    }
    ExitCode::SUCCESS
}

/// **`delvec prefab audit <library dir>`**: `DW0887` over every document in a
/// library, from the compiler's own implementation.
///
/// The per-file arm of this command answers a question about one piece's
/// palette; this arm answers the one question that needs the whole library at
/// once, because the defect it looks for is a declaration nobody ever checked
/// against the bytes beside it. The census is printed on every run, including
/// the run that finds nothing.
fn run_library_audit(dir: &Path, report: Option<&Path>, json: bool) -> ExitCode {
    let registry = match crate::compiler::registry::PrefabRegistry::load_dir(dir) {
        Ok(r) => r,
        Err(e) => {
            return input_err(
                &format!("cannot read prefabs dir {}: {e}", dir.display()),
                json,
            );
        }
    };
    let library = crate::compiler::seating::Library::read(&registry, dir);
    for line in &library.unreadable {
        Diagnostic::error(DW_INPUT, line.clone()).print(json);
    }
    let mut refused: Vec<serde_json::Value> = Vec::new();
    for facts in library.pieces.values() {
        if let Some(r) = crate::compiler::seating::waterline_reason(facts) {
            Diagnostic::error(r.code.id(), r.full.clone()).print(json);
            refused.push(serde_json::json!({
                "prefab_id": facts.id,
                "declared_waterline_y": facts.declared_waterline,
                "top_authored_water_y": facts.top_water_y,
                "water_cells": facts.water_cells,
                "reason": r.short,
            }));
        }
    }
    let (declared, borne_out) = library.waterline_census();
    let rep = serde_json::json!({
        "asset": dir.display().to_string(),
        "check": "waterline-in-the-bytes",
        "code": crate::compiler::seating::DW_WATERLINE_FICTION.id(),
        "verdict": if refused.is_empty() && library.unreadable.is_empty() { "pass" } else { "fail" },
        "documents_read": library.documents,
        "nbt_opened": library.nbt_opened,
        "waterlines_declared": declared,
        "waterlines_borne_out": borne_out,
        "waterlines_refused": refused.len(),
        "refused": refused,
    });
    let text = serde_json::to_string_pretty(&rep).expect("the report serializes") + "\n";
    match report {
        Some(p) => {
            if let Err(e) = std::fs::write(p, &text) {
                return output_err(&format!("write {}: {e}", p.display()), json);
            }
        }
        None => print!("{text}"),
    }
    eprintln!(
        "waterline audit binding: {documents} document(s) read, {opened} `.nbt` opened; \
         {declared} waterline declaration(s) examined, {borne_out} borne out by the bytes, \
         {refused} refused (DW0887).",
        documents = library.documents,
        opened = library.nbt_opened,
        refused = refused.len(),
    );
    if refused.is_empty() && library.unreadable.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(EXIT_FAIL)
    }
}

/// **`DW0887` at the admission event** — one asset, its own document, its own
/// bytes.
///
/// `meta_path` is the prefab document this asset is described by: the `.json`
/// beside a single template, and the manifest itself for a tile set (a manifest
/// IS the prefab document). `grid` is the assembled cells the arm already built,
/// so this opens nothing a second time and cannot disagree with what the rest of
/// the audit judged.
///
/// The rule itself is `compiler::seating::waterline_reason`, unchanged and
/// unduplicated: a declaration cannot be a fiction to the library sweep and a
/// fact to the per-file audit. What this function adds is the *binding* — the
/// state, the denominator and the numerator, printed on every run including the
/// run that finds nothing to check.
fn waterline_door(
    meta_path: &Path,
    grid: &crate::grammar::model::VoxelModel,
    nbt_opened: usize,
) -> (audit::WaterlineBinding, Option<Diagnostic>) {
    use crate::compiler::seating::{PieceFacts, waterline_reason};

    let meta = match delvewright_dsl::prefab::PrefabMeta::read(meta_path) {
        // No document beside the bytes yet: an ingested piece is audited before
        // its metadata exists, and there is nothing here to hold to anything.
        Ok(None) => return (audit::WaterlineBinding::no_document(), None),
        Ok(Some(m)) => m,
        // A document nobody can read is NOT a document with no waterline. The
        // same argument `DW0783` makes about the contract: the silence would
        // read as the pass.
        Err(e) => {
            return (
                audit::WaterlineBinding {
                    state: "unreadable",
                    declarations: 0,
                    borne_out: 0,
                    refused: 0,
                    nbt_opened,
                    top_authored_water_y: None,
                },
                Some(Diagnostic::error(
                    crate::compiler::seating::DW_WATERLINE_FICTION.id(),
                    format!(
                        "{}: this prefab document does not parse, so its `waterline_y` — if it \
                         declares one — could not be held to the bytes beside it. A document \
                         nobody can read is not a document with no waterline: {e}",
                        meta_path.display()
                    ),
                )),
            );
        }
    };
    let facts = PieceFacts::measure(&meta, grid, nbt_opened);
    let declarations = usize::from(facts.declared_waterline.is_some());
    let reason = waterline_reason(&facts);
    let refused = usize::from(reason.is_some());
    (
        audit::WaterlineBinding {
            state: if declarations == 0 {
                "undeclared"
            } else {
                "checked"
            },
            declarations,
            borne_out: declarations - refused,
            refused,
            nbt_opened,
            top_authored_water_y: facts.top_water_y,
        },
        reason.map(|r| Diagnostic::error(r.code.id(), r.full)),
    )
}

fn run_audit(nbt: &Path, allowlist: Option<&Path>, report: Option<&Path>, json: bool) -> ExitCode {
    // **A whole library sweeps rather than audits one file.** `DW0887` is a
    // property of a prefab document and its `.nbt` read together, so it binds
    // wherever those two are opened and in no other way — and the place they
    // are opened for every piece at once is a library. Same implementation as
    // the compiler's validation check (`compiler::seating`), because a
    // declaration cannot be a fiction to one reader and a fact to another.
    if nbt.is_dir() {
        return run_library_audit(nbt, report, json);
    }
    let allow = match allowlist {
        Some(p) => match std::fs::read_to_string(p)
            .map_err(|e| e.to_string())
            .and_then(|t| Allowlist::from_file(&t))
        {
            Ok(a) => a,
            Err(e) => return input_err(&format!("allowlist {}: {e}", p.display()), json),
        },
        None => Allowlist::default_building(),
    };

    // A tile-set manifest audits the whole zone. Handing this command one tile
    // of a set would audit a fragment and print `"verdict": "pass"` over it,
    // which is the failure mode this command exists to prevent one layer up.
    //
    // Both packagings do the same two things in the same order: build the grid
    // once, then open the spatial contract's second door on it (spec-0036 §1c)
    // against the document that declares the contract. The door is bound to
    // `audit` and not to a flag of its own — `audit` is what CI runs over the
    // prefab library and what the admission procedure runs on every piece — and
    // it is bound in EVERY arm, because the arm it was missing from is the one a
    // composed zone arrives through.
    let (mut rep, diags, door, footprint, waterline) =
        if nbt.extension().and_then(|s| s.to_str()) == Some("json") {
            let (set, tiles) = match read_zone(nbt) {
                Ok(pair) => pair,
                Err(e) => return input_err(&e, json),
            };
            let asset = nbt.display().to_string();
            let (rep, diags) = audit::audit_tile_set(&asset, set.size, &tiles, &allow);
            // The contract a manifest declares is zone-relative — its boxes and its
            // anchors are stated in the coordinates of the assembled building, not
            // of any tile — so the checker's two arguments exist at zone scale
            // exactly as they do for one template. Tiling is packaging.
            let grid = settling::zone_grid(set.size, &tiles);
            let door = Door::open(&grid, tiles.len(), nbt);
            let waterline = waterline_door(nbt, &grid, tiles.len());
            (rep, diags, door, audit::footprint_class(nbt), waterline)
        } else {
            // ...and pointing it at ONE tile of a set is refused. The verdict would
            // be correct about that file and would be read as a verdict about the
            // zone — a gate bound to a fifth of what it is believed to cover, which
            // is the shape that stays green for a year.
            if let Err(code) = refuse_fragment(
                nbt,
                "audit",
                "return a verdict over one file that reads as a verdict over the zone",
                json,
            ) {
                return code;
            }
            // Read and parsed ONCE, for both the palette audit and the door. When
            // the door had its own `if let Ok(bytes) = read(..)`, unreadable and
            // unparseable bytes were two more ways for it to fall through in
            // silence; sharing the bytes is what makes those two cases stop
            // existing rather than stop mattering.
            let bytes = match std::fs::read(nbt) {
                Ok(b) => b,
                Err(e) => return input_err(&format!("cannot read {}: {e}", nbt.display()), json),
            };
            let structure = match Structure::read(&bytes) {
                Ok(s) => s,
                Err(e) => return input_err(&format!("cannot parse {}: {e}", nbt.display()), json),
            };
            let (rep, diags) = audit(&nbt.display().to_string(), &structure, &allow);
            let meta_path = nbt.with_extension("json");
            let grid = crate::admit::spatial::grid(&structure);
            let door = Door::open(&grid, 1, &meta_path);
            let waterline = waterline_door(&meta_path, &grid, 1);
            (
                rep,
                diags,
                door,
                audit::footprint_class(&meta_path),
                waterline,
            )
        };
    for d in &diags {
        d.print(json);
    }
    for d in &door.diagnostics() {
        d.print(json);
    }
    // `DW0848` (spec-0050 §5), bound to `audit` for the reason the contract door
    // is: `audit` is what CI runs over the prefab library and what the admission
    // procedure runs on every piece, so a claim about what a piece is FOR cannot
    // enter the library unjudged. The binding line is stated whether or not
    // anything declared a class.
    if let Some(d) = &footprint.finding {
        d.print(json);
    }
    eprintln!("{}", footprint.line());
    // `DW0887` at the SAME event, and for the same reason. spec-0060 §5 says
    // this code binds "wherever a prefab document and its `.nbt` are read
    // together"; the only door that ran it was the whole-library sweep, and
    // nothing hands this command a library — the admission procedure and the
    // content repository's palette job both walk the pieces one file at a time.
    // Measured before this line existed: a fiction planted in a document passed
    // 39 audits of 39 with the code appearing zero times in their output.
    let (waterline, wl_finding) = waterline;
    if let Some(d) = &wl_finding {
        d.print(json);
    }
    eprintln!("{}", waterline.line(&rep.asset));
    let contract_failed = door.is_refusal() || footprint.is_refusal() || waterline.is_refusal();
    rep.record_waterline(waterline, wl_finding.as_ref());
    rep.record_contract_door(&door);
    let out_json = rep.to_json();
    if let Some(p) = report {
        if let Err(e) = write_file(p, out_json.as_bytes()) {
            return output_err(&format!("cannot write report {}: {e}", p.display()), json);
        }
    } else {
        print!("{out_json}");
    }
    if rep.is_pass() && !contract_failed {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(EXIT_FAIL)
    }
}

/// Read every tile a manifest names, in manifest order.
///
/// One reader for every command that takes a zone: the tiling is packaging, and
/// a second copy of "open the files the manifest names and check they are the
/// export it describes" is a second place for the two to disagree.
fn read_zone(manifest: &Path) -> Result<(TileSet, Vec<(TilePart, Structure)>), String> {
    let Some(set) = crate::schem::split::read_tile_set(manifest)? else {
        return Err(format!(
            "{} is a single-template prefab's metadata, not a tile-set manifest — pass the \
             `.nbt` beside it",
            manifest.display()
        ));
    };
    let dir = manifest.parent().unwrap_or(Path::new("."));
    let mut tiles = Vec::with_capacity(set.parts.len());
    for part in &set.parts {
        let path = dir.join(&part.file);
        let bytes =
            std::fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let structure =
            Structure::read(&bytes).map_err(|e| format!("cannot parse {}: {e}", path.display()))?;
        if structure.size != part.size {
            return Err(format!(
                "{}: the tile is {}x{}x{} but {} declares {}x{}x{} — the manifest and the tiles \
                 beside it are not the same export",
                path.display(),
                structure.size[0],
                structure.size[1],
                structure.size[2],
                manifest.display(),
                part.size[0],
                part.size[1],
                part.size[2]
            ));
        }
        tiles.push((part.clone(), structure));
    }
    Ok((set, tiles))
}

/// Refuse a path that is one tile of a tiled zone.
///
/// Bound at every entry point that takes a single `.nbt`, because a fragment
/// reaching any of them produces a confident answer about a building nobody
/// has: `verb` says what the command was about to do, `consequence` what the
/// answer would have been read as.
fn refuse_fragment(nbt: &Path, verb: &str, consequence: &str, json: bool) -> Result<(), ExitCode> {
    let evidence = match tile_evidence(nbt) {
        Ok(e) => e,
        Err(e) => return Err(input_err(&e, json)),
    };
    match fragment_refusal(nbt, &evidence, verb, consequence) {
        None => Ok(()),
        Some(message) => {
            Diagnostic::error(DW_FRAGMENT, message).print(json);
            Err(ExitCode::from(EXIT_INPUT))
        }
    }
}

struct SocketArgs {
    pos: String,
    facing: String,
    opening: String,
    name: String,
    target: String,
    pool: String,
}

fn run_socket(nbt: &Path, args: SocketArgs, json: bool) -> ExitCode {
    let pos = match parse_ivec3(&args.pos) {
        Some(p) => p,
        None => return input_err(&format!("bad --pos `{}` (want x,y,z)", args.pos), json),
    };
    let opening = match parse_ivec2(&args.opening) {
        Some(o) => o,
        None => {
            return input_err(
                &format!("bad --opening `{}` (want w,h)", args.opening),
                json,
            );
        }
    };
    let facing = args.facing;
    let (mut structure, mut meta) = match load_piece(nbt, json) {
        Ok(v) => v,
        Err(code) => return code,
    };
    let decl = SocketDecl {
        local_pos: pos,
        facing: facing.clone(),
        opening,
        name: args.name,
        target: args.target,
        pool: args.pool,
    };
    if let Err(e) = socket::carve(&mut structure, &mut meta, &decl) {
        Diagnostic::error(DW_TOOLING, e).print(json);
        return ExitCode::from(EXIT_FAIL);
    }
    if let Err(code) = write_piece(nbt, &structure, &meta, json) {
        return code;
    }
    eprintln!(
        "carved socket {} at {},{},{} facing {facing}",
        decl.name, pos[0], pos[1], pos[2]
    );
    ExitCode::SUCCESS
}

fn run_resolve_jigsaw(nbt: &Path, json: bool) -> ExitCode {
    if let Err(code) = refuse_fragment(
        nbt,
        "edit",
        "change one tile of a zone in isolation and leave the set inconsistent",
        json,
    ) {
        return code;
    }
    let bytes = match std::fs::read(nbt) {
        Ok(b) => b,
        Err(e) => return input_err(&format!("cannot read {}: {e}", nbt.display()), json),
    };
    let mut structure = match Structure::read(&bytes) {
        Ok(s) => s,
        Err(e) => return input_err(&format!("cannot parse {}: {e}", nbt.display()), json),
    };
    let resolved = crate::admit::jigsaw::resolve(&mut structure);
    for r in &resolved {
        Diagnostic::warning(DW_TOOLING, format!("resolved jigsaw -> `{}`", r.became))
            .at(r.pos)
            .print(json);
    }
    if resolved.is_empty() {
        eprintln!("no jigsaw markers to resolve");
        return ExitCode::SUCCESS;
    }
    if let Err(e) = write_file(nbt, &structure.write()) {
        return output_err(&format!("cannot write {}: {e}", nbt.display()), json);
    }
    eprintln!("resolved {} jigsaw marker(s)", resolved.len());
    ExitCode::SUCCESS
}

/// What `anchor` was told, in the shape the parser produced it.
struct AnchorArgs {
    pos: Option<String>,
    facing: Option<String>,
    region: Option<String>,
    block: Option<String>,
    role: Option<String>,
    no_role: bool,
}

fn run_anchor(nbt: &Path, name: &str, args: AnchorArgs, json: bool) -> ExitCode {
    let AnchorArgs {
        pos,
        facing,
        region,
        block,
        role,
        no_role,
    } = args;
    let (_structure, mut meta) = match load_piece(nbt, json) {
        Ok(v) => v,
        Err(code) => return code,
    };
    let region = match region {
        Some(r) => match parse_region(&r) {
            Some(reg) => Some(reg),
            None => {
                return input_err(
                    &format!("bad --region `{r}` (want x1,y1,z1:x2,y2,z2)"),
                    json,
                );
            }
        },
        None => None,
    };
    let pos = match pos {
        Some(p) => match parse_ivec3(&p) {
            Some(v) => Some(v),
            None => return input_err(&format!("bad --pos `{p}` (want x,y,z)"), json),
        },
        None => None,
    };
    if pos.is_none() && region.is_none() {
        return input_err("anchor needs --pos or --region", json);
    }
    // **The role is refused HERE, where it is typed**, against the engine's own
    // closed vocabulary rather than against a copy of it: a term this engine
    // does not know written through into the document would be `DW0346` at the
    // next build, about a file the operator would then have to go and edit.
    let role: Option<Option<AnchorRole>> = match (role, no_role) {
        (Some(r), _) => match r.parse::<AnchorRole>() {
            Ok(role) => Some(Some(role)),
            Err(e) => return input_err(&format!("--role: {e}"), json),
        },
        (None, true) => Some(None),
        (None, false) => None,
    };
    // This command declares two things: where the anchor is, and what it is for.
    // Which contract element it lands in is resolved by the exporter from the
    // piece's own contract, and the dispenser cell and trigger block are hardware
    // the prefab wired — none of that is something the operator types, so none of
    // it is this edit's to write, and re-annotating an anchor that already exists
    // keeps all of it (`PrefabMeta::edit_anchor`).
    meta.edit_anchor(
        name,
        AnchorEdit {
            pos,
            facing,
            region: region.map(|(from, to)| Region { from, to }),
            block,
            role,
        },
    );
    if let Err(e) = write_meta(nbt, &meta) {
        return output_err(&format!("cannot write metadata: {e}"), json);
    }
    match role {
        Some(Some(r)) => eprintln!("annotated anchor {name} (role {r})"),
        Some(None) => eprintln!("annotated anchor {name} (no role)"),
        None => eprintln!("annotated anchor {name}"),
    }
    ExitCode::SUCCESS
}

/// **`delvec prefab planes <asset> [--write]`** — the piece's own walk plane and
/// waterline, measured off its bytes (spec-0060 §4).
///
/// # Why this verb exists
///
/// `walk_y` has no default and is not optional on a base that derives an origin
/// from it, and `waterline_y` is a claim `DW0887` holds to the bytes. Every
/// generator writes both by reading them back out of the blocks it just laid.
/// A piece **no generator wrote** — an ingested hero asset, a hand-authored room
/// — had no way to state either except by hand, and a census derivable from the
/// object is never hand-written. Measured on the shipped content library at the
/// time this landed: five documents declare no `walk_y`, and all five are pieces
/// with no generator (`hello-room` and the four `hero-*`). None of them is in a
/// pool, and all five can be seated directly by `areas[].prefab`, which is the
/// same derivation.
///
/// # It is the same rules, not a fourth reading of them
///
/// The walk plane is `schem::nav::standable_cells`'s lowest plane — the rule the
/// seating derivation, the generators' `prefab_invariants::walkplane` and this
/// verb all mean — and the waterline is `compiler::seating::PieceFacts`'s top
/// authored water block, which is the number `DW0887` checks a declaration
/// against. So a document this verb writes is a document that check passes, by
/// construction rather than by agreement.
///
/// A zone that ships as a tile set is measured as ONE assembled building, for
/// the reason `lighting` and `audit` do: a fifth of a building's walk plane is
/// not the building's, and answering confidently about it is the shape those two
/// commands already refuse.
fn run_planes(input: &Path, write: bool, json: bool) -> ExitCode {
    let (meta_path, grid, opened) = if input.extension().and_then(|s| s.to_str()) == Some("json") {
        match read_zone(input) {
            Ok((set, tiles)) => {
                let n = tiles.len();
                (
                    input.to_path_buf(),
                    settling::zone_grid(set.size, &tiles),
                    n,
                )
            }
            Err(e) => return input_err(&e, json),
        }
    } else {
        if let Err(code) = refuse_fragment(
            input,
            "measure",
            "report one tile's lowest floor as the building's walk plane",
            json,
        ) {
            return code;
        }
        let bytes = match std::fs::read(input) {
            Ok(b) => b,
            Err(e) => return input_err(&format!("cannot read {}: {e}", input.display()), json),
        };
        let structure = match Structure::read(&bytes) {
            Ok(s) => s,
            Err(e) => return input_err(&format!("cannot parse {}: {e}", input.display()), json),
        };
        (
            input.with_extension("json"),
            crate::admit::spatial::grid(&structure),
            1,
        )
    };

    let standable = crate::schem::nav::standable_cells(&grid);
    let walk_y = standable.iter().map(|c| c[1]).min();
    let cells_at_walk = walk_y.map_or(0, |w| standable.iter().filter(|c| c[1] == w).count());
    let mut water_cells = 0usize;
    let mut waterline_y: Option<i32> = None;
    for pos in grid.region().positions() {
        let Some(state) = grid.get(pos) else { continue };
        if state.name == "minecraft:water" {
            water_cells += 1;
            waterline_y = Some(waterline_y.map_or(pos[1], |t: i32| t.max(pos[1])));
        }
    }

    // The report states the DENOMINATOR beside each number: a walk plane is the
    // lowest standable plane, so one stray cell one course down is the whole
    // answer, and the count of cells standing on the plane is what says whether
    // the number is a floor or a tuft of grass in the sea.
    let report = serde_json::json!({
        "asset": input.display().to_string(),
        "nbt_opened": opened,
        "walk_y": walk_y,
        "waterline_y": waterline_y,
        "binding": {
            "standable_cells": standable.len(),
            "cells_on_the_walk_plane": cells_at_walk,
            "water_cells": water_cells,
        },
    });
    println!("{}", serde_json::to_string_pretty(&report).unwrap());

    // A piece with no standable cell anywhere has no walk plane, and writing
    // some number for it would be inventing the measurement. The generators
    // panic here; a command refuses and says which count was zero.
    let Some(w) = walk_y else {
        Diagnostic::error(
            DW_UNBOUND,
            format!(
                "{}: no standable cell anywhere in {} cell(s) of extent {:?}, so this piece has \
                 no walk plane to declare and none was invented. A body's feet need a cell that \
                 passes a body, open above, over a block that supports one; a piece that offers \
                 none is solid, flooded, or floored in something a body falls through",
                input.display(),
                grid.region().positions().count(),
                grid.region().size,
            ),
        )
        .print(json);
        return ExitCode::from(EXIT_FAIL);
    };

    if write {
        // The same refusal `lighting --write` makes, and for the same reason: a
        // skeleton written here would assert `source: unknown` about a piece
        // whose provenance is in the file beside it.
        let mut doc = match PrefabMeta::read(&meta_path) {
            Ok(Some(d)) => d,
            Ok(None) => return no_provenance_err(input, &meta_path, json),
            Err(e) => return input_err(&e, json),
        };
        doc.walk_y = Some(w);
        // A piece that authors no water writes NO key: a waterline over no water
        // is `DW0887`, and leaving a stale one behind would manufacture one.
        doc.waterline_y = waterline_y;
        if let Err(e) = write_file(&meta_path, doc.to_json().as_bytes()) {
            return output_err(&format!("cannot write {}: {e}", meta_path.display()), json);
        }
        eprintln!(
            "planes binding: wrote `walk_y: {w}` ({cells_at_walk} cell(s) stand on that plane, of \
             {total} standable) and {wl} into {path}",
            total = standable.len(),
            wl = match waterline_y {
                Some(y) => format!("`waterline_y: {y}` ({water_cells} water cell(s))"),
                None => "no `waterline_y` (this piece authors no water)".to_string(),
            },
            path = meta_path.display(),
        );
    }
    ExitCode::SUCCESS
}

fn run_lighting(input: &Path, write: bool, dark_threshold: i32, json: bool) -> ExitCode {
    // A zone that ships as a tile set is one building, so it is probed as one:
    // its manifest is a first-class input, and light crosses a packaging plane
    // exactly as it crosses any other cell. Handing this command one tile is
    // refused for the reason `audit` refuses it.
    let (meta_path, size, tiles) = if input.extension().and_then(|s| s.to_str()) == Some("json") {
        match read_zone(input) {
            Ok((set, tiles)) => (input.to_path_buf(), set.size, tiles),
            Err(e) => return input_err(&e, json),
        }
    } else {
        if let Err(code) = refuse_fragment(
            input,
            "probe",
            "measure a fifth of a building and report the answer as the building's",
            json,
        ) {
            return code;
        }
        let bytes = match std::fs::read(input) {
            Ok(b) => b,
            Err(e) => return input_err(&format!("cannot read {}: {e}", input.display()), json),
        };
        let structure = match Structure::read(&bytes) {
            Ok(s) => s,
            Err(e) => return input_err(&format!("cannot parse {}: {e}", input.display()), json),
        };
        let size = structure.size;
        let part = TilePart {
            file: input
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string(),
            id: String::new(),
            grid_index: [0, 0, 0],
            offset: [0, 0, 0],
            size,
        };
        (input.with_extension("json"), size, vec![(part, structure)])
    };

    // **Which sky this piece stands under is the piece's own claim, read before
    // anything is measured** (`light::SkyClaim`). A detail piece is walked under
    // the whole's roof and never meets the sky; measured as if it stood in open
    // air it reports the night floor at every cell and is written `lit`, a
    // profile true in no world it will be placed in.
    //
    // Read here rather than inside the probe because the claim lives in the
    // metadata document and the probe is handed blocks. A document that is
    // absent or unreadable is not a claim of enclosure — the probe falls back to
    // open air, exactly as it does for the contractless kit pieces — and
    // `--write` refuses on its own terms further down, where it can say why.
    let meta = PrefabMeta::read(&meta_path).ok().flatten();
    let sky = light::SkyClaim::of(meta.as_ref().and_then(|m| m.spatial_contract.as_ref()));

    let zone = Zone::from_tiles(size, &tiles);
    let probe = light::probe(&zone, dark_threshold, sky);

    // The machine-readable line states the BINDING and the SKY, not only the
    // verdict: a minimum with no count beside it cannot be read afterwards, and
    // one with no sky beside it is not a light level at all — the same floor is
    // bright at noon and black at midnight.
    let report = serde_json::json!({
        "asset": input.display().to_string(),
        "files": tiles.len(),
        "size": size,
        "profile": probe.profile,
        "measured_min_light": probe.measured_min_light,
        "min_light_daylight": probe.min_light_daylight,
        "darkest_cell": probe.darkest_cell,
        "dark_threshold": probe.dark_threshold,
        // **The distribution, not the minimum alone.** One cell at light 0 in
        // the lee of a pillar and a room where every cell is at light 0 report
        // the same `measured_min_light`, and they are a detail and a room
        // nobody can see in. `cells_by_light` counts the measured floor at each
        // level below the threshold; `cells` and `fraction` say how much of the
        // room that is.
        "dark_cells": {
            "cells_by_light": probe.dark_by_level.iter()
                .map(|(level, count)| (level.to_string(), *count))
                .collect::<BTreeMap<String, usize>>(),
            "cells": probe.dark_cells(),
            "fraction": probe.dark_fraction(),
        },
        "assumed_sky": {
            "profile_taken_at": probe.sky_light,
            "daylight": probe.daylight_sky_light,
            "admits_sky": probe.sky.admits_sky(),
            "why": probe.sky.why(),
        },
        "binding": {
            "standable_cells": probe.standable_cells,
            "entry_cells": probe.entry_cells,
            "measured_cells": probe.measured_cells,
        },
    });
    println!("{}", serde_json::to_string_pretty(&report).unwrap());

    // A binding of zero is a FINDING, never a pass. It is also the one way a
    // genuinely pitch-black piece could slip past this probe — a sealed crypt
    // has no entrance, binds nothing, and would otherwise report "not dark".
    if probe.is_unbound() {
        Diagnostic::error(
            DW_UNBOUND,
            format!(
                "the light probe bound to ZERO cells, so nothing was measured: {}",
                probe.unbound_reason()
            ),
        )
        .print(json);
        return ExitCode::from(EXIT_FAIL);
    }
    if probe.is_dark() {
        let cell = probe
            .darkest_cell
            .map(|c| format!("; darkest at {},{},{}", c[0], c[1], c[2]))
            .unwrap_or_default();
        // "Lit by day" is a sentence about a piece the sky reaches. An enclosed
        // piece meets no sky at either end of the table, so the second figure is
        // the first one again and offering it as a consolation would be false.
        let by_day = if !probe.sky.admits_sky() {
            String::new()
        } else {
            match probe.min_light_daylight {
                Some(d) if d >= dark_threshold => format!(
                    "; by day it is {} — this is a piece the sky reaches, and it needs a light \
                     only where the delve reaches night",
                    d
                ),
                Some(d) => format!("; still {d} under full daylight"),
                None => String::new(),
            }
        };
        let under = if probe.sky.admits_sky() {
            format!(
                "at sky light {} (a clear night, the darkest the engine models)",
                probe.sky_light
            )
        } else {
            format!("with no sky ({})", probe.sky.why())
        };
        // **The distribution, and the minimum only as the place to start.** One
        // cell at light 0 behind a pillar and a room where every cell is at
        // light 0 used to print the same sentence, and they are a detail and a
        // room nobody can see in. The repair is to re-arrange the room or raise
        // the density of what is already lighting it, and neither is a decision
        // a reader can take from one number.
        Diagnostic::warning(
            DW_DARK,
            format!(
                "dark interior {under}: {dark} of {measured} floor cell(s) a player can walk to \
                 ({pct:.1}%) are below light {threshold} — {distribution}{cell}{by_day}",
                dark = probe.dark_cells(),
                measured = probe.measured_cells,
                pct = probe.dark_fraction() * 100.0,
                threshold = dark_threshold,
                distribution = probe.dark_distribution(),
            ),
        )
        .print(json);
    }

    if write {
        // A tool that cannot establish where a piece came from REFUSES; it never
        // invents. Writing a skeleton here manufactured `source: unknown`,
        // `spdx: UNKNOWN` and no provenance row — a document asserting that
        // nothing is known about an asset whose provenance is sitting in the
        // file next to it, and asserting it silently.
        let mut doc = match PrefabMeta::read(&meta_path) {
            Ok(Some(d)) => d,
            Ok(None) => {
                return no_provenance_err(input, &meta_path, json);
            }
            Err(e) => return input_err(&e, json),
        };
        meta::set_lighting_from_probe(&mut doc, &probe);
        if let Err(e) = write_file(&meta_path, doc.to_json().as_bytes()) {
            return output_err(&format!("cannot write {}: {e}", meta_path.display()), json);
        }
        eprintln!(
            "wrote lighting profile `{}` (bound to {} cell(s)) into {}",
            probe.profile,
            probe.measured_cells,
            meta_path.display()
        );
    }
    ExitCode::SUCCESS
}

/// `--write` with nothing to write into: refuse, and say what to do.
fn no_provenance_err(input: &Path, meta_path: &Path, json: bool) -> ExitCode {
    Diagnostic::error(
        DW_NO_PROVENANCE,
        format!(
            "there is no prefab metadata at {} to write the measurement into, and this tool will \
             not invent one: a skeleton it wrote would claim `source: unknown`, `spdx: UNKNOWN` \
             and no provenance row about a piece whose licence and origin it has not established. \
             Create the metadata beside {} first (the generators and `delvec grammar export` write \
             it; for an ingested piece, `delvec prefab anchor`/`socket` start one), then re-run with \
             --write. Without --write the measurement is still printed above.",
            meta_path.display(),
            input.display()
        ),
    )
    .print(json);
    ExitCode::from(EXIT_INPUT)
}

fn run_catalog_validate(files: &[PathBuf], json: bool) -> ExitCode {
    if files.is_empty() {
        return input_err("no catalog card files given", json);
    }
    let mut ok = true;
    for f in files {
        let text = match std::fs::read_to_string(f) {
            Ok(t) => t,
            Err(e) => {
                Diagnostic::error(DW_INPUT, format!("cannot read {}: {e}", f.display()))
                    .print(json);
                ok = false;
                continue;
            }
        };
        match CatalogCard::from_json(&text) {
            Ok(card) => {
                let diags = card.validate();
                for d in &diags {
                    d.print(json);
                }
                if diags.iter().any(|d| d.is_error()) {
                    ok = false;
                } else {
                    eprintln!("{}: valid", f.display());
                }
            }
            Err(e) => {
                Diagnostic::error(DW_INPUT, format!("{}: {e}", f.display())).print(json);
                ok = false;
            }
        }
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(EXIT_FAIL)
    }
}

fn run_gallery(dir: &Path, out: &Path, id: Option<String>, cols: usize, json: bool) -> ExitCode {
    let gallery_id = id.unwrap_or_else(|| {
        dir.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("gallery")
            .to_string()
    });
    let mut nbts: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("nbt"))
            .collect(),
        Err(e) => return input_err(&format!("cannot read {}: {e}", dir.display()), json),
    };
    nbts.sort();
    if nbts.is_empty() {
        return input_err(&format!("no .nbt candidates in {}", dir.display()), json);
    }
    let mut cands: Vec<Candidate> = Vec::new();
    for p in &nbts {
        // The door nobody would point at deliberately: walking `*.nbt` in a
        // directory that holds a tile set puts each tile on a plinth as if it
        // were a prefab, and a reviewer walks past five slices of one building
        // believing they reviewed five pieces.
        if let Err(code) = refuse_fragment(
            p,
            "show",
            "put one slice of a building on a plinth as if it were a piece",
            json,
        ) {
            return code;
        }
        let bytes = match std::fs::read(p) {
            Ok(b) => b,
            Err(e) => return input_err(&format!("cannot read {}: {e}", p.display()), json),
        };
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("piece");
        let asset_id = match PrefabMeta::beside_nbt(p) {
            Ok(Some(m)) => m.prefab_id.trim_start_matches("prefab/").to_string(),
            Ok(None) => stem.to_string(),
            Err(e) => return input_err(&e, json),
        };
        match Candidate::from_nbt(&asset_id, stem, bytes) {
            Ok(c) => cands.push(c),
            Err(e) => return input_err(&format!("{}: {e}", p.display()), json),
        }
    }
    // Emission validates every line it wrote against the pinned 1.21.11 command
    // tree, so a gallery that the server would refuse to load is never written
    // at all. One rejected line costs the whole function it sits in.
    let tree = match gallery::emit(&gallery_id, &cands, cols) {
        Ok(t) => t,
        Err(errors) => {
            for e in &errors {
                Diagnostic::error(
                    DW_GALLERY,
                    format!(
                        "emitted command is not valid on Minecraft {}: `{}` — {}",
                        crate::compiler::MC_VERSION,
                        e.line.trim(),
                        e.reason
                    ),
                )
                .print(json);
            }
            return ExitCode::from(EXIT_OUTPUT);
        }
    };
    if let Err(e) = write_tree(out, &tree) {
        Diagnostic::error(DW_GALLERY, format!("cannot write gallery: {e}")).print(json);
        return ExitCode::from(EXIT_OUTPUT);
    }
    eprintln!(
        "gallery `{gallery_id}`: {} pieces -> {}",
        cands.len(),
        out.display()
    );
    ExitCode::SUCCESS
}

fn run_curate(log: &Path, layout: &Path, out: Option<&Path>, json: bool) -> ExitCode {
    let log_text = match std::fs::read_to_string(log) {
        Ok(t) => t,
        Err(e) => return input_err(&format!("cannot read {}: {e}", log.display()), json),
    };
    let layout_text = match std::fs::read_to_string(layout) {
        Ok(t) => t,
        Err(e) => return input_err(&format!("cannot read {}: {e}", layout.display()), json),
    };
    let report = match gallery::curate(&log_text, &layout_text) {
        Ok(r) => r,
        Err(e) => return input_err(&e, json),
    };
    let text = report.to_json();
    if let Some(p) = out {
        if let Err(e) = write_file(p, text.as_bytes()) {
            return output_err(&format!("cannot write {}: {e}", p.display()), json);
        }
    } else {
        print!("{text}");
    }
    ExitCode::SUCCESS
}

fn run_curate_merge(report: &Path, catalog: &Path, json: bool) -> ExitCode {
    let text = match std::fs::read_to_string(report) {
        Ok(t) => t,
        Err(e) => return input_err(&format!("cannot read {}: {e}", report.display()), json),
    };
    let value: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => return input_err(&format!("bad curation report: {e}"), json),
    };
    let assets = match value.get("assets").and_then(|a| a.as_object()) {
        Some(a) => a,
        None => return input_err("curation report has no `assets` object", json),
    };
    let mut merged = 0usize;
    for (asset_id, notes_val) in assets {
        let notes: Vec<crate::admit::catalog::CurationNote> =
            match serde_json::from_value(notes_val.clone()) {
                Ok(n) => n,
                Err(e) => return input_err(&format!("asset {asset_id}: {e}"), json),
            };
        let card_path = catalog.join(format!("{}.json", gallery::sanitize(asset_id)));
        if !card_path.exists() {
            Diagnostic::warning(
                DW_TOOLING,
                format!(
                    "no catalog card for `{asset_id}` at {}",
                    card_path.display()
                ),
            )
            .print(json);
            continue;
        }
        let card_text = match std::fs::read_to_string(&card_path) {
            Ok(t) => t,
            Err(e) => return input_err(&format!("{}: {e}", card_path.display()), json),
        };
        let mut card = match CatalogCard::from_json(&card_text) {
            Ok(c) => c,
            Err(e) => return input_err(&format!("{}: {e}", card_path.display()), json),
        };
        card.curation = Some(gallery::merge_into_card(&notes, card.curation.take()));
        if let Err(e) = write_file(&card_path, card.to_json().as_bytes()) {
            return output_err(&format!("{}: {e}", card_path.display()), json);
        }
        merged += 1;
    }
    eprintln!("merged curation notes into {merged} catalog card(s)");
    ExitCode::SUCCESS
}

// -------------------------------------------------------------------------
// shared helpers
// -------------------------------------------------------------------------

/// Load a piece's structure + metadata (creating a skeleton when metadata is
/// absent, with a warning — admission steps are chainable on a fresh piece).
fn load_piece(nbt: &Path, json: bool) -> Result<(Structure, PrefabMeta), ExitCode> {
    refuse_fragment(
        nbt,
        "edit",
        "change one tile of a zone in isolation and leave the set inconsistent",
        json,
    )?;
    let bytes = std::fs::read(nbt)
        .map_err(|e| input_err(&format!("cannot read {}: {e}", nbt.display()), json))?;
    let structure = Structure::read(&bytes)
        .map_err(|e| input_err(&format!("cannot parse {}: {e}", nbt.display()), json))?;
    let meta = match PrefabMeta::beside_nbt(nbt) {
        Ok(Some(m)) => m,
        Ok(None) => {
            Diagnostic::warning(
                DW_TOOLING,
                "no sibling metadata; created a skeleton — set license before admission",
            )
            .print(json);
            skeleton_for(nbt, &structure)
        }
        Err(e) => return Err(input_err(&e, json)),
    };
    Ok((structure, meta))
}

fn skeleton_for(nbt: &Path, structure: &Structure) -> PrefabMeta {
    let id = nbt
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("piece")
        .to_string();
    PrefabMeta::skeleton(
        &id,
        structure.size,
        structure.data_version,
        "delvec prefab (external admission)",
        License {
            source: "unknown".to_string(),
            spdx: "UNKNOWN".to_string(),
            note: "set at admission — see catalog card".to_string(),
            provenance: "external admission via delvec prefab".to_string(),
            // Nothing regenerates an ingested piece: there is no program and no
            // seed, so the row is absent rather than invented.
            generated_by: None,
        },
    )
}

fn write_piece(
    nbt: &Path,
    structure: &Structure,
    meta: &PrefabMeta,
    json: bool,
) -> Result<(), ExitCode> {
    write_file(nbt, &structure.write())
        .map_err(|e| output_err(&format!("cannot write {}: {e}", nbt.display()), json))?;
    write_meta(nbt, meta).map_err(|e| output_err(&format!("cannot write metadata: {e}"), json))?;
    Ok(())
}

fn write_meta(nbt: &Path, meta: &PrefabMeta) -> std::io::Result<()> {
    let json_path = nbt.with_extension("json");
    std::fs::write(json_path, meta.to_json())
}

fn write_file(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, data)
}

fn write_tree(root: &Path, tree: &BTreeMap<String, Vec<u8>>) -> std::io::Result<()> {
    for (rel, bytes) in tree {
        write_file(&root.join(rel), bytes)?;
    }
    Ok(())
}

fn parse_ivec3(s: &str) -> Option<[i32; 3]> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 3 {
        return None;
    }
    Some([
        parts[0].trim().parse().ok()?,
        parts[1].trim().parse().ok()?,
        parts[2].trim().parse().ok()?,
    ])
}

fn parse_ivec2(s: &str) -> Option<[i32; 2]> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 2 {
        return None;
    }
    Some([parts[0].trim().parse().ok()?, parts[1].trim().parse().ok()?])
}

fn parse_region(s: &str) -> Option<([i32; 3], [i32; 3])> {
    let (a, b) = s.split_once(':')?;
    Some((parse_ivec3(a)?, parse_ivec3(b)?))
}

fn input_err(msg: &str, json: bool) -> ExitCode {
    Diagnostic::error(DW_INPUT, msg).print(json);
    ExitCode::from(EXIT_INPUT)
}

fn output_err(msg: &str, json: bool) -> ExitCode {
    Diagnostic::error(DW_INPUT, msg).print(json);
    ExitCode::from(EXIT_OUTPUT)
}
