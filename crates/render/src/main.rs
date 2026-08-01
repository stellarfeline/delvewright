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

use delvewright_render::detect;
use delvewright_render::diag::{
    DW_INPUT, DW_MISSING_TEXTURE, DW_OUTPUT, DW_RENDER, Diagnostic, exit,
};
use delvewright_render::fidelity;
use delvewright_render::meta::PrefabMeta;
use delvewright_render::nbt;
use delvewright_render::render::{self, RenderParams};
use delvewright_render::scene::{self, SceneOptions};
use delvewright_render::shots;

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
    /// Render the deterministic multi-angle set for one prefab `.nbt`.
    Piece {
        /// Prefab structure `.nbt` (its `.json` metadata is read when present).
        nbt: PathBuf,
        /// Output directory for the PNGs.
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Render the piece set for every `.nbt` in a prefab library directory.
    Batch {
        /// Directory of prefab `.nbt` files.
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
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match &cli.command {
        Command::Piece { nbt, out } => run_piece(nbt, out, &cli),
        Command::Batch { dir, out } => run_batch(dir, out, &cli),
        Command::FidelityGate { out } => run_fidelity_gate(out.as_deref(), &cli),
        Command::Scene {
            build_dir,
            out,
            world,
        } => run_scene(build_dir, out, world, &cli),
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

fn run_piece(nbt_path: &Path, out: &Path, cli: &Cli) -> ExitCode {
    let textures = match resolve_textures(cli) {
        Ok(t) => t,
        Err(d) => return fail(d, cli.json, exit::RENDER),
    };
    let pack = match render::load_pack(&textures) {
        Ok(p) => p,
        Err(e) => return fail(Diagnostic::error(DW_RENDER, e), cli.json, exit::RENDER),
    };
    match render_piece(nbt_path, out, &pack, cli.size, cli.json) {
        Ok(n) => {
            eprintln!(
                "rendered {n} shot(s) for {} -> {}",
                nbt_path.display(),
                out.display()
            );
            ExitCode::SUCCESS
        }
        Err((d, code)) => fail(d, cli.json, code),
    }
}

/// Render every planned shot for one prefab into `out`. Returns the shot count.
fn render_piece(
    nbt_path: &Path,
    out: &Path,
    pack: &nucleation::meshing::ResourcePackSource,
    size: u32,
    json: bool,
) -> Result<usize, (Diagnostic, u8)> {
    let st = nbt::parse_structure(nbt_path)
        .map_err(|e| (Diagnostic::error(DW_INPUT, e.to_string()), exit::INPUT))?;
    let meta = PrefabMeta::beside_nbt(nbt_path)
        .map_err(|e| (Diagnostic::error(DW_INPUT, e), exit::INPUT))?;
    let plan = shots::plan_piece(st.size, meta.as_ref());
    std::fs::create_dir_all(out).map_err(|e| {
        (
            Diagnostic::error(DW_OUTPUT, format!("mkdir {}: {e}", out.display())),
            exit::OUTPUT,
        )
    })?;
    let stem = nbt_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("prefab");

    for shot in &plan {
        let params = RenderParams {
            yaw_deg: shot.yaw_deg,
            pitch_deg: shot.pitch_deg,
            zoom: shot.zoom,
            target: shot.target,
            dim: size,
        };
        let frame = render::render_structure(&st, pack, shot.cutaway, &params)
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
        let path = out.join(format!("{stem}-{}.png", shot.name));
        save_png(&frame, &path)?;
    }
    Ok(plan.len())
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
    let mut nbts: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("nbt"))
        .collect();
    nbts.sort();
    let mut total = 0usize;
    for nbt_path in &nbts {
        let stem = nbt_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("prefab");
        let sub = out.join(stem);
        match render_piece(nbt_path, &sub, &pack, cli.size, cli.json) {
            Ok(n) => {
                total += n;
                eprintln!("  {stem}: {n} shot(s)");
            }
            Err((d, code)) => return fail(d, cli.json, code),
        }
    }
    eprintln!(
        "batch: {} prefab(s), {total} shot(s) -> {}",
        nbts.len(),
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
        zoom: 1.0,
        target: None,
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
    let scenes = match scene::scenes_from_plan(&bytes, &opts) {
        Ok(s) => s,
        Err(d) => return fail(d, cli.json, exit::INPUT),
    };
    if let Err((d, code)) = (|| {
        std::fs::create_dir_all(out).map_err(|e| {
            (
                Diagnostic::error(DW_OUTPUT, format!("mkdir {}: {e}", out.display())),
                exit::OUTPUT,
            )
        })?;
        for (name, data) in &scenes {
            std::fs::write(out.join(name), data).map_err(|e| {
                (
                    Diagnostic::error(DW_OUTPUT, format!("write {name}: {e}")),
                    exit::OUTPUT,
                )
            })?;
        }
        Ok(())
    })() {
        return fail(d, cli.json, code);
    }
    eprintln!(
        "emitted {} Chunky scene(s) -> {} (render with {}; see README)",
        scenes.len(),
        out.display(),
        scene::CHUNKY_CORE
    );
    ExitCode::SUCCESS
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
