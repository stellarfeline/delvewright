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
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_compiler::{DELVEC_VERSION, DSL_VERSION, MC_VERSION};
use delvewright_dsl::{
    Diagnostic, Severity, Stage, parse_campaign, stage_schema, validate_campaign_with,
};

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
        /// Output tree directory.
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Emit the l10n key inventory (key → canonical English) as JSON, with the
    /// existing `--lang` sidecar and NPC persona context — the machine-readable
    /// input for translation tooling (`tools/i18n-translate.py`, docs/reference/i18n.md).
    L10nInventory {
        /// Campaign directory.
        campaign_dir: PathBuf,
    },
    /// Export a stage's JSON Schema (LLM authoring aid).
    Schema {
        /// Stage `1..7` or `all`.
        #[arg(long)]
        stage: String,
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
    /// Convert a harvested `rehearsal-report.json` (spec-0019) into per-shot
    /// `anchor + offset` DSL patches. Reads only the report and the creator
    /// overlay's `layout.json` — no campaign, no build, no world assembly.
    Calibrate {
        /// The harvested rehearsal report (`delve-harvest --rehearsal-out`).
        report: PathBuf,
        /// The creator overlay's layout manifest, which carries the
        /// resolved-anchor vocabulary to snap onto.
        #[arg(long)]
        layout: PathBuf,
        /// Where to write the patch document (`-` for stdout).
        #[arg(short, long, default_value = "shot-patch.json")]
        out: String,
    },
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
        Command::Build { campaign_dir, out } => {
            run_build(campaign_dir, out, &cli.prefabs, &cli.lang, cli.json)
        }
        Command::L10nInventory { campaign_dir } => {
            run_l10n_inventory(campaign_dir, &cli.lang, cli.json)
        }
        Command::Schema { stage } => run_schema(stage),
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
                     `delve-harvest` when a playtest session fired `/trigger dw.done`; \
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
                     `delve-harvest` over the session log; do NOT edit the report by hand.",
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
                 `delve-harvest`; do NOT edit the version field.",
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
    diags: Vec<Diagnostic>,
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

/// Validate and return the parsed campaign + shared context (prefabs, loaded dir,
/// l10n sidecars) + diagnostics; prints diagnostics. Returns `Err(exit)` on
/// internal error.
fn validate_stage(campaign_dir: &Path, prefabs_dir: &Path, json: bool) -> Result<Validated, u8> {
    let loaded = load_campaign_dir(campaign_dir).map_err(|e| {
        eprintln!("internal error: cannot read campaign dir: {e}");
        EXIT_INTERNAL
    })?;
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
            // v0.6 sound + art-title surface (spec-0014): sound-event ids
            // (DW0326), the deferred `play-sound at: actor` gate (DW0335), and
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
            // spec-0025 (DSL v0.8): branch-complete narrative verification. Every
            // declared branch is enumerated and every static proof re-run under
            // its flag assignment — terminality, cast continuity, exclusive-content
            // leakage, hard event contradictions — plus the forcing function that
            // every story node says what it does to the story. No-op below 0.8.0.
            diags.extend(delvewright_compiler::branch::check_branches(&campaign));
            print_diags(&diags, json);
            Ok(Validated {
                campaign,
                prefabs,
                loaded,
                sidecars,
                diags,
            })
        }
        Err(diags) => {
            print_diags(&diags, json);
            Err(1)
        }
    }
}

/// Whether any diagnostic is a hard rejection. Warnings (`Severity::Warning`) are
/// printed like errors but never fail a run: they flag things the compiler cannot
/// decide with certainty (e.g. `DW0330`, where the true limit depends on the
/// player's window size and GUI scale), so failing on them would dress a judgement
/// call as a fact. Every `Severity::Error` still exits non-zero exactly as before.
fn has_error(diags: &[Diagnostic]) -> bool {
    diags.iter().any(|d| d.severity == Severity::Error)
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
    let adiags = analyze_campaign(&v.campaign, &v.prefabs);
    print_diags(&adiags, json);
    if adiags.is_empty() {
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
    let loaded = match load_campaign_dir(campaign_dir) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("internal error: cannot read campaign dir: {e}");
            return ExitCode::from(EXIT_INTERNAL);
        }
    };
    let campaign = match parse_campaign(&loaded.raw) {
        Ok(c) => c,
        Err(diags) => {
            print_diags(&diags, json);
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
            print_build_error(e.code, &e.message, json);
            return ExitCode::from(3);
        }
    };
    let structures = match read_structures(&plan, &prefabs, prefabs_dir, json) {
        Ok(s) => s,
        Err(code) => return ExitCode::from(code),
    };

    let started = std::time::Instant::now();
    let blocks = match edited_assembled(&plan, &prefabs, &structures, json) {
        Ok(a) => a.blocks,
        Err(code) => return ExitCode::from(code),
    };
    let grid = snapshot::VoxelGrid::build(&blocks);
    let assembled_ms = started.elapsed().as_secs_f64() * 1000.0;

    let cam = match resolve_camera(&plan, &prefabs, &structures, &grid, &args) {
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
    match delvewright_compiler::horizon::base_of(campaign) {
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
    // Placed pieces, plus any structure a stage-7 `fragment` verb stamps that
    // no piece placed (spec-0017 PR 2) — the replay needs those bytes too.
    let mut files: Vec<String> = Vec::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            files.push(piece.structure_file.clone());
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
                        "DW0300",
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
    structures: &BTreeMap<String, Vec<u8>>,
    grid: &delvewright_compiler::snapshot::VoxelGrid,
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
        return camera_from_shot(plan, prefabs, structures, id);
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
/// an unobstructed line back to `subject`, sampled every [`PULL_STEP`] blocks.
///
/// This is what makes `--at` usable on interiors: a fire pit 14 blocks inside a
/// mountain has no exterior vantage, so the requested distance is honoured only
/// as far as the rock allows and the camera then sits in the room with its
/// subject. Falls back to `eye` when even the subject's own cell is solid (a
/// marker embedded in a wall — worth seeing as such).
fn pull_into_open_air(
    grid: &delvewright_compiler::snapshot::VoxelGrid,
    subject: [f64; 3],
    eye: [f64; 3],
) -> [f64; 3] {
    /// Sampling step, in blocks, for the pull-in walk.
    const PULL_STEP: f64 = 0.5;
    let d = [
        eye[0] - subject[0],
        eye[1] - subject[1],
        eye[2] - subject[2],
    ];
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if len < 1e-6 {
        return eye;
    }
    let dir = [d[0] / len, d[1] / len, d[2] / len];
    let cell_of = |p: [f64; 3]| {
        [
            p[0].floor() as i32,
            p[1].floor() as i32,
            p[2].floor() as i32,
        ]
    };
    let mut best = eye;
    let mut found = false;
    let mut t = 0.0;
    while t <= len + 1e-9 {
        let p = [
            subject[0] + dir[0] * t,
            subject[1] + dir[1] * t,
            subject[2] + dir[2] * t,
        ];
        if grid.solid(cell_of(p)) {
            break; // the rock starts here; keep the last open sample
        }
        best = p;
        found = true;
        t += PULL_STEP;
    }
    if found { best } else { eye }
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
    structures: &BTreeMap<String, Vec<u8>>,
    id: &str,
) -> Result<delvewright_compiler::snapshot::Camera, String> {
    use delvewright_compiler::render_plan;
    use delvewright_compiler::snapshot::{Camera, DEFAULT_FOV};

    let pov = if id.starts_with("pov/") {
        let world = delvewright_compiler::nav::World::from_plan(plan, structures);
        let routes = delvewright_compiler::nav::critical_path_routes(plan, &world);
        render_plan::pov_shots(plan, &routes)
    } else {
        Vec::new()
    };
    let doc = render_plan::render_plan(plan, prefabs, &pov);
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
            print_build_error(e.code, &e.message, json);
            return ExitCode::from(3);
        }
    };
    let structures = match read_structures(&plan, &prefabs, prefabs_dir, json) {
        Ok(s) => s,
        Err(code) => return ExitCode::from(code),
    };

    let started = std::time::Instant::now();
    let assembled = match edited_assembled(&plan, &prefabs, &structures, json) {
        Ok(a) => a,
        Err(code) => return ExitCode::from(code),
    };
    let world = delvewright_compiler::nav::World::from_occupancy(
        delvewright_compiler::assembled::occupancy_of(
            assembled.blocks.clone(),
            &assembled.open_gates,
        ),
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

fn run_build(
    campaign_dir: &Path,
    out: &Path,
    prefabs_dir: &Path,
    lang: &str,
    json: bool,
) -> ExitCode {
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
        mut loaded,
        sidecars,
        ..
    } = v;
    let adiags = analyze_campaign(&campaign, &prefabs);
    if !adiags.is_empty() {
        print_diags(&adiags, json);
        return ExitCode::from(2);
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
        if let Some(bytes) = loaded.l10n.get(lang) {
            loaded
                .inputs
                .insert(format!("l10n/{lang}.json"), bytes.clone());
        }
    }
    let build_lang = if is_english { None } else { Some(lang) };

    let plan = match Plan::build(&campaign, &prefabs) {
        Ok(p) => p,
        Err(e) => {
            print_build_error(e.code, &e.message, json);
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
    let content_sha = resolve_content_sha();
    let output = match emit::build_with_warnings(
        &plan,
        &loaded.inputs,
        &structures,
        &tree,
        &prefabs,
        build_lang,
        &content_sha,
        &skins,
    ) {
        Ok((o, warnings)) => {
            // Advisory build-tier findings (stage-7 edit replay: DW0353/DW0354).
            // Printed exactly like the validation-tier warnings, and like them
            // they never change the exit code.
            print_diags(&warnings, json);
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
            // Analysis-tier build diagnostics (exit 2, like DW02xx reachability): a
            // content/prefab defect the author fixes in the content, not a
            // compiler/geometry defect. These are the DW02xx lighting codes
            // (DW0210/DW0211, spec-0010), wave-capacity DW0312 (task #41: a wave too
            // big for its room), and DW0313 (task #42: a gravity floor that despawns
            // into the void — fix the prefab with a substrate). Geometry/navigation
            // diagnostics (DW0307/DW0308/DW0311) print like a solver DW03xx error and
            // exit 3.
            print_build_error(code, &message, json);
            let analysis_tier = code.starts_with("DW02")
                || code == emit::DW_WAVE_NO_ROOM
                || code == delvewright_compiler::assembled::DW_GRAVITY_DESPAWN
                || code == delvewright_compiler::nav::DW_TRAP_LETHAL_UNAVOIDABLE;
            let exit = if analysis_tier { 2 } else { 3 };
            return ExitCode::from(exit);
        }
    };

    if let Err(e) = write_output(out, &output) {
        eprintln!("internal error: cannot write output: {e}");
        return ExitCode::from(EXIT_INTERNAL);
    }
    ExitCode::SUCCESS
}

/// Read the NPC-skin PNGs referenced by mannequin NPCs (spec-0009 bake). The PNG
/// lives in the campaign dir at `skins/<texture_id>.png`; a missing one is a
/// build error (`DW0309`), not a silent skip. Shared by `build` and by `edit`'s
/// build-tier proof run — the editor must prove exactly what `build` proves.
fn read_skins(
    campaign_dir: &Path,
    campaign: &delvewright_dsl::Campaign,
    json: bool,
) -> Result<BTreeMap<String, Vec<u8>>, u8> {
    let mut skins: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for npc in &campaign.npcs.content.npcs {
        let Some(skin) = &npc.skin else { continue };
        if skins.contains_key(&skin.texture_id) {
            continue;
        }
        let path = campaign_dir
            .join("skins")
            .join(format!("{}.png", skin.texture_id));
        match std::fs::read(&path) {
            Ok(bytes) => {
                skins.insert(skin.texture_id.clone(), bytes);
            }
            Err(e) => {
                print_build_error(
                    "DW0309",
                    &format!(
                        "cannot read skin PNG `{}`: {e} — a mannequin npc declares this \
                         `skin.texture_id` but the campaign has no matching \
                         `skins/<texture_id>.png`. Add the PNG at that path, or remove the \
                         npc's `skin`",
                        path.display()
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

    let mut loaded = match load_campaign_dir(campaign_dir) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("internal error: cannot read campaign dir: {e}");
            return ExitCode::from(EXIT_INTERNAL);
        }
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
            print_build_error(e.code, &e.message, json);
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
            // Same tier mapping as `build` (analysis-tier content defects → 2).
            let analysis_tier = e.code.starts_with("DW02")
                || e.code == emit::DW_WAVE_NO_ROOM
                || e.code == delvewright_compiler::assembled::DW_GRAVITY_DESPAWN
                || e.code == delvewright_compiler::nav::DW_TRAP_LETHAL_UNAVOIDABLE;
            return ExitCode::from(if analysis_tier { 2 } else { 3 });
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
    let adiags = analyze_campaign(&v.campaign, &v.prefabs);
    if !adiags.is_empty() {
        print_diags(&adiags, json);
        return ExitCode::from(2);
    }
    let tree = CommandTree::v1_21_11();
    let skins = match read_skins(campaign_dir, &v.campaign, json) {
        Ok(s) => s,
        Err(code) => return ExitCode::from(code),
    };
    let content_sha = resolve_content_sha();
    match emit::build_with_warnings(
        &plan,
        &v.loaded.inputs,
        &structures,
        &tree,
        &v.prefabs,
        None,
        &content_sha,
        &skins,
    ) {
        Ok((_, warnings)) => print_diags(&warnings, json),
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
            let analysis_tier = code.starts_with("DW02")
                || code == emit::DW_WAVE_NO_ROOM
                || code == delvewright_compiler::assembled::DW_GRAVITY_DESPAWN
                || code == delvewright_compiler::nav::DW_TRAP_LETHAL_UNAVOIDABLE;
            return ExitCode::from(if analysis_tier { 2 } else { 3 });
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

/// The pinned content-repo SHA stamped into `manifest.json` (spec-0007 Step 0).
///
/// Read from `versions.toml` `[content].sha` — the value pinned in the repo, NOT
/// live git state — so the build stays deterministic and offline (ADR-0006:
/// same DSL + same seed + same content_sha → byte-identical output). We walk up
/// from the current directory to find `versions.toml` (delvec normally runs from
/// the repo root); if none is found, or it declares no `[content].sha`, the SHA
/// is reported as `"unpinned"`. Deliberately a plain line scan of the one key we
/// need — no TOML dependency, no absolute path in the output.
fn resolve_content_sha() -> String {
    fn find_versions_toml() -> Option<PathBuf> {
        let mut dir = std::env::current_dir().ok()?;
        loop {
            let candidate = dir.join("versions.toml");
            if candidate.is_file() {
                return Some(candidate);
            }
            if !dir.pop() {
                return None;
            }
        }
    }
    fn parse_content_sha(text: &str) -> Option<String> {
        let mut in_content = false;
        for raw in text.lines() {
            let line = raw.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            if line.starts_with('[') {
                in_content = line == "[content]";
                continue;
            }
            if in_content
                && let Some(rest) = line.strip_prefix("sha")
                && let Some(rest) = rest.trim_start().strip_prefix('=')
            {
                // strip any inline `# comment`, then surrounding quotes/space.
                let val = rest.split('#').next().unwrap_or(rest).trim();
                let val = val.trim_matches('"');
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
        None
    }
    find_versions_toml()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| parse_content_sha(&t))
        .unwrap_or_else(|| "unpinned".to_string())
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
        "7" => vec![Stage::WorldEdits],
        "all" => vec![
            Stage::World,
            Stage::Npcs,
            Stage::Classes,
            Stage::QuestPlan,
            Stage::Quests,
            Stage::Dialogue,
            Stage::WorldEdits,
        ],
        other => {
            eprintln!("unknown stage `{other}` (want 1..7 or all)");
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
