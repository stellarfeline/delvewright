//! **The one function every frame goes through** (spec-0112 §6): a written
//! world, the chunk rectangle it is loaded over, the client jar, the build's
//! resource pack, a camera, a sky, a mode and a backend in; the pixels, what
//! the renderer drew and what it did not, and the timings out.
//!
//! The renderer is Lumenwright, linked as a library and called in this
//! process (`lumenwright::render`). It resolves no jar, opens no network
//! connection and writes no file; the engine hands it every input:
//!
//! - **the world and its chunks** — a directory the engine wrote
//!   ([`crate::compiler::view::world`]) and the exact inclusive chunk
//!   rectangle of the build's loaded extent, never a radius around the
//!   camera;
//! - **the jar** — resolved by [`crate::compiler::view::cli::resolve_textures`],
//!   the engine's one ladder;
//! - **the sky** — the engine's reading of the pinned client's sky at the
//!   frame's tick under its weather ([`crate::compiler::view::scene::sky_cell`],
//!   [`crate::compiler::view::scene::sky_attributes`],
//!   [`crate::compiler::view::scene::sun_at`]), stated to the renderer as
//!   [`lumenwright::Sky::Stated`] ([`stated_sky`]): the colours, the sun and
//!   the lightmap's factor are always the engine's. Under a clear sky the
//!   statement also stands at the frame's clock with the game's look by the
//!   time of day ([`lumenwright::SkyLook::ByTime`]), so what the client draws
//!   from the tick alone — the sunrise fan, the moon at its phase, the stars,
//!   and in hq the light the game's sky casts — is drawn at that clock; under
//!   rain or thunder it stands at no tick, because the game's look draws no
//!   weather;
//! - **the bodies** — every entity and block entity the world holds, with a
//!   body the renderer has no model for skipped and counted, never drawn as a
//!   box ([`lumenwright::UnknownEntities::Skip`]).
//!
//! The mode is the caller's, read off the shot (`render-plan.json` derives it
//! per kind), never a flag: [`Mode::Quick`] is the vanilla-client look a
//! review frame asks for, [`Mode::Hq`] the path-traced picture.
//!
//! Pixels are validation artifacts: no pixel enters a hash or a gate
//! (spec-0112 §6.3). The CPU backend gives byte-identical pixels for any
//! thread count on one machine.

use std::path::{Path, PathBuf};

use lumenwright::glam::{DVec3, Vec3};
use lumenwright::render::sky::HqSky;
use lumenwright::render::{BackendKind, Mode as LwMode};
use lumenwright::scene::camera::Camera as LwCamera;
use lumenwright::world::{Dimension, World};
use lumenwright::{
    ChunkRect, EntityPolicy, HqOptions, QuickOptions, QuickSkyColours, Request, RequestError, Sky,
    SkyLook, StatedSky, StatedTick, UnknownEntities, WorldSource,
};

use delvewright_dsl::{Clock, WorldWeather};

use crate::compiler::framing::Camera;
use crate::compiler::view::diag::{DW_INPUT, DW_RENDER, Diagnostic};
use crate::compiler::view::scene;

/// The Lumenwright version this binary links, as `versions.toml [render]
/// lumenwright_version` and `crates/delvec/Cargo.toml` pin it — written on
/// every frame manifest and on the first line of every run, so a frame's
/// provenance is readable without the frame.
pub const RENDERER_VERSION: &str = "0.3.0";

/// The client's *Mipmap Levels* option at its default, the texture filtering
/// a player who has not changed it sees.
pub const CLIENT_MIPMAP_LEVELS: u32 = 4;

/// The renderer, in words: `lumenwright <version>`.
pub fn renderer() -> String {
    format!("lumenwright {RENDERER_VERSION}")
}

/// The review frame's size, square, in pixels.
pub const REVIEW_SIZE: u32 = 1024;

/// The hq review budget, samples a pixel (spec-0112 §6.1 item 4).
pub const REVIEW_HQ_SPP: u32 = 256;

/// **The exposure every hq frame is drawn at: derived, never stated.** The
/// renderer's tone curve maps a radiance of one to the display's white; a
/// showcase camera that states its own is refused (`DW1010`), because a room
/// that reads too dark is lit, never exposed.
pub const HQ_EXPOSURE: f32 = 1.0;

/// How a frame is drawn. Derived per shot by the plan, never chosen by a flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The vanilla-client look: rasterised, lit by the lightmap from the light
    /// the world carries.
    Quick,
    /// Path traced at `spp` samples a pixel.
    Hq { spp: u32 },
}

impl Mode {
    /// `quick` or `hq`, as the plan writes it.
    pub fn keyword(self) -> &'static str {
        match self {
            Mode::Quick => "quick",
            Mode::Hq { .. } => "hq",
        }
    }
}

/// Which backend draws a frame. `auto` takes the GPU wgpu finds and falls back
/// to the CPU, saying so; a named backend that is not available is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Backend {
    Auto,
    Cpu,
    Metal,
    Vulkan,
    Dx12,
}

impl Backend {
    fn kind(self) -> BackendKind {
        match self {
            Backend::Auto => BackendKind::Auto,
            Backend::Cpu => BackendKind::Cpu,
            Backend::Metal => BackendKind::Metal,
            Backend::Vulkan => BackendKind::Vulkan,
            Backend::Dx12 => BackendKind::Dx12,
        }
    }

    /// The backend's name as the command line spells it.
    pub fn name(self) -> &'static str {
        match self {
            Backend::Auto => "auto",
            Backend::Cpu => "cpu",
            Backend::Metal => "metal",
            Backend::Vulkan => "vulkan",
            Backend::Dx12 => "dx12",
        }
    }
}

/// The clock a frame is of: the day, the `daytime` tick and the weather.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameSky {
    /// `time query day` of the frame's clock: the moon shows this day's phase.
    pub day: i64,
    /// `time query daytime`: what the sun and the sky's colours are a
    /// function of.
    pub daytime_ticks: i64,
    pub weather: WorldWeather,
}

impl FrameSky {
    /// The absolute `dayTime` (`day × 24000 + daytime`): the tick a clear
    /// frame's sky stands at ([`stated_sky`]).
    pub fn day_time(&self) -> i64 {
        Clock {
            day: self.day,
            daytime: self.daytime_ticks,
        }
        .absolute()
    }
}

/// The unit vector toward the sun at a tick, in world axes (+X east, +Y up),
/// from [`scene::sun_at`]: the body travels the east–west great circle.
pub fn sun_direction(daytime_ticks: i64) -> [f64; 3] {
    let s = scene::sun_at(daytime_ticks);
    [
        s.azimuth.cos() * s.altitude.cos(),
        s.altitude.sin(),
        s.azimuth.sin() * s.altitude.cos(),
    ]
}

/// The colours quick mode draws for a camera looking along `look`, read off
/// the pinned client ([`scene::sky_attributes`]): the sky colour, the clear
/// colour the client draws behind its sky disc and under the horizon for that
/// look ([`scene::clear_color`] — the renderer draws a stated fog as given),
/// the sun's direction (placed below the horizon under rain or thunder, where
/// the client draws no sun) and the lightmap's sky factor.
pub fn quick_sky(sky: FrameSky, look: [f64; 3]) -> QuickSkyColours {
    let at = scene::sky_attributes(sky.daytime_ticks, sky.weather);
    let c = |v: [u8; 3]| Vec3::new(v[0] as f32, v[1] as f32, v[2] as f32) / 255.0;
    let sun = if sky.weather == WorldWeather::Clear {
        let d = sun_direction(sky.daytime_ticks);
        Vec3::new(d[0] as f32, d[1] as f32, d[2] as f32)
    } else {
        Vec3::new(0.0, -1.0, 0.0)
    };
    QuickSkyColours {
        sky: c(at.sky_color),
        fog: c(scene::clear_color(sky.daytime_ticks, sky.weather, look)),
        sun_dir: sun,
        sky_factor: at.sky_light_factor as f32,
    }
}

/// The sky an hq frame is traced under: Lumenwright's, the sky Chunky traces
/// (`render::sky::hq_sky`: the Preetham sky, its fall through twilight, and a
/// white sun at Chunky's default intensity, cited there) along the engine's
/// sun direction ([`sun_direction`]). Two readings of the client hold it to
/// the game: the sun is dark while the client draws no disc
/// ([`scene::LookCell::sun_draw_texture`]: below the horizon, or under rain
/// or thunder), and the sky's radiance is scaled by the light the client's sky
/// casts under the frame's weather as a fraction of what it casts in the clear
/// at the same tick ([`scene::sky_cell`]'s `sky_light`), so a clear sky is
/// Lumenwright's own and a rain sky is as much darker as the game's.
pub fn hq_sky(sky: FrameSky) -> HqSky {
    let cell = scene::sky_cell(sky.daytime_ticks, sky.weather);
    let clear = scene::sky_cell(sky.daytime_ticks, WorldWeather::Clear);
    let d = sun_direction(sky.daytime_ticks);
    let dir = Vec3::new(d[0] as f32, d[1] as f32, d[2] as f32);
    let intensity = if clear.sky_light > 0.0 {
        (cell.sky_light / clear.sky_light) as f32
    } else {
        1.0
    };
    let mut hq = lumenwright::render::sky::hq_sky(
        dir,
        lumenwright::render::sky::DEFAULT_SUN_INTENSITY,
        intensity,
    );
    if !(cell.sun_draw_texture && d[1] > 0.0) {
        hq.sun_color = Vec3::ZERO;
    }
    hq
}

/// **The sky stated to the renderer for one frame** (spec-0112 §6.1 item 6).
/// In both modes the colours, the sun and the lightmap's factor are the
/// engine's ([`quick_sky`] for the camera's look, and [`hq_sky`], Chunky's part
/// of an hq frame).
///
/// **Under a clear sky the statement stands at the frame's clock**
/// ([`FrameSky::day_time`], so the moon shows the clock's phase) with the
/// game's look by the time of day ([`SkyLook::ByTime`]) at the client's render
/// distance ([`scene::CLIENT_RENDER_DISTANCE`]): the renderer draws what the
/// client derives from the tick alone — the sunrise fan, the moon and the stars
/// in both modes — and an hq frame is lit as the game lights the tick, Chunky's
/// look on the client's day plateau, the game's at night and a mix by the
/// lightmap's sky factor between them. A quick frame takes the tick because its
/// question is what the player sees, and the client draws the fan, the moon and
/// the stars of a clear sky.
///
/// **Under rain or thunder it stands at no tick**: the game's look draws a
/// clear sky and no weather, and the client at full rain draws no sun, moon or
/// stars, so the frame is the engine's darkened colours in quick mode and
/// Chunky's sky scaled to the weather's light, its sun dark, in hq mode.
pub fn stated_sky(sky: FrameSky, look: [f64; 3]) -> StatedSky {
    let stated = StatedSky::new(quick_sky(sky, look), hq_sky(sky));
    if sky.weather != WorldWeather::Clear {
        return stated;
    }
    let mut tick = StatedTick::new(sky.day_time(), SkyLook::ByTime);
    tick.render_distance = scene::CLIENT_RENDER_DISTANCE * 16.0;
    stated.at(tick)
}

/// One frame to draw.
pub struct FrameRequest<'a> {
    /// The written world's directory (`level.dat`, `region/`, `entities/`).
    pub world: &'a Path,
    /// The inclusive chunk rectangle loaded: `[min_x, min_z]..=[max_x, max_z]`.
    pub chunks: ([i32; 2], [i32; 2]),
    /// The pinned client jar.
    pub jar: &'a Path,
    /// The build's resource pack, when it has one.
    pub packs: &'a [PathBuf],
    pub camera: Camera,
    pub sky: FrameSky,
    pub mode: Mode,
    pub backend: Backend,
    pub width: u32,
    pub height: u32,
    /// **How far the frame draws.** `None`: every loaded chunk, the fog closing
    /// past the farthest of them — a review frame shows the whole scene.
    /// `Some(n)`: what the pinned client draws at render distance `n` — the
    /// square of `n` chunks around the camera's own chunk ([`served_rect`]),
    /// the fog closing at `16 · n` blocks — so a frame taken at a pose shows
    /// what a player standing there sees, and nothing the server never sends.
    pub served: Option<u8>,
}

/// The chunks the pinned client draws at render distance `n` around a camera
/// at `eye`, inside the `loaded` rectangle: the square of `n` chunks around the
/// camera's own chunk (`delvewright_dsl::viewdistance`, spec-0091 §2), clipped
/// to what was loaded. `None` when the two do not meet: the camera stands
/// farther than `n` chunks from every loaded chunk, and the client draws
/// nothing there.
pub fn served_rect(
    eye: [f64; 3],
    n: u8,
    loaded: ([i32; 2], [i32; 2]),
) -> Option<([i32; 2], [i32; 2])> {
    let cx = (eye[0].floor() as i32).div_euclid(16);
    let cz = (eye[2].floor() as i32).div_euclid(16);
    let n = i32::from(n);
    let lo = [loaded.0[0].max(cx - n), loaded.0[1].max(cz - n)];
    let hi = [loaded.1[0].min(cx + n), loaded.1[1].min(cz + n)];
    (lo[0] <= hi[0] && lo[1] <= hi[1]).then_some((lo, hi))
}

/// What the renderer reported of one body kind: drawn, drawn as a box, or
/// skipped (no model).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stats {
    /// Entity kinds drawn, by short id, with their count.
    pub entities_rendered: std::collections::BTreeMap<String, usize>,
    /// Entity kinds drawn as a box (never under the engine's policy).
    pub entities_boxed: std::collections::BTreeMap<String, usize>,
    /// Entity kinds the renderer drew nothing of: a kind it has no model for,
    /// or a dropped item or item display whose item it cannot draw.
    pub entities_skipped: std::collections::BTreeMap<String, usize>,
    /// Item stacks drawn (dropped, displayed, framed, held or worn), by short
    /// item id.
    pub items_rendered: std::collections::BTreeMap<String, usize>,
    /// Item stacks the renderer could not draw, by short item id.
    pub items_skipped: std::collections::BTreeMap<String, usize>,
    /// Block-entity kinds drawn.
    pub block_entities_rendered: std::collections::BTreeMap<String, usize>,
    /// Block-entity kinds not drawn.
    pub block_entities_skipped: std::collections::BTreeMap<String, usize>,
    /// Sign lines with text, and the glyphs drawn for them.
    pub text_lines: usize,
    pub glyphs: usize,
}

impl Stats {
    /// Whether a body of entity kind `kind` (short id, `mannequin`) is drawn
    /// in this frame: the renderer reported the kind rendered, or it is a kind
    /// that has no body to draw (`interaction`, `marker`).
    pub fn drew(&self, kind: &str) -> bool {
        self.entities_rendered.contains_key(kind)
    }

    /// Whether the renderer skipped (or boxed) bodies of `kind` in this frame,
    /// or `kind` holds items and an item stack in this frame was not drawn.
    pub fn undrew(&self, kind: &str) -> bool {
        self.entities_skipped.contains_key(kind)
            || self.entities_boxed.contains_key(kind)
            || (HOLDS_ITEMS.contains(&kind) && !self.items_skipped.is_empty())
    }

    /// The statistics as the frame manifest writes them.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "entities": {
                "rendered": self.entities_rendered,
                "fallback_boxes": self.entities_boxed,
                "skipped": self.entities_skipped,
                "items_rendered": self.items_rendered,
                "items_skipped": self.items_skipped,
            },
            "block_entities": {
                "rendered": self.block_entities_rendered,
                "skipped": self.block_entities_skipped,
                "text_lines": self.text_lines,
                "glyphs": self.glyphs,
            },
        })
    }
}

/// The entity kinds (short ids) that carry item stacks the renderer draws as
/// items: a dropped item, an item display, a frame's content, a held or worn
/// stack. Its statistics count items by item id, not by holder, so a stack it
/// could not draw is a doubt on every holder in the frame.
pub const HOLDS_ITEMS: [&str; 6] = [
    "item",
    "item_display",
    "item_frame",
    "glow_item_frame",
    "armor_stand",
    "mannequin",
];

/// A drawn frame.
pub struct Drawn {
    pub width: u32,
    pub height: u32,
    /// RGBA8, sRGB, row major, top row first.
    pub rgba: Vec<u8>,
    /// The backend that drew it, in the renderer's words (`cpu (10 threads)`).
    pub backend: String,
    /// Set when `auto` took the CPU: why no GPU was used.
    pub fallback: Option<String>,
    /// Seconds per stage, in order.
    pub timings: Vec<(&'static str, f64)>,
    /// Chunks the renderer loaded.
    pub chunks_loaded: usize,
    pub stats: Stats,
}

impl Drawn {
    /// The backend's name without the renderer's parenthetical (`cpu`, not
    /// `cpu (10 threads)`): what a manifest and the index record, so the bytes
    /// of a frame's record do not move with the thread count that drew it.
    pub fn backend_name(&self) -> String {
        self.backend
            .split(" (")
            .next()
            .unwrap_or(&self.backend)
            .to_string()
    }

    /// The frame's total time, seconds.
    pub fn seconds(&self) -> f64 {
        self.timings.iter().map(|(_, s)| s).sum()
    }
}

/// How far the quick-mode fog closes from `eye` so every loaded chunk is
/// drawn: the distance to the farthest corner of the chunk rectangle at the
/// eye's height band, never under 64 blocks.
fn draw_distance(eye: [f64; 3], chunks: ([i32; 2], [i32; 2])) -> f32 {
    let (lo, hi) = chunks;
    let mut far: f64 = 64.0;
    for x in [lo[0] * 16, hi[0] * 16 + 16] {
        for z in [lo[1] * 16, hi[1] * 16 + 16] {
            for y in [-64.0f64, 320.0] {
                let d = ((x as f64 - eye[0]).powi(2)
                    + (y - eye[1]).powi(2)
                    + (z as f64 - eye[2]).powi(2))
                .sqrt();
                far = far.max(d);
            }
        }
    }
    far.ceil() as f32
}

/// The world a run renders from, opened once and drawn many times.
pub struct OpenedWorld {
    world: World,
}

impl OpenedWorld {
    /// Open a written world.
    pub fn open(dir: &Path) -> Result<OpenedWorld, Diagnostic> {
        World::open(dir, Dimension::Overworld)
            .map(|world| OpenedWorld { world })
            .map_err(|e| {
                Diagnostic::error(DW_INPUT, format!("open world {}: {e:#}", dir.display()))
            })
    }
}

/// **Draw one frame.** A request the renderer refuses before drawing (a jar
/// or pack that is not there, a chunk rectangle holding no chunk) and a
/// backend that cannot be opened are `DW0723`, naming what was asked.
pub fn draw(req: &FrameRequest<'_>, opened: Option<&OpenedWorld>) -> Result<Drawn, Diagnostic> {
    let (chunks, reach) = match req.served {
        None => (req.chunks, draw_distance(req.camera.pos, req.chunks)),
        Some(n) => match served_rect(req.camera.pos, n, req.chunks) {
            Some(r) => (r, f32::from(n) * 16.0),
            None => {
                return Err(Diagnostic::error(
                    DW_RENDER,
                    format!(
                        "the camera at {:?} stands farther than the served {n} chunk(s) from \
                         every loaded chunk: the client draws nothing there",
                        req.camera.pos
                    ),
                ));
            }
        },
    };
    let cam = LwCamera {
        pos: DVec3::new(req.camera.pos[0], req.camera.pos[1], req.camera.pos[2]),
        yaw: req.camera.yaw as f32,
        pitch: req.camera.pitch as f32,
        fov_y: req.camera.fov as f32,
    };
    let (mode, hq) = match req.mode {
        Mode::Quick => (LwMode::Quick, HqOptions::default()),
        Mode::Hq { spp } => (
            LwMode::Hq,
            HqOptions {
                spp,
                exposure: HQ_EXPOSURE,
                ..HqOptions::default()
            },
        ),
    };
    let world = match opened {
        Some(o) => WorldSource::Opened(&o.world),
        None => WorldSource::Dir {
            path: req.world,
            dimension: Dimension::Overworld,
        },
    };
    let request = Request {
        world,
        chunks: ChunkRect {
            min: chunks.0,
            max: chunks.1,
        },
        jar: req.jar,
        // No Unifont is handed, so a character no bitmap font of the jar
        // covers (CJK among them) draws nothing on a sign.
        unifont: None,
        resource_packs: req.packs,
        camera: cam,
        mode,
        backend: req.backend.kind(),
        width: req.width,
        height: req.height,
        entities: EntityPolicy {
            entities: true,
            unknown: UnknownEntities::Skip,
            block_entities: true,
            players: false,
        },
        quick: QuickOptions {
            ssaa: 1,
            draw_distance: reach,
            brightness: scene::DEFAULT_BRIGHTNESS as f32,
            smooth: true,
            mipmap_levels: CLIENT_MIPMAP_LEVELS,
        },
        hq,
        sky: Sky::Stated(stated_sky(req.sky, req.camera.basis().0)),
    };
    let r = lumenwright::render(&request).map_err(|e| {
        let why = match e.downcast_ref::<RequestError>() {
            Some(r) => format!("the renderer refused the request: {r}"),
            None => format!(
                "the renderer failed on the {} backend: {e:#}",
                req.backend.name()
            ),
        };
        Diagnostic::error(DW_RENDER, why)
    })?;
    let short = |m: &std::collections::BTreeMap<String, usize>| {
        m.iter()
            .map(|(k, v)| (k.strip_prefix("minecraft:").unwrap_or(k).to_string(), *v))
            .collect()
    };
    Ok(Drawn {
        width: r.width,
        height: r.height,
        rgba: r.rgba,
        backend: r.backend,
        fallback: r.backend_fallback,
        timings: r.timings,
        chunks_loaded: r.chunks_loaded,
        stats: Stats {
            entities_rendered: short(&r.entity_stats.rendered),
            entities_boxed: short(&r.entity_stats.fallback_boxes),
            entities_skipped: short(&r.entity_stats.skipped),
            items_rendered: short(&r.entity_stats.items_rendered),
            items_skipped: short(&r.entity_stats.items_skipped),
            block_entities_rendered: short(&r.block_entity_stats.rendered),
            block_entities_skipped: short(&r.block_entity_stats.skipped),
            text_lines: r.block_entity_stats.text_lines,
            glyphs: r.block_entity_stats.glyphs,
        },
    })
}

/// The file stem a shot's frame is written under: its id with `/`, `:` and
/// spaces as `_`.
pub fn frame_stem(shot_id: &str) -> String {
    shot_id.replace(['/', ':', ' '], "_")
}

/// The mean luma (Rec. 709) of an RGBA frame, `0..=255`.
pub fn mean_luma(rgba: &[u8]) -> f64 {
    let n = rgba.len() / 4;
    if n == 0 {
        return 0.0;
    }
    let sum: f64 = rgba
        .chunks_exact(4)
        .map(|p| 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64)
        .sum();
    sum / n as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sun_rises_in_the_east_and_sets_in_the_west() {
        let dawn = sun_direction(0);
        let noon = sun_direction(6000);
        let dusk = sun_direction(12000);
        // The game's day starts with the sun already a little up in the east
        // (its celestial angle eases, so tick 0 is about 12° of altitude) and
        // tick 12000 has it the same height in the west.
        assert!(noon[1] > 0.99, "{noon:?}");
        assert!(dawn[0] > 0.9 && dawn[1] > 0.0 && dawn[1] < 0.3, "{dawn:?}");
        assert!(dusk[0] < -0.9 && dusk[1] > 0.0 && dusk[1] < 0.3, "{dusk:?}");
        let midnight = sun_direction(18000);
        assert!(midnight[1] < -0.99, "{midnight:?}");
    }

    #[test]
    fn a_rain_sky_draws_no_sun_and_a_midnight_sky_is_dark() {
        let rain = quick_sky(
            FrameSky {
                day: 0,
                daytime_ticks: 6000,
                weather: WorldWeather::Rain,
            },
            [0.0, 0.0, 1.0],
        );
        assert!(rain.sun_dir.y < 0.0);
        let noon = quick_sky(
            FrameSky {
                day: 0,
                daytime_ticks: 6000,
                weather: WorldWeather::Clear,
            },
            [0.0, 0.0, 1.0],
        );
        let night = quick_sky(
            FrameSky {
                day: 0,
                daytime_ticks: 18000,
                weather: WorldWeather::Clear,
            },
            [0.0, 0.0, 1.0],
        );
        assert!(night.sky_factor < noon.sky_factor);
        assert!(night.sky.length() < noon.sky.length());
        assert_eq!(
            hq_sky(FrameSky {
                day: 0,
                daytime_ticks: 6000,
                weather: WorldWeather::Thunder
            })
            .sun_color,
            Vec3::ZERO
        );
    }

    /// The clear colour is the client's (`AtmosphericFogEnvironment
    /// .getBaseColor`), never the raw `visual/fog_color`: the renderer draws a
    /// stated fog as given, so a frame whose fog skipped the lean showed the
    /// horizon at the fog colour itself. The lean is `Mth.clampedLerp`'s form,
    /// `1 − (0.25 + 0.75 x)^0.25` at `x = 12 / 32`, not `1 − clamp(x, 0.25,
    /// 1)^0.25` (0.218 at the same distance).
    #[test]
    fn the_clear_colour_is_leaned_as_the_client_leans_it() {
        let lean = scene::clear_lean(scene::CLIENT_RENDER_DISTANCE);
        assert!(
            (lean - (1.0 - 0.53125f32.powf(0.25))).abs() < 1e-6,
            "{lean}"
        );
        assert!((lean - 0.146).abs() < 5e-4, "{lean}");
        assert!((lean - (1.0 - 0.375f32.powf(0.25))).abs() > 0.05, "{lean}");
        // Past 32 chunks the lean is gone; under 8 chunks it holds at the floor.
        assert!(scene::clear_lean(40.0).abs() < 1e-6);
        assert_eq!(
            scene::clear_lean(2.0),
            1.0 - (0.25f32 + 0.75 * 2.0 / 32.0).powf(0.25)
        );

        // At noon, clear: the fog colour leaned toward the sky colour, per
        // 8-bit channel by `Mth.lerpInt`, whichever way the camera looks.
        let at = scene::sky_attributes(6000, WorldWeather::Clear);
        let want: [u8; 3] = std::array::from_fn(|i| {
            let (a, b) = (i32::from(at.fog_color[i]), i32::from(at.sky_color[i]));
            (a + (lean * (b - a) as f32).floor() as i32) as u8
        });
        assert_ne!(want, at.fog_color, "the lean moves the noon fog");
        for look in [[1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]] {
            assert_eq!(scene::clear_color(6000, WorldWeather::Clear, look), want);
            let q = quick_sky(
                FrameSky {
                    day: 0,
                    daytime_ticks: 6000,
                    weather: WorldWeather::Clear,
                },
                look,
            );
            let got = (q.fog * 255.0).round();
            assert_eq!([got.x as u8, got.y as u8, got.z as u8], want);
        }

        // At sunset the sun stands west: a camera looking west sees the
        // sunrise colour in its clear colour, one looking east does not.
        let t = 12000;
        let at = scene::sky_attributes(t, WorldWeather::Clear);
        assert!(
            at.sunrise[0] > 0,
            "a sunset glow at tick {t}: {:?}",
            at.sunrise
        );
        let west = scene::clear_color(t, WorldWeather::Clear, [-1.0, 0.0, 0.0]);
        let east = scene::clear_color(t, WorldWeather::Clear, [1.0, 0.0, 0.0]);
        assert_ne!(west, east);
    }

    /// A clear frame's sky stands at its clock, its day included, with the
    /// game's look by the time of day at the client's render distance; a rain
    /// or thunder frame's at no tick. The colours and sun are the engine's in
    /// every weather.
    #[test]
    fn a_clear_sky_stands_at_its_clock_and_a_wet_one_at_none() {
        let dusk = |weather| FrameSky {
            day: 4,
            daytime_ticks: 12782,
            weather,
        };
        let look = [-0.92, 0.05, -0.38];
        let clear = stated_sky(dusk(WorldWeather::Clear), look);
        let tick = clear.tick.expect("a clear sky stands at a tick");
        assert_eq!(tick.time, 4 * 24000 + 12782);
        assert_eq!(tick.look, SkyLook::ByTime);
        assert_eq!(tick.render_distance, 192.0);
        let engine = quick_sky(dusk(WorldWeather::Clear), look);
        assert_eq!(clear.quick.fog, engine.fog);
        assert_eq!(clear.quick.sun_dir, engine.sun_dir);
        assert_eq!(clear.hq.sun_dir, hq_sky(dusk(WorldWeather::Clear)).sun_dir);
        for w in [WorldWeather::Rain, WorldWeather::Thunder] {
            let wet = stated_sky(dusk(w), look);
            assert!(wet.tick.is_none(), "{w:?}");
            assert_eq!(wet.quick.fog, quick_sky(dusk(w), look).fog, "{w:?}");
            assert_eq!(wet.hq.sun_color, Vec3::ZERO, "{w:?}");
        }
    }

    #[test]
    fn a_served_frame_draws_the_clients_square_inside_what_was_loaded() {
        let loaded = ([-10, -10], [30, 40]);
        // A camera in chunk (2, -1) at render distance 10.
        assert_eq!(
            served_rect([40.5, 80.0, -3.2], 10, loaded),
            Some(([-8, -10], [12, 9]))
        );
        // Far outside every loaded chunk: nothing is drawn.
        assert_eq!(served_rect([1000.0, 80.0, 0.0], 10, loaded), None);
        // The camera's own chunk is read by flooring, so -0.5 is chunk -1.
        assert_eq!(
            served_rect([-0.5, 64.0, 0.0], 0, loaded),
            Some(([-1, 0], [-1, 0]))
        );
    }

    #[test]
    fn the_fog_reaches_every_loaded_chunk() {
        let d = draw_distance([0.0, 70.0, 0.0], ([-2, -2], [2, 2]));
        assert!(d >= (48.0f32 * 48.0 * 2.0).sqrt(), "{d}");
    }
}
