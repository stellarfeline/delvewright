//! `delvec drawing` — the drawing's command line (spec-0072 §9), mounted by
//! the `delvec` binary (ADR-0023 §3).
//!
//! Two verbs, and they are the two questions an author asks:
//!
//! ```text
//! delvec drawing check   drawings/gatehouse.json
//! delvec drawing execute drawings/gatehouse.json --region 27x56x20 -o out/
//! ```
//!
//! `check` answers *does this document hold together* — every name, every define
//! chain, every paint that would write a property the engine derives, and the
//! contract's reference integrity — with no box, no seed and no cell involved.
//! `execute` answers *what does it build*: it executes, judges with the same
//! gates `delvec grammar expand` runs, and freezes through the same freezer, so
//! a piece built from a drawing is a piece of the same shape as one built from a
//! program.
//!
//! Exit codes, mirroring `delvec grammar`: `0` ok · `2` input/usage · `3`
//! output · `4` a machine gate went red.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Subcommand};

use crate::drawing::execute::{self, ExecuteOptions, Execution};
use crate::drawing::ir::Drawing;
use crate::grammar::block::BlockState;
use crate::grammar::expand::Overrides;
use crate::grammar::ir::States;
use crate::grammar::{Box3, export, gates};

const EXIT_INPUT: u8 = 2;
const EXIT_OUTPUT: u8 = 3;
const EXIT_GATE: u8 = 4;

/// `delvec drawing`: the command line, as a type.
#[derive(Clone, Args)]
pub struct DrawingArgs {
    /// The drawing verb.
    #[command(subcommand)]
    pub command: DrawingCommand,
}

/// The two verbs of `delvec drawing`.
#[derive(Clone, Subcommand)]
pub enum DrawingCommand {
    /// Validate a drawing without executing it.
    ///
    /// Fast, and the right first call when a drawing was just written: every
    /// name, every `define` chain, every paint that writes a property the engine
    /// derives, and the contract's reference integrity are decided here, with no
    /// region and no seed involved.
    Check {
        /// The drawing document.
        file: PathBuf,
    },
    /// Execute a drawing over a region, judge it and freeze it as a prefab.
    Execute {
        /// The drawing document.
        file: PathBuf,
        /// Region to execute into, `XxYxZ` (e.g. `27x56x20`).
        #[arg(long)]
        region: String,
        /// The seed weighted paints draw from. Geometry has none.
        #[arg(long, default_value_t = 0)]
        seed: u64,
        /// Override an integer parameter: `--param bay=5`. Repeatable.
        #[arg(long = "param", value_name = "NAME=VALUE")]
        params: Vec<String>,
        /// Rebind a palette role: `--role wall=minecraft:deepslate_bricks`.
        /// Repeatable.
        #[arg(long = "role", value_name = "ROLE=BLOCKSTATE")]
        roles: Vec<String>,
        /// Prefab id: lowercase letters, digits and hyphens. Defaults to the
        /// input file's stem — the drawing's own `name` identifies the DOCUMENT
        /// in provenance and is never the artifact's id.
        #[arg(long)]
        id: Option<String>,
        /// Also gate on the piece being walkable from its approach end to its
        /// exit end.
        #[arg(long)]
        traversable: bool,
        /// With `--traversable`, allow a fall edge — for a piece entered by
        /// stepping off a ledge.
        #[arg(long)]
        allow_falls: bool,
        /// Also gate on the piece being its own mirror image across this world
        /// axis (`x`, `y` or `z`).
        #[arg(long, value_name = "AXIS")]
        symmetric: Option<String>,
        /// Also gate on every piece of floor under a roof being walkable to from
        /// the grade entrance.
        #[arg(long)]
        reachable_floor: bool,
        /// Output directory. Created if absent.
        #[arg(short, long)]
        out: PathBuf,
    },
}

fn bad_input(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::from(EXIT_INPUT)
}

/// Run one `delvec drawing` invocation.
pub fn run(args: DrawingArgs) -> ExitCode {
    match args.command {
        DrawingCommand::Check { file } => run_check(&file),
        DrawingCommand::Execute {
            file,
            region,
            seed,
            params,
            roles,
            id,
            traversable,
            allow_falls,
            symmetric,
            reachable_floor,
            out,
        } => {
            let symmetric = match symmetric.as_deref().map(parse_axis) {
                Some(Ok(a)) => Some(a),
                Some(Err(e)) => return bad_input(e),
                None => None,
            };
            run_execute(
                &file,
                &region,
                seed,
                &params,
                &roles,
                id.as_deref(),
                gates::Options {
                    traversable,
                    allow_falls,
                    symmetric,
                    reachable_floor,
                },
                &out,
            )
        }
    }
}

fn parse_axis(s: &str) -> Result<crate::grammar::Axis, String> {
    match s.trim() {
        "x" | "X" => Ok(crate::grammar::Axis::X),
        "y" | "Y" => Ok(crate::grammar::Axis::Y),
        "z" | "Z" => Ok(crate::grammar::Axis::Z),
        other => Err(format!("{other:?} is not a world axis; give x, y or z")),
    }
}

fn parse_region(s: &str) -> Result<[u32; 3], String> {
    let parts: Vec<&str> = s.split(['x', 'X']).collect();
    if parts.len() != 3 {
        return Err(format!("region {s:?} is not XxYxZ (e.g. 27x56x20)"));
    }
    let mut out = [0u32; 3];
    for (i, p) in parts.iter().enumerate() {
        out[i] = p
            .trim()
            .parse()
            .map_err(|_| format!("region {s:?}: {p:?} is not a positive integer"))?;
    }
    if out.contains(&0) {
        return Err(format!("region {s:?} has a zero axis"));
    }
    Ok(out)
}

fn split_once_eq<'a>(s: &'a str, what: &str) -> Result<(&'a str, &'a str), String> {
    s.split_once('=')
        .ok_or_else(|| format!("{what} {s:?} is not NAME=VALUE"))
}

fn run_check(file: &Path) -> ExitCode {
    let drawing = match crate::drawing::load::load(file) {
        Ok(d) => d,
        Err(e) => return bad_input(e),
    };
    match execute::check(&drawing) {
        Ok(census) => {
            println!(
                "{}: ok — {} operation(s) ({} at the top level), {} define(s), {} role(s), {} \
                 param(s), {} claimed region(s), {} mark(s). Structure only: execute it to learn \
                 whether it fits a region.",
                file.display(),
                census.ops,
                census.top_level_ops,
                census.defines,
                census.roles,
                census.params,
                census.claimed_regions,
                census.marks
            );
            // A define nothing uses is dead text in the document of record, and
            // it is said rather than refused: a library of parts under
            // construction is a legitimate state to be in.
            if !census.unused_defines.is_empty() {
                eprintln!(
                    "  note: {} define(s) no `use` names: {}",
                    census.unused_defines.len(),
                    census.unused_defines.join(", ")
                );
            }
            ExitCode::SUCCESS
        }
        Err(e) => bad_input(e),
    }
}

/// The overrides a `--param` / `--role` run applies, and the drawing they
/// applied to.
fn with_overrides(
    mut drawing: Drawing,
    params: &[String],
    roles: &[String],
) -> Result<(Drawing, Overrides), String> {
    let mut overrides = Overrides::none();
    for spec in params {
        let (name, value) = split_once_eq(spec, "--param")?;
        let value: i64 = value
            .trim()
            .parse()
            .map_err(|_| format!("--param {spec:?}: {value:?} is not an integer"))?;
        let name = name.trim();
        if !drawing.params.contains_key(name) {
            return Err(format!(
                "--param {spec:?}: the drawing declares no parameter {name:?}; it declares {}. \
                 Overriding an undeclared parameter is refused, because a typo would otherwise \
                 build the default silently.",
                if drawing.params.is_empty() {
                    "none".to_string()
                } else {
                    drawing
                        .params
                        .keys()
                        .map(|k| format!("{k:?}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            ));
        }
        drawing.params.insert(name.to_string(), value);
        overrides.params.insert(name.to_string(), value);
    }
    for spec in roles {
        let (name, value) = split_once_eq(spec, "--role")?;
        let name = name.trim();
        let block: BlockState = value
            .trim()
            .parse()
            .map_err(|e| format!("--role {spec:?}: {e}"))?;
        if !drawing.palette.contains_key(name) {
            return Err(format!(
                "--role {spec:?}: the drawing binds no role {name:?}; it binds {}.",
                if drawing.palette.is_empty() {
                    "none".to_string()
                } else {
                    drawing
                        .palette
                        .keys()
                        .map(|k| format!("{k:?}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            ));
        }
        // A `--role` override is a RESTYLE: it says which material, and every
        // state in a drawing is already read in the frame of the scope that
        // paints it, so there is no frame to inherit and none to lose.
        drawing.palette.insert(name.to_string(), States::One(block));
        overrides
            .roles
            .insert(name.to_string(), value.trim().to_string());
    }
    Ok((drawing, overrides))
}

#[allow(clippy::too_many_arguments)]
fn run_execute(
    file: &Path,
    region: &str,
    seed: u64,
    params: &[String],
    roles: &[String],
    id_override: Option<&str>,
    options: gates::Options,
    out: &Path,
) -> ExitCode {
    let drawing = match crate::drawing::load::load(file) {
        Ok(d) => d,
        Err(e) => return bad_input(e),
    };

    // The id is settled and checked FIRST, before anything is executed, judged,
    // printed or written: an id that cannot become a structure is a property of
    // the inputs alone, so checking it late would put a gate report headed
    // `pass` above a refusal, and a reader's eye stops at the word `pass`.
    let default_id = file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("drawing")
        .to_string();
    let id = id_override.unwrap_or(&default_id).to_string();
    if !export::is_valid_id(&id) {
        return bad_input(format!(
            "{}\n  the id came from {}\n  nothing was executed and nothing was written.",
            export::ExportError::BadId { id: id.clone() },
            if id_override.is_some() {
                "--id".to_string()
            } else {
                format!(
                    "the stem of the input filename {:?}; pass --id <id> to name the prefab \
                     yourself",
                    file.file_name().and_then(|s| s.to_str()).unwrap_or("?")
                )
            },
        ));
    }

    let size = match parse_region(region) {
        Ok(s) => s,
        Err(e) => return bad_input(e),
    };
    let (drawing, overrides) = match with_overrides(drawing, params, roles) {
        Ok(p) => p,
        Err(e) => return bad_input(e),
    };

    // A `grammar` operation names a program file relative to the drawing that
    // writes it, so the root is the drawing's own directory and never the
    // working directory (ADR-0006: a document that resolves against where the
    // tool was run builds on one machine and on no other).
    let root = file.parent().unwrap_or(Path::new(".")).to_path_buf();
    let opts = ExecuteOptions {
        seed,
        limits: crate::grammar::Limits::default(),
        overrides,
        root,
    };
    let box3 = Box3::at_origin(size);

    // Judge before freezing: the report is about the execution, and a red gate
    // must not leave a `.nbt` on disk for someone to pick up later.
    let run: Execution = match execute::execute(&drawing, box3, &opts) {
        Ok(e) => e,
        Err(e) => return bad_input(format!("{}: {e}", file.display())),
    };
    let report = gates::judge(&run.expansion, options);

    if let Err(e) = std::fs::create_dir_all(out) {
        eprintln!("error: create {}: {e}", out.display());
        return ExitCode::from(EXIT_OUTPUT);
    }
    let report_path = out.join(format!("{id}.report.json"));
    if let Err(e) = std::fs::write(&report_path, report.to_json()) {
        eprintln!("error: write {}: {e}", report_path.display());
        return ExitCode::from(EXIT_OUTPUT);
    }
    if report.is_fail() {
        crate::grammar::cli::report_to_stderr(&id, &report);
        run_report_to_stderr(&run);
        eprintln!(
            "error: {id}: a machine gate went red; no prefab was written. The report is at {}.",
            report_path.display()
        );
        return ExitCode::from(EXIT_GATE);
    }

    let provenance = provenance_of(&drawing, &run, &opts);
    let exported = match export::freeze_zone(
        run.expansion.clone(),
        box3,
        &id,
        &drawing.shown_faces,
        &provenance,
    ) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("error: {id}: {e}");
            eprintln!(
                "  no prefab was written. The gate report is at {} — its gates passed; this \
                 refusal is not a gate.",
                report_path.display()
            );
            return ExitCode::from(EXIT_INPUT);
        }
    };
    if let Err(e) = exported.write_to_dir(out) {
        eprintln!("error: write into {}: {e}", out.display());
        return ExitCode::from(EXIT_OUTPUT);
    }
    crate::grammar::cli::report_to_stderr(&id, &report);
    run_report_to_stderr(&run);

    let structures = exported.structure_files();
    let grid = exported.grid();
    let written = if structures.len() == 1 {
        format!("{}/{}", out.display(), structures[0])
    } else {
        eprintln!(
            "  packaging      {} tile(s) in a {}x{}x{} grid — the zone is past the {}-per-axis \
             structure-template cap, so it ships as a tile set and one manifest. Every gate above \
             judged the whole zone.",
            structures.len(),
            grid[0],
            grid[1],
            grid[2],
            export::MAX_STRUCTURE_AXIS
        );
        format!(
            "{}/ {} tile(s) in a {}x{}x{} grid",
            out.display(),
            structures.len(),
            grid[0],
            grid[1],
            grid[2]
        )
    };
    println!(
        "{written} + {} + {} — {}x{}x{}, seed {seed}, {} filled cell(s), {} anchor(s)",
        exported.metadata_file(),
        report_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("report"),
        size[0],
        size[1],
        size[2],
        report.measurements.filled_cells,
        report.anchors.len()
    );
    ExitCode::SUCCESS
}

/// The provenance row a frozen drawing carries: the document, its hash, and the
/// hash of every program a `grammar` operation named.
///
/// The hash covers the drawing's own bytes **and** each named program's, in the
/// order the operations named them, because the row promises that its inputs
/// regenerate the NBT byte for byte — and a program a `grammar` operation reads
/// is one of those inputs.
pub fn provenance_of(
    drawing: &Drawing,
    run: &Execution,
    options: &ExecuteOptions,
) -> export::Provenance {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(drawing.canonical_bytes());
    for (path, hash) in &run.programs {
        hasher.update(path.as_bytes());
        hasher.update(hash.as_bytes());
    }
    let mut hash = String::from("sha256:");
    for byte in hasher.finalize() {
        hash.push_str(&format!("{byte:02x}"));
    }
    export::Provenance {
        generator: "drawing".to_string(),
        module: crate::drawing::GENERATOR,
        source: drawing.name.clone(),
        hash,
        seed: options.seed,
        overrides: options.overrides.clone(),
        composed: run.programs.clone(),
    }
}

/// **What the run did, and what did nothing** (spec-0072 §8) — printed on every
/// execution, whether it passed or not.
///
/// An operation that paints nothing is dead text in the document of record. It
/// is listed rather than refused, because a `define` written for many sizes may
/// honestly have an operation with nothing to do at one of them — but a document
/// whose author cannot see which of its lines built nothing is a document nobody
/// can prune.
pub fn run_report_to_stderr(run: &Execution) {
    let r = &run.report;
    eprintln!(
        "  drawing        {} operation(s) written · {} instance(s) executed · {} cell(s) painted \
         · {} surviving to the model ({} weighted) · {} stair(s) settled",
        r.ops_written,
        r.instances,
        r.cells_painted,
        r.cells_surviving,
        r.weighted_cells,
        r.stairs_settled
    );
    if r.silent.is_empty() {
        eprintln!("  silent         none — every written operation did something");
    } else {
        eprintln!(
            "  silent         {} of {} written operation(s) did nothing at this size:",
            r.silent.len(),
            r.ops_written
        );
        for address in &r.silent {
            eprintln!("      {address}");
        }
    }
    for (path, hash) in &run.programs {
        eprintln!("  grammar        {path} ({hash})");
    }
}
