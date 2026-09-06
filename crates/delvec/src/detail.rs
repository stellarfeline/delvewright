//! `delvec detail` — one verb details a place inside the allocation the whole
//! handed (spec-0058).
//!
//! Stage 6 used to be a chain the creator re-typed by hand: read the allocation,
//! copy the frame into `--region`, the seam cells into `--param`s, the ids into
//! `--id` and into a hand-written `details[]` row, expand, move a file, audit,
//! light. Every procedural input was typed at least once and eight of them
//! twice, with nothing comparing the copies until `DW0843`/`DW0844` refused
//! after the piece existed.
//!
//! This verb takes the campaign and the place. Everything else is derived: the
//! allocation is computed from the site plan (the same function `delvec
//! allocation` prints from), bound into the program under the `handed/`
//! parameter prefix, expanded at the frame, judged by every gate the chain ran —
//! **before any file is written** — then frozen into the prefab directory with
//! its row written into `detail-plan.json`. `--all` re-details every place that
//! has a program, in site-plan order, so a plan edit is answered by one command
//! and no creator input.
//!
//! The dependency direction of spec-0050 §10.3 holds: the compiler library
//! reads a frozen piece and nothing of the program; this binary composes the
//! compiler's allocation and bindings check with the grammar's expansion and
//! the admission crate's audit and light probe.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use delvewright_admit::allowlist::Allowlist;
use delvewright_admit::audit;
use delvewright_admit::light::{self, DEFAULT_DARK_THRESHOLD, Zone};
use delvewright_admit::meta as admit_meta;
use delvewright_admit::structure::Structure;
use delvewright_compiler::detail::{self as engine, Allocation};
use delvewright_compiler::registry::{PrefabRegistry, REPORT_SUFFIX};
use delvewright_dsl::detailplan::{Detail, DetailPlanContent};
use delvewright_dsl::prefab::PrefabMeta;
use delvewright_dsl::split::TilePart;
use delvewright_dsl::{
    Campaign, Diagnostic, DwCode, Envelope, ExitTier, Fenced, NodeId, PrefabId, Stage,
    parse_campaign,
};
use delvewright_grammar::cli::{composition_to_stderr, report_to_stderr};
use delvewright_grammar::ir::Paint;
use delvewright_grammar::{BlockState, Box3, ExpandOptions, Overrides, document, expand, export, gates};
use sha2::{Digest, Sha256};

use crate::{
    EXIT_INTERNAL, has_error, load_or_refuse, print_build_error, print_diags, print_one_diag,
    read_skins, read_structures, validate_stage,
};

/// `DW0882`: **the program asks for a value the whole does not hand.** A
/// parameter declared under the `handed/` prefix names a seam this place does
/// not have, or a name the handing does not use. Refused where entered — at
/// `detail`, before the program is expanded — naming the parameter and every
/// name the allocation hands this place, so the repair is a rename in the
/// program and never a number.
const DW_NOT_HANDED: DwCode = DwCode::every_version("DW0882", ExitTier::Build);

/// Where a campaign keeps its detail programs: `<campaign>/programs/<place
/// stem>.json`. The address is derived from the place, as the piece id is; the
/// document at it is the creator's.
pub const PROGRAMS_DIR: &str = "programs";

/// The prefix a program reads a handed value through (spec-0058 §2.3).
pub const HANDED_PREFIX: &str = "handed/";

/// The stage the row is written into.
const STAGE: &str = "detail-plan";

/// `delvec detail <campaign-dir> <place>` / `--all`.
pub(crate) fn run_detail(
    campaign_dir: &Path,
    place: Option<&str>,
    all: bool,
    prefabs_dir: &Path,
    lang: &str,
    json: bool,
) -> ExitCode {
    match run(campaign_dir, place, all, prefabs_dir, lang, json) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => ExitCode::from(code),
    }
}

fn run(
    campaign_dir: &Path,
    place: Option<&str>,
    all: bool,
    prefabs_dir: &Path,
    lang: &str,
    json: bool,
) -> Result<(), u8> {
    let loaded = load_or_refuse(campaign_dir, json)?;
    // Parsed rather than fully validated, on `allocation`'s precedent: the
    // campaign is validated in full at the end of the run, over the pieces this
    // run wrote, which is the verdict that matters.
    let campaign = match parse_campaign(&loaded.raw) {
        Ok(c) => c,
        Err(diags) => {
            print_diags(&Fenced::structural(diags), json);
            return Err(1);
        }
    };
    if campaign.site_plan.is_none() {
        eprintln!(
            "error: `{}` carries no `site-plan.json`. A place is detailed inside the box the \
             WHOLE gave it, so there is nothing to detail until the whole exists.",
            campaign_dir.display()
        );
        return Err(1);
    }
    // **The walk gate, before the program is opened.** The same `DW0841`
    // `allocation` and validation raise: detail work begins here too.
    if let Some(d) = engine::allocation_walk_gate(&campaign, loaded.walk_record.as_deref()) {
        print_one_diag(&d, json);
        return Err(1);
    }

    let targets = targets(campaign_dir, &campaign, place, all)?;

    if let Err(e) = std::fs::create_dir_all(prefabs_dir) {
        eprintln!(
            "internal error: cannot create prefabs dir {}: {e}",
            prefabs_dir.display()
        );
        return Err(EXIT_INTERNAL);
    }
    let library = match PrefabRegistry::load_dir(prefabs_dir) {
        Ok(r) => r,
        Err(e) => {
            eprintln!(
                "internal error: cannot read prefabs dir {}: {e}",
                prefabs_dir.display()
            );
            return Err(EXIT_INTERNAL);
        }
    };

    let mut done: Vec<String> = Vec::new();
    for node in &targets {
        let written = detail_one(
            campaign_dir,
            &campaign,
            loaded.walk_record.as_deref(),
            &library,
            prefabs_dir,
            node,
            json,
        )?;
        done.push(written);
    }
    eprintln!(
        "detail: {} place(s) detailed of {} named — {}",
        done.len(),
        targets.len(),
        done.join(", ")
    );

    // **Traversal equivalence against the blockout**, once per run: the campaign
    // as it now stands on disk, validated and built in memory through the same
    // observers `delvec build` runs. A red here is the state `build` would
    // report and is left standing — the broken intermediate is a real, lookable
    // object (spec-0050 §1) — and it names the place.
    battery(campaign_dir, prefabs_dir, lang, json)
}

/// Which places this run details, in site-plan box order.
fn targets(
    campaign_dir: &Path,
    campaign: &Campaign,
    place: Option<&str>,
    all: bool,
) -> Result<Vec<NodeId>, u8> {
    let boxes: Vec<NodeId> = engine::frames(campaign)
        .into_iter()
        .map(|(f, _)| f.node)
        .collect();
    if all {
        let dir = campaign_dir.join(PROGRAMS_DIR);
        let mut stems: Vec<String> = Vec::new();
        if dir.is_dir() {
            let mut entries: Vec<PathBuf> = match std::fs::read_dir(&dir) {
                Ok(rd) => rd.filter_map(|e| e.ok().map(|e| e.path())).collect(),
                Err(e) => {
                    eprintln!("internal error: cannot read {}: {e}", dir.display());
                    return Err(EXIT_INTERNAL);
                }
            };
            entries.sort();
            for p in entries {
                if p.extension().and_then(|e| e.to_str()) == Some("json")
                    && let Some(stem) = p.file_stem().and_then(|s| s.to_str())
                {
                    stems.push(stem.to_string());
                }
            }
        }
        // A program naming no place is a stale file, refused by name before
        // anything is written: `--all` must not quietly detail fewer places
        // than the directory holds programs.
        let orphans: Vec<&String> = stems
            .iter()
            .filter(|s| !boxes.iter().any(|b| stem_of(b) == s.as_str()))
            .collect();
        if !orphans.is_empty() {
            eprintln!(
                "error: {n} program(s) under `{dir}` name no place the site plan allocates a box \
                 to: {list}. A program is `{dir}/<place stem>.json` for a `node/<place stem>` \
                 of the layout graph; the plan allocates {b} box(es): {boxes}.",
                n = orphans.len(),
                dir = campaign_dir.join(PROGRAMS_DIR).display(),
                list = orphans
                    .iter()
                    .map(|s| format!("`{s}.json`"))
                    .collect::<Vec<_>>()
                    .join(", "),
                b = boxes.len(),
                boxes = boxes
                    .iter()
                    .map(|b| b.0.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            return Err(1);
        }
        let out: Vec<NodeId> = boxes
            .into_iter()
            .filter(|b| stems.iter().any(|s| s == stem_of(b)))
            .collect();
        if out.is_empty() {
            eprintln!(
                "error: `--all` found ZERO programs under `{}`, so there is nothing to detail. \
                 A place is detailed from `{}/<place stem>.json`; write one and run again. This \
                 is a refusal rather than a pass because a run that detailed nothing is not \
                 evidence that every place is detailed.",
                campaign_dir.join(PROGRAMS_DIR).display(),
                PROGRAMS_DIR
            );
            return Err(1);
        }
        return Ok(out);
    }
    let Some(place) = place else {
        eprintln!("error: name a place (`node/<kebab>`), or pass `--all`");
        return Err(EXIT_INTERNAL);
    };
    let node = NodeId(place.to_string());
    if !boxes.contains(&node) {
        eprintln!(
            "error: the plan allocates no box to `{place}` — run `delvec allocation --all` to \
             see every place it does, or `delvec validate` if you expected one here"
        );
        return Err(1);
    }
    Ok(vec![node])
}

/// `node/<stem>` → `<stem>`.
fn stem_of(node: &NodeId) -> &str {
    node.0.strip_prefix("node/").unwrap_or(&node.0)
}

/// The expansion seed for a place: the first eight bytes of SHA-256 over the
/// node id, big-endian. A derivation, not a judgement: two places detailed from
/// one included program draw different texture, the same place draws the same
/// texture on every regeneration, and no flag exists to re-roll (spec-0058
/// §2.4).
fn seed_of(node: &NodeId) -> u64 {
    let digest = Sha256::digest(node.0.as_bytes());
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    u64::from_be_bytes(bytes)
}

/// Every name the allocation hands a program, with its value (spec-0058 §2.3).
fn handed(a: &Allocation) -> BTreeMap<String, i64> {
    let mut out = BTreeMap::new();
    out.insert(format!("{HANDED_PREFIX}datum-y"), a.datum_y);
    for s in &a.seams {
        let edge = s.edge.strip_prefix("edge/").unwrap_or(&s.edge);
        let key = |k: &str| format!("{HANDED_PREFIX}seam/{edge}/{k}");
        let [lo, hi] = s.cells;
        out.insert(key("x0"), lo[0]);
        out.insert(key("y0"), lo[1]);
        out.insert(key("z0"), lo[2]);
        out.insert(key("x1"), hi[0]);
        out.insert(key("y1"), hi[1]);
        out.insert(key("z1"), hi[2]);
        out.insert(key("rise"), s.rise);
    }
    out
}

/// Detail one place: steps 1–8 of spec-0058 §2.2. Returns the piece id written.
fn detail_one(
    campaign_dir: &Path,
    campaign: &Campaign,
    walk_record: Option<&str>,
    library: &PrefabRegistry,
    prefabs_dir: &Path,
    node: &NodeId,
    json: bool,
) -> Result<String, u8> {
    let stem = stem_of(node);
    let place = node.0.as_str();
    let a = engine::allocation(campaign, node).expect("a target is a place the plan allocates");
    let id = format!("{}-{stem}", campaign.world.campaign_id.0);
    if !export::is_valid_id(&id) {
        eprintln!(
            "error: `{place}`: the piece id `{id}` (campaign id plus place stem) is not a usable \
             structure id — lowercase letters, digits and hyphens only."
        );
        return Err(1);
    }

    // ---- 2. the program, and the handing bound into it ----
    let program_path = campaign_dir.join(PROGRAMS_DIR).join(format!("{stem}.json"));
    if !program_path.is_file() {
        eprintln!(
            "error: `{place}` has no program at `{}`. A place is detailed from \
             `{PROGRAMS_DIR}/<place stem>.json` inside the campaign; write it against \
             `delvec allocation {place}` and run again.",
            program_path.display()
        );
        return Err(1);
    }
    let loaded = match document::load(&program_path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("error: `{place}`: {}: {e}", program_path.display());
            return Err(1);
        }
    };
    composition_to_stderr(&loaded);
    let mut program = loaded.program;
    let handed = handed(&a);
    let mut overrides = Overrides::none();
    let declared: Vec<String> = program
        .params
        .keys()
        .filter(|k| k.starts_with(HANDED_PREFIX))
        .cloned()
        .collect();
    for name in &declared {
        let Some(value) = handed.get(name) else {
            let d = Diagnostic::error(
                DW_NOT_HANDED,
                STAGE,
                program_path.display().to_string(),
                format!(
                    "the program declares `{name}`, and the whole hands `{place}` no such value. \
                     A `{HANDED_PREFIX}…` parameter is bound by `delvec detail` from the \
                     allocation and by nothing else, so one the allocation does not hand would \
                     expand at its default in silence — a number standing where the plan's own \
                     figure belongs. Rename it to one of the {n} name(s) this place is handed: \
                     {list}. (A seam is `{HANDED_PREFIX}seam/<edge stem>/{{x0,y0,z0,x1,y1,z1,\
                     rise}}`, keyed by the layout-graph edge without its `edge/` prefix.)",
                    n = handed.len(),
                    list = handed
                        .keys()
                        .map(|k| format!("`{k}`"))
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            );
            print_one_diag(&d, json);
            return Err(1);
        };
        program
            .set_param(name, *value)
            .expect("a declared parameter can be set");
        overrides.params.insert(name.clone(), *value);
    }
    // ---- the palette, handed and gated by nothing (spec-0050 §4) ----
    if let Some(palette) = campaign
        .detail_plan
        .as_ref()
        .and_then(|e| e.content.palette.as_ref())
    {
        for (role, state) in palette {
            let Some(role_stem) = role.strip_prefix("role/") else {
                continue;
            };
            if !program.palette.contains_key(role_stem) {
                continue;
            }
            let block: BlockState = match state.parse() {
                Ok(b) => b,
                Err(e) => {
                    eprintln!(
                        "error: `{place}`: the detail plan's `palette` binds `{role}` to \
                         {state:?}, which is not a block state: {e}"
                    );
                    return Err(1);
                }
            };
            // A restyle keeps the frame of the binding it replaces — the one
            // rule `delvec grammar expand --role` applies.
            let paint = if program.palette.get(role_stem).is_some_and(Paint::is_local) {
                Paint::local_block(block)
            } else {
                Paint::block(block)
            };
            program
                .set_role(role_stem, paint)
                .expect("a declared role can be rebound");
            overrides.roles.insert(role_stem.to_string(), state.clone());
        }
    }

    // ---- 3. the expansion, at the frame ----
    let seed = seed_of(node);
    let opts = ExpandOptions::seeded(seed).with_overrides(overrides);
    let size = [a.extent[0] as u32, a.extent[1] as u32, a.extent[2] as u32];
    let region = Box3::at_origin(size);
    let expansion = match expand(&program, region, &opts) {
        Ok(e) => e,
        Err(e) => {
            eprintln!(
                "error: `{place}`: `{}` cannot expand at the frame the whole hands this place \
                 ({}x{}x{}): {e}. The frame is not editable from here — the box is the site \
                 plan's — so the program is what changes.",
                program_path.display(),
                size[0],
                size[1],
                size[2]
            );
            return Err(1);
        }
    };

    // ---- 4. the grammar gates, exactly as `grammar expand` runs them ----
    let report = gates::judge(&expansion, gates::Options::default());
    if report.is_fail() {
        report_to_stderr(&id, &report);
        eprintln!(
            "error: `{place}`: a machine gate went red on `{}`; nothing was written.",
            program_path.display()
        );
        return Err(1);
    }

    // ---- 5. the piece, in memory ----
    let exported = match export::export_zone(&program, region, &opts, &id) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("error: `{place}`: {id}: {e}\n  nothing was written.");
            return Err(1);
        }
    };
    // Re-read through the one reader that defines the document, whichever
    // packaging the frame needed, then fill what the export could not know: the
    // size class the box is for, and the light the piece was measured to have.
    let mut meta = match PrefabMeta::from_json(exported.metadata_json()) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("internal error: the export's own metadata does not read back: {e}");
            return Err(EXIT_INTERNAL);
        }
    };
    meta.footprint_class = campaign
        .layout_graph
        .as_ref()
        .and_then(|g| g.content.nodes.iter().find(|n| &n.id == node))
        .and_then(|n| n.size_class.clone());
    let tiles: Vec<(TilePart, Structure)> = match structures_of(&exported, &meta) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("internal error: the export's own bytes do not read back: {e}");
            return Err(EXIT_INTERNAL);
        }
    };
    let sky = light::SkyClaim::of(meta.spatial_contract.as_ref());
    let zone = Zone::from_tiles(meta.size(), &tiles);
    let probe = light::probe(&zone, DEFAULT_DARK_THRESHOLD, sky);
    if probe.is_unbound() {
        eprintln!(
            "{} [error] {place}: the light probe bound to ZERO cells, so nothing was measured: \
             {}. Nothing was written.",
            delvewright_admit::diag::DW_UNBOUND,
            probe.unbound_reason()
        );
        return Err(1);
    }
    admit_meta::set_lighting_from_probe(&mut meta, &probe);

    // ---- 6. the bindings check, on the row this run would write ----
    let row = row_for(campaign, node, &id, &meta);
    let mut registry = library.clone();
    registry.insert(meta.clone());
    let judged = with_row(campaign, &row);
    let (diags, binding) = engine::check(&judged, &registry, walk_record);
    let errors: Vec<&Diagnostic> = diags
        .iter()
        .filter(|d| d.severity == delvewright_dsl::Severity::Error)
        .collect();
    if !errors.is_empty() {
        for d in &errors {
            print_one_diag(d, json);
        }
        eprintln!(
            "error: `{place}`: the piece `{}` would not bind; nothing was written. {}",
            exported.prefab_id(),
            binding.line()
        );
        return Err(1);
    }

    // ---- 7. the admission audit ----
    let allow = Allowlist::default_building();
    let (audit_report, audit_diags) = if tiles.len() == 1 {
        audit::audit(&id, &tiles[0].1, &allow)
    } else {
        audit::audit_tile_set(&id, meta.size(), &tiles, &allow)
    };
    for d in &audit_diags {
        d.print(json);
    }
    if audit_report.verdict != "pass" {
        eprintln!(
            "error: `{place}`: `{id}` fails the admission audit ({} forbidden, {} not \
             allowlisted, {} unknown block(s)); nothing was written.",
            audit_report.forbidden, audit_report.not_allowlisted, audit_report.unknown_blocks
        );
        return Err(1);
    }

    // ---- 8. the write ----
    let mut files: Vec<String> = Vec::new();
    for (part, _) in &tiles {
        let bytes = match &exported {
            export::ZoneExport::Single(e) => &e.nbt,
            export::ZoneExport::Tiled(e) => {
                &e.tiles
                    .iter()
                    .find(|t| t.file == part.file)
                    .expect("every tile part has its bytes")
                    .nbt
            }
        };
        write(prefabs_dir, &part.file, bytes)?;
        files.push(part.file.clone());
    }
    write(prefabs_dir, exported.metadata_file(), meta.to_json().as_bytes())?;
    files.push(exported.metadata_file().to_string());
    let report_file = format!("{id}{REPORT_SUFFIX}");
    write(prefabs_dir, &report_file, report.to_json().as_bytes())?;
    files.push(report_file);
    write_row(campaign_dir, campaign, &row)?;

    // ---- what was done, with every count beside its denominator ----
    let faces = meta
        .spatial_contract
        .as_ref()
        .map_or(0, |c| c.faces.len());
    eprintln!(
        "{place}: `{}` written from `{}` — frame {}x{}x{}, seed {seed}; {} of {} handed name(s) \
         bound; {} declared face(s) answering {} allocated seam(s); {} of {} owed name(s) bound; \
         light `{}` over {} measured cell(s){}; files: {}",
        exported.prefab_id(),
        program_path
            .strip_prefix(campaign_dir)
            .unwrap_or(&program_path)
            .display(),
        size[0],
        size[1],
        size[2],
        declared.len(),
        handed.len(),
        faces,
        a.seams.len(),
        row.anchors.len(),
        a.owed_anchors.len(),
        probe.profile,
        probe.measured_cells,
        if probe.is_dark() {
            format!(" (dark: {})", probe.dark_distribution())
        } else {
            String::new()
        },
        files.join(", ")
    );
    if json {
        println!(
            "{}",
            serde_json::json!({
                "place": place,
                "piece": exported.prefab_id(),
                "program": program_path.strip_prefix(campaign_dir).unwrap_or(&program_path).display().to_string(),
                "frame": size,
                "seed": seed,
                "handed": { "bound": declared.len(), "offered": handed.len() },
                "seams": { "faces": faces, "allocated": a.seams.len() },
                "owed": { "bound": row.anchors.len(), "owed": a.owed_anchors.len() },
                "lighting": { "profile": probe.profile, "measured_cells": probe.measured_cells },
                "files": files,
            })
        );
    }
    Ok(exported.prefab_id().to_string())
}

/// The export's bytes as structures, tile by tile, in the order the metadata
/// names them.
fn structures_of(
    exported: &export::ZoneExport,
    meta: &PrefabMeta,
) -> Result<Vec<(TilePart, Structure)>, String> {
    let mut out = Vec::new();
    match exported {
        export::ZoneExport::Single(e) => {
            let s = Structure::read(&e.nbt)?;
            out.push((
                TilePart {
                    file: e.structure_file.clone(),
                    id: String::new(),
                    grid_index: [0, 0, 0],
                    offset: [0, 0, 0],
                    size: s.size,
                },
                s,
            ));
        }
        export::ZoneExport::Tiled(e) => {
            let set = meta
                .structure_set
                .as_ref()
                .ok_or("a tiled export reads back with no structure_set")?;
            for part in &set.parts {
                let tile = e
                    .tiles
                    .iter()
                    .find(|t| t.file == part.file)
                    .ok_or_else(|| format!("no tile bytes for {}", part.file))?;
                out.push((part.clone(), Structure::read(&tile.nbt)?));
            }
        }
    }
    Ok(out)
}

/// The row this run writes: the place, the piece, and every owed name bound to
/// the piece anchor of the same stem (spec-0058 §2.6). An owed name the piece
/// does not answer is left unbound here, and `DW0845` names it.
fn row_for(campaign: &Campaign, node: &NodeId, id: &str, meta: &PrefabMeta) -> Detail {
    let mut anchors = BTreeMap::new();
    for owed in delvewright_dsl::owed_anchors(campaign, node) {
        let stem = owed.strip_prefix("anchor/").unwrap_or(&owed);
        let key = format!("anchor/{stem}");
        if meta.anchors.contains_key(&key) {
            anchors.insert(owed.clone(), key);
        }
    }
    Detail {
        place: node.clone(),
        piece: PrefabId(format!("prefab/{id}")),
        anchors,
    }
}

/// The campaign with `row` standing in its detail plan — the document as the
/// write would leave it, judged before the write.
fn with_row(campaign: &Campaign, row: &Detail) -> Campaign {
    let mut c = campaign.clone();
    let env = c.detail_plan.get_or_insert_with(|| Envelope {
        dsl_version: delvewright_compiler::DSL_VERSION.to_string(),
        campaign_id: campaign.world.campaign_id.clone(),
        stage: Stage::DetailPlan,
        content: DetailPlanContent {
            palette: None,
            details: Vec::new(),
        },
    });
    env.content.details.retain(|d| d.place != row.place);
    env.content.details.push(row.clone());
    c
}

/// Write the row into `detail-plan.json`, canonical, creating the document when
/// the campaign has none. A re-run that changes nothing moves no byte.
fn write_row(campaign_dir: &Path, campaign: &Campaign, row: &Detail) -> Result<(), u8> {
    let path = campaign_dir.join(delvewright_compiler::load::DETAIL_PLAN_FILE);
    let mut env: Envelope<DetailPlanContent> = if path.is_file() {
        let text = std::fs::read_to_string(&path).map_err(|e| {
            eprintln!("internal error: cannot read {}: {e}", path.display());
            EXIT_INTERNAL
        })?;
        serde_json::from_str(&text).map_err(|e| {
            eprintln!("error: {} does not parse as a detail plan: {e}", path.display());
            1u8
        })?
    } else {
        Envelope {
            dsl_version: delvewright_compiler::DSL_VERSION.to_string(),
            campaign_id: campaign.world.campaign_id.clone(),
            stage: Stage::DetailPlan,
            content: DetailPlanContent {
                palette: None,
                details: Vec::new(),
            },
        }
    };
    match env.content.details.iter().position(|d| d.place == row.place) {
        Some(i) => env.content.details[i] = row.clone(),
        None => env.content.details.push(row.clone()),
    }
    let text = delvewright_dsl::to_canonical_string(&env).expect("a detail plan serialises");
    std::fs::write(&path, text).map_err(|e| {
        eprintln!("internal error: cannot write {}: {e}", path.display());
        EXIT_INTERNAL
    })
}

fn write(dir: &Path, file: &str, bytes: &[u8]) -> Result<(), u8> {
    std::fs::write(dir.join(file), bytes).map_err(|e| {
        eprintln!("internal error: cannot write {}: {e}", dir.join(file).display());
        EXIT_INTERNAL
    })
}

/// The whole, validated and built in memory — the same observers `delvec
/// build` runs, output discarded.
fn battery(campaign_dir: &Path, prefabs_dir: &Path, lang: &str, json: bool) -> Result<(), u8> {
    use delvewright_compiler::analyze::analyze_campaign;
    use delvewright_compiler::commands::CommandTree;
    use delvewright_compiler::emit;
    use delvewright_compiler::plan::Plan;

    let v = validate_stage(campaign_dir, prefabs_dir, json)?;
    if has_error(&v.diags) {
        eprintln!("detail: the campaign does not validate with the piece(s) this run wrote.");
        return Err(1);
    }
    let adiags = Fenced::apply(&v.campaign, analyze_campaign(&v.campaign, &v.prefabs));
    if !adiags.reported().is_empty() {
        print_diags(&adiags, json);
        return Err(2);
    }
    let mut campaign = v.campaign;
    if lang == delvewright_dsl::CANONICAL_LANG {
        delvewright_dsl::tag_translatables(&mut campaign);
    }
    let plan = match Plan::build(&campaign, &v.prefabs) {
        Ok(p) => p,
        Err(e) => {
            print_diags(&Fenced::apply(&campaign, e.warnings), json);
            print_build_error(e.failure.code, &e.failure.message, json);
            return Err(3);
        }
    };
    let structures = read_structures(&plan, &v.prefabs, prefabs_dir, json)?;
    let skins = read_skins(campaign_dir, &campaign, json)?;
    let tree = CommandTree::v1_21_11();
    match emit::build_with_warnings(
        &plan,
        &v.loaded.inputs,
        &structures,
        &tree,
        &v.prefabs,
        None,
        &skins,
    ) {
        Ok((_, warnings)) => {
            print_diags(&Fenced::apply(&campaign, warnings), json);
            eprintln!("detail: the whole builds with the piece(s) this run wrote.");
            Ok(())
        }
        Err(emit::BuildFailure::Validation(errors)) => {
            eprintln!(
                "build failure: {} emitted command(s) failed validation:",
                errors.len()
            );
            for e in errors.iter().take(20) {
                eprintln!("  {}: {}", e.reason, e.line);
            }
            Err(3)
        }
        Err(emit::BuildFailure::Diagnostic { code, message }) => {
            print_build_error(code, &message, json);
            Err(code.exit_tier().exit_status())
        }
    }
}
