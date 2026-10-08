#[derive(Subcommand)]
enum RigAction {
    /// Check a rig and print it: the part count, every clip with its length in
    /// ticks, and per clip the footprint of its last frame — the cells its
    /// parts stand in relative to the assembly's mark, which is the number a
    /// strike's landing box is declared from (`DW0938`). A rig that breaks a
    /// rule is refused with `DW0935`, naming the field. Two runs over one rig
    /// are byte-identical.
    Describe {
        /// The rig id, `rig/<name>`, resolved in the `--prefabs` library.
        rig: String,
        /// The facing the footprint is printed at — the assembly's `facing`.
        #[arg(long, value_enum, default_value = "south")]
        facing: FacingArg,
    },
}

/// The four cardinals an assembly can face, for `delvec rig describe`.
#[derive(Clone, Copy, clap::ValueEnum)]
enum FacingArg {
    South,
    North,
    West,
    East,
}

impl FacingArg {
    fn facing(self) -> delvewright_dsl::Facing {
        match self {
            FacingArg::South => delvewright_dsl::Facing::South,
            FacingArg::North => delvewright_dsl::Facing::North,
            FacingArg::West => delvewright_dsl::Facing::West,
            FacingArg::East => delvewright_dsl::Facing::East,
        }
    }
}

/// `delvec rig describe` (spec-0082 §3.1): check one library rig and print
/// what it is, from the one footprint function the strike check judges by.
///
/// Exit codes: `0` printed · `1` the rig is missing, malformed or breaks a rig
/// rule (`DW0935`), or the id is not `rig/<kebab>` · `10` the library cannot
/// be read.
fn run_rig_describe(
    rig: &str,
    facing: delvewright_dsl::Facing,
    prefabs_dir: &Path,
    json: bool,
) -> ExitCode {
    use delvewright_dsl::rig::{self, RigLookup};
    let id = delvewright_dsl::RigId(rig.to_string());
    let refuse = |message: String| {
        let d = Diagnostic::error(
            delvewright_dsl::codes::ASSEMBLY_RIG,
            "rig",
            rig.to_string(),
            message,
        );
        print_diags(std::slice::from_ref(&d), json);
        ExitCode::from(1)
    };
    if !id.is_valid_syntax() {
        return refuse(format!(
            "`{rig}` is not a rig id — a rig is named {}",
            id.syntax_form()
        ));
    }
    let prefabs = match PrefabRegistry::load_dir(prefabs_dir) {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "internal error: cannot read prefabs dir {}: {e}",
                prefabs_dir.display()
            );
            return ExitCode::from(EXIT_INTERNAL);
        }
    };
    let found = match delvewright_dsl::AnchorRegistry::rig(&prefabs, &id) {
        RigLookup::Found(r) => r,
        RigLookup::Missing | RigLookup::Unknown => {
            return refuse(format!(
                "the library at {} holds no `{}/{}/{}`",
                prefabs_dir.display(),
                rig::RIGS_DIR,
                delvewright_dsl::local_id(rig),
                rig::RIG_FILE
            ));
        }
        RigLookup::Malformed(e) => {
            return refuse(format!(
                "`{}` does not parse as a rig document: {e}",
                rig::RIG_FILE
            ));
        }
    };
    let issues = rig::check(found);
    if !issues.is_empty() {
        let diags: Vec<Diagnostic> = issues
            .iter()
            .map(|i| {
                Diagnostic::error(
                    delvewright_dsl::codes::ASSEMBLY_RIG,
                    "rig",
                    format!("{rig}{}", i.field),
                    format!(
                        "rig `{rig}` breaks a rig rule at `{}`: {}",
                        i.field, i.message
                    ),
                )
            })
            .collect();
        print_diags(&diags, json);
        return ExitCode::from(1);
    }
    if json {
        let clips: Vec<serde_json::Value> = found
            .clips
            .iter()
            .map(|(name, c)| {
                serde_json::json!({
                    "clip": name,
                    "frames": c.frames.len(),
                    "ticks_per_frame": c.ticks_per_frame,
                    "length_ticks": c.length_ticks(),
                    "loop": c.looping,
                    "last_frame_footprint": rig::last_frame_footprint(c, facing),
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::json!({
                "rig": rig,
                "facing": facing.token(),
                "parts": found.parts.len(),
                "clips": clips,
            })
        );
    } else {
        print!("{}", rig::describe(rig, found, facing));
    }
    ExitCode::SUCCESS
}

/// `delvec calibrate` (spec-0019 §4): the write-back half of the rehearsal loop.
///
/// Deliberately the cheapest subcommand in the CLI — it needs neither the
/// campaign nor the assembled world, only two JSON artifacts of a build that
/// already happened. The patch it prints is **never applied here**: nothing
/// writes to a stage document from the game (spec-0019 §4). The agent applies
/// it, reruns `delvec build`, and the normal proofs gate the result exactly as
/// they gate a hand-written shot.
///
/// Exit codes: `0` every proposal snapped · `1` unreadable/mismatched inputs
/// (`DW0391`/`DW0392`) · `3` at least one proposal names no anchor within the
/// snap radius (`DW0390`). The patch file is still written on exit 3 — the
/// snappable shots are real work, and withholding them would only make the
/// creator redo the session.
fn run_calibrate(report_path: &Path, layout_path: &Path, out: &str, json: bool) -> ExitCode {
    use delvec::compiler::calibrate;

    let report_raw = match std::fs::read_to_string(report_path) {
        Ok(s) => s,
        Err(e) => {
            print_build_error(
                calibrate::DW_SHOT_REPORT_INVALID,
                &format!(
                    "cannot read rehearsal report `{}`: {e}. It is written by \
                     `delvec harvest` when a playtest session fired `/trigger dw.done`; \
                     do NOT hand-write one.",
                    report_path.display()
                ),
                json,
            );
            return ExitCode::from(1);
        }
    };
    let report: calibrate::RehearsalReport = match serde_json::from_str(&report_raw) {
        Ok(r) => r,
        Err(e) => {
            print_build_error(
                calibrate::DW_SHOT_REPORT_INVALID,
                &format!(
                    "`{}` is not a readable rehearsal report: {e}. Re-run \
                     `delvec harvest` over the session log; do NOT edit the report by hand.",
                    report_path.display()
                ),
                json,
            );
            return ExitCode::from(1);
        }
    };
    if report.version != calibrate::PATCH_VERSION {
        print_build_error(
            calibrate::DW_SHOT_REPORT_INVALID,
            &format!(
                "rehearsal report schema version `{}` is not the `{}` this delvec \
                 understands. Re-harvest the session log with the matching \
                 `delvec harvest`; do NOT edit the version field.",
                report.version,
                calibrate::PATCH_VERSION
            ),
            json,
        );
        return ExitCode::from(1);
    }
    let layout_raw = match std::fs::read_to_string(layout_path) {
        Ok(s) => s,
        Err(e) => {
            print_build_error(
                calibrate::DW_SHOT_REPORT_INVALID,
                &format!(
                    "cannot read layout manifest `{}`: {e}. It is \
                     `creator-datapack/layout.json` of the build the session played.",
                    layout_path.display()
                ),
                json,
            );
            return ExitCode::from(1);
        }
    };
    let layout: calibrate::LayoutAnchors = match serde_json::from_str(&layout_raw) {
        Ok(l) => l,
        Err(e) => {
            print_build_error(
                calibrate::DW_SHOT_REPORT_INVALID,
                &format!(
                    "`{}` is not a readable layout manifest: {e}",
                    layout_path.display()
                ),
                json,
            );
            return ExitCode::from(1);
        }
    };
    if layout.campaign_id != report.campaign_id {
        print_build_error(
            calibrate::DW_SHOT_CAMPAIGN_MISMATCH,
            &format!(
                "the rehearsal report is for campaign `{}` but the layout manifest is \
                 for `{}` — the proposals would snap onto another delve's anchors. Point \
                 `--layout` at the `creator-datapack/layout.json` of the build that \
                 session actually played; do NOT reuse an older build's manifest.",
                report.campaign_id, layout.campaign_id
            ),
            json,
        );
        return ExitCode::from(1);
    }

    let result = calibrate::calibrate(&report, &layout);
    if out == "-" {
        print!("{}", String::from_utf8_lossy(&result.to_json()));
    } else if let Err(e) = write_file(Path::new(out), &result.to_json()) {
        eprintln!("internal error: cannot write patch {out}: {e}");
        return ExitCode::from(EXIT_INTERNAL);
    }

    for u in &result.unsnappable {
        let near = match &u.nearest {
            Some(n) => format!(
                "the nearest declared anchor is `{}`, {} blocks away",
                n.anchor, n.distance
            ),
            None => "the build declares no anchors at all".to_string(),
        };
        print_build_error(
            calibrate::DW_SHOT_UNSNAPPABLE,
            &format!(
                "shot {} {}[{}] proposes cell [{}, {}, {}], and {near} — beyond the {} \
                 block snap radius. The DSL has no free-floating world coordinates \
                 (spec-0019 §5): declare an anchor near that cell in the prefab's \
                 metadata and re-mark the shot, or move the shot to an anchored spot. \
                 Do NOT widen the radius and do NOT write a raw coordinate into the \
                 stage document.",
                u.shot,
                u.kind,
                u.index,
                u.cell[0],
                u.cell[1],
                u.cell[2],
                calibrate::SNAP_RADIUS
            ),
            json,
        );
    }

    if !json {
        println!(
            "calibrated {} shot(s) into {out} ({} un-snappable cell(s); integer snap error 0)",
            result.patches.len(),
            result.unsnappable.len()
        );
    }
    if result.unsnappable.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(3)
    }
}

fn run_metrics(json: bool, gym_dir: Option<&std::path::Path>) -> ExitCode {
    use delvewright_dsl::metrics::{Metrics, export};

    let table = Metrics::table();
    let check = table.self_check();

    println!(
        "{}",
        serde_json::to_string_pretty(&export(&table)).expect("the metrics table serializes")
    );

    let uncalibrated = table.building.values().filter(|e| !e.calibrated).count();
    eprintln!(
        "metrics: version {v}, {p} player metric(s), {b} building metric(s), {uncalibrated} of \
         them not yet walked by the metrics gym.",
        v = table.metrics_version,
        p = table.player.len(),
        b = table.building.len(),
    );
    eprintln!(
        "metrics self-check binding: {i} invariant(s) over {e} building entries; {r} entry(ies) \
         read, {pr} of them provisional.",
        i = check.binding.invariants,
        e = check.binding.entries,
        r = check.binding.reads.read,
        pr = check.binding.reads.provisional,
    );

    if check.binding.invariants == 0 || check.binding.reads.read == 0 {
        eprintln!(
            "{EXIT_INTERNAL_PREFIX} the metrics self-check bound to nothing, so its green says \
             nothing about the table. A check that examined no entry is vacuous, not a pass."
        );
        return ExitCode::from(EXIT_INTERNAL);
    }

    if !check.failures.is_empty() {
        for f in &check.failures {
            eprintln!("{EXIT_INTERNAL_PREFIX} the metrics table contradicts itself: {f}.");
        }
        return ExitCode::from(EXIT_INTERNAL);
    }

    if let Some(d) = table.notice(&check.reads, "metrics") {
        if json {
            println!("{}", serde_json::json!(d));
        } else {
            eprintln!("{} [warning] {}: {}", d.code, d.stage, d.message);
        }
    }

    // The gym, generated FROM the table this run just exported (spec-0049 2.3).
    // It is written where the caller asks and never into this repository: a
    // generated campaign is content, and the engine ships the generator the way
    // it ships a prefab generator rather than the prefabs.
    if let Some(dir) = gym_dir {
        let gym = delvec::compiler::gym::generate(&table, "metrics-gym");
        if let Err(e) = delvec::compiler::gym::write(&gym, dir) {
            eprintln!(
                "delvec metrics --gym: cannot write into {}: {e}",
                dir.display()
            );
            return ExitCode::from(EXIT_INTERNAL);
        }
        eprintln!(
            "metrics gym binding: {d} document(s) written to {p}; {b} bay(s), {s} seam(s); {r} of \
             the {t} building metric(s) instantiated.",
            d = gym.documents.len(),
            p = dir.display(),
            b = gym.bays,
            s = gym.seams,
            r = gym.read.len(),
            t = gym.entries,
        );
        if let Some(d) = gym.unwalked(&table) {
            if json {
                println!("{}", serde_json::json!(d));
            } else {
                eprintln!("{} [warning] {}: {}", d.code, d.stage, d.message);
            }
        }
    }

    ExitCode::SUCCESS
}

/// What an internal error says before it says what went wrong.
const EXIT_INTERNAL_PREFIX: &str = "internal error:";
