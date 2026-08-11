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
use crate::diag::{DW_UNRESOLVED_BLOCK, Diagnostic};
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

/// One palette entry the supplied pack cannot draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unresolved {
    /// The block id, without its state properties.
    pub block: String,
    /// Cells of the structure carrying it.
    pub cells: usize,
}

/// Every block of `st` the pack has no blockstate for — and how many cells each
/// one covers.
///
/// # Why this is a check and not a log line
///
/// The mesher's answer to a block it cannot resolve is to **leave it out** and
/// print a warning per occurrence on stderr, unnamed by piece and uncounted. A
/// picture with the blocks silently removed is not a degraded picture; it is a
/// picture that lies to whoever is reviewing it, and it lies most where it
/// matters — a hanging curtain, a barred gate, a rail — because those are
/// exactly the blocks a texture pack is most likely to be missing. Measured: a
/// renamed vanilla block (`minecraft:chain`, which 1.21.11 calls
/// `minecraft:iron_chain`) took 127 cells out of three of eight zone sheets, and
/// the only trace was 127 interleaved warning lines nobody counted.
///
/// So resolution is decided **before** any GPU work, over the palette rather
/// than per cell, and the caller reports it as a diagnostic naming the block and
/// its cell count. Vanilla-absent and pack-absent are the same failure here on
/// purpose: both mean this pack cannot draw this piece, which is the only
/// question a renderer is entitled to answer.
///
/// `resolves` answers "can this pack draw this block id" — [`resolver`] for a
/// real pack, a closure in a test, so the rule is exercised without a GPU or a
/// client jar.
pub fn unresolved_blocks(
    st: &crate::nbt::Structure,
    resolves: impl Fn(&str) -> bool,
) -> Vec<Unresolved> {
    use std::collections::BTreeMap;
    let mut cells: BTreeMap<usize, usize> = BTreeMap::new();
    for (_, idx) in &st.blocks {
        *cells.entry(*idx).or_default() += 1;
    }
    let mut out: BTreeMap<String, usize> = BTreeMap::new();
    for (idx, state) in st.palette.iter().enumerate() {
        let name = state.split('[').next().unwrap_or(state);
        if name == "minecraft:air" {
            continue;
        }
        if !resolves(name) {
            *out.entry(name.to_string()).or_default() += cells.get(&idx).copied().unwrap_or(0);
        }
    }
    out.into_iter()
        .map(|(block, cells)| Unresolved { block, cells })
        .collect()
}

/// The pack's own answer, for the real path.
pub fn resolver(pack: &ResourcePackSource) -> impl Fn(&str) -> bool + '_ {
    move |name| pack.get_blockstate_json(name).is_some()
}

/// Refuse a piece the supplied pack cannot draw whole.
///
/// Error tier, and deliberately not a warning: the frame would show a building
/// the `.nbt` does not describe, and a render is the only evidence a reviewer
/// has. Silence here is how a renamed vanilla block emptied three of eight zone
/// sheets with nothing louder than the mesher's own per-cell chatter.
pub fn refuse_unresolved(stem: &str, missing: &[Unresolved]) -> Option<Diagnostic> {
    if missing.is_empty() {
        return None;
    }
    let total: usize = missing.iter().map(|m| m.cells).sum();
    let list: Vec<String> = missing
        .iter()
        .map(|m| format!("{} ({} cell(s))", m.block, m.cells))
        .collect();
    Some(Diagnostic::error(
        DW_UNRESOLVED_BLOCK,
        format!(
            "{stem}: the supplied pack has no blockstate for {} block(s) covering {total} \
             cell(s): {} — the mesher would leave them out and the render would show a building \
             this .nbt does not describe. Either the block was renamed or removed in the pinned \
             version (check it against the 1.21.11 registry), or the pack is not a complete \
             client jar. Do NOT render past this",
            missing.len(),
            list.join(", ")
        ),
    ))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nbt::Structure;

    fn structure(palette: &[&str], counts: &[usize]) -> Structure {
        let mut blocks = Vec::new();
        let mut n = 0i32;
        for (idx, c) in counts.iter().enumerate() {
            for _ in 0..*c {
                blocks.push(([n, 0, 0], idx));
                n += 1;
            }
        }
        Structure {
            size: [n.max(1), 1, 1],
            palette: palette.iter().map(|s| s.to_string()).collect(),
            blocks,
        }
    }

    /// The measured case, in miniature: 1.21.11 renamed the plain chain to
    /// `iron_chain`, so `minecraft:chain` resolves in no pack at all — and every
    /// cell of it vanished from the picture with no verdict anywhere.
    #[test]
    fn a_block_the_pack_cannot_draw_is_named_counted_and_refused() {
        let st = structure(
            &["minecraft:air", "minecraft:stone", "minecraft:chain"],
            &[4, 10, 36],
        );
        let missing = unresolved_blocks(&st, |name| name != "minecraft:chain");
        assert_eq!(
            missing,
            vec![Unresolved {
                block: "minecraft:chain".to_string(),
                cells: 36
            }]
        );
        let d = refuse_unresolved("bell-gate-ward", &missing).expect("refused");
        assert_eq!(d.code, "DW0728");
        assert!(d.is_error(), "a picture that omits blocks is not a warning");
        assert!(d.message.contains("36 cell(s)"), "{}", d.message);
    }

    /// ...and it stays quiet when the pack can draw everything, including a
    /// block carrying state properties (the palette entry is a state string, the
    /// pack indexes block ids).
    #[test]
    fn a_pack_that_draws_everything_refuses_nothing() {
        let st = structure(
            &[
                "minecraft:air",
                "minecraft:stone",
                "minecraft:iron_chain[axis=y,waterlogged=false]",
            ],
            &[4, 10, 36],
        );
        let missing = unresolved_blocks(&st, |name| {
            ["minecraft:stone", "minecraft:iron_chain"].contains(&name)
        });
        assert!(missing.is_empty(), "{missing:?}");
        assert!(refuse_unresolved("clean", &missing).is_none());
    }
}
