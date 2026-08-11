//! Headless GPU render wrapper around Nucleation (wgpu → Metal/Vulkan,
//! off-screen). Textures come from a resource pack — the pinned **1.21.11 client
//! jar** — which is what determines block fidelity.
//!
//! **Fixed light + fixed camera math** so a render is reproducible per (input,
//! version): every `CameraConfig` field is pinned here (light direction,
//! intensities, background), and the caller supplies only orbit yaw/pitch/zoom +
//! target. See the crate README for the measured double-render stability finding
//! (pixel-equal within tolerance, not byte-identical — GPU float rasterization;
//! renders are validation artifacts, excluded from ADR-0006 byte-identity).

use nucleation::meshing::{MeshConfig, ResourcePackSource};
use nucleation::rendering::camera::CameraConfig;
use nucleation::rendering::gpu::GpuRenderer;

use crate::cutaway::Cutaway;
use crate::nbt::Structure;

/// Fixed directional-light direction (pinned for reproducibility).
const LIGHT_DIRECTION: [f32; 3] = [0.35, 1.0, 0.5];
const DIRECTIONAL_INTENSITY: f32 = 1.0;
const AMBIENT_LIGHT: f32 = 0.42;
/// Fixed dark-slate background so the missing-texture magenta scan is unambiguous.
const BACKGROUND: [f32; 4] = [0.10, 0.10, 0.12, 1.0];
const FOV_DEG: f32 = 45.0;

/// A rendered RGBA8 frame.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// One render request (orbit camera).
#[derive(Debug, Clone)]
pub struct RenderParams {
    pub yaw_deg: f32,
    pub pitch_deg: f32,
    pub zoom: f32,
    pub target: Option<[f32; 3]>,
    pub dim: u32,
}

/// Load a resource pack (client jar / zip / dir).
pub fn load_pack(path: &str) -> Result<ResourcePackSource, String> {
    ResourcePackSource::from_file(path).map_err(|e| format!("load resource pack `{path}`: {e:?}"))
}

/// Render a parsed structure to an RGBA frame. `cut` says which solid the
/// viewer is inside — [`Cutaway::none`] for an exterior shot, any section for an
/// interior one. Deterministic camera + fixed light.
pub fn render_structure(
    st: &Structure,
    pack: &ResourcePackSource,
    cut: &Cutaway,
    p: &RenderParams,
) -> Result<Frame, String> {
    let schem = crate::nbt::build_schematic(st, cut).map_err(|e| e.to_string())?;
    let mesh = schem
        .to_mesh(pack, &MeshConfig::default())
        .map_err(|e| format!("mesh: {e:?}"))?;
    let meshes = vec![mesh];

    let camera = CameraConfig {
        yaw_deg: p.yaw_deg,
        pitch_deg: p.pitch_deg,
        zoom: p.zoom,
        fov_deg: FOV_DEG,
        target: p.target,
        projection: nucleation::rendering::camera::Projection::Perspective,
        background: Some(BACKGROUND),
        sphere_fit: p.target.is_none(),
        light_direction: LIGHT_DIRECTION,
        directional_intensity: DIRECTIONAL_INTENSITY,
        ambient_light: AMBIENT_LIGHT,
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
