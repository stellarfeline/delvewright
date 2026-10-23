//! `delvec frames --campaign`, the camera stand check and `blocking-chart`:
//! the view arms that read the binary's campaign loader.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use delvec::compiler::framing;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;

use crate::EXIT_INTERNAL;
use crate::cli::campaign::{read_structures, validate_stage};
use crate::cli::report::{print_build_error, print_diags, write_file};

/// What a `frames --campaign` frame shows: the targets in and out of its frame
/// by the one sight rule ([`framing::resolve_targets`]), and the `shows` its
/// labels are stamped from — each entry the manifest's, with the body a post
/// names (`body`).
#[allow(clippy::type_complexity)]
pub(crate) fn snapshot_frame<'a>(
    grid: &framing::VoxelGrid,
    cam: &framing::Camera,
    (width, height): (u32, u32),
    targets: &'a [framing::Target],
    body: &dyn Fn(&framing::Target) -> Option<String>,
) -> (
    Vec<framing::ResolvedTarget<'a>>,
    Vec<&'a framing::Target>,
    Vec<serde_json::Value>,
) {
    let (inside, outside) = framing::resolve_targets(grid, cam, width, height, targets);
    let shows = inside
        .iter()
        .map(|r| {
            let mut v = r.entry();
            if let Some(b) = body(r.target()) {
                v["body"] = serde_json::json!(b);
            }
            v
        })
        .collect();
    (inside, outside, shows)
}

/// The `frames --campaign` arguments, bundled so the dispatcher stays legible.
pub(crate) struct FramesCampaignArgs<'a> {
    pub(crate) camera: Option<&'a str>,
    pub(crate) at: Option<&'a str>,
    pub(crate) orbit: f64,
    pub(crate) dist: Option<f64>,
    pub(crate) out: Option<&'a Path>,
    pub(crate) labels: bool,
    pub(crate) size: u32,
    pub(crate) draw: &'a delvec::compiler::view::cli::DrawArgs,
}

/// `delvec frames --campaign <dir> (--camera … | --at …)` — the authoring
/// loop's viewport (spec-0015): one quick frame of the campaign's load world,
/// drawn by Lumenwright, with its manifest beside it.
///
/// ## Which pipeline stages this needs
///
/// Placement and assembly, and no more (spec-0015: "works on a partial
/// build"): parse the campaign → [`Plan::build`] → read the placed `.nbt`
/// structures → the assembled world, its load configuration written in this
/// process under the output's directory (`world/`). **No emission**: no
/// datapack, no nav proofs — so a campaign whose quests are half-written still
/// draws. Validation diagnostics are printed but never gate the frame. Only an
/// unparseable campaign (exit 1) or a placement failure (exit 3) stops it.
pub(crate) fn run_frames_campaign(
    campaign_dir: &Path,
    prefabs_dir: &Path,
    args: FramesCampaignArgs<'_>,
    json: bool,
) -> ExitCode {
    use delvec::compiler::view::frameset;

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
    print_diags(&plan.warnings.clone(), json);
    let structures = match read_structures(&plan, &prefabs, prefabs_dir, json) {
        Ok(s) => s,
        Err(code) => return ExitCode::from(code),
    };
    let assembled = match edited_assembled(&plan, &prefabs, &structures, json) {
        Ok(a) => a,
        Err(code) => return ExitCode::from(code),
    };
    let (world, relight) = camera_world(&plan, &assembled);
    let base = camera_base(&plan, &assembled, &relight);
    let load = delvec::compiler::view::beat::load_blocks(&plan, &world, &base);
    let grid = framing::VoxelGrid::build(&load);
    let cam = match resolve_camera(&plan, &grid, &args) {
        Ok(c) => c,
        Err(msg) => {
            eprintln!("error: {msg}");
            return ExitCode::from(1);
        }
    };
    let png_path = args
        .out
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("frame.png"));
    let dir = png_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let world_dir = dir.join("world");
    let maker =
        delvec::compiler::view::review::WorldMaker::new(&plan, &assembled, &base, &structures);
    let laid = delvec::compiler::view::beat::load_laid(&plan, &world);
    let day_time = delvec::compiler::view::review::Clock::at(&plan, 0).day_time(&plan);
    let (_, written) = maker.encode_blocks(&load, &laid, day_time, &[]);
    if let Err(e) = delvec::compiler::view::world::write_files(&written, &world_dir) {
        eprintln!("internal error: cannot write {}: {e}", world_dir.display());
        return ExitCode::from(EXIT_INTERNAL);
    }
    eprintln!(
        "{}",
        delvec::compiler::view::review::world_line(
            delvec::compiler::view::beat::AT_LOAD,
            &written,
            &world_dir.display().to_string(),
            0
        )
    );
    let jar = match delvec::compiler::view::cli::resolve_textures(args.draw.textures.as_deref()) {
        Ok(j) => j,
        Err(d) => {
            d.print(json);
            return ExitCode::from(delvec::compiler::view::diag::exit::RENDER);
        }
    };
    let clock = delvec::compiler::view::review::Clock::at(&plan, 0);
    let (lo, hi) = maker.extent;
    let drawn = match frameset::draw_frame(
        &delvec::compiler::view::frames::FrameRequest {
            world: &world_dir,
            chunks: delvec::compiler::view::scene::chunk_rect(lo, hi),
            jar: Path::new(&jar),
            packs: &[],
            camera: cam,
            sky: delvec::compiler::view::frames::FrameSky {
                day: clock.clock(&plan).day,
                daytime_ticks: clock.daytime_ticks(),
                weather: clock.weather,
            },
            mode: delvec::compiler::view::frames::Mode::Quick,
            backend: args.draw.backend,
            width: args.size,
            height: args.size,
        },
        args.draw.threads,
    ) {
        Ok(d) => d,
        Err(d) => {
            d.print(json);
            return ExitCode::from(delvec::compiler::view::diag::exit::RENDER);
        }
    };
    eprintln!("{}", frameset::renderer_line(&drawn, args.draw.backend));
    let targets = framing::collect_targets(&plan);
    let (inside, outside, shows) =
        snapshot_frame(&grid, &cam, (drawn.width, drawn.height), &targets, &|t| {
            framing::body_of(&plan, t)
        });
    let gone = frameset::undrawn(&shows, &drawn.stats);
    let mut rgba = drawn.rgba.clone();
    if args.labels {
        frameset::stamp_labels(&mut rgba, drawn.width, drawn.height, &shows);
    }
    let png = delvec::compiler::png::encode_rgba(drawn.width, drawn.height, &rgba);
    let image_name = png_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "frame.png".to_string());
    let doc = framing::manifest(
        campaign.world.campaign_id.as_str(),
        &image_name,
        &cam,
        &framing::Picture {
            width: drawn.width,
            height: drawn.height,
            rgba: &rgba,
        },
        &grid,
        framing::Scene {
            pieces: &framing::collect_pieces(&plan),
            inside: &inside,
            outside: &outside,
        },
    );
    let mut doc = doc;
    doc["renderer"] = serde_json::json!(delvec::compiler::view::frames::renderer());
    doc["backend"] = serde_json::json!(drawn.backend_name());
    doc["mode"] = serde_json::json!("quick");
    doc["world"]["key"] = serde_json::json!(delvec::compiler::view::beat::AT_LOAD);
    doc["undrawn"] = serde_json::json!(gone);
    doc["renderer_stats"] = drawn.stats.to_json();
    let manifest_path = manifest_path_for(&png_path);
    let mut manifest_bytes = match serde_json::to_vec_pretty(&doc) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("internal error: cannot serialize manifest: {e}");
            return ExitCode::from(EXIT_INTERNAL);
        }
    };
    manifest_bytes.push(b'\n');
    if let Err(e) =
        write_file(&png_path, &png).and_then(|()| write_file(&manifest_path, &manifest_bytes))
    {
        eprintln!("internal error: cannot write the frame: {e}");
        return ExitCode::from(EXIT_INTERNAL);
    }
    if json {
        println!(
            "{}",
            serde_json::json!({
                "png": png_path.display().to_string(),
                "manifest": manifest_path.display().to_string(),
                "in_frame": inside.len(),
                "out_of_frame": outside.len(),
                "undrawn": gone.len(),
            })
        );
    } else {
        println!(
            "{} ({}×{}, quick, {:.2} s) + {} — {} target(s) in frame, {} out, {} not drawn by the \
             renderer",
            png_path.display(),
            drawn.width,
            drawn.height,
            drawn.seconds(),
            manifest_path.display(),
            inside.len(),
            outside.len(),
            gone.len()
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
    stood_over(campaign_dir, prefabs_dir, json, |plan, world, base| {
        delvec::compiler::view::beat::stands(plan, world, base, cameras).map_err(|why| {
            (
                Some(delvec::compiler::view::diag::Diagnostic::error(
                    delvec::compiler::view::camera::DW_RECORD_AT_BUILD.id(),
                    why,
                )),
                2,
            )
        })
    })
}

/// What a configuring closure answers: the worlds to write, or a refusal.
type StandsOrRefusal = Result<
    delvec::compiler::view::beat::Stands,
    (Option<delvec::compiler::view::diag::Diagnostic>, u8),
>;

/// The campaign assembled for a written world, and `configure` asked of the
/// plan, the world under the proofs' premises and the picture base.
fn stood_over(
    campaign_dir: &Path,
    prefabs_dir: &Path,
    json: bool,
    configure: impl FnOnce(
        &Plan,
        &delvec::compiler::nav::World,
        &delvec::compiler::blockstate::BlockMap,
    ) -> StandsOrRefusal,
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
    let (world, relight) = camera_world(&plan, &assembled);
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
    let base = camera_base(plan, &assembled, &relight);
    let stands = configure(plan, &world, &base)?;
    Ok(delvec::compiler::view::beat::Stood {
        stands,
        maker: delvec::compiler::view::review::WorldMaker::new(
            plan,
            &assembled,
            &base,
            &structures,
        ),
        day_time: delvec::compiler::view::review::Clock::at(plan, 0).day_time(plan),
        ancestors: plan.strict_ancestor_steps.clone(),
    })
}

/// **The world a view stands its cameras in, and the bytes its pictures
/// start from — the build's own, derived through the build's entry points.**
/// The world is [`delvec::compiler::light::final_world`]: the assembled cells
/// under the campaign's premises with the relight pass's fixtures standing in
/// them, the world `delvec build` walks. The bytes are
/// [`delvec::compiler::view::beat::picture_base`] over the same pass's
/// placements, asked of `path_plan` — the plan with the links the route proof
/// takes spliced in where the caller has taken them, as the build asks it. A
/// relight pass that refuses is the build's to raise; a view still draws the
/// world it would have walked.
pub(crate) fn camera_world(
    plan: &Plan,
    assembled: &delvec::compiler::assembled::Assembled,
) -> (
    delvec::compiler::nav::World,
    delvec::compiler::light::Relight,
) {
    delvec::compiler::light::final_world(plan, assembled)
}

/// See [`camera_world`]: the picture base over the relight `relight` placed.
pub(crate) fn camera_base(
    path_plan: &Plan,
    assembled: &delvec::compiler::assembled::Assembled,
    relight: &delvec::compiler::light::Relight,
) -> delvec::compiler::blockstate::BlockMap {
    delvec::compiler::view::beat::picture_base(path_plan, assembled, &relight.placements)
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

/// Parse + validate a campaign for a **view-only** command: diagnostics are
/// printed, but only a parse failure stops the run (exit 1). See
/// [`run_frames_campaign`] for why a view command must not gate on validation.
fn load_for_view(
    campaign_dir: &Path,
    prefabs_dir: &Path,
    json: bool,
) -> Result<(delvewright_dsl::Campaign, PrefabRegistry), u8> {
    let v = validate_stage(campaign_dir, prefabs_dir, json)?;
    Ok((v.campaign, v.prefabs))
}

/// Decide the frame's camera from the mutually-exclusive framing flags.
///
/// Precedence: `--camera` (explicit) → `--at` (subject framing) → the default
/// layout overview. Clap already rejects combining the first two.
fn resolve_camera(
    plan: &Plan,
    grid: &delvec::compiler::framing::VoxelGrid,
    args: &FramesCampaignArgs<'_>,
) -> Result<delvec::compiler::framing::Camera, String> {
    use delvec::compiler::framing::{Camera, DEFAULT_FOV, DEFAULT_ORBIT_DIST};

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
        // the orbit eye inside the mountain and draw the inside of the rock.
        // Pull the eye along its own sight line until it stands in open air, so
        // `--at` frames an interior without the author having to guess a distance.
        return Ok(Camera::looking_at(
            pull_into_open_air(grid, look, eye),
            look,
            DEFAULT_FOV,
        ));
    }

    // Default: a dollhouse overview of the whole layout from the south-east,
    // high enough that the full AABB fits the vertical FOV.
    let (lo, hi) = grid
        .bounds()
        .ok_or_else(|| "the assembled world is empty — nothing to draw".to_string())?;
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
/// render plan's own cameras.
pub(super) fn pull_into_open_air(
    grid: &delvec::compiler::framing::VoxelGrid,
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

/// `delvec blocking-chart <campaign-dir> [-o dir]` — the spec-0015 pillar-3
/// cutaway floor plans.
///
/// Needs the same three stages `frames --campaign` does (parse → placement → assembled
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
    // reviewer is looking at in a frame.
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
    // The corridor is the build's: one derivation (`blocking::walked_corridor`)
    // over the world the build's proofs walk (relight fixtures, gate seals,
    // lethal volumes, clocked regions) and the path with the links the route
    // proof takes spliced in. A chart taken over a smaller premise set draws a
    // corridor no proof ever walked.
    let walked = match delvec::compiler::blocking::walked_corridor(
        &plan,
        &prefabs,
        &assembled,
        &delvec::compiler::holders::placed_holders(&plan, &structures),
    ) {
        Ok(w) => w,
        Err(f) => {
            eprintln!("{} [error] build: {}", f.code, f.message);
            return ExitCode::from(f.code.exit_tier().exit_status());
        }
    };
    let blocks = assembled.blocks;
    let targets = delvec::compiler::framing::collect_targets(&plan);
    let chart = delvec::compiler::blocking::chart(&plan, &blocks, &targets, &walked);
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
