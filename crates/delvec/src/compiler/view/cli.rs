//! The render arms as `delvec` subcommands (ADR-0021 §1).
//!
//! `frames`, `cameras`, `place-camera`, `panorama`, `viewer`, `contact-sheet`
//! and `palette` are declared here and flattened into the top level of the one
//! binary — not because rendering is a compiler concern, but because the
//! surface a creator installs is ONE binary. The drivers below read the
//! modules under [`crate::compiler::view`]; every frame is drawn in this
//! process by [`crate::compiler::view::frames`].
//!
//! **No cargo feature gates any of this.** A feature would ship a
//! same-name-different-capability binary — an artifact whose name promises a
//! surface its bytes may not carry — so the arms are unconditional code, and
//! `tools/ci/build-release-binaries.sh` proves per target that the built binary's
//! own `--help` lists exactly the surface the source declares.
//!
//! The three prefab-piece arms — `piece`, `batch`, `fidelity-gate` — mesh and
//! rasterise through `nucleation`. They live in `crates/delvec/src/render` and
//! mount as `delvec render …` (ADR-0023 §3).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Subcommand;

use crate::compiler::view::assets::Assets;
use crate::compiler::view::blockcolor::Deriver;
use crate::compiler::view::camera::{self, Bracket, DRAFT_DIVISOR, DRAFT_SPP, EmitOptions};
use crate::compiler::view::diag::{
    DW_BINDING, DW_INPUT, DW_OUTPUT, DW_RANK_ORDER, DW_RENDER, DW_UNDERSPECIFIED_STATE,
    DW_UNRESOLVED_BLOCK, DW_VIEWER_RESOURCES, Diagnostic, exit,
};
use crate::compiler::view::frames::Backend;
use crate::compiler::view::panorama::{self, Bearing, PanoramaOptions, Subject};
use crate::compiler::view::sheet::{self, ScoreSet, SheetOptions};
use crate::compiler::view::{frameset, showing, viewer};

/// The options every render arm shares. On `delvec render` these were three
/// global flags; here they are declared by the arms that read them, so that
/// `delvec build --help` does not grow a `--textures` it has no use for.
pub struct ViewOpts {
    /// Emit diagnostics as one JSON object per line.
    pub json: bool,
    /// The pinned client jar. Overrides `$DELVEWRIGHT_CLIENT_JAR` and the
    /// location Init I5 writes it to ([`client_jar_home`]).
    pub textures: Option<String>,
    /// A delve's `resourcepack.zip`, layered above the textures (spec-0084
    /// §5.1): a block texture the delve replaces is drawn as the delve draws it.
    pub pack: Option<String>,
    /// Rendered frame dimension (square), in pixels.
    pub size: u32,
}

/// The default frame dimension of a page arm.
pub const DEFAULT_SIZE: u32 = 1024;

/// How a run that draws frames draws them: the backend, the CPU backend's
/// thread count, and the client jar.
#[derive(Debug, Clone, clap::Args)]
pub struct DrawArgs {
    /// Which backend draws: `auto` takes the GPU wgpu finds (Metal, Vulkan,
    /// DX12) and falls back to the CPU, saying so on the first line; a named
    /// backend that is not available is refused (`DW0723`).
    #[arg(long, value_enum, default_value_t = Backend::Auto)]
    pub backend: Backend,
    /// Threads the CPU backend draws with (default: every core). The CPU
    /// backend's pixels are the same for any count.
    #[arg(long)]
    pub threads: Option<usize>,
    /// The pinned client jar. Overrides `$DELVEWRIGHT_CLIENT_JAR` and the
    /// location Init I5 writes it to.
    #[arg(long)]
    pub textures: Option<String>,
}

#[derive(Subcommand)]
pub enum ViewCommand {
    /// Draw the review frames of a build — every shot of its `render-plan.json`
    /// (or the `--shot`s named), each from the world the build wrote for the
    /// configuration it stands in — with `shot-index.json` beside them; or,
    /// with `--campaign`, one frame of a campaign's load world from a camera
    /// stated by hand, for placing things (spec-0015).
    Frames {
        /// A `delvec build` output directory (holding `render-plan.json` and
        /// `validation/worlds/`).
        #[arg(required_unless_present = "campaign", conflicts_with = "campaign")]
        build_dir: Option<PathBuf>,
        /// Output: the directory the frames and the index are written to
        /// (default `<build-dir>/frames`), or with `--campaign` the PNG
        /// (default `frame.png`).
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Only this shot, by id (repeatable): `pov/leg0/wp0`, `cutscene/…`.
        #[arg(long, conflicts_with = "campaign")]
        shot: Vec<String>,
        /// Stamp every target's outline and name on the frame.
        #[arg(long)]
        labels: bool,
        /// Frame dimension (square), in pixels.
        #[arg(long, default_value_t = crate::compiler::view::frames::REVIEW_SIZE,
              value_parser = clap::value_parser!(u32).range(1..))]
        size: u32,
        #[command(flatten)]
        draw: DrawArgs,
        /// A campaign directory: draw one frame of its load world, assembled
        /// in this process (placement and assembly, no emission).
        #[arg(long)]
        campaign: Option<PathBuf>,
        /// The camera, `x,y,z,yaw,pitch[,fov]` in Minecraft degrees (yaw 0 =
        /// south/+Z, 90 = west/−X; pitch positive looks down).
        #[arg(long, requires = "campaign", conflicts_with = "at")]
        camera: Option<String>,
        /// Frame an anchor (`anchor/fire-pit`, or `area/island:anchor/pen`)
        /// from `--dist` blocks away.
        #[arg(long, requires = "campaign")]
        at: Option<String>,
        /// Compass bearing (degrees) the `--at` camera stands on: 0 = due south
        /// of the subject looking north, 90 = due west looking east.
        #[arg(long, requires = "at", default_value_t = 0.0)]
        orbit: f64,
        /// Distance in blocks from the `--at` subject.
        #[arg(long, requires = "at")]
        dist: Option<f64>,
        /// Draw only what the pinned client draws from this camera: the
        /// square of `world.view_distance` chunks around it, the fog closing
        /// at 16 blocks a chunk — what a player standing there sees. Without
        /// it the frame draws every loaded chunk.
        #[arg(long, requires = "campaign")]
        served: bool,
    },
    /// Draw a frame per showcase camera a campaign states in
    /// `design/cameras.json` — a camera on the assembled world, estimated from
    /// the approved image it answers or placed by hand — each in the world the
    /// engine writes for the configuration the camera stands in (at load, or
    /// after its `after` step), path traced at the camera's own samples.
    Cameras {
        /// A `delvec build` output directory (containing `render-plan.json`).
        build_dir: PathBuf,
        /// The campaign directory: its `design/cameras.json` is read, and every
        /// camera's `answers` is held to its `design.json`.
        #[arg(long)]
        campaign: PathBuf,
        /// Output directory for the frames, and for the worlds they stand in
        /// (`<out>/worlds/<key>/`, one per configuration a camera stands in:
        /// `at-load`, or `after-<step>`). The worlds are written by the engine
        /// from the campaign and `--prefabs` (spec-0089); no server boot is
        /// needed.
        #[arg(short, long)]
        out: PathBuf,
        /// Only this camera, by name (repeatable).
        #[arg(long)]
        only: Vec<String>,
        /// Draw, beside each camera, the camera moved each way by each step —
        /// `yaw=8,pitch=4,fov=10,dolly=6,truck=3,rise=3`, any subset — and write every
        /// candidate to `candidates.json` in the output directory, in the record
        /// format.
        #[arg(long, value_parser = Bracket::parse)]
        bracket: Option<Bracket>,
        // The numbers are the draft constants' own, so the help cannot drift from them.
        #[arg(long, conflicts_with = "preview", help = format!(
            "Draw each frame at 1/{DRAFT_DIVISOR} of its width and height and at most \
             {DRAFT_SPP} samples, as `<stem>_draft.png`: for judging what is in frame."
        ))]
        draft: bool,
        /// Draw each camera (and candidate) in quick mode — the vanilla
        /// client's look — at half its width and height, as `<stem>_preview.png`:
        /// for placing a camera before light is judged.
        #[arg(long)]
        preview: bool,
        #[command(flatten)]
        draw: DrawArgs,
    },
    /// Write one row of `design/cameras.json` — the record's one writer: a pose a
    /// person captured in the game (`--report` + `--slot`, written as `hand`),
    /// an estimate picked from a record-format file (`--candidates` + `--pick`,
    /// written as `estimated`), or `--delete`. An estimate is never written
    /// over a hand camera.
    PlaceCamera {
        /// The campaign directory whose `design/cameras.json` is written; every
        /// camera's `answers` is held to its `design.json`.
        campaign: PathBuf,
        /// The row: a camera name (lowercase letters, digits, `.`, `+`, `-`).
        #[arg(long)]
        name: String,
        /// The `design.json` row the camera answers. Required for a new hand
        /// row; for an existing row it must be the row's own.
        #[arg(long)]
        answers: Option<String>,
        /// A `camera-report.json` from `delvec harvest`: the row becomes the
        /// pose stamped on `--slot`, as `hand`.
        #[arg(long, requires_all = ["slot", "fov"],
              conflicts_with_all = ["candidates", "delete"])]
        report: Option<PathBuf>,
        /// The `dw.cam` slot whose pose is written.
        #[arg(long, requires = "report")]
        slot: Option<u32>,
        /// The vertical field of view the person framed with: their client's
        /// FOV setting, asked of them (the server never receives it).
        #[arg(long, requires = "report")]
        fov: Option<f64>,
        /// A record-format file (`candidates.json` from `delvec cameras
        /// --bracket`): the row becomes the camera named `--pick`, as
        /// `estimated`.
        #[arg(long, requires = "pick", conflicts_with = "delete")]
        candidates: Option<PathBuf>,
        /// The camera in `--candidates` to write.
        #[arg(long, requires = "candidates")]
        pick: Option<String>,
        /// The sky the picture is taken under, `<time>,<weather>` (`noon,clear`),
        /// when it is not the sky of the `design.json` row the camera answers.
        /// Without it a new row takes its row's sky, a replaced hand row keeps
        /// the sky it had, and an estimate carries its own.
        #[arg(long, conflicts_with = "delete")]
        sky: Option<String>,
        /// The step the picture is taken after, `<step>[@<path>]`: an objective
        /// id (`obj/<id>`) or a trigger id (`trigger/<id>`) of the critical
        /// path, or of the branch named after `@`. Held to the campaign's
        /// planned path (read with `--prefabs`). Without it a hand row keeps the
        /// step it had and an estimate carries its own.
        #[arg(long, conflicts_with = "delete")]
        after: Option<String>,
        /// Remove the row. A hand camera is deleted only when the person who
        /// placed it asks.
        #[arg(long)]
        delete: bool,
    },
    /// Draw an oblique exterior of the delve's built place — the storybook
    /// shot — path traced from the world the build wrote at load. The camera
    /// frames the subject (the placed areas unless `--subject` names anchors);
    /// the ground a horizon built around them is loaded, not framed.
    Panorama {
        /// A `delvec build` output directory (containing `render-plan.json` and
        /// `validation/worlds/at-load`).
        build_dir: PathBuf,
        /// Output directory for the frame.
        #[arg(short, long)]
        out: PathBuf,
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
        /// Path-tracing samples a pixel: ~64 for a draft, ~300 for release art.
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
        #[command(flatten)]
        draw: DrawArgs,
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
        /// Similarity scores from `tools/creator/refscore.py`. Absent → id order.
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
        /// The pinned client jar. Overrides `$DELVEWRIGHT_CLIENT_JAR` and the
        /// location Init I5 writes it to.
        #[arg(long)]
        textures: Option<String>,
        /// A delve's `resourcepack.zip`, read above the textures — the order a
        /// client applies a server-sent pack in — so a block texture the delve
        /// replaces (`world.textures[]`) is drawn as the delve draws it.
        #[arg(long)]
        pack: Option<String>,
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
        /// A `delvec build` output: with `--place`, the table is derived under
        /// the biome that place stands in at the first tick (spec-0080 §5.3),
        /// read from the build's `validation/biome-map.json` and its datapack.
        #[arg(long, requires = "place", conflicts_with = "biome")]
        build: Option<PathBuf>,
        /// The place (`area/…`, or a site-plan box's `node/…`) whose biome
        /// tints the table. Requires `--build`.
        #[arg(long, requires = "build")]
        place: Option<String>,
        /// The pinned client jar. Overrides `$DELVEWRIGHT_CLIENT_JAR` and the
        /// location Init I5 writes it to.
        #[arg(long)]
        textures: Option<String>,
        /// A delve's `resourcepack.zip`, read above the textures — the order a
        /// client applies a server-sent pack in — so a block texture the delve
        /// replaces (`world.textures[]`) is drawn as the delve draws it.
        #[arg(long)]
        pack: Option<String>,
    },
}

impl ViewCommand {
    /// Run the arm. `json` is `delvec`'s own global diagnostics flag, which the
    /// render arms answer to exactly as every other subcommand does.
    pub fn run(&self, json: bool) -> ExitCode {
        match self {
            ViewCommand::Frames {
                build_dir: Some(build_dir),
                out,
                shot,
                labels,
                size,
                draw,
                campaign: None,
                ..
            } => frameset::run_frames(
                build_dir,
                &frameset::FramesOptions {
                    out: out
                        .clone()
                        .unwrap_or_else(|| build_dir.join(frameset::FRAMES_DIR)),
                    shots: shot.clone(),
                    labels: *labels,
                    size: *size,
                    draw: draw.clone(),
                },
                json,
            ),
            ViewCommand::Frames { .. }
            | ViewCommand::Cameras { .. }
            | ViewCommand::PlaceCamera { .. } => fail(
                Diagnostic::error(
                    DW_INPUT,
                    "`delvec frames --campaign`, `delvec cameras` and `delvec place-camera` \
                     assemble the world, and are run by the `delvec` binary rather than this arm",
                ),
                json,
                exit::INPUT,
            ),
            ViewCommand::Panorama {
                build_dir,
                out,
                bearing,
                pitch,
                fov,
                subject,
                spp,
                width,
                height,
                draw,
            } => run_panorama(
                build_dir,
                out,
                json,
                PanoramaOptions {
                    width: *width,
                    height: *height,
                    spp_target: *spp,
                    bearing: *bearing,
                    pitch_deg: *pitch,
                    fov_deg: *fov,
                    subject: subject.clone(),
                },
                draw,
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
                    pack: None,
                    size: DEFAULT_SIZE,
                },
            ),
            ViewCommand::Viewer {
                inputs,
                out,
                title,
                textures,
                pack,
            } => run_viewer(
                inputs,
                out,
                title.as_deref(),
                &ViewOpts {
                    json,
                    textures: textures.clone(),
                    pack: pack.clone(),
                    size: DEFAULT_SIZE,
                },
            ),
            ViewCommand::Palette {
                inputs,
                out,
                biome,
                build,
                place,
                textures,
                pack,
            } => run_palette(
                inputs,
                out,
                biome,
                build.as_deref().zip(place.as_deref()),
                &ViewOpts {
                    json,
                    textures: textures.clone(),
                    pack: pack.clone(),
                    size: DEFAULT_SIZE,
                },
            ),
        }
    }
}

/// **Where Init I5 writes the client jar**: `~/.delvewright/client/minecraft-
/// <mc>.jar`, the last of the three places every jar-reading arm looks. `None`
/// when no home directory is known.
pub fn client_jar_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| {
        PathBuf::from(h)
            .join(".delvewright")
            .join("client")
            .join(format!(
                "minecraft-{}.jar",
                delvewright_dsl::blocks::MC_VERSION
            ))
    })
}

/// Resolve the client jar from `--textures`, `$DELVEWRIGHT_CLIENT_JAR`, then
/// [`client_jar_home`]: the first that exists, else `DW0723` naming every place
/// looked and the step that puts the jar there (Init I5).
pub fn resolve_textures(textures: Option<&str>) -> Result<String, Diagnostic> {
    let mut candidates: Vec<String> = Vec::new();
    if let Some(t) = textures {
        candidates.push(t.to_string());
    }
    if let Ok(env) = std::env::var("DELVEWRIGHT_CLIENT_JAR") {
        candidates.push(env);
    }
    if let Some(home) = client_jar_home() {
        candidates.push(home.display().to_string());
    }
    // A path that exists: the jar, or (for the viewer and the palette) a
    // resource pack directory standing in for it.
    for c in &candidates {
        if Path::new(c).exists() {
            return Ok(c.clone());
        }
    }
    Err(Diagnostic::error(
        DW_RENDER,
        format!(
            "no client jar found — looked at {}. Every frame is drawn with Minecraft {}'s own \
             textures, and the jar is never downloaded by an arm that draws: Init I5 puts it at \
             {}; or pass --textures <jar>, or set $DELVEWRIGHT_CLIENT_JAR",
            if candidates.is_empty() {
                "nothing (no --textures, no $DELVEWRIGHT_CLIENT_JAR, no home directory)".to_string()
            } else {
                candidates.join(", ")
            },
            delvewright_dsl::blocks::MC_VERSION,
            client_jar_home()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "~/.delvewright/client/".to_string())
        ),
    ))
}

pub fn fail(d: Diagnostic, json: bool, code: u8) -> ExitCode {
    d.print(json);
    ExitCode::from(code)
}

/// Create the parent directory of an output file, so `-o dir/new/file` works
/// the same on every subcommand that writes one file.
fn create_parent_dir(out: &Path, json: bool) -> Result<(), ExitCode> {
    if let Some(parent) = out.parent()
        && !parent.as_os_str().is_empty()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        return Err(fail(
            Diagnostic::error(DW_OUTPUT, format!("mkdir {}: {e}", parent.display())),
            json,
            exit::OUTPUT,
        ));
    }
    Ok(())
}

fn run_panorama(
    build_dir: &Path,
    out: &Path,
    json: bool,
    opts: PanoramaOptions,
    draw: &DrawArgs,
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
    let world = match frameset::build_world(build_dir, crate::compiler::view::beat::AT_LOAD) {
        Ok(w) => w,
        Err(d) => return fail(d, json, exit::INPUT),
    };
    let jar = match resolve_textures(draw.textures.as_deref()) {
        Ok(j) => j,
        Err(d) => return fail(d, json, exit::RENDER),
    };
    let packs = frameset::build_packs(build_dir);
    let drawn = match frameset::draw_frame(
        &crate::compiler::view::frames::FrameRequest {
            world: &world,
            chunks: pano.chunks,
            jar: Path::new(&jar),
            packs: &packs,
            camera: pano.frame.view(),
            sky: crate::compiler::view::frames::FrameSky {
                day: pano.sky.day,
                daytime_ticks: pano.sky.daytime_ticks,
                weather: pano.sky.weather,
            },
            mode: crate::compiler::view::frames::Mode::Hq {
                spp: pano.frame.spp,
            },
            backend: draw.backend,
            width: pano.frame.width,
            height: pano.frame.height,
            served: None,
        },
        draw.threads,
    ) {
        Ok(d) => d,
        Err(d) => return fail(d, json, exit::RENDER),
    };
    eprintln!("{}", frameset::renderer_line(&drawn, draw.backend));
    let png = out.join(format!("{}.png", pano.stem));
    if let Err(d) = frameset::write_png(&png, &drawn) {
        return fail(d, json, exit::OUTPUT);
    }
    let (smin, smax) = pano.subject;
    // The frame's manifest: its camera, the world, the chunks loaded, the
    // subject box it was fitted to and the share of the frame that box takes —
    // what `check-whole-map-render.py` reads the framing from.
    let manifest = serde_json::json!({
        "manifest_version": 3,
        "delvec": crate::compiler::DELVEC_VERSION,
        "renderer": crate::compiler::view::frames::renderer(),
        "backend": drawn.backend_name(),
        "kind": "panorama",
        "image": {
            "path": format!("{}.png", pano.stem),
            "width": drawn.width,
            "height": drawn.height,
        },
        "camera": {
            "pos": pano.frame.pos,
            "yaw": pano.frame.yaw_deg,
            "pitch": pano.frame.pitch_deg,
            "fov": pano.frame.fov_deg,
            "convention": "minecraft degrees: yaw 0 = south (+Z), 90 = west (-X); \
                           pitch positive looks down; fov is vertical",
        },
        "world": crate::compiler::view::beat::AT_LOAD,
        "chunks": { "min": pano.chunks.0, "max": pano.chunks.1 },
        "sky": {
            "day": pano.sky.day,
            "daytime_ticks": pano.sky.daytime_ticks,
            "weather": pano.sky.weather.keyword(),
        },
        "mode": "hq",
        "spp": pano.frame.spp,
        "subject": {
            "box": { "min": smin, "max": smax },
            "span": {
                "width": crate::compiler::framing::round3(pano.span.width),
                "height": crate::compiler::framing::round3(pano.span.height),
                "fill": crate::compiler::framing::round3(pano.span.fill()),
            },
        },
        "frame": {
            "featureless": crate::compiler::view::detect::is_featureless(
                &drawn.rgba,
                drawn.width,
                drawn.height,
            )
            .map(|f| serde_json::json!({ "distinct_colors": f.distinct }))
            .unwrap_or(serde_json::Value::Null),
        },
        "renderer_stats": drawn.stats.to_json(),
    });
    let mut mbytes = serde_json::to_vec_pretty(&manifest).expect("a manifest serializes");
    mbytes.push(b'\n');
    let mpath = out.join(format!("{}.manifest.json", pano.stem));
    if let Err(e) = std::fs::write(&mpath, mbytes) {
        return fail(
            Diagnostic::error(DW_OUTPUT, format!("write {}: {e}", mpath.display())),
            json,
            exit::OUTPUT,
        );
    }
    eprintln!(
        "drew panorama {} ({}x{}, {} spp, hq) in {:.2} s -> {}",
        pano.stem,
        pano.frame.width,
        pano.frame.height,
        pano.frame.spp,
        drawn.seconds(),
        png.display()
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

/// Where `place-camera` takes its row from; clap holds the three exclusive.
/// Where `delvec place-camera` writes a row from.
pub struct PlaceFrom<'a> {
    pub report: Option<((&'a Path, u32), f64)>,
    pub candidates: Option<(&'a Path, &'a str)>,
    pub delete: bool,
}

/// How a run that writes or reads the record asks where a camera stands
/// (spec-0089 §4): the caller assembles the campaign, so this module never
/// plans one. A refusal is `(diagnostic, exit code)`, or `(None, code)` when
/// the caller has already printed why.
pub type StandAsker<'a> = dyn FnMut(&[camera::Camera]) -> Result<crate::compiler::view::beat::Stood, (Option<Diagnostic>, u8)>
    + 'a;

/// `delvec place-camera`: write one row of the record. `ask` is consulted only
/// when the written row states an `after`, so a row with none never needs the
/// campaign to plan.
#[allow(clippy::too_many_arguments)]
pub fn run_place_camera(
    campaign: &Path,
    name: &str,
    answers: Option<&str>,
    sky: Option<&str>,
    after: Option<&str>,
    from: PlaceFrom<'_>,
    json: bool,
    ask: &mut StandAsker<'_>,
) -> ExitCode {
    // Parsed here rather than by clap, for the reason `--sky` is.
    let after = match after.map(camera::CameraAfter::parse).transpose() {
        Ok(a) => a,
        Err(why) => {
            return fail(
                Diagnostic::error(
                    DW_INPUT,
                    format!("{}: camera `{name}`: --after {why}", camera::CAMERAS_FILE),
                ),
                json,
                exit::INPUT,
            );
        }
    };
    // Parsed here rather than by clap, so a sky that is not one is refused under
    // the record's own code, as every other rule of the record is.
    let sky = match sky.map(camera::CameraSky::parse).transpose() {
        Ok(s) => s,
        Err(why) => {
            return fail(
                Diagnostic::error(
                    DW_INPUT,
                    format!("{}: camera `{name}`: --sky {why}", camera::CAMERAS_FILE),
                ),
                json,
                exit::INPUT,
            );
        }
    };
    let read = |path: &Path| {
        std::fs::read(path)
            .map_err(|e| Diagnostic::error(DW_INPUT, format!("read {}: {e}", path.display())))
    };
    let record_path = campaign.join(camera::CAMERAS_FILE);
    let sheet = match std::fs::read(&record_path) {
        Ok(b) => match camera::parse_sheet(&b) {
            Ok(s) => Some(s),
            Err(d) => return fail(d, json, exit::INPUT),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            return fail(
                Diagnostic::error(DW_INPUT, format!("read {}: {e}", record_path.display())),
                json,
                exit::INPUT,
            );
        }
    };
    let write = |bytes: Option<Vec<u8>>| -> Result<(), Diagnostic> {
        let out = |e: std::io::Error| {
            Diagnostic::error(DW_OUTPUT, format!("write {}: {e}", record_path.display()))
        };
        match bytes {
            Some(b) => {
                if let Some(dir) = record_path.parent() {
                    std::fs::create_dir_all(dir).map_err(out)?;
                }
                std::fs::write(&record_path, b).map_err(out)
            }
            None => std::fs::remove_file(&record_path).map_err(out),
        }
    };

    if from.delete {
        let Some(sheet) = sheet else {
            return fail(
                Diagnostic::error(
                    DW_INPUT,
                    format!(
                        "{} is not there, so it has no camera `{name}`",
                        record_path.display()
                    ),
                ),
                json,
                exit::INPUT,
            );
        };
        let left = match camera::delete(sheet, name) {
            Ok(l) => l,
            Err(d) => return fail(d, json, exit::INPUT),
        };
        let bytes = match left.as_ref().map(camera::sheet_bytes).transpose() {
            Ok(b) => b,
            Err(d) => return fail(d, json, exit::OUTPUT),
        };
        let remaining = left.as_ref().map_or(0, |s| s.cameras.len());
        if let Err(d) = write(bytes) {
            return fail(d, json, exit::OUTPUT);
        }
        eprintln!(
            "deleted camera `{name}` from {} ({remaining} camera(s) left)",
            record_path.display()
        );
        return ExitCode::SUCCESS;
    }

    let placed = (|| -> Result<(String, camera::Placement, String), Diagnostic> {
        if let Some(((report_path, slot), fov)) = from.report {
            let report: crate::orchestrator::camera::CameraReport =
                serde_json::from_slice(&read(report_path)?).map_err(|e| {
                    Diagnostic::error(
                        DW_INPUT,
                        format!(
                            "parse {}: {e}. It is written by `delvec harvest`",
                            report_path.display()
                        ),
                    )
                })?;
            let Some(cam) = report.cameras.iter().find(|c| c.slot == slot) else {
                let slots: Vec<String> =
                    report.cameras.iter().map(|c| c.slot.to_string()).collect();
                return Err(Diagnostic::error(
                    DW_INPUT,
                    format!(
                        "{} holds no pose on slot {slot}. Slots stamped: {}",
                        report_path.display(),
                        if slots.is_empty() {
                            "none".to_string()
                        } else {
                            slots.join(", ")
                        }
                    ),
                ));
            };
            let said = format!(
                "the pose stamped on slot {slot} at {} ({} stamp(s), eye in {})",
                cam.at, cam.stamps, cam.eye_in
            );
            Ok((
                report.campaign_id.clone(),
                camera::Placement::Hand {
                    pos: cam.eye,
                    yaw: cam.yaw,
                    pitch: cam.pitch,
                    fov,
                },
                said,
            ))
        } else if let Some((path, pick)) = from.candidates {
            let file = camera::parse_sheet(&read(path)?)?;
            let Some(cam) = file.cameras.iter().find(|c| c.name == pick) else {
                let names: Vec<&str> = file.cameras.iter().map(|c| c.name.as_str()).collect();
                return Err(Diagnostic::error(
                    DW_INPUT,
                    format!(
                        "{} has no camera `{pick}`. Cameras: {}",
                        path.display(),
                        names.join(", ")
                    ),
                ));
            };
            Ok((
                file.campaign_id.clone(),
                camera::Placement::Estimate(cam.clone()),
                format!("the estimate `{pick}` from {}", path.display()),
            ))
        } else {
            Err(Diagnostic::error(
                DW_INPUT,
                "place-camera writes a row from `--report` + `--slot` + `--fov`, from \
                 `--candidates` + `--pick`, or deletes it with `--delete`",
            ))
        }
    })();
    let (campaign_id, placement, said) = match placed {
        Ok(p) => p,
        Err(d) => return fail(d, json, exit::INPUT),
    };
    let (written, how) =
        match camera::place(sheet, &campaign_id, name, answers, sky, after, placement) {
            Ok(w) => w,
            Err(d) => return fail(d, json, exit::INPUT),
        };
    let (approved, test_level) = match camera::rows_of(campaign) {
        Ok(r) => r,
        Err(d) => return fail(d, json, exit::INPUT),
    };
    let rows: Vec<String> = approved.iter().map(|r| r.name.clone()).collect();
    let answers = camera::tally(&written, &rows);
    if let Some(why) = camera::unbound_refusal(&answers, test_level) {
        return fail(Diagnostic::error(DW_INPUT, why), json, exit::INPUT);
    }
    if let Some((name, a)) = answers.stray.first() {
        return fail(
            Diagnostic::error(DW_INPUT, camera::stray_message(name, a, &rows)),
            json,
            exit::INPUT,
        );
    }
    // The row is written only if the record's sky rule holds of it: the same
    // question `delvec cameras` and `delvec build` ask (spec-0079 §6).
    let placed_row: Vec<camera::Camera> = written
        .cameras
        .iter()
        .filter(|c| c.name == name)
        .cloned()
        .collect();
    let sky_of_row = camera::skies(&placed_row, &approved);
    if let Some(why) = sky_of_row.refusal {
        return fail(Diagnostic::error(DW_INPUT, why), json, exit::INPUT);
    }
    // And its `after` rules (spec-0089 §6): the same question `delvec cameras`
    // and `delvec build` ask, asked of the campaign's own planned path.
    let stood = if placed_row.iter().any(|c| c.after.is_some()) {
        match ask(&placed_row) {
            Ok(s) => Some(s),
            Err((Some(d), code)) => return fail(d, json, code),
            Err((None, code)) => return ExitCode::from(code),
        }
    } else {
        None
    };
    let bytes = match camera::sheet_bytes(&written) {
        Ok(b) => b,
        Err(d) => return fail(d, json, exit::OUTPUT),
    };
    if let Err(d) = write(Some(bytes)) {
        return fail(d, json, exit::OUTPUT);
    }
    let row = written
        .cameras
        .iter()
        .find(|c| c.name == name)
        .expect("the placed row is in the record");
    eprintln!(
        "{} camera `{name}` ({}) in {} from {said}: pos {:?} yaw {} pitch {} fov {}, answers `{}`",
        match how {
            camera::Placed::Added => "added",
            camera::Placed::Replaced => "replaced",
        },
        row.source.as_str(),
        record_path.display(),
        row.pos,
        row.yaw,
        row.pitch,
        row.fov,
        row.answers.as_deref().unwrap_or("no approved image")
    );
    for r in &sky_of_row.resolved {
        eprintln!("{}", r.line());
    }
    for s in stood.iter().flat_map(|s| &s.stands.stands) {
        eprintln!("{}", s.line(0));
    }
    // The count a creator is placing cameras to move (spec-0070 §5), so it moves
    // under their hand rather than at the next build.
    eprintln!("{}", answers.line());
    ExitCode::SUCCESS
}

/// One world the engine wrote: its binding line, its absolute path, and how
/// many block-entity pictures it draws blank.
pub struct WorldWritten {
    pub line: String,
    pub path: String,
    pub omitted: usize,
    pub written: crate::compiler::view::world::WrittenWorld,
}

/// **The one writer of a configuration's world** (spec-0089 §5, spec-0112 §5):
/// `blocks` written under `dir` by the stood campaign's
/// [`crate::compiler::view::review::WorldMaker`], with the regions world `key`
/// lays held out of the generator's ground, and the binding line that names
/// it. `delvec cameras`, `delvec written-world` and `delvec frames --campaign`
/// write through here, and `delvec build` through the same maker.
pub fn write_world(
    key: &str,
    blocks: &crate::compiler::blockstate::BlockMap,
    stood: &crate::compiler::view::beat::Stood,
    rows: &[&crate::compiler::placed::Placed],
    dir: &Path,
) -> Result<WorldWritten, Diagnostic> {
    let started = std::time::Instant::now();
    let laid = stood.stands.laid_in(key).to_vec();
    let (blocks, written) = stood
        .maker
        .encode_blocks(blocks, &laid, stood.day_time, rows);
    let blocks = &blocks;
    crate::compiler::view::world::write_files(&written, dir)
        .map_err(|e| Diagnostic::error(DW_OUTPUT, format!("write {}: {e}", dir.display())))?;
    let abs = std::path::absolute(dir)
        .map_err(|e| Diagnostic::error(DW_OUTPUT, format!("resolve {}: {e}", dir.display())))?
        .display()
        .to_string();
    let bes = stood.maker.block_entities(blocks);
    Ok(WorldWritten {
        line: crate::compiler::view::review::world_line(
            key,
            &written,
            &abs,
            started.elapsed().as_millis(),
        ),
        path: abs,
        omitted: crate::compiler::view::world::block_entities_omitted(blocks, &bes),
        written,
    })
}

/// `delvec cameras`: draw one frame per camera the run frames, each in the
/// world the engine writes for the configuration it stands in (spec-0089
/// §5.3). `ask` is the caller's assembly of the campaign; it is consulted after
/// every rule of the record that needs no world has held. `preview` draws each
/// in quick mode at [`camera::PREVIEW_DIVISOR`] of its size.
#[allow(clippy::too_many_arguments)]
pub fn run_cameras(
    build_dir: &Path,
    campaign: &Path,
    out: &Path,
    json: bool,
    opts: EmitOptions,
    preview: bool,
    draw: &DrawArgs,
    ask: &mut StandAsker<'_>,
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
    let (rows, test_level) = match camera::rows_of(campaign) {
        Ok(r) => r,
        Err(d) => return fail(d, json, exit::INPUT),
    };
    // **Every approved picture is answered** (`DW0900`, spec-0070). Read where
    // the record is read FRESH, so a record edited after the build cannot draw
    // a set with a hole in it; `--only` selects which frames are drawn and
    // exempts nothing, because the record is what is judged. `--preview` is
    // the instrument that closes the hole, so it reports the count and never
    // refuses on it (spec-0070 §3).
    let answers = camera::tally(
        &sheet,
        &rows.iter().map(|r| r.name.clone()).collect::<Vec<_>>(),
    );
    // **The sky each camera is taken under** (spec-0079 §6), printed on every
    // run, refused or not: one line per camera this run frames, and the summary
    // with its denominator.
    let framed = camera::selected(
        &sheet.campaign_id,
        &sheet,
        &opts.only,
        opts.bracket.as_ref(),
    )
    .unwrap_or_default();
    let skies = camera::skies(&framed, &rows);
    for r in &skies.resolved {
        eprintln!("{}", r.line());
    }
    eprintln!("{}", skies.summary());
    if let Some(why) = camera::unbound_refusal(&answers, test_level) {
        return fail(
            Diagnostic::error(camera::DW_RECORD_AT_BUILD.id(), why),
            json,
            exit::INPUT,
        );
    }
    if let Some((name, a)) = answers.stray.first() {
        let names: Vec<String> = rows.iter().map(|r| r.name.clone()).collect();
        return fail(
            Diagnostic::error(
                camera::DW_RECORD_AT_BUILD.id(),
                camera::stray_message(name, a, &names),
            ),
            json,
            exit::INPUT,
        );
    }
    if !preview && !answers.unanswered.is_empty() {
        return fail(
            Diagnostic::error(
                crate::compiler::design::DW_DESIGN_ANSWERED.id(),
                crate::compiler::design::unanswered_message(&rows, &answers),
            ),
            json,
            exit::INPUT,
        );
    }
    if let Some(why) = skies.refusal {
        return fail(
            Diagnostic::error(camera::DW_RECORD_AT_BUILD.id(), why),
            json,
            exit::INPUT,
        );
    }
    let frames = match camera::frames_of(&plan, &sheet, &rows, &opts) {
        Ok(f) => f,
        Err(d) => return fail(d, json, exit::INPUT),
    };
    let jar = match resolve_textures(draw.textures.as_deref()) {
        Ok(j) => j,
        Err(d) => return fail(d, json, exit::RENDER),
    };
    // **The world each camera stands in** (spec-0089 §4, §5): one world per
    // distinct configuration, written by the engine under `<out>/worlds/`.
    // Asked of exactly the cameras this run frames — a candidate stands where
    // its camera does, since it carries the camera's `after`.
    let stood = match ask(&framed) {
        Ok(s) => s,
        Err((Some(d), code)) => return fail(d, json, code),
        Err((None, code)) => return ExitCode::from(code),
    };
    // The bodies and props the datapack places, from the build's own ledger
    // (`validation/placed.json`): the rows each world's arrival holds.
    let ledger = std::fs::read(build_dir.join("validation").join("placed.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .map(|v| crate::compiler::placed::Ledger::from_json(&v))
        .unwrap_or_default();
    let worlds_dir = out.join("worlds");
    let mut world_lines = Vec::new();
    let mut paths = std::collections::BTreeMap::new();
    let mut omitted = std::collections::BTreeMap::new();
    for (key, blocks) in &stood.stands.worlds {
        let rows = match stood.arrival_of(key) {
            Some(a) => {
                crate::compiler::placed::rows_under(&ledger, a, &|g, s| stood.fired_before(g, s))
            }
            None => crate::compiler::placed::rows_under(&ledger, 0, &|_, _| false),
        };
        let w = match write_world(key, blocks, &stood, &rows, &worlds_dir.join(key)) {
            Ok(w) => w,
            Err(d) => return fail(d, json, exit::OUTPUT),
        };
        world_lines.push(w.line);
        omitted.insert(key.clone(), w.omitted);
        paths.insert(key.clone(), PathBuf::from(w.path));
    }
    for s in &stood.stands.stands {
        eprintln!("{}", s.line(omitted.get(&s.key).copied().unwrap_or(0)));
    }
    for l in &world_lines {
        eprintln!("{l}");
    }
    eprintln!("{}", crate::compiler::view::beat::summary(&stood.stands));
    let packs = frameset::build_packs(build_dir);
    let chunks = {
        let (lo, hi) = stood.maker.extent;
        crate::compiler::view::scene::chunk_rect(lo, hi)
    };
    if let Err(e) = std::fs::create_dir_all(out) {
        return fail(
            Diagnostic::error(DW_OUTPUT, format!("mkdir {}: {e}", out.display())),
            json,
            exit::OUTPUT,
        );
    }
    let mut first = true;
    let mut seconds = 0.0f64;
    let mut obstructed = 0usize;
    for f in &frames {
        let Some(stand) = stood
            .stands
            .stands
            .iter()
            .find(|s| s.camera == f.camera.name)
        else {
            return fail(
                Diagnostic::error(
                    DW_INPUT,
                    format!(
                        "camera `{}` stands in no written world: every camera a run frames is \
                         asked where it stands before a frame is drawn",
                        f.camera.name
                    ),
                ),
                json,
                exit::INPUT,
            );
        };
        let grid_blocks = stood
            .stands
            .worlds
            .iter()
            .find(|(k, _)| *k == stand.key)
            .map(|(_, b)| b);
        if let Some(blocks) = grid_blocks
            && let Some(cell) = camera::lens_obstruction(f.frame.pos, |c| blocks.contains_key(&c))
        {
            obstructed += 1;
            eprintln!(
                "camera `{}`: the lens at {:?} is inside or within {} block of `{}` at {cell:?}. A \
                 pinhole camera has no near plane, so the frame shows that block's inside faces or \
                 a sliver of it across a corner: move the camera",
                f.camera.name,
                f.frame.pos,
                camera::LENS_CLEARANCE,
                blocks.get(&cell).map_or("a block", |b| b.as_str())
            );
        }
        let (mode, width, height, file) = if preview {
            (
                crate::compiler::view::frames::Mode::Quick,
                (f.frame.width / camera::PREVIEW_DIVISOR).max(1),
                (f.frame.height / camera::PREVIEW_DIVISOR).max(1),
                camera::preview_file(&sheet.campaign_id, &f.camera.name),
            )
        } else {
            (
                crate::compiler::view::frames::Mode::Hq { spp: f.frame.spp },
                f.frame.width,
                f.frame.height,
                format!("{}.png", f.stem),
            )
        };
        let drawn = match frameset::draw_frame(
            &crate::compiler::view::frames::FrameRequest {
                world: &paths[&stand.key],
                chunks,
                jar: Path::new(&jar),
                packs: &packs,
                camera: f.frame.view(),
                sky: crate::compiler::view::frames::FrameSky {
                    day: f.sky.day,
                    daytime_ticks: f.sky.daytime_ticks,
                    weather: f.sky.weather,
                },
                mode,
                backend: draw.backend,
                width,
                height,
                served: None,
            },
            draw.threads,
        ) {
            Ok(d) => d,
            Err(d) => return fail(d, json, exit::RENDER),
        };
        if first {
            eprintln!("{}", frameset::renderer_line(&drawn, draw.backend));
            first = false;
        }
        seconds += drawn.seconds();
        if let Err(d) = frameset::write_png(&out.join(&file), &drawn) {
            return fail(d, json, exit::OUTPUT);
        }
        // The frame's manifest: what it is of, in the frame's own words —
        // the camera, the world it stands in (by key, relative to `-o`), the
        // sky, the mode and the renderer — so a frame is never a picture
        // nobody can say the inputs of.
        let manifest = serde_json::json!({
            "manifest_version": 3,
            "campaign_id": sheet.campaign_id,
            "delvec": crate::compiler::DELVEC_VERSION,
            "renderer": crate::compiler::view::frames::renderer(),
            "backend": drawn.backend_name(),
            "camera": {
                "name": f.camera.name,
                "pos": f.frame.pos,
                "yaw": f.frame.yaw_deg,
                "pitch": f.frame.pitch_deg,
                "fov": f.frame.fov_deg,
                "convention": "minecraft degrees: yaw 0 = south (+Z), 90 = west (-X); \
                               pitch positive looks down; fov is vertical",
            },
            "image": { "path": file, "width": drawn.width, "height": drawn.height },
            "world": {
                "key": stand.key,
                "path": format!("worlds/{}", stand.key),
            },
            "sky": {
                "day": f.sky.day,
                "daytime_ticks": f.sky.daytime_ticks,
                "weather": f.sky.weather.keyword(),
            },
            "mode": mode.keyword(),
        "frame": {
                "featureless": crate::compiler::view::detect::is_featureless(
                    &drawn.rgba,
                    drawn.width,
                    drawn.height,
                )
                .map(|f| serde_json::json!({ "distinct_colors": f.distinct }))
                .unwrap_or(serde_json::Value::Null),
            },
            "renderer_stats": drawn.stats.to_json(),
        });
        let mut mbytes = serde_json::to_vec_pretty(&manifest).expect("a manifest serializes");
        mbytes.push(b'\n');
        let mpath = out.join(file.replace(".png", ".manifest.json"));
        if let Err(e) = std::fs::write(&mpath, mbytes) {
            return fail(
                Diagnostic::error(DW_OUTPUT, format!("write {}: {e}", mpath.display())),
                json,
                exit::OUTPUT,
            );
        }
    }
    if opts.bracket.is_some() {
        let path = out.join(camera::CANDIDATES_FILE);
        let cams: Vec<camera::Camera> = frames.iter().map(|f| f.camera.clone()).collect();
        let written = camera::candidates_bytes(&sheet.campaign_id, &cams).and_then(|b| {
            std::fs::write(&path, b)
                .map_err(|e| Diagnostic::error(DW_OUTPUT, format!("write {}: {e}", path.display())))
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
        "{} {} camera frame(s) for {stated} of {} stated camera(s) -> {} ({}{} in {seconds:.2} \
         s){}",
        if preview { "previewed" } else { "drew" },
        frames.len(),
        sheet.cameras.len(),
        out.display(),
        if preview {
            "previews, quick"
        } else if opts.draft {
            "drafts, hq"
        } else {
            "hq"
        },
        if preview {
            ""
        } else {
            " at each camera's samples"
        },
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
        "lens: {} of {} camera(s) clear of every block by {} block, {obstructed} flagged",
        frames.len() - obstructed,
        frames.len(),
        camera::LENS_CLEARANCE
    );
    eprintln!("{}", answers.line());
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
    if let Err(code) = create_parent_dir(out, vopts.json) {
        return code;
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
            // The pool declaration and a gate report are `.json` files here
            // that are not prefab documents — the registry's one rule.
            if !crate::compiler::registry::is_prefab_document(path) {
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
    let opened = match &vopts.pack {
        // spec-0084 §5.1: the delve's pack above the pinned jar. Said on every
        // run, so a page drawn with the delve's look is never mistaken for
        // vanilla's, or the other way round.
        Some(pack) => {
            eprintln!("textures: {pack} layered above {path}");
            Assets::open_layered(Path::new(pack), Path::new(&path))
        }
        None => Assets::open(Path::new(&path)),
    };
    opened.map_err(|e| Diagnostic::error(DW_RENDER, format!("open asset source: {e}")))
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
    // the thing a person cannot see in a picture is said here: how much of its
    // roofed floor no body can walk to. Asked before the client jar is opened,
    // because a report owed to a reviewer is worth nothing after the page is
    // written.
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
    if let Err(code) = create_parent_dir(out, vopts.json) {
        return code;
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

/// The biome a build says `place` stands in at the first tick, and its
/// definition when the build ships it (an atmosphere) — `None` for the
/// definition means the pinned jar holds it.
fn place_biome(
    build: &Path,
    place: &str,
) -> Result<(String, Option<serde_json::Value>), Diagnostic> {
    let read = |p: &Path| -> Result<serde_json::Value, Diagnostic> {
        let text = std::fs::read_to_string(p)
            .map_err(|e| Diagnostic::error(DW_INPUT, format!("read {}: {e}", p.display())))?;
        serde_json::from_str(&text)
            .map_err(|e| Diagnostic::error(DW_INPUT, format!("parse {}: {e}", p.display())))
    };
    let map_path = build.join("validation/biome-map.json");
    let biome = if map_path.is_file() {
        let map = read(&map_path)?;
        map["places"]
            .as_array()
            .and_then(|ps| ps.iter().find(|p| p["place"] == place))
            .or(None)
            .map(|p| p["biome"].as_str().unwrap_or_default().to_string())
            .unwrap_or_else(|| map["ground"].as_str().unwrap_or_default().to_string())
    } else {
        // No place in this build carries an atmosphere, so every place stands
        // in the ground biome: the one `generator-settings` lays.
        let props =
            std::fs::read_to_string(build.join("server/server.properties")).map_err(|e| {
                Diagnostic::error(
                    DW_INPUT,
                    format!("read {}/server/server.properties: {e}", build.display()),
                )
            })?;
        props
            .lines()
            .find_map(|l| l.strip_prefix("generator-settings="))
            .and_then(|g| serde_json::from_str::<serde_json::Value>(&g.replace("\\:", ":")).ok())
            .and_then(|g| g["biome"].as_str().map(str::to_string))
            .unwrap_or_else(|| crate::compiler::view::blockcolor::DEFAULT_BIOME.to_string())
    };
    let (ns, id) = biome
        .split_once(':')
        .unwrap_or(("minecraft", biome.as_str()));
    let file = build.join(format!("datapack/data/{ns}/worldgen/biome/{id}.json"));
    let definition = if file.is_file() {
        Some(read(&file)?)
    } else {
        None
    };
    Ok((biome, definition))
}

fn run_palette(
    inputs: &[PathBuf],
    out: &Path,
    biome: &str,
    at: Option<(&Path, &str)>,
    vopts: &ViewOpts,
) -> ExitCode {
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
    let deriver = match at {
        None => Deriver::with_biome(&assets, biome),
        Some((build, place)) => match place_biome(build, place) {
            Ok((id, Some(def))) => {
                eprintln!("palette: `{place}` stands in `{id}`, read from the build's datapack");
                Deriver::with_biome_definition(&assets, &id, &def)
            }
            Ok((id, None)) => {
                eprintln!("palette: `{place}` stands in `{id}`, read from the pinned jar");
                Deriver::with_biome(&assets, &id)
            }
            Err(d) => return fail(d, vopts.json, exit::INPUT),
        },
    };
    let table = viewer::palette_for(&models, &deriver);

    let mut json = match serde_json::to_string_pretty(&table) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("internal: serialise palette: {e}");
            return ExitCode::from(exit::INTERNAL);
        }
    };
    json.push('\n');
    if let Err(code) = create_parent_dir(out, vopts.json) {
        return code;
    }
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
