//! The whole-map release panorama (`delvec panorama`).
//!
//! Every content release ships one 45° oblique view of the entire delve. The
//! camera for it is *computed from the layout*, not
//! authored: `render-plan.json` states the layout AABB and the world-generator
//! horizon, which is everything the framing needs — so the panorama is a
//! first-class emission rather than a hand-edited scene JSON (the way the first
//! one was made).
//!
//! ## Why its own subcommand, not another shot in `scene`
//!
//! `scene` emits exactly one Chunky scene per shot in the compiler's plan, and
//! `index` pairs each of those scenes with the plan's machine `expect` lines for
//! the vision reviewer. The panorama has no `expect` pair and no plan shot: it is
//! a release artifact, not a review frame. Folding it into `scene` would break
//! the shot↔scene correspondence both commands rest on and push a release
//! decision (bearing, sample budget) into the review path. It also carries
//! render settings the review scenes must not have — an explicitly placed key
//! light and a lower default sample target — and its bearing is a creator's
//! choice at render time, which is a CLI flag, not a compiled constant.
//!
//! ## Framing
//!
//! The camera looks down at [`PITCH_DEG`] from one of four corner [`Bearing`]s,
//! aimed at the centre of the layout box. Its distance is solved exactly, not
//! guessed: all eight corners of the box are projected into the camera basis and
//! the camera is pushed back until every one of them is inside the frustum, with
//! [`MARGIN`] of breathing room. A narrow [`FOV_DEG`] keeps the perspective close
//! to the oblique "model on a table" look and away from wide-angle distortion.
//!
//! The sun sits at [`SUN_ALTITUDE_DEG`], turned [`SUN_YAW_OFFSET_DEG`] off the
//! camera's own bearing: behind the camera, so the slopes facing the viewer are
//! the lit ones, but off-axis enough that the relief still casts shadows instead
//! of flattening out.
//!
//! Only the layout's own chunks are loaded (`scene::chunk_list`) and, on an
//! ocean horizon, the surrounding sea is Chunky's ambient water plane
//! (`scene::water_world`) — see those docs for why mixing the two sources
//! leaves a seam.

use crate::view::diag::Diagnostic;
use crate::view::scene::{
    self, ChunkyCamera, ChunkyScene, ChunkySun, Orientation, WorldRef, Xyz, chunky_orientation,
};

/// Camera pitch, degrees below horizontal — the oblique "45°" of the brief.
pub const PITCH_DEG: f64 = 45.0;

/// Field of view, degrees (square frame, so this is both axes). Narrower than
/// the 70° first-person default: a map view wants low distortion, not reach.
pub const FOV_DEG: f64 = 40.0;

/// Framing margin: the solved distance leaves this much slack around the
/// layout's tightest fit (1.0 = corners exactly on the frame edge).
pub const MARGIN: f64 = 1.12;

/// Never place the camera closer than this to the layout centre, so a
/// degenerate (single-block) AABB still yields a usable scene.
const MIN_DISTANCE: f64 = 16.0;

/// Sun altitude above the horizon, degrees. High enough to light the interiors
/// of open courtyards, low enough that massing still reads through shadow.
pub const SUN_ALTITUDE_DEG: f64 = 50.0;

/// How far the sun's bearing is turned off the camera's own bearing, degrees.
/// Zero would put the sun directly behind the camera and flatten every face.
pub const SUN_YAW_OFFSET_DEG: f64 = 40.0;

/// Default sample target for a final panorama (`--spp` overrides). The tiered
/// doctrine: ~64 for a draft look, ~300 for release art.
pub const DEFAULT_SPP: u32 = 300;

/// Which corner of the layout the camera stands over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
#[clap(rename_all = "lower")]
pub enum Bearing {
    /// South-east (the default): camera at +X/+Z, looking north-west.
    Se,
    /// South-west: camera at −X/+Z, looking north-east.
    Sw,
    /// North-east: camera at +X/−Z, looking south-west.
    Ne,
    /// North-west: camera at −X/−Z, looking south-east.
    Nw,
}

impl Bearing {
    /// Lowercase name, used in the emitted file and scene names.
    pub fn name(self) -> &'static str {
        match self {
            Bearing::Se => "se",
            Bearing::Sw => "sw",
            Bearing::Ne => "ne",
            Bearing::Nw => "nw",
        }
    }

    /// Horizontal unit vector from the layout centre **toward** the camera.
    /// Minecraft axes: +X east, +Z south.
    fn toward_camera(self) -> [f64; 3] {
        const D: f64 = std::f64::consts::FRAC_1_SQRT_2;
        match self {
            Bearing::Se => [D, 0.0, D],
            Bearing::Sw => [-D, 0.0, D],
            Bearing::Ne => [D, 0.0, -D],
            Bearing::Nw => [-D, 0.0, -D],
        }
    }
}

/// A solved panorama camera, in the render-plan convention (`yaw =
/// atan2(-dz,dx)`, positive `pitch` looks down).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanoramaCamera {
    pub pos: [f64; 3],
    pub yaw_deg: f64,
    pub pitch_deg: f64,
    pub fov_deg: f64,
}

/// Round to 6 decimals: enough precision for a camera, few enough digits that
/// libm ulp differences between platforms cannot move the emitted bytes
/// (ADR-0006).
fn round6(v: f64) -> f64 {
    let r = (v * 1e6).round() / 1e6;
    // Never emit `-0.0`; it serializes differently from `0.0`.
    if r == 0.0 { 0.0 } else { r }
}

/// The geometric box of an inclusive block AABB: blocks span `[c, c+1)`, so the
/// far corner is `max + 1`.
fn box_corners(min: [i32; 3], max: [i32; 3]) -> ([f64; 3], [[f64; 3]; 8]) {
    let lo = [min[0] as f64, min[1] as f64, min[2] as f64];
    let hi = [
        max[0] as f64 + 1.0,
        max[1] as f64 + 1.0,
        max[2] as f64 + 1.0,
    ];
    let centre = [
        (lo[0] + hi[0]) / 2.0,
        (lo[1] + hi[1]) / 2.0,
        (lo[2] + hi[2]) / 2.0,
    ];
    let mut corners = [[0.0; 3]; 8];
    for (i, c) in corners.iter_mut().enumerate() {
        *c = [
            if i & 1 == 0 { lo[0] } else { hi[0] },
            if i & 2 == 0 { lo[1] } else { hi[1] },
            if i & 4 == 0 { lo[2] } else { hi[2] },
        ];
    }
    (centre, corners)
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Solve the panorama camera for an inclusive layout AABB.
///
/// The camera aims at the box centre from `bearing`, [`PITCH_DEG`] above the
/// ground, and stands back far enough that every corner of the box projects
/// inside a [`FOV_DEG`] square frame with [`MARGIN`] to spare. Exact, because
/// with the direction fixed the constraint on each corner is linear in the
/// distance: `max(|right|,|up|) ≤ (t + depth)·tan(fov/2)`.
pub fn frame(min: [i32; 3], max: [i32; 3], bearing: Bearing) -> PanoramaCamera {
    let (centre, corners) = box_corners(min, max);
    let h = bearing.toward_camera();
    let p = PITCH_DEG.to_radians();
    // View direction: horizontally toward the layout, tilted down by `p`.
    let f = [-h[0] * p.cos(), -p.sin(), -h[2] * p.cos()];
    // Camera basis. `right` is horizontal (the frame is never rolled).
    let right = {
        let v = [-f[2], 0.0, f[0]];
        let n = (v[0] * v[0] + v[2] * v[2]).sqrt();
        [v[0] / n, 0.0, v[2] / n]
    };
    let up = cross(right, f);

    let tan_half = (FOV_DEG.to_radians() / 2.0).tan() / MARGIN;
    let mut t: f64 = MIN_DISTANCE;
    for c in corners {
        let rel = [c[0] - centre[0], c[1] - centre[1], c[2] - centre[2]];
        let spread = dot(rel, right).abs().max(dot(rel, up).abs());
        let depth = dot(rel, f);
        t = t.max(spread / tan_half - depth);
    }

    PanoramaCamera {
        pos: [
            round6(centre[0] - f[0] * t),
            round6(centre[1] - f[1] * t),
            round6(centre[2] - f[2] * t),
        ],
        // Re-derived from the view vector in the render-plan convention rather
        // than restated, so the two can never drift.
        yaw_deg: round6((-f[2]).atan2(f[0]).to_degrees()),
        pitch_deg: round6(
            (-f[1])
                .atan2((f[0] * f[0] + f[2] * f[2]).sqrt())
                .to_degrees(),
        ),
        fov_deg: FOV_DEG,
    }
}

/// The key light for a panorama shot from `bearing`, in Chunky's sun convention
/// (radians; azimuth 0 = +X, growing toward +Z — the opposite turn from the
/// render-plan yaw, hence the negation).
pub fn sun(bearing: Bearing) -> ChunkySun {
    let h = bearing.toward_camera();
    // Render-plan yaw of the direction from the layout toward the camera.
    let camera_side_yaw = (-h[2]).atan2(h[0]).to_degrees();
    let sun_yaw = camera_side_yaw + SUN_YAW_OFFSET_DEG;
    let azimuth = (-sun_yaw).to_radians().rem_euclid(std::f64::consts::TAU);
    ChunkySun {
        altitude: round6(SUN_ALTITUDE_DEG.to_radians()),
        azimuth: round6(azimuth),
    }
}

/// Options for panorama emission.
#[derive(Debug, Clone)]
pub struct PanoramaOptions {
    /// Path Chunky should load the delve world from.
    pub world_path: String,
    pub width: u32,
    pub height: u32,
    /// Path-tracing sample target ([`DEFAULT_SPP`] for release art).
    pub spp_target: u32,
    pub bearing: Bearing,
}

impl Default for PanoramaOptions {
    fn default() -> Self {
        PanoramaOptions {
            world_path: "world".to_string(),
            width: 1024,
            height: 1024,
            spp_target: DEFAULT_SPP,
            bearing: Bearing::Se,
        }
    }
}

/// Emit the whole-map panorama scene for a `render-plan.json`. Returns
/// `(filename, bytes)`; byte-deterministic like [`crate::view::scene`].
pub fn panorama_from_plan(
    plan_json: &[u8],
    opts: &PanoramaOptions,
) -> Result<(String, Vec<u8>), Diagnostic> {
    let plan = scene::parse_plan(plan_json)?;
    let (min, max) = (plan.layout_aabb.min, plan.layout_aabb.max);
    let cam = frame(min, max, opts.bearing);
    let bearing = opts.bearing.name();
    // File name == scene `name`, so Chunky's own save lands back on this file and
    // its caches share the stem (see `scene::scene_file_stem`).
    let stem = scene::scene_file_stem(&plan.campaign_id, &format!("panorama_{bearing}"));

    let scene = ChunkyScene {
        sdf_version: 9,
        name: stem.clone(),
        width: opts.width,
        height: opts.height,
        y_clip_min: (min[1] - 8).max(-64),
        y_clip_max: (max[1] + 16).min(320),
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
        water_world_enabled: None,
        water_world_height: None,
        water_world_height_offset_enabled: None,
        water_world_clip_enabled: None,
        sun: Some(sun(opts.bearing)),
        // The panorama is an exterior daylight frame; the night-vision review
        // emulation belongs to declared-dark POV shots and never applies here.
        materials: None,
        delvewright_review_policy: None,
        world: WorldRef {
            path: opts.world_path.clone(),
            dimension: 0,
        },
        camera: ChunkyCamera {
            name: "camera 1",
            position: Xyz {
                x: cam.pos[0],
                y: cam.pos[1],
                z: cam.pos[2],
            },
            orientation: round_orientation(chunky_orientation(cam.yaw_deg, cam.pitch_deg)),
            projection_mode: "PINHOLE",
            fov: cam.fov_deg,
        },
        chunk_list: scene::chunk_list(min, max),
    }
    .with_water_world(scene::water_world(plan.horizon));

    Ok((format!("{stem}.json"), scene.to_bytes()?))
}

fn round_orientation(o: Orientation) -> Orientation {
    Orientation {
        roll: round6(o.roll),
        pitch: round6(o.pitch),
        yaw: round6(o.yaw),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 64×16×64 layout sitting on y=60, centred on (32, 68, 32).
    const MIN: [i32; 3] = [0, 60, 0];
    const MAX: [i32; 3] = [63, 75, 63];

    #[test]
    fn se_camera_stands_over_the_south_east_corner_looking_down_at_45() {
        let c = frame(MIN, MAX, Bearing::Se);
        assert_eq!(c.pitch_deg, 45.0);
        assert_eq!(c.yaw_deg, 135.0, "SE camera looks north-west");
        assert_eq!(c.fov_deg, FOV_DEG);
        // Centre of the box is (32, 68, 32); the camera is out along +X/+Z and up.
        assert!(c.pos[0] > 32.0 && c.pos[2] > 32.0, "{c:?}");
        assert!(c.pos[1] > 75.0, "camera above the layout: {c:?}");
        // A 45° pitch puts the rise exactly equal to the horizontal run.
        let run = ((c.pos[0] - 32.0).powi(2) + (c.pos[2] - 32.0).powi(2)).sqrt();
        assert!((run - (c.pos[1] - 68.0)).abs() < 1e-6, "not 45°: {c:?}");
        // Equal offsets on both horizontal axes — a true corner bearing.
        assert!((c.pos[0] - c.pos[2]).abs() < 1e-6, "{c:?}");
    }

    #[test]
    fn every_bearing_looks_back_at_the_layout() {
        for (b, yaw, sx, sz) in [
            (Bearing::Se, 135.0, 1.0, 1.0),
            (Bearing::Sw, 45.0, -1.0, 1.0),
            (Bearing::Ne, -135.0, 1.0, -1.0),
            (Bearing::Nw, -45.0, -1.0, -1.0),
        ] {
            let c = frame(MIN, MAX, b);
            assert_eq!(c.yaw_deg, yaw, "{b:?}");
            assert_eq!(c.pitch_deg, 45.0, "{b:?}");
            assert!((c.pos[0] - 32.0) * sx > 0.0, "{b:?} x side: {c:?}");
            assert!((c.pos[2] - 32.0) * sz > 0.0, "{b:?} z side: {c:?}");
        }
    }

    /// The framing promise: every corner of the layout box lands inside the
    /// frame, and the fit is tight (the margin, not an arbitrary distance).
    #[test]
    fn the_whole_layout_fits_the_frame_with_the_declared_margin() {
        let c = frame(MIN, MAX, Bearing::Se);
        let (centre, corners) = box_corners(MIN, MAX);
        let p = c.pitch_deg.to_radians();
        let yaw = c.yaw_deg.to_radians();
        let f = [yaw.cos() * p.cos(), -p.sin(), -yaw.sin() * p.cos()];
        let right = {
            let n = (f[2] * f[2] + f[0] * f[0]).sqrt();
            [-f[2] / n, 0.0, f[0] / n]
        };
        let up = cross(right, f);
        let tan_half = (c.fov_deg.to_radians() / 2.0).tan();

        let mut worst = 0.0f64;
        for corner in corners {
            let rel = [
                corner[0] - c.pos[0],
                corner[1] - c.pos[1],
                corner[2] - c.pos[2],
            ];
            let depth = dot(rel, f);
            assert!(depth > 0.0, "corner behind the camera: {corner:?}");
            let frac = dot(rel, right).abs().max(dot(rel, up).abs()) / (depth * tan_half);
            worst = worst.max(frac);
        }
        // Inside the frame…
        assert!(worst < 1.0, "layout overflows the frame: {worst}");
        // …and filling it: the tightest corner sits exactly at 1/MARGIN.
        assert!(
            (worst - 1.0 / MARGIN).abs() < 1e-6,
            "framing is not the declared margin: {worst}"
        );
        // Sanity: the camera really is aimed at the centre.
        let to_centre = [
            centre[0] - c.pos[0],
            centre[1] - c.pos[1],
            centre[2] - c.pos[2],
        ];
        let n = dot(to_centre, to_centre).sqrt();
        assert!((dot(to_centre, f) / n - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_degenerate_layout_still_gets_a_usable_camera() {
        let c = frame([0, 64, 0], [0, 64, 0], Bearing::Se);
        let d = ((c.pos[0] - 0.5).powi(2) + (c.pos[1] - 64.5).powi(2) + (c.pos[2] - 0.5).powi(2))
            .sqrt();
        assert!(d >= MIN_DISTANCE - 1e-6, "camera inside the block: {c:?}");
    }

    #[test]
    fn the_sun_lights_the_slopes_that_face_the_camera() {
        for b in [Bearing::Se, Bearing::Sw, Bearing::Ne, Bearing::Nw] {
            let s = sun(b);
            assert!(
                (s.altitude - SUN_ALTITUDE_DEG.to_radians()).abs() < 1e-6,
                "{b:?}"
            );
            // Chunky: direction toward the sun.
            let dir = [
                s.azimuth.cos() * s.altitude.cos(),
                s.altitude.sin(),
                s.azimuth.sin() * s.altitude.cos(),
            ];
            assert!(dir[1] > 0.0, "sun below the horizon for {b:?}");
            // It shares the camera's side of the layout (positive dot with the
            // toward-camera horizontal), so camera-facing slopes are the lit
            // ones — but off-axis, so the frame is not flat.
            let h = b.toward_camera();
            let along = (dir[0] * h[0] + dir[2] * h[2]) / (dir[0].hypot(dir[2]));
            assert!(along > 0.0, "sun is behind the layout for {b:?}: {dir:?}");
            let off_axis = along.acos().to_degrees();
            // Tolerance is the emitted azimuth's own 6-decimal-radian rounding
            // (round6), not slack in the rule.
            assert!(
                (off_axis - SUN_YAW_OFFSET_DEG).abs() < 1e-3,
                "{b:?} sun {off_axis}° off the camera bearing"
            );
        }
    }

    const MINI: &[u8] = include_bytes!("../../tests/fixtures/view/render-plan-mini.json");
    const OCEAN: &[u8] = include_bytes!("../../tests/fixtures/view/render-plan-ocean.json");

    #[test]
    fn scene_loads_only_the_layouts_own_chunks() {
        // The ocean fixture spans x 0..=31 (chunks 0..=1), z 0..=47 (0..=2).
        let (name, bytes) = panorama_from_plan(OCEAN, &PanoramaOptions::default()).unwrap();
        assert_eq!(name, "isle_panorama_se.json");
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let chunks: Vec<[i32; 2]> = serde_json::from_value(v["chunkList"].clone()).unwrap();
        assert_eq!(
            chunks,
            vec![[0, 0], [0, 1], [0, 2], [1, 0], [1, 1], [1, 2]],
            "layout chunks only — surrounding ocean chunks make a two-tone seam"
        );
    }

    #[test]
    fn the_water_plane_is_raised_only_for_an_ocean_horizon() {
        let (_, ocean) = panorama_from_plan(OCEAN, &PanoramaOptions::default()).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&ocean).unwrap();
        assert_eq!(v["waterWorldEnabled"], serde_json::json!(true));
        assert_eq!(v["waterWorldHeight"], serde_json::json!(62.875));
        assert_eq!(v["waterWorldHeightOffsetEnabled"], serde_json::json!(false));
        assert_eq!(v["waterWorldClipEnabled"], serde_json::json!(true));

        let (_, void) = panorama_from_plan(MINI, &PanoramaOptions::default()).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&void).unwrap();
        assert!(v.get("waterWorldEnabled").is_none());
        assert!(v.get("waterWorldHeight").is_none());
    }

    #[test]
    fn bearing_names_the_file_and_the_scene_identically() {
        for b in [Bearing::Se, Bearing::Sw, Bearing::Ne, Bearing::Nw] {
            let opts = PanoramaOptions {
                bearing: b,
                ..Default::default()
            };
            let (name, bytes) = panorama_from_plan(OCEAN, &opts).unwrap();
            // One stem for the file, the Chunky scene `name`, its caches and
            // the rendered PNG.
            assert_eq!(name, format!("isle_panorama_{}.json", b.name()));
            let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(v["name"], format!("isle_panorama_{}", b.name()));
        }
    }

    #[test]
    fn emission_is_byte_deterministic() {
        let a = panorama_from_plan(OCEAN, &PanoramaOptions::default()).unwrap();
        let b = panorama_from_plan(OCEAN, &PanoramaOptions::default()).unwrap();
        assert_eq!(a, b);
        assert!(a.1.ends_with(b"\n"));
    }

    #[test]
    fn malformed_plan_json_is_dw0721() {
        let err = panorama_from_plan(b"not json", &PanoramaOptions::default()).unwrap_err();
        assert_eq!(
            err.code,
            crate::view::diag::DW_INPUT,
            "expected DW0721: {err:?}"
        );
    }
}
