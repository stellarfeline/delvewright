//! **The shot index** (spec-0003 visual tier, spec-0112 §9): the (image ↔
//! expect) pairs a reviewing agent is handed, one entry per frame `delvec
//! frames` drew, each stating what the frame is of and what it shows.
//!
//! [`plan_shots`] is the one reader of a `render-plan.json`'s shots: the id and
//! kind, the camera in Minecraft's convention, the world the frame stands in,
//! its clock, its mode, its `expect` checklist and its `lighting` stamp. An
//! entry ([`IndexEntry`]) adds what only drawing the frame can say: the image,
//! the backend and renderer that drew it, what it shows, what the renderer did
//! not draw (`undrawn`, `DW1039`), what lies outside it and what no frame can
//! show (`unchecked`).
//!
//! Entries follow the plan's shot order; the bytes are a function of the plan,
//! the review record and the renderer's statistics, never of the pixels.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::compiler::framing::Camera;
use crate::compiler::view::diag::{DW_INPUT, Diagnostic};
use crate::compiler::view::frames::{FrameSky, Mode, REVIEW_HQ_SPP};

#[derive(Debug, Deserialize)]
struct RenderPlan {
    campaign_id: String,
    shots: Vec<RawShot>,
}

#[derive(Debug, Deserialize)]
struct RawCamera {
    pos: [f64; 3],
    yaw: f64,
    pitch: f64,
    #[serde(default)]
    fov: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct RawSky {
    #[serde(default)]
    time: Value,
    daytime_ticks: i64,
    weather: delvewright_dsl::WorldWeather,
    #[serde(default)]
    set_by: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawShot {
    id: String,
    kind: String,
    camera: RawCamera,
    #[serde(default)]
    world: Option<String>,
    #[serde(default)]
    sky: Option<RawSky>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    leg: Option<u32>,
    #[serde(default)]
    objective: Option<String>,
    #[serde(default)]
    expect: Vec<String>,
    #[serde(default)]
    lighting: Option<Value>,
    /// A cutscene shot's facts (`step`, `shot`, `tick`, `seconds`).
    #[serde(default)]
    step: Option<Value>,
    #[serde(default)]
    shot: Option<Value>,
    #[serde(default)]
    tick: Option<Value>,
    #[serde(default)]
    seconds: Option<Value>,
}

/// One shot of a plan, as a frame is drawn from it.
#[derive(Debug, Clone)]
pub struct PlanShot {
    pub id: String,
    pub kind: String,
    pub camera: Camera,
    /// The key of the world the frame stands in.
    pub world: String,
    pub sky: FrameSky,
    /// The `sky` as the plan states it, passed through to the index.
    pub sky_json: Value,
    pub mode: Mode,
    pub leg: Option<u32>,
    pub objective: Option<String>,
    pub expect: Vec<String>,
    pub lighting: Option<Value>,
    /// A cutscene shot's `{step, shot, tick, seconds}`.
    pub cutscene: Option<Value>,
}

/// **Read every shot of a `render-plan.json`.** A shot that states no world,
/// no sky or no mode was written by an engine that did not say what its
/// frame is of, and is refused by name (`DW0721`): the remedy is a rebuild.
pub fn plan_shots(plan_json: &[u8]) -> Result<(String, Vec<PlanShot>), Diagnostic> {
    let plan: RenderPlan = serde_json::from_slice(plan_json)
        .map_err(|e| Diagnostic::error(DW_INPUT, format!("parse render-plan.json: {e}")))?;
    let mut out = Vec::with_capacity(plan.shots.len());
    for s in plan.shots {
        let stale = |what: &str| {
            Diagnostic::error(
                DW_INPUT,
                format!(
                    "render-plan.json: shot `{}` states no `{what}`, so nothing says what its \
                     frame is of. The plan was written by an engine older than this one (delvec \
                     {}): rebuild the delve with `delvec build`",
                    s.id,
                    crate::compiler::DELVEC_VERSION
                ),
            )
        };
        let world = s.world.clone().ok_or_else(|| stale("world"))?;
        let sky = s.sky.as_ref().ok_or_else(|| stale("sky"))?;
        let mode = match s.mode.as_deref() {
            Some("quick") => Mode::Quick,
            Some("hq") => Mode::Hq { spp: REVIEW_HQ_SPP },
            Some(other) => {
                return Err(Diagnostic::error(
                    DW_INPUT,
                    format!(
                        "render-plan.json: shot `{}` states mode `{other}`, which is neither \
                         `quick` nor `hq`",
                        s.id
                    ),
                ));
            }
            None => return Err(stale("mode")),
        };
        let cutscene = (s.kind == "cutscene").then(|| {
            serde_json::json!({
                "step": s.step,
                "shot": s.shot,
                "tick": s.tick,
                "seconds": s.seconds,
            })
        });
        out.push(PlanShot {
            camera: Camera {
                pos: s.camera.pos,
                yaw: s.camera.yaw,
                pitch: s.camera.pitch,
                fov: s
                    .camera
                    .fov
                    .unwrap_or(crate::compiler::render_plan::DEFAULT_FOV_DEG),
            },
            world,
            sky: FrameSky {
                daytime_ticks: sky.daytime_ticks,
                weather: sky.weather,
            },
            sky_json: serde_json::json!({
                "time": sky.time,
                "daytime_ticks": sky.daytime_ticks,
                "weather": sky.weather.keyword(),
                "set_by": sky.set_by,
            }),
            mode,
            leg: s.leg,
            objective: s.objective.clone(),
            expect: s.expect.clone(),
            lighting: s.lighting.clone(),
            cutscene,
            id: s.id,
            kind: s.kind,
        });
    }
    Ok((plan.campaign_id, out))
}

/// One entry of `shot-index.json`.
#[derive(Debug, Clone, Serialize)]
pub struct IndexEntry {
    pub id: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leg: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub objective: Option<String>,
    /// The frame's file name, beside the index.
    pub image: String,
    pub expect: Vec<String>,
    /// The shot's `lighting` stamp, passed through from the plan.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lighting: Option<Value>,
    pub world: String,
    pub sky: Value,
    pub mode: &'static str,
    /// The backend that drew the frame, in the renderer's words.
    pub backend: String,
    /// `lumenwright <revision>`.
    pub renderer: String,
    /// Every target in frame, with its screen box, distance and whether a
    /// block hides it.
    pub shows: Vec<Value>,
    /// The bodies in `shows` the renderer did not draw, each with its reason
    /// and how to judge it (`DW1039`).
    pub undrawn: Vec<Value>,
    pub out_of_frame: Vec<Value>,
    /// What this frame cannot show, by name (spec-0112 §11).
    pub unchecked: Vec<String>,
    /// A cutscene frame's `{step, shot, tick, seconds}`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cutscene: Option<Value>,
}

#[derive(Debug, Serialize)]
struct ShotIndex<'a> {
    campaign_id: &'a str,
    count: usize,
    shots: &'a [IndexEntry],
}

/// The index bytes: fixed field order, 2-space pretty, trailing newline.
pub fn index_bytes(campaign_id: &str, entries: &[IndexEntry]) -> Vec<u8> {
    let idx = ShotIndex {
        campaign_id,
        count: entries.len(),
        shots: entries,
    };
    let mut bytes = serde_json::to_vec_pretty(&idx).expect("the shot index serializes");
    bytes.push(b'\n');
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    const POV: &[u8] = include_bytes!("../../../tests/fixtures/view/render-plan-pov.json");

    #[test]
    fn malformed_plan_is_dw0721() {
        let err = plan_shots(b"nope").unwrap_err();
        assert_eq!(err.code, DW_INPUT);
    }

    #[test]
    fn every_shot_states_its_world_sky_and_mode() {
        let (id, shots) = plan_shots(POV).unwrap();
        assert_eq!(id, "hello-world");
        assert!(!shots.is_empty());
        // hello-world's first leg walks at load; its second, to the exit,
        // stands in the world the talk's completion arrives under.
        let first = shots.iter().find(|s| s.kind == "pov").expect("a pov shot");
        assert_eq!(first.objective.as_deref(), Some("obj/talk"));
        assert_eq!(first.world, "at-load");
        let exit = shots
            .iter()
            .find(|s| s.kind == "pov" && s.objective.as_deref() == Some("obj/exit"))
            .expect("a pov shot on the exit's leg");
        assert_eq!(exit.world, "after-obj-talk");
        assert!(shots.iter().all(|s| s.mode == Mode::Quick));
        assert!(
            shots
                .iter()
                .all(|s| s.sky_json["daytime_ticks"].is_number())
        );
    }

    #[test]
    fn a_shot_that_states_no_world_is_refused_by_name() {
        let plan = br#"{"campaign_id":"c","shots":[{"id":"spawn","kind":"spawn",
          "camera":{"pos":[0.5,65.0,0.5],"yaw":0.0,"pitch":0.0}}]}"#;
        let err = plan_shots(plan).unwrap_err();
        assert_eq!(err.code, DW_INPUT);
        assert!(err.message.contains("`world`"), "{err:?}");
    }
}
