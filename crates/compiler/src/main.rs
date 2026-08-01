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
            run_build(campaign_dir, out, &cli.prefabs, &cli.lang, cli.json)
        }
        Command::Schema { stage } => run_schema(stage),
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
            // i18n l10n sidecar coverage (DW0180/DW0181) — language-independent,
            // runs on every validate/analyze/build. No-op for English-only campaigns.
            let sidecars = parse_sidecars(&loaded.l10n, &mut diags);
            diags.extend(delvewright_dsl::validate_l10n(&campaign, &sidecars));
            // v0.6 sound + art-title surface (spec-0014): sound-event ids
            // (DW0326), the deferred `play-sound at: actor` gate (DW0335), and
            // art-title glyph coverage against the `delve:art` font over the source
            // text and every declared-language sidecar (DW0328). Validation-tier
            // (exit 1) — no-op for a campaign that uses neither surface.
            diags.extend(delvewright_compiler::atmos::check_sounds(&campaign));
            diags.extend(delvewright_compiler::atmos::check_art(&campaign, &sidecars));
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

fn run_validate(campaign_dir: &Path, prefabs_dir: &Path, json: bool) -> ExitCode {
    match validate_stage(campaign_dir, prefabs_dir, json) {
        Ok(v) if v.diags.is_empty() => ExitCode::SUCCESS,
        Ok(_) => ExitCode::from(1),
        Err(code) => ExitCode::from(code),
    }
}

fn run_analyze(campaign_dir: &Path, prefabs_dir: &Path, json: bool) -> ExitCode {
    let v = match validate_stage(campaign_dir, prefabs_dir, json) {
        Ok(v) => v,
        Err(code) => return ExitCode::from(code),
    };
    if !v.diags.is_empty() {
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
    if !v.diags.is_empty() {
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
    // Determine the night-vision `DW0210` mitigation verdict on the canonical
    // English campaign, before any localization swaps the kit-item display names it
    // reads. Threading this into the build keeps the lighting gate language-
    // independent (ADR-0006): the same campaign cannot pass `en` and fail `zh-cn`.
    let night_vision = delvewright_compiler::light::has_night_vision(&campaign);
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
                        &format!(
                            "cannot read prefab structure file `{}`: {e} — the prefab metadata \
                             points at an `.nbt` that is missing or unreadable in the prefabs dir. \
                             Restore the file or fix the metadata path (prefab-library issue)",
                            path.display()
                        ),
                        json,
                    );
                    return ExitCode::from(3);
                }
            }
        }
    }

    // read the NPC-skin PNGs referenced by mannequin NPCs (spec-0009 bake). The
    // PNG lives in the campaign dir at `skins/<texture_id>.png`; a missing one is
    // a build error (DW0309), not a silent skip.
    let mut skins: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for npc in &campaign.npcs.content.npcs {
        if let Some(skin) = &npc.skin {
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
                    return ExitCode::from(3);
                }
            }
        }
    }

    let tree = CommandTree::v1_21_11();
    let content_sha = resolve_content_sha();
    let output = match emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &tree,
        &prefabs,
        build_lang,
        &content_sha,
        &skins,
        night_vision,
    ) {
        Ok(o) => o,
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
