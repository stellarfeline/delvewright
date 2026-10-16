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
//!   [`lumenwright::Sky::Stated`]. Lumenwright's own derivation of a vanilla
//!   sky from a tick is never what a `delvec` frame shows;
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
    StatedSky, UnknownEntities, WorldSource,
};

use delvewright_dsl::WorldWeather;

use crate::compiler::framing::Camera;
use crate::compiler::view::diag::{DW_INPUT, DW_RENDER, Diagnostic};
use crate::compiler::view::scene;

/// The Lumenwright revision this binary links, as `versions.toml [render]
/// lumenwright_revision` and `crates/delvec/Cargo.toml` pin it — written on
/// every frame manifest and on the first line of every run, so a frame's
/// provenance is readable without the frame.
pub const RENDERER_REVISION: &str = "ed3f049cc496d5f1b7df9829bda1bcb443ad1d06";

/// The renderer, in words: `lumenwright <revision>`.
pub fn renderer() -> String {
    format!("lumenwright {RENDERER_REVISION}")
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

/// The clock a frame is of: the `daytime` tick and the weather.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameSky {
    pub daytime_ticks: i64,
    pub weather: WorldWeather,
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

/// The colours quick mode draws, read off the pinned client
/// ([`scene::sky_attributes`]): the sky colour, the fog colour, the sun's
/// direction (placed below the horizon under rain or thunder, where the client
/// draws no sun) and the lightmap's sky factor.
pub fn quick_sky(sky: FrameSky) -> QuickSkyColours {
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
        fog: c(at.fog_color),
        sun_dir: sun,
        sky_factor: at.sky_light_factor as f32,
    }
}

/// The sun's irradiance in an hq frame under a clear sky with the sun above
/// the horizon, as a multiple of white. **Authored**: the game's lightmap has
/// no directional term, so the value is chosen for a picture, not read off
/// the client; zero under rain or thunder and below the horizon.
pub const HQ_SUN_IRRADIANCE: f32 = 1.5;

/// The sky an hq frame is traced under: the zenith is the light the sky casts
/// ([`scene::sky_cell`]: the lightmap's colour decoded to linear), the horizon
/// the fog colour, and the sun as [`HQ_SUN_IRRADIANCE`] while its disc is
/// drawn ([`scene::LookCell::sun_draw_texture`]).
pub fn hq_sky(sky: FrameSky) -> HqSky {
    let cell = scene::sky_cell(sky.daytime_ticks, sky.weather);
    let d = sun_direction(sky.daytime_ticks);
    let zenith = Vec3::new(
        (cell.sky_color.red * cell.sky_light) as f32,
        (cell.sky_color.green * cell.sky_light) as f32,
        (cell.sky_color.blue * cell.sky_light) as f32,
    );
    let horizon = Vec3::new(
        cell.fog_color.red as f32,
        cell.fog_color.green as f32,
        cell.fog_color.blue as f32,
    );
    let sun = if cell.sun_draw_texture && d[1] > 0.0 {
        Vec3::splat(HQ_SUN_IRRADIANCE)
    } else {
        Vec3::ZERO
    };
    HqSky {
        sun_dir: Vec3::new(d[0] as f32, d[1] as f32, d[2] as f32),
        sun_color: sun,
        zenith,
        horizon,
    }
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
}

/// What the renderer reported of one body kind: drawn, drawn as a box, or
/// skipped (no model).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stats {
    /// Entity kinds drawn, by short id, with their count.
    pub entities_rendered: std::collections::BTreeMap<String, usize>,
    /// Entity kinds drawn as a box (never under the engine's policy).
    pub entities_boxed: std::collections::BTreeMap<String, usize>,
    /// Entity kinds the renderer has no model for, not drawn.
    pub entities_skipped: std::collections::BTreeMap<String, usize>,
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

    /// Whether the renderer skipped (or boxed) bodies of `kind` in this frame.
    pub fn undrew(&self, kind: &str) -> bool {
        self.entities_skipped.contains_key(kind) || self.entities_boxed.contains_key(kind)
    }

    /// The statistics as the frame manifest writes them.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "entities": {
                "rendered": self.entities_rendered,
                "fallback_boxes": self.entities_boxed,
                "skipped": self.entities_skipped,
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
            min: req.chunks.0,
            max: req.chunks.1,
        },
        jar: req.jar,
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
            draw_distance: draw_distance(req.camera.pos, req.chunks),
            brightness: scene::DEFAULT_BRIGHTNESS as f32,
            smooth: true,
        },
        hq,
        sky: Sky::Stated(StatedSky {
            quick: quick_sky(req.sky),
            hq: hq_sky(req.sky),
        }),
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
        let rain = quick_sky(FrameSky {
            daytime_ticks: 6000,
            weather: WorldWeather::Rain,
        });
        assert!(rain.sun_dir.y < 0.0);
        let noon = quick_sky(FrameSky {
            daytime_ticks: 6000,
            weather: WorldWeather::Clear,
        });
        let night = quick_sky(FrameSky {
            daytime_ticks: 18000,
            weather: WorldWeather::Clear,
        });
        assert!(night.sky_factor < noon.sky_factor);
        assert!(night.sky.length() < noon.sky.length());
        assert_eq!(
            hq_sky(FrameSky {
                daytime_ticks: 6000,
                weather: WorldWeather::Thunder
            })
            .sun_color,
            Vec3::ZERO
        );
    }

    #[test]
    fn the_fog_reaches_every_loaded_chunk() {
        let d = draw_distance([0.0, 70.0, 0.0], ([-2, -2], [2, 2]));
        assert!(d >= (48.0f32 * 48.0 * 2.0).sqrt(), "{d}");
    }
}
