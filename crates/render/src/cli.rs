//! `delvec render` — the **GPU arms** of the Delvewright render layer
//! (spec-0007 / spec-0003, M3), mounted by the `delvec` binary (ADR-0023 §3).
//!
//! Three subcommands live here — `piece`, `batch` and `fidelity-gate` — because
//! they are the three that mesh and rasterise through `nucleation`/`wgpu`. The
//! CPU arms are top-level `delvec` subcommands (ADR-0021 §1): `delvec viewer`,
//! `delvec scene`, `delvec panorama`, `delvec contact-sheet`, `delvec palette`,
//! `delvec index`.
//!
//! Exit codes (mirrors schem/compiler): `0` ok · `2` input/usage · `3` output ·
//! `4` fidelity-gate failure (missing-texture placeholder detected) · `5`
//! renderer/GPU/textures error · `≥10` internal.
//!
//! Textures (the 1.21.11 client jar, never committed — EULA) resolve from
//! `--textures`, then `$DELVEWRIGHT_CLIENT_JAR`, then `~/.chunky/resources/
//! minecraft.jar`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Subcommand};

use crate::detect;
use crate::diag::{DW_INPUT, DW_MISSING_TEXTURE, DW_OUTPUT, DW_RENDER, Diagnostic, exit};
use crate::fidelity;
use crate::meta::PrefabMeta;
use crate::render::{self, RenderParams};
use crate::shots;
use crate::view::View;
use delvewright_compiler::view::cli::{fail, resolve_textures};
use delvewright_compiler::view::tileset;

/// `delvec render`: the command line, as a type. `--textures` and `--size` are
/// global to the three arms, so `delvec render piece x -o y --size 640` and
/// `delvec render --size 640 piece x -o y` are the same call.
#[derive(Clone, Args)]
pub struct RenderArgs {
    /// Resource pack for textures (the 1.21.11 client jar). Overrides the
    /// `$DELVEWRIGHT_CLIENT_JAR` / `~/.chunky` fallbacks.
    #[arg(long, global = true)]
    pub textures: Option<String>,
    /// Rendered frame dimension (square), in pixels.
    #[arg(long, global = true, default_value_t = 1024)]
    pub size: u32,
    #[command(subcommand)]
    pub command: RenderCommand,
}

#[derive(Clone, Subcommand)]
pub enum RenderCommand {
    /// Render the deterministic multi-angle set for one prefab.
    Piece {
        /// Prefab structure `.nbt` (its `.json` metadata is read when present),
        /// or the `.json` manifest of a zone that ships as a tile set — which
        /// renders the whole assembled zone as one scene.
        input: PathBuf,
        /// Output directory for the PNGs.
        #[arg(short, long)]
        out: PathBuf,
        /// An extra camera you aim yourself, appended to the planned set.
        /// Repeatable. `key=value,…` — `face=north|south|east|west|up|down`
        /// (square-on at that face of the subject box) or `yaw=<deg>`; plus
        /// `name=`, `pitch=`, `fov=`, `zoom=`, `of=model|<anchor>`, `cutaway=`.
        #[arg(long = "view", value_name = "SPEC")]
        views: Vec<String>,
    },
    /// Render the piece set for every prefab in a library directory.
    Batch {
        /// Directory of prefab `.nbt` files and tile-set manifests.
        dir: PathBuf,
        /// Output directory (one subdirectory per prefab).
        #[arg(short, long)]
        out: PathBuf,
        /// An extra camera you aim yourself, added to EVERY prefab's set (see
        /// `piece --view`). A view naming a subject some prefab does not declare
        /// is an error for that prefab, never a silently different picture.
        #[arg(long = "view", value_name = "SPEC")]
        views: Vec<String>,
    },
    /// Render the newest-1.21.11-block fixture and FAIL if any missing-texture
    /// placeholder is detected.
    FidelityGate {
        /// Optional directory to save the rendered fixture PNG for inspection.
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
}

/// Run `delvec render`. `json` is `delvec`'s global diagnostics flag.
pub fn run(cli: RenderArgs, json: bool) -> ExitCode {
    match &cli.command {
        RenderCommand::Piece { input, out, views } => run_piece(input, out, views, &cli, json),
        RenderCommand::Batch { dir, out, views } => run_batch(dir, out, views, &cli, json),
        RenderCommand::FidelityGate { out } => run_fidelity_gate(out.as_deref(), &cli, json),
    }
}

/// Parse the `--view` specs before anything else runs.
///
/// A malformed spec is a usage error, and it is worth exactly nothing to find it
/// after a GPU has been initialised and twenty-eight frames have been written.
fn parse_views(specs: &[String]) -> Result<Vec<View>, Diagnostic> {
    specs
        .iter()
        .map(|s| View::parse(s).map_err(|e| Diagnostic::error(DW_INPUT, e)))
        .collect()
}

fn run_piece(
    input: &Path,
    out: &Path,
    view_specs: &[String],
    cli: &RenderArgs,
    json: bool,
) -> ExitCode {
    let views = match parse_views(view_specs) {
        Ok(v) => v,
        Err(d) => return fail(d, json, exit::INPUT),
    };
    let textures = match resolve_textures(cli.textures.as_deref()) {
        Ok(t) => t,
        Err(d) => return fail(d, json, exit::RENDER),
    };
    let pack = match render::load_pack(&textures) {
        Ok(p) => p,
        Err(e) => return fail(Diagnostic::error(DW_RENDER, e), json, exit::RENDER),
    };
    match render_piece(input, out, &pack, &views, cli.size, json) {
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
        Err((d, code)) => fail(d, json, code),
    }
}

/// What one prefab's render produced, for the caller's summary line.
struct PieceResult {
    shots: usize,
    manifest: PathBuf,
    binding: shots::AnchorBinding,
    views: shots::ViewBinding,
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
        if self.views.declared > 0 {
            s.push_str(&format!(
                "; {} declared view(s), {} planned",
                self.views.declared, self.views.planned
            ));
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
    views: &[View],
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
    let mut plan = shots::plan_piece(st, meta.as_ref(), views).map_err(|d| (d, exit::INPUT))?;
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
        // A frame that shows nothing: the render succeeded and the file is a
        // rectangle of background. Left unsaid it reads, in a directory listing,
        // as one more shot of the room. Measured on every camera, not only the
        // eye ones — "this picture is blank" is a property of a rendered frame,
        // and an aimable camera is far easier to point at nothing than a derived
        // one is.
        if let Some(f) = detect::is_featureless(&frame.rgba, frame.width, frame.height) {
            let d = shots::empty_frame_diagnostic(stem, shot, &f);
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
        views: plan.views,
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
            if let Some(w) = &s.view {
                v["view"] = serde_json::json!({
                    "spec": w.spec,
                    "face": w.face.map(|f| f.as_str()),
                    "subject": w.subject.tag(),
                    "aim": match s.framing {
                        shots::Framing::Orbit { target, .. } => target,
                        shots::Framing::Eye { pos } => Some(pos),
                    },
                    "zoom": w.zoom,
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
        "eye_height": (f64::from(crate::occupancy::EYE_HEIGHT) * 1000.0).round() / 1000.0,
        "anchors": {
            "declared": plan.binding.declared,
            "eye_eligible": plan.binding.eligible,
            "eye_shots": plan.binding.eye_shots,
            "unplaceable": plan.binding.unplaceable,
        },
        "views": {
            "declared": plan.views.declared,
            "planned": plan.views.planned,
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

fn run_batch(
    dir: &Path,
    out: &Path,
    view_specs: &[String],
    cli: &RenderArgs,
    json: bool,
) -> ExitCode {
    let views = match parse_views(view_specs) {
        Ok(v) => v,
        Err(d) => return fail(d, json, exit::INPUT),
    };
    let textures = match resolve_textures(cli.textures.as_deref()) {
        Ok(t) => t,
        Err(d) => return fail(d, json, exit::RENDER),
    };
    let pack = match render::load_pack(&textures) {
        Ok(p) => p,
        Err(e) => return fail(Diagnostic::error(DW_RENDER, e), json, exit::RENDER),
    };
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            return fail(
                Diagnostic::error(DW_INPUT, format!("read dir {}: {e}", dir.display())),
                json,
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
        // The pool declaration is the one `.json` in a prefab library
        // that is not a prefab document.
        if path.file_name().and_then(|n| n.to_str()) == Some(delvewright_schem::prefab::POOLS_FILE)
        {
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
            Err(e) => return fail(Diagnostic::error(DW_INPUT, e), json, exit::INPUT),
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
        match render_piece(path, &sub, &pack, &views, cli.size, json) {
            Ok(r) => {
                total += r.shots;
                eprintln!("  {stem}: {} shot(s) — {}", r.shots, r.binding_line());
            }
            Err((d, code)) => return fail(d, json, code),
        }
    }
    eprintln!(
        "batch: {} prefab(s), {total} shot(s) -> {}",
        pieces.len(),
        out.display()
    );
    ExitCode::SUCCESS
}

fn run_fidelity_gate(out: Option<&Path>, cli: &RenderArgs, json: bool) -> ExitCode {
    let textures = match resolve_textures(cli.textures.as_deref()) {
        Ok(t) => t,
        Err(d) => return fail(d, json, exit::RENDER),
    };
    let pack = match render::load_pack(&textures) {
        Ok(p) => p,
        Err(e) => return fail(Diagnostic::error(DW_RENDER, e), json, exit::RENDER),
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
        Err(e) => return fail(Diagnostic::error(DW_RENDER, e), json, exit::RENDER),
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
        return fail(d, json, code);
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
            fail(d, json, exit::FIDELITY)
        }
        None => {
            eprintln!(
                "fidelity gate PASSED: no missing-texture placeholder in the newest-block fixture"
            );
            ExitCode::SUCCESS
        }
    }
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
