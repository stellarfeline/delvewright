//! **The pinned game's clear sky as an image** (`compiler.md` §4, *The sky of a
//! scene*): what the client draws in every direction at a tick, for one camera,
//! written as the equirectangular sky map a clear scene's lens sees.
//!
//! The pinned core's solid and gradient skies are the same in every compass
//! direction (`Sky.getSkyDiffuseColorInner`: `SOLID_COLOR` returns one colour,
//! `GRADIENT` reads only `asin(ray.d.y)`), so the band of colour the game draws
//! at the horizon on the sun's side cannot be one of them. Its image skies can
//! (`SKYMAP_EQUIRECTANGULAR` reads `atan2(z, x)` and `asin(y)`), and a floating
//! point `.pfm` map is read as linear radiance with no gamma step
//! (`PFMTexture`, `AbstractHdriTexture`). But every sky mode feeds ONE colour
//! to the lens and to the light the sky casts — `getSkyColorInterpolated`
//! (a camera ray) and `getSkyColorDiffuseSun` (a diffuse bounce) both read
//! `getSkyDiffuseColorInner` and differ only by the scalar they multiply it by,
//! `apparentSkyLight` or `skyLight` (`PathTracer.pathTrace`, the three sky
//! branches). So the map is the game's sky **plus the least uniform veil** that
//! makes the light it casts on an open top face the lightmap's
//! ([`clear_sky`]).
//!
//! ## What the client draws, read off its bytecode
//!
//! Every number below is the pinned 1.21.11 client's, read through Mojang's
//! client mappings (`SkyRenderer`, `FogRenderer`, `AtmosphericFogEnvironment`,
//! `RenderPipelines`, `Options`) and its shaders (`core/sky.fsh`,
//! `include/fog.glsl`). **Cited**, unless marked otherwise:
//!
//! 1. **The clear colour** (`AtmosphericFogEnvironment.getBaseColor`): the
//!    `visual/fog_color`; when the render distance is at least 4 chunks and the
//!    camera's look vector has a positive dot with `(∓1, 0, 0)` — toward the
//!    side the sun is on, `−X` while `sin(sun_angle) > 0` — it is lerped toward
//!    the opaque `visual/sunrise_sunset_color` by that dot times the colour's
//!    alpha; then it is lerped toward `visual/sky_color` by
//!    `1 − clamp(min(sky_fog_end / 16, render distance) / 32, 0.25, 1)^0.25`.
//!    Lerps are `ARGB.srgbLerp`, per 8-bit channel.
//! 2. **The sky disc** (`buildSkyDisc(16)`): an octagon at height 16, its rim
//!    vertices at 512 blocks every 45° from −180°, drawn in `visual/sky_color`
//!    with the sky fog of `sky.fsh`: the fog colour above at
//!    `max(spherical / sky_end, cylindrical ≥ sky_end ? 1 : 0)`, the two
//!    distances interpolated between the centre vertex and the rim, and
//!    `sky_end = min(render distance × 16, visual/sky_fog_end_distance)`.
//!    Past the octagon's edge, and under the horizon, the clear colour shows.
//! 3. **The sunrise fan** (`buildSunriseFan`, `renderSunriseAndSunset`): a
//!    triangle fan from `(0, 100, 0)` (white, alpha 1) to 17 rim vertices
//!    `(sin a · 120, cos a · 120, −cos a · 40)` (white, alpha 0), `a = 2πj/16`,
//!    turned by `XP 90°` then `ZP (90° or 270°)`, its depth scaled by the
//!    colour's alpha, tinted by `visual/sunrise_sunset_color` and blended
//!    `TRANSLUCENT` (`RenderPipelines.SUNRISE_SUNSET`); skipped below alpha
//!    0.001. Its centre stands on the horizon on the sun's side.
//! 4. The client's defaults: render distance 12 chunks (`options.renderDistance`)
//!    and `visual/sky_fog_end_distance` 512.
//!
//! **Not drawn:** the sun's disc (the core draws its own), the moon and the
//! stars, the clouds, and the dark disc under the horizon (drawn only for a
//! camera below the world's horizon height).
//!
//! ## Determinism (ADR-0006)
//!
//! The map is a function of the tick and the camera's stored orientation alone.
//! Every sine and cosine is [`det_sin`] — a Taylor series in IEEE arithmetic,
//! never a platform `sin` — and the file is raw little-endian `f32`, so its
//! bytes are a function of the inputs on every machine.

use super::scene::{
    DEFAULT_BRIGHTNESS, GROUND_SKY_RGB, OVERWORLD_FOG_RGB, day_argb, day_float, day_rgb,
    lightmap_sky, multiply_rgb, to_linear,
};

/// The map's width and height in pixels: one degree each way.
pub const WIDTH: usize = 360;
/// See [`WIDTH`].
pub const HEIGHT: usize = 180;

/// The client's default render distance, in chunks (`options.renderDistance`).
pub const RENDER_DISTANCE: f64 = 12.0;
/// `visual/sky_fog_end_distance` at its registry default.
pub const SKY_FOG_END_DISTANCE: f64 = 512.0;
/// The sky disc's height and its rim's distance (`SkyRenderer.buildSkyDisc`).
const DISC_HEIGHT: f64 = 16.0;
const DISC_RADIUS: f64 = 512.0;
/// The fan's centre height, rim radius and rim lift (`buildSunriseFan`).
const FAN_CENTRE: f64 = 100.0;
const FAN_RIM: f64 = 120.0;
const FAN_LIFT: f64 = 40.0;
const FAN_STEPS: usize = 16;

/// The largest `skyLight` the pinned core states (`Sky.MAX_INTENSITY`, 50). A
/// sky the lens sees as black (the night) needs no veil but a positive light,
/// and the light is reached by the multiplier up to this bound. **Cited** (the
/// bound); **authored** (using it as the cap).
pub const MAX_SKY_LIGHT: f64 = 50.0;

const PI: f64 = std::f64::consts::PI;

/// `sin x` by a Taylor series after reducing `x` into `[−π, π]` by repeated
/// subtraction — arithmetic only, so the value does not depend on a platform's
/// `sin`. Accurate to about 1e-15 on the reduced range.
pub fn det_sin(x: f64) -> f64 {
    let tau = 2.0 * PI;
    let mut r = x;
    while r > PI {
        r -= tau;
    }
    while r < -PI {
        r += tau;
    }
    let r2 = r * r;
    let mut term = r;
    let mut sum = r;
    for k in 1..=17 {
        let n = (2 * k) as f64;
        term *= -r2 / (n * (n + 1.0));
        sum += term;
    }
    sum
}

/// `cos x` as [`det_sin`]`(x + π/2)`.
pub fn det_cos(x: f64) -> f64 {
    det_sin(x + PI / 2.0)
}

fn lerp_int(f: f64, a: i32, b: i32) -> i32 {
    a + (f * f64::from(b - a)).floor() as i32
}

fn srgb_lerp(f: f64, a: [i32; 3], b: [i32; 3]) -> [i32; 3] {
    std::array::from_fn(|i| lerp_int(f, a[i], b[i]))
}

/// The tint the fan carries, and its alpha in `0..=1`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fan {
    pub rgb: [f64; 3],
    pub alpha: f64,
    /// `−1` when the fan stands at `−X` (the sun west), `+1` at `+X`.
    pub side: f64,
}

/// What the client draws its sky from, at a tick for one look vector: the clear
/// colour, the sky disc's colour, and the fan — display values in `0..=1`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GameSky {
    pub fog: [f64; 3],
    pub sky: [f64; 3],
    pub fan: Option<Fan>,
}

/// The [`GameSky`] of a tick, for a camera looking along `view` (unit).
pub fn game_sky(daytime_ticks: i64, view: [f64; 3]) -> GameSky {
    let t = daytime_ticks;
    let to_i = |c: [u8; 3]| c.map(i32::from);
    let mut fog = to_i(multiply_rgb(
        day_rgb("visual/fog_color", t),
        OVERWORLD_FOG_RGB,
    ));
    let sky = to_i(multiply_rgb(day_rgb("visual/sky_color", t), GROUND_SKY_RGB));
    let [a, r, g, b] = day_argb("visual/sunrise_sunset_color", t);
    let alpha = f64::from(a) / 255.0;
    let west = delvewright_dsl::celestial::sun_is_west(t as f64);
    let side = if west { -1.0 } else { 1.0 };
    let dot = view[0] * side;
    if RENDER_DISTANCE >= 4.0 && dot > 0.0 && alpha > 0.0 {
        fog = srgb_lerp(dot * alpha, fog, to_i([r, g, b]));
    }
    let reach = ((SKY_FOG_END_DISTANCE / 16.0).min(RENDER_DISTANCE) / 32.0).clamp(0.25, 1.0);
    let w = 1.0 - reach.sqrt().sqrt();
    fog = srgb_lerp(w, fog, sky);
    let d = |c: [i32; 3]| c.map(|v| f64::from(v) / 255.0);
    GameSky {
        fog: d(fog),
        sky: d(sky),
        fan: (alpha > 0.001).then_some(Fan {
            rgb: d(to_i([r, g, b])),
            alpha,
            side,
        }),
    }
}

/// The colour the client draws in direction `d` (unit, `y` up), as display
/// values: the sky disc with its fog above the horizon inside the octagon, the
/// clear colour elsewhere, the fan blended over both.
pub fn sky_display(gs: &GameSky, d: [f64; 3]) -> [f64; 3] {
    let mut c = gs.fog;
    if d[1] > 0.0 {
        let k = DISC_HEIGHT / d[1];
        let (px, pz) = (d[0] * k, d[2] * k);
        let step = PI / 4.0;
        let mut proj = f64::MIN;
        for e in 0..8 {
            let a = -PI + step / 2.0 + step * e as f64;
            proj = proj.max(px * det_cos(a) + pz * det_sin(a));
        }
        let f = proj / (DISC_RADIUS * det_cos(step / 2.0));
        if f < 1.0 {
            let rim = (DISC_RADIUS * DISC_RADIUS + DISC_HEIGHT * DISC_HEIGHT).sqrt();
            let sph = DISC_HEIGHT + (rim - DISC_HEIGHT) * f;
            let cyl = DISC_HEIGHT + (DISC_RADIUS - DISC_HEIGHT) * f;
            let end = (RENDER_DISTANCE * 16.0).min(SKY_FOG_END_DISTANCE);
            let lin = |v: f64, s: f64, e: f64| {
                if v <= s {
                    0.0
                } else if v >= e {
                    1.0
                } else {
                    (v - s) / (e - s)
                }
            };
            let fv = lin(sph, 0.0, end).max(lin(cyl, end, end));
            c = std::array::from_fn(|i| gs.sky[i] + (gs.fog[i] - gs.sky[i]) * fv);
        }
    }
    if let Some(fan) = gs.fan {
        let a_f = fan.alpha * fan_weight(fan, d);
        c = std::array::from_fn(|i| c[i] * (1.0 - a_f) + fan.rgb[i] * a_f);
    }
    c
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The fan's interpolated vertex alpha (1 at its centre, 0 at its rim) where a
/// ray from the camera along `d` meets it, or 0 where it misses.
fn fan_weight(fan: Fan, d: [f64; 3]) -> f64 {
    let s = fan.side;
    let centre = [FAN_CENTRE * s, 0.0, 0.0];
    let rim = |j: usize| {
        let a = 2.0 * PI * j as f64 / FAN_STEPS as f64;
        let (sa, ca) = (det_sin(a), det_cos(a));
        [
            FAN_RIM * s * ca,
            FAN_LIFT * fan.alpha * ca,
            -FAN_RIM * s * sa,
        ]
    };
    for j in 0..FAN_STEPS {
        let (e1, e2) = (sub(rim(j), centre), sub(rim(j + 1), centre));
        let p = cross(d, e2);
        let det = dot3(e1, p);
        if det.abs() < 1e-12 {
            continue;
        }
        let inv = 1.0 / det;
        let tv = [-centre[0], -centre[1], -centre[2]];
        let u = dot3(tv, p) * inv;
        if !(0.0..=1.0).contains(&u) {
            continue;
        }
        let q = cross(tv, e1);
        let v = dot3(d, q) * inv;
        if v < 0.0 || u + v > 1.0 {
            continue;
        }
        if dot3(e2, q) * inv > 0.0 {
            return 1.0 - u - v;
        }
    }
    0.0
}

/// The direction of a map pixel: column `i` at `2πi / W` round from `+X`
/// toward `+Z`, row `j` at `πj / H − π/2` up from the nadir — the pinned core's
/// unmirrored equirectangular lookup (`u = atan2(z, x) / 2π`, `v = (asin y +
/// π/2) / π`, sampled at `x = W·u`, `y = H·v`).
pub fn pixel_direction(i: usize, j: usize) -> [f64; 3] {
    let th = 2.0 * PI * i as f64 / WIDTH as f64;
    let ph = PI * j as f64 / HEIGHT as f64 - PI / 2.0;
    let cp = det_cos(ph);
    [cp * det_cos(th), det_sin(ph), cp * det_sin(th)]
}

/// The view direction of a Chunky camera's stored orientation:
/// `(cos yaw · sin pitch, −cos pitch, −sin yaw · sin pitch)` (the module header
/// of `scene`).
pub fn view_of(yaw: f64, pitch: f64) -> [f64; 3] {
    [
        det_cos(yaw) * det_sin(pitch),
        -det_cos(pitch),
        -det_sin(yaw) * det_sin(pitch),
    ]
}

/// A clear scene's sky map and the multiplier it casts light by.
#[derive(Debug, Clone, PartialEq)]
pub struct ClearSky {
    /// Linear radiance per pixel, row-major from the nadir row: the game's sky
    /// plus [`ClearSky::veil`].
    pub pixels: Vec<[f64; 3]>,
    /// `skyLight`: the map times this is the light the sky casts.
    pub sky_light: f64,
    /// The uniform radiance added to every pixel so the light is the lightmap's.
    pub veil: [f64; 3],
    /// The cosine-weighted mean of the game's sky over the upper hemisphere.
    pub mean: [f64; 3],
}

/// **The clear sky of a tick for a camera looking along `view`.**
///
/// The lens sees the map at `apparentSkyLight` 1; the light a diffuse bounce
/// takes from it is `skyLight` times the map. A Lambertian top face under it
/// returns `albedo × skyLight × E`, `E` the map's cosine-weighted mean over the
/// upper hemisphere. The target is the lightmap's full-sky colour decoded,
/// `L` ([`lightmap_sky`], the clear cell's light). With `G` the game's sky and
/// `Ē` its mean, the map is `G + R` with `R` uniform and `skyLight · (Ē + R) =
/// L`: **authored** — `skyLight` is the largest the bound allows that keeps
/// every channel of `R` at or above 0, `min(min_c L_c / Ē_c, MAX_SKY_LIGHT)`,
/// so the veil `R = L / skyLight − Ē` is the least that makes the light
/// exact. Where the game's sky is lit and blue (noon) the veil is the light's
/// remainder and pales the lens's sky; at the setting sun it is small; at
/// night, under a black sky, it is `L / 50`.
pub fn clear_sky(daytime_ticks: i64, view: [f64; 3]) -> ClearSky {
    let t = daytime_ticks;
    let gs = game_sky(t, view);
    let mut game = Vec::with_capacity(WIDTH * HEIGHT);
    let mut acc = [0.0f64; 3];
    let mut weight = 0.0f64;
    for j in 0..HEIGHT {
        for i in 0..WIDTH {
            let d = pixel_direction(i, j);
            let c = sky_display(&gs, d).map(to_linear);
            if d[1] > 0.0 {
                let w = d[1] * (1.0 - d[1] * d[1]).sqrt();
                for k in 0..3 {
                    acc[k] += w * c[k];
                }
                weight += w;
            }
            game.push(c);
        }
    }
    let mean = acc.map(|a| a / weight);
    let light = lightmap_sky(
        day_float("visual/sky_light_factor", t),
        day_rgb("visual/sky_light_color", t),
        DEFAULT_BRIGHTNESS,
    )
    .map(to_linear);
    let mut s = MAX_SKY_LIGHT;
    for k in 0..3 {
        if mean[k] > 0.0 {
            s = s.min(light[k] / mean[k]);
        }
    }
    let veil: [f64; 3] = std::array::from_fn(|k| (light[k] / s - mean[k]).max(0.0));
    let pixels = game
        .into_iter()
        .map(|c| std::array::from_fn(|k| c[k] + veil[k]))
        .collect();
    ClearSky {
        pixels,
        sky_light: s,
        veil,
        mean,
    }
}

/// The map as a `.pfm` file: the `PF` header, then every pixel as three
/// little-endian `f32`, rows from the nadir up — the order `PFMTexture` reads
/// (`buf` row `y` at `v = y / H`).
pub fn pfm_bytes(sky: &ClearSky) -> Vec<u8> {
    let mut out = format!("PF\n{WIDTH} {HEIGHT}\n-1.0\n").into_bytes();
    out.reserve(WIDTH * HEIGHT * 12);
    for p in &sky.pixels {
        for c in p {
            out.extend_from_slice(&(*c as f32).to_le_bytes());
        }
    }
    out
}

/// The sky map a scene names, derived from the scene's own bytes: its
/// `delvewrightSky.daytimeTicks`, its camera's stored orientation and its
/// `sky.skymap` file name. `None` for a scene that names no map. The scene
/// writer writes it beside the scene, so a map always answers the scene it
/// was emitted with.
pub fn companion(scene_json: &[u8]) -> Option<(String, Vec<u8>)> {
    let v: serde_json::Value = serde_json::from_slice(scene_json).ok()?;
    let name = v["sky"]["skymap"].as_str()?.to_string();
    let tick = v["delvewrightSky"]["daytimeTicks"].as_i64()?;
    let o = &v["camera"]["orientation"];
    let view = view_of(o["yaw"].as_f64()?, o["pitch"].as_f64()?);
    Some((name, pfm_bytes(&clear_sky(tick, view))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn det_trig_is_the_platforms_to_1e_14() {
        let mut worst = 0.0f64;
        for i in -2000..=2000 {
            let x = f64::from(i) * 0.005;
            worst = worst.max((det_sin(x) - x.sin()).abs());
            worst = worst.max((det_cos(x) - x.cos()).abs());
        }
        assert!(worst < 1e-14, "{worst}");
    }

    /// At the setting sun the fan stands on the western horizon: west of the
    /// camera at the horizon the map carries its tint, east it does not, and
    /// the light the map casts is the lightmap's.
    #[test]
    fn the_fan_stands_on_the_suns_side() {
        let gs = game_sky(12782, [0.0, 0.0, 1.0]);
        let fan = gs.fan.expect("the fan is drawn at the setting sun");
        assert_eq!(fan.side, -1.0);
        let west = sky_display(&gs, [-0.999, 0.04, 0.0]);
        let east = sky_display(&gs, [0.999, 0.04, 0.0]);
        assert!(west[0] > east[0] + 0.2, "{west:?} vs {east:?}");
        let sky = clear_sky(12782, [0.0, 0.0, 1.0]);
        let light = lightmap_sky(
            day_float("visual/sky_light_factor", 12782),
            day_rgb("visual/sky_light_color", 12782),
            DEFAULT_BRIGHTNESS,
        )
        .map(to_linear);
        for (k, want) in light.iter().enumerate() {
            let cast = sky.sky_light * (sky.mean[k] + sky.veil[k]);
            assert!((cast - want).abs() < 1e-12, "{k}: {cast} vs {want}");
            assert!(sky.veil[k] >= 0.0);
        }
    }

    /// Looking toward the sun tints the clear colour; looking away does not.
    #[test]
    fn the_clear_colour_follows_the_look() {
        let toward = game_sky(12782, [-1.0, 0.0, 0.0]);
        let away = game_sky(12782, [1.0, 0.0, 0.0]);
        assert!(toward.fog[0] > away.fog[0], "{toward:?} {away:?}");
    }

    /// The map's bytes are a function of the tick: two calls agree, and a
    /// moved tick moves them.
    #[test]
    fn the_map_is_byte_stable_and_moves_with_the_tick() {
        let v = view_of(3.874631, -1.815142);
        let a = pfm_bytes(&clear_sky(12782, v));
        assert_eq!(a, pfm_bytes(&clear_sky(12782, v)));
        assert_ne!(a, pfm_bytes(&clear_sky(13047, v)));
        assert_eq!(a.len(), "PF\n360 180\n-1.0\n".len() + WIDTH * HEIGHT * 12);
    }
}
