//! `delve-grammar` — drive the box-split grammar back end from the command line
//! (spec-0027 §3, the sweep step).
//!
//! Exit codes (mirrors `delve-schem`): `0` ok · `2` input/usage · `3` output ·
//! `≥10` internal.
//!
//! The sweep writes snapshots `delve-render batch` consumes directly — each one
//! with its semantics sidecar beside it, so the anchors, the walkable floor and
//! the boundary openings reach the pictures rather than stopping at the blocks —
//! and the whole §3 loop is three commands with nothing hand-assembled between
//! them:
//!
//! ```text
//! delve-grammar sweep --program bell:gate-ward --seeds 1,2,3 -o .sheets/nbt
//! delve-render  batch .sheets/nbt -o .sheets/renders
//! delve-render  contact-sheet .sheets/renders -o .sheets/gate-ward.png
//! ```
//!
//! `tools/zone-sheets.py` runs exactly that chain for a whole campaign.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use delvewright_grammar::sweep::{self, Manifest, SweepReport};

const EXIT_INPUT: u8 = 2;
const EXIT_OUTPUT: u8 = 3;

#[derive(Parser)]
#[command(
    name = "delve-grammar",
    about = "Box-split grammar back end: list programs, sweep candidates for a contact sheet",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List every program that can be swept, with its design box.
    List,
    /// Expand a program many ways and write one snapshot per candidate.
    Sweep {
        /// Program id (see `list`). Required unless `--manifest` is given.
        #[arg(long)]
        program: Option<String>,
        /// Seeds to sweep, comma-separated — the simple case. Mutually
        /// exclusive with `--manifest`.
        #[arg(long, value_delimiter = ',')]
        seeds: Option<Vec<u64>>,
        /// A sweep manifest (JSON). The full surface: per-candidate seed,
        /// region and parameter overrides.
        #[arg(long)]
        manifest: Option<PathBuf>,
        /// Region override, `XxYxZ` (default: the program's design box).
        #[arg(long)]
        region: Option<String>,
        /// Output directory for the snapshots and `sweep.json`. Created if absent.
        #[arg(short, long)]
        out: PathBuf,
        /// Write the manifest that was run beside the report, so a sweep given
        /// entirely in flags is still reproducible from a file.
        #[arg(long)]
        save_manifest: bool,
    },
}

fn bad_input(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::from(EXIT_INPUT)
}

fn parse_region(s: &str) -> Result<[u32; 3], String> {
    let parts: Vec<&str> = s.split(['x', 'X']).collect();
    if parts.len() != 3 {
        return Err(format!("region {s:?} is not XxYxZ (e.g. 20x10x84)"));
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

fn run_list() -> ExitCode {
    let reg = sweep::registry();
    println!("{} program(s) with a design box:", reg.len());
    for e in &reg {
        println!(
            "  {:<24} {}x{}x{}",
            e.id, e.region[0], e.region[1], e.region[2]
        );
    }
    ExitCode::SUCCESS
}

#[allow(clippy::too_many_arguments)]
fn run_sweep(
    program: Option<String>,
    seeds: Option<Vec<u64>>,
    manifest_path: Option<PathBuf>,
    region: Option<String>,
    out: &Path,
    save_manifest: bool,
) -> ExitCode {
    let mut manifest = match (&manifest_path, &program) {
        (Some(_), Some(_)) => {
            return bad_input(
                "--manifest and --program are two ways to say the same thing; give one",
            );
        }
        (Some(path), None) => {
            let bytes = match std::fs::read(path) {
                Ok(b) => b,
                Err(e) => return bad_input(format!("read {}: {e}", path.display())),
            };
            match serde_json::from_slice::<Manifest>(&bytes) {
                Ok(m) => m,
                Err(e) => return bad_input(format!("parse {}: {e}", path.display())),
            }
        }
        (None, Some(p)) => {
            let seeds = seeds.unwrap_or_else(|| vec![0]);
            if seeds.is_empty() {
                return bad_input("--seeds was given with no seeds");
            }
            Manifest::over_seeds(p, &seeds)
        }
        (None, None) => return bad_input("give either --program or --manifest"),
    };

    if let Some(r) = region {
        match parse_region(&r) {
            Ok(size) => manifest.region = Some(size),
            Err(e) => return bad_input(e),
        }
    }

    if let Err(e) = sweep::validate(&manifest) {
        return bad_input(e);
    }
    if let Err(e) = std::fs::create_dir_all(out) {
        eprintln!("error: create {}: {e}", out.display());
        return ExitCode::from(EXIT_OUTPUT);
    }

    let report = match sweep::run(&manifest, out) {
        Ok(r) => r,
        Err(e) => return bad_input(e),
    };

    let report_json = match serde_json::to_string_pretty(&report) {
        Ok(mut s) => {
            s.push('\n');
            s
        }
        Err(e) => {
            eprintln!("internal: serialise report: {e}");
            return ExitCode::from(10);
        }
    };
    if let Err(e) = std::fs::write(out.join("sweep.json"), &report_json) {
        eprintln!("error: write sweep.json: {e}");
        return ExitCode::from(EXIT_OUTPUT);
    }
    if save_manifest {
        match serde_json::to_string_pretty(&manifest) {
            Ok(mut s) => {
                s.push('\n');
                if let Err(e) = std::fs::write(out.join("manifest.json"), &s) {
                    eprintln!("error: write manifest.json: {e}");
                    return ExitCode::from(EXIT_OUTPUT);
                }
            }
            Err(e) => {
                eprintln!("internal: serialise manifest: {e}");
                return ExitCode::from(10);
            }
        }
    }

    report_to_stderr(&report);
    ExitCode::SUCCESS
}

/// Say what the sweep produced, and say plainly when it produced no choice —
/// or nothing to annotate.
fn report_to_stderr(report: &SweepReport) {
    eprintln!("{}", report.summary());
    for row in &report.rows {
        match &row.error {
            Some(e) => eprintln!("  {:<18} REFUSED  {e}", row.id),
            None => eprintln!(
                "  {:<18} {:>7} filled  {:>6} floor  {:>3} anchor(s)  {:>3} opening(s)  \
                 massing {}",
                row.id,
                row.filled_cells,
                row.standable_cells,
                row.anchors,
                row.boundary_openings,
                &row.massing_digest[7..15]
            ),
        }
    }
    if report.anchors_bind_to_nothing() {
        eprintln!(
            "\nFINDING: none of the {} candidates that built declares an ANCHOR. Every picture \
             drawn from this sweep will annotate zero objects, and a page that annotated \
             nothing must not read like a page with nothing to annotate. The fix is in the \
             program — a rule marks the cells that matter — not in the sweep.",
            report.built
        );
    }
    if report.ways_are_undeclared() {
        eprintln!(
            "\nFINDING: no candidate declares where the party ENTERS or LEAVES (an anchor named \
             {} or {}). The sidecars state every standable cell on the boundary, which is \
             where a body COULD cross; which of them is the doorway is authored, and nothing \
             here guesses at it.",
            sweep::WAY_IN_NAMES.join("/"),
            sweep::WAY_OUT_NAMES.join("/"),
        );
    }
    if report.massing_is_uniform() {
        eprintln!(
            "\nFINDING: all {} candidates that built are the SAME MASSING. A contact sheet from \
             this sweep shows one building {} times; there is nothing on it to choose between. \
             Vary the region or the parameters, not only the seed — see `delve-grammar list` and \
             the program's declared parameters.",
            report.built, report.built
        );
    }
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::List => run_list(),
        Command::Sweep {
            program,
            seeds,
            manifest,
            region,
            out,
            save_manifest,
        } => run_sweep(program, seeds, manifest, region, &out, save_manifest),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use delvewright_grammar::sweep::Candidate;

    #[test]
    fn a_region_is_parsed_as_three_axes() {
        assert_eq!(parse_region("20x10x84"), Ok([20, 10, 84]));
        assert!(parse_region("20x10").is_err());
        assert!(parse_region("20x0x84").is_err());
        assert!(parse_region("20xax84").is_err());
    }

    #[test]
    fn the_simple_case_builds_one_candidate_per_seed() {
        let m = Manifest::over_seeds("bell:cliff-road", &[4, 5]);
        assert_eq!(m.candidates.len(), 2);
        assert_eq!(m.candidates[0], Candidate::at_seed("seed-004", 4));
    }
}
