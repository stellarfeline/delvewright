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

use std::path::PathBuf;
use std::process::ExitCode;

mod cli;
mod detail;

use clap::{Parser, Subcommand};
use delvec::compiler::blockout::Knob;
use delvec::compiler::{DELVEC_VERSION, DSL_VERSION, MC_VERSION};

use crate::cli::campaign::{run_analyze, run_build, run_textures, run_validate};
use crate::cli::document::{run_allocation, run_codes, run_fmt, run_schema, schema_stage_help};
use crate::cli::edit::{EditAction, run_edit};
use crate::cli::l10n::{run_l10n_apply, run_l10n_inventory};
use crate::cli::metrics::{RigAction, run_calibrate, run_metrics, run_rig_describe};
use crate::cli::view::{
    SnapshotArgs, camera_stands, run_blocking_chart, run_cameras_preview, run_snapshot,
};

/// Internal-error exit code (spec-0002: ≥10).
pub(crate) const EXIT_INTERNAL: u8 = 10;

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
    /// Emit the l10n key inventory (key → canonical English) as JSON, each row
    /// with its kind of text, speaker, situation and the existing `--lang`
    /// translation (and whether it is stale), plus NPC persona context — the
    /// machine-readable input for transcreation (`tools/creator/i18n-translate.py`,
    /// docs/reference/i18n.md).
    L10nInventory {
        /// Campaign directory.
        campaign_dir: PathBuf,
    },
    /// Write the `--lang` l10n sidecar from a table of canonical English →
    /// translation (spec-0071 §4) — the verb a translating agent has instead of
    /// addressing positional keys it cannot derive.
    ///
    /// The keys are the tool's: every inventory row whose English the table
    /// carries is written with its `source`, rows already translated from
    /// unchanged English are kept, and the run ends by stating how many of the
    /// inventory's rows are translated, which English strings are still missing,
    /// and which table entries matched no row.
    L10nApply {
        /// Campaign directory.
        campaign_dir: PathBuf,
        /// A JSON object mapping canonical English to its translation. `-`
        /// reads it from stdin.
        #[arg(long, value_name = "FILE")]
        table: PathBuf,
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
        // Names come from `Stage::ALL`, so a stage added later is listed the day it exists.
        #[arg(long, help = schema_stage_help())]
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
        /// stair pitches and a designed fall of one low storey. Walking
        /// it is what retires `DW0813`. Nothing in it is authored geometry — it
        /// compiles through the ordinary derivation.
        #[arg(long, value_name = "DIR")]
        gym: Option<PathBuf>,
    },
    /// Every DW code this binary declares, one JSON object per line on stdout —
    /// `code`, `tier` (`Analysis`, `Build`, or null for a code of a verb with
    /// its own exit table), `subject`, the constant's `name` and `module` —
    /// sorted by code. The list is the registry every `dw_code!` declaration
    /// writes itself into, so it is what this binary can print.
    Codes,
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
        #[arg(long, default_value_t = delvec::compiler::snapshot::DEFAULT_WIDTH)]
        width: u32,
        /// Frame height in pixels.
        #[arg(long, default_value_t = delvec::compiler::snapshot::DEFAULT_HEIGHT)]
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
    /// vector into the build.
    Allocation {
        /// Campaign directory.
        campaign_dir: PathBuf,
        /// The place — a layout-graph node id (`node/<kebab>`).
        place: Option<String>,
        /// Every place of the plan, in document order.
        #[arg(long)]
        all: bool,
    },
    /// Detail a place inside the allocation the whole handed it (spec-0058):
    /// read the allocation, bind it into the place's program
    /// (`programs/<place stem>.json` in the campaign, under the `handed/`
    /// parameter prefix), expand at the frame, run every gate — the grammar's
    /// contract gates, `DW0843`–`DW0845`, the admission audit, the
    /// light probe — before any file is written, then freeze the piece into
    /// the prefab directory (`--prefabs`) with its gate report beside it and
    /// write the `details[]` row. The run ends by building the whole in
    /// memory, so traversal equivalence against the blockout is proved by the
    /// same observers `build` runs.
    ///
    /// Every input but the place is derived: the frame, the datum, the seams,
    /// the owed names, the palette, the piece id, the seed and the row are the
    /// tool's.
    Detail {
        /// Campaign directory.
        campaign_dir: PathBuf,
        /// The place — a layout-graph node id (`node/<kebab>`).
        place: Option<String>,
        /// Every place that has a program, in site-plan order, stopping at the
        /// first refusal with the place named.
        #[arg(long)]
        all: bool,
    },
    /// One comparison sheet per `world.textures[]` row (spec-0084 §5.2):
    /// vanilla's texture on the left and the campaign's on the right, scaled to
    /// the same width on a chequered ground that shows alpha. The review medium
    /// for a texture no frame this engine renders can show — a mob's skin, the
    /// moon. Reads the pinned client jar, which a build never does, so it is a
    /// verb of its own rather than a build output.
    Textures {
        /// Campaign directory.
        campaign_dir: PathBuf,
        /// Output directory; one `<id>.png` per row.
        #[arg(short, long, default_value = "review/textures")]
        out: PathBuf,
        /// The 1.21.11 client jar. Overrides the `$DELVEWRIGHT_CLIENT_JAR` /
        /// `~/.chunky` fallbacks.
        #[arg(long)]
        textures: Option<String>,
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
    View(delvec::compiler::view::cli::ViewCommand),
    /// Grammar programs: list the corpus, show or check one, expand it into a
    /// prefab, measure demonstration coverage, audit every program.
    Grammar(delvec::grammar::cli::GrammarArgs),
    /// A prefab piece under admission: audit, socket, anchor, lighting, catalog
    /// card, gallery world, curation.
    Prefab(delvec::admit::cli::PrefabArgs),
    /// Sculpt a prefab from a form document: a body stated as implicit solids
    /// over its own ground (spec-0087). `delvec schema --stage sculpt-form`
    /// prints the form's shape.
    Sculpt(delvec::sculpt::cli::SculptArgs),
    /// An outside schematic: convert a Sponge `.schem` into a structure `.nbt`.
    Schem(delvec::schem::cli::SchemArgs),
    /// A playtest log: pair `[DelveNote]` stamps with the creator's notes into
    /// `playtest-report.json` (and `[DelveShot]` stamps into a rehearsal report).
    Harvest(delvec::orchestrator::cli::HarvestArgs),
    /// GPU renders through Nucleation/wgpu: one piece's shot set, a whole
    /// library, or the missing-texture fidelity gate.
    Render(delvec::render::cli::RenderArgs),
    /// An assembly's rig (spec-0082): the parts and clips a generator wrote
    /// beside the prefab library, at `<prefabs>/rigs/<name>/rig.json`.
    Rig {
        #[command(subcommand)]
        action: RigAction,
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
        Command::L10nApply {
            campaign_dir,
            table,
        } => run_l10n_apply(campaign_dir, &cli.lang, table, cli.json),
        Command::Fmt { paths, check } => run_fmt(paths, *check, cli.json),
        Command::Schema { stage } => run_schema(stage),
        Command::Allocation {
            campaign_dir,
            place,
            all,
        } => run_allocation(campaign_dir, place.as_deref(), *all, cli.json),
        Command::Detail {
            campaign_dir,
            place,
            all,
        } => detail::run_detail(
            campaign_dir,
            place.as_deref(),
            *all,
            &cli.prefabs,
            &cli.lang,
            cli.json,
        ),
        Command::Metrics { gym } => run_metrics(cli.json, gym.as_deref()),
        Command::Codes => run_codes(),
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
        Command::Textures {
            campaign_dir,
            out,
            textures,
        } => run_textures(campaign_dir, out, textures.as_deref(), cli.json),
        Command::View(delvec::compiler::view::cli::ViewCommand::Cameras {
            build_dir,
            campaign,
            out,
            only,
            bracket,
            preview: true,
            ..
        }) => run_cameras_preview(
            build_dir,
            campaign,
            &cli.prefabs,
            out,
            only,
            bracket.as_ref(),
            cli.json,
        ),
        Command::View(delvec::compiler::view::cli::ViewCommand::Cameras {
            build_dir,
            campaign,
            out,
            only,
            bracket,
            draft,
            preview: false,
        }) => delvec::compiler::view::cli::run_cameras(
            build_dir,
            campaign,
            out,
            cli.json,
            delvec::compiler::view::camera::EmitOptions {
                world_paths: Default::default(),
                only: only.clone(),
                bracket: *bracket,
                draft: *draft,
            },
            &mut |cams| camera_stands(campaign, &cli.prefabs, cli.json, cams),
        ),
        Command::View(delvec::compiler::view::cli::ViewCommand::PlaceCamera {
            campaign,
            name,
            answers,
            report,
            slot,
            fov,
            candidates,
            pick,
            sky,
            after,
            delete,
        }) => delvec::compiler::view::cli::run_place_camera(
            campaign,
            name,
            answers.as_deref(),
            sky.as_deref(),
            after.as_deref(),
            delvec::compiler::view::cli::PlaceFrom {
                report: report.as_deref().zip(*slot).zip(*fov),
                candidates: candidates.as_deref().zip(pick.as_deref()),
                delete: *delete,
            },
            cli.json,
            &mut |cams| camera_stands(campaign, &cli.prefabs, cli.json, cams),
        ),
        Command::View(cmd) => cmd.run(cli.json),
        Command::Grammar(args) => delvec::grammar::cli::run(args.clone()),
        Command::Prefab(args) => delvec::admit::cli::run(args.clone(), &cli.prefabs, cli.json),
        Command::Sculpt(args) => delvec::sculpt::cli::run(args.clone()),
        Command::Schem(args) => delvec::schem::cli::run(args.clone(), cli.json),
        Command::Harvest(args) => delvec::orchestrator::cli::run(args.clone()),
        Command::Render(args) => delvec::render::cli::run(args.clone(), cli.json),
        Command::Rig {
            action: RigAction::Describe { rig, facing },
        } => run_rig_describe(rig, facing.facing(), &cli.prefabs, cli.json),
    }
}

#[cfg(test)]
mod tests {
    use clap::{Command, CommandFactory};

    use super::Cli;

    /// The whole mounted tree is one clap `Command`, and clap's own debug
    /// assertions are the authority on what it holds well-formed. They do NOT
    /// hold an id unique between a global flag and a subcommand's own argument
    /// — that mismatch is raised only when the derived `FromArgMatches` reads
    /// the subcommand's field, i.e. at the first parse that reaches the arm —
    /// so the test below asserts that property itself.
    #[test]
    fn the_whole_command_tree_is_well_formed() {
        Cli::command().debug_assert();
    }

    /// **One id per meaning across the whole mounted tree.** A global argument
    /// (`--json`, `--prefabs`, `--lang`, `--version`) is propagated into every
    /// subcommand's matches under its id; a subcommand that declares an
    /// argument of its own under the same id then reads one definition through
    /// the other's accessor, and clap panics with "Mismatch between definition
    /// and access" — at run time, in whichever CI step first runs that arm. The
    /// mounted crates declared their command lines before the global existed,
    /// so this is the shape a merge produces silently. Walked over every
    /// descendant, with the population printed so a zero cannot pass.
    #[test]
    fn no_subcommand_redeclares_a_global_argument_id() {
        let root = Cli::command();
        let globals: Vec<String> = root
            .get_arguments()
            .filter(|a| a.is_global_set())
            .map(|a| a.get_id().to_string())
            .collect();
        assert!(
            !globals.is_empty(),
            "the root declares no global argument — the walk would examine nothing"
        );

        fn walk(
            cmd: &Command,
            path: &str,
            globals: &[String],
            seen: &mut usize,
            hits: &mut Vec<String>,
        ) {
            for sub in cmd.get_subcommands() {
                let here = format!("{path} {}", sub.get_name());
                *seen += 1;
                for arg in sub.get_arguments() {
                    let id = arg.get_id().to_string();
                    if globals.contains(&id) && !arg.is_global_set() {
                        hits.push(format!(
                            "`delvec{here}` declares `{id}`, which is a global flag's id"
                        ));
                    }
                }
                walk(sub, &here, globals, seen, hits);
            }
        }
        let (mut seen, mut hits) = (0usize, Vec::new());
        walk(&root, "", &globals, &mut seen, &mut hits);
        assert!(
            seen >= 40,
            "only {seen} subcommand(s) walked — the tree is larger than that"
        );
        assert!(
            hits.is_empty(),
            "{} argument id(s) shadow a global over {seen} subcommand(s):\n  {}",
            hits.len(),
            hits.join("\n  ")
        );
    }
}
