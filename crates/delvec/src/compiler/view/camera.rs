//! **A showcase camera is one record** (`delvec cameras`).
//!
//! A storybook picture, a front-page hero, a room shown at its best: each is a
//! camera somebody chose — an agent estimating the view an approved concept image
//! was drawn from, or a person standing where the picture should be taken. Both
//! write the same record, and this module is its one reader. There is no second
//! way to say where a showcase camera is.
//!
//! # The record
//!
//! `design/cameras.json` in the campaign, beside the approved images the cameras
//! answer:
//!
//! ```json
//! {
//!   "campaign_id": "doune-castle-tour",
//!   "cameras": [
//!     {
//!       "answers": "concept/kitchen-near",
//!       "exposure": 8.0,
//!       "fov": 62.0,
//!       "height": 900,
//!       "name": "kitchen",
//!       "pitch": 4.0,
//!       "pos": [24.5, 78.9, 45.2],
//!       "spp": 300,
//!       "width": 1600,
//!       "yaw": 205.0
//!     }
//!   ]
//! }
//! ```
//!
//! - `pos` is the lens, in world blocks — not the feet of a body. A camera is
//!   placed where the picture is best, and nothing here asks whether a body could
//!   stand there: in the air, above a courtyard, outside a window, in a corner
//!   above head height are all cameras.
//! - `yaw` and `pitch` are **Minecraft's own entity rotation**, the numbers the
//!   game's debug screen and `/tp <x> <y> <z> <yaw> <pitch>` speak: yaw `0` looks
//!   south (+Z), `90` west, `180` north, `270` (or `-90`) east, and any value in
//!   between is a direction between them; pitch is positive looking down, within
//!   `-90..=90`. So a camera placed by hand in the running game is copied into the
//!   record without arithmetic (the eye is 1.62 blocks above the feet position the
//!   game prints).
//! - `fov` is the **vertical** field of view in degrees — Minecraft's FOV setting
//!   and Chunky's `fov` are both vertical — so a narrower frame from further back
//!   is a smaller number.
//! - `exposure` is the camera's, as a photograph's is: it scales what reaches the
//!   lens and puts no light into the world. A room lit by its torches and a few
//!   slit windows reaches a path tracer's lens far dimmer than the game draws the
//!   same light levels, so an interior is exposed for the interior — stated, per
//!   camera, and judged at the value stated. `1.0` is the review frames' value.
//! - `width`, `height` and `spp` are the frame and the path tracer's sample target:
//!   everything else Chunky needs to reproduce the frame comes from the build (the
//!   declared hour's sun, the loaded chunks, the ocean plane).
//! - `answers` names the row of `design.json` whose approved image the camera is
//!   judged against, and is refused when it names no row.
//!
//! # Candidates
//!
//! An estimate is a start, not a frame. `--bracket yaw=8,pitch=4,fov=10,dolly=6,
//! truck=3,rise=3` emits, beside each camera, the camera moved each way by each step, and
//! writes every candidate into `candidates.json` in the same record format — so the
//! one a creator picks, by looking at it beside the approved image, is copied into
//! `design/cameras.json` verbatim. `--draft` renders any of them small and cheap.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::compiler::view::diag::{DW_INPUT, Diagnostic};
use crate::compiler::view::scene::{
    self, ChunkyCamera, ChunkyScene, Orientation, RenderPlan, WorldRef, Xyz, chunky_orientation,
    round6,
};

/// Where the record lives inside a campaign directory.
pub const CAMERAS_FILE: &str = "design/cameras.json";

/// The file `--bracket` writes the candidates into, in the output directory.
pub const CANDIDATES_FILE: &str = "candidates.json";

/// A draft frame is the stated one divided by this on each side.
pub const DRAFT_DIVISOR: u32 = 4;

/// A draft frame's sample target: enough to judge what is in frame, never
/// enough to judge a picture. Measured on a torch-lit hall (doune's great hall,
/// 400x225, exposure 8): at 16 samples the frame is scattered sparks on black and
/// what is in it cannot be read; at 128 the walls, tables and hearth read, in
/// about 20 seconds on a ten-core machine.
pub const DRAFT_SPP: u32 = 128;

/// The record: every showcase camera of one campaign.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraSheet {
    pub campaign_id: String,
    pub cameras: Vec<Camera>,
}

/// One camera. Field order is alphabetical, so the record a tool writes is
/// already in the canonical key order `delvec fmt` holds campaign JSON to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Camera {
    /// The `design.json` row whose approved image this camera answers.
    pub answers: String,
    /// The camera's exposure: `1.0` renders the light as the path tracer
    /// measures it.
    pub exposure: f64,
    /// Vertical field of view, degrees.
    pub fov: f64,
    /// Frame height, pixels.
    pub height: u32,
    /// The camera's name: lowercase letters, digits, `.`, `+`, `-`.
    pub name: String,
    /// Minecraft pitch, degrees: positive looks down.
    pub pitch: f64,
    /// The lens, world blocks.
    pub pos: [f64; 3],
    /// Who placed the camera: an estimate, or a person standing in the game.
    pub source: Source,
    /// Path-tracing sample target.
    pub spp: u32,
    /// Frame width, pixels.
    pub width: u32,
    /// Minecraft yaw, degrees: 0 south, 90 west, 180 north, 270 east.
    pub yaw: f64,
}

/// Who placed a camera. A hand camera is final: no estimate is written over it
/// ([`place`]), and only deleting its row frees the name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// Derived by a tool or an agent: a panorama fit, a bracket candidate, a
    /// camera estimated from an approved image.
    Estimated,
    /// Captured in the running game where a person stood and looked
    /// (`/trigger dw.cam`, harvested by `delvec harvest`).
    Hand,
}

impl Source {
    /// The record's own spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Estimated => "estimated",
            Source::Hand => "hand",
        }
    }
}

/// Minecraft yaw → the render plan's yaw (`atan2(-dz, dx)`: 0 east, 90 north),
/// in `(-180, 180]`.
///
/// Minecraft looks along `(-sin yaw, 0, cos yaw)`; the plan's yaw of that vector
/// is `atan2(-cos yaw, -sin yaw)`, which is `-yaw - 90`.
pub fn plan_yaw(minecraft_yaw: f64) -> f64 {
    let y = (-minecraft_yaw - 90.0).rem_euclid(360.0);
    if y > 180.0 { y - 360.0 } else { y }
}

/// The render plan's yaw → Minecraft yaw, in `[0, 360)`: the inverse of
/// [`plan_yaw`].
pub fn minecraft_yaw(plan_yaw_deg: f64) -> f64 {
    (-plan_yaw_deg - 90.0).rem_euclid(360.0)
}

/// Is `name` a legal camera name?
fn legal_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '+' | '-'))
}

impl Camera {
    /// Every refusal a single camera can earn, named by field.
    fn check(&self) -> Result<(), String> {
        if !legal_name(&self.name) {
            return Err(format!(
                "name `{}` is not a camera name: lowercase letters, digits, `.`, `+` and `-`, \
                 starting with a letter or digit",
                self.name
            ));
        }
        if self.answers.trim().is_empty() {
            return Err(
                "`answers` is empty. Every showcase camera is judged against an approved image: \
                 name the `design.json` row it answers"
                    .to_string(),
            );
        }
        if !(self.exposure.is_finite() && self.exposure > 0.0) {
            return Err(format!(
                "`exposure` {} is not an exposure: a positive number, 1.0 for the light as measured",
                self.exposure
            ));
        }
        if !self.pos.iter().all(|v| v.is_finite()) {
            return Err(format!("`pos` {:?} is not three finite numbers", self.pos));
        }
        if !self.yaw.is_finite() {
            return Err("`yaw` is not a finite number".to_string());
        }
        if !(self.pitch.is_finite() && (-90.0..=90.0).contains(&self.pitch)) {
            return Err(format!(
                "`pitch` {} is outside -90..=90 (positive looks down)",
                self.pitch
            ));
        }
        if !(self.fov.is_finite() && self.fov > 0.0 && self.fov < 180.0) {
            return Err(format!(
                "`fov` {} is not a vertical field of view: it is degrees, above 0 and below 180",
                self.fov
            ));
        }
        if self.width == 0 || self.height == 0 {
            return Err(format!(
                "a {}x{} frame has no pixels",
                self.width, self.height
            ));
        }
        if self.spp == 0 {
            return Err("`spp` is 0: a path trace with no samples is an empty frame".to_string());
        }
        Ok(())
    }

    /// The horizontal unit vector the camera looks along, `(dx, dz)`.
    fn heading(&self) -> (f64, f64) {
        let y = self.yaw.to_radians();
        (-y.sin(), y.cos())
    }
}

/// Parse and check a record. A document that is not the record, a camera that
/// breaks a rule, and two cameras of one name are each refused ([`DW_INPUT`]).
pub fn parse_sheet(bytes: &[u8]) -> Result<CameraSheet, Diagnostic> {
    let sheet: CameraSheet = serde_json::from_slice(bytes)
        .map_err(|e| Diagnostic::error(DW_INPUT, format!("parse {CAMERAS_FILE}: {e}")))?;
    let mut seen = BTreeSet::new();
    for cam in &sheet.cameras {
        cam.check().map_err(|why| {
            Diagnostic::error(
                DW_INPUT,
                format!("{CAMERAS_FILE}: camera `{}`: {why}", cam.name),
            )
        })?;
        if !seen.insert(cam.name.as_str()) {
            return Err(Diagnostic::error(
                DW_INPUT,
                format!(
                    "{CAMERAS_FILE}: two cameras are named `{}`; a name is a scene's identity",
                    cam.name
                ),
            ));
        }
    }
    if sheet.cameras.is_empty() {
        return Err(Diagnostic::error(
            DW_INPUT,
            format!("{CAMERAS_FILE} states no camera, so there is nothing to emit"),
        ));
    }
    Ok(sheet)
}

/// The names of the approved-image rows a `design.json` carries.
pub fn reference_names(design_json: &[u8]) -> Result<Vec<String>, Diagnostic> {
    let doc: serde_json::Value = serde_json::from_slice(design_json)
        .map_err(|e| Diagnostic::error(DW_INPUT, format!("parse design.json: {e}")))?;
    let rows = doc
        .get("content")
        .and_then(|c| c.get("references"))
        .and_then(|r| r.as_array())
        .ok_or_else(|| {
            Diagnostic::error(
                DW_INPUT,
                "design.json carries no `content.references`, so no camera can answer an \
                 approved image",
            )
        })?;
    Ok(rows
        .iter()
        .filter_map(|r| r.get("name").and_then(|n| n.as_str()).map(str::to_string))
        .collect())
}

/// Every camera answers a row that exists. Returns the rows no camera answers,
/// in the design's own order.
pub fn bind_answers(sheet: &CameraSheet, rows: &[String]) -> Result<Vec<String>, Diagnostic> {
    for cam in &sheet.cameras {
        if !rows.iter().any(|r| r == &cam.answers) {
            return Err(Diagnostic::error(
                DW_INPUT,
                format!(
                    "{CAMERAS_FILE}: camera `{}` answers `{}`, which is not a row of design.json. \
                     Rows: {}",
                    cam.name,
                    cam.answers,
                    if rows.is_empty() {
                        "none".to_string()
                    } else {
                        rows.join(", ")
                    }
                ),
            ));
        }
    }
    Ok(rows
        .iter()
        .filter(|r| !sheet.cameras.iter().any(|c| &c.answers == *r))
        .cloned()
        .collect())
}

/// The steps a bracket moves a camera by. A zero step is not bracketed.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Bracket {
    /// Degrees of yaw each way.
    pub yaw: f64,
    /// Degrees of pitch each way.
    pub pitch: f64,
    /// Degrees of field of view each way.
    pub fov: f64,
    /// Blocks along the horizontal heading, forward and back.
    pub dolly: f64,
    /// Blocks sideways, to the camera's right and left.
    pub truck: f64,
    /// Blocks up and down.
    pub rise: f64,
}

impl Bracket {
    /// Parse `yaw=8,pitch=4,fov=10,dolly=6,truck=3,rise=3`; every key optional, every step
    /// a positive number, at least one given.
    pub fn parse(spec: &str) -> Result<Bracket, String> {
        let mut b = Bracket::default();
        let mut any = false;
        for part in spec.split(',').filter(|p| !p.is_empty()) {
            let (key, value) = part
                .split_once('=')
                .ok_or_else(|| format!("`{part}` is not key=step"))?;
            let step: f64 = value
                .parse()
                .map_err(|_| format!("`{part}`: `{value}` is not a number"))?;
            if !(step.is_finite() && step > 0.0) {
                return Err(format!("`{part}`: a step is a positive number"));
            }
            let slot = match key {
                "yaw" => &mut b.yaw,
                "pitch" => &mut b.pitch,
                "fov" => &mut b.fov,
                "dolly" => &mut b.dolly,
                "truck" => &mut b.truck,
                "rise" => &mut b.rise,
                other => {
                    return Err(format!(
                        "`{other}` is not a bracket key: yaw, pitch, fov, dolly, truck, rise"
                    ));
                }
            };
            *slot = step;
            any = true;
        }
        if !any {
            return Err(
                "a bracket names at least one of yaw, pitch, fov, dolly, truck, rise".to_string(),
            );
        }
        Ok(b)
    }

    /// The camera and its candidates, the stated one first. Each candidate moves
    /// ONE thing by one step, so the page of them says which change helped.
    /// A candidate that would leave the legal range (a pitch past vertical, a
    /// field of view at 0) is not emitted.
    pub fn candidates(&self, cam: &Camera) -> Vec<Camera> {
        let mut out = vec![cam.clone()];
        let fmt = |v: f64| {
            let r = round6(v);
            if r.fract() == 0.0 {
                format!("{}", r as i64)
            } else {
                format!("{r}")
            }
        };
        let (dx, dz) = cam.heading();
        for sign in [1.0f64, -1.0] {
            let tag = if sign > 0.0 { "+" } else { "-" };
            let mut push = |key: &str, step: f64, apply: &dyn Fn(&mut Camera)| {
                if step == 0.0 {
                    return;
                }
                let mut c = cam.clone();
                apply(&mut c);
                c.name = format!("{}.{key}{tag}{}", cam.name, fmt(step));
                // A moved camera is not where anybody stood.
                c.source = Source::Estimated;
                if c.check().is_ok() {
                    out.push(c);
                }
            };
            push("yaw", self.yaw, &|c| {
                c.yaw = round6(c.yaw + sign * self.yaw)
            });
            push("pitch", self.pitch, &|c| {
                c.pitch = round6(c.pitch + sign * self.pitch)
            });
            push("fov", self.fov, &|c| {
                c.fov = round6(c.fov + sign * self.fov)
            });
            push("dolly", self.dolly, &|c| {
                c.pos[0] = round6(c.pos[0] + sign * self.dolly * dx);
                c.pos[2] = round6(c.pos[2] + sign * self.dolly * dz);
            });
            // The camera's right is the heading turned a quarter toward east
            // from south: (-dz, dx).
            push("truck", self.truck, &|c| {
                c.pos[0] = round6(c.pos[0] - sign * self.truck * dz);
                c.pos[2] = round6(c.pos[2] + sign * self.truck * dx);
            });
            push("rise", self.rise, &|c| {
                c.pos[1] = round6(c.pos[1] + sign * self.rise)
            });
        }
        out
    }
}

/// The scene stem of a camera: `<campaign>_camera_<name>`, with `_draft` when
/// the frame is a draft, so a draft never shares a name — and so never a cache —
/// with the frame it drafts.
pub fn camera_stem(campaign_id: &str, name: &str, draft: bool) -> String {
    let shot = if draft {
        format!("camera_{name}_draft")
    } else {
        format!("camera_{name}")
    };
    scene::scene_file_stem(campaign_id, &shot)
}

/// A frame as Chunky reads it: where, which way, how wide, how big, how many
/// samples. The one thing every world scene of a stated or solved camera is
/// built from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub pos: [f64; 3],
    /// The render plan's yaw convention (`atan2(-dz, dx)`).
    pub plan_yaw_deg: f64,
    /// Positive looks down.
    pub pitch_deg: f64,
    /// Vertical field of view.
    pub fov_deg: f64,
    pub exposure: f64,
    pub width: u32,
    pub height: u32,
    pub spp: u32,
}

impl Frame {
    /// The frame a camera states, optionally drafted.
    pub fn of(cam: &Camera, draft: bool) -> Frame {
        let (width, height, spp) = if draft {
            (
                (cam.width / DRAFT_DIVISOR).max(1),
                (cam.height / DRAFT_DIVISOR).max(1),
                cam.spp.min(DRAFT_SPP),
            )
        } else {
            (cam.width, cam.height, cam.spp)
        };
        Frame {
            pos: cam.pos,
            plan_yaw_deg: plan_yaw(cam.yaw),
            pitch_deg: cam.pitch,
            fov_deg: cam.fov,
            exposure: cam.exposure,
            width,
            height,
            spp,
        }
    }
}

/// The Chunky scene of one frame of the assembled world: the plan's hour as the
/// sun, the layout and whatever ground its horizon built as the loaded chunks,
/// the ocean plane on an ocean horizon. [`crate::compiler::view::panorama`]
/// emits through this too, so a solved camera and a stated one are one scene
/// shape.
pub(crate) fn world_scene(
    plan: &RenderPlan,
    stem: &str,
    frame: &Frame,
    world_path: &str,
) -> Result<ChunkyScene, Diagnostic> {
    let sky = scene::plan_sky(plan)?;
    let (loaded_min, loaded_max) = scene::loaded_extent(&plan.layout_aabb, plan.horizon);
    let o = chunky_orientation(frame.plan_yaw_deg, frame.pitch_deg);
    Ok(ChunkyScene {
        sdf_version: 9,
        name: stem.to_string(),
        width: frame.width,
        height: frame.height,
        y_clip_min: (loaded_min[1] - 8).max(-64),
        y_clip_max: (loaded_max[1] + 16).min(320),
        exposure: round6(frame.exposure),
        postprocess: "GAMMA",
        output_mode: "PNG",
        render_time: 0,
        spp: 0,
        spp_target: frame.spp,
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
        sun: Some(scene::sun_at(sky.daytime_ticks)),
        // A showcase frame is the world as a player sees it at the declared
        // hour; the night-vision review emulation never applies to it.
        materials: None,
        delvewright_review_policy: None,
        world: WorldRef {
            path: world_path.to_string(),
            dimension: 0,
        },
        camera: ChunkyCamera {
            name: "camera 1",
            position: Xyz {
                x: round6(frame.pos[0]),
                y: round6(frame.pos[1]),
                z: round6(frame.pos[2]),
            },
            orientation: Orientation {
                roll: round6(o.roll),
                pitch: round6(o.pitch),
                yaw: round6(o.yaw),
            },
            projection_mode: "PINHOLE",
            fov: round6(frame.fov_deg),
        },
        chunk_list: scene::chunk_list(loaded_min, loaded_max),
    }
    .with_water_world(scene::water_world(plan.horizon)))
}

/// What one emission produced.
#[derive(Debug, Clone, PartialEq)]
pub struct Emission {
    /// `(file name, bytes)` per scene, in record order (stated camera, then its
    /// candidates).
    pub scenes: Vec<(String, Vec<u8>)>,
    /// Every camera a scene was emitted for, in the same order — the candidates
    /// file when bracketed.
    pub cameras: Vec<Camera>,
}

/// Options for [`emit`].
#[derive(Debug, Clone, Default)]
pub struct EmitOptions {
    /// Absolute path of the world save the scenes load.
    pub world_path: String,
    /// Only these cameras, by name; empty = every camera.
    pub only: Vec<String>,
    pub bracket: Option<Bracket>,
    pub draft: bool,
}

/// How close, in blocks, a block may come to the lens before a frame is flagged.
/// A pinhole camera has no near plane, so a lens inside a block, or grazing one,
/// draws that block's inside faces or a sliver of it across a corner of the frame.
pub const LENS_CLEARANCE: f64 = 0.25;

/// The first occupied cell (in `x`, `y`, `z` order) that holds the lens or comes
/// within [`LENS_CLEARANCE`] of it, by `occupied` — `None` when the lens stands
/// clear. The query counts every placed block as a full cube, so a torch or a
/// carpet beside the lens is flagged too: that can only call a frame suspect.
pub fn lens_obstruction(pos: [f64; 3], occupied: impl Fn([i32; 3]) -> bool) -> Option<[i32; 3]> {
    let base = [
        pos[0].floor() as i32,
        pos[1].floor() as i32,
        pos[2].floor() as i32,
    ];
    for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                let cell = [base[0] + dx, base[1] + dy, base[2] + dz];
                if !occupied(cell) {
                    continue;
                }
                let gap2: f64 = (0..3)
                    .map(|a| {
                        let (lo, hi) = (f64::from(cell[a]), f64::from(cell[a]) + 1.0);
                        let d = (lo - pos[a]).max(pos[a] - hi).max(0.0);
                        d * d
                    })
                    .sum();
                if gap2 <= LENS_CLEARANCE * LENS_CLEARANCE {
                    return Some(cell);
                }
            }
        }
    }
    None
}

/// The campaign a build's `render-plan.json` was written for.
pub fn plan_campaign_id(plan_json: &[u8]) -> Result<String, Diagnostic> {
    Ok(scene::parse_plan(plan_json)?.campaign_id)
}

/// The file a preview of a camera is written to: `<stem>_preview.png`.
pub fn preview_file(campaign_id: &str, name: &str) -> String {
    format!("{}_preview.png", camera_stem(campaign_id, name, false))
}

/// A preview frame is the stated one divided by this on each side.
pub const PREVIEW_DIVISOR: u32 = 2;

/// The cameras one run frames, in record order: the record's (or the `--only`
/// selection), each followed by its bracket candidates. Refuses a record for
/// another campaign than the build's, and an `--only` name the record lacks.
pub fn selected(
    campaign_id: &str,
    sheet: &CameraSheet,
    only: &[String],
    bracket: Option<&Bracket>,
) -> Result<Vec<Camera>, Diagnostic> {
    if campaign_id != sheet.campaign_id {
        return Err(Diagnostic::error(
            DW_INPUT,
            format!(
                "{CAMERAS_FILE} is for `{}` and the build is `{campaign_id}`: a camera's position \
                 means something only in the world it was placed in",
                sheet.campaign_id
            ),
        ));
    }
    for name in only {
        if !sheet.cameras.iter().any(|c| &c.name == name) {
            let names: Vec<&str> = sheet.cameras.iter().map(|c| c.name.as_str()).collect();
            return Err(Diagnostic::error(
                DW_INPUT,
                format!(
                    "--only `{name}` names no camera in {CAMERAS_FILE}. Cameras: {}",
                    names.join(", ")
                ),
            ));
        }
    }
    let mut out = Vec::new();
    for cam in &sheet.cameras {
        if !only.is_empty() && !only.contains(&cam.name) {
            continue;
        }
        match bracket {
            Some(b) => out.extend(b.candidates(cam)),
            None => out.push(cam.clone()),
        }
    }
    Ok(out)
}

/// Emit the scenes of a record against a build's `render-plan.json`.
/// Byte-deterministic (ADR-0006): the same plan, record and options give the
/// same bytes.
pub fn emit(
    plan_json: &[u8],
    sheet: &CameraSheet,
    opts: &EmitOptions,
) -> Result<Emission, Diagnostic> {
    let plan = scene::parse_plan(plan_json)?;
    let cameras = selected(&plan.campaign_id, sheet, &opts.only, opts.bracket.as_ref())?;
    let mut out = Emission {
        scenes: Vec::new(),
        cameras: Vec::new(),
    };
    for c in cameras {
        let stem = camera_stem(&plan.campaign_id, &c.name, opts.draft);
        let scene = world_scene(&plan, &stem, &Frame::of(&c, opts.draft), &opts.world_path)?;
        out.scenes.push((format!("{stem}.json"), scene.to_bytes()?));
        out.cameras.push(c);
    }
    Ok(out)
}

/// What a row of the record is written from.
#[derive(Debug, Clone, PartialEq)]
pub enum Placement {
    /// A camera a tool estimated, taken verbatim from a record-format file
    /// (`candidates.json`): pose, lens, exposure and frame. Written as
    /// `estimated`.
    Estimate(Camera),
    /// A pose a person captured in the running game (`camera-report.json`),
    /// with the field of view they stated. Written as `hand`; the row's frame
    /// and exposure are kept.
    Hand {
        pos: [f64; 3],
        yaw: f64,
        pitch: f64,
        fov: f64,
    },
}

/// What [`place`] did to the record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placed {
    /// A new row was appended.
    Added,
    /// An existing row's camera was replaced.
    Replaced,
}

/// Write one row of the record: the one writer of `design/cameras.json`.
///
/// - A row placed by hand is **final**: an estimate written over it is refused,
///   naming the row. Only deleting the row ([`delete`]) frees its name; a new
///   hand pose replaces it.
/// - A row answers one approved image. `answers` names it for a new row (an
///   estimate may carry its own), and must equal the row's when given for an
///   existing one: aiming a camera at another picture is a new row.
/// - A new hand row takes the showcase frame defaults (`panorama`'s: 1600×900,
///   300 spp) and exposure 1.0; a replaced hand row keeps the row's frame and
///   exposure, because a person placed the pose, not the exposure.
///
/// `sheet` is the record as it stands (`None` when the campaign has none yet),
/// `campaign_id` the campaign the placement belongs to.
pub fn place(
    sheet: Option<CameraSheet>,
    campaign_id: &str,
    name: &str,
    answers: Option<&str>,
    placement: Placement,
) -> Result<(CameraSheet, Placed), Diagnostic> {
    let refuse = |why: String| Diagnostic::error(DW_INPUT, format!("{CAMERAS_FILE}: {why}"));
    let mut sheet = sheet.unwrap_or_else(|| CameraSheet {
        campaign_id: campaign_id.to_string(),
        cameras: Vec::new(),
    });
    if sheet.campaign_id != campaign_id {
        return Err(refuse(format!(
            "the record is for `{}` and this camera was placed in `{campaign_id}`: a position \
             means something only in the world it was placed in",
            sheet.campaign_id
        )));
    }
    let existing = sheet.cameras.iter().position(|c| c.name == name);
    let row = match (existing, &placement) {
        (Some(i), _) => {
            let old = &sheet.cameras[i];
            if old.source == Source::Hand && matches!(placement, Placement::Estimate(_)) {
                return Err(refuse(format!(
                    "camera `{name}` was placed by hand, and an estimate is never written over a \
                     hand camera. Delete the row (`delvec place-camera … --name {name} --delete`) \
                     only when the person who placed it asks"
                )));
            }
            if let Some(a) = answers
                && a != old.answers
            {
                return Err(refuse(format!(
                    "camera `{name}` answers `{}`, not `{a}`: a camera is an answer to one \
                     picture, so aiming at another picture is a new row with its own name",
                    old.answers
                )));
            }
            match placement {
                Placement::Estimate(est) => Camera {
                    answers: old.answers.clone(),
                    name: name.to_string(),
                    source: Source::Estimated,
                    ..est
                },
                Placement::Hand {
                    pos,
                    yaw,
                    pitch,
                    fov,
                } => Camera {
                    fov,
                    pitch,
                    pos,
                    source: Source::Hand,
                    yaw,
                    ..old.clone()
                },
            }
        }
        (None, Placement::Estimate(est)) => {
            let answers = answers.unwrap_or(&est.answers).to_string();
            Camera {
                answers,
                name: name.to_string(),
                source: Source::Estimated,
                ..est.clone()
            }
        }
        (
            None,
            Placement::Hand {
                pos,
                yaw,
                pitch,
                fov,
            },
        ) => {
            let Some(answers) = answers else {
                return Err(refuse(format!(
                    "camera `{name}` is a new row, so it needs `--answers <design.json row>`: every \
                     showcase camera is judged against an approved image"
                )));
            };
            Camera {
                answers: answers.to_string(),
                exposure: 1.0,
                fov: *fov,
                height: crate::compiler::view::panorama::DEFAULT_HEIGHT,
                name: name.to_string(),
                pitch: *pitch,
                pos: *pos,
                source: Source::Hand,
                spp: crate::compiler::view::panorama::DEFAULT_SPP,
                width: crate::compiler::view::panorama::DEFAULT_WIDTH,
                yaw: *yaw,
            }
        }
    };
    row.check()
        .map_err(|why| refuse(format!("camera `{name}`: {why}")))?;
    let placed = match existing {
        Some(i) => {
            sheet.cameras[i] = row;
            Placed::Replaced
        }
        None => {
            sheet.cameras.push(row);
            Placed::Added
        }
    };
    Ok((sheet, placed))
}

/// Delete one row of the record. The empty record that deleting the last row
/// leaves is `None`: the file is removed rather than written as a record that
/// states no camera.
pub fn delete(mut sheet: CameraSheet, name: &str) -> Result<Option<CameraSheet>, Diagnostic> {
    let Some(i) = sheet.cameras.iter().position(|c| c.name == name) else {
        let names: Vec<&str> = sheet.cameras.iter().map(|c| c.name.as_str()).collect();
        return Err(Diagnostic::error(
            DW_INPUT,
            format!(
                "{CAMERAS_FILE} has no camera `{name}`. Cameras: {}",
                names.join(", ")
            ),
        ));
    };
    sheet.cameras.remove(i);
    Ok((!sheet.cameras.is_empty()).then_some(sheet))
}

/// The record's bytes as every tool writes them: canonical key order (every
/// map sorted, which is the order `delvec fmt` holds campaign JSON to), pretty,
/// with a trailing newline.
pub fn sheet_bytes(sheet: &CameraSheet) -> Result<Vec<u8>, Diagnostic> {
    let value = serde_json::to_value(sheet)
        .map_err(|e| Diagnostic::error(DW_INPUT, format!("serialize {CAMERAS_FILE}: {e}")))?;
    let mut bytes = serde_json::to_vec_pretty(&value)
        .map_err(|e| Diagnostic::error(DW_INPUT, format!("serialize {CAMERAS_FILE}: {e}")))?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// The lowest world Y a block can occupy on the pinned overworld.
pub const WORLD_MIN_Y: f64 = -64.0;
/// One above the highest world Y a block can occupy on the pinned overworld.
pub const WORLD_MAX_Y: f64 = 320.0;

/// Why a showcase camera does not photograph the scene.
#[derive(Debug, Clone, PartialEq)]
pub enum ShowcaseRefusal {
    /// The record itself is refused (`DW0721`).
    Record(Diagnostic),
    /// A camera's eye cell holds a block, or its frame holds none of the loaded
    /// world (`DW0724`).
    Camera(String),
}

/// **Every showcase camera photographs the scene** — the build's proof over
/// `design/cameras.json`, asked of the final assembled world and of the plan
/// the scenes are emitted from. Returns how many cameras it examined.
///
/// Two shapes of one rule:
///
/// 1. **The eye is not inside a block**: `occupied(cell)` of the lens's own cell
///    is false. A camera is never moved to make this hold — a hand camera is
///    where the person stood, an estimate is where the agent put it — so a
///    violation is a refusal naming the row.
/// 2. **The frame holds the loaded world**: the lens lies within the world's
///    build height, and the ray along its view meets the box the scene's chunk
///    list is cut from ([`scene::loaded_extent`], widened to whole blocks). A
///    camera looking away from everything the scene loads renders an empty sky
///    at exit 0.
///
/// A record for another campaign is refused as `delvec cameras` refuses it.
pub(crate) fn prove_showcase(
    record: &[u8],
    plan: &RenderPlan,
    occupied: impl Fn([i32; 3]) -> bool,
) -> Result<usize, ShowcaseRefusal> {
    let sheet = parse_sheet(record).map_err(ShowcaseRefusal::Record)?;
    selected(&plan.campaign_id, &sheet, &[], None).map_err(ShowcaseRefusal::Record)?;
    let (lmin, lmax) = scene::loaded_extent(&plan.layout_aabb, plan.horizon);
    let lo = [f64::from(lmin[0]), f64::from(lmin[1]), f64::from(lmin[2])];
    let hi = [
        f64::from(lmax[0]) + 1.0,
        f64::from(lmax[1]) + 1.0,
        f64::from(lmax[2]) + 1.0,
    ];
    for cam in &sheet.cameras {
        let who = match cam.source {
            Source::Hand => format!("showcase camera `{}` (placed by hand)", cam.name),
            Source::Estimated => format!("showcase camera `{}` (estimated)", cam.name),
        };
        let remedy = match cam.source {
            Source::Hand => {
                "The geometry is what the picture is of, so it is never moved: stand somewhere \
                 else and fire `/trigger dw.cam` again, then write the row with `delvec \
                 place-camera`"
            }
            Source::Estimated => {
                "The geometry is what the picture is of, so it is never moved: move the camera \
                 (`delvec cameras --preview` draws it and names the block beside the lens)"
            }
        };
        let cell = [
            cam.pos[0].floor() as i32,
            cam.pos[1].floor() as i32,
            cam.pos[2].floor() as i32,
        ];
        if occupied(cell) {
            return Err(ShowcaseRefusal::Camera(format!(
                "{CAMERAS_FILE}: {who}: the eye cell {cell:?} is occupied (a solid block or water) \
                 in the assembled world, so the frame would render the inside of a block. {remedy}"
            )));
        }
        if !(WORLD_MIN_Y..WORLD_MAX_Y).contains(&cam.pos[1]) {
            return Err(ShowcaseRefusal::Camera(format!(
                "{CAMERAS_FILE}: {who}: the lens at y {} is outside the world's build height \
                 {WORLD_MIN_Y}..{WORLD_MAX_Y}. {remedy}",
                cam.pos[1]
            )));
        }
        let (y, p) = (cam.yaw.to_radians(), cam.pitch.to_radians());
        let dir = [-y.sin() * p.cos(), -p.sin(), y.cos() * p.cos()];
        if !ray_meets_box(cam.pos, dir, lo, hi) {
            return Err(ShowcaseRefusal::Camera(format!(
                "{CAMERAS_FILE}: {who}: looking along yaw {} pitch {} from {:?}, the view meets \
                 nothing the scene loads — the framed extent is {lmin:?}..{lmax:?} — so the frame \
                 would be empty sky. {remedy}",
                cam.yaw, cam.pitch, cam.pos
            )));
        }
    }
    Ok(sheet.cameras.len())
}

/// Does the ray from `origin` along `dir` (t ≥ 0) meet the box `lo..hi`?
fn ray_meets_box(origin: [f64; 3], dir: [f64; 3], lo: [f64; 3], hi: [f64; 3]) -> bool {
    let (mut t0, mut t1) = (0.0f64, f64::INFINITY);
    for a in 0..3 {
        if dir[a].abs() < 1e-12 {
            if origin[a] < lo[a] || origin[a] > hi[a] {
                return false;
            }
            continue;
        }
        let (mut near, mut far) = ((lo[a] - origin[a]) / dir[a], (hi[a] - origin[a]) / dir[a]);
        if near > far {
            std::mem::swap(&mut near, &mut far);
        }
        t0 = t0.max(near);
        t1 = t1.min(far);
        if t0 > t1 {
            return false;
        }
    }
    true
}

/// The candidates file: the record format, holding every emitted camera.
pub fn candidates_bytes(campaign_id: &str, cameras: &[Camera]) -> Result<Vec<u8>, Diagnostic> {
    let sheet = CameraSheet {
        campaign_id: campaign_id.to_string(),
        cameras: cameras.to_vec(),
    };
    let mut bytes = serde_json::to_vec_pretty(&sheet)
        .map_err(|e| Diagnostic::error(DW_INPUT, format!("serialize candidates: {e}")))?;
    bytes.push(b'\n');
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OCEAN: &[u8] = include_bytes!("../../../tests/fixtures/view/render-plan-ocean.json");
    const MINI: &[u8] = include_bytes!("../../../tests/fixtures/view/render-plan-mini.json");

    fn cam(name: &str) -> Camera {
        Camera {
            answers: "concept/quay".to_string(),
            exposure: 1.0,
            fov: 50.0,
            height: 900,
            name: name.to_string(),
            pitch: 10.0,
            pos: [10.5, 70.0, -4.25],
            source: Source::Estimated,
            spp: 300,
            width: 1600,
            yaw: 30.0,
        }
    }

    fn sheet(id: &str, cams: Vec<Camera>) -> CameraSheet {
        CameraSheet {
            campaign_id: id.to_string(),
            cameras: cams,
        }
    }

    /// The view direction Chunky computes from a stored orientation — the pinned
    /// core's basis (see `scene`'s module docs), written out here rather than
    /// taken from the emitter.
    fn chunky_forward(v: &serde_json::Value) -> [f64; 3] {
        let o = &v["camera"]["orientation"];
        let (yaw, pitch) = (o["yaw"].as_f64().unwrap(), o["pitch"].as_f64().unwrap());
        [
            yaw.cos() * pitch.sin(),
            -pitch.cos(),
            -yaw.sin() * pitch.sin(),
        ]
    }

    /// Minecraft's own look vector for an entity rotation.
    fn minecraft_forward(yaw: f64, pitch: f64) -> [f64; 3] {
        let (y, p) = (yaw.to_radians(), pitch.to_radians());
        [-y.sin() * p.cos(), -p.sin(), y.cos() * p.cos()]
    }

    /// **The record speaks Minecraft's rotation, and the scene looks where the
    /// game would.** At the four cardinals, the four diagonals, an arbitrary
    /// yaw, negative and past-360 yaws and several pitches, the direction Chunky
    /// derives from the emitted orientation is the direction Minecraft derives
    /// from the record's rotation.
    #[test]
    fn the_scene_looks_where_minecraft_would_look() {
        let mut judged = 0;
        for yaw in [
            0.0, 45.0, 90.0, 135.0, 180.0, 225.0, 270.0, 315.0, 213.7, -90.0, 450.0,
        ] {
            for pitch in [-60.0, -5.0, 0.0, 12.5, 45.0, 89.0] {
                let mut c = cam("x");
                c.yaw = yaw;
                c.pitch = pitch;
                c.answers = "a".into();
                let s = sheet("isle", vec![c]);
                let e = emit(OCEAN, &s, &EmitOptions::default()).unwrap();
                let v: serde_json::Value = serde_json::from_slice(&e.scenes[0].1).unwrap();
                let (got, want) = (chunky_forward(&v), minecraft_forward(yaw, pitch));
                for k in 0..3 {
                    assert!(
                        (got[k] - want[k]).abs() < 1e-5,
                        "yaw {yaw} pitch {pitch}: chunky {got:?} vs minecraft {want:?}"
                    );
                }
                judged += 1;
            }
        }
        assert_eq!(judged, 11 * 6);
    }

    #[test]
    fn a_lens_inside_or_grazing_a_block_is_named() {
        let wall = |c: [i32; 3]| c[0] == 5;
        assert_eq!(
            lens_obstruction([5.5, 70.5, 0.5], wall),
            Some([5, 70, 0]),
            "inside"
        );
        assert_eq!(
            lens_obstruction([4.8, 70.5, 0.5], wall),
            Some([5, 70, 0]),
            "grazing"
        );
        assert_eq!(lens_obstruction([4.7, 70.5, 0.5], wall), None, "0.3 clear");
        assert_eq!(lens_obstruction([2.5, 70.5, 0.5], wall), None);
        // Edge and corner neighbours count by true distance, not by cell.
        let corner = |c: [i32; 3]| c == [6, 71, 1];
        assert_eq!(lens_obstruction([5.9, 70.9, 0.9], corner), Some([6, 71, 1]));
        assert_eq!(lens_obstruction([5.7, 70.7, 0.7], corner), None);
    }

    #[test]
    fn the_cardinals_are_the_games() {
        // 0 south, 90 west, 180 north, 270 east — in the plan's convention
        // (0 east, 90 north).
        assert_eq!(plan_yaw(0.0), -90.0);
        assert_eq!(plan_yaw(90.0), 180.0);
        assert_eq!(plan_yaw(180.0), 90.0);
        assert_eq!(plan_yaw(270.0), 0.0);
        for y in [0.0, 37.5, 90.0, 213.25, 359.0] {
            assert!((minecraft_yaw(plan_yaw(y)) - y).abs() < 1e-9, "{y}");
        }
    }

    #[test]
    fn a_stated_camera_is_emitted_verbatim() {
        let s = sheet("isle", vec![cam("hero")]);
        let e = emit(
            OCEAN,
            &s,
            &EmitOptions {
                world_path: "/abs/world".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(e.scenes.len(), 1);
        let (file, bytes) = &e.scenes[0];
        assert_eq!(file, "isle_camera_hero.json");
        let v: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        assert_eq!(v["name"], "isle_camera_hero");
        assert_eq!(v["width"], 1600);
        assert_eq!(v["height"], 900);
        assert_eq!(v["sppTarget"], 300);
        assert_eq!(v["camera"]["fov"], 50.0);
        assert_eq!(v["camera"]["position"]["x"], 10.5);
        assert_eq!(v["camera"]["position"]["y"], 70.0);
        assert_eq!(v["camera"]["position"]["z"], -4.25);
        assert_eq!(v["world"]["path"], "/abs/world");
        // The ocean plane and the declared hour come from the build.
        assert_eq!(v["waterWorldEnabled"], true);
        assert!(v["sun"].is_object());
        assert!(v.get("materials").is_none());
    }

    /// Every stated field reaches a byte: perturb one, and the scene moves.
    #[test]
    fn every_stated_field_moves_the_scene() {
        let base = emit(
            OCEAN,
            &sheet("isle", vec![cam("a")]),
            &EmitOptions::default(),
        )
        .unwrap();
        type Edit = (&'static str, fn(&mut Camera));
        let edits: [Edit; 9] = [
            ("exposure", |c| c.exposure *= 2.0),
            ("pos.x", |c| c.pos[0] += 1.0),
            ("pos.y", |c| c.pos[1] += 1.0),
            ("pos.z", |c| c.pos[2] += 1.0),
            ("yaw", |c| c.yaw += 1.0),
            ("pitch", |c| c.pitch += 1.0),
            ("fov", |c| c.fov += 1.0),
            ("width", |c| c.width += 1),
            ("spp", |c| c.spp += 1),
        ];
        for (what, edit) in edits {
            let mut c = cam("a");
            edit(&mut c);
            let moved = emit(OCEAN, &sheet("isle", vec![c]), &EmitOptions::default()).unwrap();
            assert_ne!(base.scenes, moved.scenes, "{what} did not reach the scene");
        }
    }

    #[test]
    fn emission_is_byte_deterministic() {
        let s = sheet("isle", vec![cam("a"), cam("b")]);
        let opts = EmitOptions {
            bracket: Some(Bracket::parse("yaw=8,dolly=3").unwrap()),
            ..Default::default()
        };
        assert_eq!(
            emit(OCEAN, &s, &opts).unwrap(),
            emit(OCEAN, &s, &opts).unwrap()
        );
    }

    #[test]
    fn a_record_for_another_campaign_is_refused() {
        let err = emit(
            OCEAN,
            &sheet("mini", vec![cam("a")]),
            &EmitOptions::default(),
        )
        .unwrap_err();
        assert_eq!(err.code, DW_INPUT);
        assert!(err.message.contains("`mini`"), "{err:?}");
    }

    #[test]
    fn a_plan_with_no_hour_is_refused() {
        let no_sky =
            br#"{"campaign_id":"c","layout_aabb":{"min":[0,64,0],"max":[1,65,1]},"shots":[]}"#;
        let err = emit(no_sky, &sheet("c", vec![cam("a")]), &EmitOptions::default()).unwrap_err();
        assert_eq!(err.code, DW_INPUT);
    }

    #[test]
    fn the_record_refuses_what_is_not_a_camera() {
        let ok = serde_json::to_value(sheet("c", vec![cam("a")])).unwrap();
        assert!(parse_sheet(ok.to_string().as_bytes()).is_ok());
        let edits: [(&str, serde_json::Value); 12] = [
            ("/cameras/0/exposure", 0.0.into()),
            ("/cameras/0/name", "Hero".into()),
            ("/cameras/0/answers", "".into()),
            ("/cameras/0/pitch", 91.0.into()),
            ("/cameras/0/fov", 0.0.into()),
            ("/cameras/0/fov", 180.0.into()),
            ("/cameras/0/width", 0.into()),
            ("/cameras/0/spp", 0.into()),
            ("/cameras/0/pos", serde_json::json!([1.0, 2.0])),
            ("/cameras/0/lens", 35.into()),
            ("/cameras/0/source", "guessed".into()),
            ("/cameras", serde_json::json!([])),
        ];
        for (path, value) in edits {
            let mut doc = ok.clone();
            let (parent, key) = path.rsplit_once('/').unwrap();
            let target = if parent.is_empty() {
                &mut doc
            } else {
                doc.pointer_mut(parent).unwrap()
            };
            target[key] = value;
            let err = parse_sheet(doc.to_string().as_bytes()).unwrap_err();
            assert_eq!(err.code, DW_INPUT, "{path}");
        }
        let twice = sheet("c", vec![cam("a"), cam("a")]);
        let err = parse_sheet(serde_json::to_string(&twice).unwrap().as_bytes()).unwrap_err();
        assert!(err.message.contains("two cameras"), "{err:?}");
    }

    #[test]
    fn a_camera_answers_a_row_of_the_design() {
        let design = br#"{"campaign_id":"c","content":{"references":[
            {"name":"concept/quay","shows":"x","time":"day","weather":"clear"},
            {"name":"concept/hall","shows":"y","time":"day","weather":"clear"}]},
            "stage":"design"}"#;
        let rows = reference_names(design).unwrap();
        let unanswered = bind_answers(&sheet("c", vec![cam("a")]), &rows).unwrap();
        assert_eq!(unanswered, vec!["concept/hall".to_string()]);
        let mut stray = cam("a");
        stray.answers = "concept/cellar".into();
        let err = bind_answers(&sheet("c", vec![stray]), &rows).unwrap_err();
        assert_eq!(err.code, DW_INPUT);
        assert!(err.message.contains("concept/hall"), "{err:?}");
    }

    /// A bracket moves one thing at a time, by the stated step, and says which
    /// in the candidate's name; the stated camera leads.
    #[test]
    fn a_bracket_moves_one_thing_per_candidate() {
        let b = Bracket::parse("yaw=8,pitch=4,fov=10,dolly=6,truck=2,rise=3").unwrap();
        let c = cam("hero");
        let set = b.candidates(&c);
        assert_eq!(set[0], c);
        assert_eq!(set.len(), 13);
        let named = |n: &str| set.iter().find(|x| x.name == n).unwrap().clone();
        assert_eq!(named("hero.yaw+8").yaw, 38.0);
        assert_eq!(named("hero.yaw-8").yaw, 22.0);
        assert_eq!(named("hero.pitch-4").pitch, 6.0);
        assert_eq!(named("hero.fov+10").fov, 60.0);
        assert_eq!(named("hero.rise+3").pos[1], 73.0);
        // Dolly moves along the heading: yaw 30 looks toward -X and +Z.
        let fwd = named("hero.dolly+6");
        let (dx, dz) = (fwd.pos[0] - c.pos[0], fwd.pos[2] - c.pos[2]);
        assert!((dx - -6.0 * 30f64.to_radians().sin()).abs() < 1e-5, "{dx}");
        assert!((dz - 6.0 * 30f64.to_radians().cos()).abs() < 1e-5, "{dz}");
        assert_eq!(fwd.pos[1], c.pos[1]);
        // Truck moves square to the heading, to the right on `+`: the camera's
        // right is the snapshot rasteriser's `right` negated, i.e. the direction
        // a frame's right edge lies in. Heading (dx, dz) turned to (-dz, dx).
        let side = named("hero.truck+2");
        let (sx, sz) = (side.pos[0] - c.pos[0], side.pos[2] - c.pos[2]);
        assert!(
            (sx * dx + sz * dz).abs() < 1e-5,
            "truck is square to the heading"
        );
        assert!((sx.hypot(sz) - 2.0).abs() < 1e-5);
        // Minecraft's own right for yaw 30: facing south-south-west, right is
        // west-north-west, (-cos yaw, -sin yaw).
        let y = 30f64.to_radians();
        assert!(
            (sx - -2.0 * y.cos()).abs() < 1e-5 && (sz - -2.0 * y.sin()).abs() < 1e-5,
            "{sx},{sz}"
        ); // Every candidate differs from the stated camera in exactly one field.
        for x in &set[1..] {
            let changed = [
                x.yaw != c.yaw,
                x.pitch != c.pitch,
                x.fov != c.fov,
                x.pos[1] != c.pos[1],
                x.pos[0] != c.pos[0] || x.pos[2] != c.pos[2],
            ];
            assert_eq!(changed.iter().filter(|b| **b).count(), 1, "{}", x.name);
        }
        // A candidate past the legal range is not emitted.
        let mut steep = cam("s");
        steep.pitch = 88.0;
        let set = Bracket::parse("pitch=4").unwrap().candidates(&steep);
        assert_eq!(
            set.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(),
            vec!["s", "s.pitch-4"]
        );
        for bad in ["", "yaw", "yaw=0", "yaw=-2", "zoom=3", "yaw=x"] {
            assert!(Bracket::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_draft_is_small_cheap_and_never_shares_a_name_with_its_frame() {
        let s = sheet("isle", vec![cam("a")]);
        let draft = emit(
            OCEAN,
            &s,
            &EmitOptions {
                draft: true,
                ..Default::default()
            },
        )
        .unwrap();
        let (file, bytes) = &draft.scenes[0];
        assert_eq!(file, "isle_camera_a_draft.json");
        let v: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        assert_eq!(v["width"], 400);
        assert_eq!(v["height"], 225);
        assert_eq!(v["sppTarget"], DRAFT_SPP);
        // The candidates file keeps the stated frame, so a pick is copied verbatim.
        assert_eq!(draft.cameras[0], cam("a"));
    }

    #[test]
    fn only_selects_and_refuses_a_name_it_does_not_have() {
        let s = sheet("mini", vec![cam("a"), cam("b")]);
        let e = emit(
            MINI,
            &s,
            &EmitOptions {
                only: vec!["b".into()],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(e.cameras.len(), 1);
        assert_eq!(e.cameras[0].name, "b");
        let err = emit(
            MINI,
            &s,
            &EmitOptions {
                only: vec!["c".into()],
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(err.code, DW_INPUT);
    }

    fn hand(pos: [f64; 3]) -> Placement {
        Placement::Hand {
            pos,
            yaw: -45.75,
            pitch: 12.1,
            fov: 70.0,
        }
    }

    /// **A hand camera is final.** An estimate over a `hand` row is refused
    /// naming the row; once the row is deleted the same write succeeds; a new
    /// hand pose replaces a hand row and keeps its frame and exposure.
    #[test]
    fn an_estimate_is_never_written_over_a_hand_camera() {
        let (sheet, placed) = place(
            None,
            "isle",
            "hero",
            Some("concept/quay"),
            hand([1.5, 70.62, -3.0]),
        )
        .unwrap();
        assert_eq!(placed, Placed::Added);
        let row = &sheet.cameras[0];
        assert_eq!(row.source, Source::Hand);
        assert_eq!(
            (row.pos, row.yaw, row.pitch, row.fov),
            ([1.5, 70.62, -3.0], -45.75, 12.1, 70.0)
        );
        assert_eq!(
            (row.width, row.height, row.spp, row.exposure),
            (1600, 900, 300, 1.0)
        );

        let estimate = Placement::Estimate(cam("hero.yaw+8"));
        let err = place(Some(sheet.clone()), "isle", "hero", None, estimate.clone()).unwrap_err();
        assert_eq!(err.code, DW_INPUT);
        assert!(err.message.contains("`hero` was placed by hand"), "{err:?}");

        // She re-fires `dw.cam`: the hand pose replaces the hand pose, and the
        // exposure the agent set for the room stays.
        let mut exposed = sheet.clone();
        exposed.cameras[0].exposure = 8.0;
        let (again, placed) =
            place(Some(exposed), "isle", "hero", None, hand([2.5, 71.0, -3.0])).unwrap();
        assert_eq!(placed, Placed::Replaced);
        assert_eq!(again.cameras[0].pos, [2.5, 71.0, -3.0]);
        assert_eq!(again.cameras[0].exposure, 8.0);

        // Deleted, the name is free, and the same estimate is written.
        assert_eq!(delete(sheet.clone(), "hero").unwrap(), None);
        let mut two = sheet.clone();
        two.cameras.push(cam("other"));
        let left = delete(two, "hero").unwrap().unwrap();
        let (written, placed) =
            place(Some(left), "isle", "hero", Some("concept/quay"), estimate).unwrap();
        assert_eq!(placed, Placed::Added);
        let row = written.cameras.iter().find(|c| c.name == "hero").unwrap();
        assert_eq!(row.source, Source::Estimated);
        assert_eq!(row.yaw, cam("x").yaw);
        assert!(delete(written, "nobody").is_err());
    }

    #[test]
    fn a_placement_answers_one_picture_in_one_world() {
        let err = place(None, "isle", "hero", None, hand([0.5, 70.0, 0.5])).unwrap_err();
        assert!(err.message.contains("--answers"), "{err:?}");
        let (sheet, _) = place(
            None,
            "isle",
            "hero",
            Some("concept/quay"),
            hand([0.5, 70.0, 0.5]),
        )
        .unwrap();
        let err = place(
            Some(sheet.clone()),
            "isle",
            "hero",
            Some("concept/hall"),
            hand([0.5, 70.0, 0.5]),
        )
        .unwrap_err();
        assert!(err.message.contains("new row"), "{err:?}");
        let err = place(
            Some(sheet.clone()),
            "mini",
            "hero",
            None,
            hand([0.5, 70.0, 0.5]),
        )
        .unwrap_err();
        assert!(err.message.contains("`isle`"), "{err:?}");
        let bad = Placement::Hand {
            pos: [0.5, 70.0, 0.5],
            yaw: 0.0,
            pitch: 0.0,
            fov: 0.0,
        };
        assert!(place(Some(sheet), "isle", "hero", None, bad).is_err());
    }

    /// The record a tool writes reads back as itself, in canonical key order.
    #[test]
    fn a_written_record_is_canonical_and_reads_back() {
        let (sheet, _) = place(
            None,
            "isle",
            "hero",
            Some("concept/quay"),
            hand([0.5, 70.0, 0.5]),
        )
        .unwrap();
        let bytes = sheet_bytes(&sheet).unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(text.find("\"cameras\"").unwrap() < text.find("\"campaign_id\"").unwrap());
        assert!(text.contains("\"source\": \"hand\""));
        assert_eq!(parse_sheet(&bytes).unwrap(), sheet);
    }

    /// Both shapes of the showcase proof, on the ocean fixture plan.
    #[test]
    fn a_showcase_camera_photographs_the_scene() {
        let plan = scene::parse_plan(OCEAN).unwrap();
        let (lmin, lmax) = scene::loaded_extent(&plan.layout_aabb, plan.horizon);
        let mid = [
            f64::from(lmin[0] + lmax[0]) / 2.0,
            f64::from(lmax[1]) + 20.0,
            f64::from(lmin[2] + lmax[2]) / 2.0,
        ];
        let record = |c: Camera| serde_json::to_vec(&sheet("isle", vec![c])).unwrap();
        let nothing = |_: [i32; 3]| false;
        // Above the middle of the layout, looking down: clear eye, full frame.
        let mut down = cam("down");
        down.pos = mid;
        down.pitch = 60.0;
        assert_eq!(prove_showcase(&record(down.clone()), &plan, nothing), Ok(1));
        // The same camera with a block at its eye.
        let cell = [
            mid[0].floor() as i32,
            mid[1].floor() as i32,
            mid[2].floor() as i32,
        ];
        for source in [Source::Hand, Source::Estimated] {
            let mut c = down.clone();
            c.source = source;
            let Err(ShowcaseRefusal::Camera(why)) =
                prove_showcase(&record(c), &plan, |x| x == cell)
            else {
                panic!("{source:?}: an occupied eye is refused");
            };
            assert!(why.contains("`down`") && why.contains("occupied"), "{why}");
            assert_eq!(why.contains("dw.cam"), source == Source::Hand, "{why}");
        }
        // Looking straight up from above the layout: empty sky.
        let mut up = down.clone();
        up.pitch = -60.0;
        let Err(ShowcaseRefusal::Camera(why)) = prove_showcase(&record(up), &plan, nothing) else {
            panic!("a frame of empty sky is refused");
        };
        assert!(why.contains("framed extent"), "{why}");
        // Above the build height.
        let mut high = down;
        high.pos[1] = 400.0;
        assert!(matches!(
            prove_showcase(&record(high), &plan, nothing),
            Err(ShowcaseRefusal::Camera(_))
        ));
        // A record for another world, and a record that is not one.
        let other = serde_json::to_vec(&sheet("mini", vec![cam("a")])).unwrap();
        assert!(matches!(
            prove_showcase(&other, &plan, nothing),
            Err(ShowcaseRefusal::Record(_))
        ));
        assert!(matches!(
            prove_showcase(b"{}", &plan, nothing),
            Err(ShowcaseRefusal::Record(_))
        ));
    }

    #[test]
    fn the_candidates_file_is_a_record() {
        let b = Bracket::parse("fov=5").unwrap();
        let cams = b.candidates(&cam("a"));
        let bytes = candidates_bytes("isle", &cams).unwrap();
        let back = parse_sheet(&bytes).unwrap();
        assert_eq!(back.cameras, cams);
    }
}
