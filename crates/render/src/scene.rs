//! Chunky scene emission (spec-0007 whole-scene renders / spec-0003 visual tier).
//!
//! Converts the compiler's `render-plan.json` into **Chunky** scene description
//! JSONs — one per shot. Chunky itself is **not** bundled (GPL-3.0; the pinned
//! snapshot core lives in `versions.toml [render]`) — it is the project's official
//! renderer, invoked as a separate program (`docs/reference/tools.md` §4a).
//! Emitting correct scenes is the deliverable here, pinned by a golden-file test.
//!
//! ## One stem per scene
//!
//! Every emitted file is named after the scene's own Chunky `name`
//! ([`scene_file_stem`]) — because Chunky treats `name` as the scene's identity
//! and will re-save a loaded scene, and key its caches, under it. See that
//! function's docs; [`crate::cache`] is the other half.
//!
//! ## Ocean horizons
//!
//! A campaign that declares `horizon: ocean` states the fact in
//! `render-plan.json`, and every scene of it gets Chunky's ambient water plane
//! ([`water_world`]) — the shipped world save holds only the layout's chunks, so
//! the sea is not in it.
//!
//! ## Camera convention
//!
//! `render-plan.json` gives each camera as `pos` + `yaw`/`pitch` **degrees**
//! (yaw = atan2(-dz,dx): 0→+X, 90→−Z; pitch = atan2(-dy,horiz): +down).
//!
//! Chunky's scene camera orientation is **not** a straight degrees→radians copy —
//! its camera basis differs, verified directly against the pinned
//! `chunky-core-2.5.0-SNAPSHOT.474` bytecode: `Camera.updateTransform` builds
//! `rotY(π/2 + yaw) · rotX(π/2 − pitch) · rotZ(roll)`, the pinhole projector's
//! centre ray is local `+Z`, and screen-`y` points down. Composing these, the
//! world view direction for stored `(yaw, pitch)` is
//! `(cos yaw·sin pitch, −cos pitch, −sin yaw·sin pitch)`, upright iff
//! `pitch ∈ (−π, 0)`. Inverting for our degree inputs gives:
//!
//! * `yaw_chunky   = yaw_deg·π/180 + π`   (MC 0°→+X stays +X east)
//! * `pitch_chunky = pitch_deg·π/180 − π/2` (level 0° → −π/2, upright; +down stays down)
//! * `roll = 0`
//!
//! The earlier "straight deg→rad" emission pointed every POV camera at the ground
//! (level shots looked straight down; downward shots rendered upside-down); the
//! offsets above were reverse-engineered and confirmed by rendering
//! nobodys-cave-island POV shots (worker session 2026-08-01).
//!
//! ## REVIEW POLICY — night-vision emulation for declared-dark shots
//!
//! A shot whose `lighting` stamp is `{"profile": "dark", "mitigation":
//! "night-vision"}` frames an area that is *meant* to be dark and whose players
//! are kept under `minecraft:night_vision` by the compiler's clocked effect. An
//! honest path trace of that scene is **pure black**: the first Chunky run on
//! nobodys-cave-island proved exposure boosts cannot reveal a sealed cave (with
//! no light source there is nothing to amplify but noise), while real emitters
//! (the fire pit) do render. The review pipeline was therefore blind exactly
//! where the player, under night vision, sees everything.
//!
//! The emulation ([`REVIEW_POLICY`]): scenes for those shots — and **only**
//! those shots — carry a Chunky `materials` override giving every
//! non-light-emitting block of the campaign's structure palette a low uniform
//! [`REVIEW_EMITTANCE`]. Every surface then self-illuminates faintly and
//! evenly, which is the closest Chunky analogue of Minecraft night vision
//! (night vision renders every block at full, flat brightness). Real light
//! sources are deliberately **excluded** from the override
//! ([`emulation_overrides`]'s deny-list) so a placed fixture still reads as a
//! genuine glow against the emulated base light.
//!
//! **This is an approximation for review legibility, not ground truth.** The
//! scene file is marked (`delvewrightReviewPolicy` = [`REVIEW_POLICY`], plus
//! the review-only `materials` block), the shot index marks the same shots
//! (`review_policy`), and the marker string says so. It is never applied to a
//! shot stamped `lit` or carrying no stamp — those scenes stay byte-identical
//! to the pre-policy emission. The block list is derived deterministically from
//! the build's shipped structure `.nbt` palettes (sorted, deduped), so the
//! scene bytes ride the determinism gate like everything else. Verified on the
//! island cavern (worker session 2026-08-01): `pov/leg16/wp11` went from pure
//! black to fully legible at emittance 0.05 while the camp fire pit kept its
//! own glow.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::diag::{DW_INPUT, Diagnostic};

/// The Chunky snapshot-core version the spike verified against 1.21.11. Recorded
/// here + in `versions.toml [render]` + the README (Chunky 1.21.x needs snapshot
/// builds; stable stops at 1.20.4).
pub const CHUNKY_CORE: &str = "chunky-core-2.5.0-SNAPSHOT.474.g156e2bb";

/// Marker written into every emulated scene (`delvewrightReviewPolicy`) and shot
/// index entry (`review_policy`): the frame approximates the night-vision player
/// view and must never be read as the world's real lighting.
pub const REVIEW_POLICY: &str = "night-vision-emulated — review only";

/// Uniform emittance applied to non-emitting palette blocks of an emulated
/// scene. Calibrated on the nobodys-cave-island cavern (2026-08-01): 0.02 is
/// legible but dim, 0.15 verges on overbright; 0.05 (× the scene's
/// `emitterIntensity` 13) reads like the in-game night-vision view while real
/// emitters (the camp fire pit) still stand out.
pub const REVIEW_EMITTANCE: f64 = 0.05;

/// How far below a water cell's top face the rendered surface sits, in blocks.
/// Vanilla draws a full source block 1/8 short of the cell top; Chunky matches
/// it (`Water.TOP_BLOCK_GAP` in the pinned core). So the surface of the water
/// block at `y` is at `y + 1 - 0.125`, and a plane anywhere else meets the
/// authored block water in a visible two-tone seam.
pub const WATER_SURFACE_GAP: f64 = 0.125;

/// Chunky's ambient "water world" plane — the sea a delve's world save does not
/// contain (see [`water_world`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WaterWorld {
    /// Absolute Y of the rendered water surface.
    pub(crate) height: f64,
}

/// The water plane for a plan's declared horizon: `Some` **iff** the campaign
/// declares `horizon: ocean` (spec-0013), never inferred from blocks.
///
/// A delve's world save holds only the chunks its layout occupies — the sea
/// around an island is the level generator's, so a scene loading that save
/// renders void past the shoreline. Chunky's water-world plane supplies it, and
/// its surface must land on the block-water surface ([`WATER_SURFACE_GAP`]) or
/// the two waters meet in a two-tone seam. A void horizon has no ambient sea and
/// gets no keys at all, so those scenes stay byte-identical.
pub(crate) fn water_world(horizon: Option<Horizon>) -> Option<WaterWorld> {
    horizon.map(|Horizon::Ocean { sea_level }| WaterWorld {
        height: sea_level as f64 + 1.0 - WATER_SURFACE_GAP,
    })
}

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
pub(crate) struct RenderPlan {
    pub(crate) campaign_id: String,
    pub(crate) layout_aabb: Aabb,
    /// The world-generator horizon the compiler declared (spec-0013). Absent =
    /// `void`: no ambient sea, nothing to add under the frame.
    #[serde(default)]
    pub(crate) horizon: Option<Horizon>,
    shots: Vec<Shot>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Aabb {
    pub(crate) min: [i32; 3],
    pub(crate) max: [i32; 3],
}

/// The `horizon` fact `render-plan.json` carries (compiler `render_plan::
/// horizon_fact`). Only the ambients that change what a renderer must draw are
/// spelled out; `void` is the absent case.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Horizon {
    /// A superflat sea backdrop. `sea_level` is the Y of the topmost ambient
    /// water block (the compiler's `plan::SEA_LEVEL`).
    Ocean { sea_level: i32 },
}

/// Parse a `render-plan.json` (shared by scene and panorama emission).
pub(crate) fn parse_plan(plan_json: &[u8]) -> Result<RenderPlan, Diagnostic> {
    serde_json::from_slice(plan_json)
        .map_err(|e| Diagnostic::error(DW_INPUT, format!("parse render-plan.json: {e}")))
}

#[derive(Debug, Deserialize)]
struct Shot {
    id: String,
    #[allow(dead_code)]
    kind: String,
    camera: Cam,
    /// The compiler's declaration-derived lighting stamp (POV/interior shots of
    /// areas that declare `lighting` and/or `mitigation`; absent otherwise).
    #[serde(default)]
    lighting: Option<LightingStamp>,
}

/// The `lighting` stamp a shot may carry in `render-plan.json` (pure metadata
/// from the area's stage-1 declarations; see the compiler reference).
#[derive(Debug, Deserialize)]
pub struct LightingStamp {
    /// `"lit"` (relight-guaranteed) or `"dark"` (declared-dark, mitigated).
    pub profile: String,
    /// The declared darkness mitigation (`"night-vision"`), if any.
    #[serde(default)]
    pub mitigation: Option<String>,
}

/// Whether a stamp calls for the night-vision review emulation: declared dark
/// **and** declared night-vision. A `lit` profile — even with the mitigation
/// also declared — has real fixtures for the path tracer, so it is never
/// emulated; an absent stamp means an undeclared area, also never emulated.
pub fn needs_emulation(stamp: Option<&LightingStamp>) -> bool {
    stamp.is_some_and(|l| l.profile == "dark" && l.mitigation.as_deref() == Some("night-vision"))
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
pub(crate) struct ChunkyScene {
    pub(crate) sdf_version: u32,
    pub(crate) name: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) y_clip_min: i32,
    pub(crate) y_clip_max: i32,
    pub(crate) exposure: f64,
    pub(crate) postprocess: &'static str,
    pub(crate) output_mode: &'static str,
    pub(crate) render_time: u64,
    pub(crate) spp: u32,
    pub(crate) spp_target: u32,
    pub(crate) ray_depth: u32,
    pub(crate) path_trace: bool,
    pub(crate) dump_frequency: u32,
    pub(crate) save_snapshots: bool,
    pub(crate) emitters_enabled: bool,
    pub(crate) emitter_intensity: f64,
    pub(crate) sun_enabled: bool,
    pub(crate) still_water: bool,
    /// Ocean horizons only ([`water_world`]): Chunky's ambient water plane, so
    /// the sea the world save does not contain is still in frame. All four keys
    /// are written together — `waterWorldHeightOffsetEnabled` in particular
    /// defaults to `true` in the pinned core and would silently drop the plane
    /// 0.125 below the block-water surface. Absent (not `null`) on void
    /// horizons, keeping those scenes byte-identical.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) water_world_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) water_world_height: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) water_world_height_offset_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) water_world_clip_enabled: Option<bool>,
    /// An explicitly placed sun (the panorama's key light). Absent on review
    /// scenes, which keep Chunky's default sun.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) sun: Option<ChunkySun>,
    /// REVIEW POLICY (night-vision emulation) only: per-block material
    /// overrides making the scene's structural palette faintly self-emitting.
    /// Absent (not `null`) on every non-emulated scene, so those stay
    /// byte-identical to the pre-policy emission.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) materials: Option<BTreeMap<String, MaterialOverride>>,
    /// Set to [`REVIEW_POLICY`] on emulated scenes. Chunky ignores unknown
    /// scene keys (verified against the pinned core), so this is a pure marker
    /// for humans and review tooling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) delvewright_review_policy: Option<&'static str>,
    pub(crate) world: WorldRef,
    pub(crate) camera: ChunkyCamera,
    pub(crate) chunk_list: Vec<[i32; 2]>,
}

impl ChunkyScene {
    /// Attach `water` (if any) as the four `waterWorld*` keys.
    pub(crate) fn with_water_world(mut self, water: Option<WaterWorld>) -> Self {
        if let Some(w) = water {
            self.water_world_enabled = Some(true);
            self.water_world_height = Some(w.height);
            // Chunky's default subtracts 0.125 from the stored height; say so
            // explicitly rather than pre-compensating for a default.
            self.water_world_height_offset_enabled = Some(false);
            // Clip the plane to the unloaded chunks, so it never doubles up
            // with the layout's own authored water.
            self.water_world_clip_enabled = Some(true);
        }
        self
    }

    /// Serialize to the emitted scene bytes: fixed field order, 2-space pretty,
    /// trailing newline.
    pub(crate) fn to_bytes(&self) -> Result<Vec<u8>, Diagnostic> {
        let mut bytes = serde_json::to_vec_pretty(self)
            .map_err(|e| Diagnostic::error(DW_INPUT, format!("serialize scene: {e}")))?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

/// A Chunky per-material override (the subset the review policy sets).
#[derive(Debug, Serialize)]
pub(crate) struct MaterialOverride {
    emittance: f64,
}

/// Chunky's sun, in its own convention: `altitude`/`azimuth` **radians**, where
/// the direction toward the sun is
/// `(cos azimuth · cos altitude, sin altitude, sin azimuth · cos altitude)` —
/// azimuth `0` is +X (east) and grows toward +Z (south), the opposite turn from
/// the render-plan yaw convention. Verified against the pinned core's
/// `Sun.initSun`.
#[derive(Debug, Serialize)]
pub struct ChunkySun {
    pub altitude: f64,
    pub azimuth: f64,
}

#[derive(Debug, Serialize)]
pub(crate) struct WorldRef {
    pub(crate) path: String,
    pub(crate) dimension: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChunkyCamera {
    pub(crate) name: &'static str,
    pub(crate) position: Xyz,
    pub(crate) orientation: Orientation,
    pub(crate) projection_mode: &'static str,
    pub(crate) fov: f64,
}

#[derive(Debug, Serialize)]
pub(crate) struct Xyz {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) z: f64,
}

#[derive(Debug, Serialize)]
pub(crate) struct Orientation {
    pub(crate) roll: f64,
    pub(crate) pitch: f64,
    pub(crate) yaw: f64,
}

/// Render-plan (Minecraft) camera degrees → Chunky camera radians. **Not** a
/// straight deg→rad: Chunky's camera basis needs a `−π/2` pitch and `+π` yaw
/// offset (see the module header for the derivation).
pub(crate) fn chunky_orientation(yaw_deg: f64, pitch_deg: f64) -> Orientation {
    Orientation {
        roll: 0.0,
        pitch: pitch_deg.to_radians() - std::f64::consts::FRAC_PI_2,
        yaw: yaw_deg.to_radians() + std::f64::consts::PI,
    }
}

/// Sanitize a shot id into a filesystem-safe scene name (`/` → `_`).
pub fn scene_name(shot_id: &str) -> String {
    shot_id.replace(['/', ':', ' '], "_")
}

/// The scene's identity: its Chunky `name`, its `<stem>.json` file name, the
/// `<stem>.png` a render produces, and the stem its caches are keyed on — **one
/// string, four uses**.
///
/// Chunky treats the scene's `name` field as the scene's real identity: loading
/// `foo.json` whose `name` is `bar` makes it write `bar.json`, `bar.octree2` and
/// `bar.dump` into the scene directory. Emitting a file under any other name
/// therefore left a second, diverging scene description on disk and — worse —
/// put every cache on a stem re-emission would never invalidate, which is
/// exactly the stale-render trap [`crate::cache`] exists to close. Verified
/// against the pinned core by rendering a panorama (2026-08-06).
pub fn scene_file_stem(campaign_id: &str, shot_id: &str) -> String {
    format!("{}_{}", scene_name(campaign_id), scene_name(shot_id))
}

/// Chunk column range `[[cx,cz], …]` covering an inclusive block AABB (16-block
/// chunks, floor-divided), row-major (`cx` outer, `cz` inner) for determinism.
///
/// The layout's own chunks and **nothing more**: on an ocean horizon the sea is
/// Chunky's ambient plane ([`water_world`]), and loading the surrounding
/// pure-ocean chunks from the save puts block water beside plane water — a
/// visible two-tone seam right where the frame is emptiest.
pub(crate) fn chunk_list(min: [i32; 3], max: [i32; 3]) -> Vec<[i32; 2]> {
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

/// Block-state strings (or bare ids) the review emulation must **never**
/// override: non-blocks and real light emitters. Overriding an emitter would
/// replace its true emittance with the low review value and dim it; leaving it
/// out keeps its genuine glow against the emulated base light. The match is a
/// conservative substring heuristic over the block id — a false *exclusion*
/// merely leaves that block lit by its glowing neighbours, so erring wide is
/// safe; the world's light-truth is still only judged by the compiler's
/// measured model, never by this list.
fn is_emulation_target(state: &str) -> bool {
    let name = state.split('[').next().unwrap_or(state);
    const SKIP_EXACT: [&str; 6] = [
        "minecraft:air",
        "minecraft:cave_air",
        "minecraft:void_air",
        "minecraft:water",
        "minecraft:lava",
        "minecraft:structure_void",
    ];
    const SKIP_SUBSTR: [&str; 16] = [
        "torch",
        "lantern", // also sea_lantern, soul_lantern, jack_o_lantern
        "campfire",
        "fire",
        "glow", // glowstone, glow_lichen, glow_berries…
        "lamp",
        "magma",
        "froglight",
        "beacon",
        "end_rod",
        "candle",
        "amethyst",
        "sculk",
        "shroomlight",
        "conduit",
        "respawn_anchor",
    ];
    !SKIP_EXACT.contains(&name) && !SKIP_SUBSTR.iter().any(|s| name.contains(s))
}

/// The review-only `materials` override map for an emulated scene: every
/// eligible block of `palette` (the build's structure palettes, block-state
/// strings or bare ids) at [`REVIEW_EMITTANCE`]. Bare block names (state
/// brackets stripped) keyed in a `BTreeMap`, so the emitted map is sorted and
/// deterministic regardless of input order.
fn emulation_overrides(palette: &[String]) -> BTreeMap<String, MaterialOverride> {
    palette
        .iter()
        .filter(|s| is_emulation_target(s))
        .map(|s| {
            let name = s.split('[').next().unwrap_or(s).to_string();
            (
                name,
                MaterialOverride {
                    emittance: REVIEW_EMITTANCE,
                },
            )
        })
        .collect()
}

/// Emit one Chunky scene JSON per shot. Returns `(filename, bytes)` pairs, sorted
/// by filename. Byte-deterministic (fixed field order, 2-space pretty, trailing
/// newline) so it rides the determinism gate as a validation artifact.
///
/// `world_palette` is the union of the build's structure `.nbt` palettes (see
/// `delve-render scene` in `main.rs`), consumed only by the night-vision REVIEW
/// POLICY (module docs): shots stamped dark-with-night-vision get a review-only
/// `materials` override built from it; every other shot ignores it entirely. A
/// dark-stamped shot with an empty (post-filter) palette is a `DW0721` error —
/// emitting a knowingly-black "reviewable" scene would silently re-blind the
/// pipeline.
pub fn scenes_from_plan(
    plan_json: &[u8],
    opts: &SceneOptions,
    world_palette: &[String],
) -> Result<Vec<(String, Vec<u8>)>, Diagnostic> {
    let plan = parse_plan(plan_json)?;

    let chunks = chunk_list(plan.layout_aabb.min, plan.layout_aabb.max);
    let water = water_world(plan.horizon);
    // Y clip with a small margin around the layout so path traces are not culled.
    let y_clip_min = (plan.layout_aabb.min[1] - 8).max(-64);
    let y_clip_max = (plan.layout_aabb.max[1] + 16).min(320);

    let mut out = Vec::with_capacity(plan.shots.len());
    for shot in &plan.shots {
        let stem = scene_file_stem(&plan.campaign_id, &shot.id);
        let emulate = needs_emulation(shot.lighting.as_ref());
        let materials = if emulate {
            let overrides = emulation_overrides(world_palette);
            if overrides.is_empty() {
                return Err(Diagnostic::error(
                    DW_INPUT,
                    format!(
                        "shot `{}` is stamped dark-with-night-vision but no structure palette \
                         is available to build its review emulation (no structure .nbt under \
                         the build's datapack, or every block filtered) — the scene would \
                         render pure black and the review would be blind. Emit scenes from a \
                         complete `delvec build` output directory",
                        shot.id
                    ),
                ));
            }
            Some(overrides)
        } else {
            None
        };
        let scene = ChunkyScene {
            sdf_version: 9,
            name: stem.clone(),
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
            water_world_enabled: None,
            water_world_height: None,
            water_world_height_offset_enabled: None,
            water_world_clip_enabled: None,
            sun: None,
            materials,
            delvewright_review_policy: emulate.then_some(REVIEW_POLICY),
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
                orientation: chunky_orientation(shot.camera.yaw, shot.camera.pitch),
                projection_mode: "PINHOLE",
                fov: shot.camera.fov.unwrap_or(DEFAULT_FOV_DEG),
            },
            chunk_list: chunks.clone(),
        }
        .with_water_world(water);
        out.push((format!("{stem}.json"), scene.to_bytes()?));
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
        let err = scenes_from_plan(b"not json", &SceneOptions::default(), &[]).unwrap_err();
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
        let scenes = scenes_from_plan(FIXTURE, &SceneOptions::default(), &[]).unwrap();
        // The mini fixture has 2 shots.
        assert_eq!(scenes.len(), 2);
        assert!(scenes.iter().all(|(n, _)| n.ends_with(".json")));
        // Sorted, no `/` in names.
        assert!(scenes.iter().all(|(n, _)| !n.contains('/')));
    }

    #[test]
    fn camera_degrees_map_to_chunky_orientation() {
        use std::f64::consts::{FRAC_PI_2, PI};
        let scenes = scenes_from_plan(FIXTURE, &SceneOptions::default(), &[]).unwrap();
        let (_, bytes) = scenes.iter().find(|(n, _)| n == "mini_spawn.json").unwrap();
        let v: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        let o = &v["camera"]["orientation"];
        // Spawn shot: MC yaw −90°, pitch 15.945°. Chunky = yaw_deg+π, pitch_deg−π/2.
        let yaw = o["yaw"].as_f64().unwrap();
        let pitch = o["pitch"].as_f64().unwrap();
        assert!(
            (yaw - ((-90f64).to_radians() + PI)).abs() < 1e-9,
            "yaw={yaw}"
        );
        assert!(
            (pitch - (15.945f64.to_radians() - FRAC_PI_2)).abs() < 1e-9,
            "pitch={pitch}"
        );
        assert_eq!(o["roll"].as_f64().unwrap(), 0.0);
    }

    #[test]
    fn level_forward_pov_is_upright_and_horizontal() {
        // Regression for the camera-orientation bug (worker session 2026-08-01):
        // a first-person POV walking east — MC (yaw 0°, pitch 0°), the exact
        // nobodys-cave-island `pov/leg0/wp1` camera — rendered straight DOWN at
        // the sand because emission used a naive deg→rad. The verified mapping
        // (yaw+π, pitch−π/2) puts it level (pitch −π/2, upright) and facing +X.
        let plan = br#"{"campaign_id":"c","layout_aabb":{"min":[0,64,0],"max":[1,65,1]},
          "shots":[{"id":"pov/leg0/wp1","kind":"pov","camera":{"pos":[7.5,68.62,10.5],
          "yaw":0.0,"pitch":0.0,"look_at":[8.5,68.62,10.5]}}]}"#;
        let scenes = scenes_from_plan(plan, &SceneOptions::default(), &[]).unwrap();
        let (_, bytes) = &scenes[0];
        let v: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        let o = &v["camera"]["orientation"];
        assert_eq!(o["roll"].as_f64().unwrap(), 0.0);
        assert!((o["yaw"].as_f64().unwrap() - std::f64::consts::PI).abs() < 1e-12);
        assert!((o["pitch"].as_f64().unwrap() + std::f64::consts::FRAC_PI_2).abs() < 1e-12);
    }

    /// A two-shot plan: one dark-with-night-vision POV (emulated) and one lit
    /// interior (never emulated), sharing one layout.
    const DARK_PLAN: &[u8] =
        br#"{"campaign_id":"cave","layout_aabb":{"min":[0,64,0],"max":[15,80,15]},
      "shots":[
        {"id":"pov/leg0/wp0","kind":"pov",
         "lighting":{"profile":"dark","mitigation":"night-vision"},
         "camera":{"pos":[7.5,66.62,7.5],"yaw":0.0,"pitch":0.0,"look_at":[8.5,66.62,7.5]}},
        {"id":"interior/cave/0","kind":"interior",
         "lighting":{"profile":"lit"},
         "camera":{"pos":[0.5,70.0,0.5],"yaw":45.0,"pitch":30.0,"look_at":[7.5,65.0,7.5]}}
      ]}"#;

    fn palette() -> Vec<String> {
        [
            "minecraft:stone",
            "minecraft:air",
            "minecraft:cobblestone_stairs[facing=east,half=bottom]",
            "minecraft:wall_torch[facing=north]",
            "minecraft:campfire[lit=true]",
            "minecraft:water",
            "minecraft:glowstone",
            "minecraft:tuff",
        ]
        .into_iter()
        .map(str::to_string)
        .collect()
    }

    #[test]
    fn dark_night_vision_shot_gets_the_review_emulation() {
        let scenes = scenes_from_plan(DARK_PLAN, &SceneOptions::default(), &palette()).unwrap();
        let (_, bytes) = scenes
            .iter()
            .find(|(n, _)| n == "cave_pov_leg0_wp0.json")
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        // Marked as review-only emulation.
        assert_eq!(v["delvewrightReviewPolicy"], REVIEW_POLICY);
        // Structural blocks are overridden at the review emittance, with state
        // brackets stripped; air/water and real emitters are not.
        let m = v["materials"].as_object().unwrap();
        assert_eq!(m["minecraft:stone"]["emittance"], REVIEW_EMITTANCE);
        assert_eq!(
            m["minecraft:cobblestone_stairs"]["emittance"],
            REVIEW_EMITTANCE
        );
        assert_eq!(m["minecraft:tuff"]["emittance"], REVIEW_EMITTANCE);
        for excluded in [
            "minecraft:air",
            "minecraft:water",
            "minecraft:wall_torch",
            "minecraft:campfire",
            "minecraft:glowstone",
        ] {
            assert!(
                !m.contains_key(excluded),
                "{excluded} must not be overridden"
            );
        }
    }

    #[test]
    fn lit_and_unstamped_shots_are_never_emulated() {
        let scenes = scenes_from_plan(DARK_PLAN, &SceneOptions::default(), &palette()).unwrap();
        let (_, lit) = scenes
            .iter()
            .find(|(n, _)| n == "cave_interior_cave_0.json")
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(lit).unwrap();
        assert!(
            v.get("materials").is_none(),
            "lit shot must carry no override"
        );
        assert!(v.get("delvewrightReviewPolicy").is_none());
        // Unstamped shots (the mini fixture) are byte-identical whether or not a
        // palette is supplied — the policy never leaks into them.
        let without = scenes_from_plan(FIXTURE, &SceneOptions::default(), &[]).unwrap();
        let with = scenes_from_plan(FIXTURE, &SceneOptions::default(), &palette()).unwrap();
        assert_eq!(without, with);
    }

    #[test]
    fn dark_shot_without_a_palette_is_dw0721() {
        // Emitting a knowingly-black "reviewable" scene would re-blind the
        // pipeline — refuse instead.
        let err = scenes_from_plan(DARK_PLAN, &SceneOptions::default(), &[]).unwrap_err();
        assert_eq!(err.code, DW_INPUT, "expected DW0721: {err:?}");
        // A palette that filters to nothing (only emitters/air) is the same error.
        let all_filtered = vec!["minecraft:air".to_string(), "minecraft:torch".to_string()];
        let err2 =
            scenes_from_plan(DARK_PLAN, &SceneOptions::default(), &all_filtered).unwrap_err();
        assert_eq!(err2.code, DW_INPUT);
    }

    #[test]
    fn emulation_is_deterministic_and_sorted() {
        let mut reversed = palette();
        reversed.reverse();
        let a = scenes_from_plan(DARK_PLAN, &SceneOptions::default(), &palette()).unwrap();
        let b = scenes_from_plan(DARK_PLAN, &SceneOptions::default(), &reversed).unwrap();
        assert_eq!(a, b, "palette order must not affect scene bytes");
        let (_, bytes) = a
            .iter()
            .find(|(n, _)| n == "cave_pov_leg0_wp0.json")
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        let keys: Vec<&String> = v["materials"].as_object().unwrap().keys().collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted, "materials keys are sorted");
    }

    /// The invariant Chunky forces: a scene's file stem IS its `name`. Chunky
    /// re-saves a loaded scene under its `name` field and keys `<name>.octree2`
    /// / `<name>.dump` on it, so any other file name leaves a second, diverging
    /// scene description on disk and puts the caches on a stem re-emission
    /// cannot invalidate (verified against the pinned core, 2026-08-06).
    #[test]
    fn every_scenes_file_stem_is_its_chunky_name() {
        for plan in [FIXTURE, DARK_PLAN, OCEAN_FIXTURE] {
            let scenes = scenes_from_plan(plan, &SceneOptions::default(), &palette()).unwrap();
            assert!(!scenes.is_empty());
            for (file, bytes) in &scenes {
                let v: serde_json::Value = serde_json::from_slice(bytes).unwrap();
                let stem = file.strip_suffix(".json").unwrap();
                assert_eq!(v["name"], stem, "{file} disagrees with its scene name");
            }
        }
    }

    const OCEAN_FIXTURE: &[u8] = include_bytes!("../tests/fixtures/render-plan-ocean.json");

    #[test]
    fn ocean_horizon_scenes_stand_on_the_water_world_plane() {
        // The world save only holds chunks near the layout; without Chunky's
        // ambient water plane every ocean-horizon frame shows void past the
        // shoreline. The plane's surface must sit flush with block water.
        let scenes = scenes_from_plan(OCEAN_FIXTURE, &SceneOptions::default(), &[]).unwrap();
        let (_, bytes) = &scenes[0];
        let v: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        assert_eq!(v["waterWorldEnabled"], serde_json::json!(true));
        assert_eq!(v["waterWorldHeight"], serde_json::json!(62.875));
        // Chunky's default subtracts 0.125 from the stored height; the emission
        // must not depend on that default.
        assert_eq!(
            v["waterWorldHeightOffsetEnabled"],
            serde_json::json!(false),
            "the plane height must be absolute, not offset by a Chunky default"
        );
        assert_eq!(v["waterWorldClipEnabled"], serde_json::json!(true));
    }

    #[test]
    fn void_horizon_scenes_have_no_water_world_plane() {
        let scenes = scenes_from_plan(FIXTURE, &SceneOptions::default(), &[]).unwrap();
        for (name, bytes) in &scenes {
            let v: serde_json::Value = serde_json::from_slice(bytes).unwrap();
            assert!(
                v.get("waterWorldEnabled").is_none(),
                "{name} must carry no water-world keys"
            );
        }
    }

    #[test]
    fn golden_scene_matches() {
        let scenes = scenes_from_plan(FIXTURE, &SceneOptions::default(), &[]).unwrap();
        let golden = include_bytes!("../tests/fixtures/golden/spawn.json");
        let (_, spawn) = scenes.iter().find(|(n, _)| n == "mini_spawn.json").unwrap();
        assert_eq!(
            std::str::from_utf8(spawn).unwrap(),
            std::str::from_utf8(golden).unwrap(),
            "spawn.json scene drifted from golden"
        );
    }
}
