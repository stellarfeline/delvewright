//! `delve-harvest` (spec-0006 M2): pair `[DelveNote]` stamps in a playtest server
//! log with the creator's chat notes and emit a versioned `playtest-report.json`.
//!
//! ```text
//! delve-harvest <server-log> <manifest> -o playtest-report.json
//! ```
//!
//! `<manifest>` is the creator overlay's `layout.json` (emitted beside
//! `creator-datapack/`) — the harvester's only campaign knowledge (area→prefab,
//! objective→quest). Exit codes: `0` ok · `1` bad input · `≥10` internal error.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use delvewright_orchestrator::{Layout, harvest, report_json};

#[derive(Parser)]
#[command(
    name = "delve-harvest",
    about = "Pair [DelveNote] stamps with creator notes into playtest-report.json (spec-0006)"
)]
struct Cli {
    /// The playtest server log (stdout capture, e.g. `docker logs`).
    server_log: PathBuf,
    /// The creator overlay's layout manifest (`creator-datapack/layout.json`).
    manifest: PathBuf,
    /// Where to write the report (`-` for stdout).
    #[arg(short, long, default_value = "playtest-report.json")]
    out: String,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let log = match std::fs::read_to_string(&cli.server_log) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read server log {}: {e}", cli.server_log.display());
            return ExitCode::from(1);
        }
    };
    let manifest_raw = match std::fs::read_to_string(&cli.manifest) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read manifest {}: {e}", cli.manifest.display());
            return ExitCode::from(1);
        }
    };
    let layout = match Layout::from_json(&manifest_raw) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };

    let report = harvest(&log, &layout);
    let json = report_json(&report);

    if cli.out == "-" {
        print!("{json}");
    } else if let Err(e) = std::fs::write(&cli.out, &json) {
        eprintln!("cannot write report {}: {e}", cli.out);
        return ExitCode::from(10);
    }
    eprintln!(
        "harvested {} note(s) from {}",
        report.notes.len(),
        cli.server_log.display()
    );
    ExitCode::SUCCESS
}
