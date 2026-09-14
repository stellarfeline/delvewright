//! The CPU render arms as `delvec` subcommands (ADR-0021 §1).
//!
//! `viewer`, `scene`, `panorama`, `contact-sheet`, `index` and `palette` are
//! declared here and flattened into the top level of the one binary — not
//! because rendering is a compiler concern, but because the surface a creator
//! installs is ONE binary. The drivers below read the modules under
//! [`crate::compiler::view`].
//!
//! **No cargo feature gates any of this.** A feature would ship a
//! same-name-different-capability binary — an artifact whose name promises a
//! surface its bytes may not carry — so the arms are unconditional code, and
//! `tools/build-release-binaries.sh` proves per target that the built binary's
//! own `--help` lists exactly the surface the source declares.
//!
//! The three arms that are NOT here — `piece`, `batch`, `fidelity-gate` — are
//! the ones that mesh and rasterise through `nucleation`/`wgpu`. They live in
//! `crates/delvec/src/render` and mount as `delvec render …` (ADR-0023 §3).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Subcommand;

use crate::compiler::view::assets::Assets;
use crate::compiler::view::blockcolor::Deriver;
use crate::compiler::view::camera::{self, Bracket, EmitOptions};
use crate::compiler::view::diag::{
    DW_BINDING, DW_INPUT, DW_OUTPUT, DW_RANK_ORDER, DW_RENDER, DW_UNDERSPECIFIED_STATE,
    DW_UNRESOLVED_BLOCK, DW_VIEWER_RESOURCES, Diagnostic, exit,
};
use crate::compiler::view::panorama::{self, Bearing, PanoramaOptions, Subject};
use crate::compiler::view::scene::{self, SceneOptions};
use crate::compiler::view::sheet::{self, ScoreSet, SheetOptions};
use crate::compiler::view::{cache, index, nbt, showing, viewer};

/// The options every render arm shares. On `delvec render` these were three
/// global flags; here they are declared by the arms that read them, so that
/// `delvec build --help` does not grow a `--textures` it has no use for.
pub struct ViewOpts {
    /// Emit diagnostics as one JSON object per line.
    pub json: bool,
    /// Resource pack for textures (the 1.21.11 client jar). Overrides the
    /// `$DELVEWRIGHT_CLIENT_JAR` / `~/.chunky` fallbacks.
    pub textures: Option<String>,
    /// Rendered frame dimension (square), in pixels.
    pub size: u32,
}

/// The default frame dimension, unchanged from `delvec render`.
pub const DEFAULT_SIZE: u32 = 1024;

#[derive(Subcommand)]
pub enum ViewCommand {
    /// Emit Chunky scene JSONs from a build output's `render-plan.json`.
    Scene {
        /// A `delvec build` output directory (containing `render-plan.json`).
        build_dir: PathBuf,
        /// Output directory for the scene JSONs.
        #[arg(short, long)]
        out: PathBuf,
        /// The world save the scenes load (default `<build-dir>/world`, where
        /// `validation/world-save.sh` writes it). Refused unless it holds
        /// `level.dat` and a region file; written into the scenes as an absolute
        /// path.
        #[arg(long)]
        world: Option<PathBuf>,
        /// Rendered frame dimension (square), in pixels.
        #[arg(long, default_value_t = DEFAULT_SIZE)]
        size: u32,
    },
    /// Emit a Chunky scene per showcase camera a campaign states in
    /// `design/cameras.json` — a camera on the assembled world, estimated from the
    /// approved image it answers or placed by hand — against a build's
    /// `render-plan.json`.
    Cameras {
        /// A `delvec build` output directory (containing `render-plan.json`).
        build_dir: PathBuf,
        /// The campaign directory: its `design/cameras.json` is read, and every
        /// camera's `answers` is held to its `design.json`.
        #[arg(long)]
        campaign: PathBuf,
        /// Output directory for the scene JSONs.
        #[arg(short, long)]
        out: PathBuf,
        /// The world save the scenes load (default `<build-dir>/world`, where
        /// `validation/world-save.sh` writes it). Refused unless it holds
        /// `level.dat` and a region file; written into the scenes as an absolute
        /// path.
        #[arg(long)]
        world: Option<PathBuf>,
        /// Only this camera, by name (repeatable).
        #[arg(long)]
        only: Vec<String>,
        /// Emit, beside each camera, the camera moved each way by each step —
        /// `yaw=8,pitch=4,fov=10,dolly=6,rise=3`, any subset — and write every
        /// candidate to `candidates.json` in the output directory, in the record
        /// format.
        #[arg(long, value_parser = Bracket::parse)]
        bracket: Option<Bracket>,
        /// Emit each frame at a quarter of its width and height and 16 samples,
        /// under its own `_draft` scene name: for judging what is in frame.
        #[arg(long, conflicts_with = "preview")]
        draft: bool,
        /// Write no scene: draw each camera (and candidate) on the CPU, flat-lit,
        /// at half its width and height, as `<stem>_preview.png` — seconds per
        /// frame, for placing a camera before a path tracer is asked about light.
        /// Reads the campaign and `--prefabs` to assemble the world.
        #[arg(long, conflicts_with = "world")]
        preview: bool,
    },
    /// Emit an oblique exterior scene of the delve's built place — the storybook
    /// shot — from a build output's `render-plan.json`. The camera frames the
    /// subject (the placed areas unless `--subject` names anchors); the ground a
    /// horizon built around them is loaded, not framed.
    Panorama {
        /// A `delvec build` output directory (containing `render-plan.json`).
        build_dir: PathBuf,
        /// Output directory for the scene JSON.
        #[arg(short, long)]
        out: PathBuf,
        /// The world save the scene loads (default `<build-dir>/world`, where
        /// `validation/world-save.sh` writes it). Refused unless it holds
        /// `level.dat` and a region file; written into the scene as an absolute
        /// path.
        #[arg(long)]
        world: Option<PathBuf>,
        /// Which side the camera stands on: a corner (se, sw, ne, nw) looks along
        /// the diagonal, a face (n, e, s, w) looks square at that side.
        #[arg(long, value_enum, default_value_t = Bearing::Se)]
        bearing: Bearing,
        /// Degrees the camera looks down: 45 is the oblique over the whole place,
        /// lower shows its walls in elevation, higher approaches a plan.
        #[arg(long, default_value_t = panorama::DEFAULT_PITCH_DEG,
              value_parser = clap::value_parser!(u8).range(
                  i64::from(*panorama::PITCH_RANGE.start())..=i64::from(*panorama::PITCH_RANGE.end())))]
        pitch: u8,
        /// Vertical field of view, degrees: a narrower frame stands further back,
        /// which is how a building is shown without wide-angle distortion.
        #[arg(long, default_value_t = panorama::FOV_DEG as u8,
              value_parser = clap::value_parser!(u8).range(
                  i64::from(*panorama::FOV_RANGE.start())..=i64::from(*panorama::FOV_RANGE.end())))]
        fov: u8,
        /// What the camera is fitted to: `layout` (the placed areas), or the
        /// building named by the anchors that stand in it —
        /// `anchor/stop-gate,anchor/stop-kitchen` — whose horizontal span is framed
        /// at the layout's full height.
        #[arg(long, default_value = "layout", value_parser = Subject::parse)]
        subject: Subject,
        /// Path-tracing sample target: ~64 for a draft, ~300 for release art.
        #[arg(long, default_value_t = panorama::DEFAULT_SPP)]
        spp: u32,
        /// Frame width, in pixels.
        #[arg(long, default_value_t = panorama::DEFAULT_WIDTH,
              value_parser = clap::value_parser!(u32).range(1..))]
        width: u32,
        /// Frame height, in pixels.
        #[arg(long, default_value_t = panorama::DEFAULT_HEIGHT,
              value_parser = clap::value_parser!(u32).range(1..))]
        height: u32,
    },
    /// Lay candidate renders out as ONE contact sheet for the owner to curate
    /// massing from (spec-0027 §3). With `--scores`, the similarity score
    /// ORDERS the page — it never removes a candidate (spec-0028 §3).
    ContactSheet {
        /// Directory of candidates: one subdirectory of renders per candidate
        /// (`delvec render batch` output), or a flat directory of `.png` renders.
        dir: PathBuf,
        /// Output PNG. The manifest naming every cell is always written beside
        /// it as `<stem>.json` — a sheet whose ranks cannot be resolved back to
        /// candidate ids is not a curation page.
        #[arg(short, long)]
        out: PathBuf,
        /// Similarity scores from `tools/refscore.py`. Absent → id order.
        #[arg(long)]
        scores: Option<PathBuf>,
        /// Representative shot per candidate in the per-directory layout
        /// (default `ext-se`, falling back to the first render by name). Given
        /// explicitly, a candidate missing that shot is an error, never a
        /// silent substitution of another angle.
        #[arg(long)]
        shot: Option<String>,
        /// Cells per row (default: the squarest page, `ceil(sqrt(n))`).
        #[arg(long)]
        columns: Option<u32>,
        /// Thumbnail side, in pixels.
        #[arg(long, default_value_t = 256)]
        thumb: u32,
        /// Header title.
        #[arg(long)]
        title: Option<String>,
    },
    /// Turn one or more prefabs into ONE self-contained interactive HTML page: a
    /// camera the reviewer drives, preset points of view including player eye
    /// height at the way in, and every block drawn from the pinned version's own
    /// model and textures.
    Viewer {
        /// Prefab `.nbt` files, tile-set manifests (`.json`), or directories of
        /// them. Each prefab's `<basename>.json` is read when present — that is
        /// where anchors live. A directory holding a tiled zone shows the zone,
        /// never its tiles.
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
        /// Output `.html`.
        #[arg(short, long)]
        out: PathBuf,
        /// Page title. Defaults to the prefab id, or a count when several.
        #[arg(long)]
        title: Option<String>,
        /// Resource pack for textures (the 1.21.11 client jar). Overrides the
        /// `$DELVEWRIGHT_CLIENT_JAR` / `~/.chunky` fallbacks.
        #[arg(long)]
        textures: Option<String>,
    },
    /// Derive the appearance table (colour, coverage and model bounds per
    /// blockstate) for some prefabs, as JSON — what a palette actually looks
    /// like, measured rather than recalled.
    Palette {
        /// Prefab `.nbt` files, or directories of them.
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
        /// Output `.json`.
        #[arg(short, long)]
        out: PathBuf,
        /// Biome whose tints are baked into the table.
        #[arg(long, default_value = crate::compiler::view::blockcolor::DEFAULT_BIOME)]
        biome: String,
        /// Resource pack for textures (the 1.21.11 client jar). Overrides the
        /// `$DELVEWRIGHT_CLIENT_JAR` / `~/.chunky` fallbacks.
        #[arg(long)]
        textures: Option<String>,
    },
    /// Emit a shot index (image ↔ expect pairs) from a build's `render-plan.json`,
    /// for handing shots to a reviewing agent / vision model.
    Index {
        /// A `delvec build` output directory (containing `render-plan.json`).
        build_dir: PathBuf,
        /// Output file for the shot index JSON.
        #[arg(short, long)]
        out: PathBuf,
    },
}

impl ViewCommand {
    /// Run the arm. `json` is `delvec`'s own global diagnostics flag, which the
    /// render arms answer to exactly as every other subcommand does.
    pub fn run(&self, json: bool) -> ExitCode {
        match self {
            ViewCommand::Scene {
                build_dir,
                out,
                world,
                size,
            } => run_scene(
                build_dir,
                out,
                world.as_deref(),
                &ViewOpts {
                    json,
                    textures: None,
                    size: *size,
                },
            ),
            ViewCommand::Cameras {
                build_dir,
                campaign,
                out,
                world,
                only,
                bracket,
                draft,
                preview,
            } if !*preview => run_cameras(
                build_dir,
                campaign,
                out,
                world.as_deref(),
                json,
                EmitOptions {
                    world_path: String::new(),
                    only: only.clone(),
                    bracket: *bracket,
                    draft: *draft,
                },
            ),
            ViewCommand::Cameras { .. } => fail(
                Diagnostic::error(
                    DW_INPUT,
                    "`delvec cameras --preview` assembles the world, and is run by the `delvec` \
                     binary rather than this arm",
                ),
                json,
                exit::INPUT,
            ),
            ViewCommand::Panorama {
                build_dir,
                out,
                world,
                bearing,
                pitch,
                fov,
                subject,
                spp,
                width,
                height,
            } => run_panorama(
                build_dir,
                out,
                world.as_deref(),
                json,
                PanoramaOptions {
                    world_path: String::new(),
                    width: *width,
                    height: *height,
                    spp_target: *spp,
                    bearing: *bearing,
                    pitch_deg: *pitch,
                    fov_deg: *fov,
                    subject: subject.clone(),
                },
            ),
            ViewCommand::ContactSheet {
                dir,
                out,
                scores,
                shot,
                columns,
                thumb,
                title,
            } => run_contact_sheet(
                dir,
                out,
                scores.as_deref(),
                shot.as_deref(),
                *columns,
                *thumb,
                title.as_deref(),
                &ViewOpts {
                    json,
                    textures: None,
                    size: DEFAULT_SIZE,
                },
            ),
            ViewCommand::Viewer {
                inputs,
                out,
                title,
                textures,
            } => run_viewer(
                inputs,
                out,
                title.as_deref(),
                &ViewOpts {
                    json,
                    textures: textures.clone(),
                    size: DEFAULT_SIZE,
                },
            ),
            ViewCommand::Palette {
                inputs,
                out,
                biome,
                textures,
            } => run_palette(
                inputs,
                out,
                biome,
                &ViewOpts {
                    json,
                    textures: textures.clone(),
                    size: DEFAULT_SIZE,
                },
            ),
            ViewCommand::Index { build_dir, out } => run_index(
                build_dir,
                out,
                &ViewOpts {
                    json,
                    textures: None,
                    size: DEFAULT_SIZE,
                },
            ),
        }
    }
}

/// Resolve the textures path from `--textures`, `$DELVEWRIGHT_CLIENT_JAR`, then
/// `~/.chunky/resources/minecraft.jar`. Returns the first that exists.
pub fn resolve_textures(textures: Option<&str>) -> Result<String, Diagnostic> {
    let mut candidates: Vec<String> = Vec::new();
    if let Some(t) = textures {
        candidates.push(t.to_string());
    }
    if let Ok(env) = std::env::var("DELVEWRIGHT_CLIENT_JAR") {
        candidates.push(env);
    }
    if let Ok(home) = std::env::var("HOME") {
        candidates.push(format!("{home}/.chunky/resources/minecraft.jar"));
    }
    for c in &candidates {
        if Path::new(c).exists() {
            return Ok(c.clone());
        }
    }
    Err(Diagnostic::error(
        DW_RENDER,
        "no textures found — pass --textures <1.21.11 client jar>, set \
         $DELVEWRIGHT_CLIENT_JAR, or place it at ~/.chunky/resources/minecraft.jar",
    ))
}

pub fn fail(d: Diagnostic, json: bool, code: u8) -> ExitCode {
    d.print(json);
    ExitCode::from(code)
}

/// Union block palette (block-state strings, sorted, deduped) of every shipped
/// structure `.nbt` under the build's `datapack/data/<ns>/structure/`. Consumed
/// only by the night-vision review emulation (`scene::scenes_from_plan`); an
/// absent structure tree yields an empty palette (harmless unless the plan
/// stamps a dark shot — then scene emission refuses, see `scene.rs`). An
/// *unreadable* structure is an error: a corrupt build dir must not silently
/// degrade the review.
fn structure_palette(build_dir: &Path) -> Result<Vec<String>, Diagnostic> {
    let data = build_dir.join("datapack").join("data");
    let mut nbts: Vec<PathBuf> = Vec::new();
    if let Ok(namespaces) = std::fs::read_dir(&data) {
        let mut dirs: Vec<PathBuf> = namespaces
            .filter_map(|e| e.ok().map(|e| e.path().join("structure")))
            .collect();
        dirs.sort();
        for dir in dirs {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            nbts.extend(
                entries
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("nbt")),
            );
        }
    }
    nbts.sort();
    let mut palette = std::collections::BTreeSet::new();
    for path in &nbts {
        let st = nbt::parse_structure(path).map_err(|e| {
            Diagnostic::error(DW_INPUT, format!("structure {}: {e}", path.display()))
        })?;
        palette.extend(st.palette);
    }
    Ok(palette.into_iter().collect())
}

fn run_scene(build_dir: &Path, out: &Path, world: Option<&Path>, vopts: &ViewOpts) -> ExitCode {
    let plan_path = build_dir.join("render-plan.json");
    let bytes = match std::fs::read(&plan_path) {
        Ok(b) => b,
        Err(e) => {
            return fail(
                Diagnostic::error(DW_INPUT, format!("read {}: {e}", plan_path.display())),
                vopts.json,
                exit::INPUT,
            );
        }
    };
    let world_path = match resolve_world(build_dir, world) {
        Ok(w) => w,
        Err(d) => return fail(d, vopts.json, exit::INPUT),
    };
    let opts = SceneOptions {
        world_path,
        width: vopts.size,
        height: vopts.size,
        spp_target: 500,
    };
    // The shipped structures' block palette feeds the dark-shot night-vision
    // review emulation (no-op for plans without dark-stamped shots).
    let palette = match structure_palette(build_dir) {
        Ok(p) => p,
        Err(d) => return fail(d, vopts.json, exit::INPUT),
    };
    let scenes = match scene::scenes_from_plan(&bytes, &opts, &palette) {
        Ok(s) => s,
        Err(d) => return fail(d, vopts.json, exit::INPUT),
    };
    let purged = match write_scenes(out, &scenes) {
        Ok(n) => n,
        Err((d, code)) => return fail(d, vopts.json, code),
    };
    eprintln!(
        "emitted {} Chunky scene(s) -> {} ({purged} stale cache file(s) purged; render with {}; \
         see README)",
        scenes.len(),
        out.display(),
        scene::CHUNKY_CORE
    );
    ExitCode::SUCCESS
}

/// Write scene JSONs into `out`, deleting each one's now-stale Chunky caches
/// (see `cache`: Chunky reuses `<scene>.octree2`/`.dump` silently, so a re-emitted
/// scene would render from the chunks and settings it just replaced). Returns
/// the number of cache files removed.
fn write_scenes(out: &Path, scenes: &[(String, Vec<u8>)]) -> Result<usize, (Diagnostic, u8)> {
    std::fs::create_dir_all(out).map_err(|e| {
        (
            Diagnostic::error(DW_OUTPUT, format!("mkdir {}: {e}", out.display())),
            exit::OUTPUT,
        )
    })?;
    let mut purged = 0usize;
    for (name, data) in scenes {
        std::fs::write(out.join(name), data).map_err(|e| {
            (
                Diagnostic::error(DW_OUTPUT, format!("write {name}: {e}")),
                exit::OUTPUT,
            )
        })?;
        purged += cache::purge_scene_caches(out, name)
            .map_err(|d| (d, exit::OUTPUT))?
            .len();
    }
    Ok(purged)
}

fn run_panorama(
    build_dir: &Path,
    out: &Path,
    world: Option<&Path>,
    json: bool,
    mut opts: PanoramaOptions,
) -> ExitCode {
    let plan_path = build_dir.join("render-plan.json");
    let bytes = match std::fs::read(&plan_path) {
        Ok(b) => b,
        Err(e) => {
            return fail(
                Diagnostic::error(DW_INPUT, format!("read {}: {e}", plan_path.display())),
                json,
                exit::INPUT,
            );
        }
    };
    opts.world_path = match resolve_world(build_dir, world) {
        Ok(w) => w,
        Err(d) => return fail(d, json, exit::INPUT),
    };
    // An anchor subject is resolved against the anchors this build placed; the
    // default subject reads nothing but the plan.
    let anchors = match &opts.subject {
        Subject::Layout => Vec::new(),
        Subject::Anchors(_) => {
            let path = build_dir.join("creator-datapack").join("layout.json");
            match std::fs::read(&path)
                .map_err(|e| Diagnostic::error(DW_INPUT, format!("read {}: {e}", path.display())))
                .and_then(|b| panorama::resolved_anchors(&b))
            {
                Ok(a) => a,
                Err(d) => return fail(d, json, exit::INPUT),
            }
        }
    };
    let pano = match panorama::panorama_from_plan(&bytes, &anchors, &opts) {
        Ok(s) => s,
        Err(d) => return fail(d, json, exit::INPUT),
    };
    let scene = (pano.file_name.clone(), pano.bytes.clone());
    let purged = match write_scenes(out, std::slice::from_ref(&scene)) {
        Ok(n) => n,
        Err((d, code)) => return fail(d, json, code),
    };
    let (smin, smax) = pano.subject;
    eprintln!(
        "emitted panorama {} -> {} ({purged} stale cache file(s) purged; {}x{}, {} spp, render with {}; \
         see README)",
        pano.file_name,
        out.display(),
        opts.width,
        opts.height,
        opts.spp_target,
        scene::CHUNKY_CORE
    );
    eprintln!(
        "subject: {}, box {smin:?}..{smax:?} — covers {:.0}% of the frame \
         ({:.0}% of its width x {:.0}% of its height) from bearing {} at {} degrees, fov {}. \
         That share is a floor, never the verdict: look at the frame",
        match &opts.subject {
            Subject::Layout => "the placed areas".to_string(),
            Subject::Anchors(names) => format!("the building {} anchor(s) stand in", names.len()),
        },
        pano.span.fill() * 100.0,
        pano.span.width * 100.0,
        pano.span.height * 100.0,
        opts.bearing.name(),
        opts.pitch_deg,
        opts.fov_deg
    );
    // The solved camera, in the record format: the frame is kept or refined by
    // copying this into `design/cameras.json` and naming the image it answers.
    match serde_json::to_string(&pano.record) {
        Ok(line) => eprintln!(
            "camera record (set `answers`, then keep or refine it in {}): {line}",
            camera::CAMERAS_FILE
        ),
        Err(e) => {
            return fail(
                Diagnostic::error(DW_OUTPUT, format!("serialize the camera record: {e}")),
                json,
                exit::OUTPUT,
            );
        }
    }
    ExitCode::SUCCESS
}

fn run_cameras(
    build_dir: &Path,
    campaign: &Path,
    out: &Path,
    world: Option<&Path>,
    json: bool,
    mut opts: EmitOptions,
) -> ExitCode {
    let read = |path: PathBuf| {
        std::fs::read(&path)
            .map_err(|e| Diagnostic::error(DW_INPUT, format!("read {}: {e}", path.display())))
    };
    let plan = match read(build_dir.join("render-plan.json")) {
        Ok(b) => b,
        Err(d) => return fail(d, json, exit::INPUT),
    };
    let sheet =
        match read(campaign.join(camera::CAMERAS_FILE)).and_then(|b| camera::parse_sheet(&b)) {
            Ok(s) => s,
            Err(d) => return fail(d, json, exit::INPUT),
        };
    let rows = match read(campaign.join("design.json")).and_then(|b| camera::reference_names(&b)) {
        Ok(r) => r,
        Err(d) => return fail(d, json, exit::INPUT),
    };
    let unanswered = match camera::bind_answers(&sheet, &rows) {
        Ok(u) => u,
        Err(d) => return fail(d, json, exit::INPUT),
    };
    opts.world_path = match resolve_world(build_dir, world) {
        Ok(w) => w,
        Err(d) => return fail(d, json, exit::INPUT),
    };
    let emission = match camera::emit(&plan, &sheet, &opts) {
        Ok(e) => e,
        Err(d) => return fail(d, json, exit::INPUT),
    };
    let purged = match write_scenes(out, &emission.scenes) {
        Ok(n) => n,
        Err((d, code)) => return fail(d, json, code),
    };
    if opts.bracket.is_some() {
        let path = out.join(camera::CANDIDATES_FILE);
        let written =
            camera::candidates_bytes(&sheet.campaign_id, &emission.cameras).and_then(|b| {
                std::fs::write(&path, b).map_err(|e| {
                    Diagnostic::error(DW_OUTPUT, format!("write {}: {e}", path.display()))
                })
            });
        if let Err(d) = written {
            return fail(d, json, exit::OUTPUT);
        }
    }
    let stated = if opts.only.is_empty() {
        sheet.cameras.len()
    } else {
        opts.only.len()
    };
    eprintln!(
        "emitted {} Chunky scene(s) for {stated} of {} stated camera(s) -> {} ({purged} stale cache \
         file(s) purged; {}render with {}){}",
        emission.scenes.len(),
        sheet.cameras.len(),
        out.display(),
        if opts.draft { "drafts, " } else { "" },
        scene::CHUNKY_CORE,
        if opts.bracket.is_some() {
            format!(
                "; every candidate is in {}",
                out.join(camera::CANDIDATES_FILE).display()
            )
        } else {
            String::new()
        }
    );
    eprintln!(
        "answers: {} of {} approved image(s) in design.json have a camera{}",
        rows.len() - unanswered.len(),
        rows.len(),
        if unanswered.is_empty() {
            String::new()
        } else {
            format!("; none answers {}", unanswered.join(", "))
        }
    );
    ExitCode::SUCCESS
}

/// The world save a scene names, as the absolute path Chunky will be handed.
///
/// Chunky resolves a scene's world path against the RENDERING process's working
/// directory, and renders a world it cannot find as an empty frame at exit 0 with
/// the reason inside a Java stack trace. `delvec build` writes no world — the
/// datapack stamps the geometry during a server boot, and
/// `validation/world-save.sh` copies that save to `<build-dir>/world`. So the
/// default is that directory, the path is made absolute here, and a directory
/// without `level.dat` and at least one region file is refused before a scene is
/// written.
fn resolve_world(build_dir: &Path, world: Option<&Path>) -> Result<String, Diagnostic> {
    let dir = world.map_or_else(|| build_dir.join("world"), Path::to_path_buf);
    let level = dir.join("level.dat").is_file();
    let regions = std::fs::read_dir(dir.join("region"))
        .map(|rd| {
            rd.filter_map(Result::ok)
                .filter(|e| e.path().extension().is_some_and(|x| x == "mca"))
                .count()
        })
        .unwrap_or(0);
    if !level || regions == 0 {
        return Err(Diagnostic::error(
            DW_INPUT,
            format!(
                "{} holds no world save (level.dat: {}, region files: {regions}). A scene names \
                 the world Chunky loads, `delvec build` writes none, and Chunky renders a \
                 missing world as an empty frame at exit 0. Boot the build once to write it: \
                 `EULA=TRUE \"$DELVEWRIGHT_ENGINE/validation/world-save.sh\" {} --project dw-<id>` \
                 writes `<build-dir>/world`, the default; `--world` names a save elsewhere",
                dir.display(),
                if level { "present" } else { "MISSING" },
                build_dir.display(),
            ),
        ));
    }
    let abs = std::path::absolute(&dir)
        .map_err(|e| Diagnostic::error(DW_INPUT, format!("resolve {}: {e}", dir.display())))?;
    Ok(abs.display().to_string())
}

fn run_index(build_dir: &Path, out: &Path, vopts: &ViewOpts) -> ExitCode {
    let plan_path = build_dir.join("render-plan.json");
    let bytes = match std::fs::read(&plan_path) {
        Ok(b) => b,
        Err(e) => {
            return fail(
                Diagnostic::error(DW_INPUT, format!("read {}: {e}", plan_path.display())),
                vopts.json,
                exit::INPUT,
            );
        }
    };
    let idx = match index::index_from_plan(&bytes) {
        Ok(b) => b,
        Err(d) => return fail(d, vopts.json, exit::INPUT),
    };
    if let Some(parent) = out.parent()
        && !parent.as_os_str().is_empty()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        return fail(
            Diagnostic::error(DW_OUTPUT, format!("mkdir {}: {e}", parent.display())),
            vopts.json,
            exit::OUTPUT,
        );
    }
    if let Err(e) = std::fs::write(out, &idx) {
        return fail(
            Diagnostic::error(DW_OUTPUT, format!("write {}: {e}", out.display())),
            vopts.json,
            exit::OUTPUT,
        );
    }
    eprintln!("wrote shot index -> {}", out.display());
    ExitCode::SUCCESS
}

/// Which exit code a contact-sheet diagnostic carries. `DW0725` is the
/// rank-never-gate guard tripping — a defect in the ordering itself, not in the
/// user's input, so it exits `10` (internal) rather than `2`.
fn sheet_exit(d: &Diagnostic) -> u8 {
    match d.code {
        DW_RANK_ORDER => exit::INTERNAL,
        DW_OUTPUT => exit::OUTPUT,
        DW_INPUT | DW_BINDING => exit::INPUT,
        _ => exit::INTERNAL,
    }
}

#[allow(clippy::too_many_arguments)]
fn run_contact_sheet(
    dir: &Path,
    out: &Path,
    scores_path: Option<&Path>,
    shot: Option<&str>,
    columns: Option<u32>,
    thumb: u32,
    title: Option<&str>,
    vopts: &ViewOpts,
) -> ExitCode {
    let scores = match scores_path.map(read_scores).transpose() {
        Ok(s) => s,
        Err(d) => return fail_sheet(d, vopts.json),
    };
    let (candidates, layout) = match sheet::discover(dir, shot) {
        Ok(c) => c,
        Err(d) => return fail_sheet(d, vopts.json),
    };
    let opts = SheetOptions {
        columns,
        thumb,
        // The directory's NAME, not its path: a long absolute path would push
        // every other header claim off the page, and the full path is in the
        // manifest's `source` anyway.
        title: title.map(str::to_string).unwrap_or_else(|| {
            format!(
                "contact sheet: {}",
                dir.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("candidates")
            )
        }),
    };
    let built = match sheet::build_sheet(
        dir,
        &candidates,
        scores.as_ref(),
        layout,
        shot,
        &opts,
        // The ordering seam. `rank_by_score` ORDERS; `build_sheet` then puts
        // whatever comes back through the total-order guard, so no ranker —
        // this one or a later one — can turn the score into a filter.
        sheet::rank_by_score,
    ) {
        Ok(s) => s,
        Err(d) => return fail_sheet(d, vopts.json),
    };
    for d in &built.diagnostics {
        d.print(vopts.json);
    }
    if let Some(parent) = out.parent()
        && !parent.as_os_str().is_empty()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        return fail(
            Diagnostic::error(DW_OUTPUT, format!("mkdir {}: {e}", parent.display())),
            vopts.json,
            exit::OUTPUT,
        );
    }
    if let Err(e) = built.image.save(out) {
        return fail(
            Diagnostic::error(DW_OUTPUT, format!("save {}: {e}", out.display())),
            vopts.json,
            exit::OUTPUT,
        );
    }
    let manifest_path = out.with_extension("json");
    if let Err(e) = std::fs::write(&manifest_path, &built.manifest) {
        return fail(
            Diagnostic::error(DW_OUTPUT, format!("write {}: {e}", manifest_path.display())),
            vopts.json,
            exit::OUTPUT,
        );
    }
    let b = &built.binding;
    eprintln!(
        "contact sheet: {} candidate(s), {} scored / {} unscored ({} unmatched score row(s)) \
         -> {} + {}",
        b.candidates,
        b.scored,
        b.unscored.len(),
        b.unmatched_score_rows.len(),
        out.display(),
        manifest_path.display()
    );
    ExitCode::SUCCESS
}

fn read_scores(path: &Path) -> Result<ScoreSet, Diagnostic> {
    let bytes = std::fs::read(path)
        .map_err(|e| Diagnostic::error(DW_INPUT, format!("read {}: {e}", path.display())))?;
    ScoreSet::parse(&bytes)
}

fn fail_sheet(d: Diagnostic, json: bool) -> ExitCode {
    let code = sheet_exit(&d);
    fail(d, json, code)
}

/// Collect prefab `.nbt` paths from files and/or directories, sorted by name so
/// a page built from a directory is the same page on every machine.
/// Resolve the paths an author passed into the pieces to show.
///
/// A file is taken as given — `tileset::load_piece` decides whether it is a
/// prefab, a manifest, or a lone tile of a set (which it refuses).
///
/// A DIRECTORY is where the care is. Walking `*.nbt` in a directory that holds a
/// tiled zone would put each tile on the page as if it were a prefab, which is
/// the same defect `piece` and `batch` each close on their own door: a page of a
/// building sliced at a packaging boundary is a review that passes and means
/// nothing. So the manifests are collected first and every `.nbt` they claim is
/// dropped in favour of its manifest.
fn collect_pieces(inputs: &[PathBuf]) -> Result<Vec<PathBuf>, Diagnostic> {
    let mut out: Vec<PathBuf> = Vec::new();
    for input in inputs {
        if !input.is_dir() {
            out.push(input.clone());
            continue;
        }
        let entries: Vec<PathBuf> = std::fs::read_dir(input)
            .map_err(|e| Diagnostic::error(DW_INPUT, format!("read dir {}: {e}", input.display())))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .collect();

        let mut manifests: Vec<PathBuf> = Vec::new();
        let mut claimed: std::collections::BTreeSet<PathBuf> = std::collections::BTreeSet::new();
        for path in &entries {
            if path.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            // The pool declaration is the one `.json` here that is not a
            // prefab document (`delvewright_dsl::prefab::POOLS_FILE`).
            if path.file_name().and_then(|n| n.to_str())
                == Some(delvewright_dsl::prefab::POOLS_FILE)
            {
                continue;
            }
            match delvewright_dsl::split::read_tile_set(path) {
                Ok(Some(set)) => {
                    for part in &set.parts {
                        claimed.insert(path.with_file_name(&part.file));
                    }
                    manifests.push(path.clone());
                }
                // An ordinary prefab's metadata: its `.nbt` stands on its own.
                Ok(None) => {}
                Err(e) => return Err(Diagnostic::error(DW_INPUT, e)),
            }
        }

        let mut found: Vec<PathBuf> = entries
            .into_iter()
            .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("nbt"))
            .filter(|p| !claimed.contains(p))
            .collect();
        found.extend(manifests);
        found.sort();
        out.extend(found);
    }
    if out.is_empty() {
        return Err(Diagnostic::error(DW_INPUT, "no prefabs in the given paths"));
    }
    Ok(out)
}

fn load_models(paths: &[PathBuf]) -> Result<Vec<viewer::ViewerModel>, Diagnostic> {
    let mut models = Vec::with_capacity(paths.len());
    for p in paths {
        models.push(
            viewer::ViewerModel::load(p)
                .map_err(|e| Diagnostic::error(DW_INPUT, format!("{}: {e}", p.display())))?,
        );
    }
    Ok(models)
}

/// Open the asset source colours are derived from — the same jar the GPU path
/// textures with.
fn open_assets(vopts: &ViewOpts) -> Result<Assets, Diagnostic> {
    let path = resolve_textures(vopts.textures.as_deref())?;
    Assets::open(Path::new(&path))
        .map_err(|e| Diagnostic::error(DW_RENDER, format!("open asset source: {e}")))
}

/// Report what the page could not draw as the game draws it, and every binding
/// count behind that verdict. A page that silently drew an unknown block grey
/// would hide exactly the finding it exists to surface — and one that reported
/// nothing over a palette full of under-specified states would be worse, because
/// it would read as a clean bill of health.
fn report_page(stats: &viewer::BuildStats, models: usize, json: bool) {
    for u in &stats.unresolved {
        Diagnostic::warning(
            DW_UNRESOLVED_BLOCK,
            format!(
                "{}: {} ({}) — {} cell(s) draw as the missing-texture placeholder. \
                 The pinned asset source has no definition for it. Whether a pinned \
                 SERVER loads the block is a separate question, decided by the \
                 template's own DataVersion — a pre-pin file is datafixed on load — \
                 and `delvec prefab audit` is what answers it",
                u.state, u.reason, u.detail, u.cells
            ),
        )
        .print(json);
    }
    for u in &stats.under_specified {
        let filled: Vec<String> = u.filled.iter().map(|(k, v)| format!("{k}={v}")).collect();
        let consequence = if u.multipart {
            "this block's definition is `multipart`, so an unwritten property matches no \
             case at all and the block is drawn from the default state rather than from \
             what the file says"
        } else {
            "the variant is selected from the default state rather than from what the \
             file says"
        };
        Diagnostic::warning(
            DW_UNDERSPECIFIED_STATE,
            format!(
                "{}: leaves {} unwritten — {} cell(s). Minecraft {} fills {}, and {}",
                u.state,
                u.filled.keys().cloned().collect::<Vec<_>>().join(", "),
                u.cells,
                delvewright_dsl::blocks::MC_VERSION,
                filled.join(", "),
                consequence
            ),
        )
        .print(json);
    }
    // Binding counts. Each of the checks above is capable of reporting nothing
    // for two very different reasons, and only these numbers tell them apart.
    if stats.states == 0 {
        Diagnostic::warning(
            DW_BINDING,
            format!(
                "0 blockstates bound over {models} prefab(s): the resolution and \
                 completeness checks examined nothing, so a clean page here means \
                 the prefabs are empty, not that they are sound"
            ),
        )
        .print(json);
    }
    if stats.anchors == 0 {
        Diagnostic::warning(
            DW_BINDING,
            format!(
                "0 anchors bound over {models} prefab(s): no `<basename>.json` \
                 declared any anchor or socket, so the page offers the exterior and \
                 plan views only and no player point of view"
            ),
        )
        .print(json);
    }
}

fn run_viewer(inputs: &[PathBuf], out: &Path, title: Option<&str>, vopts: &ViewOpts) -> ExitCode {
    let paths = match collect_pieces(inputs) {
        Ok(p) => p,
        Err(d) => return fail(d, vopts.json, exit::INPUT),
    };
    let models = match load_models(&paths) {
        Ok(m) => m,
        Err(d) => return fail(d, vopts.json, exit::INPUT),
    };

    // **Before the page, not after it** (`compiler::view::showing`). A viewer
    // page is one of the three doors a piece reaches a person's eye through, and
    // the two things a person cannot see in a picture are said here: whether
    // anybody ever measured this building's light, and how much of its roofed
    // floor no body can walk to. Asked before the client jar is opened, because
    // a refusal owed to a reviewer is worth nothing after the page is written.
    let surveys: Vec<showing::Enclosure> = models
        .iter()
        .map(|m| showing::survey(m.structure()))
        .collect();
    for (model, enclosure) in models.iter().zip(&surveys) {
        if let Some(d) = enclosure.finding(model.id()) {
            d.print(vopts.json);
        }
        eprintln!("{}", enclosure.line(model.id()));
    }
    // What each anchor's point of view shows (`DW0893`), measured off the same
    // bytes before the page exists: a preset pressed against a wall is said
    // here, beside the room camera the page offers for the same anchor.
    for model in &models {
        let frames = crate::compiler::view::sight::anchor_frames(model.structure(), model.meta());
        for d in crate::compiler::view::sight::frames_findings(
            &frames,
            |a| format!("{} point of view `pov:{a}`", model.id()),
            |a| format!("{} room view `room:{a}`", model.id()),
        ) {
            d.print(vopts.json);
        }
        eprintln!(
            "{}",
            crate::compiler::view::sight::frames_line(
                model.id(),
                model.meta().map_or(0, |m| m.anchors.len()),
                &frames
            )
        );
    }
    // The light verdict's one escape is a COUNT off these same bytes, never a
    // word in the document — see `LightVerdict::of`.
    let light = showing::LightVerdict::of(
        models
            .iter()
            .zip(&surveys)
            .map(|(m, e)| (m.id(), m.meta(), e.standable)),
    );
    if let Some(d) = light.finding() {
        d.print(vopts.json);
    }
    eprintln!("{}", light.line());
    if light.is_refusal() {
        return ExitCode::from(exit::INPUT);
    }

    let title = title.map(|t| t.to_string()).unwrap_or_else(|| {
        if models.len() == 1 {
            models[0].id().to_string()
        } else {
            format!("{} prefabs", models.len())
        }
    });

    // The page draws real block models, so it needs the real resources: the
    // pinned client jar is not an optimisation here, it is the content.
    let assets = match open_assets(vopts) {
        Ok(a) => a,
        Err(d) => return fail(d, vopts.json, exit::RENDER),
    };

    let (html, stats) = match viewer::build_page(&models, &assets, &title) {
        Ok(v) => v,
        Err(viewer::BuildError::Input(e)) => {
            return fail(Diagnostic::error(DW_INPUT, e), vopts.json, exit::INPUT);
        }
        Err(e @ viewer::BuildError::Bundle(_)) => {
            return fail(
                Diagnostic::error(DW_VIEWER_RESOURCES, e.to_string()),
                vopts.json,
                exit::INTERNAL,
            );
        }
        Err(e @ viewer::BuildError::SpecialTextures(_)) => {
            return fail(
                Diagnostic::error(
                    DW_VIEWER_RESOURCES,
                    format!(
                        "{e}. A block-entity texture is asked for by id and never by a model \
                         file, so a wrong id is invisible: the block renders as the \
                         missing-texture checker and nothing is said. Fix the table in \
                         crates/delvec/src/compiler/view/viewer/resources.rs against the pinned version."
                    ),
                ),
                vopts.json,
                exit::INTERNAL,
            );
        }
    };
    if let Some(parent) = out.parent()
        && !parent.as_os_str().is_empty()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        return fail(
            Diagnostic::error(DW_OUTPUT, format!("create {}: {e}", parent.display())),
            vopts.json,
            exit::OUTPUT,
        );
    }
    if let Err(e) = std::fs::write(out, &html) {
        return fail(
            Diagnostic::error(DW_OUTPUT, format!("write {}: {e}", out.display())),
            vopts.json,
            exit::OUTPUT,
        );
    }

    report_page(&stats, models.len(), vopts.json);

    if vopts.json {
        let summary = serde_json::json!({
            "page": out.display().to_string(),
            "prefabs": models.len(),
            "bytes": stats.bytes,
            "anchors": stats.anchors,
            "states": stats.states,
            "textures": stats.textures,
            "unresolved": stats.unresolved.len(),
            "under_specified": stats.under_specified.len(),
            "special_textures_bound": stats.special_bound,
        });
        println!("{summary}");
    } else {
        println!(
            "{} — {} prefab(s), {} anchors, {} blockstates, {} textures, \
             {} unresolved, {} under-specified, {} KiB",
            out.display(),
            models.len(),
            stats.anchors,
            stats.states,
            stats.textures,
            stats.unresolved.len(),
            stats.under_specified.len(),
            stats.bytes / 1024
        );
    }
    ExitCode::from(exit::OK)
}

fn run_palette(inputs: &[PathBuf], out: &Path, biome: &str, vopts: &ViewOpts) -> ExitCode {
    let paths = match collect_pieces(inputs) {
        Ok(p) => p,
        Err(d) => return fail(d, vopts.json, exit::INPUT),
    };
    let models = match load_models(&paths) {
        Ok(m) => m,
        Err(d) => return fail(d, vopts.json, exit::INPUT),
    };
    let assets = match open_assets(vopts) {
        Ok(a) => a,
        Err(d) => return fail(d, vopts.json, exit::RENDER),
    };
    let deriver = Deriver::with_biome(&assets, biome);
    let table = viewer::palette_for(&models, &deriver);

    let mut json = match serde_json::to_string_pretty(&table) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("internal: serialise palette: {e}");
            return ExitCode::from(exit::INTERNAL);
        }
    };
    json.push('\n');
    if let Err(e) = std::fs::write(out, &json) {
        return fail(
            Diagnostic::error(DW_OUTPUT, format!("write {}: {e}", out.display())),
            vopts.json,
            exit::OUTPUT,
        );
    }

    for (state, reason) in &table.unresolved {
        Diagnostic::warning(DW_UNRESOLVED_BLOCK, format!("{state}: {reason}")).print(vopts.json);
    }
    if vopts.json {
        let summary = serde_json::json!({
            "palette": out.display().to_string(),
            "biome": table.biome,
            "entries": table.entries.len(),
            "unresolved": table.unresolved.len(),
        });
        println!("{summary}");
    } else {
        println!(
            "{} — {} blockstates from {}, {} unresolved",
            out.display(),
            table.entries.len(),
            table.biome,
            table.unresolved.len()
        );
    }
    ExitCode::from(exit::OK)
}
