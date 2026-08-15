//! Headless GPU render wrapper around Nucleation (wgpu → Metal/Vulkan,
//! off-screen). Textures come from a resource pack — the pinned **1.21.11 client
//! jar** — which is what determines block fidelity.
//!
//! **Fixed light + fixed camera math** so a render is reproducible per (input,
//! version): every `CameraConfig` field is pinned here (light direction,
//! intensities, background), and the caller supplies only a [`Framing`], a
//! yaw/pitch and a field of view. See the crate README for the measured
//! double-render stability finding (pixel-equal within tolerance, not
//! byte-identical — GPU float rasterization; renders are validation artifacts,
//! excluded from ADR-0006 byte-identity).
//!
//! # Standing a camera inside the model
//!
//! Nucleation's camera is an **orbit** camera: it takes a look-at point and fits
//! its own distance to the model, so there is no "put the eye here" field. An
//! eye-level interior shot needs exactly that, and the fit is invertible:
//!
//! ```text
//! eye = target − dir·distance,    distance = fit(target) · 1.1 / zoom
//! ```
//!
//! `fit` does not depend on `zoom`. So for a wanted eye `E` and view direction
//! `dir`, aiming at `target = E + dir·L` and setting `zoom = fit(target)·1.1 / L`
//! puts the eye at `E` exactly, for any `L > 0` ([`FOCUS_DISTANCE`]).
//!
//! [`fit_distance`] reimplements Nucleation's projected-corner fit, which is a
//! **replication of a pinned dependency's internals** — Nucleation is pinned by
//! git rev (`versions.toml [render]`), so it cannot drift without a deliberate
//! bump. It is not trusted on that basis alone: [`solve_eye_camera`] then
//! measures the camera it built through Nucleation's *own* projection
//! ([`nucleation::rendering::camera::project_point`]) and refuses the render if
//! the eye is not where it claims. A frame whose camera is somewhere other than
//! where the manifest says it is would be indistinguishable from a correct one
//! by eye, which is the whole failure this module exists to end.

use nucleation::meshing::{MeshConfig, ResourcePackSource};
use nucleation::rendering::camera::{
    CameraConfig, compute_view_proj, merged_bounds, project_point,
};
use nucleation::rendering::gpu::GpuRenderer;

use crate::nbt::Structure;
use crate::shots::Framing;

/// Fixed directional-light direction (pinned for reproducibility).
const LIGHT_DIRECTION: [f32; 3] = [0.35, 1.0, 0.5];
const DIRECTIONAL_INTENSITY: f32 = 1.0;
const AMBIENT_LIGHT: f32 = 0.42;
/// Fixed dark-slate background so the missing-texture magenta scan is unambiguous.
const BACKGROUND: [f32; 4] = [0.10, 0.10, 0.12, 1.0];

/// How far ahead of the eye a free camera aims, in blocks.
///
/// It changes nothing a viewer sees — the eye, the direction and the field of
/// view are what frame a perspective shot — but Nucleation derives its clip
/// planes from it (`near = 0.01·L`, `far = 10·L`), so it is chosen to keep both
/// out of the way for **any** prefab: `near = 0.16` is well inside half a block,
/// so nothing a body could stand beside is clipped away, and `far = 160` clears
/// the 83-block body diagonal of the largest structure template vanilla allows
/// (48 per axis).
pub const FOCUS_DISTANCE: f32 = 16.0;

/// How far the solved eye may sit from the requested one before the render is
/// refused, in pixels of lateral projection error.
const EYE_TOLERANCE_PX: f32 = 1.0;

/// A rendered RGBA8 frame.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// One render request.
#[derive(Debug, Clone)]
pub struct RenderParams {
    pub yaw_deg: f32,
    pub pitch_deg: f32,
    /// Field of view, degrees.
    pub fov_deg: f32,
    pub framing: Framing,
    pub dim: u32,
}

/// Load a resource pack (client jar / zip / dir).
pub fn load_pack(path: &str) -> Result<ResourcePackSource, String> {
    ResourcePackSource::from_file(path).map_err(|e| format!("load resource pack `{path}`: {e:?}"))
}

/// Nucleation's view direction for an orbit yaw/pitch (degrees): the unit vector
/// the camera looks **along**.
pub fn view_direction(yaw_deg: f32, pitch_deg: f32) -> [f32; 3] {
    let (y, p) = (yaw_deg.to_radians(), pitch_deg.to_radians());
    normalize([-(p.cos() * y.sin()), -(p.sin()), -(p.cos() * y.cos())])
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l < 1e-10 {
        return [0.0; 3];
    }
    [v[0] / l, v[1] / l, v[2] / l]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The orbit distance Nucleation would use at `zoom == 1` for a camera aimed at
/// `target` — a replication of its projected-corner fit (see the module header
/// for why this is a replication and what checks it).
pub fn fit_distance(
    bounds_min: [f32; 3],
    bounds_max: [f32; 3],
    aspect: f32,
    target: [f32; 3],
    yaw_deg: f32,
    pitch_deg: f32,
    fov_deg: f32,
) -> f32 {
    let forward = view_direction(yaw_deg, pitch_deg);
    let right = normalize(cross(forward, [0.0, 1.0, 0.0]));
    let up = cross(right, forward);
    let half_fov_y = fov_deg.to_radians() * 0.5;
    let half_fov_x = (half_fov_y.tan() * aspect).atan();

    let mut max_dist = 1.0f32;
    for cx in [bounds_min[0], bounds_max[0]] {
        for cy in [bounds_min[1], bounds_max[1]] {
            for cz in [bounds_min[2], bounds_max[2]] {
                let rel = [cx - target[0], cy - target[1], cz - target[2]];
                let proj_depth = -dot(rel, forward);
                let dist_h = dot(rel, right).abs() / half_fov_x.tan() + proj_depth;
                let dist_v = dot(rel, up).abs() / half_fov_y.tan() + proj_depth;
                max_dist = max_dist.max(dist_h).max(dist_v);
            }
        }
    }
    max_dist * 1.1
}

/// Build the orbit camera whose eye stands at `pos` looking along `yaw/pitch`,
/// and verify it against Nucleation's own projection.
///
/// The verification is the point: it re-derives, from the matrix the renderer
/// will actually use, where the eye must be, and refuses rather than emitting a
/// frame taken from somewhere the manifest does not name. A probe point one
/// block ahead and a tenth of a block to the right projects to a screen offset
/// that depends only on `r/d` — i.e. only on how far the eye really is from it.
pub fn solve_eye_camera(
    bounds_min: [f32; 3],
    bounds_max: [f32; 3],
    dim: u32,
    pos: [f32; 3],
    yaw_deg: f32,
    pitch_deg: f32,
    fov_deg: f32,
) -> Result<CameraConfig, String> {
    let aspect = 1.0; // square frames
    let dir = view_direction(yaw_deg, pitch_deg);
    let target = [
        pos[0] + dir[0] * FOCUS_DISTANCE,
        pos[1] + dir[1] * FOCUS_DISTANCE,
        pos[2] + dir[2] * FOCUS_DISTANCE,
    ];
    let fit = fit_distance(
        bounds_min, bounds_max, aspect, target, yaw_deg, pitch_deg, fov_deg,
    );
    let camera = CameraConfig {
        yaw_deg,
        pitch_deg,
        zoom: fit / FOCUS_DISTANCE,
        fov_deg,
        target: Some(target),
        projection: nucleation::rendering::camera::Projection::Perspective,
        background: Some(BACKGROUND),
        sphere_fit: false,
        light_direction: LIGHT_DIRECTION,
        directional_intensity: DIRECTIONAL_INTENSITY,
        ambient_light: AMBIENT_LIGHT,
    };

    let (view_proj, _) = compute_view_proj(bounds_min, bounds_max, aspect, &camera);
    let right = normalize(cross(dir, [0.0, 1.0, 0.0]));
    const PROBE_AHEAD: f32 = 1.0;
    const PROBE_SIDE: f32 = 0.1;
    let probe = [
        pos[0] + dir[0] * PROBE_AHEAD + right[0] * PROBE_SIDE,
        pos[1] + dir[1] * PROBE_AHEAD + right[1] * PROBE_SIDE,
        pos[2] + dir[2] * PROBE_AHEAD + right[2] * PROBE_SIDE,
    ];
    let Some((px, py)) = project_point(&view_proj, probe, dim, dim) else {
        return Err(format!(
            "eye camera at {pos:?} (yaw {yaw_deg}, pitch {pitch_deg}): the point one block in \
             front of the eye projects behind the camera — the solved camera is not standing where \
             it was asked to"
        ));
    };
    let half = dim as f32 / 2.0;
    let half_fov_x = ((fov_deg.to_radians() * 0.5).tan() * aspect).atan();
    let want_px = half + (PROBE_SIDE / PROBE_AHEAD) / half_fov_x.tan() * half;
    let want_py = half;
    let err = ((px - want_px).powi(2) + (py - want_py).powi(2)).sqrt();
    if err > EYE_TOLERANCE_PX {
        return Err(format!(
            "eye camera at {pos:?} (yaw {yaw_deg}, pitch {pitch_deg}) solved to zoom {} but the \
             renderer's own projection puts the probe point at ({px:.2},{py:.2}) instead of \
             ({want_px:.2},{want_py:.2}) — {err:.2} px out, over the {EYE_TOLERANCE_PX} px limit. \
             The camera is not where the shot manifest says it is; refusing to save the frame \
             rather than ship a picture taken from an unknown place",
            camera.zoom
        ));
    }
    Ok(camera)
}

/// Render a parsed structure to an RGBA frame. `strip_ceiling` builds the
/// dollhouse cutaway for orbit interior shots. Deterministic camera + fixed light.
pub fn render_structure(
    st: &Structure,
    pack: &ResourcePackSource,
    strip_ceiling: bool,
    p: &RenderParams,
) -> Result<Frame, String> {
    let schem = build_schematic(st, strip_ceiling).map_err(|e| e.to_string())?;
    let mesh = schem
        .to_mesh(pack, &MeshConfig::default())
        .map_err(|e| format!("mesh: {e:?}"))?;
    let meshes = vec![mesh];

    let camera = match p.framing {
        Framing::Orbit { zoom, target } => CameraConfig {
            yaw_deg: p.yaw_deg,
            pitch_deg: p.pitch_deg,
            zoom,
            fov_deg: p.fov_deg,
            target,
            projection: nucleation::rendering::camera::Projection::Perspective,
            background: Some(BACKGROUND),
            sphere_fit: target.is_none(),
            light_direction: LIGHT_DIRECTION,
            directional_intensity: DIRECTIONAL_INTENSITY,
            ambient_light: AMBIENT_LIGHT,
        },
        Framing::Eye { pos } => {
            let (bmin, bmax) = merged_bounds(&meshes);
            solve_eye_camera(bmin, bmax, p.dim, pos, p.yaw_deg, p.pitch_deg, p.fov_deg)?
        }
    };

    let renderer = pollster::block_on(GpuRenderer::new(&meshes, p.dim, p.dim, None))
        .map_err(|e| format!("gpu init: {e:?}"))?;
    let rgba = renderer
        .render_frame(&camera)
        .map_err(|e| format!("render: {e:?}"))?;
    Ok(Frame {
        width: p.dim,
        height: p.dim,
        rgba,
    })
}

/// Rebuild a parsed [`Structure`](crate::nbt::Structure) as a Nucleation
/// `UniversalSchematic`. When
/// `strip_ceiling` is set, blocks at the top Y layer are omitted, yielding a
/// "dollhouse" cutaway so an orbit camera can see the (roofed) interior — used
/// for the per-piece interior/anchor shots (a validation artifact, never
/// shipped). `air` states are skipped (they carry no mesh).
pub fn build_schematic(
    st: &crate::nbt::Structure,
    strip_ceiling: bool,
) -> Result<nucleation::UniversalSchematic, crate::nbt::NbtError> {
    use nucleation::{BlockState, UniversalSchematic};
    let mut schem = UniversalSchematic::new("delve-prefab".to_string());
    let top_y = st.size[1] - 1;
    for (pos, idx) in &st.blocks {
        if strip_ceiling && pos[1] >= top_y {
            continue;
        }
        let state_str = &st.palette[*idx];
        if state_str == "minecraft:air" {
            continue;
        }
        let bs = BlockState::from_block_string(state_str).map_err(|e| {
            crate::nbt::NbtError(format!("cannot parse block state `{state_str}`: {e:?}"))
        })?;
        schem.set_block(pos[0], pos[1], pos[2], &bs);
    }
    Ok(schem)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BMIN: [f32; 3] = [0.0, 0.0, 0.0];
    const BMAX: [f32; 3] = [16.0, 9.0, 26.0];

    /// Recover the eye from the view-projection matrix by intersecting two
    /// centre-row rays, using nothing but Nucleation's public unprojection —
    /// no assumption about how it picks its near/far planes.
    fn eye_from(camera: &CameraConfig) -> [f32; 3] {
        let (_, inv) = compute_view_proj(BMIN, BMAX, 1.0, camera);
        let unproject = |x: f32, y: f32, z: f32| -> [f32; 3] {
            let v = [x, y, z, 1.0];
            let mut o = [0.0f32; 4];
            for (row, out) in o.iter_mut().enumerate() {
                *out = (0..4).map(|col| inv[col][row] * v[col]).sum();
            }
            [o[0] / o[3], o[1] / o[3], o[2] / o[3]]
        };
        // Two rays through different screen columns meet at the pinhole.
        let a0 = unproject(-0.5, 0.0, 0.2);
        let a1 = unproject(-0.5, 0.0, 0.8);
        let b0 = unproject(0.5, 0.0, 0.2);
        let b1 = unproject(0.5, 0.0, 0.8);
        let da = normalize([a1[0] - a0[0], a1[1] - a0[1], a1[2] - a0[2]]);
        let db = normalize([b1[0] - b0[0], b1[1] - b0[1], b1[2] - b0[2]]);
        // Closest point of approach between the two lines.
        let w0 = [a0[0] - b0[0], a0[1] - b0[1], a0[2] - b0[2]];
        let (a, b, c) = (dot(da, da), dot(da, db), dot(db, db));
        let (d, e) = (dot(da, w0), dot(db, w0));
        let den = a * c - b * b;
        let s = (b * e - c * d) / den;
        let t = (a * e - b * d) / den;
        let pa = [a0[0] + da[0] * s, a0[1] + da[1] * s, a0[2] + da[2] * s];
        let pb = [b0[0] + db[0] * t, b0[1] + db[1] * t, b0[2] + db[2] * t];
        [
            (pa[0] + pb[0]) * 0.5,
            (pa[1] + pb[1]) * 0.5,
            (pa[2] + pb[2]) * 0.5,
        ]
    }

    /// The claim the eye shots rest on: the solved camera's pinhole really is at
    /// the requested point, as Nucleation's own matrix reports it.
    #[test]
    fn a_solved_eye_camera_stands_where_it_was_asked_to() {
        for pos in [
            [1.5, 2.62, 3.5],
            [10.5, 2.62, 11.5],
            [14.5, 6.62, 22.5],
            [4.5, 2.62, 25.5],
        ] {
            for yaw in [0.0f32, 90.0, 180.0, 270.0] {
                let cam = solve_eye_camera(BMIN, BMAX, 640, pos, yaw, 0.0, 70.0).expect("solve");
                let eye = eye_from(&cam);
                for i in 0..3 {
                    assert!(
                        (eye[i] - pos[i]).abs() < 0.02,
                        "yaw {yaw} at {pos:?}: eye came out {eye:?}"
                    );
                }
            }
        }
    }

    /// The view direction is the facing's, not the fit's: four yaws, four
    /// distinct forward vectors, each the Minecraft unit step of its cardinal.
    #[test]
    fn view_direction_follows_yaw() {
        assert!(dist(view_direction(0.0, 0.0), [0.0, 0.0, -1.0]) < 1e-6);
        assert!(dist(view_direction(90.0, 0.0), [-1.0, 0.0, 0.0]) < 1e-6);
        assert!(dist(view_direction(180.0, 0.0), [0.0, 0.0, 1.0]) < 1e-6);
        assert!(dist(view_direction(270.0, 0.0), [1.0, 0.0, 0.0]) < 1e-6);
    }

    fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
    }

    /// The clip planes the focus distance buys, stated as an assertion rather
    /// than as a comment: nothing a body could stand beside is clipped, and the
    /// largest template vanilla allows still fits in front of the far plane.
    #[test]
    fn the_focus_distance_keeps_both_clip_planes_out_of_the_way() {
        let near = FOCUS_DISTANCE * 0.01;
        let far = FOCUS_DISTANCE * 10.0;
        assert!(near < 0.5, "near plane {near} would clip an adjacent block");
        let max_diagonal = ((48.0f32 * 48.0) * 3.0).sqrt();
        assert!(far > max_diagonal, "far plane {far} < {max_diagonal}");
    }

    /// Solving is a pure function of its inputs — same request, same camera.
    #[test]
    fn solving_is_deterministic() {
        let a = solve_eye_camera(BMIN, BMAX, 640, [3.5, 2.62, 4.5], 90.0, 0.0, 70.0).unwrap();
        let b = solve_eye_camera(BMIN, BMAX, 640, [3.5, 2.62, 4.5], 90.0, 0.0, 70.0).unwrap();
        assert_eq!(a.zoom, b.zoom);
        assert_eq!(a.target, b.target);
        assert_eq!(a.yaw_deg, b.yaw_deg);
    }
}
