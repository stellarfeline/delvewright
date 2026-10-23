//! **`delvec frames`: a build's review set, drawn** (spec-0112 §6.1 item 2, §9).
//!
//! Every shot of the build's `render-plan.json` (or the `--shot`s named) is
//! drawn from the world the build wrote for the configuration it stands in
//! (`validation/worlds/<key>/`), in the mode the plan derived for it, under the
//! sky the plan stated, through [`crate::compiler::view::frames::draw`] — in
//! this process, by Lumenwright. Beside the frames: one manifest per frame and
//! `shot-index.json`, each entry stating what the frame is of, what it shows
//! (`validation/review-frames.json`'s `shows`, computed by the build with the
//! one sight rule), which bodies in it the renderer did not draw (`DW1039`),
//! and what no frame shows.
//!
//! The first line of every run names the renderer and the backend; the last
//! lines are the binding lines: how many frames over how many worlds in how
//! long at what peak memory, how many objects in frame and how many not drawn,
//! and how many skies a step set.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use delvewright_dsl::{DwCode, ExitTier};
use serde_json::{Value, json};

use crate::compiler::view::cli::{DrawArgs, fail, resolve_textures};
use crate::compiler::view::diag::{DW_INPUT, DW_OUTPUT, Diagnostic, exit};
use crate::compiler::view::frames::{self, Backend, Drawn, FrameRequest};
use crate::compiler::view::index::{self, IndexEntry, PlanShot};

delvewright_dsl::dw_code! {
    /// `DW1039`: **a frame posts a body the renderer did not draw** (spec-0112
    /// §8.1). A review frame whose `shows` holds a body the renderer's own
    /// statistics for that frame put among the kinds it skipped. Printed once
    /// per frame by `delvec frames` — the shot, the body's id, kind, cell,
    /// distance, the reason and how to judge it — and written into the shot
    /// index as `undrawn`. A report, never a refusal: the frame is right and the
    /// body is real; what it protects is a reviewer reading empty floor as an
    /// absent keeper.
    pub const DW_FRAME_UNDRAWN: DwCode = DwCode::new("DW1039", ExitTier::Build);
}

/// The directory a build's frames are drawn to by default: `<build>/frames`.
pub const FRAMES_DIR: &str = "frames";

/// The shot index's file name, beside the frames.
pub const INDEX_FILE: &str = "shot-index.json";

/// What a `delvec frames` run is asked.
#[derive(Debug, Clone)]
pub struct FramesOptions {
    pub out: PathBuf,
    /// Only these shots, by id; empty = every shot.
    pub shots: Vec<String>,
    pub labels: bool,
    pub size: u32,
    pub draw: DrawArgs,
}

/// The directory of the world a build wrote under `key`, or the refusal naming
/// the key and the remedy.
pub fn build_world(build_dir: &Path, key: &str) -> Result<PathBuf, Diagnostic> {
    let dir = build_dir.join("validation").join("worlds").join(key);
    if dir.join("level.dat").is_file() {
        return Ok(dir);
    }
    Err(Diagnostic::error(
        DW_INPUT,
        format!(
            "{} holds no world `{key}` ({} is not there): a frame stands in the world the build \
             wrote for its configuration, and this build directory does not carry it — it was \
             written by an engine that writes no review worlds, or it is incomplete. Rebuild \
             the delve with `delvec build`",
            build_dir.display(),
            dir.join("level.dat").display()
        ),
    ))
}

/// The build's resource pack, when it has one (spec-0084, spec-0009): read
/// from the build directory, never from a flag.
pub fn build_packs(build_dir: &Path) -> Vec<PathBuf> {
    let p = build_dir.join("resourcepack.zip");
    if p.is_file() { vec![p] } else { Vec::new() }
}

/// **Draw one frame**, on a pool of `threads` threads when one is stated.
pub fn draw_frame(req: &FrameRequest<'_>, threads: Option<usize>) -> Result<Drawn, Diagnostic> {
    draw_frame_in(req, None, threads)
}

/// [`draw_frame`] over a world already opened.
fn draw_frame_in(
    req: &FrameRequest<'_>,
    opened: Option<&frames::OpenedWorld>,
    threads: Option<usize>,
) -> Result<Drawn, Diagnostic> {
    match threads {
        None => frames::draw(req, opened),
        Some(n) => {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(n.max(1))
                .build()
                .map_err(|e| {
                    Diagnostic::error(
                        crate::compiler::view::diag::DW_RENDER,
                        format!("a pool of {n} thread(s): {e}"),
                    )
                })?;
            pool.install(|| frames::draw(req, opened))
        }
    }
}

/// The first line of every run that draws: the renderer and the backend that
/// drew, and whether `auto` fell back to the CPU.
pub fn renderer_line(d: &Drawn, asked: Backend) -> String {
    format!(
        "renderer: {}, backend {} ({})",
        frames::renderer(),
        d.backend,
        match (&d.fallback, asked) {
            (Some(why), _) => format!("auto: fell back to cpu — {why}"),
            (None, Backend::Auto) => "auto".to_string(),
            (None, _) => "named".to_string(),
        }
    )
}

/// Write a drawn frame as a PNG (the engine's own encoder, a fixed DEFLATE
/// level: the bytes are a function of the pixels).
pub fn write_png(path: &Path, d: &Drawn) -> Result<(), Diagnostic> {
    write_rgba(path, d.width, d.height, &d.rgba)
}

fn write_rgba(path: &Path, w: u32, h: u32, rgba: &[u8]) -> Result<(), Diagnostic> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| {
            Diagnostic::error(DW_OUTPUT, format!("mkdir {}: {e}", parent.display()))
        })?;
    }
    let png = crate::compiler::png::encode_rgba(w, h, rgba);
    std::fs::write(path, png)
        .map_err(|e| Diagnostic::error(DW_OUTPUT, format!("write {}: {e}", path.display())))
}

/// This process's peak resident memory in bytes, where the platform reports
/// it — the one reading of `getrusage` the binary has; `delvec detail --check`
/// prints it and [`peak_mib`] scales it.
#[must_use]
pub fn peak_rss_bytes() -> Option<u64> {
    #[cfg(unix)]
    {
        let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
        // SAFETY: `getrusage` writes the struct it is handed and nothing else.
        if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut ru) } != 0 {
            return None;
        }
        let raw = u64::try_from(ru.ru_maxrss).ok()?;
        // macOS reports bytes, Linux kilobytes.
        Some(if cfg!(target_os = "macos") {
            raw
        } else {
            raw * 1024
        })
    }
    #[cfg(not(unix))]
    {
        None
    }
}

/// This process's peak resident memory, MiB, where the platform reports it.
pub fn peak_mib() -> Option<f64> {
    peak_rss_bytes().map(|b| b as f64 / (1024.0 * 1024.0))
}

/// A `shows` entry's screen box at `size` pixels, from the box the build
/// resolved at `at` pixels: a square frame's projection scales linearly.
fn scaled(entry: &Value, at: u32, size: u32) -> Value {
    if at == size {
        return entry.clone();
    }
    let mut v = entry.clone();
    let k = f64::from(size) / f64::from(at);
    if let Some(b) = v.get_mut("screen_bbox") {
        for key in ["x", "y", "w", "h"] {
            if let Some(n) = b[key].as_i64() {
                b[key] = json!((n as f64 * k).round() as i64);
            }
        }
    }
    v
}

/// Stamp every target's outline and name on a frame — visible targets first,
/// an occluded name only where it lands clear.
pub fn stamp_labels(rgba: &mut Vec<u8>, w: u32, h: u32, shows: &[Value]) {
    use crate::compiler::raster::{
        Canvas, GLYPH_ROWS, LabelPlacer, ScreenBox, kind_color, text_width,
    };
    let mut canvas = Canvas {
        width: w,
        height: h,
        rgba: std::mem::take(rgba),
    };
    let boxed: Vec<(ScreenBox, &str, String, bool)> = shows
        .iter()
        .filter_map(|s| {
            let b = &s["screen_bbox"];
            Some((
                ScreenBox {
                    x: b["x"].as_i64()?,
                    y: b["y"].as_i64()?,
                    w: b["w"].as_i64()?,
                    h: b["h"].as_i64()?,
                },
                s["kind"].as_str().unwrap_or(""),
                s["id"]
                    .as_str()
                    .unwrap_or("")
                    .rsplit('/')
                    .next()
                    .unwrap_or("")
                    .to_ascii_uppercase(),
                s["occluded"].as_bool().unwrap_or(false),
            ))
        })
        .collect();
    for (b, kind, _, occluded) in &boxed {
        canvas.stroke_rect(*b, kind_color(kind), if *occluded { 0.45 } else { 0.95 });
    }
    let mut placer = LabelPlacer::default();
    for pass_occluded in [false, true] {
        for (b, kind, text, occluded) in boxed.iter().filter(|e| e.3 == pass_occluded) {
            let _ = occluded;
            let tw = text_width(text, 1);
            let th = GLYPH_ROWS;
            let lx = b.x.min(i64::from(w) - tw - 2).max(2);
            let ly = if b.y - th - 3 < 2 {
                b.y + b.h + 3
            } else {
                b.y - th - 3
            };
            let want = ScreenBox {
                x: lx,
                y: ly,
                w: tw + 2,
                h: th + 2,
            };
            let nudges = if pass_occluded { 0 } else { 24 };
            let Some(placed) = placer.place(want, th + 4, nudges) else {
                continue;
            };
            canvas.stamp_text(lx, placed.y, text, 1, kind_color(kind));
        }
    }
    *rgba = canvas.rgba;
}

/// What the renderer did not draw in one frame: every `shows` body whose kind
/// its statistics skipped or drew as a box, with the reason in words and how
/// to judge it (`DW1039`).
pub fn undrawn(shows: &[Value], stats: &frames::Stats) -> Vec<Value> {
    shows
        .iter()
        .filter_map(|s| {
            let body = s["body"].as_str()?;
            if !stats.undrew(body) {
                return None;
            }
            let reason = if frames::HOLDS_ITEMS.contains(&body) && !stats.items_skipped.is_empty() {
                format!(
                    "an item the renderer cannot draw in this frame: {}",
                    stats
                        .items_skipped
                        .keys()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            } else {
                "no model in the renderer".to_string()
            };
            let judge_by = if s["occluded"].as_bool().unwrap_or(false) {
                "in game: a block stands between it and this camera"
            } else {
                "by its row in validation/placed.json (what it holds), or in game"
            };
            let mut v = json!({
                "id": s["id"],
                "kind": s["kind"],
                "body": body,
                "distance": s["distance"],
                "reason": reason,
                "judge_by": judge_by,
            });
            for k in ["pos", "box", "facing"] {
                if let Some(x) = s.get(k) {
                    v[k] = x.clone();
                }
            }
            Some(v)
        })
        .collect()
}

/// The `unchecked` list of one frame: what no frame shows, with the items this
/// frame's own state makes apply (spec-0112 §11).
fn unchecked_of(shot: &PlanShot) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if shot.sky.weather != delvewright_dsl::WorldWeather::Clear {
        out.push(
            "the rain in the air: the sky's colours under the weather are drawn, the rain is not"
                .into(),
        );
    }
    let stamp: Option<crate::compiler::view::scene::LightingStamp> = shot
        .lighting
        .clone()
        .and_then(|v| serde_json::from_value(v).ok());
    if crate::compiler::view::scene::needs_emulation(stamp.as_ref()) {
        out.push(
            "a night-vision room's lightmap: drawn without the effect — judge in game under night \
             vision"
                .into(),
        );
    }
    if shot.kind == "cutscene" {
        out.push("a body's pose at this tick".into());
    }
    out
}

/// **`delvec frames <build-dir>`.**
pub fn run_frames(build_dir: &Path, opts: &FramesOptions, json: bool) -> ExitCode {
    let started = std::time::Instant::now();
    let read = |p: PathBuf| {
        std::fs::read(&p)
            .map_err(|e| Diagnostic::error(DW_INPUT, format!("read {}: {e}", p.display())))
    };
    let plan_bytes = match read(build_dir.join("render-plan.json")) {
        Ok(b) => b,
        Err(d) => return fail(d, json, exit::INPUT),
    };
    let (campaign_id, shots) = match index::plan_shots(&plan_bytes) {
        Ok(s) => s,
        Err(d) => return fail(d, json, exit::INPUT),
    };
    let record: Value = match read(build_dir.join("validation").join("review-frames.json"))
        .and_then(|b| {
            serde_json::from_slice(&b).map_err(|e| {
                Diagnostic::error(
                    DW_INPUT,
                    format!("parse validation/review-frames.json: {e}"),
                )
            })
        }) {
        Ok(r) => r,
        Err(d) => return fail(d, json, exit::INPUT),
    };
    let rows: BTreeMap<String, Value> = record["shots"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|r| Some((r["id"].as_str()?.to_string(), r.clone())))
                .collect()
        })
        .unwrap_or_default();
    // `--shot` names shots of this plan.
    for want in &opts.shots {
        if !shots.iter().any(|s| &s.id == want) {
            let mut ids: Vec<&str> = shots.iter().map(|s| s.id.as_str()).collect();
            ids.sort_unstable();
            return fail(
                Diagnostic::error(
                    DW_INPUT,
                    format!(
                        "--shot `{want}` is not in this build's render plan. Shots: {}{}",
                        ids.iter().take(24).copied().collect::<Vec<_>>().join(", "),
                        if ids.len() > 24 { ", …" } else { "" }
                    ),
                ),
                json,
                exit::INPUT,
            );
        }
    }
    let chosen: Vec<&PlanShot> = shots
        .iter()
        .filter(|s| opts.shots.is_empty() || opts.shots.contains(&s.id))
        .collect();
    // Every world the chosen shots stand in, refused by name when absent.
    let mut worlds: BTreeMap<String, PathBuf> = BTreeMap::new();
    for s in &chosen {
        if worlds.contains_key(&s.world) {
            continue;
        }
        match build_world(build_dir, &s.world) {
            Ok(d) => worlds.insert(s.world.clone(), d),
            Err(d) => return fail(d, json, exit::INPUT),
        };
    }
    let jar = match resolve_textures(opts.draw.textures.as_deref()) {
        Ok(j) => j,
        Err(d) => return fail(d, json, exit::RENDER),
    };
    let packs = build_packs(build_dir);
    let chunks = match crate::compiler::view::scene::parse_plan(&plan_bytes) {
        Ok(p) => {
            let (lo, hi) = crate::compiler::view::scene::loaded_extent(&p.layout_aabb, p.horizon);
            crate::compiler::view::scene::chunk_rect(lo, hi)
        }
        Err(d) => return fail(d, json, exit::INPUT),
    };
    let mut opened: BTreeMap<String, frames::OpenedWorld> = BTreeMap::new();
    for (k, dir) in &worlds {
        match frames::OpenedWorld::open(dir) {
            Ok(o) => {
                opened.insert(k.clone(), o);
            }
            Err(d) => return fail(d, json, exit::INPUT),
        }
    }
    if let Err(e) = std::fs::create_dir_all(&opts.out) {
        return fail(
            Diagnostic::error(DW_OUTPUT, format!("mkdir {}: {e}", opts.out.display())),
            json,
            exit::OUTPUT,
        );
    }
    let mut entries: Vec<IndexEntry> = Vec::new();
    let mut first = true;
    let (mut in_frame, mut not_drawn, mut frames_with_undrawn, mut set_by_step) =
        (0usize, 0usize, 0usize, 0usize);
    let mut frame_seconds = Vec::new();
    for shot in &chosen {
        let row = rows.get(&shot.id);
        let at = row
            .and_then(|r| r["size"].as_u64())
            .map_or(frames::REVIEW_SIZE, |n| n as u32);
        let shows: Vec<Value> = row
            .and_then(|r| r["shows"].as_array())
            .map(|a| a.iter().map(|e| scaled(e, at, opts.size)).collect())
            .unwrap_or_default();
        let out_of_frame: Vec<Value> = row
            .and_then(|r| r["out_of_frame"].as_array())
            .cloned()
            .unwrap_or_default();
        let drawn = match draw_frame_in(
            &FrameRequest {
                world: &worlds[&shot.world],
                chunks,
                jar: Path::new(&jar),
                packs: &packs,
                camera: shot.camera,
                sky: shot.sky,
                mode: shot.mode,
                backend: opts.draw.backend,
                width: opts.size,
                height: opts.size,
            },
            opened.get(&shot.world),
            opts.draw.threads,
        ) {
            Ok(d) => d,
            Err(d) => return fail(d, json, exit::RENDER),
        };
        if first {
            eprintln!("{}", renderer_line(&drawn, opts.draw.backend));
            first = false;
        }
        frame_seconds.push(drawn.seconds());
        let gone = undrawn(&shows, &drawn.stats);
        // One report per frame (spec-0112 §8.1), naming every body in it the
        // renderer did not draw.
        if !gone.is_empty() {
            let bodies: Vec<String> = gone
                .iter()
                .map(|u| {
                    format!(
                        "`{}` ({}) at {} facing {}, {} blocks away — {}; judge it {}",
                        u["id"].as_str().unwrap_or(""),
                        u["body"].as_str().unwrap_or(""),
                        u.get("pos")
                            .or(u.get("box"))
                            .cloned()
                            .unwrap_or(Value::Null),
                        u["facing"].as_str().unwrap_or("-"),
                        u["distance"],
                        u["reason"].as_str().unwrap_or(""),
                        u["judge_by"].as_str().unwrap_or("")
                    )
                })
                .collect();
            crate::compiler::view::diag::Diagnostic::warning(
                DW_FRAME_UNDRAWN.id(),
                format!(
                    "shot `{}`: {} body(ies) in frame the renderer did not draw: {}",
                    shot.id,
                    gone.len(),
                    bodies.join("; ")
                ),
            )
            .print(json);
        }
        in_frame += shows.len();
        not_drawn += gone.len();
        if !gone.is_empty() {
            frames_with_undrawn += 1;
        }
        if !shot.sky_json["set_by"].is_null() {
            set_by_step += 1;
        }
        let stem = frames::frame_stem(&shot.id);
        let image = format!("{stem}.png");
        let mut rgba = drawn.rgba.clone();
        if opts.labels {
            stamp_labels(&mut rgba, drawn.width, drawn.height, &shows);
        }
        if let Err(d) = write_rgba(&opts.out.join(&image), drawn.width, drawn.height, &rgba) {
            return fail(d, json, exit::OUTPUT);
        }
        let unchecked = unchecked_of(shot);
        let manifest = json!({
            "manifest_version": 3,
            "campaign_id": campaign_id,
            "delvec": crate::compiler::DELVEC_VERSION,
            "renderer": frames::renderer(),
            "backend": drawn.backend_name(),
            "shot": shot.id,
            "image": { "path": image, "width": drawn.width, "height": drawn.height },
            "camera": {
                "pos": shot.camera.pos,
                "yaw": shot.camera.yaw,
                "pitch": shot.camera.pitch,
                "fov": shot.camera.fov,
                "convention": "minecraft degrees: yaw 0 = south (+Z), 90 = west (-X); \
                               pitch positive looks down; fov is vertical",
            },
            "world": shot.world,
            "sky": shot.sky_json,
            "mode": shot.mode.keyword(),
            "targets": shows,
            "out_of_frame": out_of_frame,
            "undrawn": gone,
            "unchecked": unchecked,
            "renderer_stats": drawn.stats.to_json(),
            "frame": {
                "targets_in_frame": shows.len(),
                "targets_out_of_frame": out_of_frame.len(),
                "featureless": crate::compiler::view::detect::is_featureless(
                    &rgba, drawn.width, drawn.height,
                )
                .map(|f| json!({ "distinct_colors": f.distinct }))
                .unwrap_or(Value::Null),
            },
        });
        let mut mbytes = serde_json::to_vec_pretty(&manifest).expect("a manifest serializes");
        mbytes.push(b'\n');
        let mpath = opts.out.join(format!("{stem}.manifest.json"));
        if let Err(e) = std::fs::write(&mpath, mbytes) {
            return fail(
                Diagnostic::error(DW_OUTPUT, format!("write {}: {e}", mpath.display())),
                json,
                exit::OUTPUT,
            );
        }
        entries.push(IndexEntry {
            id: shot.id.clone(),
            kind: shot.kind.clone(),
            leg: shot.leg,
            objective: shot.objective.clone(),
            image,
            expect: shot.expect.clone(),
            lighting: shot.lighting.clone(),
            world: shot.world.clone(),
            sky: shot.sky_json.clone(),
            mode: shot.mode.keyword(),
            backend: drawn.backend_name(),
            renderer: frames::renderer(),
            shows,
            undrawn: gone,
            out_of_frame,
            unchecked,
            cutscene: shot.cutscene.clone(),
        });
    }
    let idx_path = opts.out.join(INDEX_FILE);
    if let Err(e) = std::fs::write(&idx_path, index::index_bytes(&campaign_id, &entries)) {
        return fail(
            Diagnostic::error(DW_OUTPUT, format!("write {}: {e}", idx_path.display())),
            json,
            exit::OUTPUT,
        );
    }
    let mut sorted = frame_seconds.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let pick = |q: f64| -> f64 {
        if sorted.is_empty() {
            0.0
        } else {
            sorted[((sorted.len() - 1) as f64 * q).round() as usize]
        }
    };
    eprintln!(
        "frames: {} of {} shot(s) rendered over {} world(s) in {:.2} s, peak {} MiB; per frame \
         {:.3} / {:.3} / {:.3} s (min / median / max) -> {}",
        entries.len(),
        chosen.len(),
        worlds.len(),
        started.elapsed().as_secs_f64(),
        peak_mib().map_or("n/a".to_string(), |m| format!("{m:.0}")),
        pick(0.0),
        pick(0.5),
        pick(1.0),
        idx_path.display()
    );
    eprintln!(
        "shows: {in_frame} object(s) in frame over {} frame(s), {not_drawn} not drawn by the \
         renderer (DW1039) in {frames_with_undrawn} frame(s); skies: {} at the world's clock, \
         {set_by_step} set by a step",
        entries.len(),
        entries.len() - set_by_step
    );
    if json {
        println!(
            "{}",
            json!({
                "frames": entries.len(),
                "worlds": worlds.len(),
                "index": idx_path.display().to_string(),
                "in_frame": in_frame,
                "not_drawn": not_drawn,
            })
        );
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(kind: &str) -> Value {
        json!({"id": format!("dw_{kind}@1,2,3"), "kind": "body", "body": kind,
               "distance": 4.0, "occluded": false, "pos": [1, 2, 3]})
    }

    fn counts(kinds: &[&str]) -> BTreeMap<String, usize> {
        kinds.iter().map(|k| (k.to_string(), 1)).collect()
    }

    /// A kind the renderer skipped is undrawn for want of a model; a drawn
    /// kind is not.
    #[test]
    fn a_skipped_kind_is_undrawn_and_a_drawn_one_is_not() {
        let stats = frames::Stats {
            entities_rendered: counts(&["mannequin"]),
            entities_skipped: counts(&["bat"]),
            ..Default::default()
        };
        let gone = undrawn(&[body("bat"), body("mannequin")], &stats);
        assert_eq!(gone.len(), 1, "{gone:?}");
        assert_eq!(gone[0]["body"], "bat");
        assert_eq!(gone[0]["reason"], "no model in the renderer");
    }

    /// The renderer counts items by item id, not by holder: a stack it could
    /// not draw puts every holder in the frame in doubt, named by the item,
    /// and moves no body that holds no item.
    #[test]
    fn an_item_not_drawn_puts_every_holder_in_doubt() {
        let mut stats = frames::Stats {
            entities_rendered: counts(&["item_display", "villager"]),
            items_rendered: counts(&["lantern"]),
            ..Default::default()
        };
        let shows = [body("item_display"), body("villager")];
        assert_eq!(undrawn(&shows, &stats), Vec::<Value>::new());
        stats.items_skipped = counts(&["shield"]);
        let gone = undrawn(&shows, &stats);
        assert_eq!(gone.len(), 1, "{gone:?}");
        assert_eq!(gone[0]["body"], "item_display");
        assert_eq!(
            gone[0]["reason"],
            "an item the renderer cannot draw in this frame: shield"
        );
    }
}
