//! `delve-admit` CLI — the prefab admission pipeline (spec-0007, M3).
//!
//! Exit codes: `0` ok · `1` audit/validation failure · `2` input error · `3`
//! output error · `≥10` internal. Diagnostics (`DW073x..DW076x`) go to stderr,
//! one JSON object per line under `--json`; machine-readable reports go to stdout
//! (or a `--report`/`-o` file).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use delvewright_admit::allowlist::Allowlist;
use delvewright_admit::audit::{self, audit};
use delvewright_admit::catalog::CatalogCard;
use delvewright_admit::diag::{
    DW_DARK, DW_FRAGMENT, DW_GALLERY, DW_INPUT, DW_NO_PROVENANCE, DW_TOOLING, DW_UNBOUND,
    Diagnostic,
};
use delvewright_admit::gallery::{self, Candidate};
use delvewright_admit::light::{self, DEFAULT_DARK_THRESHOLD, Zone};
use delvewright_admit::meta::{self, Anchor, License, PrefabDoc, PrefabMeta, Region};
use delvewright_admit::socket::{self, SocketDecl};
use delvewright_admit::structure::Structure;
use delvewright_schem::split::{TilePart, TileSet, fragment_refusal, tile_evidence};

const EXIT_FAIL: u8 = 1;
const EXIT_INPUT: u8 = 2;
const EXIT_OUTPUT: u8 = 3;

#[derive(Parser)]
#[command(
    name = "delve-admit",
    about = "Prefab admission pipeline: audit, socket carving, anchors, lighting, catalog cards, gallery world",
    version
)]
struct Cli {
    /// Emit diagnostics as one JSON object per line.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Mechanical NBT palette audit (CI gate): allowlist + code-injection forbid.
    Audit {
        /// Input structure `.nbt`, or the `.json` manifest of a zone that ships
        /// as a tile set — which audits every tile and returns ONE zone verdict.
        nbt: PathBuf,
        /// A JSON allowlist override (replaces the built-in default).
        #[arg(long)]
        allowlist: Option<PathBuf>,
        /// Write the machine-readable report here instead of stdout.
        #[arg(short = 'o', long)]
        report: Option<PathBuf>,
    },
    /// Carve a jigsaw socket into a piece (updates the `.nbt` + metadata).
    Socket {
        nbt: PathBuf,
        /// Jigsaw cell (bottom-centre of the opening): `x,y,z`.
        #[arg(long)]
        pos: String,
        /// Outward facing: north|south|east|west.
        #[arg(long)]
        facing: String,
        /// Opening `w,h` (default 3,3).
        #[arg(long, default_value = "3,3")]
        opening: String,
        /// Jigsaw name (default keep:socket).
        #[arg(long, default_value = "keep:socket")]
        name: String,
        /// Jigsaw target (default keep:socket).
        #[arg(long, default_value = "keep:socket")]
        target: String,
        /// Jigsaw pool (default keep:pool).
        #[arg(long, default_value = "keep:pool")]
        pool: String,
    },
    /// Resolve foreign worldgen jigsaw markers to their `final_state` (import-time
    /// neutralization; run BEFORE `socket`).
    ResolveJigsaw { nbt: PathBuf },
    /// Annotate a named anchor into a piece's metadata.
    Anchor {
        nbt: PathBuf,
        /// Anchor name (e.g. `anchor/npc-stand`).
        #[arg(long)]
        name: String,
        /// Point position `x,y,z`.
        #[arg(long)]
        pos: Option<String>,
        /// Facing keyword for a point anchor.
        #[arg(long)]
        facing: Option<String>,
        /// Gate region `x1,y1,z1:x2,y2,z2`.
        #[arg(long)]
        region: Option<String>,
        /// Block id (for gate anchors).
        #[arg(long)]
        block: Option<String>,
    },
    /// Static block-light probe over player space -> declared lighting profile.
    Lighting {
        /// Input structure `.nbt`, or the `.json` manifest of a zone that ships
        /// as a tile set — which reassembles the zone and probes it as one
        /// building.
        nbt: PathBuf,
        /// Persist the measured profile into the prefab's metadata.
        #[arg(long)]
        write: bool,
        /// Dark threshold (floor block-light below this = dark).
        #[arg(long, default_value_t = DEFAULT_DARK_THRESHOLD)]
        dark_threshold: i32,
    },
    /// Validate catalog card(s) (`catalog/<id>.json`).
    Catalog {
        #[command(subcommand)]
        cmd: CatalogCmd,
    },
    /// Build a gallery browse world + datapack from converted candidate pieces.
    Gallery {
        /// Directory of candidate `.nbt` (+ optional sibling metadata).
        dir: PathBuf,
        /// Output directory.
        #[arg(short = 'o', long)]
        out: PathBuf,
        /// Gallery id (default: the directory name).
        #[arg(long)]
        id: Option<String>,
        /// Grid columns.
        #[arg(long, default_value_t = 4)]
        cols: usize,
    },
    /// Harvest a gallery playtest server log into a per-asset curation report.
    Curate {
        /// The server stdout log from the gallery playtest.
        log: PathBuf,
        /// The gallery's `gallery-layout.json`.
        #[arg(long)]
        layout: PathBuf,
        /// Write the curation report here instead of stdout.
        #[arg(short = 'o', long)]
        out: Option<PathBuf>,
    },
    /// Merge a curation report's notes into catalog cards.
    CurateMerge {
        /// The curation report from `curate`.
        report: PathBuf,
        /// The catalog directory (`catalog/<id>.json`).
        #[arg(long)]
        catalog: PathBuf,
    },
}

#[derive(Subcommand)]
enum CatalogCmd {
    /// Validate one or more catalog card files.
    Validate {
        /// Card JSON file(s).
        files: Vec<PathBuf>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let json = cli.json;
    match cli.command {
        Command::Audit {
            nbt,
            allowlist,
            report,
        } => run_audit(&nbt, allowlist.as_deref(), report.as_deref(), json),
        Command::Socket {
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
        Command::ResolveJigsaw { nbt } => run_resolve_jigsaw(&nbt, json),
        Command::Anchor {
            nbt,
            name,
            pos,
            facing,
            region,
            block,
        } => run_anchor(&nbt, &name, pos, facing, region, block, json),
        Command::Lighting {
            nbt,
            write,
            dark_threshold,
        } => run_lighting(&nbt, write, dark_threshold, json),
        Command::Catalog { cmd } => match cmd {
            CatalogCmd::Validate { files } => run_catalog_validate(&files, json),
        },
        Command::Gallery { dir, out, id, cols } => run_gallery(&dir, &out, id, cols, json),
        Command::Curate { log, layout, out } => run_curate(&log, &layout, out.as_deref(), json),
        Command::CurateMerge { report, catalog } => run_curate_merge(&report, &catalog, json),
    }
}

// -------------------------------------------------------------------------

fn run_audit(nbt: &Path, allowlist: Option<&Path>, report: Option<&Path>, json: bool) -> ExitCode {
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
    let (rep, diags) = if nbt.extension().and_then(|s| s.to_str()) == Some("json") {
        match audit_zone(nbt, &allow) {
            Ok(pair) => pair,
            Err(e) => return input_err(&e, json),
        }
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
        let bytes = match std::fs::read(nbt) {
            Ok(b) => b,
            Err(e) => return input_err(&format!("cannot read {}: {e}", nbt.display()), json),
        };
        let structure = match Structure::read(&bytes) {
            Ok(s) => s,
            Err(e) => return input_err(&format!("cannot parse {}: {e}", nbt.display()), json),
        };
        audit(&nbt.display().to_string(), &structure, &allow)
    };
    for d in &diags {
        d.print(json);
    }
    let out_json = rep.to_json();
    if let Some(p) = report {
        if let Err(e) = write_file(p, out_json.as_bytes()) {
            return output_err(&format!("cannot write report {}: {e}", p.display()), json);
        }
    } else {
        print!("{out_json}");
    }
    if rep.is_pass() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(EXIT_FAIL)
    }
}

/// Audit every tile a manifest names, and return the zone's verdict.
fn audit_zone(
    manifest: &Path,
    allow: &Allowlist,
) -> Result<(audit::AuditReport, Vec<Diagnostic>), String> {
    let (set, tiles) = read_zone(manifest)?;
    Ok(audit::audit_tile_set(
        &manifest.display().to_string(),
        set.size,
        &tiles,
        allow,
    ))
}

/// Read every tile a manifest names, in manifest order.
///
/// One reader for every command that takes a zone: the tiling is packaging, and
/// a second copy of "open the files the manifest names and check they are the
/// export it describes" is a second place for the two to disagree.
fn read_zone(manifest: &Path) -> Result<(TileSet, Vec<(TilePart, Structure)>), String> {
    let Some(set) = delvewright_schem::split::read_tile_set(manifest)? else {
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
    let resolved = delvewright_admit::jigsaw::resolve(&mut structure);
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

fn run_anchor(
    nbt: &Path,
    name: &str,
    pos: Option<String>,
    facing: Option<String>,
    region: Option<String>,
    block: Option<String>,
    json: bool,
) -> ExitCode {
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
    meta.set_anchor(
        name,
        Anchor {
            pos,
            facing,
            region: region.map(|(from, to)| Region { from, to }),
            block,
        },
    );
    if let Err(e) = write_meta(nbt, &meta) {
        return output_err(&format!("cannot write metadata: {e}"), json);
    }
    eprintln!("annotated anchor {name}");
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

    let zone = Zone::from_tiles(size, &tiles);
    let probe = light::probe(&zone, dark_threshold);

    // The machine-readable line states the BINDING, not only the verdict: a
    // minimum with no count beside it cannot be read afterwards.
    let report = serde_json::json!({
        "asset": input.display().to_string(),
        "files": tiles.len(),
        "size": size,
        "profile": probe.profile,
        "measured_min_light": probe.measured_min_light,
        "darkest_cell": probe.darkest_cell,
        "dark_threshold": probe.dark_threshold,
        "binding": {
            "standable_cells": probe.standable_cells,
            "entry_cells": probe.entry_cells,
            "reachable_cells": probe.reachable_cells,
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
            .map(|c| format!(" (darkest at {},{},{})", c[0], c[1], c[2]))
            .unwrap_or_default();
        Diagnostic::warning(
            DW_DARK,
            format!(
                "dark interior: min light {} < {} over {} roofed floor cell(s) a player can walk \
                 to{cell}",
                probe.measured_min_light.unwrap_or(0),
                dark_threshold,
                probe.measured_cells
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
        let mut doc = match PrefabDoc::read(&meta_path) {
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
             Create the metadata beside {} first (the generators and `delve-grammar export` write \
             it; for an ingested piece, `delve-admit anchor`/`socket` start one), then re-run with \
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
    // at all (task #70: four legacy gamerules and an out-of-range byte had been
    // silently costing `admit:load` and `admit:finish` in their entirety).
    let tree = match gallery::emit(&gallery_id, &cands, cols) {
        Ok(t) => t,
        Err(errors) => {
            for e in &errors {
                Diagnostic::error(
                    DW_GALLERY,
                    format!(
                        "emitted command is not valid on Minecraft {}: `{}` — {}",
                        delvewright_compiler::MC_VERSION,
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
        let notes: Vec<delvewright_admit::catalog::CurationNote> =
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
        "delve-admit (external admission)",
        License {
            source: "unknown".to_string(),
            spdx: "UNKNOWN".to_string(),
            note: "set at admission — see catalog card".to_string(),
            provenance: "external admission via delve-admit".to_string(),
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
