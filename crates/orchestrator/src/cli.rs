//! `delvec harvest` (spec-0006 M2): pair `[DelveNote]` stamps in a playtest
//! server log with the creator's chat notes and emit a versioned
//! `playtest-report.json`. Mounted by the `delvec` binary (ADR-0023 §3).
//!
//! ```text
//! delvec harvest <server-log> <manifest> -o playtest-report.json
//! ```
//!
//! `<manifest>` is the creator overlay's `layout.json` (emitted beside
//! `creator-datapack/`) — the harvester's only campaign knowledge (area→prefab,
//! objective→quest). Exit codes: `0` ok · `1` bad input · `≥10` internal error.
//!
//! The same pass also harvests spec-0019 `[DelveShot]` stamps (the creator's
//! `/trigger dw.done`) into a versioned `rehearsal-report.json` **beside** the
//! playtest report. It is written only when the session actually stamped a
//! proposal, so a plain note-taking playtest produces exactly the artifact it
//! did before.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Args;

use crate::rehearsal::{harvest_rehearsal, rehearsal_json};
use crate::{Layout, harvest, report_json};

/// `delvec harvest`: the command line, as a type.
#[derive(Clone, Args)]
pub struct HarvestArgs {
    /// The playtest server log (stdout capture, e.g. `docker logs`).
    pub server_log: PathBuf,
    /// The creator overlay's layout manifest (`creator-datapack/layout.json`).
    pub manifest: PathBuf,
    /// Where to write the report (`-` for stdout).
    #[arg(short, long, default_value = "playtest-report.json")]
    pub out: String,
    /// Where to write the spec-0019 rehearsal report. Written only when the log
    /// carries at least one `[DelveShot]` stamp (`-` for stdout).
    #[arg(long, default_value = "rehearsal-report.json")]
    pub rehearsal_out: String,
}

/// Run `delvec harvest`.
pub fn run(cli: HarvestArgs) -> ExitCode {
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
    // spec-0019: the shot proposals the creator stamped with `dw.done`. Silent
    // when the session stamped none — a note-only playtest keeps its old output.
    let rehearsal = harvest_rehearsal(&log, &layout.campaign_id);
    if !rehearsal.shots.is_empty() {
        let json = rehearsal_json(&rehearsal);
        if cli.rehearsal_out == "-" {
            print!("{json}");
        } else if let Err(e) = std::fs::write(&cli.rehearsal_out, &json) {
            eprintln!("cannot write rehearsal report {}: {e}", cli.rehearsal_out);
            return ExitCode::from(10);
        }
        eprintln!(
            "harvested {} shot proposal(s) into {}",
            rehearsal.shots.len(),
            cli.rehearsal_out
        );
    }

    eprintln!(
        "harvested {} note(s) from {}",
        report.notes.len(),
        cli.server_log.display()
    );
    ExitCode::SUCCESS
}
