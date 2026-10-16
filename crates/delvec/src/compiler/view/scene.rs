//! **The sky a frame is taken under, and the plan a frame is read from**
//! (spec-0079, spec-0112 §6.1 item 6).
//!
//! ## A sky is the pinned game's, at its tick under its weather ([`sky_cell`])
//!
//! A frame is lit as the pinned game lights its tick under its weather: the
//! light the sky casts is the client lightmap's sky colour, the lens sees the
//! timeline's sky colour and its fog at the horizon, the weather moves both
//! through the client's own weather layer ([`WeatherLayer`]), and the sun gives
//! no light in the game's own lightmap, because no light term in the game
//! depends on the sun's direction. Noon, the dusk and dawn ramps, the night,
//! rain and thunder are points of one function of the tick and the weather.
//! [`crate::compiler::view::frames`] hands these numbers to the renderer; the
//! renderer's own derivation of a sky from a tick is never what a frame shows.
//!
//! ## The sun is the hour's ([`sun_at`])
//!
//! The sun's direction is the pinned day timeline's `visual/sun_angle` track at
//! the frame's tick.
//!
//! ## `render-plan.json`
//!
//! [`parse_plan`] is the one reader of the plan's facts a frame needs beyond
//! its shots: the campaign, the layout box, the horizon and the declared sky.

use serde::{Deserialize, Serialize};

use delvewright_dsl::WorldWeather;

use crate::compiler::view::diag::{DW_INPUT, Diagnostic};

// ---- render-plan.json (input) -------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct RenderPlan {
    pub campaign_id: String,
    pub layout_aabb: Aabb,
    /// The world-generator horizon the compiler declared (spec-0013). Absent =
    /// `void`: no ambient sea, nothing to add under the frame.
    #[serde(default)]
    pub horizon: Option<Horizon>,
    /// The hour the campaign declared. Optional **in the document type only**, so
    /// a plan that omits it can be refused by name ([`plan_sky`]) instead of by
    /// a serde message about a missing field — the fact a creator needs is which
    /// engine wrote the plan, not which key is absent.
    #[serde(default)]
    pub sky: Option<Sky>,
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
/// that BUILT ground has to say how far the ground reaches, and a frame loads
/// the union of that and the layout.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Aabb {
    pub min: [i32; 3],
    pub max: [i32; 3],
}

/// The `horizon` fact `render-plan.json` carries (compiler `render_plan::
/// horizon_fact`); `void` is the absent case.
/// **Every base the compiler can state, and the enumeration is the point.**
/// `#[serde(tag = "kind")]` has no fallback, so one unknown value fails the
/// whole document, so a base the compiler states and this reader does not
/// know refuses every plan of it by name; `tools/ci/check-whole-map-render.py`
/// renders the panorama on every base the schema declares.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Horizon {
    /// A superflat sea. `sea_level` is the Y of the topmost generator water
    /// block (the compiler's `plan::SEA_LEVEL`).
    Ocean { sea_level: i32 },
    /// A landform the compiler BUILT and the world save therefore contains.
    /// Nothing ambient to add — but the ground is real geometry outside the
    /// layout, so [`Horizon::extent`] is what keeps it in the loaded chunks. It is
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
    /// The world AABB of ground this horizon BUILT beyond the layout, if any.
    /// An ocean's sea is the level generator's, written by the world writer
    /// into every chunk of the extent, so it widens nothing; a valley's
    /// landform is blocks outside the layout, and a frame that leaves it out
    /// shows a delve standing on nothing.
    pub fn extent(self) -> Option<([i32; 3], [i32; 3])> {
        match self {
            Horizon::Ocean { .. } => None,
            Horizon::Valley { extent, .. } => Some((extent.min, extent.max)),
        }
    }
}

/// The union of a layout AABB with whatever ground the horizon built under it —
/// what a frame loads, and every chunk of which the world writer fills
/// (spec-0112 §5.3). One function because every frame owes the same answer;
/// the camera is solved from the layout alone
/// (`panorama`), so the ground is in the picture as the place's setting without
/// being what the frame is fitted to.
pub fn loaded_extent(layout: &Aabb, horizon: Option<Horizon>) -> ([i32; 3], [i32; 3]) {
    let (mut min, mut max) = (layout.min, layout.max);
    if let Some((hmin, hmax)) = horizon.and_then(Horizon::extent) {
        for a in 0..3 {
            min[a] = min[a].min(hmin[a]);
            max[a] = max[a].max(hmax[a]);
        }
    }
    (min, max)
}

/// The inclusive chunk rectangle `[min_x, min_z]..=[max_x, max_z]` an inclusive
/// block box stands in: what a frame of it loads.
pub fn chunk_rect(min: [i32; 3], max: [i32; 3]) -> ([i32; 2], [i32; 2]) {
    (
        [min[0].div_euclid(16), min[2].div_euclid(16)],
        [max[0].div_euclid(16), max[2].div_euclid(16)],
    )
}

/// Parse a `render-plan.json`'s whole-plan facts (the frames, the panorama
/// and the showcase cameras all read them here).
pub fn parse_plan(plan_json: &[u8]) -> Result<RenderPlan, Diagnostic> {
    serde_json::from_slice(plan_json)
        .map_err(|e| Diagnostic::error(DW_INPUT, format!("parse render-plan.json: {e}")))
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

/// Whether a stamp names a room the player sees under night vision: declared
/// dark **and** declared night-vision. A frame of it is drawn at the lightmap
/// without the effect, and its index entry says so under `unchecked`
/// (spec-0112 §11 item 5). A `lit` profile has real fixtures, and an absent
/// stamp is an undeclared area: neither is.
pub fn needs_emulation(stamp: Option<&LightingStamp>) -> bool {
    stamp.is_some_and(|l| l.profile == "dark" && l.mitigation.as_deref() == Some("night-vision"))
}

/// Round to 6 decimals, never emitting `-0.0` (it serializes differently from
/// `0.0`), so libm ulp differences between platforms cannot move a written
/// number (ADR-0006).
pub(crate) fn round6(v: f64) -> f64 {
    let r = (v * 1e6).round() / 1e6;
    if r == 0.0 { 0.0 } else { r }
}

/// Where the sun stands: `altitude` above the horizon and `azimuth` from +X
/// (east) toward +Z, both radians, so the direction toward it is
/// `(cos az · cos alt, sin alt, sin az · cos alt)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SunAt {
    pub altitude: f64,
    pub azimuth: f64,
}

/// **The sun of a Minecraft `daytime` tick value.**
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
/// A sun below the horizon is a negative altitude. The azimuth is exactly
/// east or exactly west; at noon and midnight the sun is at the zenith or the
/// nadir and the azimuth means nothing, so east is stated and the value is
/// still a function of the hour alone.
pub fn sun_at(daytime_ticks: i64) -> SunAt {
    let t = daytime_ticks as f64;
    let altitude = delvewright_dsl::celestial::sun_altitude_degrees(t).to_radians();
    let azimuth = if delvewright_dsl::celestial::sun_is_west(t) {
        std::f64::consts::PI
    } else {
        0.0
    };
    SunAt {
        altitude: round6(altitude),
        azimuth: round6(azimuth),
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

/// The sky a frame is taken under: the hour as the `daytime` tick value the
/// sun is a function of, and the weather. Built from `render-plan.json` by
/// [`plan_sky`] (the panorama) or from a showcase camera
/// (`crate::compiler::view::camera`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneSky {
    pub daytime_ticks: i64,
    pub weather: WorldWeather,
}

/// The hour and weather a plan states, or [`DW_INPUT`] naming what is missing.
///
/// **A frame is never drawn without both**: a plan with no `sky` would leave
/// the hour to a default, and one with no `sky.weather` the weather. The
/// refusal is here, at the one door every plan-sky frame goes through, rather
/// than in the type, so it can say what to do about it.
pub fn plan_sky(plan: &RenderPlan) -> Result<SceneSky, Diagnostic> {
    let sky = plan.sky.as_ref().ok_or_else(|| {
        Diagnostic::error(
            DW_INPUT,
            format!(
                "render-plan.json states no `sky`, so there is no hour to put a sun at, and a \
                 frame of it would be of no hour `world.json` declares. The plan was written by an \
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
                 thunder delve would be drawn under a clear sky. The plan was \
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

// ---- the sky a frame is taken under ------------------------------------------

/// A colour as three channels in `0..=1`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Rgb {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
}

const fn rgb(red: f64, green: f64, blue: f64) -> Rgb {
    Rgb { red, green, blue }
}

/// The sky of one tick under one weather, as a frame states it ([`sky_cell`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LookCell {
    /// The hue of the light the sky casts, its brightest channel 1.
    pub sky_color: Rgb,
    /// The brightest channel of the light the sky casts, linear.
    pub sky_light: f64,
    /// Whether the client draws the sun's disc.
    pub sun_draw_texture: bool,
    /// The fog colour, linear.
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
/// weather's layer applied ([`WeatherLayer`]). The one place a frame's sky
/// reads the game's numbers: the light ([`sky_cell`]) and the colours quick
/// mode draws ([`crate::compiler::view::frames::quick_sky`]) both take them from
/// here.
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
/// plateau, rain and thunder are points of one function (spec-0079 §4).
///
/// The game lights the world from the sky alone: a cell open to the sky is
/// drawn at the lightmap's sky colour ([`lightmap_sky`]), whatever the sun's
/// direction, and no block casts a shadow from it. The sun's direction moves
/// only what is drawn in the sky. The weather moves the attributes the
/// lightmap and the sky are drawn from ([`sky_attributes`], [`WeatherLayer`]).
/// So:
///
/// - **`sky_color` and `sky_light` — the light the sky casts.** Cited: the
///   lightmap colour of `visual/sky_light_factor` and `visual/sky_light_color`
///   at the tick under the weather ([`sky_attributes`]). Authored: the mapping
///   — the colour decoded to linear ([`to_linear`]) is the radiance of a
///   uniform sky that shows a top face at the shader's colour, written as its
///   hue (the brightest channel 1) and that channel.
/// - **`fog_color`** — the attribute fog colour, linear.
/// - **The sun's disc** is drawn while any of it stands above the horizon
///   under a clear sky. Cited: the disc's half-width is
///   [`delvewright_dsl::celestial::SUN_DISC_HALF_CENTIDEG`]; the client draws
///   the sun at the alpha `1 − rain level` (`SkyRenderer.extractRenderState`
///   → `renderSunMoonAndStars`), so under rain or thunder not at all.
///
/// Integer channel arithmetic, float arithmetic where the client's is float:
/// deterministic (ADR-0006).
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
        sun_draw_texture: weather == WorldWeather::Clear && sun_at(t).altitude >= -disc,
        fog_color: rgb(fog[0], fog[1], fog[2]),
    }
}

/// The cell a frame's sky reads, as one phrase for a binding line.
pub fn sky_phrase(weather: WorldWeather, daytime_ticks: i64) -> String {
    let c = sky_cell(daytime_ticks, weather);
    format!(
        "{} sky at daytime {daytime_ticks}: the client's sky colours, its light {} (the \
         lightmap's, peak channel), sun {}",
        weather.keyword(),
        round6(c.sky_light),
        if c.sun_draw_texture {
            "drawn"
        } else {
            "not drawn"
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let deg = |s: &SunAt| s.altitude.to_degrees();
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

    const WEATHERS: [WorldWeather; 3] = [
        WorldWeather::Clear,
        WorldWeather::Rain,
        WorldWeather::Thunder,
    ];

    /// **The weather is the client's layer** (`WeatherAttributes`), value by
    /// value at noon, where every attribute it touches is lit: the sky light's
    /// factor and colour `alpha_blend`ed toward 0.24 and `#7a7aff` at 0.3125
    /// (rain) and 0.52734375 (thunder), the sky colour `blend_to_gray`ed — the
    /// void and ocean biomes' `#7ba4ff` to (102, 113, 135) under rain and
    /// (43, 45, 51) under thunder, worked by hand from `ARGB.greyscale`,
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
            let mut worst = [0.0f64; 2];
            let mut prev = sky_cell(23999, w);
            for t in 0..24000 {
                let c = sky_cell(t, w);
                for (k, (x, y)) in [
                    (c.sky_light, prev.sky_light),
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
}
