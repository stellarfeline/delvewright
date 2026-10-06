//! `delvec sculpt <form.json> -o <dir> [--seed N] [--id <prefab-id>]`.
//!
//! Exit status: `0` written; `1` the form is refused (`DW0951`), nothing fitted
//! or written; `3` the body is refused (`DW0952`) or a machine gate went red,
//! nothing written; `10` the form or the output could not be read or written.

use std::path::PathBuf;
use std::process::ExitCode;

use crate::grammar::gates::{Gate, GateState};
use crate::sculpt::{Readings, SculptError, parse_form, sculpt};

/// The arguments of `delvec sculpt`.
#[derive(Clone, clap::Args)]
pub struct SculptArgs {
    /// The form document (`delvec schema --stage sculpt-form` prints its shape).
    pub form: PathBuf,
    /// Output directory. Created only when the piece is written.
    #[arg(short, long)]
    pub out: PathBuf,
    /// The seed of the weathering and the material picks, recorded in the
    /// metadata's `generated_by`.
    #[arg(long, default_value_t = 0)]
    pub seed: u64,
    /// The prefab id (lowercase letters, digits and hyphens); the form's own
    /// `id` when absent.
    #[arg(long)]
    pub id: Option<String>,
}

const EXIT_FORM: u8 = 1;
const EXIT_BODY: u8 = 3;
const EXIT_IO: u8 = 10;

fn print_gates(gates: &[Gate]) {
    for g in gates {
        eprintln!(
            "  gate {:<16} {}  bound {:<8} {}",
            g.id,
            match g.state {
                GateState::Pass => "pass",
                GateState::Fail => "FAIL",
                GateState::Undecided => "UNDECIDED",
            },
            g.bound,
            g.detail
        );
    }
}

fn print_readings(r: &Readings) {
    eprintln!(
        "  sculpted       {} cell(s), {} filled ({} stair(s), {} slab(s)), {} solid sub-voxel(s); \
         body y=0 lies at piece y={}",
        r.cells, r.filled, r.stairs, r.slabs, r.sub_voxels, r.body_floor
    );
    eprintln!(
        "  fit            {} thin-plate block(s) refilled, {} apron cell(s) laid, {} island \
         block(s) dropped, {} component(s) kept",
        r.fit.thin_filled, r.fit.apron_laid, r.fit.islands_dropped, r.fit.components
    );
    eprintln!(
        "  entry          {} standable cell(s); the walk from {} grade cell(s) on the box's \
         vertical faces reaches {}, and {} of {} standing anchor(s)",
        r.entry.standable,
        r.entry.grade_cells,
        r.entry.reached,
        r.entry.anchors_walked,
        r.entry.anchors
    );
    for h in &r.hull_lights {
        eprintln!("  {}", h.line());
    }
    eprintln!("  {}", r.pockets.line());
    eprintln!(
        "  walk_y         {}",
        r.walk_y
            .map_or("none (no standable cell)".to_string(), |y| y.to_string())
    );
    eprintln!("  shown_faces    {}", r.shown_faces.join(", "));
}

/// Run `delvec sculpt`.
pub fn run(args: SculptArgs) -> ExitCode {
    let text = match std::fs::read_to_string(&args.form) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: read {}: {e}", args.form.display());
            return ExitCode::from(EXIT_IO);
        }
    };
    let where_ = args.form.display().to_string();
    let result = parse_form(&text).and_then(|form| sculpt(&form, args.seed, args.id.as_deref()));
    let sculpture = match result {
        Ok(s) => s,
        Err(SculptError::Form(lines)) => {
            for l in &lines {
                eprintln!("DW0951 [error] sculpt {where_}: {l}");
            }
            eprintln!("nothing was fitted and nothing was written.");
            return ExitCode::from(EXIT_FORM);
        }
        Err(SculptError::Body(lines, readings, gates)) => {
            print_gates(&gates);
            print_readings(&readings);
            for l in &lines {
                eprintln!("DW0952 [error] sculpt {where_}: {l}");
            }
            eprintln!("no prefab was written.");
            return ExitCode::from(EXIT_BODY);
        }
        Err(SculptError::Gates(gates, readings)) => {
            print_gates(&gates);
            print_readings(&readings);
            for g in gates.iter().filter(|g| g.failed()) {
                eprintln!(
                    "error: sculpt {where_}: gate `{}` went red (bound {}): {}",
                    g.id, g.bound, g.detail
                );
            }
            eprintln!("no prefab was written.");
            return ExitCode::from(EXIT_BODY);
        }
        Err(SculptError::Export(e)) => {
            eprintln!("error: sculpt {where_}: {e}");
            eprintln!("no prefab was written.");
            return ExitCode::from(EXIT_BODY);
        }
    };
    if let Err(e) = std::fs::create_dir_all(&args.out) {
        eprintln!("error: create {}: {e}", args.out.display());
        return ExitCode::from(EXIT_IO);
    }
    if let Err(e) = sculpture.write_to_dir(&args.out) {
        eprintln!("error: write into {}: {e}", args.out.display());
        return ExitCode::from(EXIT_IO);
    }
    let r = &sculpture.readings;
    eprintln!("prefab/{}: written", sculpture.id);
    print_gates(&sculpture.gates);
    print_readings(r);
    eprintln!(
        "  light: the probe bound {} grade entry cell(s) and measured {} cell(s): {}",
        r.light_entry_cells,
        r.light_measured_cells,
        sculpture
            .metadata
            .lighting
            .as_ref()
            .map_or("no profile".to_string(), |l| format!("{:?}", l.profile)
                .to_lowercase())
    );
    println!(
        "{}/{}.json + {} structure file(s) — {} cell(s), {} part(s), seed {}",
        args.out.display(),
        sculpture.id,
        sculpture.files.len(),
        r.cells,
        r.parts,
        args.seed
    );
    ExitCode::SUCCESS
}
