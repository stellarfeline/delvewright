//! `delvec schem` — Sponge schematic (`.schem`) -> vanilla structure `.nbt`,
//! mounted by the `delvec` binary (ADR-0023 §3).
//!
//! Exit codes: `0` ok · `2` input error (unreadable/unparseable schematic or bad
//! usage) · `3` output error (cannot write) · `≥10` internal error.
//!
//! Diagnostics (strips, split notices, warnings) go to stderr; `--json` renders
//! them one JSON object per line. `--palette-report` prints the input palette to
//! stdout, which is otherwise unused so it stays machine-parseable.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::diag::{DW_INPUT, Diagnostic};
use crate::{ConvertOutput, convert};
use clap::{Args, Subcommand};

const EXIT_INPUT: u8 = 2;
const EXIT_OUTPUT: u8 = 3;

/// Vanilla structure templates cap each axis at 48.
const DEFAULT_SPLIT: i32 = 48;

/// `delvec schem`: the command line, as a type.
#[derive(Clone, Args)]
pub struct SchemArgs {
    #[command(subcommand)]
    pub command: SchemCommand,
}

#[derive(Clone, Subcommand)]
pub enum SchemCommand {
    /// Convert a `.schem` to a structure `.nbt` (tiling if oversize).
    Convert {
        /// Input Sponge schematic (`.schem`).
        input: PathBuf,
        /// Output structure `.nbt`. When the schematic is oversize, split parts
        /// and a `<base>.split.json` manifest are written beside it.
        #[arg(short, long)]
        out: PathBuf,
        /// Max part size per axis (the 48-cube structure cap).
        #[arg(long, default_value_t = DEFAULT_SPLIT)]
        split: i32,
        /// Print the full input block-state palette to stdout (audit feed).
        #[arg(long)]
        palette_report: bool,
    },
}

/// Run `delvec schem`. `json` is `delvec`'s global diagnostics flag.
pub fn run(args: SchemArgs, json: bool) -> ExitCode {
    match args.command {
        SchemCommand::Convert {
            input,
            out,
            split,
            palette_report,
        } => run_convert(&input, &out, split, palette_report, json),
    }
}

fn run_convert(input: &Path, out: &Path, split: i32, palette_report: bool, json: bool) -> ExitCode {
    let bytes = match std::fs::read(input) {
        Ok(b) => b,
        Err(e) => {
            Diagnostic::error(DW_INPUT, format!("cannot read {}: {e}", input.display()))
                .print(json);
            return ExitCode::from(EXIT_INPUT);
        }
    };

    // Base name for split parts/manifest: the output file stem.
    let base = out
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("structure")
        .to_string();

    let result = match convert(&bytes, &base, split) {
        Ok(r) => r,
        Err(e) => {
            Diagnostic::error(DW_INPUT, format!("cannot parse {}: {e}", input.display()))
                .print(json);
            return ExitCode::from(EXIT_INPUT);
        }
    };

    if palette_report {
        print_palette(&result.palette, json);
    }

    let parent = out.parent().unwrap_or_else(|| Path::new("."));
    let write_result = match &result.output {
        ConvertOutput::Single(data) => write_file(out, data),
        ConvertOutput::Split {
            parts,
            manifest_name,
            manifest_json,
        } => write_split(parent, parts, manifest_name, manifest_json),
    };
    if let Err(e) = write_result {
        Diagnostic::error(DW_INPUT, format!("cannot write output: {e}")).print(json);
        return ExitCode::from(EXIT_OUTPUT);
    }

    for d in &result.diagnostics {
        d.print(json);
    }
    ExitCode::SUCCESS
}

fn write_file(path: &Path, data: &[u8]) -> std::io::Result<()> {
    // (Exit code >=10 is reserved for internal errors per the module docs; the
    // current paths only surface input/output errors.)
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, data)
}

fn write_split(
    dir: &Path,
    parts: &[(String, Vec<u8>)],
    manifest_name: &str,
    manifest_json: &str,
) -> std::io::Result<()> {
    if !dir.as_os_str().is_empty() {
        std::fs::create_dir_all(dir)?;
    }
    for (name, data) in parts {
        std::fs::write(dir.join(name), data)?;
    }
    std::fs::write(dir.join(manifest_name), manifest_json)?;
    Ok(())
}

fn print_palette(palette: &[String], json: bool) {
    if json {
        // A JSON array of block-state strings; stdout is otherwise unused.
        println!("{}", serde_json::to_string(palette).unwrap());
    } else {
        for state in palette {
            println!("{state}");
        }
    }
}
