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
//! function's docs; [`crate::compiler::view::cache`] is the other half.
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
//! ## The sun is the campaign's declared hour ([`sun_at`])
//!
//! Every scene carries a sun derived from `render-plan.json`'s `sky` fact, and a
//! plan that states no hour is refused ([`DW_INPUT`]) rather than emitted under
//! Chunky's default. That default is a 60° midday sun, and it is what every
//! review frame of every campaign used to come off: `world.json` declared the
//! hour, `DW0890` held the approved design's rows equal to it at every
//! `validate`, and nothing ever told the renderer. Measured on the first full
//! drill — 55 of 59 emitted scenes carried no `sun` and no `sky` key at all, and
//! a delve declaring `dusk` rendered noon blue.
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

use delvewright_dsl::WorldWeather;

use crate::compiler::view::diag::{DW_INPUT, Diagnostic};

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
    match horizon {
        Some(Horizon::Ocean { sea_level }) => Some(WaterWorld {
            height: sea_level as f64 + 1.0 - WATER_SURFACE_GAP,
        }),
        // A valley builds its own ground and stands in no water. Written as an
        // arm rather than a catch-all so a base added later has to say what it
        // does here instead of inheriting a silence.
        Some(Horizon::Valley { .. }) | None => None,
    }
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
    /// The hour the campaign declared. Optional **in the document type only**, so
    /// a plan that omits it can be refused by name ([`plan_sky`]) instead of by
    /// a serde message about a missing field — the fact a creator needs is which
    /// engine wrote the plan, not which key is absent.
    #[serde(default)]
    pub(crate) sky: Option<Sky>,
    shots: Vec<Shot>,
}

/// The `sky` fact `render-plan.json` carries: the hour and the weather this
/// delve is played at (`crate::compiler::render_plan`'s `sky_fact`).
#[derive(Debug, Clone, Deserialize)]
pub struct Sky {
    /// The keyword the author wrote (`dusk`) — for messages, never for the sun.
    #[allow(dead_code)]
    pub time: String,
    /// The vanilla `daytime` tick value that keyword sets. **This** is what the
    /// sun is a function of, so a state vanilla does not name is worth as much
    /// as one it does.
    pub daytime_ticks: i64,
    /// The declared initial weather. Optional **in the document type only**, for
    /// the reason [`RenderPlan::sky`] is: a plan an older engine wrote is refused
    /// by name ([`plan_sky`]).
    #[serde(default)]
    pub weather: Option<WorldWeather>,
}

/// An inclusive world box. Public because [`Horizon`] carries one: a horizon
/// that BUILT ground has to say how far the ground reaches, and a scene loads
/// the union of that and the layout.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Aabb {
    pub min: [i32; 3],
    pub max: [i32; 3],
}

/// The `horizon` fact `render-plan.json` carries (compiler `render_plan::
/// horizon_fact`). Only the ambients that change what a renderer must draw are
/// spelled out; `void` is the absent case.
/// **Every base the compiler can state, and the enumeration is the point.**
/// `#[serde(tag = "kind")]` has no fallback, so one unknown value fails the
/// whole document — `delvec panorama` and `delvec scene` refused every valley
/// campaign outright with `DW0721 … unknown variant "valley", expected "ocean"`,
/// while the compiler had been writing `{"kind": "valley", …}` for as long as
/// the base existed. A producer and a consumer of one document, and nothing
/// compared them; `check-gallery-render.py` ran only `snapshot`, and only on
/// the primary gallery, which declares no valley.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Horizon {
    /// A superflat sea backdrop. `sea_level` is the Y of the topmost ambient
    /// water block (the compiler's `plan::SEA_LEVEL`).
    Ocean { sea_level: i32 },
    /// A landform the compiler BUILT and the world save therefore contains.
    /// Nothing ambient to add — but the ground is real geometry outside the
    /// layout, so [`Horizon::extent`] is what keeps it in the chunk list. It is
    /// loaded, never framed: the panorama's subject is the layout alone.
    Valley {
        #[allow(dead_code)]
        gap_floor_y: i32,
        #[allow(dead_code)]
        rim_height: i32,
        extent: Aabb,
    },
}

impl Horizon {
    /// The world AABB of geometry this horizon put in the save, if any. An
    /// ocean's sea is Chunky's ambient plane rather than blocks, so it has
    /// none; a valley's landform is blocks, and a frame that leaves it out
    /// shows a delve standing on nothing.
    pub(crate) fn extent(self) -> Option<([i32; 3], [i32; 3])> {
        match self {
            Horizon::Ocean { .. } => None,
            Horizon::Valley { extent, .. } => Some((extent.min, extent.max)),
        }
    }
}

/// The union of a layout AABB with whatever ground the horizon built under it —
/// what a scene loads (its chunk list and Y clip). One function because every
/// scene owes the same answer; the camera is solved from the layout alone
/// (`panorama`), so the ground is in the picture as the place's setting without
/// being what the frame is fitted to.
pub(crate) fn loaded_extent(layout: &Aabb, horizon: Option<Horizon>) -> ([i32; 3], [i32; 3]) {
    let (mut min, mut max) = (layout.min, layout.max);
    if let Some((hmin, hmax)) = horizon.and_then(Horizon::extent) {
        for a in 0..3 {
            min[a] = min[a].min(hmin[a]);
            max[a] = max[a].max(hmax[a]);
        }
    }
    (min, max)
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
    /// The campaign's declared hour, as a sun ([`sun_at`]). Every scene this
    /// crate emits carries one; the `Option` is what lets the type be built
    /// before it is known, never a scene that ships without it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) sun: Option<ChunkySun>,
    /// The overcast sky of a rain or thunder scene ([`sky_of`]). Absent (not
    /// `null`) on a clear scene, whose sky is the renderer's own simulated one,
    /// so every clear scene keeps its bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) sky: Option<ChunkySkyMode>,
    /// The overcast fog of a rain or thunder scene ([`sky_of`]); absent on a
    /// clear scene, for the reason `sky` is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fog: Option<ChunkyFog>,
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
    /// Put the scene under `block` — its sun, and on a non-clear scene its sky and
    /// fog. Every scene builder in this module tree takes its sky through here
    /// from [`sky_of`], so a review frame, a panorama and a showcase camera are
    /// one sky shape.
    pub(crate) fn under(mut self, block: SkyBlock) -> Self {
        self.sun = Some(block.sun);
        self.sky = block.sky;
        self.fog = block.fog;
        self
    }

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
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkySun {
    pub altitude: f64,
    pub azimuth: f64,
    /// Overcast scenes only ([`sky_of`]): the sun's emittance scale. Absent on a
    /// clear scene, which keeps Chunky's own 1.25.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intensity: Option<f64>,
    /// Overcast scenes only: the sun's colour.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Rgb>,
    /// Overcast scenes only: `false` — there is no sun disc under cloud.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draw_texture: Option<bool>,
}

/// Round to 6 decimals, never emitting `-0.0` (it serializes differently from
/// `0.0`). Enough precision for a sun, few enough digits that libm ulp
/// differences between platforms cannot move the emitted bytes (ADR-0006).
pub(crate) fn round6(v: f64) -> f64 {
    let r = (v * 1e6).round() / 1e6;
    if r == 0.0 { 0.0 } else { r }
}

/// **The sun of a Minecraft `daytime` tick value**, in Chunky's convention.
///
/// Two published facts meet here and neither is invented.
///
/// *Minecraft.* The sun and moon "appear to rotate around the player, appearing
/// directly overhead at midday and midnight, respectively", and rise in the east
/// — so the track is a great circle through the zenith in the east–west plane,
/// and one angle fixes the whole position. minecraft.wiki (*Daylight cycle*,
/// §Sky angle) publishes that angle for a `daytime` tick `t`, with 0° at noon:
///
/// ```text
/// α = (1 − cos(π · mod₁((t − 6000)/24000)) + mod₄((t − 6000)/6000)) · 60°
/// ```
///
/// Cross-checked against the game's own `DimensionType.timeOfDay`
/// (`α = 360° · (2·d + (1 − cos πd)/2)/3`, `d = frac(t/24000 − 0.25)`), which is
/// a different expression of the same curve: the two agree to six decimals at
/// every one of the six hours [`crate::compiler::view::scene`] can be handed
/// (`the_two_published_sun_angle_formulas_agree`). The curve is deliberately not
/// linear in `t` — that is the term that makes vanilla's sunrise and sunset
/// linger near the horizon — so a linear interpolation would be a third, wrong
/// answer.
///
/// *Chunky.* The direction toward the sun is
/// `(cos az · cos alt, sin alt, sin az · cos alt)`, verified against the pinned
/// core's `Sun.initSun` bytecode; nothing clamps `altitude`, so a sun below the
/// horizon is expressed as a negative one and the scene renders as the night it
/// is.
///
/// Composing them: rotating the zenith by `α` toward the west gives a sun
/// direction of `(−sin α, cos α, 0)`, hence `altitude = asin(cos α)` and an
/// azimuth of exactly east or exactly west. At noon and midnight the sun is at
/// the zenith or the nadir and the azimuth means nothing; east is emitted, so
/// the bytes are still a function of the hour alone.
pub fn sun_at(daytime_ticks: i64) -> ChunkySun {
    let alpha = sky_angle_rad(daytime_ticks);
    let altitude = alpha.cos().clamp(-1.0, 1.0).asin();
    // sin α > 0 is the half of the day after noon: the sun has gone west.
    let azimuth = if alpha.sin() > 0.0 {
        std::f64::consts::PI
    } else {
        0.0
    };
    ChunkySun {
        altitude: round6(altitude),
        azimuth: round6(azimuth),
        intensity: None,
        color: None,
        draw_texture: None,
    }
}

/// minecraft.wiki's sky angle for a `daytime` tick value, in radians, 0 at noon
/// and growing westward. See [`sun_at`] for the citation and the cross-check.
fn sky_angle_rad(daytime_ticks: i64) -> f64 {
    let t = daytime_ticks as f64 - 6000.0;
    let turn = (t / 24000.0).rem_euclid(1.0);
    let quarters = (t / 6000.0).rem_euclid(4.0);
    (1.0 - (std::f64::consts::PI * turn).cos() + quarters) * 60f64.to_radians()
}

/// The sky a scene is taken under: the hour as the `daytime` tick value the sun
/// is a function of, and the weather. Built from `render-plan.json` by
/// [`plan_sky`] (the panorama and the review frames) or from a showcase camera
/// (`crate::compiler::view::camera`); turned into scene keys by [`sky_of`] alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneSky {
    pub daytime_ticks: i64,
    pub weather: WorldWeather,
}

/// The hour and weather a plan states, or [`DW_INPUT`] naming what is missing.
///
/// **A scene is never emitted without both.** Chunky's default sun is a midday
/// one and its default sky is clear, so a plan with no `sky` would render a night
/// delve at noon, and one with no `sky.weather` would render a rain delve under a
/// clear sky, and say nothing — the exact silence this key exists to end. The
/// refusal is here, at the one door every plan-sky scene goes through, rather
/// than in the type, so it can say what to do about it.
pub(crate) fn plan_sky(plan: &RenderPlan) -> Result<SceneSky, Diagnostic> {
    let sky = plan.sky.as_ref().ok_or_else(|| {
        Diagnostic::error(
            DW_INPUT,
            format!(
                "render-plan.json states no `sky`, so there is no hour to put a sun at. Chunky's \
                 own default is a midday sun, and emitting these scenes would hand back frames of \
                 a noon sky whatever hour `world.json` declares. The plan was written by an \
                 engine older than this one (delvec {}): rebuild the delve with this engine — \
                 `delvec build` writes the key from the campaign's declared `time` and \
                 `weather` — rather than rendering a plan an older one wrote",
                env!("CARGO_PKG_VERSION")
            ),
        )
    })?;
    let weather = sky.weather.ok_or_else(|| {
        Diagnostic::error(
            DW_INPUT,
            format!(
                "render-plan.json states the hour (`{}`) but no `sky.weather`, so a rain or \
                 thunder delve would render under Chunky's default clear sky. The plan was \
                 written by an engine older than this one (delvec {}), which did not state the \
                 weather: rebuild the delve with this engine — `delvec build` writes the key \
                 from the campaign's declared `weather` — rather than rendering a plan an older \
                 one wrote",
                sky.time,
                env!("CARGO_PKG_VERSION")
            ),
        )
    })?;
    Ok(SceneSky {
        daytime_ticks: sky.daytime_ticks,
        weather,
    })
}

// ---- the sky a scene is taken under ------------------------------------------

/// A colour as Chunky's scene format writes one (`util/JsonUtil.rgbToJson` at the
/// pinned core): three channels in `0..=1`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Rgb {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
}

const fn rgb(red: f64, green: f64, blue: f64) -> Rgb {
    Rgb { red, green, blue }
}

/// Chunky's `sky` object, as an overcast scene writes it (`renderer/scene/sky/
/// Sky.java` at the pinned core): a solid colour, the light the sky casts
/// (`skyLight`) and the brightness the lens sees (`apparentSkyLight`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkySkyMode {
    pub mode: &'static str,
    pub color: Rgb,
    pub sky_light: f64,
    pub apparent_sky_light: f64,
}

/// Chunky's `fog` object, as an overcast scene writes it (`renderer/scene/Fog.java`
/// at the pinned core). `skyFogDensity` is always 0, so the fog colour never
/// paints over the sky colour.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkyFog {
    pub mode: &'static str,
    pub uniform_density: f64,
    pub color: Rgb,
    pub sky_fog_density: f64,
}

/// How much daylight an overcast sky passes, decided by **the emitted sun's
/// altitude**, never by the hour's name: an hour added to `WorldTime` falls into a
/// class by its sun, with no row to remember.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DaylightClass {
    /// The sun at or above [`HIGH_SUN_DEG`] (`noon`, `day`).
    High,
    /// The sun from the horizon up to [`HIGH_SUN_DEG`] (`dusk`).
    Low,
    /// The sun under the horizon (`night`, `midnight`, `dawn`).
    Below,
}

impl DaylightClass {
    /// Every class, in order.
    pub const ALL: [DaylightClass; 3] = [
        DaylightClass::High,
        DaylightClass::Low,
        DaylightClass::Below,
    ];

    /// The class's own spelling, as the binding line prints it.
    pub fn name(self) -> &'static str {
        match self {
            DaylightClass::High => "high",
            DaylightClass::Low => "low",
            DaylightClass::Below => "below",
        }
    }
}

/// The altitude, in degrees, at and above which an overcast sky takes the
/// `high` cell. Authored (spec-0079 §4.3).
pub const HIGH_SUN_DEG: f64 = 20.0;

/// The daylight class of a sun ([`DaylightClass`]), read off the emitted — already
/// rounded — altitude.
pub fn daylight_class(sun: &ChunkySun) -> DaylightClass {
    if sun.altitude >= HIGH_SUN_DEG.to_radians() {
        DaylightClass::High
    } else if sun.altitude >= 0.0 {
        DaylightClass::Low
    } else {
        DaylightClass::Below
    }
}

/// One cell of the overcast look table: every value an overcast scene writes
/// beyond the sun's direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OvercastCell {
    pub sky_color: Rgb,
    pub sky_light: f64,
    pub apparent_sky_light: f64,
    pub sun_intensity: f64,
    pub sun_color: Rgb,
    pub fog_density: f64,
    pub fog_color: Rgb,
}

/// The colours every cell shares: a neutral grey-slate sky, a warm-grey sun, a
/// slate fog (`docs/reference/showcase-shots.md` §2b's measured block).
const OVERCAST_SKY: Rgb = rgb(0.10, 0.11, 0.14);
const OVERCAST_SUN: Rgb = rgb(0.9, 0.75, 0.65);
const OVERCAST_FOG: Rgb = rgb(0.20, 0.22, 0.26);

/// `low` × `rain`: the measured overcast dusk, adopted verbatim — the one cell
/// anyone had looked at before the table existed.
pub const LOW_RAIN: OvercastCell = OvercastCell {
    sky_color: OVERCAST_SKY,
    sky_light: 1.6,
    apparent_sky_light: 0.6,
    sun_intensity: 0.25,
    sun_color: OVERCAST_SUN,
    fog_density: 0.002,
    fog_color: OVERCAST_FOG,
};

/// `high` × `rain`: [`LOW_RAIN`] with both sky-light modifiers doubled.
pub const HIGH_RAIN: OvercastCell = OvercastCell {
    sky_light: 3.2,
    apparent_sky_light: 1.2,
    ..LOW_RAIN
};

/// `below` × `rain`: [`LOW_RAIN`] with both sky-light modifiers at a tenth and the
/// sun, which is under the horizon, at 0.
pub const BELOW_RAIN: OvercastCell = OvercastCell {
    sky_light: 0.16,
    apparent_sky_light: 0.06,
    sun_intensity: 0.0,
    ..LOW_RAIN
};

/// `high` × `thunder`: [`HIGH_RAIN`] at 0.6 of its sky-light modifiers, half its
/// sun, 1.5 times its fog.
pub const HIGH_THUNDER: OvercastCell = OvercastCell {
    sky_light: 1.92,
    apparent_sky_light: 0.72,
    sun_intensity: 0.125,
    fog_density: 0.003,
    ..HIGH_RAIN
};

/// `low` × `thunder`: [`LOW_RAIN`] by the same rule.
pub const LOW_THUNDER: OvercastCell = OvercastCell {
    sky_light: 0.96,
    apparent_sky_light: 0.36,
    sun_intensity: 0.125,
    fog_density: 0.003,
    ..LOW_RAIN
};

/// `below` × `thunder`: [`BELOW_RAIN`] by the same rule; its sun is already 0.
pub const BELOW_THUNDER: OvercastCell = OvercastCell {
    sky_light: 0.096,
    apparent_sky_light: 0.036,
    sun_intensity: 0.0,
    fog_density: 0.003,
    ..BELOW_RAIN
};

/// The look-table cell of a class and a weather; `None` for `clear`, whose sky is
/// the renderer's own.
pub fn overcast_cell(class: DaylightClass, weather: WorldWeather) -> Option<&'static OvercastCell> {
    match (weather, class) {
        (WorldWeather::Clear, _) => None,
        (WorldWeather::Rain, DaylightClass::High) => Some(&HIGH_RAIN),
        (WorldWeather::Rain, DaylightClass::Low) => Some(&LOW_RAIN),
        (WorldWeather::Rain, DaylightClass::Below) => Some(&BELOW_RAIN),
        (WorldWeather::Thunder, DaylightClass::High) => Some(&HIGH_THUNDER),
        (WorldWeather::Thunder, DaylightClass::Low) => Some(&LOW_THUNDER),
        (WorldWeather::Thunder, DaylightClass::Below) => Some(&BELOW_THUNDER),
    }
}

/// Everything a scene states about its sky: the sun, and on a non-clear scene the
/// overcast sky and fog. Built by [`sky_of`] alone.
#[derive(Debug, Clone, PartialEq)]
pub struct SkyBlock {
    pub sun: ChunkySun,
    pub sky: Option<ChunkySkyMode>,
    pub fog: Option<ChunkyFog>,
    /// The class the sun put the scene in — the look-table row read.
    pub class: DaylightClass,
}

/// **The whole sky of a scene** (spec-0079 §4): the one writer of every sky, sun
/// and fog key under `crate::compiler::view`.
///
/// - `clear` is the hour's sun direction and nothing more — Chunky's simulated
///   sky with its default sun *is* a clear sky, so a clear scene's bytes are the
///   ones the engine wrote before the weather reached a scene.
/// - `rain` and `thunder` are an overcast block written in full: the sun where
///   the hour puts it but dimmed and without its disc, a solid grey sky, uniform
///   fog that never paints the sky — the cell of [`overcast_cell`] for the
///   sun's [`DaylightClass`].
///
/// Constants, a class decision on the rounded altitude and [`round6`] on every
/// float: deterministic (ADR-0006).
pub fn sky_of(sky: SceneSky) -> SkyBlock {
    let mut sun = sun_at(sky.daytime_ticks);
    let class = daylight_class(&sun);
    let Some(cell) = overcast_cell(class, sky.weather) else {
        return SkyBlock {
            sun,
            sky: None,
            fog: None,
            class,
        };
    };
    let r = |c: Rgb| rgb(round6(c.red), round6(c.green), round6(c.blue));
    sun.intensity = Some(round6(cell.sun_intensity));
    sun.color = Some(r(cell.sun_color));
    sun.draw_texture = Some(false);
    SkyBlock {
        sun,
        sky: Some(ChunkySkyMode {
            mode: "SOLID_COLOR",
            color: r(cell.sky_color),
            sky_light: round6(cell.sky_light),
            apparent_sky_light: round6(cell.apparent_sky_light),
        }),
        fog: Some(ChunkyFog {
            mode: "UNIFORM",
            uniform_density: round6(cell.fog_density),
            color: r(cell.fog_color),
            sky_fog_density: 0.0,
        }),
        class,
    }
}

/// The look-table cell a scene sky reads, as one phrase for a binding line:
/// `the renderer's own clear sky`, or the cell's name and its four judged numbers.
pub fn sky_phrase(class: DaylightClass, weather: WorldWeather) -> String {
    match overcast_cell(class, weather) {
        None => "the renderer's own clear sky".to_string(),
        Some(c) => format!(
            "overcast cell {}×{}: skyLight {}, apparentSkyLight {}, sun {}, fog {}",
            class.name(),
            weather.keyword(),
            c.sky_light,
            c.apparent_sky_light,
            c.sun_intensity,
            c.fog_density
        ),
    }
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
/// exactly the stale-render trap [`crate::compiler::view::cache`] exists to close. Verified
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
/// `delvec scene` in `main.rs`), consumed only by the night-vision REVIEW
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
    let sky = plan_sky(&plan)?;

    // The ground the layout stands in is loaded with it. On a `valley` the
    // landform is real blocks in the save, OUTSIDE the layout AABB — a chunk
    // list keyed to the layout alone renders a delve floating in nothing while
    // the mountains sit unloaded on disk.
    let (fmin, fmax) = loaded_extent(&plan.layout_aabb, plan.horizon);
    let chunks = chunk_list(fmin, fmax);
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
            sky: None,
            fog: None,
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
        .under(sky_of(sky))
        .with_water_world(water);
        out.push((format!("{stem}.json"), scene.to_bytes()?));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &[u8] = include_bytes!("../../../tests/fixtures/view/render-plan-mini.json");

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
          "sky":{"time":"noon","daytime_ticks":6000,"weather":"clear"},
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
      "sky":{"time":"midnight","daytime_ticks":18000,"weather":"clear"},
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

    const OCEAN_FIXTURE: &[u8] =
        include_bytes!("../../../tests/fixtures/view/render-plan-ocean.json");

    /// **The two published formulas for Minecraft's sky angle agree**, at every
    /// hour the DSL can state — minecraft.wiki's (`sky_angle_rad`, which
    /// [`sun_at`] uses) and the game's own `DimensionType.timeOfDay` curve,
    /// written out here independently. A measurement that is a deliverable is
    /// cross-checked by a second method sharing no configuration with the
    /// first, and the sun's position is exactly that: it decides what every
    /// review frame looks like.
    #[test]
    fn the_two_published_sun_angle_formulas_agree() {
        fn time_of_day_curve(ticks: i64) -> f64 {
            // DimensionType.timeOfDay: d = frac(t/24000 - 0.25);
            // (d*2 + (0.5 - cos(d*pi)/2)) / 3, a full turn.
            let d = (ticks as f64 / 24000.0 - 0.25).rem_euclid(1.0);
            let e = 0.5 - (d * std::f64::consts::PI).cos() / 2.0;
            (d * 2.0 + e) / 3.0 * std::f64::consts::TAU
        }
        let hours = [1000, 6000, 12000, 13000, 18000, 23000];
        for t in hours {
            let a = sky_angle_rad(t).rem_euclid(std::f64::consts::TAU);
            let b = time_of_day_curve(t).rem_euclid(std::f64::consts::TAU);
            assert!((a - b).abs() < 1e-9, "hour {t}: wiki {a} vs game {b}");
        }
        assert_eq!(hours.len(), 6, "every WorldTime the DSL states was checked");
    }

    /// The four facts a reader can check against the game without running it:
    /// noon is overhead, midnight is straight down, the morning sun is in the
    /// east and the evening sun in the west, and `dusk` is a low sun still above
    /// the horizon rather than a set one.
    #[test]
    fn the_sun_stands_where_the_hour_says() {
        let deg = |s: &ChunkySun| s.altitude.to_degrees();
        // The emitted azimuth is rounded to six decimal radians (`round6`), so
        // the comparisons are against the rounded values — this is the emitter's
        // own precision, not slack in the rule.
        let east = 0.0;
        let west = round6(std::f64::consts::PI);
        let tol = 1e-3; // degrees: what six decimal radians can resolve.

        let noon = sun_at(6000);
        assert!((deg(&noon) - 90.0).abs() < tol, "{noon:?}");
        let midnight = sun_at(18000);
        assert!((deg(&midnight) + 90.0).abs() < tol, "{midnight:?}");

        let morning = sun_at(1000);
        assert!(deg(&morning) > 0.0 && deg(&morning) < 45.0, "{morning:?}");
        assert_eq!(morning.azimuth, east, "the morning sun is in the east");

        let dusk = sun_at(12000);
        assert!(
            deg(&dusk) > 0.0 && deg(&dusk) < 20.0,
            "dusk is a low sun still up: {dusk:?}"
        );
        assert_eq!(dusk.azimuth, west, "the evening sun is in the west");

        // `night` and `dawn` are the same small angle below the horizon on
        // opposite sides — the sun has just gone, or is about to come.
        let night = sun_at(13000);
        let dawn = sun_at(23000);
        assert!(deg(&night) < 0.0 && deg(&dawn) < 0.0, "{night:?} {dawn:?}");
        assert!((deg(&night) - deg(&dawn)).abs() < tol);
        assert_eq!((night.azimuth, dawn.azimuth), (west, east));
    }

    /// The perturbation that proves the emitted bytes are bound to the declared
    /// hour: change the hour, and the sun in every scene moves.
    #[test]
    fn changing_the_declared_hour_moves_the_emitted_sun() {
        let at = |ticks: i64| {
            let plan = format!(
                r#"{{"campaign_id":"c","layout_aabb":{{"min":[0,64,0],"max":[1,65,1]}},
                   "sky":{{"time":"x","daytime_ticks":{ticks},"weather":"clear"}},
                   "shots":[{{"id":"seam/x/0","kind":"seam","camera":{{"pos":[0.5,66.0,0.5],
                   "yaw":0.0,"pitch":0.0,"look_at":[4.5,66.0,0.5]}}}}]}}"#
            );
            let scenes = scenes_from_plan(plan.as_bytes(), &SceneOptions::default(), &[]).unwrap();
            let v: serde_json::Value = serde_json::from_slice(&scenes[0].1).unwrap();
            v["sun"].clone()
        };
        let dusk = at(12000);
        let noon = at(6000);
        assert!(!dusk.is_null(), "every scene carries a sun");
        assert_ne!(dusk, noon, "the emitted sun follows the declared hour");
    }

    /// A plan with no hour is refused. Chunky's default sun is a midday one, so
    /// emitting anyway would hand back a noon frame of a midnight delve and say
    /// nothing — the exact silence the `sky` fact exists to end.
    #[test]
    fn a_plan_with_no_declared_hour_is_dw0721() {
        let no_sky = br#"{"campaign_id":"c","layout_aabb":{"min":[0,64,0],"max":[1,65,1]},
          "shots":[{"id":"seam/x/0","kind":"seam","camera":{"pos":[0.5,66.0,0.5],
          "yaw":0.0,"pitch":0.0,"look_at":[4.5,66.0,0.5]}}]}"#;
        let err = scenes_from_plan(no_sky, &SceneOptions::default(), &[]).unwrap_err();
        assert_eq!(err.code, DW_INPUT, "expected DW0721: {err:?}");
        assert!(err.message.contains("`sky`"), "{err:?}");
    }

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

    /// The golden is a RECORDING of what this emitter writes, and the assertion
    /// below is over its bytes — so it must never be reformatted.
    ///
    /// That is why it lives under `tests/golden/` and not under
    /// `tests/fixtures/`. The `delvec fmt --check` sweep in CI
    /// (`tools/ci/check-json-canonical.py`) covers every JSON file git tracks, and
    /// `tests/golden/` is its ONE exemption. This file is neither authored nor
    /// Delvewright JSON — it is
    /// Chunky's scene schema, in Chunky's own key order, and canonical form
    /// would sort those keys and break the byte equality this test exists to
    /// make. `delvewright_dsl::fmt`'s own `BUILD_OUTPUT_MARKER` states the same
    /// principle for `delvec build` output trees: emitted trees are not authored
    /// content, and rewriting one breaks the byte-identity contract it exists to
    /// record.
    ///
    /// The classification is not a claim anyone has to take on trust, and it is
    /// not something a drifted authored fixture could imitate: this test fails
    /// unless the file equals live emitter output byte for byte, and
    /// `every_golden_is_emitter_output` refuses any OTHER file appearing beside
    /// it. Together those are strictly stronger than canonical form would be.
    #[test]
    fn golden_scene_matches() {
        let scenes = scenes_from_plan(FIXTURE, &SceneOptions::default(), &[]).unwrap();
        let golden = include_bytes!("../../../tests/golden/view/spawn.json");
        let (_, spawn) = scenes.iter().find(|(n, _)| n == "mini_spawn.json").unwrap();
        assert_eq!(
            std::str::from_utf8(spawn).unwrap(),
            std::str::from_utf8(golden).unwrap(),
            "spawn.json scene drifted from golden"
        );
    }

    // ---- spec-0079: the sky a scene is taken under ---------------------------

    const ALL_TIMES: [delvewright_dsl::WorldTime; 6] = [
        delvewright_dsl::WorldTime::Day,
        delvewright_dsl::WorldTime::Noon,
        delvewright_dsl::WorldTime::Dusk,
        delvewright_dsl::WorldTime::Night,
        delvewright_dsl::WorldTime::Midnight,
        delvewright_dsl::WorldTime::Dawn,
    ];
    const WEATHERS: [WorldWeather; 3] = [
        WorldWeather::Clear,
        WorldWeather::Rain,
        WorldWeather::Thunder,
    ];

    /// The eleven keys an overcast block writes beyond the sun's direction, as
    /// JSON pointers into an emitted scene.
    const BLOCK_KEYS: [&str; 11] = [
        "/sky/mode",
        "/sky/color",
        "/sky/skyLight",
        "/sky/apparentSkyLight",
        "/sun/intensity",
        "/sun/color",
        "/sun/drawTexture",
        "/fog/mode",
        "/fog/uniformDensity",
        "/fog/color",
        "/fog/skyFogDensity",
    ];

    /// **Criterion 6 — the class is the sun's.** Each of the six hours' daylight
    /// class is read off `sun_at`'s altitude, and the thunder cell of every class
    /// is darker than its rain cell and denser in fog.
    ///
    /// One sub-claim is weaker than the spec's wording, and says so: `below`'s
    /// rain cell already has the sun at 0 (§4.4: the sun is under the horizon),
    /// so its thunder cell's sun can be no lower — it is EQUAL there (0 = 0), and
    /// strictly lower in the two classes whose rain sun is above 0.
    #[test]
    fn the_class_is_the_suns_and_thunder_is_darker_than_rain() {
        use delvewright_dsl::WorldTime as T;
        let want = [
            (T::Noon, DaylightClass::High),
            (T::Day, DaylightClass::High),
            (T::Dusk, DaylightClass::Low),
            (T::Night, DaylightClass::Below),
            (T::Midnight, DaylightClass::Below),
            (T::Dawn, DaylightClass::Below),
        ];
        for (t, class) in want {
            let sun = sun_at(t.daytime_ticks());
            eprintln!(
                "class: {} altitude {:.4} rad ({:.2} deg) -> {}",
                t.keyword(),
                sun.altitude,
                sun.altitude.to_degrees(),
                daylight_class(&sun).name()
            );
            assert_eq!(daylight_class(&sun), class, "{}", t.keyword());
        }
        // The dusk altitude the showcase record read off an emitted scene, to
        // the four places it states (0.2169 rad).
        let dusk = sun_at(T::Dusk.daytime_ticks()).altitude;
        assert_eq!((dusk * 1e4).round() / 1e4, 0.2169, "{dusk}");
        let mut compared = 0;
        for class in DaylightClass::ALL {
            let rain = overcast_cell(class, WorldWeather::Rain).unwrap();
            let thunder = overcast_cell(class, WorldWeather::Thunder).unwrap();
            assert!(thunder.sky_light < rain.sky_light, "{class:?} skyLight");
            assert!(
                thunder.apparent_sky_light < rain.apparent_sky_light,
                "{class:?} apparentSkyLight"
            );
            if rain.sun_intensity > 0.0 {
                assert!(thunder.sun_intensity < rain.sun_intensity, "{class:?} sun");
            } else {
                assert_eq!(thunder.sun_intensity, 0.0, "{class:?} sun");
            }
            assert!(thunder.fog_density > rain.fog_density, "{class:?} fog");
            compared += 1;
        }
        assert_eq!(compared, 3);
    }

    /// The table is the measured cell and the two authored rules of spec-0079
    /// §4.4, and nothing else: `high` doubles the measured cell's light
    /// modifiers, `below` takes a tenth of them and no sun, `thunder` is 0.6 of
    /// its rain cell's modifiers, half its sun and 1.5 times its fog.
    #[test]
    fn the_look_table_is_the_measured_cell_and_two_rules() {
        let close = |a: f64, b: f64| (a - b).abs() < 1e-12;
        assert_eq!(LOW_RAIN.sky_light, 1.6);
        assert_eq!(LOW_RAIN.apparent_sky_light, 0.6);
        assert_eq!(LOW_RAIN.sun_intensity, 0.25);
        assert_eq!(LOW_RAIN.fog_density, 0.002);
        assert!(close(HIGH_RAIN.sky_light, 2.0 * LOW_RAIN.sky_light));
        assert!(close(
            HIGH_RAIN.apparent_sky_light,
            2.0 * LOW_RAIN.apparent_sky_light
        ));
        assert!(close(BELOW_RAIN.sky_light, LOW_RAIN.sky_light / 10.0));
        assert!(close(
            BELOW_RAIN.apparent_sky_light,
            LOW_RAIN.apparent_sky_light / 10.0
        ));
        assert_eq!(BELOW_RAIN.sun_intensity, 0.0);
        for class in DaylightClass::ALL {
            let r = overcast_cell(class, WorldWeather::Rain).unwrap();
            let t = overcast_cell(class, WorldWeather::Thunder).unwrap();
            assert!(close(t.sky_light, 0.6 * r.sky_light), "{class:?}");
            assert!(
                close(t.apparent_sky_light, 0.6 * r.apparent_sky_light),
                "{class:?}"
            );
            assert!(close(t.sun_intensity, 0.5 * r.sun_intensity), "{class:?}");
            assert!(close(t.fog_density, 1.5 * r.fog_density), "{class:?}");
            for c in [r, t] {
                assert_eq!(
                    (c.sky_color, c.sun_color, c.fog_color),
                    (LOW_RAIN.sky_color, LOW_RAIN.sun_color, LOW_RAIN.fog_color)
                );
            }
        }
    }

    /// A plan of `time`, `weather`, with one review shot.
    fn sky_plan(time: delvewright_dsl::WorldTime, weather: WorldWeather) -> Vec<u8> {
        format!(
            r#"{{"campaign_id":"c","layout_aabb":{{"min":[0,64,0],"max":[15,80,15]}},
              "sky":{{"time":"{}","daytime_ticks":{},"weather":"{}"}},
              "shots":[{{"id":"pov/a","kind":"pov","camera":{{"pos":[4.5,70.0,4.5],
              "yaw":0.0,"pitch":5.0,"look_at":[8.5,70.0,4.5]}}}}]}}"#,
            time.keyword(),
            time.daytime_ticks(),
            weather.keyword()
        )
        .into_bytes()
    }

    /// **Criterion 5 — the block is whole and one.** Over every hour and every
    /// weather, through all three scene builders (a review frame, a panorama, a
    /// showcase camera): a non-clear scene carries all eleven block keys, with
    /// its cell's values, and a clear scene carries none of them.
    #[test]
    fn every_scene_builder_writes_the_whole_block_or_none_of_it() {
        use crate::compiler::view::{camera, panorama};
        let mut judged = 0;
        for time in ALL_TIMES {
            for weather in WEATHERS {
                let plan = sky_plan(time, weather);
                let review = scenes_from_plan(&plan, &SceneOptions::default(), &[]).unwrap();
                let pano =
                    panorama::panorama_from_plan(&plan, &[], &panorama::PanoramaOptions::default())
                        .unwrap();
                let sheet = camera::CameraSheet {
                    campaign_id: "c".into(),
                    cameras: vec![camera::Camera {
                        answers: "concept/x".into(),
                        exposure: 1.0,
                        fov: 60.0,
                        height: 90,
                        name: "x".into(),
                        pitch: 5.0,
                        pos: [4.5, 70.0, 4.5],
                        sky: None,
                        source: camera::Source::Estimated,
                        spp: 16,
                        width: 160,
                        yaw: 270.0,
                    }],
                };
                let rows = [camera::ApprovedRow {
                    name: "concept/x".into(),
                    shows: String::new(),
                    sky: Some(camera::CameraSky { time, weather }),
                }];
                let cam =
                    camera::emit(&plan, &sheet, &rows, &camera::EmitOptions::default()).unwrap();
                let class = daylight_class(&sun_at(time.daytime_ticks()));
                for (kind, bytes) in [
                    ("review", &review[0].1),
                    ("panorama", &pano.bytes),
                    ("camera", &cam.scenes[0].1),
                ] {
                    let v: serde_json::Value = serde_json::from_slice(bytes).unwrap();
                    let at = format!("{kind} {}+{}", time.keyword(), weather.keyword());
                    match overcast_cell(class, weather) {
                        None => {
                            for k in BLOCK_KEYS {
                                assert!(v.pointer(k).is_none(), "{at}: a clear scene wrote {k}");
                            }
                            assert!(v.get("sky").is_none() && v.get("fog").is_none(), "{at}");
                        }
                        Some(cell) => {
                            for k in BLOCK_KEYS {
                                assert!(v.pointer(k).is_some(), "{at}: missing {k}");
                            }
                            assert_eq!(v["sky"]["mode"], "SOLID_COLOR", "{at}");
                            assert_eq!(v["fog"]["mode"], "UNIFORM", "{at}");
                            assert_eq!(v["sun"]["drawTexture"], false, "{at}");
                            assert_eq!(v["fog"]["skyFogDensity"], 0.0, "{at}");
                            assert_eq!(v["sky"]["skyLight"], cell.sky_light, "{at}");
                            assert_eq!(
                                v["sky"]["apparentSkyLight"], cell.apparent_sky_light,
                                "{at}"
                            );
                            assert_eq!(v["sun"]["intensity"], cell.sun_intensity, "{at}");
                            assert_eq!(v["fog"]["uniformDensity"], cell.fog_density, "{at}");
                        }
                    }
                    // The sun's direction is the hour's, whatever the weather.
                    let sun = sun_at(time.daytime_ticks());
                    assert_eq!(v["sun"]["altitude"], sun.altitude, "{at}");
                    assert_eq!(v["sun"]["azimuth"], sun.azimuth, "{at}");
                    judged += 1;
                }
            }
        }
        eprintln!("sky block: {judged} scene(s) judged over 6 hours x 3 weathers x 3 builders");
        assert_eq!(judged, 6 * 3 * 3);
    }

    /// An overcast scene is byte-deterministic: two emissions, one set of bytes.
    #[test]
    fn an_overcast_scene_is_byte_deterministic() {
        for weather in [WorldWeather::Rain, WorldWeather::Thunder] {
            let plan = sky_plan(delvewright_dsl::WorldTime::Dusk, weather);
            assert_eq!(
                scenes_from_plan(&plan, &SceneOptions::default(), &[]).unwrap(),
                scenes_from_plan(&plan, &SceneOptions::default(), &[]).unwrap()
            );
        }
    }

    /// A plan that states the hour and not the weather was written by an older
    /// engine, and every plan-sky builder refuses it by name.
    #[test]
    fn a_plan_with_no_weather_is_refused_naming_the_engine() {
        let plan = br#"{"campaign_id":"c","layout_aabb":{"min":[0,64,0],"max":[1,65,1]},
            "sky":{"time":"dusk","daytime_ticks":12000},"shots":[]}"#;
        let err = scenes_from_plan(plan, &SceneOptions::default(), &[]).unwrap_err();
        assert_eq!(err.code, DW_INPUT);
        assert!(
            err.message.contains("sky.weather")
                && err.message.contains("older than this one")
                && err.message.contains(env!("CARGO_PKG_VERSION")),
            "{err:?}"
        );
        let err = crate::compiler::view::panorama::panorama_from_plan(
            plan,
            &[],
            &crate::compiler::view::panorama::PanoramaOptions::default(),
        )
        .unwrap_err();
        assert!(err.message.contains("sky.weather"), "{err:?}");
    }

    /// **A clear scene's bytes are the bytes the engine wrote before the weather
    /// reached a scene** (spec-0079 §4.1, criterion 4). The two goldens were
    /// emitted by the engine at the merge of spec-0079's base (`19eba477` plus
    /// `main`, before any of this change) for this record, plan and options, and
    /// are compared here byte for byte: a showcase camera answering a clear row
    /// at the plan's own hour, and the default panorama of the same clear plan.
    /// The review scene's golden is `golden_scene_matches`'s `spawn.json`.
    #[test]
    fn clear_scenes_keep_their_base_bytes() {
        use crate::compiler::view::{camera, panorama};
        use delvewright_dsl::WorldTime;
        let ocean = include_bytes!("../../../tests/fixtures/view/render-plan-ocean.json");
        let sheet = camera::CameraSheet {
            campaign_id: "isle".into(),
            cameras: vec![camera::Camera {
                answers: "concept/quay".into(),
                exposure: 2.0,
                fov: 50.0,
                height: 900,
                name: "hero".into(),
                pitch: 10.0,
                pos: [10.5, 70.0, -4.25],
                sky: None,
                source: camera::Source::Estimated,
                spp: 300,
                width: 1600,
                yaw: 30.0,
            }],
        };
        let rows = [camera::ApprovedRow {
            name: "concept/quay".into(),
            shows: String::new(),
            sky: Some(camera::CameraSky {
                time: WorldTime::Day,
                weather: WorldWeather::Clear,
            }),
        }];
        let e = camera::emit(
            ocean,
            &sheet,
            &rows,
            &camera::EmitOptions {
                world_path: "/abs/world".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            std::str::from_utf8(&e.scenes[0].1).unwrap(),
            std::str::from_utf8(include_bytes!(
                "../../../tests/golden/view/clear-camera.json"
            ))
            .unwrap(),
            "a clear camera scene moved from its base bytes"
        );
        let p = panorama::panorama_from_plan(ocean, &[], &panorama::PanoramaOptions::default())
            .unwrap();
        assert_eq!(
            std::str::from_utf8(&p.bytes).unwrap(),
            std::str::from_utf8(include_bytes!(
                "../../../tests/golden/view/clear-panorama.json"
            ))
            .unwrap(),
            "a clear panorama scene moved from its base bytes"
        );
    }

    /// `tests/golden/` sits outside the `delvec fmt --check` sweep on purpose,
    /// and it is now the **only** directory in the repository that does — the
    /// sweep's population is `git ls-files '*.json'`
    /// (`tools/ci/check-json-canonical.py`). That makes this test the sole thing
    /// standing between an authored JSON file and no canonical-form gate at all.
    /// A comment would not stop that — a doc line is not an invocation — so the
    /// directory's admission rule is enforced here: every `.json` under it is
    /// named by a golden test, and the set is closed. It is what the sweep's
    /// exemption POINTS AT rather than asserts, which is why that exemption is
    /// not a hatch a forgetful author can supply: getting a document in here
    /// means making the emitter actually emit its bytes.
    ///
    /// Add a golden and this reds until you have added the test that pins its
    /// bytes to emitter output. Drop an authored fixture here and it reds
    /// immediately, which is the outcome the sweep would have produced.
    #[test]
    fn every_golden_is_emitter_output() {
        // Relative to a FIXED root, never to the directory the recursion happens
        // to be in: stripping against `dir` yields a depth-dependent name, so a
        // nested golden would be reported under a different key than a
        // top-level one and the closed set below would compare the wrong
        // strings.
        fn walk(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<String>) {
            let mut entries: Vec<_> = std::fs::read_dir(dir)
                .unwrap()
                .map(|e| e.unwrap().path())
                .collect();
            entries.sort();
            for p in entries {
                if p.is_dir() {
                    walk(root, &p, out);
                } else if p.extension().and_then(|e| e.to_str()) == Some("json") {
                    out.push(
                        p.strip_prefix(root)
                            .unwrap()
                            .to_string_lossy()
                            .replace('\\', "/"),
                    );
                }
            }
        }

        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
        let mut found = Vec::new();
        walk(&root, &root, &mut found);

        // Every entry here is pinned byte-for-byte to live emitter output by the
        // test named beside it.
        let declared = [
            "view/clear-camera.json",   // pinned by clear_scenes_keep_their_base_bytes
            "view/clear-panorama.json", // pinned by clear_scenes_keep_their_base_bytes
            "view/spawn.json",          // pinned by golden_scene_matches
        ];

        assert!(
            !found.is_empty(),
            "tests/golden holds no .json at all — this check examined nothing, \
             which is a vacuous pass rather than a pass"
        );
        assert_eq!(
            found,
            declared.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            "a file under tests/golden is not pinned to emitter output by any \
             test. That directory is outside the `delvec fmt --check` sweep, so \
             an authored JSON file placed here would be gated by nothing at all. \
             Either pin it (a byte comparison against what the emitter writes) \
             and name it above, or put it under tests/fixtures where the \
             canonical-form sweep covers it"
        );
    }
}
