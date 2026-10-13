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
//! ## A sky is the pinned game's, at its tick under its weather ([`sky_cell`])
//!
//! A scene is lit as the pinned game lights its tick under its weather: the
//! light the sky casts is the client lightmap's sky colour, the lens sees the
//! timeline's sky colour and its fog at the horizon, the weather moves both
//! through the client's own weather layer ([`WeatherLayer`]), and the sun gives
//! no light, because no light term in the game depends on the sun's direction.
//! Noon, the dusk and dawn ramps, the night, rain and thunder are points of one
//! function of the tick and the weather. Left to
//! the renderer's simulated sky and default sun, a setting sun lit the
//! Treehouse Camp with a horizontal beam over a yellow-brown haze, where the
//! game draws a dim, flat, blue-violet evening.
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
/// legible but dim, 0.15 verges on overbright; 0.05 (× the core's default
/// `emitterIntensity` 13, [`review_emittance`]) reads like the in-game night-vision view while real
/// emitters (the camp fire pit) still stand out.
pub const REVIEW_EMITTANCE: f64 = 0.05;

/// The emittance an emulated scene writes, so that it times the scene's
/// [`emitter_intensity`] stays the calibrated `REVIEW_EMITTANCE ×
/// CORE_EMITTER_INTENSITY`: the emulation's glow does not move with the
/// lantern light.
pub fn review_emittance() -> f64 {
    round6(REVIEW_EMITTANCE * CORE_EMITTER_INTENSITY / emitter_intensity())
}

/// The pinned core's default `emitterIntensity`: an emitter's radiance is its
/// texel colour squared × its material's `emittance` × this
/// (`Scene.DEFAULT_EMITTER_INTENSITY` 13; `PathTracer.doDiffuseReflection`).
/// **Cited.**
pub const CORE_EMITTER_INTENSITY: f64 = 13.0;

/// **The scale on the core's emitter light that makes a lantern-lit surface
/// read, on average, as the game draws it.** The game spreads a lantern's
/// block light flat over 14 blocks (`lightmap.fsh`, the block factor 1.5
/// `LightTexture` writes); the core's lantern is a small emitter whose light
/// falls off with distance, and the only controls it gives a scene over
/// emitted light are this global intensity, a per-material `emittance` and the
/// emitter sampling strategy, whose sampled estimate at the pin averages the
/// emitters near a point where the unsampled path sums them (measured: half
/// the emitted light on two Treehouse Camp frames), so a scene keeps `NONE`.
/// Chunky users close the same gap with the same lever, raising the intensity
/// by eye, and pair it with a denoiser
/// (`docs/reference/chunky-lighting-practice.md`, **cited**). The scale is the
/// one at which the mean display luma of lantern-lit pixels under
/// [`tone_curve`], **after denoising**, equals the game model's: over every
/// pixel of the Treehouse Camp's twelve showcase frames that lands on a full
/// cube of a single-texture block with game block light 1–15 (102 139 pixels
/// at 400 × 225), equal weight per pixel, the render taken as the sky pass plus
/// the scale times the emitter pass, each a raw sample buffer of the pinned
/// core, denoised by Open Image Denoise with the denoiser plugin's albedo and
/// normal passes; the game model the texture's albedo × the pinned client's
/// lightmap × `ClientLevel.getShade` with the game's light flooded over the
/// same world. Fitted on the noisy buffers instead the scale reads 15.04,
/// because grain shown through the curve reads darker than its mean.
/// `compiler.md` §4 holds the record. **Authored from that measurement.**
pub const LANTERN_LIGHT_SCALE: f64 = 12.2310;

/// The `emitterIntensity` every scene is written with:
/// [`CORE_EMITTER_INTENSITY`] × [`LANTERN_LIGHT_SCALE`].
pub fn emitter_intensity() -> f64 {
    round6(CORE_EMITTER_INTENSITY * LANTERN_LIGHT_SCALE)
}

/// How far below a water cell's top face the rendered surface sits, in blocks.
/// Vanilla draws a full source block 1/8 short of the cell top; Chunky matches
/// it (`Water.TOP_BLOCK_GAP` in the pinned core). So the surface of the water
/// block at `y` is at `y + 1 - 0.125`, and a plane anywhere else meets the
/// authored block water in a visible two-tone seam.
pub const WATER_SURFACE_GAP: f64 = 0.125;

/// **The post-process every scene is written with**: the pinned core's Hable
/// filmic curve (`TONEMAP3`, `HableToneMappingFilter` at its default
/// "Filmic Worlds" preset), which rolls a radiance above the display's white
/// off along a shoulder instead of cutting it at 1 as `GAMMA` does
/// (`SimplePixelPostProcessingFilter` clamps after the filter).
///
/// Chosen by measurement over the five the core offers (`GAMMA`, `TONEMAP1`,
/// `TONEMAP2` ACES, `TONEMAP3` Hable, `UE4_FILMIC`), each re-exposed from one
/// accumulated render at exposure 1 under the clear sky ([`sky_cell`]) —
/// the Treehouse Camp's `from-the-glade`, `north-east-aerial` and
/// `lantern-night` drafts: Hable is the one curve that cuts the clipped share
/// (any channel at 254 or more: 0.60 → 0.37%, 0.18 → 0.02%, 0.49 → 0.11%)
/// without crushing the shadows (luma at 8 or less: 5.24 → 7.59%, 0.39 →
/// 0.70%, 0.84 → 1.22%); `TONEMAP1`, ACES and UE4 clip less too, and crush
/// 3–85 points more, because their toes sink the dim half of a frame. At its
/// preset the curve shows linear 0.18 at 0.393 where the game's transfer shows
/// 0.459, so it is written with its mid-grey anchored ([`tone_curve`]).
/// The record is `compiler.md` §4, *The sky of a scene*. **Authored.**
pub const POSTPROCESS: &str = "TONEMAP3";

/// The pinned core's Hable curve as a scene stores it: `postprocessSettings`,
/// seven floats `HableToneMappingFilter.loadConfiguration` reads by these
/// names. Per channel the core computes `y = 2 · exposure · x`, `h(y) =
/// (y(Ay + CB) + DE) / (y(Ay + B) + DF) − E/F` with each product of two
/// parameters taken in float, the white scale `1 / h(W)` in float arithmetic
/// (`recalculateWhiteScale`), and shows `(h(y) · whiteScale)^0.45454543828964233`
/// clamped at 1 (`processPixel`, `SimplePixelPostProcessingFilter`). **Cited**:
/// the pinned core's bytecode.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HableCurve {
    /// `A`.
    pub shoulder_strength: f32,
    /// `B`.
    pub linear_strength: f32,
    /// `C`.
    pub linear_angle: f32,
    /// `D`.
    pub toe_strength: f32,
    /// `E`.
    pub toe_numerator: f32,
    /// `F`.
    pub toe_denominator: f32,
    /// `W`, the linear value the curve shows as white.
    pub linear_white_point_value: f32,
}

/// The core's default Hable preset, `FILMIC_WORLDS` (`HableToneMappingFilter.
/// applyPreset`, the values `reset` applies). **Cited.**
pub const HABLE_FILMIC_WORLDS: HableCurve = HableCurve {
    shoulder_strength: 0.15,
    linear_strength: 0.5,
    linear_angle: 0.1,
    toe_strength: 0.2,
    toe_numerator: 0.02,
    toe_denominator: 0.3,
    linear_white_point_value: 11.2,
};

/// The mid-tone a tone curve is anchored at: linear 0.18, the reflectance of
/// the photographic grey card exposure is metered against. **Cited**: the
/// 18% grey card as the standard metering reference (Kodak Gray Card, R-27).
pub const MID_GREY: f64 = 0.18;

/// The exponent the core's display transfer raises a value to
/// (`0.45454543828964233`, the float nearest `1/2.2`, in both `GAMMA` and
/// `TONEMAP3`). The game's transfer for a decoded colour `x` is `x^(1/2.2)`
/// ([`to_linear`] inverted), so a curve whose value before this exponent is
/// `x` shows `x` as the game draws it.
pub const CORE_DISPLAY_EXPONENT: f64 = 0.45454543828964233;

impl HableCurve {
    /// `1 / h(W)`, in float arithmetic as the core computes it.
    fn white_scale(&self) -> f32 {
        let (a, b, c, d, e, f, w) = self.params();
        let num = w * (a * w + c * b) + d * e;
        let den = w * (a * w + b) + d * f;
        1.0 / (num / den - e / f)
    }

    fn params(&self) -> (f32, f32, f32, f32, f32, f32, f32) {
        (
            self.shoulder_strength,
            self.linear_strength,
            self.linear_angle,
            self.toe_strength,
            self.toe_numerator,
            self.toe_denominator,
            self.linear_white_point_value,
        )
    }

    /// `h(y)` with the parameter products in float, the rest in double, as
    /// `processPixel` evaluates it.
    fn h(&self, y: f64) -> f64 {
        let (a, b, c, d, e, f, _) = self.params();
        let (a, b) = (f64::from(a), f64::from(b));
        let (cb, de, df, ef) = (
            f64::from(c * self.linear_strength),
            f64::from(d * e),
            f64::from(d * f),
            f64::from(e / f),
        );
        (y * (a * y + cb) + de) / (y * (a * y + b) + df) - ef
    }

    /// The value the core holds for linear `x` at exposure 1, before its
    /// display exponent ([`CORE_DISPLAY_EXPONENT`]).
    pub fn before_gamma(&self, x: f64) -> f64 {
        self.h(2.0 * x) * f64::from(self.white_scale())
    }

    /// The same curve read `k` times brighter, its white point held: `h_k(y)
    /// = h(k · y)` is the curve with `A · k²`, `B · k`, `C`, `D`, `E`, `F`
    /// (multiply out the numerator and denominator), and `W / k` keeps
    /// `h_k(W / k) = h(W)`, so the white scale is the preset's.
    pub fn brightened(&self, k: f64) -> HableCurve {
        HableCurve {
            shoulder_strength: (f64::from(self.shoulder_strength) * k * k) as f32,
            linear_strength: (f64::from(self.linear_strength) * k) as f32,
            linear_angle: self.linear_angle,
            toe_strength: self.toe_strength,
            toe_numerator: self.toe_numerator,
            toe_denominator: self.toe_denominator,
            linear_white_point_value: (f64::from(self.linear_white_point_value) / k) as f32,
        }
    }

    /// The gain `k` at which [`HableCurve::brightened`] holds linear `m` at
    /// `m` before the display exponent, so `m` shows as the game draws it. In
    /// closed form: with `y = 2km` and `q = m / whiteScale + E/F`, `h(y) =
    /// m / whiteScale` is `A(1 − q)y² + (CB − qB)y + (DE − qDF) = 0`, whose
    /// positive root is the one on the curve's rising branch. A square root and
    /// the four operations: a function of the IEEE arithmetic (ADR-0006).
    pub fn mid_grey_gain(&self, m: f64) -> f64 {
        let (a, b, c, d, e, f, _) = self.params();
        let (a, b) = (f64::from(a), f64::from(b));
        let (cb, de, df, ef) = (
            f64::from(c * self.linear_strength),
            f64::from(d * e),
            f64::from(d * f),
            f64::from(e / f),
        );
        let q = m / f64::from(self.white_scale()) + ef;
        let (qa, qb, qc) = (a * (1.0 - q), cb - q * b, de - q * df);
        let y = (-qb + (qb * qb - 4.0 * qa * qc).sqrt()) / (2.0 * qa);
        y / (2.0 * m)
    }
}

/// **The curve every scene is written with** (`postprocessSettings` under
/// [`POSTPROCESS`]): the core's preset ([`HABLE_FILMIC_WORLDS`]) brightened by
/// the gain that shows [`MID_GREY`] as the game shows it
/// ([`HableCurve::mid_grey_gain`], 1.463761 at the pin). Below the mid-grey the
/// curve sits within a few hundredths of the game's transfer, and above it
/// rolls off along Hable's shoulder where the game would clip. Measured on the
/// Treehouse Camp's twelve showcase frames, re-exposed from their accumulated
/// renders (`compiler.md` §4). **Derived**; the anchor is cited.
pub fn tone_curve() -> HableCurve {
    HABLE_FILMIC_WORLDS.brightened(HABLE_FILMIC_WORLDS.mid_grey_gain(MID_GREY))
}

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
    /// The author's spelling (`"dusk"`, or a celestial object) — for messages,
    /// never for the sun.
    #[allow(dead_code)]
    pub time: serde_json::Value,
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
    pub(crate) postprocess_settings: HableCurve,
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
    /// The sky of the scene's cell ([`sky_of`]): the game's sky of its tick
    /// under its weather, as a map. Every scene this
    /// crate emits carries one; the `Option` is what lets the type be built
    /// before it is known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) sky: Option<ChunkySkyMode>,
    /// The fog of the same block ([`sky_of`]), present where `sky` is.
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
    /// The tick a clear scene's sky map is drawn at, which the scene writer
    /// reads back to write the map beside the scene
    /// ([`crate::compiler::view::sky_image::companion`]). Chunky ignores it,
    /// as it does every unknown key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) delvewright_sky: Option<SkyMarker>,
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
        let mut sky = block.sky;
        // The sky is the game's own, as a map drawn for this camera
        // (`sky_image`): the lens sees it whole, and `skyLight` is the
        // multiplier that makes the light it casts the lightmap's.
        let at = block.sky_at;
        let o = &self.camera.orientation;
        let view = crate::compiler::view::sky_image::view_of(o.yaw, o.pitch);
        let map = crate::compiler::view::sky_image::sky_map(at.daytime_ticks, at.weather, view);
        sky.mode = "SKYMAP_EQUIRECTANGULAR";
        sky.sky_light = round6(map.sky_light);
        sky.apparent_sky_light = 1.0;
        sky.skymap = Some(format!("{}{SKY_MAP_SUFFIX}", self.name));
        sky.sky_mirrored = Some(false);
        self.delvewright_sky = Some(SkyMarker {
            daytime_ticks: at.daytime_ticks,
            weather: (at.weather != WorldWeather::Clear).then(|| at.weather.keyword()),
        });
        self.sky = Some(sky);
        self.fog = Some(block.fog);
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
/// *Minecraft.* The sun the party sees is the pinned day timeline's
/// `visual/sun_angle` track — two keyframes at tick 6000 (360 then 0) eased by
/// `cubic_bezier [0.362, 0.241, 0.638, 0.759]` — which the client's
/// `SkyRenderer` reads from the attribute system (spec-0081 §2.3). It is read
/// here from the vendored file through [`delvewright_dsl::celestial`], the same
/// curve the position table of a celestial time is derived from, so a frame and
/// the table can never disagree about where the sun stands. The body travels a
/// great circle through the zenith in the east–west plane, so the angle `α`
/// from the zenith is an altitude of `90° − α` on the western half of the arc
/// and `α − 270°` on the eastern half — piecewise linear, no trigonometry.
///
/// minecraft.wiki's closed form (*Daylight cycle*, §Sky angle), which this
/// function used before the timeline was vendored, is kept as the second
/// method: it agrees with the pinned track to 0.056°
/// (`the_two_published_sun_angle_formulas_agree`).
///
/// *Chunky.* The direction toward the sun is
/// `(cos az · cos alt, sin alt, sin az · cos alt)`, verified against the pinned
/// core's `Sun.initSun` bytecode; a sun below the horizon is expressed as a
/// negative altitude. The pinned core's simulated sky does NOT render that as
/// night: `PreethamSky.updateSun` clamps the altitude to `[0, π]` before it
/// shades the sky, so a clear scene under a sun below the horizon would draw a
/// sunset; no scene is left to that sky — every clear scene carries the clear
/// cell of its tick ([`sky_cell`]). The azimuth is exactly east or exactly
/// west; at noon and midnight the sun
/// is at the zenith or the nadir and the azimuth means nothing, so east is
/// emitted and the bytes are still a function of the hour alone.
pub fn sun_at(daytime_ticks: i64) -> ChunkySun {
    let t = daytime_ticks as f64;
    let altitude = delvewright_dsl::celestial::sun_altitude_degrees(t).to_radians();
    let azimuth = if delvewright_dsl::celestial::sun_is_west(t) {
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

/// The pinned track's sun angle for a `daytime` tick value, in radians, 0 at
/// noon and growing westward. See [`sun_at`].
#[cfg(test)]
fn sky_angle_rad(daytime_ticks: i64) -> f64 {
    delvewright_dsl::celestial::sun_angle_degrees(daytime_ticks as f64).to_radians()
}

/// minecraft.wiki's closed-form sky angle, in radians — the second method
/// [`sun_at`]'s track is measured against.
#[cfg(test)]
fn closed_form_sky_angle_rad(daytime_ticks: i64) -> f64 {
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
                sky.time
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| sky.time.to_string()),
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

/// Chunky's `sky` object, as a scene writes it (`renderer/scene/sky/
/// Sky.java` at the pinned core): its mode and colour, the light the sky casts
/// (`skyLight`) and the brightness the lens sees (`apparentSkyLight`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkySkyMode {
    pub mode: &'static str,
    pub color: Rgb,
    pub sky_light: f64,
    pub apparent_sky_light: f64,
    /// The scene's sky map, a file beside the scene (`Sky.importFromJson`
    /// reads `skymap` and resolves a relative name in the scene's directory,
    /// `SceneIOProvider.resolveLinkedFile`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skymap: Option<String>,
    /// `false` with a map: the lower half of the sky is the map's own, not the
    /// upper half mirrored (`skyMirrored` defaults to `true`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sky_mirrored: Option<bool>,
}

/// The suffix a scene's sky map is named with: `<scene name>.sky.pfm`.
pub const SKY_MAP_SUFFIX: &str = ".sky.pfm";

/// [`ChunkyScene::delvewright_sky`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkyMarker {
    pub daytime_ticks: i64,
    /// The weather the map is drawn under, absent under a clear sky.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weather: Option<&'static str>,
}

/// Chunky's `fog` object, as a scene writes it (`renderer/scene/Fog.java`
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

/// Every value a scene writes about its sky beyond the sun's direction: the
/// cell of its tick and weather ([`sky_cell`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LookCell {
    pub sky_color: Rgb,
    pub sky_light: f64,
    pub apparent_sky_light: f64,
    pub sun_intensity: f64,
    pub sun_color: Rgb,
    /// Whether the renderer draws the sun's disc.
    pub sun_draw_texture: bool,
    pub fog_density: f64,
    /// How much of the fog colour the lens sees painted over the sky, from the
    /// horizon up (`Fog.addSkyFog` at the pinned core: `(1 − dy)²` of it at a
    /// view direction whose height is `dy`, scaled by this).
    pub sky_fog_density: f64,
    pub fog_color: Rgb,
}

// ---- the weather: the pinned client's weather layer ---------------------------

/// **The pinned client's weather layer** (`WeatherAttributes`, read off its
/// bytecode through Mojang's client mappings): the environment-attribute
/// system lays it over the dimension, the biome and the day timeline in every
/// level that `canHaveWeather` (`EnvironmentAttributeSystem.addDefaultLayers`),
/// so every attribute the client reads — the lightmap's two included — is the
/// timeline's value with this layer applied. Two maps, `RAIN` and `THUNDER`;
/// the value `v` of an attribute becomes `lerp(rain − thunder, v, RAIN(v))`,
/// then `lerp(thunder, ·, THUNDER(·))`, with the lerp the attribute type's own
/// (`WeatherAttributes.addLayer`). A `rain` delve stands at rain level 1 and
/// thunder level 0 and a `thunder` delve at both 1 (`Level.getThunderLevel` is
/// the thunder level times the rain level), so under rain the `RAIN` map is
/// applied whole and under thunder the `THUNDER` map alone. **Cited**, value by
/// value (`WeatherAttributes.<clinit>`):
///
/// | attribute | `RAIN` | `THUNDER` |
/// |---|---|---|
/// | `visual/sky_color` | `blend_to_gray` 0.6, 0.75 | `blend_to_gray` 0.24, 0.94 |
/// | `visual/fog_color`, `visual/sunrise_sunset_color` | `multiply` (0.5, 0.5, 0.6) | `multiply` (0.25, 0.25, 0.3) |
/// | `visual/sky_light_color` | `alpha_blend` `#7a7aff` at 0.3125 | the same at 0.52734375 |
/// | `visual/sky_light_factor` | `alpha_blend` 0.24 at 0.3125 | the same at 0.52734375 |
///
/// The layer also writes the cloud colour, the star brightness and the sky
/// light level, which nothing a scene carries reads. `#7a7aff` is
/// `Timelines.NIGHT_SKY_LIGHT_COLOR`, `ARGB.colorFromFloat(1, 0.48, 0.48, 1)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeatherLayer {
    /// `blend_to_gray` on the sky colour: (brightness, factor).
    pub sky_gray: (f32, f32),
    /// `multiply` on the fog and sunrise colours, as the float channels
    /// `ARGB.colorFromFloat` takes.
    pub darken: [f32; 3],
    /// The alpha of both `alpha_blend`s.
    pub alpha: f32,
}

/// `WeatherAttributes.RAIN`. **Cited.**
pub const RAIN_LAYER: WeatherLayer = WeatherLayer {
    sky_gray: (0.6, 0.75),
    darken: [0.5, 0.5, 0.6],
    alpha: 0.3125,
};

/// `WeatherAttributes.THUNDER`. **Cited.**
pub const THUNDER_LAYER: WeatherLayer = WeatherLayer {
    sky_gray: (0.24, 0.94),
    darken: [0.25, 0.25, 0.3],
    alpha: 0.52734375,
};

/// The sky-light factor and colour both `alpha_blend`s blend toward.
const WEATHER_SKY_LIGHT_FACTOR: f32 = 0.24;
const NIGHT_SKY_LIGHT_COLOR: [f32; 3] = [0.48, 0.48, 1.0];

/// The map a weather's delve stands under whole, or `None` under a clear sky
/// (rain level 0).
pub fn weather_layer(weather: WorldWeather) -> Option<&'static WeatherLayer> {
    match weather {
        WorldWeather::Clear => None,
        WorldWeather::Rain => Some(&RAIN_LAYER),
        WorldWeather::Thunder => Some(&THUNDER_LAYER),
    }
}

/// The rain and thunder levels of a weather, as `Level.getRainLevel` and
/// `Level.getThunderLevel` read once the weather has settled.
pub fn weather_levels(weather: WorldWeather) -> (f32, f32) {
    match weather {
        WorldWeather::Clear => (0.0, 0.0),
        WorldWeather::Rain => (1.0, 0.0),
        WorldWeather::Thunder => (1.0, 1.0),
    }
}

/// `ARGB.as8BitChannel`: `floor(f × 255)` in float. **Cited.**
fn as_8bit(f: f32) -> i32 {
    (f * 255.0).floor() as i32
}

/// `Mth.lerpInt`: `a + floor(f × (b − a))`, the product in float. **Cited.**
fn lerp_int_f32(f: f32, a: i32, b: i32) -> i32 {
    a + (f * (b - a) as f32).floor() as i32
}

/// `ARGB.scaleRGB` of one channel: `clamp((int)(c × f), 0, 255)`. **Cited.**
fn scale_channel(c: i32, f: f32) -> i32 {
    ((c as f32 * f) as i32).clamp(0, 255)
}

/// `ColorModifier.BLEND_TO_GRAY`: the colour's grey (`ARGB.greyscale`, `(int)
/// (0.3 r + 0.59 g + 0.11 b)` in float) scaled by `brightness`, and the colour
/// `srgbLerp`ed toward it by `factor`. **Cited.**
pub fn blend_to_gray(c: [u8; 3], brightness: f32, factor: f32) -> [u8; 3] {
    let [r, g, b] = c.map(f32::from);
    let grey = (r * 0.3 + g * 0.59 + b * 0.11) as i32;
    let scaled = scale_channel(grey, brightness);
    c.map(|v| lerp_int_f32(factor, i32::from(v), scaled) as u8)
}

/// `ARGB.alphaBlend` of an opaque colour under a colour of alpha `a` (8-bit):
/// `(src × a + dst × (255 − a)) / 255` per channel, in integers. Every colour
/// the timeline or a biome writes as `#rrggbb` is opaque (`STRING_RGB_COLOR`
/// decodes through `ARGB.opaque`). **Cited.**
fn alpha_blend_rgb(dst: [u8; 3], src: [u8; 3], a: i32) -> [u8; 3] {
    std::array::from_fn(|i| ((i32::from(src[i]) * a + i32::from(dst[i]) * (255 - a)) / 255) as u8)
}

/// **The sky attributes the pinned client reads at a tick under a weather**:
/// the day timeline's value, over the biome a delve stands in, with the
/// weather's layer applied ([`WeatherLayer`]). The one place a scene's sky
/// reads the game's numbers: the light ([`sky_cell`]) and the map the lens sees
/// ([`crate::compiler::view::sky_image`]) both take them from here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkyAttributes {
    /// `visual/sky_light_factor`.
    pub sky_light_factor: f64,
    /// `visual/sky_light_color`.
    pub sky_light_color: [u8; 3],
    /// `visual/sky_color`: the timeline's multiplier times [`GROUND_SKY_RGB`].
    pub sky_color: [u8; 3],
    /// `visual/fog_color`: the timeline's multiplier times [`OVERWORLD_FOG_RGB`].
    pub fog_color: [u8; 3],
    /// `visual/sunrise_sunset_color`, `[a, r, g, b]`.
    pub sunrise: [u8; 4],
}

/// See [`SkyAttributes`].
pub fn sky_attributes(daytime_ticks: i64, weather: WorldWeather) -> SkyAttributes {
    let t = daytime_ticks;
    let mut at = SkyAttributes {
        sky_light_factor: day_float("visual/sky_light_factor", t),
        sky_light_color: day_rgb("visual/sky_light_color", t),
        sky_color: multiply_rgb(day_rgb("visual/sky_color", t), GROUND_SKY_RGB),
        fog_color: multiply_rgb(day_rgb("visual/fog_color", t), OVERWORLD_FOG_RGB),
        sunrise: day_argb("visual/sunrise_sunset_color", t),
    };
    if let Some(layer) = weather_layer(weather) {
        let a = layer.alpha;
        // `Mth.lerp(alpha, v, 0.24)`, then the layer's own `lerp(1, v, ·)`, in float.
        let f = at.sky_light_factor as f32;
        let m = f + a * (WEATHER_SKY_LIGHT_FACTOR - f);
        at.sky_light_factor = f64::from(f + (m - f));
        let night = NIGHT_SKY_LIGHT_COLOR.map(|c| as_8bit(c) as u8);
        at.sky_light_color = alpha_blend_rgb(at.sky_light_color, night, as_8bit(a));
        at.sky_color = blend_to_gray(at.sky_color, layer.sky_gray.0, layer.sky_gray.1);
        let darken = layer.darken.map(|c| as_8bit(c) as u8);
        at.fog_color = multiply_rgb(at.fog_color, darken);
        let [sa, sr, sg, sb] = at.sunrise;
        let [r, g, b] = multiply_rgb([sr, sg, sb], darken);
        at.sunrise = [sa, r, g, b];
    }
    at
}

/// `AtmosphericFogEnvironment.applyWeatherDarken`: the sky colour the clear
/// colour is lerped toward, darkened by `(1 − 0.5r, 1 − 0.5r, 1 − 0.4r)` at
/// rain level `r` and by `1 − 0.5t` at thunder level `t`, each an
/// `ARGB.scaleRGB`. **Cited.**
pub fn weather_darken(c: [u8; 3], weather: WorldWeather) -> [u8; 3] {
    let (rain, thunder) = weather_levels(weather);
    let mut c = c.map(i32::from);
    if rain > 0.0 {
        let (a, b) = (1.0 - rain * 0.5, 1.0 - rain * 0.4);
        c = [
            scale_channel(c[0], a),
            scale_channel(c[1], a),
            scale_channel(c[2], b),
        ];
    }
    if thunder > 0.0 {
        let a = 1.0 - thunder * 0.5;
        c = c.map(|v| scale_channel(v, a));
    }
    c.map(|v| v as u8)
}

// ---- the clear sky: the pinned game's day, at the scene's tick ---------------

/// The visible sky colour of the biome a delve stands in, `#7ba4ff`: the
/// `minecraft:visual/sky_color` of the pinned jar's `the_void` (which the
/// delve's own void biome copies, [`crate::compiler::horizon`]) and of
/// `minecraft:ocean` (the ocean horizon's ground) alike. **Cited**
/// (`data/minecraft/worldgen/biome/{the_void,ocean}.json`, server jar
/// `f83b8e09…dd1726`). A place's declared atmosphere (spec-0080) is not read
/// here: a scene is lit as the ground biome is.
pub const GROUND_SKY_RGB: [u8; 3] = [0x7b, 0xa4, 0xff];

/// The overworld's fog colour, `#c0d8ff`: `minecraft:visual/fog_color` of the
/// pinned jar's `data/minecraft/dimension_type/overworld.json`, which neither
/// ground biome overrides. **Cited.**
pub const OVERWORLD_FOG_RGB: [u8; 3] = [0xc0, 0xd8, 0xff];

/// The client's `gamma` option at its default, 0.5 — the lightmap's
/// `BrightnessFactor`. **Cited** (the pinned client's `Options` constructor
/// builds `options.gamma` with the default `0.5d`; `LightTexture` reads it
/// through `Options.gamma()`).
pub const DEFAULT_BRIGHTNESS: f64 = 0.5;

/// The power the pinned core decodes a display value to linear radiance with,
/// and encodes it back with (`Scene.DEFAULT_GAMMA` 2.2, which its textures and
/// its gamma post-process both use). **Cited.**
pub const DISPLAY_GAMMA: f64 = 2.2;

/// `a × b / 255` per 8-bit channel, in integers — the timeline's `multiply`
/// colour modifier (`ColorModifier.MULTIPLY_RGB` → `ARGB.multiply` at the
/// pinned client). **Cited.**
pub(crate) fn multiply_rgb(a: [u8; 3], b: [u8; 3]) -> [u8; 3] {
    std::array::from_fn(|i| ((u32::from(a[i]) * u32::from(b[i])) / 255) as u8)
}

/// A colour track of the vendored day timeline at a tick, as the client reads
/// it: each 8-bit channel interpolated on its own between the two keyframes
/// the tick falls between (wrapping over the day), `a + floor(p × (b − a))` —
/// `LerpFunction.ofColor` → `ARGB.srgbLerp` → `Mth.lerpInt` at the pinned
/// client. **Cited.** The tracks a sky cell reads declare no ease, and
/// this reader refuses one that does.
pub fn day_rgb(track: &str, daytime_ticks: i64) -> [u8; 3] {
    let (a, b, p) = day_segment(track, daytime_ticks);
    let ch = |v: &serde_json::Value| -> [u8; 3] {
        let s = v
            .as_str()
            .and_then(|s| s.strip_prefix('#'))
            .filter(|s| s.len() == 6)
            .unwrap_or_else(|| panic!("`{track}` holds a value that is not `#rrggbb`: {v}"));
        std::array::from_fn(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).expect("hex"))
    };
    let (ca, cb) = (ch(&a), ch(&b));
    std::array::from_fn(|i| {
        let (x, y) = (i32::from(ca[i]), i32::from(cb[i]));
        (x + (p * f64::from(y - x)).floor() as i32) as u8
    })
}

/// An `#aarrggbb` track of the vendored day timeline at a tick, as `[a, r, g,
/// b]`, each 8-bit channel interpolated as [`day_rgb`]'s (`ARGB.srgbLerp`
/// lerps the alpha with the rest). **Cited.**
pub fn day_argb(track: &str, daytime_ticks: i64) -> [u8; 4] {
    let (a, b, p) = day_segment(track, daytime_ticks);
    let ch = |v: &serde_json::Value| -> [u8; 4] {
        let s = v
            .as_str()
            .and_then(|s| s.strip_prefix('#'))
            .filter(|s| s.len() == 8)
            .unwrap_or_else(|| panic!("`{track}` holds a value that is not `#aarrggbb`: {v}"));
        std::array::from_fn(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).expect("hex"))
    };
    let (ca, cb) = (ch(&a), ch(&b));
    std::array::from_fn(|i| {
        let (x, y) = (i32::from(ca[i]), i32::from(cb[i]));
        (x + (p * f64::from(y - x)).floor() as i32) as u8
    })
}

/// A numeric track of the vendored day timeline at a tick, linear between its
/// keyframes (`LerpFunction.ofFloat`). **Cited.**
pub fn day_float(track: &str, daytime_ticks: i64) -> f64 {
    let (a, b, p) = day_segment(track, daytime_ticks);
    let (va, vb) = (
        a.as_f64().expect("a numeric keyframe"),
        b.as_f64().expect("a numeric keyframe"),
    );
    va + (vb - va) * p
}

/// The two keyframes of the linear `minecraft:<track>` the tick falls between,
/// and the progress from the first to the second, wrapping over the day.
fn day_segment(track: &str, daytime_ticks: i64) -> (serde_json::Value, serde_json::Value, f64) {
    use delvewright_dsl::celestial::{DAY_TICKS, day_ease, day_keyframes};
    let id = format!("minecraft:{track}");
    assert!(
        day_ease(&id).is_none(),
        "`{id}` declares an ease, and the sky cell reads linear tracks only"
    );
    let kf = day_keyframes(&id);
    let at = daytime_ticks.rem_euclid(DAY_TICKS);
    let (i, at) = match kf.iter().rposition(|(t, _)| *t <= at) {
        Some(i) => (i, at),
        None => (kf.len() - 1, at + DAY_TICKS),
    };
    let (b, b_ticks) = match kf.get(i + 1) {
        Some((t, v)) => (v.clone(), *t),
        None => (kf[0].1.clone(), kf[0].0 + DAY_TICKS),
    };
    let a_ticks = kf[i].0;
    let p = if b_ticks <= a_ticks {
        0.0
    } else {
        (at - a_ticks) as f64 / (b_ticks - a_ticks) as f64
    };
    (kf[i].1.clone(), b, p)
}

/// **The colour the pinned client's lightmap gives a cell open to the whole
/// sky** (sky light 15, block light 0), as display values in `0..=1` — the
/// light a top face is drawn at. Read off `assets/minecraft/shaders/core/
/// lightmap.fsh` of the pinned client jar with the overworld's uniforms:
/// `SkyFactor` and `SkyLightColor` are the `visual/sky_light_factor` and
/// `visual/sky_light_color` attributes (`LightTexture` reads exactly those two,
/// bytecode through Mojang's mappings), `AmbientLightFactor` the dimension's
/// `ambient_light` 0, darkness, night vision and the boss fog absent,
/// `BrightnessFactor` [`DEFAULT_BRIGHTNESS`]. At level 15 the shader's
/// `get_brightness` is 1, so the sky term is the factor times the colour;
/// then a 4% mix toward 0.75, the clamp, the `notGamma` curve mixed in by the
/// brightness option, and a second 4% mix toward 0.75. **Cited.**
pub fn lightmap_sky(factor: f64, colour: [u8; 3], brightness: f64) -> [f64; 3] {
    let mix75 = |c: f64| c * 0.96 + 0.75 * 0.04;
    let c: [f64; 3] =
        std::array::from_fn(|i| mix75(f64::from(colour[i]) / 255.0 * factor).clamp(0.0, 1.0));
    let max = c[0].max(c[1]).max(c[2]);
    let inv = 1.0 - max;
    let scaled = 1.0 - inv * inv * inv * inv;
    std::array::from_fn(|i| {
        let not_gamma = c[i] * (scaled / max);
        mix75(c[i] + (not_gamma - c[i]) * brightness)
    })
}

/// `x^2.2` for `x` in `0..=1`, by arithmetic alone: `x² · x^(1/5)`, the fifth
/// root by a fixed count of Newton steps, so the value is a function of the
/// IEEE operations and not of a platform's `pow` (ADR-0006).
pub fn to_linear(x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    let mut y = 1.0f64;
    for _ in 0..64 {
        let y4 = y * y * y * y;
        y = (4.0 * y + x / y4) / 5.0;
    }
    x * x * y
}

/// **The sky of the pinned game at a `daytime` tick under a weather**, for
/// every hour and every weather — noon, the dusk and dawn ramps, the night
/// plateau, rain and thunder are points of one function (spec-0079 §4; the
/// departures recorded in `compiler.md` §4).
///
/// The game lights the world from the sky alone: a cell open to the sky is
/// drawn at the lightmap's sky colour ([`lightmap_sky`]), whatever the sun's
/// direction, and no block casts a shadow from it. The sun's direction moves
/// only what is drawn in the sky. The weather moves the attributes the
/// lightmap and the sky are drawn from ([`sky_attributes`], [`WeatherLayer`]),
/// and nothing else a scene can carry. So:
///
/// - **`sky.color` and `skyLight` — the light the sky casts.** Cited: the
///   lightmap colour of `visual/sky_light_factor` and `visual/sky_light_color`
///   at the tick under the weather ([`sky_attributes`]). Authored: the mapping
///   — the colour decoded to linear ([`to_linear`]) is the radiance of a
///   uniform sky that shows a top face at the shader's colour (a Lambertian
///   face under an open uniform sky of radiance `L` returns `albedo × L`),
///   written as its hue (the brightest channel 1) and that channel as
///   `skyLight`.
/// - **The sky the lens sees — the game's own, as a map.** Every scene's sky
///   is `SKYMAP_EQUIRECTANGULAR` over a map drawn for its camera at its tick
///   and weather ([`crate::compiler::view::sky_image`]): `visual/sky_color` in
///   the sky disc with the client's sky fog, the clear colour
///   (`visual/fog_color`, tinted toward `visual/sunrise_sunset_color` when the
///   camera looks to the sun's side, then toward the sky colour the weather
///   darkens, [`weather_darken`]) at and under the horizon, and the sunrise
///   fan blended over both — all cited from the pinned client.
///   `apparentSkyLight` is 1. Authored: the pinned core lights the scene with
///   the same map the lens sees, at `skyLight` instead of `apparentSkyLight`,
///   so the map carries the least uniform veil that makes the light it casts
///   on an open top face this cell's light
///   ([`crate::compiler::view::sky_image::sky_map`]), and the scene's
///   `skyLight` is the multiplier that does it. `sky.color` and this cell's
///   `sky_light` are the target light; `apparent_sky_light` is 1.
/// - **`fog` — none.** The client's sky fog is in the map; the core's fog is
///   written off (`uniformDensity` and `skyFogDensity` 0), its colour the
///   attribute fog colour, which no ray reads.
/// - **The sun — no light, its disc drawn while any of it stands above the
///   horizon under a clear sky.** Cited: nothing in the lightmap depends on
///   the sun's direction; the disc's half-width is
///   [`delvewright_dsl::celestial::SUN_DISC_HALF_CENTIDEG`]; the client draws
///   the sun at the alpha `1 − rain level` (`SkyRenderer.extractRenderState`
///   → `renderSunMoonAndStars`), so under rain or thunder not at all.
///   Authored: the core's sun is a directional light the game does not have,
///   so it gives none; its colour is written white and lights nothing.
/// - **Not expressed:** the stars, the moon and the clouds (the map draws none
///   of them, and the core draws no moon), the rain itself and the rain fog
///   the client draws over the ground (`AtmosphericFogEnvironment.setupFog`
///   pulls the fog's start and end in under rain: the pinned core's fog
///   scatters only the sun's light, and these scenes carry none), and the hue
///   of the light where the veil is zero in a channel — the light is exact in
///   every channel by construction, the lens carries the veil.
///
/// Integer channel arithmetic, float arithmetic where the client's is float,
/// [`to_linear`] and [`round6`] at the writer: deterministic (ADR-0006).
pub fn sky_cell(daytime_ticks: i64, weather: WorldWeather) -> LookCell {
    let t = daytime_ticks;
    let at = sky_attributes(t, weather);
    let lit = lightmap_sky(at.sky_light_factor, at.sky_light_color, DEFAULT_BRIGHTNESS);
    let cast = lit.map(to_linear);
    let peak = cast[0].max(cast[1]).max(cast[2]);
    let hue = cast.map(|c| c / peak);
    let fog = at.fog_color.map(|v| to_linear(f64::from(v) / 255.0));
    let disc = (f64::from(delvewright_dsl::celestial::SUN_DISC_HALF_CENTIDEG) / 100.0).to_radians();
    LookCell {
        sky_color: rgb(hue[0], hue[1], hue[2]),
        sky_light: peak,
        apparent_sky_light: 1.0,
        sun_intensity: 0.0,
        sun_color: rgb(1.0, 1.0, 1.0),
        sun_draw_texture: weather == WorldWeather::Clear && sun_at(t).altitude >= -disc,
        fog_density: 0.0,
        sky_fog_density: 0.0,
        fog_color: rgb(fog[0], fog[1], fog[2]),
    }
}

/// Everything a scene states about its sky: the sun, the sky and the fog. Built
/// by [`sky_of`] alone.
#[derive(Debug, Clone, PartialEq)]
pub struct SkyBlock {
    pub sun: ChunkySun,
    pub sky: ChunkySkyMode,
    pub fog: ChunkyFog,
    /// The tick and weather whose sky map [`ChunkyScene::under`] draws for the
    /// scene's camera.
    pub sky_at: SceneSky,
}

/// **The whole sky of a scene** (spec-0079 §4): the one writer of every sky, sun
/// and fog key under `crate::compiler::view`. Every scene carries the whole
/// block: the hour's sun direction, and the cell of its tick and weather
/// ([`sky_cell`]), its map drawn by [`ChunkyScene::under`].
///
/// The cell's arithmetic and [`round6`] on every float: deterministic
/// (ADR-0006).
pub fn sky_of(sky: SceneSky) -> SkyBlock {
    let mut sun = sun_at(sky.daytime_ticks);
    let cell = sky_cell(sky.daytime_ticks, sky.weather);
    let r = |c: Rgb| rgb(round6(c.red), round6(c.green), round6(c.blue));
    sun.intensity = Some(round6(cell.sun_intensity));
    sun.color = Some(r(cell.sun_color));
    sun.draw_texture = Some(cell.sun_draw_texture);
    SkyBlock {
        sun,
        sky: ChunkySkyMode {
            mode: "SOLID_COLOR",
            color: r(cell.sky_color),
            sky_light: round6(cell.sky_light),
            apparent_sky_light: round6(cell.apparent_sky_light),
            skymap: None,
            sky_mirrored: None,
        },
        fog: ChunkyFog {
            mode: "UNIFORM",
            uniform_density: round6(cell.fog_density),
            color: r(cell.fog_color),
            sky_fog_density: round6(cell.sky_fog_density),
        },
        sky_at: sky,
    }
}

/// The cell a scene sky reads, as one phrase for a binding line, with its
/// judged numbers as the scene writes them.
pub fn sky_phrase(weather: WorldWeather, daytime_ticks: i64) -> String {
    let c = sky_cell(daytime_ticks, weather);
    format!(
        "{} sky at daytime {daytime_ticks}: the game's sky as a map, its light {} \
         (the lightmap's, peak channel), sun {}",
        weather.keyword(),
        round6(c.sky_light),
        round6(c.sun_intensity)
    )
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
                    emittance: review_emittance(),
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
            postprocess: POSTPROCESS,
            postprocess_settings: tone_curve(),
            output_mode: "PNG",
            render_time: 0,
            spp: 0,
            spp_target: opts.spp_target,
            ray_depth: 5,
            path_trace: true,
            dump_frequency: 500,
            save_snapshots: false,
            emitters_enabled: true,
            emitter_intensity: emitter_intensity(),
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
            delvewright_sky: None,
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
        assert_eq!(m["minecraft:stone"]["emittance"], review_emittance());
        assert_eq!(
            m["minecraft:cobblestone_stairs"]["emittance"],
            review_emittance()
        );
        assert_eq!(m["minecraft:tuff"]["emittance"], review_emittance());
        // The emulation's glow is the calibrated one whatever the lantern scale.
        let glow = review_emittance() * v["emitterIntensity"].as_f64().unwrap();
        assert!(
            (glow - REVIEW_EMITTANCE * CORE_EMITTER_INTENSITY).abs() < 1e-3,
            "{glow}"
        );
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

    /// **The pinned track and the published closed form agree** (spec-0081
    /// §2.3) — the vendored `sun_angle` track ([`sky_angle_rad`], which
    /// [`sun_at`] reads) against minecraft.wiki's closed form and the game's
    /// own pre-timeline `DimensionType.timeOfDay`, both written out here. A
    /// measurement that is a deliverable is cross-checked by a second method
    /// sharing no configuration with the first, and the sun's position is
    /// exactly that: it decides what every review frame looks like.
    #[test]
    fn the_two_published_sun_angle_formulas_agree() {
        fn time_of_day_curve(ticks: i64) -> f64 {
            // DimensionType.timeOfDay: d = frac(t/24000 - 0.25);
            // (d*2 + (0.5 - cos(d*pi)/2)) / 3, a full turn.
            let d = (ticks as f64 / 24000.0 - 0.25).rem_euclid(1.0);
            let e = 0.5 - (d * std::f64::consts::PI).cos() / 2.0;
            (d * 2.0 + e) / 3.0 * std::f64::consts::TAU
        }
        let tau = std::f64::consts::TAU;
        let wrap = |x: f64| {
            let d = x.rem_euclid(tau);
            d.min(tau - d)
        };
        // The two closed forms are one curve.
        for t in 0..24_000 {
            let a = closed_form_sky_angle_rad(t);
            let b = time_of_day_curve(t);
            assert!(wrap(a - b) < 1e-9, "tick {t}: wiki {a} vs game {b}");
        }
        // The pinned track against the closed form: every tick within 0.06°,
        // and the worst tick is where the measurement says it is.
        let mut worst = (0.0f64, 0);
        for t in 0..24_000 {
            let d = wrap(sky_angle_rad(t) - closed_form_sky_angle_rad(t)).to_degrees();
            assert!(
                d < 0.06,
                "tick {t}: the track and the closed form differ by {d}°"
            );
            if d > worst.0 {
                worst = (d, t);
            }
        }
        assert!(
            (worst.0 - 0.056).abs() < 0.0005 && (worst.1 - 11457).abs() <= 1,
            "the worst disagreement moved: {worst:?}"
        );
        let hours = [1000, 6000, 12000, 13000, 18000, 23000];
        for t in hours {
            let d = wrap(sky_angle_rad(t) - closed_form_sky_angle_rad(t)).to_degrees();
            assert!(d <= 0.056, "hour {t}: {d}°");
        }
        assert_eq!(hours.len(), 6, "every keyword hour was checked");
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
    /// The curve every scene is written with shows linear [`MID_GREY`] at the
    /// value the game's transfer shows it, where the core's preset shows 0.393;
    /// the gain is the closed form's, and the curve is the preset read brighter.
    #[test]
    fn the_tone_curve_shows_mid_grey_as_the_game_does() {
        let game = MID_GREY.powf(CORE_DISPLAY_EXPONENT);
        let preset = HABLE_FILMIC_WORLDS
            .before_gamma(MID_GREY)
            .powf(CORE_DISPLAY_EXPONENT);
        assert_eq!(round6(preset), 0.393285, "the preset's mid-grey");
        let shown = tone_curve().before_gamma(MID_GREY);
        assert!((shown - MID_GREY).abs() < 1e-6, "mid-grey held at {shown}");
        assert!((shown.powf(CORE_DISPLAY_EXPONENT) - game).abs() < 1e-6);
        let k = HABLE_FILMIC_WORLDS.mid_grey_gain(MID_GREY);
        assert_eq!(round6(k), 1.463761);
        // The brightened curve is the preset at `k` times the light, over the
        // whole range a frame holds, black to past white.
        for i in 0..=400 {
            let x = 1e-4 * 1.04f64.powi(i);
            let (a, b) = (
                tone_curve().before_gamma(x),
                HABLE_FILMIC_WORLDS.before_gamma(k * x),
            );
            assert!(
                (a - b).abs() <= 1e-5 * b.abs().max(1e-3),
                "x {x}: {a} vs {b}"
            );
        }
        // Monotone, and white stays white: the shoulder is the preset's.
        assert!(tone_curve().before_gamma(11.2 / k) > 0.9999);
    }

    /// The anchor is the derivation's input, and it moves the emitted curve: a
    /// different mid-grey writes different `postprocessSettings` bytes.
    #[test]
    fn the_anchor_moves_the_emitted_curve() {
        let emitted = serde_json::to_string(&tone_curve()).unwrap();
        let moved = HABLE_FILMIC_WORLDS.brightened(HABLE_FILMIC_WORLDS.mid_grey_gain(0.2));
        assert_ne!(serde_json::to_string(&moved).unwrap(), emitted);
        assert_ne!(
            serde_json::to_string(&HABLE_FILMIC_WORLDS).unwrap(),
            emitted,
            "the preset itself is not what a scene carries"
        );
    }

    /// The lantern scale is the derivation's input, and it moves the emitted
    /// `emitterIntensity`; the emulation's glow does not move with it.
    #[test]
    fn the_lantern_scale_moves_the_emitted_intensity() {
        assert_eq!(emitter_intensity(), 159.003);
        let scenes = scenes_from_plan(FIXTURE, &SceneOptions::default(), &[]).unwrap();
        for (name, bytes) in &scenes {
            let v: serde_json::Value = serde_json::from_slice(bytes).unwrap();
            assert_eq!(v["emitterIntensity"], emitter_intensity(), "{name}");
        }
        let moved = round6(CORE_EMITTER_INTENSITY * (LANTERN_LIGHT_SCALE * 1.01));
        assert_ne!(moved, emitter_intensity());
        assert!((review_emittance() * emitter_intensity() - REVIEW_EMITTANCE * 13.0).abs() < 1e-3);
    }

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

    /// The eleven keys a sky block writes beyond the sun's direction, as
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

    /// **The weather is the client's layer** (`WeatherAttributes`), value by
    /// value at noon, where every attribute it touches is lit: the sky light's
    /// factor and colour `alpha_blend`ed toward 0.24 and `#7a7aff` at 0.3125
    /// (rain) and 0.52734375 (thunder), the sky colour `blend_to_gray`ed — the
    /// void and ocean biomes' `#7ba4ff` to `#667187` under rain and
    /// `#2b2d33` under thunder, worked by hand from `ARGB.greyscale`,
    /// `scaleRGB` and `srgbLerp` — and the fog and sunrise colours multiplied.
    /// Thunder is darker than rain and rain than clear wherever the sky is lit;
    /// on the night plateau the layer blends toward the night's own light, so
    /// the light is the clear night's; and under weather the sun's disc is never
    /// drawn.
    #[test]
    fn the_weather_is_the_clients_layer() {
        let clear = sky_attributes(6000, WorldWeather::Clear);
        let rain = sky_attributes(6000, WorldWeather::Rain);
        let thunder = sky_attributes(6000, WorldWeather::Thunder);
        assert_eq!(
            (clear.sky_light_factor, clear.sky_light_color),
            (1.0, [255; 3])
        );
        assert_eq!(
            rain.sky_light_factor,
            f64::from(1.0f32 + 0.3125f32 * (0.24f32 - 1.0))
        );
        assert_eq!(round6(rain.sky_light_factor), 0.7625);
        assert_eq!(rain.sky_light_color, [213, 213, 255]);
        assert_eq!(round6(thunder.sky_light_factor), 0.599219);
        assert_eq!(thunder.sky_light_color, [185, 185, 255]);
        assert_eq!(clear.sky_color, GROUND_SKY_RGB);
        assert_eq!(rain.sky_color, [102, 113, 135]);
        assert_eq!(thunder.sky_color, [43, 45, 51]);
        assert_eq!(
            rain.fog_color,
            multiply_rgb(clear.fog_color, [127, 127, 153])
        );
        assert_eq!(
            thunder.fog_color,
            multiply_rgb(clear.fog_color, [63, 63, 76])
        );
        let light = |t: i64, w: WorldWeather| sky_cell(t, w).sky_light;
        for t in [6000, 12782] {
            assert!(
                light(t, WorldWeather::Thunder) < light(t, WorldWeather::Rain),
                "{t}"
            );
            assert!(
                light(t, WorldWeather::Rain) < light(t, WorldWeather::Clear),
                "{t}"
            );
        }
        for w in WEATHERS {
            // Equal to the float the client blends in (the factor 0.24 read as a float).
            let (n, c) = (
                sky_cell(18000, w).sky_light,
                sky_cell(18000, WorldWeather::Clear).sky_light,
            );
            assert!((n - c).abs() < 1e-6, "{w:?}: {n} vs {c}");
            for t in (0..24000).step_by(500) {
                let c = sky_cell(t, w);
                if w != WorldWeather::Clear {
                    assert!(!c.sun_draw_texture, "{t} {w:?}");
                }
                assert_eq!(
                    (c.sun_intensity, c.fog_density, c.sky_fog_density),
                    (0.0, 0.0, 0.0)
                );
            }
        }
        eprintln!(
            "weather: noon light clear {} rain {} thunder {}; setting sun clear {} rain {} thunder {}",
            round6(light(6000, WorldWeather::Clear)),
            round6(light(6000, WorldWeather::Rain)),
            round6(light(6000, WorldWeather::Thunder)),
            round6(light(12782, WorldWeather::Clear)),
            round6(light(12782, WorldWeather::Rain)),
            round6(light(12782, WorldWeather::Thunder)),
        );
    }

    /// The weather's layer is the derivation's input: a different alpha moves
    /// the emitted light, and a different `blend_to_gray` the emitted map.
    #[test]
    fn the_weather_layer_moves_the_emitted_sky() {
        let plan = sky_plan(delvewright_dsl::WorldTime::Noon, WorldWeather::Rain);
        let scenes = scenes_from_plan(&plan, &SceneOptions::default(), &[]).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&scenes[0].1).unwrap();
        let o = &v["camera"]["orientation"];
        let view = crate::compiler::view::sky_image::view_of(
            o["yaw"].as_f64().unwrap(),
            o["pitch"].as_f64().unwrap(),
        );
        let emitted = crate::compiler::view::sky_image::sky_map(6000, WorldWeather::Rain, view);
        assert_eq!(v["sky"]["skyLight"], round6(emitted.sky_light));
        let f = 1.0f32;
        let moved = f + (RAIN_LAYER.alpha * 1.01) * (0.24 - f);
        assert_ne!(
            round6(f64::from(moved)),
            round6(sky_attributes(6000, WorldWeather::Rain).sky_light_factor)
        );
        let grey = blend_to_gray(
            GROUND_SKY_RGB,
            RAIN_LAYER.sky_gray.0 * 1.1,
            RAIN_LAYER.sky_gray.1,
        );
        assert_ne!(grey, sky_attributes(6000, WorldWeather::Rain).sky_color);
    }

    /// **The clear sky is the pinned timeline at its tick** — read back value
    /// by value from the vendored day timeline's keyframes, not from the
    /// cell's own arithmetic: on the night plateau the lens sees no sky (the
    /// visible sky's multiplier is black from 13670 to 22330) and the sky casts
    /// the lightmap colour of factor 0.24 and `#7a7aff`; at noon the cast light
    /// is the lightmap's white; and at the setting sun (tick 12782, the
    /// Treehouse Camp's hour) every written number is the one
    /// `tools/ci/check-whole-map-render.py` derives with its own reader of the
    /// same file. The sun gives no light at any hour, and its disc is drawn
    /// exactly while some of it stands above the horizon.
    #[test]
    fn the_clear_sky_is_the_timeline_at_its_tick() {
        use delvewright_dsl::celestial::day_keyframes;
        let at = |track: &str, tick: i64| -> serde_json::Value {
            day_keyframes(&format!("minecraft:visual/{track}"))
                .into_iter()
                .find(|(t, _)| *t == tick)
                .unwrap_or_else(|| panic!("{track} has a keyframe at {tick}"))
                .1
        };
        let hex = |v: serde_json::Value| -> [u8; 3] {
            let s = v.as_str().unwrap().trim_start_matches('#').to_string();
            std::array::from_fn(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        };
        // The night plateau: black sky, factor 0.24, `#7a7aff`.
        assert_eq!(hex(at("sky_color", 13670)), [0, 0, 0]);
        let night_factor = at("sky_light_factor", 13140).as_f64().unwrap();
        let night_colour = hex(at("sky_light_color", 13140));
        assert_eq!((night_factor, night_colour), (0.24, [0x7a, 0x7a, 0xff]));
        let lit = lightmap_sky(night_factor, night_colour, DEFAULT_BRIGHTNESS);
        for t in [13670, 18000, 22330] {
            let c = sky_cell(t, WorldWeather::Clear);
            assert_eq!(
                c.apparent_sky_light, 1.0,
                "{t}: the lens sees the map whole"
            );
            assert_eq!(c.sky_light, to_linear(lit[2]), "{t}");
            assert_eq!(
                c.sky_color.red,
                to_linear(lit[0]) / to_linear(lit[2]),
                "{t}"
            );
            assert!(!c.sun_draw_texture, "{t}");
        }
        // Noon: the lightmap's white, the biome's blue seen at its luminance.
        let noon = sky_cell(6000, WorldWeather::Clear);
        let white = lightmap_sky(1.0, [255; 3], DEFAULT_BRIGHTNESS);
        assert_eq!(noon.sky_light, to_linear(white[0]));
        assert_eq!(noon.sky_color, rgb(1.0, 1.0, 1.0));
        assert!(noon.sun_draw_texture);
        // The setting sun, as the scene writes it (`round6`), against the
        // checker's own derivation.
        let set = sky_cell(12782, WorldWeather::Clear);
        let w = |v: f64| round6(v);
        assert_eq!(w(set.sky_light), 0.374446);
        assert_eq!(
            (
                w(set.sky_color.red),
                w(set.sky_color.green),
                w(set.sky_color.blue)
            ),
            (0.357581, 0.357581, 1.0)
        );
        assert_eq!(
            (
                w(set.fog_color.red),
                w(set.fog_color.green),
                w(set.fog_color.blue)
            ),
            (0.12753, 0.163641, 0.25084)
        );
        assert!(set.sun_draw_texture, "the setting sun's disc is half up");
        for t in (0..24000).step_by(250) {
            let c = sky_cell(t, WorldWeather::Clear);
            assert_eq!(c.sun_intensity, 0.0, "{t}");
            assert_eq!((c.sky_fog_density, c.fog_density), (0.0, 0.0));
            let disc = f64::from(delvewright_dsl::celestial::SUN_DISC_HALF_CENTIDEG) / 100.0;
            let alt = sun_at(t).altitude.to_degrees();
            assert_eq!(
                c.sun_draw_texture,
                alt >= -disc - 1e-9,
                "{t}: altitude {alt}"
            );
        }
    }

    /// **One rule for the whole day**: under each weather the cell moves
    /// smoothly from tick to tick over the whole day — no rule switches at a
    /// plateau's edge, so the night is a point on the same curve as noon and the
    /// dusk ramp between. Measured as the largest change of any written number
    /// between two adjacent ticks, over all 24000.
    #[test]
    fn the_sky_cell_has_no_seam_over_the_day() {
        for w in WEATHERS {
            let mut worst = [0.0f64; 3];
            let mut prev = sky_cell(23999, w);
            for t in 0..24000 {
                let c = sky_cell(t, w);
                for (k, (x, y)) in [
                    (c.sky_light, prev.sky_light),
                    (c.apparent_sky_light, prev.apparent_sky_light),
                    (c.sky_color.red, prev.sky_color.red),
                ]
                .into_iter()
                .enumerate()
                {
                    worst[k] = worst[k].max((x - y).abs());
                }
                prev = c;
            }
            eprintln!("{w:?} sky cell: largest step between adjacent ticks {worst:?}");
            assert!(worst.iter().all(|w| *w < 0.01), "{w:?} {worst:?}");
        }
    }

    /// The lightmap port and the arithmetic power are the shader and `x^2.2`:
    /// the shader's full-sky white under the default brightness is 0.9852, and
    /// [`to_linear`] agrees with the platform's `powf` to 1e-12 over `0..=1`.
    #[test]
    fn the_lightmap_and_the_power_are_the_shader_and_the_curve() {
        let white = lightmap_sky(1.0, [255; 3], DEFAULT_BRIGHTNESS);
        // 1 → 0.99 (4% toward 0.75), notGamma of it, half of the way there, 4% toward 0.75 again.
        let c = 0.99f64;
        let ng = c * ((1.0 - (1.0 - c).powi(4)) / c);
        let want = (c + (ng - c) * 0.5) * 0.96 + 0.03;
        for ch in white {
            assert!((ch - want).abs() < 1e-15, "{ch} vs {want}");
        }
        let mut worst = 0.0f64;
        for i in 0..=1000 {
            let x = f64::from(i) / 1000.0;
            worst = worst.max((to_linear(x) - x.powf(DISPLAY_GAMMA)).abs());
        }
        assert!(worst < 1e-12, "{worst}");
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
    /// showcase camera): every scene carries all eleven block keys with its
    /// cell's values — the game's sky of its tick and weather as a map, the
    /// light the map casts the lightmap's, no sun light, no fog, the sun's disc
    /// only under a clear sky.
    #[test]
    fn every_scene_builder_writes_the_whole_block() {
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
                        after: None,
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
                let cam = camera::emit(
                    &plan,
                    &sheet,
                    &rows,
                    &camera::EmitOptions {
                        world_paths: [("x".to_string(), "/abs/world".to_string())].into(),
                        ..camera::EmitOptions::default()
                    },
                )
                .unwrap();
                for (kind, bytes) in [
                    ("review", &review[0].1),
                    ("panorama", &pano.bytes),
                    ("camera", &cam.scenes[0].1),
                ] {
                    let v: serde_json::Value = serde_json::from_slice(bytes).unwrap();
                    let at = format!("{kind} {}+{}", time.keyword(), weather.keyword());
                    let cell = sky_cell(time.daytime_ticks(), weather);
                    for k in BLOCK_KEYS {
                        assert!(v.pointer(k).is_some(), "{at}: missing {k}");
                    }
                    assert_eq!(v["fog"]["mode"], "UNIFORM", "{at}");
                    assert_eq!(v["sun"]["drawTexture"], cell.sun_draw_texture, "{at}");
                    assert_eq!(v["fog"]["skyFogDensity"], 0.0, "{at}");
                    assert_eq!(
                        v["sky"]["apparentSkyLight"],
                        round6(cell.apparent_sky_light),
                        "{at}"
                    );
                    // The game's sky as a map, written beside the scene; its
                    // multiplier is the one the map answers.
                    assert_eq!(v["sky"]["mode"], "SKYMAP_EQUIRECTANGULAR", "{at}");
                    assert_eq!(v["sky"]["skyMirrored"], false, "{at}");
                    let name = format!("{}{SKY_MAP_SUFFIX}", v["name"].as_str().unwrap());
                    assert_eq!(v["sky"]["skymap"], name.as_str(), "{at}");
                    assert_eq!(
                        v["delvewrightSky"]["daytimeTicks"],
                        time.daytime_ticks(),
                        "{at}"
                    );
                    if weather == WorldWeather::Clear {
                        assert!(v["delvewrightSky"].get("weather").is_none(), "{at}");
                    } else {
                        assert_eq!(v["delvewrightSky"]["weather"], weather.keyword(), "{at}");
                    }
                    let (map, bytes) =
                        crate::compiler::view::sky_image::companion(bytes).expect("a map");
                    assert_eq!(map, name, "{at}");
                    assert!(bytes.starts_with(b"PF\n360 180\n-1.0\n"), "{at}");
                    let o = &v["camera"]["orientation"];
                    let view = crate::compiler::view::sky_image::view_of(
                        o["yaw"].as_f64().unwrap(),
                        o["pitch"].as_f64().unwrap(),
                    );
                    let sky = crate::compiler::view::sky_image::sky_map(
                        time.daytime_ticks(),
                        weather,
                        view,
                    );
                    assert_eq!(v["sky"]["skyLight"], round6(sky.sky_light), "{at}");
                    assert_eq!(
                        bytes,
                        crate::compiler::view::sky_image::pfm_bytes(&sky),
                        "{at}: the companion is the scene's own sky"
                    );
                    assert_eq!(v["sun"]["intensity"], cell.sun_intensity, "{at}");
                    assert_eq!(v["fog"]["uniformDensity"], cell.fog_density, "{at}");
                    assert_eq!(v["postprocess"], POSTPROCESS, "{at}");
                    assert_eq!(
                        v["postprocessSettings"],
                        serde_json::from_str::<serde_json::Value>(
                            &serde_json::to_string(&tone_curve()).unwrap()
                        )
                        .unwrap(),
                        "{at}"
                    );
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

    /// A rain or thunder scene is byte-deterministic: two emissions, one set of bytes.
    #[test]
    fn a_weather_scene_is_byte_deterministic() {
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

    /// **A clear scene's bytes are recorded** (spec-0079 §4.1, criterion 4, as
    /// `compiler.md` §4 records it since the clear cell): the two goldens are
    /// this emitter's output for this record, plan and options — a showcase
    /// camera answering a clear row at the plan's own hour, and the default
    /// panorama of the same clear plan — compared byte for byte, so a change
    /// to a clear scene's bytes is a change to these files in the same commit.
    /// The review scene's golden is `golden_scene_matches`'s `spawn.json`.
    #[test]
    fn clear_scenes_match_their_goldens() {
        use crate::compiler::view::{camera, panorama};
        use delvewright_dsl::WorldTime;
        let ocean = include_bytes!("../../../tests/fixtures/view/render-plan-ocean.json");
        let sheet = camera::CameraSheet {
            campaign_id: "isle".into(),
            cameras: vec![camera::Camera {
                after: None,
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
                world_paths: sheet
                    .cameras
                    .iter()
                    .map(|c| (c.name.clone(), "/abs/world".to_string()))
                    .collect(),
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
            "a clear camera scene moved from its golden"
        );
        let p = panorama::panorama_from_plan(ocean, &[], &panorama::PanoramaOptions::default())
            .unwrap();
        assert_eq!(
            std::str::from_utf8(&p.bytes).unwrap(),
            std::str::from_utf8(include_bytes!(
                "../../../tests/golden/view/clear-panorama.json"
            ))
            .unwrap(),
            "a clear panorama scene moved from its golden"
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
            "view/clear-camera.json",   // pinned by clear_scenes_match_their_goldens
            "view/clear-panorama.json", // pinned by clear_scenes_match_their_goldens
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
