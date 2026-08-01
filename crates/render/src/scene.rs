//! Chunky scene emission (spec-0007 whole-scene renders / spec-0003 visual tier).
//!
//! Converts the compiler's `render-plan.json` into **Chunky** scene description
//! JSONs — one per shot. Chunky itself is **not** bundled (GPLv3, out-of-process,
//! headless under xvfb; the snapshot-core version verified by the spike is pinned
//! in `versions.toml [render]` + the README). Actually *running* Chunky stays
//! manual / CI-future; emitting correct scenes is the deliverable here, pinned by
//! a golden-file test.
//!
//! ## Camera convention
//!
//! `render-plan.json` gives each camera as `pos` + `yaw`/`pitch` **degrees**
//! (yaw = atan2(-dz,dx): 0→+X, 90→−Z; pitch = atan2(-dy,horiz): +down). The spike
//! verified Chunky's scene camera uses the same axes in **radians** (yaw ≈ π/2
//! faces −Z, positive pitch = down), so orientation is a straight
//! degrees→radians conversion. See `spike-render-fidelity` evidence
//! (`spike-output/chunky_keep_*.png`).

use serde::{Deserialize, Serialize};

use crate::diag::{DW_INPUT, Diagnostic};

/// The Chunky snapshot-core version the spike verified against 1.21.11. Recorded
/// here + in `versions.toml [render]` + the README (Chunky 1.21.x needs snapshot
/// builds; stable stops at 1.20.4).
pub const CHUNKY_CORE: &str = "chunky-core-2.5.0-SNAPSHOT.474.g156e2bb";

/// Options for scene emission.
#[derive(Debug, Clone)]
pub struct SceneOptions {
    /// Path Chunky should load the delve world from (the world extracted after a
    /// `--profile play` boot places the structures). Documented; not resolved
    /// here.
    pub world_path: String,
    /// Output image dimensions.
    pub width: u32,
    pub height: u32,
    /// Path-tracing sample target.
    pub spp_target: u32,
}

impl Default for SceneOptions {
    fn default() -> Self {
        SceneOptions {
            world_path: "world".to_string(),
            width: 1024,
            height: 1024,
            spp_target: 500,
        }
    }
}

// ---- render-plan.json (input) -------------------------------------------------

#[derive(Debug, Deserialize)]
struct RenderPlan {
    campaign_id: String,
    layout_aabb: Aabb,
    shots: Vec<Shot>,
}

#[derive(Debug, Deserialize)]
struct Aabb {
    min: [i32; 3],
    max: [i32; 3],
}

#[derive(Debug, Deserialize)]
struct Shot {
    id: String,
    #[allow(dead_code)]
    kind: String,
    camera: Cam,
}

#[derive(Debug, Deserialize)]
struct Cam {
    pos: [f64; 3],
    yaw: f64,
    pitch: f64,
    #[allow(dead_code)]
    look_at: [f64; 3],
    /// Per-shot field of view (degrees). Player-POV shots declare the first-person
    /// FOV (~70°); other kinds omit it and take the scene default.
    #[serde(default)]
    fov: Option<f64>,
}

/// Default field of view (degrees) for shots that do not declare one.
const DEFAULT_FOV_DEG: f64 = 70.0;

// ---- Chunky scene (output) ----------------------------------------------------
// Field names + order follow Chunky's scene description format (`sdfVersion`).
// serde serializes structs in declaration order, so output is deterministic.

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChunkyScene {
    sdf_version: u32,
    name: String,
    width: u32,
    height: u32,
    y_clip_min: i32,
    y_clip_max: i32,
    exposure: f64,
    postprocess: &'static str,
    output_mode: &'static str,
    render_time: u64,
    spp: u32,
    spp_target: u32,
    ray_depth: u32,
    path_trace: bool,
    dump_frequency: u32,
    save_snapshots: bool,
    emitters_enabled: bool,
    emitter_intensity: f64,
    sun_enabled: bool,
    still_water: bool,
    world: WorldRef,
    camera: ChunkyCamera,
    chunk_list: Vec<[i32; 2]>,
}

#[derive(Debug, Serialize)]
struct WorldRef {
    path: String,
    dimension: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChunkyCamera {
    name: &'static str,
    position: Xyz,
    orientation: Orientation,
    projection_mode: &'static str,
    fov: f64,
}

#[derive(Debug, Serialize)]
struct Xyz {
    x: f64,
    y: f64,
    z: f64,
}

#[derive(Debug, Serialize)]
struct Orientation {
    roll: f64,
    pitch: f64,
    yaw: f64,
}

/// Sanitize a shot id into a filesystem-safe scene name (`/` → `_`).
pub fn scene_name(shot_id: &str) -> String {
    shot_id.replace(['/', ':', ' '], "_")
}

/// Chunk column range `[[cx,cz], …]` covering an inclusive block AABB (16-block
/// chunks, floor-divided), row-major (`cx` outer, `cz` inner) for determinism.
fn chunk_list(min: [i32; 3], max: [i32; 3]) -> Vec<[i32; 2]> {
    let cxr = min[0].div_euclid(16)..=max[0].div_euclid(16);
    let czr = min[2].div_euclid(16)..=max[2].div_euclid(16);
    let mut out = Vec::new();
    for cx in cxr {
        for cz in czr.clone() {
            out.push([cx, cz]);
        }
    }
    out
}

/// Emit one Chunky scene JSON per shot. Returns `(filename, bytes)` pairs, sorted
/// by filename. Byte-deterministic (fixed field order, 2-space pretty, trailing
/// newline) so it rides the determinism gate as a validation artifact.
pub fn scenes_from_plan(
    plan_json: &[u8],
    opts: &SceneOptions,
) -> Result<Vec<(String, Vec<u8>)>, Diagnostic> {
    let plan: RenderPlan = serde_json::from_slice(plan_json)
        .map_err(|e| Diagnostic::error(DW_INPUT, format!("parse render-plan.json: {e}")))?;

    let chunks = chunk_list(plan.layout_aabb.min, plan.layout_aabb.max);
    // Y clip with a small margin around the layout so path traces are not culled.
    let y_clip_min = (plan.layout_aabb.min[1] - 8).max(-64);
    let y_clip_max = (plan.layout_aabb.max[1] + 16).min(320);

    let mut out = Vec::with_capacity(plan.shots.len());
    for shot in &plan.shots {
        let scene = ChunkyScene {
            sdf_version: 9,
            name: format!("{}_{}", plan.campaign_id, scene_name(&shot.id)),
            width: opts.width,
            height: opts.height,
            y_clip_min,
            y_clip_max,
            exposure: 1.0,
            postprocess: "GAMMA",
            output_mode: "PNG",
            render_time: 0,
            spp: 0,
            spp_target: opts.spp_target,
            ray_depth: 5,
            path_trace: true,
            dump_frequency: 500,
            save_snapshots: false,
            emitters_enabled: true,
            emitter_intensity: 13.0,
            sun_enabled: true,
            still_water: false,
            world: WorldRef {
                path: opts.world_path.clone(),
                dimension: 0,
            },
            camera: ChunkyCamera {
                name: "camera 1",
                position: Xyz {
                    x: shot.camera.pos[0],
                    y: shot.camera.pos[1],
                    z: shot.camera.pos[2],
                },
                orientation: Orientation {
                    roll: 0.0,
                    // render-plan degrees → Chunky radians (same axes; see header).
                    pitch: shot.camera.pitch.to_radians(),
                    yaw: shot.camera.yaw.to_radians(),
                },
                projection_mode: "PINHOLE",
                fov: shot.camera.fov.unwrap_or(DEFAULT_FOV_DEG),
            },
            chunk_list: chunks.clone(),
        };
        let mut bytes = serde_json::to_vec_pretty(&scene)
            .map_err(|e| Diagnostic::error(DW_INPUT, format!("serialize scene: {e}")))?;
        bytes.push(b'\n');
        out.push((format!("{}.json", scene_name(&shot.id)), bytes));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &[u8] = include_bytes!("../tests/fixtures/render-plan-mini.json");

    #[test]
    fn malformed_plan_json_is_dw0721() {
        let err = scenes_from_plan(b"not json", &SceneOptions::default()).unwrap_err();
        assert_eq!(err.code, DW_INPUT, "expected DW0721: {err:?}");
    }

    #[test]
    fn chunk_list_covers_aabb() {
        let cl = chunk_list([0, 64, 0], [17, 69, 3]);
        // x spans chunks 0..=1, z spans chunk 0 → [[0,0],[1,0]]
        assert_eq!(cl, vec![[0, 0], [1, 0]]);
    }

    #[test]
    fn emits_one_scene_per_shot() {
        let scenes = scenes_from_plan(FIXTURE, &SceneOptions::default()).unwrap();
        // The mini fixture has 2 shots.
        assert_eq!(scenes.len(), 2);
        assert!(scenes.iter().all(|(n, _)| n.ends_with(".json")));
        // Sorted, no `/` in names.
        assert!(scenes.iter().all(|(n, _)| !n.contains('/')));
    }

    #[test]
    fn yaw_pitch_convert_to_radians() {
        let scenes = scenes_from_plan(FIXTURE, &SceneOptions::default()).unwrap();
        let (_, bytes) = scenes.iter().find(|(n, _)| n == "spawn.json").unwrap();
        let v: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        // Fixture spawn shot yaw = -90 deg → -π/2 rad.
        let yaw = v["camera"]["orientation"]["yaw"].as_f64().unwrap();
        assert!((yaw - (-90f64).to_radians()).abs() < 1e-9, "yaw={yaw}");
    }

    #[test]
    fn golden_scene_matches() {
        let scenes = scenes_from_plan(FIXTURE, &SceneOptions::default()).unwrap();
        let golden = include_bytes!("../tests/fixtures/golden/spawn.json");
        let (_, spawn) = scenes.iter().find(|(n, _)| n == "spawn.json").unwrap();
        assert_eq!(
            std::str::from_utf8(spawn).unwrap(),
            std::str::from_utf8(golden).unwrap(),
            "spawn.json scene drifted from golden"
        );
    }
}
