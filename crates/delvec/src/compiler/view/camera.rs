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
//!   and the renderer's are both vertical — so a narrower frame from further back
//!   is a smaller number.
//! - There is no `exposure`: every frame is exposed at
//!   [`crate::compiler::view::frames::HQ_EXPOSURE`], derived by the engine. A
//!   record that states one is refused ([`DW_EXPOSURE_STATED`]); a room that
//!   reads too dark is lit, never exposed.
//! - `width`, `height` and `spp` are the frame and the path tracer's sample target:
//!   everything else the renderer needs to reproduce the frame comes from the
//!   build (the written world and its loaded chunks) or from the record (the sky).
//! - `answers` names the row of `design.json` whose approved image the camera is
//!   judged against, and is refused when it names no row.
//! - `sky` (optional, `{"time": "noon", "weather": "clear"}`) is the sky the
//!   picture is taken under when it is not the answered row's: absent, the camera
//!   renders under its row's `time` and `weather` ([`resolve_sky`], spec-0079), and
//!   a stated sky equal to the row's is refused.
//!
//! # Candidates
//!
//! An estimate is a start, not a frame. `--bracket yaw=8,pitch=4,fov=10,dolly=6,
//! truck=3,rise=3` emits, beside each camera, the camera moved each way by each step, and
//! writes every candidate into `candidates.json` in the same record format — so the
//! one a creator picks, by looking at it beside the approved image, is copied into
//! `design/cameras.json` verbatim. `--draft` renders any of them small and cheap.

use std::collections::BTreeSet;

use delvewright_dsl::{DwCode, ExitTier, WorldTime, WorldWeather};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::compiler::view::diag::{DW_INPUT, Diagnostic};
use crate::compiler::view::scene::{self, RenderPlan, SceneSky, round6};

/// Where the record lives inside a campaign directory.
pub const CAMERAS_FILE: &str = "design/cameras.json";

/// `DW0721` **as the build raises it**: the record's own rules — a document that
/// is the record, a camera that answers a row that exists — read by a run that
/// consumes the record and therefore reads all of it. The same code
/// `delvec cameras` refuses those rules under, at the build's tier, so the two
/// give one answer (spec-0070 §5).
pub const DW_RECORD_AT_BUILD: delvewright_dsl::DwCode =
    delvewright_dsl::DwCode::new(DW_INPUT, delvewright_dsl::ExitTier::Build);

delvewright_dsl::dw_code! {
    /// `DW1010`: **a camera record states an `exposure`.** A frame's exposure is
    /// derived by the engine ([`crate::compiler::view::frames::HQ_EXPOSURE`]), so a typed one is a
    /// derivation entered where only judgements belong, and the usual reason
    /// for typing one — a room too dark to read — is repaired by the room's
    /// lights. Refused by the record's one reader ([`parse_sheet`]), so every
    /// command that reads the record and `delvec build` refuse it alike. Build
    /// tier; `delvec cameras` and `delvec place-camera` exit 2 on it, as on
    /// `DW0721`.
    pub const DW_EXPOSURE_STATED: DwCode = DwCode::new("DW1010", ExitTier::Build);
}

/// The code a refusal of the record's reader carries at the build's tier:
/// [`DW_EXPOSURE_STATED`] keeps its own name, every other rule of the record
/// is [`DW_RECORD_AT_BUILD`].
pub fn record_code_at_build(d: &Diagnostic) -> DwCode {
    if d.code == DW_EXPOSURE_STATED.id() {
        DW_EXPOSURE_STATED
    } else {
        DW_RECORD_AT_BUILD
    }
}

/// The key under which [`record_schema`] names the file the record lives at, so
/// a tool reading the export (the gallery's coverage gate) finds the document
/// without a second list of paths.
pub const SCHEMA_FILE_KEY: &str = "x-delvewright-file";

/// **The camera record's JSON Schema** — `delvec schema --stage cameras`. A
/// campaign artifact, not a stage document, exported beside `prefab-metadata`
/// because it is a document a creator's tools write and an author reads the
/// shape of; it names its own path ([`SCHEMA_FILE_KEY`]).
pub fn record_schema() -> serde_json::Value {
    let mut v = serde_json::to_value(schemars::schema_for!(CameraSheet))
        .expect("the camera-record schema serializes to JSON");
    if let Some(obj) = v.as_object_mut() {
        obj.insert(
            SCHEMA_FILE_KEY.into(),
            serde_json::Value::String(CAMERAS_FILE.into()),
        );
    }
    v
}

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

/// The showcase camera record, `design/cameras.json`: every showcase camera of
/// one campaign. Written by `delvec place-camera` (or copied from a
/// record-format file `delvec cameras --bracket` or `delvec panorama` writes);
/// read by `delvec cameras` and `delvec build`. A campaign artifact, not a stage
/// document: it carries no `dsl_version` and no `stage`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CameraSheet {
    /// The campaign the cameras stand in: a position means something only in
    /// the world it was placed in.
    pub campaign_id: String,
    /// The cameras, each named once.
    pub cameras: Vec<Camera>,
}

/// One camera. Field order is alphabetical, so the record a tool writes is
/// already in the canonical key order `delvec fmt` holds campaign JSON to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Camera {
    /// The critical-path step the picture is taken after (spec-0089): the
    /// world as the region model holds it once that step is played. Absent:
    /// the world at load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<CameraAfter>,
    /// The `design.json` row whose approved image this camera answers. Absent
    /// only on a test level (`world.json` `test_level: true`, spec-0122 §7.2),
    /// which carries no design record: its `note` names what it frames.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answers: Option<String>,
    /// Vertical field of view, degrees.
    pub fov: f64,
    /// Frame height, pixels.
    pub height: u32,
    /// The camera's name: lowercase letters, digits, `.`, `+`, `-`.
    pub name: String,
    /// What the camera frames, in words, where it answers no approved image
    /// (a test level's camera).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Minecraft pitch, degrees: positive looks down.
    pub pitch: f64,
    /// The lens, world blocks.
    pub pos: [f64; 3],
    /// The sky the picture is taken under, when it is not the sky of the row
    /// the camera answers. Absent: the row's `time` and `weather`. Stating the
    /// row's own sky is refused (`DW0721`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sky: Option<CameraSky>,
    /// Who placed the camera: an estimate, or a person standing in the game.
    pub source: Source,
    /// Path-tracing sample target.
    pub spp: u32,
    /// Frame width, pixels.
    pub width: u32,
    /// Minecraft yaw, degrees: 0 south, 90 west, 180 north, 270 east.
    pub yaw: f64,
}

/// The step a camera is taken after (spec-0089 §3): a step of an exported
/// path, named in the vocabulary `critical-path.json` speaks — an objective id
/// (`obj/<id>`) or a trigger id (`trigger/<id>`), never an index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CameraAfter {
    /// The branch id (`validation/branch-plan.json`) whose path the step is
    /// on. Absent: the critical path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The step's id: `obj/<id>` or `trigger/<id>`.
    pub step: String,
}

impl CameraAfter {
    /// Parse `<step>[@<path>]` — `delvec place-camera --after`'s spelling.
    pub fn parse(spec: &str) -> Result<CameraAfter, String> {
        let shape = "an `--after` is `<step>[@<path>]`: an objective id (`obj/<id>`) or a trigger \
                     id (`trigger/<id>`), and optionally `@` and the branch id whose path it is on \
                     (`obj/clear-the-muster`, `obj/open-the-vault@branch/skipped`)";
        let (step, path) = match spec.split_once('@') {
            Some((s, p)) => (s, Some(p)),
            None => (spec, None),
        };
        let well_formed = |s: &str, prefix: &str| {
            s.strip_prefix(prefix).is_some_and(|rest| {
                !rest.is_empty()
                    && rest.chars().all(|c| {
                        c.is_ascii_lowercase()
                            || c.is_ascii_digit()
                            || matches!(c, '-' | '_' | '/' | '.')
                    })
            })
        };
        if !(well_formed(step, "obj/") || well_formed(step, "trigger/")) {
            return Err(format!("`{spec}` is not a step: {shape}"));
        }
        if let Some(p) = path
            && (p.is_empty() || p.contains('@') || p.chars().any(char::is_whitespace))
        {
            return Err(format!("`{spec}` names no path after `@`: {shape}"));
        }
        Ok(CameraAfter {
            path: path.map(str::to_string),
            step: step.to_string(),
        })
    }

    /// `obj/x` or `obj/x@branch/y`: the pair as every line prints it.
    pub fn label(&self) -> String {
        match &self.path {
            Some(p) => format!("{}@{p}", self.step),
            None => self.step.clone(),
        }
    }
}

/// Who placed a camera. A hand camera is final: no estimate is written over it
/// ([`place`]), and only deleting its row frees the name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// Derived by a tool or an agent: a panorama fit, a bracket candidate, a
    /// camera estimated from an approved image.
    Estimated,
    /// Captured in the running game where a person stood and looked
    /// (`/trigger dw.cam`, harvested by `delvec harvest`).
    Hand,
}

/// The sky a camera states (spec-0079 §3): one state, both halves required,
/// typed by the enums `world.json` and `design.json` use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CameraSky {
    /// The hour the picture is taken at.
    pub time: WorldTime,
    /// The weather the picture is taken under.
    pub weather: WorldWeather,
}

impl CameraSky {
    /// Parse `<time>,<weather>` — `delvec place-camera --sky`'s spelling: the
    /// time a keyword or a celestial object in JSON
    /// (`{"moon":"high","phase":"new-moon"},clear`), the weather a keyword
    /// (`dusk,rain`). The weather is split off the LAST comma, so a celestial
    /// object's own commas stay inside the time.
    pub fn parse(spec: &str) -> Result<CameraSky, String> {
        let shape = "a sky is `<time>,<weather>`: a time — day | noon | dusk | night | midnight \
                     | dawn, or a celestial object such as {\"moon\":\"high\"} — and a weather, \
                     clear | rain | thunder (`dusk,rain`)";
        let (time, weather) = spec
            .rsplit_once(',')
            .ok_or_else(|| format!("`{spec}` is not a sky: {shape}"))?;
        let time = time.trim();
        let parsed: Result<WorldTime, _> = if time.starts_with('{') {
            serde_json::from_str(time).map_err(|e| e.to_string())
        } else {
            serde_json::from_value(serde_json::Value::from(time)).map_err(|e| e.to_string())
        };
        let time = parsed.map_err(|e| format!("`{time}` is not a time of day: {e}. {shape}"))?;
        let weather: WorldWeather = serde_json::from_value(serde_json::Value::from(weather.trim()))
            .map_err(|_| format!("`{weather}` is not a weather: {shape}"))?;
        Ok(CameraSky { time, weather })
    }

    /// `dusk+rain`: the pair as every line prints it.
    pub fn label(self) -> String {
        format!("{}+{}", self.time.keyword(), self.weather.keyword())
    }

    /// The scene sky this pair is, in a world whose own time is `world`.
    pub fn scene(self, world: WorldTime) -> SceneSky {
        SceneSky {
            day: scene::frame_day(self.time, world),
            daytime_ticks: self.time.daytime_ticks(),
            weather: self.weather,
        }
    }
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
        match (&self.answers, &self.note) {
            (Some(a), _) if a.trim().is_empty() => {
                return Err(
                    "`answers` is empty. Every showcase camera is judged against an approved \
                     image: name the `design.json` row it answers"
                        .to_string(),
                );
            }
            (None, n) if n.as_deref().is_none_or(|n| n.trim().is_empty()) => {
                return Err(
                    "states neither `answers` nor `note`. A camera answers a `design.json` row, \
                     or — on a test level, which carries no design record — names what it \
                     frames in `note`"
                        .to_string(),
                );
            }
            _ => {}
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
    if let Some(refusal) = exposure_stated(bytes) {
        return Err(refusal);
    }
    let sheet: CameraSheet = serde_json::from_slice(bytes).map_err(|e| {
        Diagnostic::error(
            DW_INPUT,
            format!(
                "parse {CAMERAS_FILE}: {}",
                camera_at_fault(bytes).unwrap_or_else(|| e.to_string())
            ),
        )
    })?;
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

/// `DW1010` for the first camera of a record that states `exposure`, `None`
/// otherwise (and for a document that is not JSON, which serde refuses next).
/// Asked before serde, whose unknown-field sentence names no remedy.
fn exposure_stated(bytes: &[u8]) -> Option<Diagnostic> {
    let doc: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let cams = doc.get("cameras")?.as_array()?;
    let c = cams.iter().find(|c| c.get("exposure").is_some())?;
    Some(Diagnostic::error(
        DW_EXPOSURE_STATED.id(),
        format!(
            "{CAMERAS_FILE}: camera `{}` (answers `{}`) states `exposure` {}. A frame's exposure \
             is derived by the engine, not stated: every showcase frame is exposed at {:.1}, where \
             a top face under the whole sky and a lantern-lit surface read as the game draws \
             them. Remove the field. A room that reads too dark at that exposure is short of \
             light: light the room where it is designed — add or re-arrange its lights until \
             `delvec prefab lighting` calls it lit (the skill's `new-pieces.md`, *Light inside a \
             cave or a body*) — never the picture",
            c.get("name").and_then(|n| n.as_str()).unwrap_or("?"),
            c.get("answers").and_then(|a| a.as_str()).unwrap_or("?"),
            c["exposure"],
            crate::compiler::view::frames::HQ_EXPOSURE,
        ),
    ))
}

/// The first camera of a record that does not parse as a camera, named with the
/// row it answers and serde's reason — so a refusal of one field (a half-stated
/// `sky`, a weather that is not one) says which camera to open. `None` when the
/// document is not JSON, or the fault is not inside one camera.
fn camera_at_fault(bytes: &[u8]) -> Option<String> {
    let doc: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    doc.get("cameras")?.as_array()?.iter().find_map(|c| {
        match serde_json::from_value::<Camera>(c.clone()) {
            Ok(_) => None,
            Err(e) => Some(format!(
                "camera `{}` (answers `{}`): {e}",
                c.get("name").and_then(|n| n.as_str()).unwrap_or("?"),
                c.get("answers").and_then(|a| a.as_str()).unwrap_or("?")
            )),
        }
    })
}

/// One row of `design.json` as the camera surface reads it: the name a camera
/// answers, and the sentence that says which picture it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovedRow {
    /// The row's `name`: `concept/morning-quay`.
    pub name: String,
    /// The row's `shows` sentence, empty when the document states none.
    pub shows: String,
    /// The `time` and `weather` the picture was drawn under — the sky a camera
    /// answering the row renders under unless it states its own. `None` only
    /// when the document does not state both (which `delvec validate` refuses).
    pub sky: Option<CameraSky>,
}

/// The approved-image rows a `design.json` carries, in the design's own order.
///
/// The one parse of that document on this side of the engine
/// ([`reference_names`] is this function's names), so every camera command and
/// the build read the record the same way.
pub fn reference_rows(design_json: &[u8]) -> Result<Vec<ApprovedRow>, Diagnostic> {
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
        .filter_map(|r| {
            let name = r.get("name").and_then(|n| n.as_str())?;
            let time = r
                .get("time")
                .and_then(|t| serde_json::from_value::<WorldTime>(t.clone()).ok());
            let weather = r
                .get("weather")
                .and_then(|w| serde_json::from_value::<WorldWeather>(w.clone()).ok());
            Some(ApprovedRow {
                name: name.to_string(),
                shows: r
                    .get("shows")
                    .and_then(|s| s.as_str())
                    .unwrap_or_default()
                    .to_string(),
                sky: time
                    .zip(weather)
                    .map(|(time, weather)| CameraSky { time, weather }),
            })
        })
        .collect())
}

/// The names of the approved-image rows a `design.json` carries.
pub fn reference_names(design_json: &[u8]) -> Result<Vec<String>, Diagnostic> {
    Ok(reference_rows(design_json)?
        .into_iter()
        .map(|r| r.name)
        .collect())
}

/// **What a camera record answers of a design record's rows** — the one
/// comparison between the two documents (spec-0070 §2).
///
/// Both directions come out of one walk, because they are one question asked
/// from two ends: a camera whose `answers` names no row is `stray`, and a row no
/// camera names is `unanswered`. Nothing else in this engine compares the two
/// documents; [`bind_answers`] is this function, refused at the first stray.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Answers {
    /// How many cameras the record states.
    pub cameras: usize,
    /// Rows at least one camera answers, in the design's order.
    pub answered: Vec<String>,
    /// Rows no camera answers, in the design's order.
    pub unanswered: Vec<String>,
    /// `(camera name, what it answers)` for every camera naming no row, in the
    /// record's order.
    pub stray: Vec<(String, String)>,
    /// Cameras that state no `answers` — a test level's (spec-0122 §7.2), in
    /// the record's order. Refused on any other campaign ([`unbound_refusal`]).
    pub unbound: Vec<String>,
}

impl Answers {
    /// How many rows were counted: the design record's own denominator.
    pub fn rows(&self) -> usize {
        self.answered.len() + self.unanswered.len()
    }

    /// The one sentence every camera command prints about the two records,
    /// zeroes included.
    pub fn line(&self) -> String {
        format!(
            "answers: {} of {} approved image(s) in design.json have a camera{}",
            self.answered.len(),
            self.rows(),
            if self.unanswered.is_empty() {
                String::new()
            } else {
                format!("; none answers {}", self.unanswered.join(", "))
            }
        )
    }
}

/// Count what the record answers of `rows` — see [`Answers`].
pub fn tally(sheet: &CameraSheet, rows: &[String]) -> Answers {
    let mut a = Answers {
        cameras: sheet.cameras.len(),
        ..Answers::default()
    };
    for cam in &sheet.cameras {
        match &cam.answers {
            None => a.unbound.push(cam.name.clone()),
            Some(answers) if !rows.iter().any(|r| r == answers) => {
                a.stray.push((cam.name.clone(), answers.clone()));
            }
            Some(_) => {}
        }
    }
    for r in rows {
        if sheet.cameras.iter().any(|c| c.answers.as_ref() == Some(r)) {
            a.answered.push(r.clone());
        } else {
            a.unanswered.push(r.clone());
        }
    }
    a
}

/// Every camera answers a row that exists. Returns the rows no camera answers,
/// in the design's own order.
///
/// The refusal's sentence is the one the build raises too (`DW0721`,
/// spec-0070 §5): the record has one reader, so it has one answer.
pub fn bind_answers(sheet: &CameraSheet, rows: &[String]) -> Result<Vec<String>, Diagnostic> {
    let a = tally(sheet, rows);
    if let Some((name, answers)) = a.stray.first() {
        return Err(Diagnostic::error(
            DW_INPUT,
            stray_message(name, answers, rows),
        ));
    }
    Ok(a.unanswered)
}

/// **A camera that answers no approved image belongs to a test level**
/// (spec-0122 §7.2): on any other campaign the first such camera is refused,
/// naming it; on a test level nothing is.
pub fn unbound_refusal(a: &Answers, test_level: bool) -> Option<String> {
    if test_level {
        return None;
    }
    a.unbound.first().map(|name| {
        format!(
            "{CAMERAS_FILE}: camera `{name}` states no `answers`. Every showcase camera of a \
             campaign answers a `design.json` row; only a test level (`world.json` \
             `test_level: true`), which carries no design record, has cameras that answer no \
             approved image. Name the row it answers"
        )
    })
}

/// Whether the campaign directory declares itself a test level: `world.json`
/// `content.test_level` is `true`. Read by the camera commands, which read the
/// record beside the campaign rather than the parsed campaign.
pub fn declares_test_level(campaign: &std::path::Path) -> bool {
    std::fs::read(campaign.join("world.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| v.get("content")?.get("test_level")?.as_bool())
        .unwrap_or(false)
}

/// The approved rows a camera command holds the record to: `design.json`'s,
/// or none on a test level that carries no design record (spec-0122 §7.2).
pub fn rows_of(campaign: &std::path::Path) -> Result<(Vec<ApprovedRow>, bool), Diagnostic> {
    let test_level = declares_test_level(campaign);
    let path = campaign.join("design.json");
    if test_level && !path.exists() {
        return Ok((Vec::new(), true));
    }
    let bytes = std::fs::read(&path)
        .map_err(|e| Diagnostic::error(DW_INPUT, format!("read {}: {e}", path.display())))?;
    Ok((reference_rows(&bytes)?, test_level))
}

/// The sentence a camera answering no row earns, wherever it is read.
pub fn stray_message(name: &str, answers: &str, rows: &[String]) -> String {
    format!(
        "{CAMERAS_FILE}: camera `{name}` answers `{answers}`, which is not a row of design.json. \
         Rows: {}",
        if rows.is_empty() {
            "none".to_string()
        } else {
            rows.join(", ")
        }
    )
}

/// Where a camera's sky came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkyOrigin {
    /// The camera states none: the sky of the `design.json` row it answers.
    Derived(String),
    /// The camera states its own `sky`.
    Stated,
}

/// The sky one camera is emitted under (spec-0079 §3, §6).
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedSky {
    /// The camera's name.
    pub camera: String,
    pub sky: CameraSky,
    pub origin: SkyOrigin,
}

impl ResolvedSky {
    /// The per-camera binding line: `sky: <name> <time>+<weather> <derived from
    /// <row> | stated> — <cell>`.
    pub fn line(&self) -> String {
        format!(
            "sky: {} {} {} — {}{}",
            self.camera,
            self.sky.label(),
            match &self.origin {
                SkyOrigin::Derived(row) => format!("derived from {row}"),
                SkyOrigin::Stated => "stated".to_string(),
            },
            scene::sky_phrase(self.sky.weather, self.sky.time.daytime_ticks()),
            // A clear frame stands at its clock (`frames::stated_sky`), so the
            // renderer draws the moon at the clock's phase; under rain or
            // thunder the client draws none, and neither does the frame — said
            // where it is read.
            match (
                delvewright_dsl::celestial::moon_up(self.sky.time.daytime_ticks()),
                self.sky.weather,
            ) {
                (false, _) => String::new(),
                (true, WorldWeather::Clear) => "; moon drawn at its day's phase".to_string(),
                (true, w) => format!("; no moon under {}, as the client draws none", w.keyword()),
            }
        )
    }
}

/// **The sky a camera is taken under** — its stated `sky`, else the `time` and
/// `weather` of the row it answers. The record's one rule about the sky lives
/// here, and every reader (`delvec cameras`, `delvec place-camera`, `delvec
/// build`) asks it through this function: a stated sky equal to the row's is
/// refused, because it is a derivation typed where a judgement belongs and goes
/// stale the moment the row changes.
pub fn resolve_sky(cam: &Camera, rows: &[ApprovedRow]) -> Result<ResolvedSky, String> {
    let names = || rows.iter().map(|r| r.name.clone()).collect::<Vec<_>>();
    let Some(answers) = &cam.answers else {
        // A test level's camera answers no row, so its sky is its own.
        return match cam.sky {
            Some(sky) => Ok(ResolvedSky {
                camera: cam.name.clone(),
                sky,
                origin: SkyOrigin::Stated,
            }),
            None => Err(format!(
                "{CAMERAS_FILE}: camera `{}` answers no approved image and states no `sky`, so \
                 nothing says what sky it is taken under. STATE `sky` on the camera",
                cam.name
            )),
        };
    };
    let row = rows
        .iter()
        .find(|r| &r.name == answers)
        .ok_or_else(|| stray_message(&cam.name, answers, &names()))?;
    let (sky, origin) = match (cam.sky, row.sky) {
        (Some(stated), Some(own)) if stated == own => {
            return Err(restated_message(&cam.name, &row.name, stated));
        }
        (Some(stated), _) => (stated, SkyOrigin::Stated),
        (None, Some(own)) => (own, SkyOrigin::Derived(row.name.clone())),
        (None, None) => {
            return Err(format!(
                "{CAMERAS_FILE}: camera `{}` states no `sky`, and the row it answers, `{}`, \
                 states no `time` and `weather` to take one from. Run `delvec validate`: \
                 every row of design.json states the sky its picture was drawn under",
                cam.name, row.name
            ));
        }
    };
    Ok(ResolvedSky {
        camera: cam.name.clone(),
        sky,
        origin,
    })
}

/// The sentence a camera restating its row's sky earns, wherever it is read.
pub fn restated_message(name: &str, row: &str, sky: CameraSky) -> String {
    format!(
        "{CAMERAS_FILE}: camera `{name}` states `sky` {}, which is the sky of the row it \
         answers, `{row}`. A camera with no `sky` already renders under its row's, and a copy \
         of the row's sky goes stale the moment the row changes: REMOVE the `sky` field from \
         camera `{name}`",
        sky.label()
    )
}

/// Every camera's sky, and the first refusal among them (in record order).
///
/// Both are returned so a run prints the binding line — every camera whose sky
/// resolves — whether or not it then refuses (spec-0079 §6).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Skies {
    pub resolved: Vec<ResolvedSky>,
    /// How many cameras were asked about: the line's denominator.
    pub cameras: usize,
    pub refusal: Option<String>,
}

impl Skies {
    /// The summary binding line: `skies: D derived, S stated, over C camera(s);
    /// weathers emitted: {…}` — the weathers set names the non-clear weathers
    /// only, since a clear scene emits no sky block.
    pub fn summary(&self) -> String {
        let derived = self
            .resolved
            .iter()
            .filter(|r| matches!(r.origin, SkyOrigin::Derived(_)))
            .count();
        let stated = self.resolved.len() - derived;
        let weathers: BTreeSet<&'static str> = [WorldWeather::Rain, WorldWeather::Thunder]
            .into_iter()
            .filter(|w| self.resolved.iter().any(|r| r.sky.weather == *w))
            .map(|w| w.keyword())
            .collect();
        let refused = self.cameras - self.resolved.len();
        format!(
            "skies: {derived} derived, {stated} stated, over {} camera(s){}; weathers emitted: {{{}}}",
            self.cameras,
            if refused == 0 {
                String::new()
            } else {
                format!(" ({refused} refused)")
            },
            weathers.into_iter().collect::<Vec<_>>().join(", ")
        )
    }
}

/// Resolve every camera's sky ([`resolve_sky`]).
pub fn skies(cameras: &[Camera], rows: &[ApprovedRow]) -> Skies {
    let mut out = Skies {
        cameras: cameras.len(),
        ..Skies::default()
    };
    for cam in cameras {
        match resolve_sky(cam, rows) {
            Ok(r) => out.resolved.push(r),
            Err(why) => {
                out.refusal.get_or_insert(why);
            }
        }
    }
    out
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

/// The file stem of a camera's frame: `<campaign>_camera_<name>`, with
/// `_draft` when the frame is a draft and `_preview` when it is a preview, so
/// no two frames of one camera share a file.
pub fn camera_stem(campaign_id: &str, name: &str, draft: bool) -> String {
    let shot = if draft {
        format!("camera_{name}_draft")
    } else {
        format!("camera_{name}")
    };
    format!(
        "{}_{}",
        crate::compiler::view::frames::frame_stem(campaign_id),
        crate::compiler::view::frames::frame_stem(&shot)
    )
}

/// A frame of a stated or solved camera: where, which way, how wide, how big,
/// how many samples. The one thing every showcase frame is drawn from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub pos: [f64; 3],
    /// Minecraft yaw: 0 south, 90 west.
    pub yaw_deg: f64,
    /// Positive looks down.
    pub pitch_deg: f64,
    /// Vertical field of view.
    pub fov_deg: f64,
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
            yaw_deg: cam.yaw,
            pitch_deg: cam.pitch,
            fov_deg: cam.fov,
            width,
            height,
            spp,
        }
    }

    /// The camera the renderer is handed.
    pub fn view(&self) -> crate::compiler::framing::Camera {
        crate::compiler::framing::Camera {
            pos: self.pos,
            yaw: self.yaw_deg,
            pitch: self.pitch_deg,
            fov: self.fov_deg,
        }
    }
}

/// One showcase frame a run draws: the camera it is of, the file stem, the
/// frame and the sky it is taken under.
#[derive(Debug, Clone, PartialEq)]
pub struct CameraFrame {
    pub camera: Camera,
    pub stem: String,
    pub frame: Frame,
    pub sky: SceneSky,
}

/// Options for [`frames_of`].
#[derive(Debug, Clone, Default)]
pub struct EmitOptions {
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

/// The frames of a record against a build's `render-plan.json`, each under
/// its camera's sky ([`resolve_sky`] over `rows`, the design record's), in
/// record order (a stated camera, then its candidates). Deterministic
/// (ADR-0006): the same plan, record, rows and options give the same frames.
pub fn frames_of(
    plan_json: &[u8],
    sheet: &CameraSheet,
    rows: &[ApprovedRow],
    opts: &EmitOptions,
) -> Result<Vec<CameraFrame>, Diagnostic> {
    let plan = scene::parse_plan(plan_json)?;
    // A camera's sky is its picture's, not the plan's — but a plan that states no
    // sky, or no weather, was written by an older engine, and is refused by name
    // here as every frame of a plan refuses it.
    scene::plan_sky(&plan)?;
    let world = plan
        .sky
        .as_ref()
        .expect("plan_sky refuses a plan with no sky")
        .world_time()?;
    let cameras = selected(&plan.campaign_id, sheet, &opts.only, opts.bracket.as_ref())?;
    let mut out = Vec::new();
    for c in cameras {
        let sky = resolve_sky(&c, rows).map_err(|why| Diagnostic::error(DW_INPUT, why))?;
        out.push(CameraFrame {
            stem: camera_stem(&plan.campaign_id, &c.name, opts.draft),
            frame: Frame::of(&c, opts.draft),
            sky: sky.sky.scene(world),
            camera: c,
        });
    }
    Ok(out)
}

/// What a row of the record is written from.
#[derive(Debug, Clone, PartialEq)]
pub enum Placement {
    /// A camera a tool estimated, taken verbatim from a record-format file
    /// (`candidates.json`): pose, lens and frame. Written as
    /// `estimated`.
    Estimate(Camera),
    /// A pose a person captured in the running game (`camera-report.json`),
    /// with the field of view they stated. Written as `hand`; the row's frame
    /// is kept.
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
///   300 spp); a replaced hand row keeps the row's frame, because a person
///   placed the pose, not the frame.
///
/// `sheet` is the record as it stands (`None` when the campaign has none yet),
/// `campaign_id` the campaign the placement belongs to.
pub fn place(
    sheet: Option<CameraSheet>,
    campaign_id: &str,
    name: &str,
    answers: Option<&str>,
    sky: Option<CameraSky>,
    after: Option<CameraAfter>,
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
    let mut row = match (existing, &placement) {
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
                && Some(a) != old.answers.as_deref()
            {
                return Err(refuse(format!(
                    "camera `{name}` answers `{}`, not `{a}`: a camera is an answer to one \
                     picture, so aiming at another picture is a new row with its own name",
                    old.answers.as_deref().unwrap_or("no approved image")
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
            let answers = answers.map(str::to_string).or_else(|| est.answers.clone());
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
                after: None,
                answers: Some(answers.to_string()),
                fov: *fov,
                height: crate::compiler::view::panorama::DEFAULT_HEIGHT,
                name: name.to_string(),
                note: None,
                pitch: *pitch,
                pos: *pos,
                sky: None,
                source: Source::Hand,
                spp: crate::compiler::view::panorama::DEFAULT_SPP,
                width: crate::compiler::view::panorama::DEFAULT_WIDTH,
                yaw: *yaw,
            }
        }
    };
    // `--sky` states the row's sky; without it a hand row keeps the sky it had
    // (a person placed the pose, not the sky) and an estimate carries its own.
    if sky.is_some() {
        row.sky = sky;
    }
    // `--after` states the step the picture is taken after (spec-0089 §3.4);
    // without it a hand row keeps the step it had and an estimate carries its
    // own — the overlay cannot read the quest state, so a stamp never carries one.
    if after.is_some() {
        row.after = after;
    }
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
    /// A camera's subject lies past the served view distance (`DW0956`).
    Beyond(String),
}

/// **Every showcase camera photographs the scene** — the build's proof over
/// `design/cameras.json`, asked of the final assembled world and of the plan
/// the frames are drawn from. Returns how many cameras it examined.
///
/// Two shapes of one rule:
///
/// 1. **The eye is not inside a block**: `occupied(cell)` of the lens's own cell
///    is false. A camera is never moved to make this hold — a hand camera is
///    where the person stood, an estimate is where the agent put it — so a
///    violation is a refusal naming the row.
/// 2. **The frame holds the loaded world**: the lens lies within the world's
///    build height, and the ray along its view meets the box the frame's chunk
///    rectangle is cut from ([`scene::loaded_extent`], widened to whole blocks). A
///    camera looking away from everything the frame loads draws an empty sky.
///
/// 3. **The subject is served** (spec-0091, `DW0956`): the thing the camera is
///    aimed at — the first occupied cell on its central ray, else the point
///    where that ray enters the loaded scene — lies within `served_radius`
///    blocks of the lens, the radius a player standing at the lens is sent.
///    A subject past it is never sent to the client, so the picture promises a
///    view no player gets.
///
/// A record for another campaign is refused as `delvec cameras` refuses it.
pub(crate) fn prove_showcase(
    record: &[u8],
    plan: &RenderPlan,
    served_radius: f64,
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
        let Some((enter, exit)) = ray_meets_box(cam.pos, dir, lo, hi) else {
            return Err(ShowcaseRefusal::Camera(format!(
                "{CAMERAS_FILE}: {who}: looking along yaw {} pitch {} from {:?}, the view meets \
                 nothing the scene loads — the framed extent is {lmin:?}..{lmax:?} — so the frame \
                 would be empty sky. {remedy}",
                cam.yaw, cam.pitch, cam.pos
            )));
        };
        // The subject: the first occupied cell on the central ray inside the
        // loaded scene, else where the ray enters it. `dir` is a unit vector, so
        // a ray parameter is a distance in blocks.
        let far = [
            cam.pos[0] + dir[0] * exit,
            cam.pos[1] + dir[1] * exit,
            cam.pos[2] + dir[2] * exit,
        ];
        let mut first: Option<[i32; 3]> = None;
        crate::compiler::nav::walk_cells(cam.pos, far, |c| {
            if occupied(c) {
                first = Some(c);
                return true;
            }
            false
        });
        let (subject, subject_distance) = match first {
            Some(c) => {
                let centre = [
                    f64::from(c[0]) + 0.5,
                    f64::from(c[1]) + 0.5,
                    f64::from(c[2]) + 0.5,
                ];
                let d = [
                    centre[0] - cam.pos[0],
                    centre[1] - cam.pos[1],
                    centre[2] - cam.pos[2],
                ];
                (
                    format!("the first solid cell on its line of sight, {c:?}"),
                    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt(),
                )
            }
            None => (
                "where its line of sight enters the loaded scene".to_string(),
                enter,
            ),
        };
        if subject_distance > served_radius {
            let chunks = delvewright_dsl::viewdistance::chunks_for(subject_distance);
            return Err(ShowcaseRefusal::Beyond(format!(
                "{CAMERAS_FILE}: {who}: its subject — {subject} — is {subject_distance:.1} blocks \
                 from the lens, and the served view distance reaches {served_radius:.0} blocks \
                 ({} chunks): a player standing at the lens is never sent what this picture \
                 shows. Declare `world.view_distance: {chunks}` (the fewest chunks that serve \
                 it), or stand nearer.",
                delvewright_dsl::viewdistance::chunks_for(served_radius)
            )));
        }
    }
    Ok(sheet.cameras.len())
}

/// Where the ray from `origin` along unit `dir` (t ≥ 0) enters and leaves the
/// box `lo..hi`, as ray parameters; `None` when it misses. An origin inside the
/// box enters at 0.
fn ray_meets_box(
    origin: [f64; 3],
    dir: [f64; 3],
    lo: [f64; 3],
    hi: [f64; 3],
) -> Option<(f64, f64)> {
    let (mut t0, mut t1) = (0.0f64, f64::INFINITY);
    for a in 0..3 {
        if dir[a].abs() < 1e-12 {
            if origin[a] < lo[a] || origin[a] > hi[a] {
                return None;
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
            return None;
        }
    }
    Some((t0, t1))
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
            after: None,
            answers: "concept/quay".to_string(),
            fov: 50.0,
            height: 900,
            name: name.to_string(),
            pitch: 10.0,
            pos: [10.5, 70.0, -4.25],
            sky: None,
            source: Source::Estimated,
            spp: 300,
            width: 1600,
            yaw: 30.0,
        }
    }

    /// The design rows the test cameras answer: both at the ocean fixture's
    /// declared sky (`day`, `clear`).
    fn rows() -> Vec<ApprovedRow> {
        ["concept/quay", "a"]
            .into_iter()
            .map(|name| ApprovedRow {
                name: name.to_string(),
                shows: String::new(),
                sky: Some(CameraSky {
                    time: WorldTime::Day,
                    weather: WorldWeather::Clear,
                }),
            })
            .collect()
    }

    /// The frames of a record against `plan`, under the test rows.
    fn emit3(
        plan: &[u8],
        sheet: &CameraSheet,
        opts: &EmitOptions,
    ) -> Result<Vec<CameraFrame>, Diagnostic> {
        frames_of(plan, sheet, &rows(), opts)
    }

    fn place5(
        sheet: Option<CameraSheet>,
        campaign_id: &str,
        name: &str,
        answers: Option<&str>,
        placement: Placement,
    ) -> Result<(CameraSheet, Placed), Diagnostic> {
        place(sheet, campaign_id, name, answers, None, None, placement)
    }

    fn sheet(id: &str, cams: Vec<Camera>) -> CameraSheet {
        CameraSheet {
            campaign_id: id.to_string(),
            cameras: cams,
        }
    }

    /// Minecraft's own look vector for an entity rotation.
    fn minecraft_forward(yaw: f64, pitch: f64) -> [f64; 3] {
        let (y, p) = (yaw.to_radians(), pitch.to_radians());
        [-y.sin() * p.cos(), -p.sin(), y.cos() * p.cos()]
    }

    /// **The record speaks Minecraft's rotation, and the frame looks where the
    /// game would.** At the four cardinals, the four diagonals, an arbitrary
    /// yaw, negative and past-360 yaws and several pitches, the direction the
    /// frame's camera looks along is the direction Minecraft derives from the
    /// record's rotation.
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
                let e = emit3(OCEAN, &s, &EmitOptions::default()).unwrap();
                let (got, _, _) = e[0].frame.view().basis();
                let want = minecraft_forward(yaw, pitch);
                for k in 0..3 {
                    assert!(
                        (got[k] - want[k]).abs() < 1e-5,
                        "yaw {yaw} pitch {pitch}: frame {got:?} vs minecraft {want:?}"
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
    fn a_stated_camera_is_emitted_verbatim() {
        let s = sheet("isle", vec![cam("hero")]);
        let e = emit3(OCEAN, &s, &EmitOptions::default()).unwrap();
        assert_eq!(e.len(), 1);
        let f = &e[0];
        assert_eq!(f.stem, "isle_camera_hero");
        assert_eq!(
            (f.frame.width, f.frame.height, f.frame.spp),
            (1600, 900, 300)
        );
        assert_eq!(f.frame.fov_deg, 50.0);
        assert_eq!(f.frame.pos, [10.5, 70.0, -4.25]);
        assert_eq!((f.frame.yaw_deg, f.frame.pitch_deg), (30.0, 10.0));
        // The row's sky.
        assert_eq!(
            f.sky,
            SceneSky {
                day: 0,
                daytime_ticks: WorldTime::Day.daytime_ticks(),
                weather: WorldWeather::Clear
            }
        );
    }

    /// Every stated field reaches the frame: perturb one, and the frame moves.
    #[test]
    fn every_stated_field_moves_the_scene() {
        let base = emit3(
            OCEAN,
            &sheet("isle", vec![cam("a")]),
            &EmitOptions::default(),
        )
        .unwrap();
        type Edit = (&'static str, fn(&mut Camera));
        let edits: [Edit; 8] = [
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
            let moved = emit3(OCEAN, &sheet("isle", vec![c]), &EmitOptions::default()).unwrap();
            assert_ne!(
                base[0].frame, moved[0].frame,
                "{what} did not reach the frame"
            );
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
            emit3(OCEAN, &s, &opts).unwrap(),
            emit3(OCEAN, &s, &opts).unwrap()
        );
    }

    #[test]
    fn a_record_for_another_campaign_is_refused() {
        let err = emit3(
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
        let err = emit3(no_sky, &sheet("c", vec![cam("a")]), &EmitOptions::default()).unwrap_err();
        assert_eq!(err.code, DW_INPUT);
    }

    #[test]
    fn the_record_refuses_what_is_not_a_camera() {
        let ok = serde_json::to_value(sheet("c", vec![cam("a")])).unwrap();
        assert!(parse_sheet(ok.to_string().as_bytes()).is_ok());
        let edits: [(&str, serde_json::Value); 11] = [
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

    /// **An exposure is derived, never stated.** A camera carrying `exposure`
    /// is refused under `DW1010` at any value — the derived one included,
    /// since a copy of a derivation is still typed — naming the camera, its
    /// row and the remedy: remove the field, and light a dark room.
    #[test]
    fn a_stated_exposure_is_refused_with_its_remedy() {
        let ok = serde_json::to_value(sheet("c", vec![cam("a"), cam("b")])).unwrap();
        for value in [
            serde_json::json!(1.0),
            serde_json::json!(16.0),
            serde_json::json!(0),
        ] {
            let mut doc = ok.clone();
            doc["cameras"][1]["exposure"] = value.clone();
            let err = parse_sheet(doc.to_string().as_bytes()).unwrap_err();
            assert_eq!(err.code, "DW1010", "{value}");
            assert_eq!(err.code, DW_EXPOSURE_STATED.id());
            for needle in [
                "camera `b`",
                "`concept/quay`",
                "Remove the field",
                "light the room",
            ] {
                assert!(err.message.contains(needle), "{needle}: {err:?}");
            }
        }
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
        // Truck moves square to the heading, to the right on `+`: the
        // direction a frame's right edge lies in. Heading (dx, dz) turned to
        // (-dz, dx).
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
        let draft = emit3(
            OCEAN,
            &s,
            &EmitOptions {
                draft: true,
                ..Default::default()
            },
        )
        .unwrap();
        let d = &draft[0];
        assert_eq!(d.stem, "isle_camera_a_draft");
        assert_eq!((d.frame.width, d.frame.height), (400, 225));
        assert_eq!(d.frame.spp, DRAFT_SPP);
        // The candidates file keeps the stated frame, so a pick is copied verbatim.
        assert_eq!(d.camera, cam("a"));
    }

    #[test]
    fn only_selects_and_refuses_a_name_it_does_not_have() {
        let s = sheet("mini", vec![cam("a"), cam("b")]);
        let e = emit3(
            MINI,
            &s,
            &EmitOptions {
                only: vec!["b".into()],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].camera.name, "b");
        let err = emit3(
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
    /// hand pose replaces a hand row and keeps its frame.
    #[test]
    fn an_estimate_is_never_written_over_a_hand_camera() {
        let (sheet, placed) = place5(
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
        assert_eq!((row.width, row.height, row.spp), (1600, 900, 300));

        let estimate = Placement::Estimate(cam("hero.yaw+8"));
        let err = place5(Some(sheet.clone()), "isle", "hero", None, estimate.clone()).unwrap_err();
        assert_eq!(err.code, DW_INPUT);
        assert!(err.message.contains("`hero` was placed by hand"), "{err:?}");

        // She re-fires `dw.cam`: the hand pose replaces the hand pose, and the
        // frame the row had stays.
        let mut framed = sheet.clone();
        framed.cameras[0].spp = 4096;
        let (again, placed) =
            place5(Some(framed), "isle", "hero", None, hand([2.5, 71.0, -3.0])).unwrap();
        assert_eq!(placed, Placed::Replaced);
        assert_eq!(again.cameras[0].pos, [2.5, 71.0, -3.0]);
        assert_eq!(again.cameras[0].spp, 4096);

        // Deleted, the name is free, and the same estimate is written.
        assert_eq!(delete(sheet.clone(), "hero").unwrap(), None);
        let mut two = sheet.clone();
        two.cameras.push(cam("other"));
        let left = delete(two, "hero").unwrap().unwrap();
        let (written, placed) =
            place5(Some(left), "isle", "hero", Some("concept/quay"), estimate).unwrap();
        assert_eq!(placed, Placed::Added);
        let row = written.cameras.iter().find(|c| c.name == "hero").unwrap();
        assert_eq!(row.source, Source::Estimated);
        assert_eq!(row.yaw, cam("x").yaw);
        assert!(delete(written, "nobody").is_err());
    }

    #[test]
    fn a_placement_answers_one_picture_in_one_world() {
        let err = place5(None, "isle", "hero", None, hand([0.5, 70.0, 0.5])).unwrap_err();
        assert!(err.message.contains("--answers"), "{err:?}");
        let (sheet, _) = place5(
            None,
            "isle",
            "hero",
            Some("concept/quay"),
            hand([0.5, 70.0, 0.5]),
        )
        .unwrap();
        let err = place5(
            Some(sheet.clone()),
            "isle",
            "hero",
            Some("concept/hall"),
            hand([0.5, 70.0, 0.5]),
        )
        .unwrap_err();
        assert!(err.message.contains("new row"), "{err:?}");
        let err = place5(
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
        assert!(place5(Some(sheet), "isle", "hero", None, bad).is_err());
    }

    /// The record a tool writes reads back as itself, in canonical key order.
    #[test]
    fn a_written_record_is_canonical_and_reads_back() {
        let (sheet, _) = place5(
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
        assert_eq!(
            prove_showcase(&record(down.clone()), &plan, 1e9, nothing),
            Ok(1)
        );
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
                prove_showcase(&record(c), &plan, 1e9, |x| x == cell)
            else {
                panic!("{source:?}: an occupied eye is refused");
            };
            assert!(why.contains("`down`") && why.contains("occupied"), "{why}");
            assert_eq!(why.contains("dw.cam"), source == Source::Hand, "{why}");
        }
        // Looking straight up from above the layout: empty sky.
        let mut up = down.clone();
        up.pitch = -60.0;
        let Err(ShowcaseRefusal::Camera(why)) = prove_showcase(&record(up), &plan, 1e9, nothing)
        else {
            panic!("a frame of empty sky is refused");
        };
        assert!(why.contains("framed extent"), "{why}");
        // Above the build height.
        let mut high = down;
        high.pos[1] = 400.0;
        assert!(matches!(
            prove_showcase(&record(high), &plan, 1e9, nothing),
            Err(ShowcaseRefusal::Camera(_))
        ));
        // A record for another world, and a record that is not one.
        let other = serde_json::to_vec(&sheet("mini", vec![cam("a")])).unwrap();
        assert!(matches!(
            prove_showcase(&other, &plan, 1e9, nothing),
            Err(ShowcaseRefusal::Record(_))
        ));
        assert!(matches!(
            prove_showcase(b"{}", &plan, 1e9, nothing),
            Err(ShowcaseRefusal::Record(_))
        ));
    }

    /// The third shape (spec-0091, `DW0956`): the subject must be within the
    /// served radius of the lens — where the central ray enters the loaded
    /// scene when nothing solid stands on it, else the first solid cell.
    #[test]
    fn a_showcase_camera_whose_subject_is_past_the_served_radius_is_refused() {
        let plan = scene::parse_plan(OCEAN).unwrap();
        let (lmin, lmax) = scene::loaded_extent(&plan.layout_aabb, plan.horizon);
        let record = |c: Camera| serde_json::to_vec(&sheet("isle", vec![c])).unwrap();
        let nothing = |_: [i32; 3]| false;
        // 200 blocks south of the layout's middle, level, looking north at it.
        let mut far = cam("far");
        far.pos = [
            f64::from(lmin[0] + lmax[0]) / 2.0,
            f64::from(lmax[1]) - 1.0,
            f64::from(lmax[2]) + 200.0,
        ];
        far.yaw = 180.0;
        far.pitch = 0.0;
        let Err(ShowcaseRefusal::Beyond(why)) =
            prove_showcase(&record(far.clone()), &plan, 160.0, nothing)
        else {
            panic!("a subject 200 blocks off at a 160-block radius is refused");
        };
        assert!(
            why.contains("`far`") && why.contains("enters the loaded scene"),
            "{why}"
        );
        assert!(why.contains("world.view_distance: 13"), "{why}");
        // The remedy the message names reaches a different verdict.
        assert_eq!(
            prove_showcase(&record(far.clone()), &plan, 13.0 * 16.0, nothing),
            Ok(1)
        );
        // A solid cell 50 blocks along the ray is the subject instead, and is served.
        let solid = [
            far.pos[0].floor() as i32,
            far.pos[1].floor() as i32,
            (far.pos[2] - 50.0).floor() as i32,
        ];
        assert_eq!(
            prove_showcase(&record(far.clone()), &plan, 160.0, |c| c == solid),
            Ok(1)
        );
        // The same solid cell 170 blocks along the ray is named as the subject.
        let solid = [solid[0], solid[1], (far.pos[2] - 170.0).floor() as i32];
        let Err(ShowcaseRefusal::Beyond(why)) =
            prove_showcase(&record(far), &plan, 160.0, |c| c == solid)
        else {
            panic!("a solid subject past the radius is refused");
        };
        assert!(why.contains("first solid cell"), "{why}");
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
