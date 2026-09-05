//! `delvec` — the delve creator: the one binary the engine ships (ADR-0023
//! §3). Everything a creator runs is a subcommand of it. The compiler's own
//! surface (spec-0002) is declared here; the CPU render arms are flattened in
//! from the compiler library; the grammar, prefab-admission, schematic,
//! playtest-harvest and GPU-render surfaces are mounted from their crates'
//! `cli` modules, each named for the object it acts on.
//!
//! Exit codes of the compiler's surface: `0` ok · `1` validation failure · `2`
//! analysis failure · `3` build failure · `≥10` internal error. A mounted
//! surface keeps its own exit-code table, documented on its `cli` module.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use delvewright_compiler::analyze::analyze_campaign;
use delvewright_compiler::blockout::{Knob, Perturb};
use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit;
use delvewright_compiler::load::{
    LoadedCampaign, load_campaign_dir, missing_stage_documents_diagnostic,
};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_compiler::{DELVEC_VERSION, DSL_VERSION, MC_VERSION};
use delvewright_dsl::{
    Diagnostic, DwCode, ExitTier, Fenced, Stage, parse_campaign, stage_schema,
    validate_campaign_with,
};

/// `DW0309`: a staged **body** — a stage-2 npc or a stage-5 actor alike —
/// declares a `skin.texture_id` for which the campaign ships no
/// `skins/<texture_id>.png`. Build-tier (exit 3).
const DW_SKIN_PNG_MISSING: DwCode = DwCode::every_version("DW0309", ExitTier::Build);

/// Internal-error exit code (spec-0002: ≥10).
const EXIT_INTERNAL: u8 = 10;

#[derive(Parser)]
#[command(
    name = "delvec",
    version = DELVEC_VERSION,
    about = "The delve creator: campaign documents in, a provably completable Minecraft adventure map out — and every tool that authors, admits and renders its rooms",
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
    /// Directory holding prefab metadata (`*.json`) and `.nbt` files. Defaults to
    /// `campaigns/prefabs` — the content repo (`delvewright-campaigns`) symlinked
    /// at `campaigns/` for local dev (spec-0007 Step 0). CI passes an explicit
    /// path to a checkout pinned by `versions.toml` `[content].sha`.
    #[arg(long, global = true, default_value = "campaigns/prefabs")]
    prefabs: PathBuf,
    /// Build language (i18n): `en` (default, canonical English) or a code declared
    /// in `world.json` `languages`. Only affects `build`; `validate`/`analyze` are
    /// language-independent apart from sidecar coverage checks.
    #[arg(long, global = true, default_value = "en")]
    lang: String,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Stages 1–7 schema + referential validation.
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
        /// Output tree directory. Required for an ordinary build, and refused
        /// beside `--perturb`: a perturbed build has nowhere to write.
        #[arg(
            short,
            long,
            required_unless_present = "perturb",
            conflicts_with = "perturb"
        )]
        out: Option<PathBuf>,
        /// Ask the derivation for a named defect and watch the observer.
        ///
        /// The stage-5 blockout battery claims to observe the built bytes
        /// rather than replay the arithmetic that laid them, and the only way
        /// to see that claim tested is to make the derivation build the map
        /// wrong in a named way. One knob per run. The run writes NO output —
        /// `--out` is refused beside it — so a perturbed tree does not exist to
        /// be shipped, walked or admitted, and the exit is always non-zero.
        #[arg(long, value_name = "KNOB", value_enum)]
        perturb: Vec<Knob>,
        /// Which place `--perturb sink|brick-up|low-ceiling` damages. Required
        /// for those three and refused for the others, and it must name a box
        /// the site plan declares.
        ///
        /// Not declared `requires = "perturb"`. That attribute was written and
        /// measured: with it in place, `--perturb-place X` with no `--perturb`
        /// parsed cleanly and reached the program, so it bound nothing here.
        /// `resolve_build_kind` refuses the combination instead, and says what
        /// the flag would have decided.
        #[arg(long, value_name = "PLACE")]
        perturb_place: Option<String>,
    },
    /// Emit the l10n key inventory (key → canonical English) as JSON, with the
    /// existing `--lang` sidecar and NPC persona context — the machine-readable
    /// input for translation tooling (`tools/i18n-translate.py`, docs/reference/i18n.md).
    L10nInventory {
        /// Campaign directory.
        campaign_dir: PathBuf,
    },
    /// Rewrite authored Delvewright JSON in canonical form — object keys
    /// sorted, two-space indent, non-ASCII raw, one trailing newline — so an
    /// insertion is a one-line diff instead of a whole-file rewrite. **Array
    /// order is semantic and is never touched** (`quests[]`, `objectives[]`,
    /// `effects[]`); the formatter proves that on every file it writes.
    ///
    /// `--check` is the `cargo fmt --check` half: it writes nothing and exits 1
    /// listing the files that are not canonical.
    Fmt {
        /// Files or directories. A directory is walked recursively for `*.json`,
        /// skipping dot-directories and any `delvec build` output tree (a
        /// directory holding `manifest.json`).
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Report instead of rewriting; exit 1 if anything is not canonical.
        #[arg(long)]
        check: bool,
    },
    /// Export a campaign document's JSON Schema (LLM authoring aid).
    Schema {
        /// Which document. A numbered stage `1..7`; a named map-pipeline stage
        /// document `geometry-brief` | `layout-graph` | `site-plan` |
        /// `detail-plan`; `walk-record` for the hand-written walk record
        /// (a campaign artifact, not a stage document); or `all` for every
        /// stage document at once.
        #[arg(long)]
        stage: String,
    },
    /// Export the metrics standard as JSON (spec-0049 §2) — the player half
    /// (facts of the pinned game) and the building half (this project's
    /// standards, each with its calibration state), on stdout; the table's
    /// self-consistency verdict and its binding counts on stderr.
    Metrics {
        /// Generate the metrics gym (spec-0049 §2.3) into this directory: a
        /// site-plan campaign built FROM the table, one place per rung of the
        /// size-class ladder at each of its bounds, every standard opening, both
        /// stair pitches and a designed fall at the drop policy's cap. Walking
        /// it is what retires `DW0813`. Nothing in it is authored geometry — it
        /// compiles through the ordinary derivation.
        #[arg(long, value_name = "DIR")]
        gym: Option<PathBuf>,
    },
    /// Draft-render one frame of the assembled world + a scene manifest
    /// (spec-0015: the visual authoring loop). Stops after placement +
    /// assembly — it never emits a datapack.
    Snapshot {
        /// Campaign directory.
        campaign_dir: PathBuf,
        /// Explicit camera: `x,y,z,yaw,pitch[,fov]` in Minecraft degrees
        /// (yaw 0 = south/+Z, 90 = west/−X; pitch positive looks down).
        #[arg(long, conflicts_with_all = ["at", "shot"])]
        camera: Option<String>,
        /// Frame an anchor (e.g. `anchor/fire-pit`, or `area/island:anchor/pen`
        /// to disambiguate) from `--dist` blocks away.
        #[arg(long, conflicts_with = "shot")]
        at: Option<String>,
        /// Compass bearing (degrees) the `--at` camera stands on: 0 = due south
        /// of the subject looking north, 90 = due west looking east.
        #[arg(long, requires = "at", default_value_t = 0.0)]
        orbit: f64,
        /// Distance in blocks from the `--at` subject.
        #[arg(long, requires = "at")]
        dist: Option<f64>,
        /// Reuse the camera of a `render-plan.json` shot id (e.g. `spawn`,
        /// `npc/perimedes`, `pov/leg0/wp0`).
        #[arg(long)]
        shot: Option<String>,
        /// Output PNG path (the manifest is written beside it — see `--help`
        /// of the reference doc; default `snapshot.png`).
        #[arg(short, long, default_value = "snapshot.png")]
        out: PathBuf,
        /// Burn in anchor/NPC/actor/interact labels and the ground coordinate grid.
        #[arg(long)]
        labels: bool,
        /// Frame width in pixels.
        #[arg(long, default_value_t = delvewright_compiler::snapshot::DEFAULT_WIDTH)]
        width: u32,
        /// Frame height in pixels.
        #[arg(long, default_value_t = delvewright_compiler::snapshot::DEFAULT_HEIGHT)]
        height: u32,
        /// Print the render wall-clock time to stderr (profiling aid; never
        /// enters the output, so determinism is unaffected).
        #[arg(long)]
        timing: bool,
    },
    /// Per-elevation cutaway floor plans of every area (spec-0015 pillar 3):
    /// one orthographic top-down PNG per detected walkable band, plus an index.
    /// Like `snapshot`, it stops after placement + assembly.
    BlockingChart {
        /// Campaign directory.
        campaign_dir: PathBuf,
        /// Output directory (created if absent).
        #[arg(short, long, default_value = "blocking-chart")]
        out: PathBuf,
        /// Print the render wall-clock time to stderr.
        #[arg(long)]
        timing: bool,
    },
    /// The map editor (spec-0017): replay the stage-7 `world-edits.json` edit
    /// script, enforce the post-batch invariants, and render one snapshot per
    /// batch — the edit → replay → snapshot loop.
    Edit {
        #[command(subcommand)]
        action: EditAction,
    },
    /// The handed allocation for a site-plan place (spec-0050 §4): the frame's
    /// extents, the datum in piece-local coordinates, every seam of the box with
    /// the answering face it requires, the owed anchor names, and the detail
    /// plan's palette.
    ///
    /// Derived from the site plan on every invocation and an input to nothing:
    /// no gate, no build step and no check ever reads what this prints, so a
    /// file made of it is a copy with no consumer and its staleness has no
    /// vector into the build. It refuses without a passed, fresh walk record,
    /// because obtaining an allocation is one of the two events that begin
    /// detail work.
    Allocation {
        /// Campaign directory.
        campaign_dir: PathBuf,
        /// The place — a layout-graph node id (`node/<kebab>`).
        place: Option<String>,
        /// Every place of the plan, in document order.
        #[arg(long)]
        all: bool,
    },
    /// Convert a harvested `rehearsal-report.json` (spec-0019) into per-shot
    /// `anchor + offset` DSL patches. Reads only the report and the creator
    /// overlay's `layout.json` — no campaign, no build, no world assembly.
    Calibrate {
        /// The harvested rehearsal report (`delvec harvest --rehearsal-out`).
        report: PathBuf,
        /// The creator overlay's layout manifest, which carries the
        /// resolved-anchor vocabulary to snap onto.
        #[arg(long)]
        layout: PathBuf,
        /// Where to write the patch document (`-` for stdout).
        #[arg(short, long, default_value = "shot-patch.json")]
        out: String,
    },
    /// The CPU render arms (ADR-0021 §1): `viewer`, `scene`, `panorama`,
    /// `contact-sheet`, `palette` and `index`. Flattened in rather than nested
    /// under a group, because these are ordinary subcommands of the one
    /// binary — `delvec viewer …`, not `delvec render viewer …`. The arms that
    /// need a GPU are `delvec render …` below.
    #[command(flatten)]
    View(delvewright_compiler::view::cli::ViewCommand),
    /// Grammar programs: list the corpus, show or check one, expand it into a
    /// prefab, measure demonstration coverage, audit every program.
    Grammar(delvewright_grammar::cli::GrammarArgs),
    /// A prefab piece under admission: audit, socket, anchor, lighting, catalog
    /// card, gallery world, curation.
    Prefab(delvewright_admit::cli::PrefabArgs),
    /// An outside schematic: convert a Sponge `.schem` into a structure `.nbt`.
    Schem(delvewright_schem::cli::SchemArgs),
    /// A playtest log: pair `[DelveNote]` stamps with the creator's notes into
    /// `playtest-report.json` (and `[DelveShot]` stamps into a rehearsal report).
    Harvest(delvewright_orchestrator::cli::HarvestArgs),
    /// GPU renders through Nucleation/wgpu: one piece's shot set, a whole
    /// library, or the missing-texture fidelity gate.
    Render(delvewright_render::cli::RenderArgs),
}

#[derive(Subcommand)]
enum EditAction {
    /// Replay the edit script — plus an optional `--batch` candidate — and, on
    /// a fully green replay, persist the candidate into `world-edits.json`
    /// (canonical form). Without `--batch`, replays and re-renders only.
    Apply {
        /// Campaign directory.
        campaign_dir: PathBuf,
        /// A candidate batch (one stage-7 `EditBatch` JSON object) to append to
        /// the script. Persisted only if the whole replay is green.
        #[arg(long)]
        batch: Option<PathBuf>,
        /// Directory for the per-batch snapshot PNGs + manifests.
        #[arg(short, long, default_value = "edit-shots")]
        out: PathBuf,
    },
    /// Exactly `apply`, but never writes to the campaign directory — the
    /// candidate batch is replayed, checked and rendered only.
    Preview {
        /// Campaign directory.
        campaign_dir: PathBuf,
        /// A candidate batch (one stage-7 `EditBatch` JSON object) to append to
        /// the script for this replay only.
        #[arg(long)]
        batch: Option<PathBuf>,
        /// Directory for the per-batch snapshot PNGs + manifests.
        #[arg(short, long, default_value = "edit-shots")]
        out: PathBuf,
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
        Command::Build {
            campaign_dir,
            out,
            perturb,
            perturb_place,
        } => run_build(
            campaign_dir,
            out.as_deref(),
            perturb,
            perturb_place.as_deref(),
            &cli.prefabs,
            &cli.lang,
            cli.json,
        ),
        Command::L10nInventory { campaign_dir } => {
            run_l10n_inventory(campaign_dir, &cli.lang, cli.json)
        }
        Command::Fmt { paths, check } => run_fmt(paths, *check, cli.json),
        Command::Schema { stage } => run_schema(stage),
        Command::Allocation {
            campaign_dir,
            place,
            all,
        } => run_allocation(campaign_dir, place.as_deref(), *all, cli.json),
        Command::Metrics { gym } => run_metrics(cli.json, gym.as_deref()),
        Command::Snapshot {
            campaign_dir,
            camera,
            at,
            orbit,
            dist,
            shot,
            out,
            labels,
            width,
            height,
            timing,
        } => run_snapshot(
            campaign_dir,
            &cli.prefabs,
            SnapshotArgs {
                camera: camera.as_deref(),
                at: at.as_deref(),
                orbit: *orbit,
                dist: *dist,
                shot: shot.as_deref(),
                out,
                labels: *labels,
                width: *width,
                height: *height,
                timing: *timing,
            },
            cli.json,
        ),
        Command::BlockingChart {
            campaign_dir,
            out,
            timing,
        } => run_blocking_chart(campaign_dir, &cli.prefabs, out, *timing, cli.json),
        Command::Edit { action } => match action {
            EditAction::Apply {
                campaign_dir,
                batch,
                out,
            } => run_edit(
                campaign_dir,
                &cli.prefabs,
                batch.as_deref(),
                out,
                true,
                cli.json,
            ),
            EditAction::Preview {
                campaign_dir,
                batch,
                out,
            } => run_edit(
                campaign_dir,
                &cli.prefabs,
                batch.as_deref(),
                out,
                false,
                cli.json,
            ),
        },
        Command::Calibrate {
            report,
            layout,
            out,
        } => run_calibrate(report, layout, out, cli.json),
        Command::View(cmd) => cmd.run(cli.json),
        Command::Grammar(args) => delvewright_grammar::cli::run(args.clone()),
        Command::Prefab(args) => delvewright_admit::cli::run(args.clone(), cli.json),
        Command::Schem(args) => delvewright_schem::cli::run(args.clone(), cli.json),
        Command::Harvest(args) => delvewright_orchestrator::cli::run(args.clone()),
        Command::Render(args) => delvewright_render::cli::run(args.clone(), cli.json),
    }
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
    use delvewright_compiler::calibrate;

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

/// The parsed campaign plus everything the CLI commands share: prefab metadata,
/// the loaded campaign directory (stage bytes + l10n sidecars), the parsed l10n
/// sidecars, and the accumulated diagnostics (schema + referential + l10n).
struct Validated {
    campaign: delvewright_dsl::Campaign,
    prefabs: PrefabRegistry,
    loaded: delvewright_compiler::load::LoadedCampaign,
    sidecars: BTreeMap<String, delvewright_dsl::L10nDoc>,
    /// The accumulated diagnostics, **after the obligation fence**
    /// ([`delvewright_dsl::fence`]): a campaign is judged at the `dsl_version` it
    /// declares, so this is the list a verdict may be read off.
    diags: Fenced,
}

/// Parse an `l10n/<code>.json` sidecar map (raw bytes) into typed [`L10nDoc`]s.
/// A malformed sidecar yields a `DW0180` diagnostic and is dropped (so coverage
/// then reports it as a missing sidecar for its declared language).
fn parse_sidecars(
    raw: &BTreeMap<String, Vec<u8>>,
    diags: &mut Vec<Diagnostic>,
) -> BTreeMap<String, delvewright_dsl::L10nDoc> {
    let mut out = BTreeMap::new();
    for (code, bytes) in raw {
        match serde_json::from_slice::<delvewright_dsl::L10nDoc>(bytes) {
            Ok(doc) => {
                out.insert(code.clone(), doc);
            }
            Err(e) => diags.push(Diagnostic::error(
                delvewright_dsl::codes::L10N_MISSING,
                "l10n",
                format!("l10n/{code}.json"),
                format!("malformed l10n sidecar: {e}"),
            )),
        }
    }
    out
}

/// **Read a campaign directory, or say which of the two things went wrong.**
///
/// The four verbs that read a campaign directory (`validate` and everything
/// built on it, `l10n-inventory`, `edit`, `allocation`) each carried their own
/// copy of one error arm, and every copy said the same thing:
/// `internal error: cannot read campaign dir: <name>`, exit 10, no `DW` code.
/// That answer is right for exactly one of the two states it covered. A campaign
/// directory holding a document this process cannot open IS an internal
/// condition worth stopping hard on; a campaign directory that does not hold all
/// six stage documents yet is an author part-way through writing one, which is
/// the state the authoring skill puts them in on purpose.
///
/// So the two are told apart here, once, and the authoring state gets an ordinary
/// coded refusal ([`DW_STAGE_DOCUMENT_MISSING`]) at the validation tier. Nothing
/// else moves: an unreadable document, a bad encoding, a path that is not a
/// directory each print exactly what they printed, at exit 10.
///
/// The order matters and is deliberate. The missing-document question is asked
/// only **after** the load has failed, so a directory that loads is never
/// probed and no verb pays for a check on its success path.
fn load_or_refuse(campaign_dir: &Path, json: bool) -> Result<LoadedCampaign, u8> {
    match load_campaign_dir(campaign_dir) {
        Ok(l) => Ok(l),
        Err(e) => match missing_stage_documents_diagnostic(campaign_dir) {
            Some(d) => {
                print_diags(&Fenced::structural(vec![d]), json);
                Err(1)
            }
            None => {
                eprintln!("internal error: cannot read campaign dir: {e}");
                Err(EXIT_INTERNAL)
            }
        },
    }
}

/// Validate and return the parsed campaign + shared context (prefabs, loaded dir,
/// l10n sidecars) + diagnostics; prints diagnostics. Returns `Err(exit)` on
/// internal error.
fn validate_stage(campaign_dir: &Path, prefabs_dir: &Path, json: bool) -> Result<Validated, u8> {
    let loaded = load_or_refuse(campaign_dir, json)?;
    validate_loaded(loaded, prefabs_dir, json)
}

/// [`validate_stage`] over an already-loaded (possibly augmented) campaign —
/// split out so `delvec edit` can validate a script with a candidate batch
/// appended before anything touches the campaign directory.
fn validate_loaded(
    loaded: delvewright_compiler::load::LoadedCampaign,
    prefabs_dir: &Path,
    json: bool,
) -> Result<Validated, u8> {
    let prefabs = PrefabRegistry::load_dir(prefabs_dir).map_err(|e| {
        eprintln!(
            "internal error: cannot read prefabs dir {}: {e}",
            prefabs_dir.display()
        );
        EXIT_INTERNAL
    })?;
    let items = FullItemRegistry::v1_21_11();
    // v0.3 wave-mob entity validation against the full 1.21.11 entity registry
    // (157 ids, same misode/mcmeta provenance as the item registry).
    let entities = FullEntityRegistry::v1_21_11();

    match parse_campaign(&loaded.raw) {
        Ok(campaign) => {
            let mut diags = validate_campaign_with(&campaign, &items, &prefabs, &entities);
            // **What this run examined, held back until the author's lines are
            // out.** These counts are not optional — a check that reports no
            // binding is one nobody can tell apart from a check that never ran,
            // and `CLAUDE.md`'s vacuity rule is why every one of them is stated
            // whether or not it is zero. What they are not is the first thing an
            // author needs, and printed as they were computed they arrived ahead
            // of the refusal. So they are collected here and emitted after
            // `print_diags`, under a heading, unchanged.
            let mut examined: Vec<String> = Vec::new();
            // Prefab-library load failures (DW0346): a metadata file that did
            // not parse (e.g. newer schema than this delvec) is a first-class
            // validation diagnostic, never a silent skip that resurfaces later
            // as a baffling DW0300 "prefab not found".
            diags.extend(prefabs.load_diagnostics().iter().cloned());
            // i18n l10n sidecar coverage (DW0180/DW0181) — language-independent,
            // runs on every validate/analyze/build. No-op for English-only campaigns.
            let sidecars = parse_sidecars(&loaded.l10n, &mut diags);
            diags.extend(delvewright_dsl::validate_l10n(&campaign, &sidecars));
            // The machine completion-marker channel is reserved (DW0182): no
            // authored or translated player-visible line may carry `[dw:complete`,
            // the bot's completion oracle. Runs for English-only campaigns too.
            diags.extend(delvewright_dsl::validate_marker_channel(
                &campaign, &sidecars,
            ));
            // The private-use block the i18n v2 translation tag is built from is
            // reserved too (DW0183, spec-0029): a string carrying U+E000..U+F8FF
            // could impersonate the key the compiler threads into a text
            // component — and has no glyph in any Minecraft font anyway.
            diags.extend(delvewright_dsl::validate_tr_sigil(&campaign, &sidecars));
            // The compiler's own chrome namespace is reserved as well (DW0186):
            // `delvewright.*` keys are the engine's on-screen strings, shipped
            // translated with the compiler, and a sidecar row under that prefix
            // would be written into the language file and replace product chrome.
            diags.extend(delvewright_dsl::validate_chrome_namespace(&sidecars));
            // Translation provenance (DW0187/DW0188): coverage proves the sidecar's
            // key SET matches the inventory, which is silent about whether a row
            // still translates the English it renders. `source` records what each
            // row was translated FROM, so a rewritten line is detected rather than
            // audited; rows with no provenance are counted, never passed over.
            diags.extend(delvewright_dsl::validate_l10n_provenance(
                &campaign, &sidecars,
            ));
            // i18n v2: every declared language must map to a Minecraft language-file
            // code, or its `assets/delvewright/lang/<code>.json` has no name a client
            // would ask for and the language ships invisible (DW0184).
            if let Err(d) = delvewright_dsl::declared_mc_codes(&campaign) {
                diags.push(d);
            }
            // v0.6 sound + art-title surface (spec-0014): sound-event ids
            // (DW0326), the unsupported `play-sound at: actor` gate (DW0335), and
            // art-title glyph coverage against the `delve:art` font over the source
            // text and every declared-language sidecar (DW0328). Validation-tier
            // (exit 1) — no-op for a campaign that uses neither surface.
            diags.extend(delvewright_compiler::atmos::check_sounds(&campaign));
            diags.extend(delvewright_compiler::atmos::check_art(&campaign, &sidecars));
            // On-screen narrate text that overruns the title/subtitle/art width
            // budget (DW0330). Advisory tier — see `textfit` for why this warns
            // rather than rejects. Runs over the English source and every
            // declared-language sidecar rendition.
            diags.extend(delvewright_compiler::textfit::check_text_fits(
                &campaign, &sidecars,
            ));
            // Dialogue option labels that overrun their dialog button (DW0331).
            // Error tier, unlike DW0330: the 150-GUI-px button is the geometry of
            // the dialog this compiler emits, not a guess about the player's
            // window, so an over-wide caption provably scrolls in game. Runs over
            // the English source and every declared-language sidecar rendition.
            diags.extend(delvewright_compiler::textfit::check_option_labels(
                &campaign, &sidecars,
            ));
            // v0.6 `close-gate` gate-block declaration (DW0343): the fill block is
            // prefab metadata, so this compiler-side check runs here (validation
            // tier). No-op for a campaign that uses no `close-gate`.
            diags.extend(delvewright_compiler::gates::check_close_gates(
                &campaign, &prefabs,
            ));
            // v0.8 seal answers (DW0423): one gate anchor, one `sealed_hint`
            // wording. No-op for a campaign that authors none.
            diags.extend(delvewright_compiler::gates::check_seal_hints(&campaign));
            // NPC location-continuity lint (DW0351). Advisory tier — a warning
            // names a staging discontinuity (an NPC materializing or vanishing
            // away from where it was last staged) but never fails the run:
            // narrative cover is a legitimate authorial answer.
            diags.extend(delvewright_compiler::continuity::check_npc_continuity(
                &campaign,
            ));
            // The NPC scene ledger (DW0460–DW0467, spec-0020): every quest must
            // say where each live NPC is, what they are doing, and what their
            // right-click offers, and the declaration is checked against the
            // effect history. Error tier, except the pre-0.7 deprecation window
            // (DW0465) and the staleness lint (DW0467), which warn.
            diags.extend(delvewright_compiler::cast::check_cast(&campaign));
            // An objective keeps the promise its prompt makes (DW0860-DW0863):
            // a failure clock armed before its own prompt could be read, an
            // adopted container nothing distinguishes from the scenery beside
            // it, a hint the emitter will never show, and a fight the party is
            // given no way to find. Error tier throughout - each judges what the
            // document says, so the fence has nothing to grandfather. The
            // binding line states what it examined, including the zeroes.
            {
                let (pd, pbind) = delvewright_compiler::promise::check(&campaign);
                examined.push(pbind.line());
                diags.extend(pd);
            }
            // spec-0050 (DSL v0.15): the detail plan. `DW0841` (the whole was
            // walked before any part is detailed), `DW0842`-`DW0845` (the
            // binding binds, the piece is the shape of its allocation, its
            // openings are the plan's seams, its anchors have standing) and
            // `DW0848`'s consumer door. Bound HERE because this is the one
            // funnel every subcommand's validation goes through — `build`
            // included — so a defect cannot reach a datapack by skipping
            // `delvec validate`. No-op for a campaign with no `detail-plan`,
            // which is every campaign below 0.15.0, and the binding line states
            // that zero rather than going quiet.
            {
                let (dd, dbind) = delvewright_compiler::detail::check(
                    &campaign,
                    &prefabs,
                    loaded.walk_record.as_deref(),
                );
                if campaign.detail_plan.is_some() || campaign.site_plan.is_some() {
                    examined.push(dbind.line());
                }
                diags.extend(dd);
                diags.extend(delvewright_compiler::detail::blockout_drift(
                    &campaign,
                    loaded.walk_record.as_deref(),
                ));
            }
            // spec-0025 (DSL v0.8): branch-complete narrative verification. Every
            // declared branch is enumerated and every static proof re-run under
            // its flag assignment — terminality, cast continuity, exclusive-content
            // leakage, hard event contradictions — plus the forcing function that
            // every story node says what it does to the story. No-op below 0.8.0.
            diags.extend(delvewright_compiler::branch::check_branches(&campaign));
            // spec-0031 (DSL v0.10): a numeric gate is judged against the writes
            // the path performs before it (DW0879). The reachability model walks
            // objectives and flags; the arithmetic a `requires_state` compares
            // needs an ORDER, and the flow model's path replay is the one thing
            // in the campaign model that has one. Bound HERE for the same reason
            // the detail plan is: this is the one funnel every subcommand's
            // validation goes through, so a delve whose finale gate can never
            // open cannot reach a datapack by skipping `delvec analyze`. The
            // binding line states what it walked, including the zeroes.
            {
                let (sd, sbind) = delvewright_compiler::statepath::check(&campaign);
                examined.push(sbind.line());
                diags.extend(sd);
            }
            // THE OBLIGATION FENCE. Every check above ran; this is where a
            // campaign's own declared `dsl_version` decides which of their
            // findings it is answerable for. Nothing
            // downstream can un-fence it: `print_diags` and `Validated::diags`
            // both hold `Fenced`, which has no constructor from a bare list.
            let diags = Fenced::apply(&campaign, diags);
            print_diags(&diags, json);
            report_grandfathered(&diags);
            report_binding_notes(&campaign, &examined);
            Ok(Validated {
                campaign,
                prefabs,
                loaded,
                sidecars,
                diags,
            })
        }
        Err(diags) => {
            // No campaign parsed, so no declared version to grandfather against —
            // and `structural` is the constructor that says so and refuses to
            // carry anything version-scoped.
            print_diags(&Fenced::structural(diags), json);
            Err(1)
        }
    }
}

/// Whether any diagnostic is a hard rejection. Warnings (`Severity::Warning`) are
/// printed like errors but never fail a run: they flag things the compiler cannot
/// decide with certainty (e.g. `DW0330`, where the true limit depends on the
/// player's window size and GUI scale), so failing on them would dress a judgement
/// call as a fact. Every `Severity::Error` still exits non-zero exactly as before.
fn has_error(diags: &Fenced) -> bool {
    diags.has_error()
}

fn run_validate(campaign_dir: &Path, prefabs_dir: &Path, json: bool) -> ExitCode {
    match validate_stage(campaign_dir, prefabs_dir, json) {
        Ok(v) if !has_error(&v.diags) => ExitCode::SUCCESS,
        Ok(_) => ExitCode::from(1),
        Err(code) => ExitCode::from(code),
    }
}

fn run_analyze(campaign_dir: &Path, prefabs_dir: &Path, json: bool) -> ExitCode {
    let v = match validate_stage(campaign_dir, prefabs_dir, json) {
        Ok(v) => v,
        Err(code) => return ExitCode::from(code),
    };
    if has_error(&v.diags) {
        return ExitCode::from(1);
    }
    let adiags = Fenced::apply(&v.campaign, analyze_campaign(&v.campaign, &v.prefabs));
    print_diags(&adiags, json);
    if adiags.reported().is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}

/// One `l10n-inventory` row: an inventory key, its canonical English source, the
/// NPC whose voice it is (when the key scheme names one), and the translation the
/// current sidecar already carries (absent = untranslated).
#[derive(serde::Serialize)]
struct InventoryEntry<'a> {
    key: &'a str,
    en: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    speaker: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    existing: Option<&'a str>,
}

/// The persona context a translator needs to keep a character's voice: who they
/// are and — above all — how they speak. Deliberately excludes `secret`,
/// `backstory` and `relationships`: plot context no line's *register* depends on.
#[derive(serde::Serialize)]
struct NpcContext<'a> {
    id: &'a str,
    name: &'a str,
    archetype: &'a str,
    speech_style: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    demeanor: Option<&'a str>,
    motivation: &'a str,
}

/// `delvec l10n-inventory <campaign-dir> [--lang <code>]` — the l10n key inventory
/// as JSON on stdout.
///
/// The inventory is [`delvewright_dsl::l10n_inventory`] itself, i.e. **exactly** the
/// key set `DW0180`/`DW0181` enforce, so a translator (human, in-agent, or an
/// external API via `tools/i18n-translate.py`) can be handed the work list up front
/// instead of discovering it by writing an empty sidecar and reading the coverage
/// diagnostics back. Rows carry the canonical English, the speaking NPC (via
/// [`delvewright_dsl::key_speaker`]) and any translation the current
/// `l10n/<lang>.json` already has — so re-running only fills the gaps (idempotence).
///
/// Deliberately runs **before** validation gating: an incomplete sidecar is the
/// normal state when you ask for the inventory. Only an unparseable campaign fails
/// (exit 1); no prefab library is needed.
fn run_l10n_inventory(campaign_dir: &Path, lang: &str, json: bool) -> ExitCode {
    let loaded = match load_or_refuse(campaign_dir, json) {
        Ok(l) => l,
        Err(exit) => return ExitCode::from(exit),
    };
    let campaign = match parse_campaign(&loaded.raw) {
        Ok(c) => c,
        Err(diags) => {
            print_diags(&Fenced::structural(diags), json);
            return ExitCode::from(1);
        }
    };
    // A malformed sidecar reads as absent — every key is then reported untranslated,
    // which is the honest work list (and what `validate` says about it too).
    let sidecar = loaded
        .l10n
        .get(lang)
        .and_then(|b| serde_json::from_slice::<delvewright_dsl::L10nDoc>(b).ok());
    let existing = sidecar.as_ref().map(|d| &d.content);

    let inv = delvewright_dsl::l10n_inventory(&campaign);
    let entries: Vec<InventoryEntry<'_>> = inv
        .iter()
        .map(|(key, en)| InventoryEntry {
            key,
            en,
            speaker: delvewright_dsl::key_speaker(key),
            existing: existing.and_then(|m| m.get(key)).map(String::as_str),
        })
        .collect();
    let npcs: Vec<NpcContext<'_>> = campaign
        .npcs
        .content
        .npcs
        .iter()
        .map(|n| NpcContext {
            id: delvewright_dsl::local_id(n.id.as_str()),
            name: &n.name,
            archetype: &n.persona.archetype,
            speech_style: &n.persona.speech_style,
            demeanor: n.persona.demeanor.as_deref(),
            motivation: &n.persona.motivation,
        })
        .collect();

    let doc = serde_json::json!({
        "campaign_id": campaign.world.campaign_id.as_str(),
        // The sidecar envelope a fresh `l10n/<lang>.json` must carry, taken from
        // the stage docs (the existing sidecar's own version is preserved by the
        // writing tool, not restated here).
        "dsl_version": campaign.world.dsl_version,
        "lang": lang,
        "declared": campaign.world.content.languages.iter().any(|l| l == lang),
        "sidecar_present": sidecar.is_some(),
        "world_title": campaign.world.content.title,
        "npcs": npcs,
        "entries": entries,
    });
    match serde_json::to_string_pretty(&doc) {
        Ok(s) => {
            println!("{s}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("internal error: cannot serialize inventory: {e}");
            ExitCode::from(EXIT_INTERNAL)
        }
    }
}

// ---------------------------------------------------------------------------
// `snapshot` (spec-0015: the visual authoring loop)
// ---------------------------------------------------------------------------

/// The `snapshot` subcommand's arguments, bundled so the dispatcher stays legible.
struct SnapshotArgs<'a> {
    camera: Option<&'a str>,
    at: Option<&'a str>,
    orbit: f64,
    dist: Option<f64>,
    shot: Option<&'a str>,
    out: &'a Path,
    labels: bool,
    width: u32,
    height: u32,
    timing: bool,
}

/// `delvec snapshot <campaign-dir> …` — draft-render one frame of the assembled
/// world plus its scene manifest.
///
/// ## Which pipeline stages this needs
///
/// Exactly three, and no more (spec-0015: "works on a partial build"): parse the
/// campaign → [`Plan::build`] (placement) → read the placed `.nbt` structures →
/// [`assembled::assembled_blocks`]. **No emission**: no command tree, no
/// datapack, no relight, no nav proofs — so a campaign that fails `DW03xx`
/// geometry checks, or one whose quests are half-written, still renders. That is
/// the point: the loop exists to look at builds that are not finished.
///
/// Validation diagnostics are printed but never gate the render. Only an
/// unparseable campaign (exit 1) or a placement failure (exit 3) stops it — in
/// both cases there is no world to look at.
fn run_snapshot(
    campaign_dir: &Path,
    prefabs_dir: &Path,
    args: SnapshotArgs<'_>,
    json: bool,
) -> ExitCode {
    use delvewright_compiler::snapshot;

    let (campaign, prefabs) = match load_for_view(campaign_dir, prefabs_dir, json) {
        Ok(v) => v,
        Err(code) => return ExitCode::from(code),
    };
    let plan = match Plan::build(&campaign, &prefabs) {
        Ok(p) => p,
        Err(e) => {
            // Advisories raised before the failure and explaining it (`DW0498`:
            // the pool draw behind an ambiguous-anchor `DW0305`) print first —
            // the cause above the symptom.
            print_diags(&Fenced::apply(&campaign, e.warnings), json);
            print_build_error(e.failure.code, &e.failure.message, json);
            return ExitCode::from(3);
        }
    };
    // The placement stage's own advisories (`DW0498`). `build`/`edit` get these
    // through `emit::build_with_warnings`; the view commands never emit, so they
    // report them here — a draw that repeats an anchored piece is exactly what a
    // reviewer is looking at in a snapshot.
    print_diags(&Fenced::apply(&campaign, plan.warnings.clone()), json);
    let structures = match read_structures(&plan, &prefabs, prefabs_dir, json) {
        Ok(s) => s,
        Err(code) => return ExitCode::from(code),
    };

    let started = std::time::Instant::now();
    let assembled = match edited_assembled(&plan, &prefabs, &structures, json) {
        Ok(a) => a,
        Err(code) => return ExitCode::from(code),
    };
    // The occupancy view of the same assembled model the grid rasterises: the
    // render plan's cameras are stood up and proven against it (`DW0724`), so a
    // `--shot` here frames exactly what the built plan states.
    //
    // Geometry alone (`nav::Premises::geometry_only`): a camera is stood up
    // against BLOCKS — the question is whether the eye is inside one — and a
    // reviewer framing a shot down into a declared lethal volume is looking at
    // open air, not at a wall. The campaign's premises are about what a body may
    // walk through, which is not what this command asks.
    let world = delvewright_compiler::nav::World::from_occupancy(
        delvewright_compiler::assembled::occupancy_of(
            assembled.blocks.clone(),
            &assembled.open_gates,
        ),
        delvewright_compiler::nav::Premises::geometry_only(),
    );
    let blocks = assembled.blocks;
    let grid = snapshot::VoxelGrid::build(&blocks);
    let assembled_ms = started.elapsed().as_secs_f64() * 1000.0;

    let cam = match resolve_camera(&plan, &prefabs, &grid, &world, &args) {
        Ok(c) => c,
        Err(msg) => {
            eprintln!("error: {msg}");
            return ExitCode::from(1);
        }
    };
    let opts = snapshot::FrameOpts {
        width: args.width,
        height: args.height,
        sea_level: sea_level_of(&campaign),
        labels: args.labels,
    };

    let render_started = std::time::Instant::now();
    let mut frame = snapshot::render_frame(&grid, &cam, &opts);
    let targets = snapshot::collect_targets(&plan);
    let (inside, outside) = snapshot::resolve_targets(&grid, &cam, &opts, &targets);
    if args.labels {
        snapshot::draw_labels(&mut frame, &grid, &cam, &inside);
    }
    let png = delvewright_compiler::png::encode_rgba(
        frame.canvas.width,
        frame.canvas.height,
        &frame.canvas.rgba,
    );
    let render_ms = render_started.elapsed().as_secs_f64() * 1000.0;

    let image_name = args
        .out
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "snapshot.png".to_string());
    let doc = snapshot::manifest(
        campaign.world.campaign_id.as_str(),
        &image_name,
        &cam,
        &opts,
        &grid,
        snapshot::Scene {
            pieces: &snapshot::collect_pieces(&plan),
            inside: &inside,
            outside: &outside,
        },
        &frame.canvas,
    );
    let manifest_path = manifest_path_for(args.out);
    let mut manifest_bytes = match serde_json::to_vec_pretty(&doc) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("internal error: cannot serialize manifest: {e}");
            return ExitCode::from(EXIT_INTERNAL);
        }
    };
    manifest_bytes.push(b'\n');
    if let Err(e) =
        write_file(args.out, &png).and_then(|()| write_file(&manifest_path, &manifest_bytes))
    {
        eprintln!("internal error: cannot write snapshot: {e}");
        return ExitCode::from(EXIT_INTERNAL);
    }

    if args.timing {
        eprintln!(
            "snapshot timing: assemble+grid {assembled_ms:.0} ms, render+manifest {render_ms:.0} ms \
             ({}×{}, {} block kinds)",
            args.width,
            args.height,
            grid.block_kinds()
        );
    }
    if json {
        println!(
            "{}",
            serde_json::json!({
                "png": args.out.display().to_string(),
                "manifest": manifest_path.display().to_string(),
                "in_frame": inside.len(),
                "out_of_frame": outside.len(),
            })
        );
    } else {
        println!(
            "{} ({}×{}) + {} — {} target(s) in frame, {} out",
            args.out.display(),
            args.width,
            args.height,
            manifest_path.display(),
            inside.len(),
            outside.len()
        );
    }
    ExitCode::SUCCESS
}

/// The manifest sidecar path for an output image: the image path with its
/// extension replaced by `manifest.json` (`shot.png` → `shot.manifest.json`).
/// A path with no extension simply gains one.
fn manifest_path_for(out: &Path) -> PathBuf {
    out.with_extension("manifest.json")
}

/// The assembled world a **view** command shows: the stage-7 edit script
/// applied in view mode (spec-0017 — invariants not enforced, so a broken
/// state can be looked at), or the plain assembly for an unedited campaign.
/// A region-resolution failure (`DW0323`) has no world state to show → exit 3.
fn edited_assembled(
    plan: &Plan,
    prefabs: &PrefabRegistry,
    structures: &BTreeMap<String, Vec<u8>>,
    json: bool,
) -> Result<delvewright_compiler::assembled::Assembled, u8> {
    match delvewright_compiler::edit::replay_view(plan, prefabs, structures) {
        Ok(Some(er)) => Ok(er.assembled),
        Ok(None) => Ok(delvewright_compiler::assembled::assemble(plan, structures)),
        Err(e) => {
            print_build_error(e.code, &e.message, json);
            Err(3)
        }
    }
}

/// The `ocean`-horizon sea level to draw as a background plane, or `None` for a
/// `void`-horizon campaign (see `snapshot::SEA_PLANE_NOTE`).
fn sea_level_of(campaign: &delvewright_dsl::Campaign) -> Option<i32> {
    match delvewright_dsl::horizon_base(&campaign.world.content.horizon) {
        delvewright_dsl::HorizonBase::Ocean => Some(delvewright_compiler::plan::SEA_LEVEL),
        _ => None,
    }
}

/// Parse + validate a campaign for a **view-only** command: diagnostics are
/// printed, but only a parse failure stops the run (exit 1). See
/// [`run_snapshot`] for why a view command must not gate on validation.
fn load_for_view(
    campaign_dir: &Path,
    prefabs_dir: &Path,
    json: bool,
) -> Result<(delvewright_dsl::Campaign, PrefabRegistry), u8> {
    let v = validate_stage(campaign_dir, prefabs_dir, json)?;
    Ok((v.campaign, v.prefabs))
}

/// Read the `.nbt` bytes of every structure the plan places. Shared by `build`
/// and the spec-0015 view commands so all three see the same world.
fn read_structures(
    plan: &Plan,
    prefabs: &PrefabRegistry,
    prefabs_dir: &Path,
    json: bool,
) -> Result<BTreeMap<String, Vec<u8>>, u8> {
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    // The horizon surround's tiles first: the compiler GENERATED those bytes, so
    // their `structure_file` keys name nothing on disk and the loop below would
    // fail to find every one of them. Inserted first so a prefab that somehow
    // shared a filename would still win — a file an author shipped is never
    // silently replaced by one the engine made up.
    if let Some(surround) = &plan.surround {
        for (file, bytes) in &surround.structures {
            structures.insert(file.clone(), bytes.clone());
        }
    }
    // Placed pieces, plus any structure a stage-7 `fragment` verb stamps that
    // no piece placed (spec-0017) — the replay needs those bytes too.
    let mut files: Vec<String> = Vec::new();
    for area in &plan.areas {
        for template in area.pieces.iter().flat_map(|p| &p.templates) {
            files.push(template.structure_file.clone());
        }
    }
    files.extend(delvewright_compiler::edit::fragment_structure_files(
        plan.campaign,
        prefabs,
    ));
    for file in files {
        {
            if structures.contains_key(&file) {
                continue;
            }
            let path = prefabs_dir.join(&file);
            match std::fs::read(&path) {
                Ok(bytes) => {
                    structures.insert(file.clone(), bytes);
                }
                Err(e) => {
                    print_build_error(
                        delvewright_compiler::plan::DW_BUILD,
                        &format!(
                            "cannot read prefab structure file `{}`: {e} — the prefab metadata \
                             points at an `.nbt` that is missing or unreadable in the prefabs dir. \
                             Restore the file or fix the metadata path (prefab-library issue)",
                            path.display()
                        ),
                        json,
                    );
                    return Err(3);
                }
            }
        }
    }
    // The templates are the size their metadata says they are (`DW0803`).
    //
    // Bound here as well as inside `emit::build_with_warnings`, and the second
    // binding is the point: this function is the ONE place every CLI consumer
    // of prefab bytes passes through — `build`, `snapshot`, `viewer` and
    // `blocking-chart` — and a review artifact drawn from a stale tile is a
    // picture that lies, which is worse than a datapack that does not build.
    // The check is pure, so running it twice on the build path costs a walk and
    // reports the same verdict.
    if let Err(delvewright_compiler::emit::BuildFailure::Diagnostic { code, message }) =
        delvewright_compiler::emit::check_template_extents(plan, &structures)
    {
        print_build_error(code, &message, json);
        return Err(3);
    }
    Ok(structures)
}

/// Decide the snapshot camera from the mutually-exclusive framing flags.
///
/// Precedence: `--camera` (explicit) → `--at` (subject framing) → `--shot`
/// (reuse a render-plan camera) → the default layout overview. Clap already
/// rejects combining the first three.
fn resolve_camera(
    plan: &Plan,
    prefabs: &PrefabRegistry,
    grid: &delvewright_compiler::snapshot::VoxelGrid,
    world: &delvewright_compiler::nav::World,
    args: &SnapshotArgs<'_>,
) -> Result<delvewright_compiler::snapshot::Camera, String> {
    use delvewright_compiler::snapshot::{Camera, DEFAULT_FOV, DEFAULT_ORBIT_DIST};

    if let Some(spec) = args.camera {
        let parts: Vec<&str> = spec.split(',').map(str::trim).collect();
        if parts.len() < 5 || parts.len() > 6 {
            return Err(format!(
                "--camera wants `x,y,z,yaw,pitch[,fov]` (got {} field(s) in `{spec}`)",
                parts.len()
            ));
        }
        let mut n = [0f64; 6];
        n[5] = DEFAULT_FOV;
        for (i, p) in parts.iter().enumerate() {
            n[i] = p
                .parse()
                .map_err(|_| format!("--camera field {} (`{p}`) is not a number", i + 1))?;
        }
        return Ok(Camera {
            pos: [n[0], n[1], n[2]],
            yaw: n[3],
            pitch: n[4],
            fov: n[5],
        });
    }

    if let Some(subject) = args.at {
        let pos = resolve_subject(plan, subject)?;
        let dist = args.dist.unwrap_or(DEFAULT_ORBIT_DIST);
        let b = args.orbit.to_radians();
        // The camera stands at compass bearing `orbit` from the subject (0 = due
        // south of it, in Minecraft's yaw sense) and looks back at it, raised so
        // the subject sits below the horizon line rather than against the sky.
        let eye = [
            pos[0] as f64 + 0.5 - b.sin() * dist,
            pos[1] as f64 + 1.5 + dist * 0.45,
            pos[2] as f64 + 0.5 + b.cos() * dist,
        ];
        let look = [
            pos[0] as f64 + 0.5,
            pos[1] as f64 + 1.0,
            pos[2] as f64 + 0.5,
        ];
        // An interior subject (a cavern fire pit, an alcove) would otherwise put
        // the orbit eye inside the mountain and render the inside of the rock.
        // Pull the eye along its own sight line until it stands in open air, so
        // `--at` frames an interior without the author having to guess a distance.
        return Ok(Camera::looking_at(
            pull_into_open_air(grid, look, eye),
            look,
            DEFAULT_FOV,
        ));
    }

    if let Some(id) = args.shot {
        return camera_from_shot(plan, prefabs, world, id);
    }

    // Default: a dollhouse overview of the whole layout from the south-east,
    // high enough that the full AABB fits the vertical FOV.
    let (lo, hi) = grid
        .bounds()
        .ok_or_else(|| "the assembled world is empty — nothing to snapshot".to_string())?;
    let centre = [
        (lo[0] + hi[0]) as f64 / 2.0,
        (lo[1] + hi[1]) as f64 / 2.0,
        (lo[2] + hi[2]) as f64 / 2.0,
    ];
    let span = ((hi[0] - lo[0]).max(hi[1] - lo[1]).max(hi[2] - lo[2])) as f64;
    let d = (span * 0.9).max(16.0);
    let eye = [
        centre[0] + d * 0.75,
        centre[1] + d * 0.65,
        centre[2] + d * 0.75,
    ];
    Ok(Camera::looking_at(eye, centre, DEFAULT_FOV))
}

/// The farthest point on the segment `subject → eye` that stands in open air with
/// an unobstructed line back to `subject`, falling back to `eye` when even the
/// subject's own cell is solid (a marker embedded in a wall — worth seeing as
/// such).
///
/// This is what makes `--at` usable on interiors: a fire pit 14 blocks inside a
/// mountain has no exterior vantage, so the requested distance is honoured only
/// as far as the rock allows and the camera then sits in the room with its
/// subject. The walk itself is `camera::stand_in_open_air`, shared with the
/// render plan's own cameras — it used to live here, private to this one flag,
/// while every derived camera in `render-plan.json` went without it.
fn pull_into_open_air(
    grid: &delvewright_compiler::snapshot::VoxelGrid,
    subject: [f64; 3],
    eye: [f64; 3],
) -> [f64; 3] {
    delvewright_compiler::camera::stand_in_open_air(|c| grid.solid(c), subject, eye).unwrap_or(eye)
}

/// Resolve an `--at` subject to a world cell. Accepts a bare anchor name
/// (`anchor/fire-pit`, matched in the first declaring area, `BTreeMap` order) or
/// an `area:anchor` pair (`area/island:anchor/pen`) to disambiguate.
fn resolve_subject(plan: &Plan, subject: &str) -> Result<[i32; 3], String> {
    use delvewright_compiler::plan::ResolvedAnchor;
    let (area, anchor) = match subject.split_once(':') {
        Some((a, n)) => (Some(a), n),
        None => (None, subject),
    };
    let hit = plan
        .anchors
        .iter()
        .find(|((a, n), _)| n == anchor && area.is_none_or(|want| a == want));
    match hit {
        Some((_, ResolvedAnchor::Point { pos, .. })) => Ok(*pos),
        Some((_, ResolvedAnchor::Gate { from, to, .. })) => Ok([
            (from[0] + to[0]) / 2,
            (from[1] + to[1]) / 2,
            (from[2] + to[2]) / 2,
        ]),
        None => {
            let mut known: Vec<String> = plan
                .anchors
                .keys()
                .map(|(a, n)| format!("{a}:{n}"))
                .collect();
            known.sort();
            known.dedup();
            Err(format!(
                "--at `{subject}` matches no anchor. Known anchors: {}",
                known.join(", ")
            ))
        }
    }
}

/// Reuse a `render-plan.json` shot's camera by id.
///
/// The render plan states cameras as `pos` + `look_at` world coordinates (its own
/// yaw convention differs from Minecraft's — see `snapshot`'s module note), so
/// the bridge reads those two points and re-derives Minecraft yaw/pitch. `pov/…`
/// ids additionally need the DW0311 critical-path routes, so those are computed
/// only when a POV shot is actually asked for.
fn camera_from_shot(
    plan: &Plan,
    prefabs: &PrefabRegistry,
    world: &delvewright_compiler::nav::World,
    id: &str,
) -> Result<delvewright_compiler::snapshot::Camera, String> {
    use delvewright_compiler::render_plan;
    use delvewright_compiler::snapshot::{Camera, DEFAULT_FOV};

    let pov = if id.starts_with("pov/") {
        let routes = delvewright_compiler::nav::critical_path_routes(plan, world);
        render_plan::pov_shots(plan, &routes)
    } else {
        Vec::new()
    };
    // The same world the snapshot itself rasterises (the EDITED assembled model),
    // so the camera this returns is the camera the plan states: a shot pulled in
    // out of the rock is pulled in by the same walk here, and a `DW0724` refusal
    // here is one `delvec build` would raise too.
    let doc = render_plan::render_plan(plan, prefabs, &pov, world)
        .map_err(|e| format!("{}: {}", e.code, e.message))?
        .0;
    let shots = doc["shots"].as_array().cloned().unwrap_or_default();
    let Some(shot) = shots.iter().find(|s| s["id"].as_str() == Some(id)) else {
        let mut ids: Vec<&str> = shots.iter().filter_map(|s| s["id"].as_str()).collect();
        ids.sort_unstable();
        return Err(format!(
            "--shot `{id}` is not in this campaign's render plan. Available: {}{}",
            ids.iter().take(24).copied().collect::<Vec<_>>().join(", "),
            if ids.len() > 24 { ", …" } else { "" }
        ));
    };
    let read3 = |v: &serde_json::Value| -> Option<[f64; 3]> {
        let a = v.as_array()?;
        Some([
            a.first()?.as_f64()?,
            a.get(1)?.as_f64()?,
            a.get(2)?.as_f64()?,
        ])
    };
    let cam = &shot["camera"];
    let (Some(pos), Some(look)) = (read3(&cam["pos"]), read3(&cam["look_at"])) else {
        return Err(format!(
            "--shot `{id}` has no usable camera in the render plan"
        ));
    };
    let fov = cam["fov"].as_f64().unwrap_or(DEFAULT_FOV);
    Ok(Camera::looking_at(pos, look, fov))
}

/// Write `bytes` to `path`, creating parent directories.
fn write_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes)
}

/// `delvec blocking-chart <campaign-dir> [-o dir]` — the spec-0015 pillar-3
/// cutaway floor plans.
///
/// Needs the same three stages `snapshot` does (parse → placement → assembled
/// blocks) plus the nav occupancy model, because "walkable" is what the bands
/// are found from and the corridor overlay is the DW0311-proven critical path.
/// Routing is best-effort: a campaign whose critical path does not route yet
/// simply charts without the corridor tint rather than refusing to chart.
fn run_blocking_chart(
    campaign_dir: &Path,
    prefabs_dir: &Path,
    out: &Path,
    timing: bool,
    json: bool,
) -> ExitCode {
    let (campaign, prefabs) = match load_for_view(campaign_dir, prefabs_dir, json) {
        Ok(v) => v,
        Err(code) => return ExitCode::from(code),
    };
    let plan = match Plan::build(&campaign, &prefabs) {
        Ok(p) => p,
        Err(e) => {
            // Advisories raised before the failure and explaining it (`DW0498`:
            // the pool draw behind an ambiguous-anchor `DW0305`) print first —
            // the cause above the symptom.
            print_diags(&Fenced::apply(&campaign, e.warnings), json);
            print_build_error(e.failure.code, &e.failure.message, json);
            return ExitCode::from(3);
        }
    };
    // The placement stage's own advisories (`DW0498`). `build`/`edit` get these
    // through `emit::build_with_warnings`; the view commands never emit, so they
    // report them here — a draw that repeats an anchored piece is exactly what a
    // reviewer is looking at in a snapshot.
    print_diags(&Fenced::apply(&campaign, plan.warnings.clone()), json);
    let structures = match read_structures(&plan, &prefabs, prefabs_dir, json) {
        Ok(s) => s,
        Err(code) => return ExitCode::from(code),
    };

    let started = std::time::Instant::now();
    let assembled = match edited_assembled(&plan, &prefabs, &structures, json) {
        Ok(a) => a,
        Err(code) => return ExitCode::from(code),
    };
    // Every premise the campaign states (`nav::Premises::of_plan`), because the
    // corridor this chart draws is `critical_path_routes` — the SAME derivation
    // the build's completability proof takes — and a chart taken over a smaller
    // premise set draws a corridor no proof ever walked. A route through a
    // declared lethal volume was exactly that: a line on the blocking chart the
    // author could read as cleared.
    let world = delvewright_compiler::nav::World::from_occupancy(
        delvewright_compiler::assembled::occupancy_of(
            assembled.blocks.clone(),
            &assembled.open_gates,
        ),
        delvewright_compiler::nav::Premises::of_plan(&plan, assembled.gate_seals.clone()),
    );
    let blocks = assembled.blocks;
    let targets = delvewright_compiler::snapshot::collect_targets(&plan);
    let corridor: std::collections::BTreeSet<[i32; 3]> =
        delvewright_compiler::nav::critical_path_routes(&plan, &world)
            .into_iter()
            .flat_map(|leg| leg.cells)
            .collect();
    let chart = delvewright_compiler::blocking::chart(&plan, &blocks, &world, &targets, &corridor);
    let elapsed = started.elapsed().as_secs_f64() * 1000.0;

    let mut index = match serde_json::to_vec_pretty(&chart.index) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("internal error: cannot serialize chart index: {e}");
            return ExitCode::from(EXIT_INTERNAL);
        }
    };
    index.push(b'\n');
    let write = || -> std::io::Result<()> {
        for slice in &chart.slices {
            write_file(&out.join(&slice.file), &slice.png)?;
        }
        write_file(&out.join("blocking-chart.json"), &index)
    };
    if let Err(e) = write() {
        eprintln!("internal error: cannot write blocking chart: {e}");
        return ExitCode::from(EXIT_INTERNAL);
    }

    if timing {
        eprintln!(
            "blocking-chart timing: {elapsed:.0} ms for {} slice(s)",
            chart.slices.len()
        );
    }
    if json {
        println!(
            "{}",
            serde_json::json!({
                "dir": out.display().to_string(),
                "slices": chart.slices.iter().map(|s| serde_json::json!({
                    "file": s.file,
                    "area": s.area_id,
                    "floor_y": s.band.floor_y,
                    "labelled": s.labelled.len(),
                })).collect::<Vec<_>>(),
            })
        );
    } else {
        for s in &chart.slices {
            println!(
                "{} — {} floor y={} (cut y{}..{}), {}×{}, {} label(s)",
                out.join(&s.file).display(),
                s.area_id,
                s.band.floor_y,
                s.y_range.0,
                s.y_range.1,
                s.size.0,
                s.size.1,
                s.labelled.len()
            );
        }
        println!("{}", out.join("blocking-chart.json").display());
    }
    ExitCode::SUCCESS
}

/// **A perturbed build is structurally unable to produce a tree.**
///
/// `--out` and `--perturb` are declared as conflicting arguments, so this pair
/// has exactly two inhabited shapes and the parser refuses the other two: an
/// ordinary build carrying a directory, or a demonstration carrying a defect and
/// no directory at all. It is a type here rather than two `Option`s and a
/// comment because the guarantee is worth stating in a form a reader cannot
/// misread: the perturbed arm has nowhere to write, so it does not decline to
/// write a tree — it has no path to write one to. No tree means no
/// `manifest.json`, and `tools/staging-gate.py` fingerprints a build by hashing
/// exactly that file: *a tree with no manifest has no identity and therefore
/// cannot be admitted at all*.
enum BuildKind<'a> {
    /// The build as it ships.
    Ship(&'a Path),
    /// The derivation asked for a named defect, so its observer can be watched.
    Demonstrate(Knob, Perturb),
}

fn run_build(
    campaign_dir: &Path,
    out: Option<&Path>,
    perturb: &[Knob],
    perturb_place: Option<&str>,
    prefabs_dir: &Path,
    lang: &str,
    json: bool,
) -> ExitCode {
    let kind = match resolve_build_kind(out, perturb, perturb_place) {
        Ok(k) => k,
        Err(msg) => {
            eprintln!("error: {msg}");
            return ExitCode::from(1);
        }
    };

    let v = match validate_stage(campaign_dir, prefabs_dir, json) {
        Ok(v) => v,
        Err(code) => return ExitCode::from(code),
    };
    if has_error(&v.diags) {
        return ExitCode::from(1);
    }
    let Validated {
        campaign,
        prefabs,
        loaded,
        sidecars,
        ..
    } = v;
    let adiags = Fenced::apply(&campaign, analyze_campaign(&campaign, &prefabs));
    if !adiags.reported().is_empty() {
        print_diags(&adiags, json);
        return ExitCode::from(2);
    }

    // A perturbation naming a place no box declares would derive a perfectly
    // clean map, and the run would then report an observer that failed to
    // observe — a true sentence about the wrong thing. So the name is checked
    // against the site plan's own boxes, which is the one authority for what a
    // place is, before anything is derived.
    if let BuildKind::Demonstrate(knob, p) = &kind
        && let Some(place) = p.place()
    {
        let declared: Vec<&str> = campaign
            .site_plan
            .as_ref()
            .map(|sp| sp.content.boxes.iter().map(|b| b.node.0.as_str()).collect())
            .unwrap_or_default();
        if !declared.contains(&place) {
            eprintln!(
                "error: --perturb {} --perturb-place `{place}`: this campaign's site plan \
                 declares no such place. It declares {}: {}",
                knob.name(),
                declared.len(),
                if declared.is_empty() {
                    "(this campaign has no site plan, so there is no derivation to perturb)"
                        .to_string()
                } else {
                    declared.join(", ")
                }
            );
            return ExitCode::from(1);
        }
    }

    // i18n: resolve the requested build language. `en` is the implicit canonical
    // build; any other code must be declared in world.json (coverage already
    // validated above). An undeclared `--lang` is a validation-class rejection of
    // the requested build (exit 1, spec-0002).
    let is_english = lang == delvewright_dsl::CANONICAL_LANG;
    let mut campaign = campaign;
    if !is_english {
        if !campaign.world.content.languages.iter().any(|l| l == lang) {
            eprintln!(
                "error: --lang `{lang}` is not a declared language (world.json declares: {:?}); \
                 `en` is always available",
                campaign.world.content.languages
            );
            return ExitCode::from(1);
        }
        let Some(doc) = sidecars.get(lang) else {
            eprintln!("error: no l10n/{lang}.json sidecar for declared language `{lang}`");
            return ExitCode::from(1);
        };
        // Swap every player-visible string to the target language, then record the
        // sidecar as a build input (manifest provenance) for the non-en build.
        delvewright_dsl::localize(&mut campaign, &doc.content);
    }
    let build_lang = if is_english { None } else { Some(lang) };
    // i18n v2 (spec-0029): the DEFAULT build ships every declared language and lets
    // the client choose, so each authored string travels to emission carrying its
    // l10n key and becomes `{"translate": key, "fallback": english}`. A `--lang`
    // bake is the unchanged single-language artifact (spec-0029 §4): its strings
    // were already swapped above, so it is emitted as literals exactly as before.
    if is_english {
        delvewright_dsl::tag_translatables(&mut campaign);
    }

    // The one caller of `Plan::build_with` outside a test, and the ordinary arm
    // is still `Plan::build` — the constructor that passes `Perturb::none()` as
    // a literal — so a build with no `--perturb` reaches the derivation through
    // exactly the code path it always did.
    let built = match &kind {
        BuildKind::Ship(_) => Plan::build(&campaign, &prefabs),
        BuildKind::Demonstrate(knob, p) => {
            eprintln!(
                "perturbed build: the derivation is asked for `{}` ({}{}); the observer this \
                 defect is documented to redden is {}. This run writes NO output tree.",
                knob.name(),
                knob.blurb(),
                p.place().map(|n| format!(" at `{n}`")).unwrap_or_default(),
                knob.documented_code(),
            );
            Plan::build_with(&campaign, &prefabs, p.clone())
        }
    };
    let plan = match built {
        Ok(p) => p,
        Err(e) => {
            // Advisories raised before the failure and explaining it (`DW0498`:
            // the pool draw behind an ambiguous-anchor `DW0305`) print first —
            // the cause above the symptom.
            print_diags(&Fenced::apply(&campaign, e.warnings), json);
            print_build_error(e.failure.code, &e.failure.message, json);
            return ExitCode::from(3);
        }
    };

    // read the structure .nbt bytes referenced by placements
    let structures = match read_structures(&plan, &prefabs, prefabs_dir, json) {
        Ok(s) => s,
        Err(code) => return ExitCode::from(code),
    };

    let skins = match read_skins(campaign_dir, &campaign, json) {
        Ok(s) => s,
        Err(code) => return ExitCode::from(code),
    };

    let tree = CommandTree::v1_21_11();
    let output = match emit::build_with_warnings(
        &plan,
        &loaded.inputs,
        &structures,
        &tree,
        &prefabs,
        build_lang,
        &skins,
    ) {
        Ok((o, warnings)) => {
            // Advisory build-tier findings (stage-7 edit replay: DW0353/DW0354).
            // Printed exactly like the validation-tier warnings, and like them
            // they never change the exit code.
            print_diags(&Fenced::apply(&campaign, warnings), json);
            o
        }
        Err(emit::BuildFailure::Validation(errors)) => {
            eprintln!(
                "build failure: {} emitted command(s) failed validation:",
                errors.len()
            );
            for e in errors.iter().take(20) {
                eprintln!("  {}: {}", e.reason, e.line);
            }
            return ExitCode::from(3);
        }
        Err(emit::BuildFailure::Diagnostic { code, message }) => {
            // The tier is the code's own ([`ExitTier`]) — an analysis-tier
            // refusal (exit 2) says the CONTENT is the defect, a build-tier one
            // (exit 3) says the compiler could not produce a tree. It used to be
            // re-derived here from the code's spelling, in a copy of the same
            // expression this verb, `edit` and `edit --check` each kept.
            print_build_error(code, &message, json);
            if let BuildKind::Demonstrate(knob, _) = &kind {
                let got = code.to_string();
                eprintln!(
                    "perturbed build: the observer REFUSED the derivation's `{}` defect. The \
                     build stops at the FIRST refusal, {got}{}. No output tree was written.",
                    knob.name(),
                    if got == knob.documented_code() {
                        ", which is the code this knob is documented to redden".to_string()
                    } else {
                        format!(
                            "; this knob is documented to redden {} — the battery's refusal \
                             line above names every rule that saw the defect, and one \
                             derivation defect is routinely seen by two of them",
                            knob.documented_code()
                        )
                    }
                );
            }
            return ExitCode::from(code.exit_tier().exit_status());
        }
    };

    // **The demonstration's own verdict, and the only place a perturbed run can
    // reach with a datapack in hand.** Getting here means the derivation built
    // the named defect and every observer passed it — which is the failure the
    // whole facility exists to be able to see, so it is a refusal rather than a
    // success, and the output is dropped unwritten. `BuildKind::Demonstrate`
    // carries no path, so this is a statement about what did not happen rather
    // than a decision not to do it.
    let out = match kind {
        BuildKind::Ship(out) => out,
        BuildKind::Demonstrate(knob, p) => {
            eprintln!(
                "perturbed build: the derivation was asked for `{}`{} and produced a datapack that \
                 NOTHING refused — {} did not fire, and neither did any other build-tier \
                 check. Either this campaign has nothing for that defect to damage, or an \
                 observer that claims to read the built bytes is reciting the arithmetic that \
                 laid them. The output was discarded; a perturbed run has no `--out` to write \
                 to.",
                knob.name(),
                p.place().map(|n| format!(" at `{n}`")).unwrap_or_default(),
                knob.documented_code(),
            );
            return ExitCode::from(3);
        }
    };

    if let Err(e) = write_output(out, &output) {
        eprintln!("internal error: cannot write output: {e}");
        return ExitCode::from(EXIT_INTERNAL);
    }
    ExitCode::SUCCESS
}

/// Which of the two builds this invocation is, or why it is neither.
///
/// `clap` already refuses `--out` beside `--perturb` and refuses a knob it does
/// not know, so what is left here is the arity — `--perturb` is a `Vec` so that
/// a second one is an ERROR rather than the silent last-wins a `Option` would
/// give, which on a defect-injection flag is the shape that reports a
/// demonstration of the knob nobody asked about — and the place, which three of
/// the knobs need and the other three refuse.
fn resolve_build_kind<'a>(
    out: Option<&'a Path>,
    perturb: &[Knob],
    place: Option<&str>,
) -> Result<BuildKind<'a>, String> {
    let knob = match perturb {
        [] => {
            let out = out.ok_or_else(|| {
                "`--out` is required for a build that is not perturbed".to_string()
            })?;
            if place.is_some() {
                return Err(
                    "`--perturb-place` names the place `--perturb` damages, and this \
                            run asks for no perturbation"
                        .to_string(),
                );
            }
            return Ok(BuildKind::Ship(out));
        }
        [one] => *one,
        many => {
            return Err(format!(
                "`--perturb` takes exactly one knob and this run names {}: {}. One defect per \
                 run is what makes the refusal attributable — two at once and the code that \
                 fires says nothing about which defect it saw",
                many.len(),
                many.iter().map(|k| k.name()).collect::<Vec<_>>().join(", ")
            ));
        }
    };
    match (knob.takes_place(), place) {
        (true, None) => Err(format!(
            "`--perturb {}` damages ONE place and this run names none — add `--perturb-place \
             <place>`",
            knob.name()
        )),
        (false, Some(p)) => Err(format!(
            "`--perturb {}` damages the whole derivation, not one place, so `--perturb-place \
             {p}` would decide nothing. The knobs that take a place are: {}",
            knob.name(),
            Knob::ALL
                .iter()
                .filter(|k| k.takes_place())
                .map(|k| k.name())
                .collect::<Vec<_>>()
                .join(", ")
        )),
        (_, p) => Ok(BuildKind::Demonstrate(
            knob,
            knob.perturb(p)
                .expect("the arity was just checked against `takes_place`"),
        )),
    }
}

/// Read the skin PNGs every staged **body** references (spec-0009 bake). The PNG
/// lives in the campaign dir at `skins/<texture_id>.png`; a missing one is a
/// build error (`DW0309`), not a silent skip. Shared by `build` and by `edit`'s
/// build-tier proof run — the editor must prove exactly what `build` proves.
///
/// **Enumerated from [`delvewright_dsl::body_skin_sites`], never from one
/// stage's list.** This walked `campaign.npcs.content.npcs` by hand, so a
/// stage-5 actor's skin was read into its summon
/// (`profile:{texture:"delvewright:npc/<id>"}`), shipped in a resource pack that
/// carried no such texture, and refused by nothing: deleting an npc's PNG exited
/// 3 with `DW0309` while deleting an actor's built green. A skin is a property
/// of a body, so the walk is over bodies.
///
/// One texture is read once however many bodies name it — a character and the
/// puppet that plays it are one face.
fn read_skins(
    campaign_dir: &Path,
    campaign: &delvewright_dsl::Campaign,
    json: bool,
) -> Result<BTreeMap<String, Vec<u8>>, u8> {
    let mut skins: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for site in delvewright_dsl::body_skin_sites(campaign) {
        if skins.contains_key(&site.skin.texture_id) {
            continue;
        }
        let path = campaign_dir
            .join("skins")
            .join(format!("{}.png", site.skin.texture_id));
        match std::fs::read(&path) {
            Ok(bytes) => {
                skins.insert(site.skin.texture_id.clone(), bytes);
            }
            Err(e) => {
                print_build_error(
                    DW_SKIN_PNG_MISSING,
                    &format!(
                        "cannot read skin PNG `{}`: {e} — `{}` declares this `skin.texture_id` \
                         at `{}` `{}`, but the campaign has no matching \
                         `skins/<texture_id>.png`. A body that declares a skin ships as a \
                         mannequin pointing at `delvewright:npc/{}`, and the resource pack is \
                         where that texture comes from. Add the PNG at that path, or remove \
                         the `skin`",
                        path.display(),
                        site.body.id(),
                        site.body.stage(),
                        site.path,
                        site.skin.texture_id,
                    ),
                    json,
                );
                return Err(3);
            }
        }
    }
    Ok(skins)
}

/// `delvec edit apply|preview` (spec-0017): the edit → replay → snapshot loop.
///
/// Replays the stage-7 edit script — with an optional `--batch` candidate
/// appended — through full validation, the deterministic replay with its
/// per-batch invariant proofs, and one auto-rendered snapshot per batch
/// (framing the batch's edited region over the final edited world). `apply`
/// additionally persists the candidate into `world-edits.json` (canonical
/// form) once the whole replay is green; `preview` never writes to the
/// campaign directory. Editing sessions leave no state outside the script.
///
/// **One proof tier, not two** (map-editor audit finding 3). The per-batch
/// invariants are a *subset* of what `build` proves — they cover gravity,
/// relight, critical-path/checkpoint walkability and boundary safety, but not
/// cutscene clipping (`DW0308`), stealth zones (`DW0327`), trap completability
/// (`DW0342`), wave seating (`DW0312`), `move-npc`/`move-actor` routability, or
/// the exported-route self-check. `apply` used to persist on that subset, so a
/// script `build` rejects could be written into the campaign. Both verbs now run
/// the **whole** build-tier proof set (`analyze` + `emit::build`, output
/// discarded) before anything is persisted; measured cost is ~0.3 s on the
/// largest content campaign against a ~0.34 s snapshot render, so there is no
/// reason for a cheaper tier to exist.
fn run_edit(
    campaign_dir: &Path,
    prefabs_dir: &Path,
    batch: Option<&Path>,
    out_dir: &Path,
    persist: bool,
    json: bool,
) -> ExitCode {
    use delvewright_compiler::load::WORLD_EDITS_FILE;
    use delvewright_compiler::snapshot::{
        self, Camera, DEFAULT_FOV, DEFAULT_HEIGHT, DEFAULT_WIDTH,
    };

    let mut loaded = match load_or_refuse(campaign_dir, json) {
        Ok(l) => l,
        Err(exit) => return ExitCode::from(exit),
    };

    // Append the candidate batch to the (possibly absent) stage-7 document, in
    // memory only — nothing touches the campaign dir unless the replay is green
    // AND this is `apply`.
    if let Some(bpath) = batch {
        let src = match std::fs::read_to_string(bpath) {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "internal error: cannot read --batch {}: {e}",
                    bpath.display()
                );
                return ExitCode::from(EXIT_INTERNAL);
            }
        };
        let candidate: delvewright_dsl::EditBatch = match serde_json::from_str(&src) {
            Ok(b) => b,
            Err(e) => {
                print_build_error(
                    delvewright_dsl::codes::SCHEMA,
                    &format!(
                        "--batch {} is not a stage-7 `EditBatch` object: {e}. Run `delvec \
                         schema --stage 7` for the exact shape (the file holds ONE batch \
                         object, not a whole world-edits document)",
                        bpath.display()
                    ),
                    json,
                );
                return ExitCode::from(1);
            }
        };
        let mut env: delvewright_dsl::Envelope<delvewright_dsl::WorldEditsContent> =
            match &loaded.raw.world_edits {
                Some(s) => match serde_json::from_str(s) {
                    Ok(env) => env,
                    Err(e) => {
                        print_build_error(
                            delvewright_dsl::codes::SCHEMA,
                            &format!(
                                "existing world-edits.json does not parse: {e} — fix it before \
                                 appending batches"
                            ),
                            json,
                        );
                        return ExitCode::from(1);
                    }
                },
                None => {
                    let world: serde_json::Value =
                        serde_json::from_str(&loaded.raw.world).unwrap_or_default();
                    let cid = world
                        .get("campaign_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    delvewright_dsl::Envelope {
                        dsl_version: delvewright_dsl::SUPPORTED_DSL_VERSION.to_string(),
                        campaign_id: delvewright_dsl::CampaignId(cid),
                        stage: Stage::WorldEdits,
                        content: delvewright_dsl::WorldEditsContent {
                            batches: Vec::new(),
                        },
                    }
                }
            };
        env.content.batches.push(candidate);
        let script = match delvewright_dsl::to_canonical_string(&env) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("internal error: cannot serialize world-edits: {e}");
                return ExitCode::from(EXIT_INTERNAL);
            }
        };
        loaded
            .inputs
            .insert(WORLD_EDITS_FILE.to_string(), script.clone().into_bytes());
        loaded.raw.world_edits = Some(script);
    }
    let augmented_script = loaded.raw.world_edits.clone();

    let v = match validate_loaded(loaded, prefabs_dir, json) {
        Ok(v) => v,
        Err(code) => return ExitCode::from(code),
    };
    if has_error(&v.diags) {
        return ExitCode::from(1);
    }
    let plan = match Plan::build(&v.campaign, &v.prefabs) {
        Ok(p) => p,
        Err(e) => {
            // Advisories raised before the failure and explaining it (`DW0498`:
            // the pool draw behind an ambiguous-anchor `DW0305`) print first —
            // the cause above the symptom.
            print_diags(&Fenced::apply(&v.campaign, e.warnings), json);
            print_build_error(e.failure.code, &e.failure.message, json);
            return ExitCode::from(3);
        }
    };
    let structures = match read_structures(&plan, &v.prefabs, prefabs_dir, json) {
        Ok(s) => s,
        Err(code) => return ExitCode::from(code),
    };

    let replay = match delvewright_compiler::edit::replay(&plan, &v.prefabs, &structures) {
        Ok(r) => r,
        Err(e) => {
            print_build_error(e.code, &e.message, json);
            return ExitCode::from(e.code.exit_tier().exit_status());
        }
    };
    let Some(replay) = replay else {
        println!("no edit batches — nothing to replay (add one with `--batch <file>`)");
        return ExitCode::SUCCESS;
    };

    // One snapshot per batch: frame the batch's edited region over the FINAL
    // edited world (a dollhouse view pulled into open air, like `--at`).
    let grid = snapshot::VoxelGrid::build(&replay.assembled.blocks);
    let targets = snapshot::collect_targets(&plan);
    // The solved layout, hoisted out of the per-batch loop: it is a property of
    // the plan, identical in every batch's manifest.
    let pieces = snapshot::collect_pieces(&plan);
    let opts = snapshot::FrameOpts {
        width: DEFAULT_WIDTH,
        height: DEFAULT_HEIGHT,
        sea_level: sea_level_of(&v.campaign),
        labels: true,
    };
    let mut shots: Vec<(String, String)> = Vec::new(); // (batch id, png path)
    for b in &replay.batches {
        let Some((lo, hi)) = b.bounds else { continue };
        let centre = [
            (lo[0] + hi[0]) as f64 / 2.0,
            (lo[1] + hi[1]) as f64 / 2.0,
            (lo[2] + hi[2]) as f64 / 2.0,
        ];
        let span = ((hi[0] - lo[0]).max(hi[1] - lo[1]).max(hi[2] - lo[2])) as f64;
        let d = (span * 1.1).max(12.0);
        let eye = [
            centre[0] + d * 0.75,
            centre[1] + d * 0.65,
            centre[2] + d * 0.75,
        ];
        let cam = Camera::looking_at(pull_into_open_air(&grid, centre, eye), centre, DEFAULT_FOV);

        let mut frame = snapshot::render_frame(&grid, &cam, &opts);
        let (inside, outside) = snapshot::resolve_targets(&grid, &cam, &opts, &targets);
        snapshot::draw_labels(&mut frame, &grid, &cam, &inside);
        let png = delvewright_compiler::png::encode_rgba(
            frame.canvas.width,
            frame.canvas.height,
            &frame.canvas.rgba,
        );
        let name = b.id.strip_prefix("batch/").unwrap_or(&b.id);
        let image_name = format!("{name}.png");
        let png_path = out_dir.join(&image_name);
        let doc = snapshot::manifest(
            v.campaign.world.campaign_id.as_str(),
            &image_name,
            &cam,
            &opts,
            &grid,
            snapshot::Scene {
                pieces: &pieces,
                inside: &inside,
                outside: &outside,
            },
            &frame.canvas,
        );
        let mut manifest_bytes = match serde_json::to_vec_pretty(&doc) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("internal error: cannot serialize manifest: {e}");
                return ExitCode::from(EXIT_INTERNAL);
            }
        };
        manifest_bytes.push(b'\n');
        if let Err(e) = write_file(&png_path, &png)
            .and_then(|()| write_file(&manifest_path_for(&png_path), &manifest_bytes))
        {
            eprintln!("internal error: cannot write snapshot: {e}");
            return ExitCode::from(EXIT_INTERNAL);
        }
        shots.push((b.id.clone(), png_path.display().to_string()));
    }

    // ---- the FULL build-tier proof set (map-editor audit finding 3) ----
    // Everything `delvec build` proves, over the same edited model: the DW02xx
    // reachability analysis, then `emit::build` (cutscene clip DW0308, stealth
    // zones DW0327, trap completability DW0342, wave seating DW0312, move
    // routability DW0307/DW0325, exported-route + POV self-checks
    // DW0314/DW0724, entry anchor DW0345, and the emitted-command validator).
    // Output is discarded — this run exists purely so `edit` can never accept a
    // script `build` would reject.
    let adiags = Fenced::apply(&v.campaign, analyze_campaign(&v.campaign, &v.prefabs));
    if !adiags.reported().is_empty() {
        print_diags(&adiags, json);
        return ExitCode::from(2);
    }
    let tree = CommandTree::v1_21_11();
    let skins = match read_skins(campaign_dir, &v.campaign, json) {
        Ok(s) => s,
        Err(code) => return ExitCode::from(code),
    };
    match emit::build_with_warnings(
        &plan,
        &v.loaded.inputs,
        &structures,
        &tree,
        &v.prefabs,
        None,
        &skins,
    ) {
        Ok((_, warnings)) => print_diags(&Fenced::apply(&v.campaign, warnings), json),
        Err(emit::BuildFailure::Validation(errors)) => {
            eprintln!(
                "build failure: {} emitted command(s) failed validation:",
                errors.len()
            );
            for e in errors.iter().take(20) {
                eprintln!("  {}: {}", e.reason, e.line);
            }
            return ExitCode::from(3);
        }
        Err(emit::BuildFailure::Diagnostic { code, message }) => {
            print_build_error(code, &message, json);
            return ExitCode::from(code.exit_tier().exit_status());
        }
    }

    // Persist the accepted candidate — `apply` only, and only now that the full
    // build-tier proof set is green. Written tmp-then-rename: a crash or a full
    // disk mid-write must never leave the campaign's script truncated, and
    // `world-edits.json` is the artifact of record (ADR-0006).
    let persisted = persist && batch.is_some();
    if persisted && let Some(script) = &augmented_script {
        let final_path = campaign_dir.join(WORLD_EDITS_FILE);
        let tmp_path = campaign_dir.join(format!("{WORLD_EDITS_FILE}.tmp"));
        if let Err(e) =
            std::fs::write(&tmp_path, script).and_then(|()| std::fs::rename(&tmp_path, &final_path))
        {
            let _ = std::fs::remove_file(&tmp_path);
            eprintln!("internal error: cannot write {WORLD_EDITS_FILE}: {e}");
            return ExitCode::from(EXIT_INTERNAL);
        }
    }

    if json {
        println!(
            "{}",
            serde_json::json!({
                "batches": replay.batches.iter().map(|b| &b.id).collect::<Vec<_>>(),
                "commands": replay.commands.len(),
                "snapshots": shots.iter().map(|(id, p)| serde_json::json!({
                    "batch": id, "png": p,
                })).collect::<Vec<_>>(),
                "persisted": persisted,
            })
        );
    } else {
        for (id, path) in &shots {
            println!("{id} → {path}");
        }
        println!(
            "{} batch(es) replayed green, {} runtime command(s){}",
            replay.batches.len(),
            replay.commands.len(),
            if persisted {
                format!(
                    " — persisted to {}",
                    campaign_dir.join(WORLD_EDITS_FILE).display()
                )
            } else {
                String::new()
            }
        );
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

/// `delvec fmt [--check] <path>…` — canonical form for authored Delvewright
/// JSON.
///
/// A formatter AND a check, deliberately in that order: a `--check`-only gate
/// makes authors hand-sort a 900-key sidecar, which nobody does twice, so the
/// gate ends up waived. `cargo fmt` is the shape that works.
///
/// Exit codes: `0` clean · `1` something is unformatted (`--check`), unparseable
/// (`DW0770`/`DW0771`), or matched nothing (`DW0774`) · `10` an I/O failure.
///
/// Every run states its binding count — how many files it examined. Zero is a
/// FINDING, not a pass (CLAUDE.md: a green gate that binds to nothing is
/// vacuous), because the way this gate dies quietly is a path that stops
/// matching after a directory is renamed.
fn run_fmt(paths: &[PathBuf], check: bool, json: bool) -> ExitCode {
    use delvewright_dsl::fmt;

    let mut files: Vec<PathBuf> = Vec::new();
    for root in paths {
        match fmt::discover(root) {
            Ok(found) => files.extend(found),
            Err(e) => {
                eprintln!("internal error: cannot read `{}`: {e}", root.display());
                return ExitCode::from(EXIT_INTERNAL);
            }
        }
    }
    files.sort();
    files.dedup();

    let mut diags: Vec<Diagnostic> = Vec::new();
    let mut changed: Vec<PathBuf> = Vec::new();

    for path in &files {
        let shown = path.display().to_string();
        let original = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("internal error: cannot read `{shown}`: {e}");
                return ExitCode::from(EXIT_INTERNAL);
            }
        };
        let formatted = match fmt::format_text(&original) {
            Ok(s) => s,
            Err(e) => {
                diags.push(Diagnostic::error(
                    e.code,
                    "fmt",
                    format!("{shown}:{}:{}", e.line, e.col),
                    e.message,
                ));
                continue;
            }
        };
        if formatted == original {
            continue;
        }
        changed.push(path.clone());
        if check {
            diags.push(Diagnostic::error(
                fmt::DW_FMT_UNFORMATTED,
                "fmt",
                shown.clone(),
                format!(
                    "not in canonical form (first difference at line {}). \
                     Run `delvec fmt {shown}`.",
                    first_differing_line(&original, &formatted)
                ),
            ));
        } else if let Err(e) = std::fs::write(path, &formatted) {
            eprintln!("internal error: cannot write `{shown}`: {e}");
            return ExitCode::from(EXIT_INTERNAL);
        }
    }

    for d in &diags {
        print_one_diag(d, json);
    }

    // Vacuity: a formatter that formatted nothing because it found nothing is
    // not a pass, and this is exactly how the CI gate would rot — a renamed
    // fixture directory, a path that no longer exists.
    if files.is_empty() {
        let d = Diagnostic::error(
            fmt::DW_FMT_NO_BINDING,
            "fmt",
            paths
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", "),
            "matched 0 JSON files. A formatter or a --check that binds to nothing is \
             vacuous, not a pass: check the paths (a `delvec build` output tree, a \
             dot-directory and a symlinked directory are all skipped deliberately)."
                .to_string(),
        );
        print_one_diag(&d, json);
        return ExitCode::from(1);
    }

    let unreadable = diags.len() - if check { changed.len() } else { 0 };
    if check {
        eprintln!(
            "delvec fmt --check: examined {} file(s); {} not in canonical form, {} unparseable",
            files.len(),
            changed.len(),
            unreadable
        );
    } else {
        eprintln!(
            "delvec fmt: examined {} file(s); reformatted {}, {} unparseable",
            files.len(),
            changed.len(),
            unreadable
        );
    }

    if diags.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

/// 1-based line of the first difference, so `--check` points an author at a
/// place rather than at a file.
fn first_differing_line(a: &str, b: &str) -> usize {
    for (i, (la, lb)) in a.lines().zip(b.lines()).enumerate() {
        if la != lb {
            return i + 1;
        }
    }
    a.lines().count().min(b.lines().count()) + 1
}

fn print_one_diag(d: &Diagnostic, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string(d).expect("diagnostic serializes")
        );
    } else {
        println!("{} [error] {} {}: {}", d.code, d.stage, d.path, d.message);
    }
}

/// `delvec allocation` — the handing (spec-0050 §4).
///
/// Refuses without a passed, fresh walk record, and the refusal is the same
/// `DW0841` validation raises: the two events that begin detail work are
/// obtaining an allocation and compiling a binding, and both are bound. There
/// is no third, because no other verb reads a `detail-plan`.
/// `delvec allocation` — the handing (spec-0050 §4).
///
/// Refuses without a passed, fresh walk record, and the refusal is the same
/// `DW0841` validation raises: the two events that begin detail work are
/// obtaining an allocation and compiling a binding, and both are bound. There is
/// no third, because no other verb reads a `detail-plan`.
///
/// **Stdout carries the allocation and nothing else** on the success path, so an
/// authoring loop can redirect it. What it prints is an input to nothing — see
/// the note inside.
///
/// It takes no prefab directory, and that is the signature saying what the verb
/// is: an allocation is what the WHOLE hands a place, computed from the site plan
/// and the metrics table. Nothing about it depends on which pieces exist, which
/// is why it can be asked for before the piece is built — which is the only
/// moment it is any use.
fn run_allocation(campaign_dir: &Path, place: Option<&str>, all: bool, json: bool) -> ExitCode {
    let loaded = match load_or_refuse(campaign_dir, json) {
        Ok(l) => l,
        Err(exit) => return ExitCode::from(exit),
    };
    // Parsed rather than fully validated, on the precedent `l10n-inventory`
    // sets: this verb's stdout is a machine-readable document an authoring loop
    // reads, and `print_diags` writes to stdout. Nothing is lost by it — an
    // allocation is derived from the plan on every invocation and is an input to
    // NOTHING, so a stale or wrong one has no vector into the build; the frame
    // is recomputed and re-judged by `DW0843` at every validation. `delvec
    // validate` is the verb that says what a campaign's state is.
    let campaign = match parse_campaign(&loaded.raw) {
        Ok(c) => c,
        Err(diags) => {
            print_diags(&Fenced::structural(diags), json);
            return ExitCode::from(1);
        }
    };
    if campaign.site_plan.is_none() {
        eprintln!(
            "error: `{}` carries no `site-plan.json`. An allocation is what the WHOLE hands a \
             place, so there is nothing to hand out until the whole exists.",
            campaign_dir.display()
        );
        return ExitCode::from(1);
    }
    // **The gate, at the second of the two events that begin detail work.** It is
    // asked of the campaign as it stands, so a campaign with no `detail-plan`
    // yet — which is exactly the campaign asking for its first allocation — is
    // asked the same question against the plan whose hash it names.
    if let Some(d) =
        delvewright_compiler::detail::allocation_walk_gate(&campaign, loaded.walk_record.as_deref())
    {
        print_one_diag(&d, json);
        return ExitCode::from(1);
    }
    let out = if all {
        serde_json::to_value(delvewright_compiler::detail::allocations(&campaign))
    } else {
        let Some(place) = place else {
            eprintln!("error: name a place (`node/<kebab>`), or pass `--all`");
            return ExitCode::from(EXIT_INTERNAL);
        };
        let id = delvewright_dsl::NodeId(place.to_string());
        match delvewright_compiler::detail::allocation(&campaign, &id) {
            Some(a) => serde_json::to_value(a),
            None => {
                eprintln!(
                    "error: the plan allocates no box to `{place}` — run with `--all` to see \
                     every place it does, or `delvec validate` if you expected one here"
                );
                return ExitCode::from(1);
            }
        }
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&out.expect("an allocation serializes")).unwrap()
    );
    ExitCode::SUCCESS
}

fn run_schema(stage: &str) -> ExitCode {
    // EVERY stage answers to its own name, the one `DW0100` prints when that
    // stage's document will not parse: a refusal that names `site-plan` and
    // then tells the author to run `--stage <1..7>` has sent them somewhere
    // their document is not. The names come from `Stage::ALL` rather than a
    // second hand-written list, so a stage added later answers here the day it
    // exists (`Stage::ALL`'s own doc comment says why).
    if let Some(s) = Stage::ALL.iter().find(|s| s.name() == stage) {
        println!(
            "{}",
            serde_json::to_string_pretty(&stage_schema(*s)).unwrap()
        );
        return ExitCode::SUCCESS;
    }
    let stages = match stage {
        // The numbered spelling of the campaign DSL's seven staged documents
        // (ADR-0002). The map-pipeline documents are NAMED and never numbered
        // into this sequence (spec-0049): that sequence is the campaign DSL's
        // staging and this is a different pipeline, so a number would assert an
        // ordering between the two that does not exist.
        "1" => vec![Stage::World],
        "2" => vec![Stage::Npcs],
        "3" => vec![Stage::Classes],
        "4" => vec![Stage::QuestPlan],
        "5" => vec![Stage::Quests],
        "6" => vec![Stage::Dialogue],
        "7" => vec![Stage::WorldEdits],
        // `walk-record.json` is not a stage document and has no `Stage` — it is
        // a campaign artifact recording an event (see `detail::walk_record_schema`).
        // It is reachable here anyway because this is the command an author is
        // told to run to see the shape of a document they must write, and the
        // walk record is one of those. The schema says what it is, so the tool
        // and the reference document agree rather than the flag's name deciding.
        "walk-record" => {
            println!(
                "{}",
                serde_json::to_string_pretty(&delvewright_compiler::detail::walk_record_schema())
                    .unwrap()
            );
            return ExitCode::SUCCESS;
        }
        "all" => Stage::ALL.to_vec(),
        other => {
            let names: Vec<String> = Stage::ALL
                .iter()
                .map(|s| format!("`{}`", s.name()))
                .collect();
            eprintln!(
                "unknown document `{other}`. Want `1`..`7` (the campaign DSL's numbered \
                 stages), any stage by name — {names} — `walk-record` for the hand-written \
                 walk record, or `all` for every stage document at once.",
                names = names.join(", "),
            );
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

/// Export the metrics standard (spec-0049 §2 — pipeline stage 0).
///
/// The table on stdout, so a tool outside the engine reads the JSON and never a
/// copy; the verdicts on stderr, so a shell pipeline gets clean JSON.
///
/// Three things are stated on stderr every run, and each is stated whether or
/// not it found anything, because a count only means something when the run that
/// found nothing prints it too:
///
/// 1. **What the table holds** — entries per half, and how many building entries
///    the metrics gym has not walked.
/// 2. **What the self-check bound to** — invariants evaluated and building
///    entries read. A run that evaluated zero invariants would be a vacuous pass
///    and is refused as an internal error, not reported as green.
/// 3. **`DW0813`**, when any verdict above rested on an uncalibrated standard.
///    This is the code's live binding at this version: no *document* reads a
///    building metric until the layout-graph and site-plan stages land, and the
///    table proving itself consistent is a real verdict resting on real seeds.
///
/// An inconsistent table exits `EXIT_INTERNAL` rather than raising a diagnostic.
/// A diagnostic is addressed to an author, and there is no author here — the
/// table is engine data, so a table that contradicts itself is a defect in
/// `dsl::metrics` and the person who has to act on it is whoever is holding the
/// compiler.
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
        let gym = delvewright_compiler::gym::generate(&table, "metrics-gym");
        if let Err(e) = delvewright_compiler::gym::write(&gym, dir) {
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

/// Print a `DW03xx` build/solver diagnostic (exit 3), honoring `--json`. Mirrors
/// the spec-0002 one-object-per-line JSON shape used for validation diagnostics.
fn print_build_error(code: DwCode, message: &str, json: bool) {
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

/// State the **binding count** of the layout-graph checks on stderr (spec-0049
/// §3.3: *every check states its binding count*).
///
/// Printed on every validate, analyze and build of a campaign that carries
/// either map-pipeline document, and printed whether or not anything was found —
/// a count only means something when the run that found nothing prints it too.
/// A campaign with neither document prints nothing at all, which is a different
/// fact from a graph that bound to zero of everything and is the reason the two
/// are distinguishable here rather than collapsed into one silence.
///
/// A **zero on a graph that exists is a finding**, and the two zeroes that can
/// occur are named rather than counted: a graph with no beats is the *graph
/// before mission* case (`DW0817` says so in its own line, at analysis tier),
/// and a graph with no traversal edges is a set of places with no space between
/// them, which no later check can catch because every one of them quantifies
/// over edges.
///
/// stderr, not stdout: `--json` reserves stdout for one diagnostic object per
/// line, and this is not a diagnostic — nothing here is wrong.
/// **The run's binding counts, under one heading, after the author's lines.**
///
/// Every count a check owes its reader is still stated, zeroes included — that
/// is the vacuity rule and nothing here relaxes it. What changed is where they
/// sit: computed as each pass ran, they printed before the first refusal, so a
/// run that refused one thing opened with four to six paragraphs about what it
/// had examined. They are the last thing a run says now, and they say it under a
/// heading so a reader can see where the answers to their own question ended.
fn report_binding_notes(campaign: &delvewright_dsl::Campaign, collected: &[String]) {
    let mut buf: Vec<String> = collected.to_vec();
    layout_binding_lines(campaign, &mut buf);
    if buf.is_empty() {
        return;
    }
    eprintln!("-- what this run examined ({} line(s))", buf.len());
    for line in &buf {
        eprintln!("{line}");
    }
}

/// The layout, plan and brief binding counts, appended rather than printed —
/// [`report_binding_notes`] is the one site that emits them, so there is one
/// place that decides when a reader sees them.
fn layout_binding_lines(campaign: &delvewright_dsl::Campaign, out: &mut Vec<String>) {
    if campaign.layout_graph.is_none()
        && campaign.geometry_brief.is_none()
        && campaign.site_plan.is_none()
    {
        return;
    }
    let b = delvewright_dsl::LayoutBinding::of(campaign);
    out.push(b.line());
    if campaign.site_plan.is_some() {
        out.push(b.plan_line());
        if b.plan.views == 0 {
            out.push(
                "site-plan binding 0: this plan names no view, so the walk has no declared \
                 vantage to judge the silhouette from and the render beside the reference sheet \
                 has nothing to frame. The plan still builds; what is missing is the picture the \
                 whole was supposed to be looked at in."
                    .to_string(),
            );
        }
        if b.plan.volumes == 0 {
            out.push(
                "site-plan binding 0: this plan declares no whole-owned volume, so the check \
                 that keeps the whole's mass out of the places examined nothing. A map made \
                 only of rooms is a legitimate map; a map with a mountain in it that forgot to \
                 say so is not, and nothing else would notice."
                    .to_string(),
            );
        }
    }
    if campaign.layout_graph.is_some() && b.traversal_edges == 0 {
        out.push(
            "layout-graph binding 0: this graph declares no traversal connection at all, so \
             every check over edges above examined nothing. A set of places with no space \
             between them is a finding, not a graph that happens to be simple."
                .to_string(),
        );
    }
    if campaign.layout_graph.is_some() && b.spine_beats == 0 {
        out.push(
            "layout-graph binding 0: no beat of this graph belongs to a quest the finale depends \
             on, so `DW0817`'s obligation to visit the mission on the way to the goal examined \
             nothing. A critical path over an unbound graph is a route through nothing."
                .to_string(),
        );
    }
    if campaign.geometry_brief.is_some() && b.brief_facts == 0 {
        out.push(
            "geometry-brief binding 0: this brief states no fact, so there is nothing for a \
             site plan's identities to bind the map to."
                .to_string(),
        );
    }
}

/// State the obligation fence's **binding count** on stderr: how many findings
/// this campaign's declared `dsl_version` excused it from, and which rules they
/// belonged to.
///
/// A fence nobody can see is indistinguishable from a check nobody wrote
/// (CLAUDE.md: a green gate that binds to nothing is vacuous, not a pass). This
/// is the line that turns "the campaign is green" into "the campaign is green,
/// and here is what it is not yet answerable for" — the input to the adoption
/// round its next `dsl_version` bump owes.
///
/// stderr, not stdout: `--json` reserves stdout for one diagnostic object per
/// line, and this is not a diagnostic — nothing here is wrong.
fn report_grandfathered(diags: &Fenced) {
    let held = diags.grandfathered();
    if held.is_empty() {
        return;
    }
    let mut by_code: BTreeMap<&str, usize> = BTreeMap::new();
    for d in held {
        *by_code.entry(d.code.as_str()).or_default() += 1;
    }
    let summary: Vec<String> = by_code.iter().map(|(c, n)| format!("{c} x{n}")).collect();
    eprintln!(
        "obligation fence: {} finding(s) grandfathered by this campaign's declared dsl_version \
         ({}). They become live when the stage that owns them adopts the version that introduced \
         them.",
        held.len(),
        summary.join(", ")
    );
}

/// Print a fenced diagnostic list, honoring `--json`.
///
/// It takes a [`Fenced`], not a `Vec<Diagnostic>`, and that is the point: the
/// obligation fence is not a step somebody has to remember to run before
/// reporting, it is the only way to obtain a value this function accepts
/// (`delvewright_dsl::fence`).
/// **Author-actionable first.**
///
/// A run's diagnostics reached the terminal in the order the passes happened to
/// produce them, which put four to six paragraphs of advisory ahead of the one
/// line the author was there to act on: on every site-plan run, `DW0813` (the
/// engine's own metric table is provisional), `DW0822` (a pacing figure that
/// "carries NO threshold and refuses nothing") and the per-run binding lines all
/// printed before the first refusal.
///
/// So the list is grouped before it is printed — refusals, then advisories about
/// this campaign, then notices about this engine — by
/// [`delvewright_dsl::Diagnostic::group`], the one authority on that order. The
/// sort is STABLE, so within a group nothing moves and every pass's own ordering
/// survives; nothing is dropped, and the grouping applies to `--json` too,
/// because a consumer reading the first line should get the actionable one for
/// the same reason a person should.
///
/// Headings are human-output only: `--json` is one JSON object per line and
/// stays that way (spec-0002).
fn print_diags(diags: &Fenced, json: bool) {
    let mut ordered: Vec<&delvewright_dsl::Diagnostic> = diags.reported().iter().collect();
    ordered.sort_by_key(|d| d.group());

    let mut heading: Option<delvewright_dsl::Group> = None;
    for d in &ordered {
        if json {
            println!("{}", serde_json::to_string(d).unwrap());
            continue;
        }
        let g = d.group();
        if heading != Some(g) {
            let n = ordered.iter().filter(|x| x.group() == g).count();
            println!("{}", g.heading(n));
            heading = Some(g);
        }
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
