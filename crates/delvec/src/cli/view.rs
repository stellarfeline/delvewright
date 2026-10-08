//! `delvec snapshot`, `cameras --preview`, the camera stand check and
//! `blocking-chart`: the view arms that read the binary's campaign loader.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;

use crate::EXIT_INTERNAL;
use crate::cli::campaign::{read_structures, validate_stage};
use crate::cli::report::{print_build_error, print_diags, write_file};

/// The `snapshot` subcommand's arguments, bundled so the dispatcher stays legible.
pub(crate) struct SnapshotArgs<'a> {
    pub(crate) camera: Option<&'a str>,
    pub(crate) at: Option<&'a str>,
    pub(crate) orbit: f64,
    pub(crate) dist: Option<f64>,
    pub(crate) shot: Option<&'a str>,
    pub(crate) out: &'a Path,
    pub(crate) labels: bool,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) timing: bool,
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
pub(crate) fn run_snapshot(
    campaign_dir: &Path,
    prefabs_dir: &Path,
    args: SnapshotArgs<'_>,
    json: bool,
) -> ExitCode {
    use delvec::compiler::snapshot;

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
            print_diags(&e.warnings, json);
            print_build_error(e.failure.code, &e.failure.message, json);
            return ExitCode::from(3);
        }
    };
    // The placement stage's own advisories (`DW0498`). `build`/`edit` get these
    // through `emit::build_with_warnings`; the view commands never emit, so they
    // report them here — a draw that repeats an anchored piece is exactly what a
    // reviewer is looking at in a snapshot.
    print_diags(&plan.warnings.clone(), json);
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
    let world = delvec::compiler::nav::World::from_occupancy(
        delvec::compiler::assembled::occupancy_over(&assembled.blocks, &assembled.open_gates),
        delvec::compiler::nav::Premises::geometry_only(),
    );
    let blocks = assembled.blocks;
    let grid = snapshot::VoxelGrid::build(&blocks);
    snapshot::report_unpainted(&grid);
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
    let png = delvec::compiler::png::encode_rgba(
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

/// **Where each camera stands** (spec-0089 §4): the campaign assembled as
/// `delvec cameras --preview` assembles it, the world given the premises the
/// build's proofs carry, and every camera's configuration asked of
/// [`delvec::compiler::view::beat::stands`] — the record's `after` rules
/// refused under `DW0721` (exit 2). A campaign that does not plan has its
/// own refusal printed here, and the caller is told only the code.
pub(crate) fn camera_stands(
    campaign_dir: &Path,
    prefabs_dir: &Path,
    json: bool,
    cameras: &[delvec::compiler::view::camera::Camera],
) -> Result<
    delvec::compiler::view::beat::Stood,
    (Option<delvec::compiler::view::diag::Diagnostic>, u8),
> {
    let (campaign, prefabs) =
        load_for_view(campaign_dir, prefabs_dir, json).map_err(|c| (None, c))?;
    let plan = match Plan::build(&campaign, &prefabs) {
        Ok(p) => p,
        Err(e) => {
            print_diags(&e.warnings, json);
            print_build_error(e.failure.code, &e.failure.message, json);
            return Err((None, 3));
        }
    };
    let structures = read_structures(&plan, &prefabs, prefabs_dir, json).map_err(|c| (None, c))?;
    let assembled = edited_assembled(&plan, &prefabs, &structures, json).map_err(|c| (None, c))?;
    let world = camera_world(&plan, &assembled);
    // The path the build's proofs read: the links the route proof takes
    // spliced in (spec-0083), so a step is the step `critical-path.json` names.
    let relinked = match delvec::compiler::nav::with_links_taken(&plan, &prefabs, &world) {
        Ok(r) => r,
        Err(f) => {
            print_build_error(f.code, &f.message, json);
            return Err((None, 3));
        }
    };
    let plan = relinked.as_ref().unwrap_or(&plan);
    let base = camera_base(plan, &assembled);
    let stands =
        delvec::compiler::view::beat::stands(plan, &world, &base, cameras).map_err(|why| {
            (
                Some(delvec::compiler::view::diag::Diagnostic::error(
                    delvec::compiler::view::camera::DW_RECORD_AT_BUILD.id(),
                    why,
                )),
                2,
            )
        })?;
    let biomes = delvec::compiler::horizon::biome_map(plan);
    Ok(delvec::compiler::view::beat::Stood {
        stands,
        biome: Box::new(move |c| biomes.at(c).0.to_string()),
        spawn: plan.campaign_start().map_or([0, 64, 0], |(_, p)| p),
    })
}

/// The world a camera's configuration is asked of: the assembled occupancy
/// under the premises the build's proofs carry — the measured world-load
/// seals among them, which is what makes a gate the prefab built shut a
/// step-0 write (`nav::Premises::of_plan`).
fn camera_world(
    plan: &Plan,
    assembled: &delvec::compiler::assembled::Assembled,
) -> delvec::compiler::nav::World {
    delvec::compiler::nav::World::from_occupancy(
        delvec::compiler::assembled::occupancy_over(&assembled.blocks, &assembled.open_gates),
        delvec::compiler::nav::Premises::of_plan(plan, assembled.gate_seals.clone()),
    )
}

/// The bytes a camera's world starts from
/// ([`delvec::compiler::view::beat::picture_base`]): the relight pass is run
/// as the build runs it, so a fixture the datapack sets is in the picture.
fn camera_base(
    plan: &Plan,
    assembled: &delvec::compiler::assembled::Assembled,
) -> delvec::compiler::blockstate::BlockMap {
    let relight = delvec::compiler::light::relight_over(plan, assembled);
    delvec::compiler::view::beat::picture_base(plan, assembled, &relight.placements)
}

/// `delvec cameras --preview`: every stated camera of `design/cameras.json` (and
/// its bracket candidates) drawn by the snapshot rasteriser over the assembled
/// world — the same world, the same Minecraft camera convention, flat-lit and in
/// seconds — so a camera is placed before the path tracer is asked about light.
/// The record is read by the one reader (`compiler::view::camera`); nothing here
/// restates where a camera is.
pub(crate) fn run_cameras_preview(
    build_dir: &Path,
    campaign_dir: &Path,
    prefabs_dir: &Path,
    out: &Path,
    only: &[String],
    bracket: Option<&delvec::compiler::view::camera::Bracket>,
    json: bool,
) -> ExitCode {
    use delvec::compiler::snapshot;
    use delvec::compiler::view::camera;

    use delvec::compiler::view::diag::{DW_INPUT, Diagnostic};
    let read = |path: PathBuf| {
        std::fs::read(&path)
            .map_err(|e| Diagnostic::error(DW_INPUT, format!("read {}: {e}", path.display())))
    };
    let selected = read(build_dir.join("render-plan.json")).and_then(|plan| {
        let id = camera::plan_campaign_id(&plan)?;
        let sheet =
            read(campaign_dir.join(camera::CAMERAS_FILE)).and_then(|b| camera::parse_sheet(&b))?;
        let approved =
            read(campaign_dir.join("design.json")).and_then(|b| camera::reference_rows(&b))?;
        let rows: Vec<String> = approved.iter().map(|r| r.name.clone()).collect();
        // The record's own rules still hold here (`DW0721`), and the answered
        // count is REPORTED and never refused: this is the instrument a creator
        // closes the hole with, so a rule that refused it would refuse the
        // repair it prescribes (spec-0070 §3).
        let answers = camera::tally(&sheet, &rows);
        if let Some((name, a)) = answers.stray.first() {
            return Err(delvec::compiler::view::diag::Diagnostic::error(
                delvec::compiler::view::diag::DW_INPUT,
                camera::stray_message(name, a, &rows),
            ));
        }
        let cams = camera::selected(&id, &sheet, only, bracket)?;
        // The sky rule too (spec-0079 §6): a flat-lit preview draws no sky, but
        // the record it reads is the record `delvec cameras` reads.
        if let Some(why) = camera::skies(&cams, &approved).refusal {
            return Err(Diagnostic::error(DW_INPUT, why));
        }
        Ok((id, cams, answers))
    });
    let (campaign_id, cameras, answers) = match selected {
        Ok(v) => v,
        Err(d) => return delvec::compiler::view::cli::fail(d, json, 2),
    };

    let (campaign, prefabs) = match load_for_view(campaign_dir, prefabs_dir, json) {
        Ok(v) => v,
        Err(code) => return ExitCode::from(code),
    };
    let plan = match Plan::build(&campaign, &prefabs) {
        Ok(p) => p,
        Err(e) => {
            print_diags(&e.warnings, json);
            print_build_error(e.failure.code, &e.failure.message, json);
            return ExitCode::from(3);
        }
    };
    let structures = match read_structures(&plan, &prefabs, prefabs_dir, json) {
        Ok(s) => s,
        Err(code) => return ExitCode::from(code),
    };
    let assembled = match edited_assembled(&plan, &prefabs, &structures, json) {
        Ok(a) => a,
        Err(code) => return ExitCode::from(code),
    };
    // Each camera is drawn in the configuration it stands in (spec-0089 §4):
    // one grid per distinct world, the record's `after` rules refused here as
    // `delvec cameras` refuses them.
    let world = camera_world(&plan, &assembled);
    let relinked = match delvec::compiler::nav::with_links_taken(&plan, &prefabs, &world) {
        Ok(r) => r,
        Err(f) => {
            print_build_error(f.code, &f.message, json);
            return ExitCode::from(3);
        }
    };
    let plan = relinked.as_ref().unwrap_or(&plan);
    let base = camera_base(plan, &assembled);
    let stood = match delvec::compiler::view::beat::stands(plan, &world, &base, &cameras) {
        Ok(s) => s,
        Err(why) => {
            return delvec::compiler::view::cli::fail(
                Diagnostic::error(camera::DW_RECORD_AT_BUILD.id(), why),
                json,
                2,
            );
        }
    };
    let grids: BTreeMap<String, snapshot::VoxelGrid> = stood
        .worlds
        .iter()
        .map(|(k, b)| (k.clone(), snapshot::VoxelGrid::build(b)))
        .collect();
    for g in grids.values() {
        snapshot::report_unpainted(g);
    }
    for s in &stood.stands {
        eprintln!("{}", s.line(0));
    }
    if let Err(e) = std::fs::create_dir_all(out) {
        eprintln!("internal error: mkdir {}: {e}", out.display());
        return ExitCode::from(EXIT_INTERNAL);
    }
    let mut obstructed = 0usize;
    for (cam, st) in cameras.iter().zip(&stood.stands) {
        let grid = &grids[&st.key];
        if let Some(cell) = camera::lens_obstruction(cam.pos, |c| grid.solid(c)) {
            obstructed += 1;
            eprintln!(
                "camera `{}`: the lens at {:?} is inside or within {} block of `{}` at {cell:?}. A \
                 pinhole camera has no near plane, so the frame shows that block's inside faces or a \
                 sliver of it across a corner: move the camera",
                cam.name,
                cam.pos,
                camera::LENS_CLEARANCE,
                grid.name(grid.at(cell))
            );
        }
        let frame = snapshot::render_frame(
            grid,
            &snapshot::Camera {
                pos: cam.pos,
                yaw: cam.yaw,
                pitch: cam.pitch,
                fov: cam.fov,
            },
            &snapshot::FrameOpts {
                width: (cam.width / camera::PREVIEW_DIVISOR).max(1),
                height: (cam.height / camera::PREVIEW_DIVISOR).max(1),
                sea_level: sea_level_of(&campaign),
                labels: false,
            },
        );
        let png = delvec::compiler::png::encode_rgba(
            frame.canvas.width,
            frame.canvas.height,
            &frame.canvas.rgba,
        );
        let path = out.join(camera::preview_file(&campaign_id, &cam.name));
        if let Err(e) = write_file(&path, &png) {
            eprintln!("internal error: cannot write {}: {e}", path.display());
            return ExitCode::from(EXIT_INTERNAL);
        }
    }
    if bracket.is_some() {
        let path = out.join(camera::CANDIDATES_FILE);
        let written = camera::candidates_bytes(&campaign_id, &cameras)
            .map_err(|d| d.message)
            .and_then(|b| write_file(&path, &b).map_err(|e| e.to_string()));
        if let Err(msg) = written {
            eprintln!("internal error: cannot write {}: {msg}", path.display());
            return ExitCode::from(EXIT_INTERNAL);
        }
    }
    eprintln!(
        "previewed {} camera frame(s) -> {} (flat-lit CPU drafts for placing a camera; the \
         light is judged in the Chunky scene `delvec cameras` emits without --preview)",
        cameras.len(),
        out.display()
    );
    eprintln!(
        "lens: {} of {} camera(s) clear of every block by {} block, {obstructed} flagged",
        cameras.len() - obstructed,
        cameras.len(),
        camera::LENS_CLEARANCE
    );
    eprintln!("{}", answers.line());
    ExitCode::SUCCESS
}

/// The manifest sidecar path for an output image: the image path with its
/// extension replaced by `manifest.json` (`shot.png` → `shot.manifest.json`).
/// A path with no extension simply gains one.
pub(super) fn manifest_path_for(out: &Path) -> PathBuf {
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
) -> Result<delvec::compiler::assembled::Assembled, u8> {
    match delvec::compiler::edit::replay_view(plan, prefabs, structures) {
        Ok(Some(er)) => Ok(er.assembled),
        Ok(None) => Ok(delvec::compiler::assembled::assemble(plan, structures)),
        Err(e) => {
            print_build_error(e.code, &e.message, json);
            Err(3)
        }
    }
}

/// The `ocean`-horizon sea level to draw as a background plane, or `None` for a
/// `void`-horizon campaign (see `snapshot::SEA_PLANE_NOTE`).
pub(super) fn sea_level_of(campaign: &delvewright_dsl::Campaign) -> Option<i32> {
    match delvewright_dsl::horizon_base(&campaign.world.content.horizon) {
        delvewright_dsl::HorizonBase::Ocean => Some(delvec::compiler::plan::SEA_LEVEL),
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

/// Decide the snapshot camera from the mutually-exclusive framing flags.
///
/// Precedence: `--camera` (explicit) → `--at` (subject framing) → `--shot`
/// (reuse a render-plan camera) → the default layout overview. Clap already
/// rejects combining the first three.
fn resolve_camera(
    plan: &Plan,
    prefabs: &PrefabRegistry,
    grid: &delvec::compiler::snapshot::VoxelGrid,
    world: &delvec::compiler::nav::World,
    args: &SnapshotArgs<'_>,
) -> Result<delvec::compiler::snapshot::Camera, String> {
    use delvec::compiler::snapshot::{Camera, DEFAULT_FOV, DEFAULT_ORBIT_DIST};

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
pub(super) fn pull_into_open_air(
    grid: &delvec::compiler::snapshot::VoxelGrid,
    subject: [f64; 3],
    eye: [f64; 3],
) -> [f64; 3] {
    delvec::compiler::camera::stand_in_open_air(|c| grid.solid(c), subject, eye).unwrap_or(eye)
}

/// Resolve an `--at` subject to a world cell. Accepts a bare anchor name
/// (`anchor/fire-pit`, matched in the first declaring area, `BTreeMap` order) or
/// an `area:anchor` pair (`area/island:anchor/pen`) to disambiguate.
fn resolve_subject(plan: &Plan, subject: &str) -> Result<[i32; 3], String> {
    use delvec::compiler::plan::ResolvedAnchor;
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
    world: &delvec::compiler::nav::World,
    id: &str,
) -> Result<delvec::compiler::snapshot::Camera, String> {
    use delvec::compiler::render_plan;
    use delvec::compiler::snapshot::{Camera, DEFAULT_FOV};

    // The path the build reads: the links the route proof takes spliced in
    // (spec-0083), so a `pov/…` id names the leg the build's own render plan
    // named.
    let relinked = delvec::compiler::nav::with_links_taken(plan, prefabs, world)
        .map_err(|f| format!("{}: {}", f.code, f.message))?;
    let plan = relinked.as_ref().unwrap_or(plan);
    let pov = if id.starts_with("pov/") {
        let routes = delvec::compiler::nav::critical_path_routes(plan, world);
        render_plan::pov_shots(plan, &routes)
    } else {
        Vec::new()
    };
    // The same world the snapshot itself rasterises (the EDITED assembled model),
    // so the camera this returns is the camera the plan states: a shot pulled in
    // out of the rock is pulled in by the same walk here, and a `DW0724` refusal
    // here is one `delvec build` would raise too.
    let doc = render_plan::render_plan(plan, prefabs, &pov, world, None)
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

/// `delvec blocking-chart <campaign-dir> [-o dir]` — the spec-0015 pillar-3
/// cutaway floor plans.
///
/// Needs the same three stages `snapshot` does (parse → placement → assembled
/// blocks) plus the nav occupancy model, because "walkable" is what the bands
/// are found from and the corridor overlay is the DW0311-proven critical path.
/// Routing is best-effort: a campaign whose critical path does not route yet
/// simply charts without the corridor tint rather than refusing to chart.
pub(crate) fn run_blocking_chart(
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
            print_diags(&e.warnings, json);
            print_build_error(e.failure.code, &e.failure.message, json);
            return ExitCode::from(3);
        }
    };
    // The placement stage's own advisories (`DW0498`). `build`/`edit` get these
    // through `emit::build_with_warnings`; the view commands never emit, so they
    // report them here — a draw that repeats an anchored piece is exactly what a
    // reviewer is looking at in a snapshot.
    print_diags(&plan.warnings.clone(), json);
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
    let world = delvec::compiler::nav::World::from_occupancy(
        delvec::compiler::assembled::occupancy_over(&assembled.blocks, &assembled.open_gates),
        delvec::compiler::nav::Premises::of_plan(&plan, assembled.gate_seals.clone()),
    );
    let blocks = assembled.blocks;
    let targets = delvec::compiler::snapshot::collect_targets(&plan);
    // The build's path: the links the route proof takes spliced in (spec-0083).
    let relinked = match delvec::compiler::nav::with_links_taken(&plan, &prefabs, &world) {
        Ok(r) => r,
        Err(f) => {
            eprintln!("{} [error] build: {}", f.code, f.message);
            return ExitCode::from(f.code.exit_tier().exit_status());
        }
    };
    let corridor: std::collections::BTreeSet<[i32; 3]> =
        delvec::compiler::nav::critical_path_routes(relinked.as_ref().unwrap_or(&plan), &world)
            .into_iter()
            .flat_map(|leg| leg.cells)
            .collect();
    let chart = delvec::compiler::blocking::chart(&plan, &blocks, &world, &targets, &corridor);
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
