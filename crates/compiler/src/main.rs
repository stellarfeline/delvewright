//! `delvec` — the Delvewright compiler CLI (spec-0002).
//!
//! Exit codes: `0` ok · `1` validation failure · `2` analysis failure · `3`
//! build failure · `≥10` internal error.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use delvewright_compiler::analyze::analyze_campaign;
use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit;
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullItemRegistry, PrefabRegistry};
use delvewright_compiler::{DELVEC_VERSION, DSL_VERSION, MC_VERSION};
use delvewright_dsl::{Diagnostic, Stage, parse_campaign, stage_schema, validate_campaign_with};

/// Internal-error exit code (spec-0002: ≥10).
const EXIT_INTERNAL: u8 = 10;

#[derive(Parser)]
#[command(
    name = "delvec",
    version = DELVEC_VERSION,
    about = "Delvewright compiler: staged DSL in, deterministic datapack out",
    long_version = None,
    disable_version_flag = true
)]
struct Cli {
    /// Print `delvec x.y.z, dsl a.b.c, mc x.y.z` and exit.
    #[arg(long, global = true)]
    version: bool,
    /// Emit diagnostics as one JSON object per line (spec-0002 `--json`).
    #[arg(long, global = true)]
    json: bool,
    /// Directory holding prefab metadata (`*.json`) and `.nbt` files.
    #[arg(long, global = true, default_value = "prefabs")]
    prefabs: PathBuf,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Stages 1–5 schema + referential validation.
    Validate {
        /// Campaign directory.
        campaign_dir: PathBuf,
    },
    /// Quest-graph reachability analysis (implies validate).
    Analyze {
        /// Campaign directory.
        campaign_dir: PathBuf,
    },
    /// Full deterministic build (implies validate + analyze).
    Build {
        /// Campaign directory.
        campaign_dir: PathBuf,
        /// Output tree directory.
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Export a stage's JSON Schema (LLM authoring aid).
    Schema {
        /// Stage `1..6` or `all`.
        #[arg(long)]
        stage: String,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    if cli.version {
        println!("delvec {DELVEC_VERSION}, dsl {DSL_VERSION}, mc {MC_VERSION}");
        return ExitCode::SUCCESS;
    }

    let Some(command) = &cli.command else {
        eprintln!("no subcommand (try `delvec --help` or `delvec --version`)");
        return ExitCode::from(EXIT_INTERNAL);
    };

    match command {
        Command::Validate { campaign_dir } => run_validate(campaign_dir, &cli.prefabs, cli.json),
        Command::Analyze { campaign_dir } => run_analyze(campaign_dir, &cli.prefabs, cli.json),
        Command::Build { campaign_dir, out } => {
            run_build(campaign_dir, out, &cli.prefabs, cli.json)
        }
        Command::Schema { stage } => run_schema(stage),
    }
}

/// Validate and return the parsed campaign + prefab registry + diagnostics;
/// prints diagnostics. Returns `Err(exit)` on internal error.
fn validate_stage(
    campaign_dir: &Path,
    prefabs_dir: &Path,
    json: bool,
) -> Result<(delvewright_dsl::Campaign, PrefabRegistry, Vec<Diagnostic>), u8> {
    let loaded = load_campaign_dir(campaign_dir).map_err(|e| {
        eprintln!("internal error: cannot read campaign dir: {e}");
        EXIT_INTERNAL
    })?;
    let prefabs = PrefabRegistry::load_dir(prefabs_dir).map_err(|e| {
        eprintln!(
            "internal error: cannot read prefabs dir {}: {e}",
            prefabs_dir.display()
        );
        EXIT_INTERNAL
    })?;
    let items = FullItemRegistry::v1_21_11();

    match parse_campaign(&loaded.raw) {
        Ok(campaign) => {
            let diags = validate_campaign_with(&campaign, &items, &prefabs);
            print_diags(&diags, json);
            Ok((campaign, prefabs, diags))
        }
        Err(diags) => {
            print_diags(&diags, json);
            Err(1)
        }
    }
}

fn run_validate(campaign_dir: &Path, prefabs_dir: &Path, json: bool) -> ExitCode {
    match validate_stage(campaign_dir, prefabs_dir, json) {
        Ok((_, _, diags)) if diags.is_empty() => ExitCode::SUCCESS,
        Ok(_) => ExitCode::from(1),
        Err(code) => ExitCode::from(code),
    }
}

fn run_analyze(campaign_dir: &Path, prefabs_dir: &Path, json: bool) -> ExitCode {
    let (campaign, prefabs, diags) = match validate_stage(campaign_dir, prefabs_dir, json) {
        Ok(v) => v,
        Err(code) => return ExitCode::from(code),
    };
    if !diags.is_empty() {
        return ExitCode::from(1);
    }
    let adiags = analyze_campaign(&campaign, &prefabs);
    print_diags(&adiags, json);
    if adiags.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}

fn run_build(campaign_dir: &Path, out: &Path, prefabs_dir: &Path, json: bool) -> ExitCode {
    let (campaign, prefabs, diags) = match validate_stage(campaign_dir, prefabs_dir, json) {
        Ok(v) => v,
        Err(code) => return ExitCode::from(code),
    };
    if !diags.is_empty() {
        return ExitCode::from(1);
    }
    let adiags = analyze_campaign(&campaign, &prefabs);
    if !adiags.is_empty() {
        print_diags(&adiags, json);
        return ExitCode::from(2);
    }

    // reload input bytes (for manifest) + structures
    let loaded = match load_campaign_dir(campaign_dir) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("internal error: {e}");
            return ExitCode::from(EXIT_INTERNAL);
        }
    };
    let plan = match Plan::build(&campaign, &prefabs) {
        Ok(p) => p,
        Err(e) => {
            print_build_error(e.code, &e.message, json);
            return ExitCode::from(3);
        }
    };

    // read the structure .nbt bytes referenced by placements
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            if structures.contains_key(&piece.structure_file) {
                continue;
            }
            let path = prefabs_dir.join(&piece.structure_file);
            match std::fs::read(&path) {
                Ok(bytes) => {
                    structures.insert(piece.structure_file.clone(), bytes);
                }
                Err(e) => {
                    print_build_error(
                        "DW0300",
                        &format!("cannot read prefab {}: {e}", path.display()),
                        json,
                    );
                    return ExitCode::from(3);
                }
            }
        }
    }

    let tree = CommandTree::v1_21_11();
    let output = match emit::build(&plan, &loaded.inputs, &structures, &tree) {
        Ok(o) => o,
        Err(errors) => {
            eprintln!(
                "build failure: {} emitted command(s) failed validation:",
                errors.len()
            );
            for e in errors.iter().take(20) {
                eprintln!("  {}: {}", e.reason, e.line);
            }
            return ExitCode::from(3);
        }
    };

    if let Err(e) = write_output(out, &output) {
        eprintln!("internal error: cannot write output: {e}");
        return ExitCode::from(EXIT_INTERNAL);
    }
    ExitCode::SUCCESS
}

fn write_output(out: &Path, output: &emit::BuildOutput) -> std::io::Result<()> {
    for (rel, bytes) in output {
        let path = out.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, bytes)?;
    }
    Ok(())
}

fn run_schema(stage: &str) -> ExitCode {
    let stages = match stage {
        "1" => vec![Stage::World],
        "2" => vec![Stage::Npcs],
        "3" => vec![Stage::Classes],
        "4" => vec![Stage::QuestPlan],
        "5" => vec![Stage::Quests],
        "6" => vec![Stage::Dialogue],
        "all" => vec![
            Stage::World,
            Stage::Npcs,
            Stage::Classes,
            Stage::QuestPlan,
            Stage::Quests,
            Stage::Dialogue,
        ],
        other => {
            eprintln!("unknown stage `{other}` (want 1..6 or all)");
            return ExitCode::from(EXIT_INTERNAL);
        }
    };
    if stages.len() == 1 {
        let schema = stage_schema(stages[0]);
        println!("{}", serde_json::to_string_pretty(&schema).unwrap());
    } else {
        let mut map = serde_json::Map::new();
        for s in stages {
            map.insert(s.name().to_string(), stage_schema(s));
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::Value::Object(map)).unwrap()
        );
    }
    ExitCode::SUCCESS
}

/// Print a `DW03xx` build/solver diagnostic (exit 3), honoring `--json`. Mirrors
/// the spec-0002 one-object-per-line JSON shape used for validation diagnostics.
fn print_build_error(code: &str, message: &str, json: bool) {
    if json {
        let d = serde_json::json!({
            "code": code,
            "severity": "error",
            "stage": "build",
            "path": "",
            "message": message,
        });
        println!("{d}");
    } else {
        eprintln!("{code} [error] build: {message}");
    }
}

fn print_diags(diags: &[Diagnostic], json: bool) {
    for d in diags {
        if json {
            println!("{}", serde_json::to_string(d).unwrap());
        } else {
            let sev = match d.severity {
                delvewright_dsl::Severity::Error => "error",
                delvewright_dsl::Severity::Warning => "warning",
            };
            println!(
                "{} [{sev}] {}{}: {}",
                d.code,
                d.stage,
                if d.path.is_empty() {
                    String::new()
                } else {
                    format!(" {}", d.path)
                },
                d.message
            );
        }
    }
}
