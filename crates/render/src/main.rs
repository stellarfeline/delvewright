//! `delve-render` — the Delvewright render CLI (spec-0007 / spec-0003, M3).
//!
//! Exit codes (mirrors schem/compiler): `0` ok · `2` input/usage · `3` output ·
//! `4` fidelity-gate failure (missing-texture placeholder detected) · `5`
//! renderer/GPU/textures error · `≥10` internal.
//!
//! Textures (the 1.21.11 client jar, never committed — EULA) resolve from
//! `--textures`, then `$DELVEWRIGHT_CLIENT_JAR`, then `~/.chunky/resources/
//! minecraft.jar`. The `scene` command needs no textures (it emits JSON).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use delvewright_render::cache;
use delvewright_render::detect;
use delvewright_render::diag::{
    DW_ANCHOR_EYE, DW_BINDING, DW_INPUT, DW_MISSING_TEXTURE, DW_OUTPUT, DW_RANK_ORDER, DW_RENDER,
    Diagnostic, exit,
};
use delvewright_render::fidelity;
use delvewright_render::index;
use delvewright_render::meta::PrefabMeta;
use delvewright_render::nbt;
use delvewright_render::occupancy;
use delvewright_render::panorama::{self, Bearing, PanoramaOptions};
use delvewright_render::render::{self, RenderParams};
use delvewright_render::scene::{self, SceneOptions};
use delvewright_render::sheet::{self, ScoreSet, SheetOptions};
use delvewright_render::shots;
use delvewright_render::tileset;

#[derive(Parser)]
#[command(
    name = "delve-render",
    about = "Delvewright render layer: per-prefab shot sets, fidelity gate, Chunky scenes",
    disable_version_flag = true
)]
struct Cli {
    /// Emit diagnostics as one JSON object per line.
    #[arg(long, global = true)]
    json: bool,
    /// Resource pack for textures (the 1.21.11 client jar). Overrides the
    /// `$DELVEWRIGHT_CLIENT_JAR` / `~/.chunky` fallbacks.
    #[arg(long, global = true)]
    textures: Option<String>,
    /// Rendered frame dimension (square), in pixels.
    #[arg(long, global = true, default_value_t = 1024)]
    size: u32,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Render the deterministic multi-angle set for one prefab.
    Piece {
        /// Prefab structure `.nbt` (its `.json` metadata is read when present),
        /// or the `.json` manifest of a zone that ships as a tile set — which
        /// renders the whole assembled zone as one scene.
        input: PathBuf,
        /// Output directory for the PNGs.
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Render the piece set for every prefab in a library directory.
    Batch {
        /// Directory of prefab `.nbt` files and tile-set manifests.
        dir: PathBuf,
        /// Output directory (one subdirectory per prefab).
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Render the newest-1.21.11-block fixture and FAIL if any missing-texture
    /// placeholder is detected.
    FidelityGate {
        /// Optional directory to save the rendered fixture PNG for inspection.
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// Emit Chunky scene JSONs from a build output's `render-plan.json`.
    Scene {
        /// A `delvec build` output directory (containing `render-plan.json`).
        build_dir: PathBuf,
        /// Output directory for the scene JSONs.
        #[arg(short, long)]
        out: PathBuf,
        /// Path Chunky should load the delve world from (documented; default `world`).
        #[arg(long, default_value = "world")]
        world: String,
    },
    /// Emit the whole-map 45° oblique panorama scene (the release illustration)
    /// from a build output's `render-plan.json`.
    Panorama {
        /// A `delvec build` output directory (containing `render-plan.json`).
        build_dir: PathBuf,
        /// Output directory for the scene JSON.
        #[arg(short, long)]
        out: PathBuf,
        /// Path Chunky should load the delve world from (documented; default `world`).
        #[arg(long, default_value = "world")]
        world: String,
        /// Which corner of the layout the camera stands over.
        #[arg(long, value_enum, default_value_t = Bearing::Se)]
        bearing: Bearing,
        /// Path-tracing sample target: ~64 for a draft, ~300 for release art.
        #[arg(long, default_value_t = panorama::DEFAULT_SPP)]
        spp: u32,
    },
    /// Lay candidate renders out as ONE contact sheet for the owner to curate
    /// massing from (spec-0027 §3). With `--scores`, the similarity score
    /// ORDERS the page — it never removes a candidate (spec-0028 §3).
    ContactSheet {
        /// Directory of candidates: one subdirectory of renders per candidate
        /// (`delve-render batch` output), or a flat directory of `.png` renders.
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

fn main() -> ExitCode {
    let cli = Cli::parse();
    match &cli.command {
        Command::Piece { input, out } => run_piece(input, out, &cli),
        Command::Batch { dir, out } => run_batch(dir, out, &cli),
        Command::FidelityGate { out } => run_fidelity_gate(out.as_deref(), &cli),
        Command::Scene {
            build_dir,
            out,
            world,
        } => run_scene(build_dir, out, world, &cli),
        Command::Panorama {
            build_dir,
            out,
            world,
            bearing,
            spp,
        } => run_panorama(build_dir, out, world, *bearing, *spp, &cli),
        Command::ContactSheet {
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
            &cli,
        ),
        Command::Index { build_dir, out } => run_index(build_dir, out, &cli),
    }
}

/// Resolve the textures path from `--textures`, `$DELVEWRIGHT_CLIENT_JAR`, then
/// `~/.chunky/resources/minecraft.jar`. Returns the first that exists.
fn resolve_textures(cli: &Cli) -> Result<String, Diagnostic> {
    let mut candidates: Vec<String> = Vec::new();
    if let Some(t) = &cli.textures {
        candidates.push(t.clone());
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

fn run_piece(input: &Path, out: &Path, cli: &Cli) -> ExitCode {
    let textures = match resolve_textures(cli) {
        Ok(t) => t,
        Err(d) => return fail(d, cli.json, exit::RENDER),
    };
    let pack = match render::load_pack(&textures) {
        Ok(p) => p,
        Err(e) => return fail(Diagnostic::error(DW_RENDER, e), cli.json, exit::RENDER),
    };
    match render_piece(input, out, &pack, cli.size, cli.json) {
        Ok(r) => {
            eprintln!(
                "rendered {} shot(s) for {} -> {} ({})",
                r.shots,
                input.display(),
                out.display(),
                r.binding_line()
            );
            eprintln!("shot manifest -> {}", r.manifest.display());
            ExitCode::SUCCESS
        }
        Err((d, code)) => fail(d, cli.json, code),
    }
}

/// What one prefab's render produced, for the caller's summary line.
struct PieceResult {
    shots: usize,
    manifest: PathBuf,
    binding: shots::AnchorBinding,
}

impl PieceResult {
    /// The eye-shot binding count, always stated: a validation artifact that
    /// does not say what it bound to cannot be told from one that bound to
    /// nothing (CLAUDE.md).
    fn binding_line(&self) -> String {
        let b = &self.binding;
        let mut s = format!(
            "{} eye-level shot(s) over {} anchor(s), {} of them eye-eligible",
            b.eye_shots, b.declared, b.eligible
        );
        if !b.unplaceable.is_empty() {
            s.push_str(&format!("; NO body cell for {}", b.unplaceable.join(", ")));
        }
        if b.eligible == 0 && b.declared > 0 {
            s.push_str(
                "; no anchor declares both a position and a cardinal facing, so this set \
                        contains no interior view",
            );
        }
        s
    }
}

/// Render every planned shot for one prefab into `out`, and write the shot
/// manifest beside them.
///
/// `input` is either a structure `.nbt` or a tile-set manifest. Which one it is
/// changes how the blocks are loaded and nothing else: a zone that needed tiling
/// is reassembled first, so the shot plan — the orbit cameras, the cutaways, the
/// eye cameras and the filenames — is the one the zone would have had if a
/// structure template had no size limit. In particular the eye shots work on a
/// tiled zone for the same reason the orbit shots do: the planner is handed the
/// assembled zone and the manifest's anchors are already in zone coordinates, so
/// a body stands where the anchor says and can look across a cut.
fn render_piece(
    input: &Path,
    out: &Path,
    pack: &nucleation::meshing::ResourcePackSource,
    size: u32,
    json: bool,
) -> Result<PieceResult, (Diagnostic, u8)> {
    let (piece, meta_path) = tileset::load_piece(input)
        .map_err(|e| (Diagnostic::error(DW_INPUT, e.to_string()), exit::INPUT))?;
    if let tileset::PieceInput::Zone { tiles, grid, .. } = &piece {
        eprintln!(
            "{}: assembled {tiles} tile(s) in a {}x{}x{} grid into one scene",
            input.display(),
            grid[0],
            grid[1],
            grid[2]
        );
    }
    let st = piece.structure();
    let meta = PrefabMeta::at_path(&meta_path)
        .map_err(|e| (Diagnostic::error(DW_INPUT, e), exit::INPUT))?;
    let mut plan = shots::plan_piece(st, meta.as_ref());
    for d in &plan.diagnostics {
        d.print(json);
    }
    // Findings only the rendered pixels can raise; folded back into the plan so
    // the manifest carries every diagnostic the run produced.
    let mut empty: Vec<Diagnostic> = Vec::new();
    std::fs::create_dir_all(out).map_err(|e| {
        (
            Diagnostic::error(DW_OUTPUT, format!("mkdir {}: {e}", out.display())),
            exit::OUTPUT,
        )
    })?;
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("prefab");

    for shot in &plan.shots {
        let params = RenderParams {
            yaw_deg: shot.yaw_deg,
            pitch_deg: shot.pitch_deg,
            fov_deg: shot.fov_deg,
            framing: shot.framing,
            dim: size,
        };
        let frame = render::render_structure(st, pack, shot.cutaway, &params)
            .map_err(|e| (Diagnostic::error(DW_RENDER, e), exit::RENDER))?;
        // Advisory only: note a placeholder in a per-piece render (the gate is
        // the enforcing command).
        if let Some(m) = detect::scan_default(&frame.rgba, frame.width, frame.height) {
            Diagnostic::warning(
                DW_MISSING_TEXTURE,
                format!(
                    "{stem}/{}: missing-texture pixels ({} px)",
                    shot.name, m.count
                ),
            )
            .print(json);
        }
        // An eye shot that shows nothing: the render succeeded and the file is a
        // rectangle of background. Left unsaid it reads, in a directory listing,
        // as one more shot of the room.
        if let Some(e) = &shot.eye
            && let Some(f) = detect::is_featureless(&frame.rgba, frame.width, frame.height)
        {
            // Two causes, two different things for the reader to do — the piece
            // is either aimed at nothing, or aimed out of itself, and only the
            // first is a defect.
            let cause = match &e.clearance {
                occupancy::Clearance::LeavesThePiece { open } => format!(
                    "The view runs {open} open cell(s) and then leaves the template. If this \
                     anchor is meant to face outward (an approach, a threshold), what it is about \
                     lives in the assembled world, and its real view is the campaign's own \
                     player-POV shot, not a per-piece render. Otherwise the piece is missing the \
                     thing the anchor names"
                ),
                occupancy::Clearance::Blocked { open, state } => format!(
                    "The view runs {open} open cell(s) and then meets `{state}`, whose face fills \
                     the frame — the anchor is pressed against a surface"
                ),
            };
            let d = Diagnostic::warning(
                DW_ANCHOR_EYE,
                format!(
                    "{stem}/{}: the eye shot for `{}` is an EMPTY frame ({} distinct colour(s)) — \
                     a body standing at {:?} and looking {} sees nothing but flat background. \
                     {cause}. Whatever the cause, no image in this set shows what that anchor is \
                     about; the fix is the anchor or the geometry, never the camera",
                    shot.name,
                    e.anchor,
                    f.distinct,
                    e.cell,
                    e.facing.as_str(),
                ),
            );
            d.print(json);
            empty.push(d);
        }
        let path = out.join(format!("{stem}-{}.png", shot.name));
        save_png(&frame, &path)?;
    }

    plan.diagnostics.extend(empty);

    let manifest = out.join(format!("{stem}-shots.json"));
    std::fs::write(&manifest, shot_manifest(stem, &st.size, &plan)?).map_err(|e| {
        (
            Diagnostic::error(DW_OUTPUT, format!("write {}: {e}", manifest.display())),
            exit::OUTPUT,
        )
    })?;

    Ok(PieceResult {
        shots: plan.shots.len(),
        manifest,
        binding: plan.binding,
    })
}

/// The shot manifest written beside every piece render set: which image is which
/// camera, and — for the eye shots — exactly which cell the body is standing in
/// and how that cell was chosen.
///
/// A nudged eye camera is invisible in its own frame: the picture of a room one
/// block east of an anchor looks exactly like the picture of a room at it. So
/// the placement is written down rather than implied, and the reviewer comparing
/// a frame to a concept image can always tell where they are standing.
fn shot_manifest(
    stem: &str,
    size: &[i32; 3],
    plan: &shots::PiecePlan,
) -> Result<Vec<u8>, (Diagnostic, u8)> {
    let entries: Vec<serde_json::Value> = plan
        .shots
        .iter()
        .map(|s| {
            let mut v = serde_json::json!({
                "name": s.name,
                "kind": s.kind,
                "image": format!("{stem}-{}.png", s.name),
                "yaw": s.yaw_deg,
                "pitch": s.pitch_deg,
                "fov": s.fov_deg,
                "cutaway": s.cutaway,
            });
            if let Some(e) = &s.eye {
                v["eye"] = serde_json::json!({
                    "anchor": e.anchor,
                    "anchor_cell": e.anchor_cell,
                    "facing": e.facing.as_str(),
                    "standing_cell": e.cell,
                    "camera": e.pos,
                    "placement": e.placement.tag(),
                    "clearance_open_cells": e.clearance.open(),
                    "clearance_stopped_by": e.clearance.stopped_by(),
                    "offset": [
                        e.cell[0] - e.anchor_cell[0],
                        e.cell[1] - e.anchor_cell[1],
                        e.cell[2] - e.anchor_cell[2],
                    ],
                    "supported": e.supported,
                });
            }
            v
        })
        .collect();
    let doc = serde_json::json!({
        "prefab": stem,
        "size": size,
        // Rounded for the reader: the constant is an `f32`, and its exact `f64`
        // widening (1.6200000047683716) says nothing a reviewer wants.
        "eye_height": (f64::from(delvewright_render::occupancy::EYE_HEIGHT) * 1000.0).round() / 1000.0,
        "anchors": {
            "declared": plan.binding.declared,
            "eye_eligible": plan.binding.eligible,
            "eye_shots": plan.binding.eye_shots,
            "unplaceable": plan.binding.unplaceable,
        },
        "diagnostics": plan.diagnostics,
        "shots": entries,
    });
    let mut bytes = serde_json::to_vec_pretty(&doc).map_err(|e| {
        (
            Diagnostic::error(DW_OUTPUT, format!("serialize shot manifest: {e}")),
            exit::INTERNAL,
        )
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn run_batch(dir: &Path, out: &Path, cli: &Cli) -> ExitCode {
    let textures = match resolve_textures(cli) {
        Ok(t) => t,
        Err(d) => return fail(d, cli.json, exit::RENDER),
    };
    let pack = match render::load_pack(&textures) {
        Ok(p) => p,
        Err(e) => return fail(Diagnostic::error(DW_RENDER, e), cli.json, exit::RENDER),
    };
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            return fail(
                Diagnostic::error(DW_INPUT, format!("read dir {}: {e}", dir.display())),
                cli.json,
                exit::INPUT,
            );
        }
    };
    let paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();

    // A batch is the second door into "review a fragment and think you reviewed
    // the zone", and it is the one nobody would point at deliberately: walking
    // `*.nbt` in a directory holding a tile set renders each tile as if it were
    // a prefab. So the manifests are collected first, and every `.nbt` they
    // claim is rendered as part of its zone rather than on its own.
    let mut manifests: Vec<PathBuf> = Vec::new();
    let mut claimed: std::collections::BTreeSet<PathBuf> = std::collections::BTreeSet::new();
    for path in &paths {
        if path.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        match delvewright_schem::split::read_tile_set(path) {
            Ok(Some(set)) => {
                for part in &set.parts {
                    claimed.insert(path.with_file_name(&part.file));
                }
                manifests.push(path.clone());
            }
            // An ordinary prefab's metadata: its `.nbt` renders on its own.
            Ok(None) => {}
            Err(e) => return fail(Diagnostic::error(DW_INPUT, e), cli.json, exit::INPUT),
        }
    }

    let mut pieces: Vec<PathBuf> = paths
        .into_iter()
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("nbt"))
        .filter(|p| !claimed.contains(p))
        .collect();
    pieces.extend(manifests);
    pieces.sort();

    let mut total = 0usize;
    for path in &pieces {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("prefab");
        let sub = out.join(stem);
        match render_piece(path, &sub, &pack, cli.size, cli.json) {
            Ok(r) => {
                total += r.shots;
                eprintln!("  {stem}: {} shot(s) — {}", r.shots, r.binding_line());
            }
            Err((d, code)) => return fail(d, cli.json, code),
        }
    }
    eprintln!(
        "batch: {} prefab(s), {total} shot(s) -> {}",
        pieces.len(),
        out.display()
    );
    ExitCode::SUCCESS
}

fn run_fidelity_gate(out: Option<&Path>, cli: &Cli) -> ExitCode {
    let textures = match resolve_textures(cli) {
        Ok(t) => t,
        Err(d) => return fail(d, cli.json, exit::RENDER),
    };
    let pack = match render::load_pack(&textures) {
        Ok(p) => p,
        Err(e) => return fail(Diagnostic::error(DW_RENDER, e), cli.json, exit::RENDER),
    };
    let st = fidelity::fixture_structure();
    // A three-quarter view that shows every showcase block's faces.
    let params = RenderParams {
        yaw_deg: 25.0,
        pitch_deg: 35.0,
        fov_deg: shots::ORBIT_FOV_DEG,
        framing: shots::Framing::Orbit {
            zoom: 1.0,
            target: None,
        },
        dim: cli.size,
    };
    let frame = match render::render_structure(&st, &pack, false, &params) {
        Ok(f) => f,
        Err(e) => return fail(Diagnostic::error(DW_RENDER, e), cli.json, exit::RENDER),
    };
    if let Some(dir) = out
        && let Err((d, code)) = (|| {
            std::fs::create_dir_all(dir).map_err(|e| {
                (
                    Diagnostic::error(DW_OUTPUT, format!("mkdir {}: {e}", dir.display())),
                    exit::OUTPUT,
                )
            })?;
            save_png(&frame, &dir.join("fidelity-gate.png"))
        })()
    {
        return fail(d, cli.json, code);
    }
    match detect::scan_default(&frame.rgba, frame.width, frame.height) {
        Some(m) => {
            let d = Diagnostic::error(
                DW_MISSING_TEXTURE,
                format!(
                    "fidelity gate FAILED: missing-texture (magenta) placeholder detected \
                     ({} px, {:.3}%, first at {},{}) — a block in the prefab has no texture in the \
                     supplied pack. Supply a complete 1.21.11 client jar/texture pack (`--textures`), \
                     or replace the unresolved block. Do NOT lower the fidelity gate to pass",
                    m.count,
                    m.fraction * 100.0,
                    m.sample.0,
                    m.sample.1
                ),
            );
            fail(d, cli.json, exit::FIDELITY)
        }
        None => {
            eprintln!(
                "fidelity gate PASSED: no missing-texture placeholder in the newest-block fixture"
            );
            ExitCode::SUCCESS
        }
    }
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

fn run_scene(build_dir: &Path, out: &Path, world: &str, cli: &Cli) -> ExitCode {
    let plan_path = build_dir.join("render-plan.json");
    let bytes = match std::fs::read(&plan_path) {
        Ok(b) => b,
        Err(e) => {
            return fail(
                Diagnostic::error(DW_INPUT, format!("read {}: {e}", plan_path.display())),
                cli.json,
                exit::INPUT,
            );
        }
    };
    let opts = SceneOptions {
        world_path: world.to_string(),
        width: cli.size,
        height: cli.size,
        spp_target: 500,
    };
    // The shipped structures' block palette feeds the dark-shot night-vision
    // review emulation (no-op for plans without dark-stamped shots).
    let palette = match structure_palette(build_dir) {
        Ok(p) => p,
        Err(d) => return fail(d, cli.json, exit::INPUT),
    };
    let scenes = match scene::scenes_from_plan(&bytes, &opts, &palette) {
        Ok(s) => s,
        Err(d) => return fail(d, cli.json, exit::INPUT),
    };
    let purged = match write_scenes(out, &scenes) {
        Ok(n) => n,
        Err((d, code)) => return fail(d, cli.json, code),
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
    world: &str,
    bearing: Bearing,
    spp: u32,
    cli: &Cli,
) -> ExitCode {
    let plan_path = build_dir.join("render-plan.json");
    let bytes = match std::fs::read(&plan_path) {
        Ok(b) => b,
        Err(e) => {
            return fail(
                Diagnostic::error(DW_INPUT, format!("read {}: {e}", plan_path.display())),
                cli.json,
                exit::INPUT,
            );
        }
    };
    let opts = PanoramaOptions {
        world_path: world.to_string(),
        width: cli.size,
        height: cli.size,
        spp_target: spp,
        bearing,
    };
    let scene = match panorama::panorama_from_plan(&bytes, &opts) {
        Ok(s) => s,
        Err(d) => return fail(d, cli.json, exit::INPUT),
    };
    let name = scene.0.clone();
    let purged = match write_scenes(out, std::slice::from_ref(&scene)) {
        Ok(n) => n,
        Err((d, code)) => return fail(d, cli.json, code),
    };
    eprintln!(
        "emitted panorama {} -> {} ({purged} stale cache file(s) purged; {} spp, render with {}; \
         see README)",
        name,
        out.display(),
        spp,
        scene::CHUNKY_CORE
    );
    ExitCode::SUCCESS
}

fn run_index(build_dir: &Path, out: &Path, cli: &Cli) -> ExitCode {
    let plan_path = build_dir.join("render-plan.json");
    let bytes = match std::fs::read(&plan_path) {
        Ok(b) => b,
        Err(e) => {
            return fail(
                Diagnostic::error(DW_INPUT, format!("read {}: {e}", plan_path.display())),
                cli.json,
                exit::INPUT,
            );
        }
    };
    let idx = match index::index_from_plan(&bytes) {
        Ok(b) => b,
        Err(d) => return fail(d, cli.json, exit::INPUT),
    };
    if let Some(parent) = out.parent()
        && !parent.as_os_str().is_empty()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        return fail(
            Diagnostic::error(DW_OUTPUT, format!("mkdir {}: {e}", parent.display())),
            cli.json,
            exit::OUTPUT,
        );
    }
    if let Err(e) = std::fs::write(out, &idx) {
        return fail(
            Diagnostic::error(DW_OUTPUT, format!("write {}: {e}", out.display())),
            cli.json,
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
    cli: &Cli,
) -> ExitCode {
    let scores = match scores_path.map(read_scores).transpose() {
        Ok(s) => s,
        Err(d) => return fail_sheet(d, cli.json),
    };
    let (candidates, layout) = match sheet::discover(dir, shot) {
        Ok(c) => c,
        Err(d) => return fail_sheet(d, cli.json),
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
        Err(d) => return fail_sheet(d, cli.json),
    };
    for d in &built.diagnostics {
        d.print(cli.json);
    }
    if let Some(parent) = out.parent()
        && !parent.as_os_str().is_empty()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        return fail(
            Diagnostic::error(DW_OUTPUT, format!("mkdir {}: {e}", parent.display())),
            cli.json,
            exit::OUTPUT,
        );
    }
    if let Err(e) = built.image.save(out) {
        return fail(
            Diagnostic::error(DW_OUTPUT, format!("save {}: {e}", out.display())),
            cli.json,
            exit::OUTPUT,
        );
    }
    let manifest_path = out.with_extension("json");
    if let Err(e) = std::fs::write(&manifest_path, &built.manifest) {
        return fail(
            Diagnostic::error(DW_OUTPUT, format!("write {}: {e}", manifest_path.display())),
            cli.json,
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

fn save_png(frame: &render::Frame, path: &Path) -> Result<(), (Diagnostic, u8)> {
    let img = image::RgbaImage::from_raw(frame.width, frame.height, frame.rgba.clone())
        .ok_or_else(|| {
            (
                Diagnostic::error(DW_OUTPUT, "frame buffer size mismatch"),
                exit::INTERNAL,
            )
        })?;
    img.save(path).map_err(|e| {
        (
            Diagnostic::error(DW_OUTPUT, format!("save {}: {e}", path.display())),
            exit::OUTPUT,
        )
    })
}

fn fail(d: Diagnostic, json: bool, code: u8) -> ExitCode {
    d.print(json);
    ExitCode::from(code)
}
