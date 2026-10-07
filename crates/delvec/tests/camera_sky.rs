//! **A picture states its sky** (spec-0079): a showcase camera renders under the
//! `time` and `weather` of the `design.json` row it answers, or under a `sky` it
//! states; the panorama and the review frames render under the plan's; and the
//! record's sky rule is one answer from `delvec cameras` and `delvec
//! place-camera` (the build's half is in `remedy_reachability.rs`).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use delvec::compiler::view::camera::{
    self, ApprovedRow, Camera, CameraSheet, CameraSky, EmitOptions, Source,
};
use delvec::compiler::view::{panorama, scene};
use delvewright_dsl::{WorldTime, WorldWeather};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");
/// A plan of a delve played at `day` in the `clear`.
const OCEAN: &[u8] = include_bytes!("fixtures/view/render-plan-ocean.json");
/// The mini fixture played at `dusk` in the `clear`.
const MINI: &[u8] = include_bytes!("fixtures/view/render-plan-mini.json");

/// Every camera of `sheet` standing in one stub world: these tests are about
/// the scene's sky, not the world it loads.
fn stood(sheet: &CameraSheet) -> EmitOptions {
    EmitOptions {
        world_paths: sheet
            .cameras
            .iter()
            .map(|c| (c.name.clone(), "/abs/world".to_string()))
            .collect(),
        ..EmitOptions::default()
    }
}

fn tmp(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("camera-sky-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn log(o: &Output) -> String {
    format!(
        "exit {:?}\n--- stdout\n{}\n--- stderr\n{}",
        o.status.code(),
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn cam(name: &str, answers: &str, sky: Option<CameraSky>) -> Camera {
    Camera {
        after: None,
        answers: answers.into(),
        exposure: 1.0,
        fov: 60.0,
        height: 90,
        name: name.into(),
        pitch: 10.0,
        pos: [10.5, 70.0, -4.25],
        sky,
        source: Source::Estimated,
        spp: 16,
        width: 160,
        yaw: 30.0,
    }
}

fn row(name: &str, time: WorldTime, weather: WorldWeather) -> ApprovedRow {
    ApprovedRow {
        name: name.into(),
        shows: String::new(),
        sky: Some(CameraSky { time, weather }),
    }
}

fn scene_of(plan: &[u8], c: Camera, rows: &[ApprovedRow]) -> serde_json::Value {
    let sheet = CameraSheet {
        campaign_id: camera::plan_campaign_id(plan).unwrap(),
        cameras: vec![c],
    };
    let e = camera::emit(plan, &sheet, rows, &stood(&sheet)).unwrap();
    serde_json::from_slice(&e.scenes[0].1).unwrap()
}

/// A plan with its `sky.weather` rewritten.
fn with_weather(plan: &[u8], weather: &str) -> Vec<u8> {
    let mut v: serde_json::Value = serde_json::from_slice(plan).unwrap();
    v["sky"]["weather"] = serde_json::json!(weather);
    serde_json::to_vec(&v).unwrap()
}

/// **Criterion 2 — default and override.** A camera with no `sky` answering a
/// `midnight`+`thunder` row, in a world played at `day` in the `clear`, emits a
/// midnight sun and the thunder block of the `below` class; a camera stating
/// `noon`+`clear` against that row emits a noon sun and nothing of the row's.
#[test]
fn a_camera_takes_its_pictures_sky_or_the_one_it_states() {
    let rows = [row(
        "concept/crypt",
        WorldTime::Midnight,
        WorldWeather::Thunder,
    )];
    let derived = scene_of(OCEAN, cam("crypt", "concept/crypt", None), &rows);
    let midnight = scene::sun_at(WorldTime::Midnight.daytime_ticks());
    assert_eq!(derived["sun"]["altitude"], midnight.altitude, "{derived}");
    assert_eq!(derived["sky"]["mode"], "SOLID_COLOR", "{derived}");
    assert_eq!(
        derived["sky"]["skyLight"],
        scene::BELOW_THUNDER.sky_light,
        "{derived}"
    );
    assert_eq!(
        derived["fog"]["uniformDensity"],
        scene::BELOW_THUNDER.fog_density,
        "{derived}"
    );

    let stated = scene_of(
        OCEAN,
        cam(
            "crypt",
            "concept/crypt",
            Some(CameraSky {
                time: WorldTime::Noon,
                weather: WorldWeather::Clear,
            }),
        ),
        &rows,
    );
    let noon = scene::sun_at(WorldTime::Noon.daytime_ticks());
    assert_eq!(stated["sun"]["altitude"], noon.altitude, "{stated}");
    assert!(
        stated.get("sky").is_none() && stated.get("fog").is_none(),
        "{stated}"
    );
    assert!(stated["sun"].get("intensity").is_none(), "{stated}");

    // A stated overcast sky against a clear row: the stated pair, wholly.
    let stormy = scene_of(
        OCEAN,
        cam(
            "crypt",
            "concept/quay",
            Some(CameraSky {
                time: WorldTime::Dusk,
                weather: WorldWeather::Rain,
            }),
        ),
        &[row("concept/quay", WorldTime::Day, WorldWeather::Clear)],
    );
    assert_eq!(
        stormy["sun"]["altitude"],
        scene::sun_at(WorldTime::Dusk.daytime_ticks()).altitude
    );
    assert_eq!(stormy["sky"]["skyLight"], scene::LOW_RAIN.sky_light);
}

/// **Criterion 2 — each scene kind reads its sky from its own source** (§5): of
/// one plan played at `dusk` in the `rain`, the panorama and a review frame are
/// overcast, and a showcase camera stating `dusk`+`clear` — against a row drawn
/// in that rain — is not; a camera deriving a `clear` row is not either, because
/// a camera's sky is its picture's, not the plan's.
#[test]
fn each_scene_kind_reads_its_sky_from_its_own_source() {
    let rain = with_weather(MINI, "rain");
    let pano =
        panorama::panorama_from_plan(&rain, &[], &panorama::PanoramaOptions::default()).unwrap();
    let pano: serde_json::Value = serde_json::from_slice(&pano.bytes).unwrap();
    assert_eq!(pano["sky"]["skyLight"], scene::LOW_RAIN.sky_light, "{pano}");

    let review = scene::scenes_from_plan(&rain, &scene::SceneOptions::default(), &[]).unwrap();
    assert!(!review.is_empty());
    for (name, bytes) in &review {
        let v: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        assert_eq!(v["sky"]["skyLight"], scene::LOW_RAIN.sky_light, "{name}");
    }

    let stated_clear = scene_of(
        &rain,
        cam(
            "gate",
            "concept/gate",
            Some(CameraSky {
                time: WorldTime::Dusk,
                weather: WorldWeather::Clear,
            }),
        ),
        &[row("concept/gate", WorldTime::Dusk, WorldWeather::Rain)],
    );
    assert!(stated_clear.get("sky").is_none(), "{stated_clear}");

    let derived_clear = scene_of(
        &rain,
        cam("gate", "concept/gate", None),
        &[row("concept/gate", WorldTime::Dusk, WorldWeather::Clear)],
    );
    assert!(derived_clear.get("sky").is_none(), "{derived_clear}");
}

/// A campaign directory holding what `delvec cameras` and `delvec
/// place-camera` read: `design.json` rows (`concept/gate` at dusk in the rain)
/// and, when given, a camera record.
fn campaign(dir: &Path, cameras: Option<serde_json::Value>) {
    std::fs::create_dir_all(dir.join("design")).unwrap();
    std::fs::write(
        dir.join("design.json"),
        br#"{"campaign_id":"mini","content":{"references":[
            {"name":"concept/gate","shows":"the gate","time":"dusk","weather":"rain"}]},
            "stage":"design"}"#,
    )
    .unwrap();
    if let Some(cams) = cameras {
        std::fs::write(
            dir.join("design/cameras.json"),
            serde_json::to_vec_pretty(&serde_json::json!({"campaign_id": "mini", "cameras": cams}))
                .unwrap(),
        )
        .unwrap();
    }
}

fn a_camera(sky: Option<serde_json::Value>) -> serde_json::Value {
    let mut c = serde_json::json!({
        "answers": "concept/gate", "exposure": 2.0, "fov": 55.0, "height": 90, "name": "gate",
        "pitch": 12.0, "pos": [9.5, 72.0, -6.0], "source": "estimated", "spp": 16,
        "width": 160, "yaw": 20.0
    });
    if let Some(s) = sky {
        c["sky"] = s;
    }
    c
}

fn build_dir(tag: &str) -> PathBuf {
    let dir = tmp(tag);
    std::fs::write(dir.join("render-plan.json"), MINI).unwrap();
    std::fs::create_dir_all(dir.join("world/region")).unwrap();
    std::fs::write(dir.join("world/level.dat"), b"stub").unwrap();
    std::fs::write(dir.join("world/region/r.0.0.mca"), b"stub").unwrap();
    dir
}

fn cameras(build: &Path, camp: &Path, out: &Path) -> Output {
    Command::new(BIN)
        .arg("cameras")
        .arg(build)
        .arg("--campaign")
        .arg(camp)
        .arg("-o")
        .arg(out)
        .output()
        .unwrap()
}

/// **Criterion 3 — refusals, `delvec cameras`.** A `sky` equal to the row's, a
/// half-stated `sky`, and an unknown keyword are each refused `DW0721` with the
/// camera and the row named, and nothing is written.
#[test]
fn cameras_refuses_a_sky_that_is_not_a_judgement() {
    let build = build_dir("refusals");
    for (tag, sky, says) in [
        (
            "restated",
            serde_json::json!({"time": "dusk", "weather": "rain"}),
            "REMOVE the `sky` field",
        ),
        (
            "half",
            serde_json::json!({"time": "dusk"}),
            "missing field `weather`",
        ),
        (
            "unknown",
            serde_json::json!({"time": "dusk", "weather": "snow"}),
            "unknown variant `snow`",
        ),
    ] {
        let camp = build.join(format!("camp-{tag}"));
        campaign(&camp, Some(serde_json::json!([a_camera(Some(sky))])));
        let out = build.join(format!("out-{tag}"));
        let r = cameras(&build, &camp, &out);
        let said = log(&r);
        assert_eq!(r.status.code(), Some(2), "{tag}: {said}");
        assert!(said.contains("DW0721"), "{tag}: {said}");
        assert!(said.contains(says), "{tag}: {said}");
        assert!(
            said.contains("camera `gate`") && said.contains("concept/gate"),
            "{tag}: the camera and the row are named: {said}"
        );
        assert!(!out.exists(), "{tag}: a refused record wrote scenes");
    }
}

/// **Criterion 3 — refusals, `delvec place-camera`.** A `--sky` that does not
/// parse, and one that restates the row's sky, are refused `DW0721` and leave
/// the record unwritten; a `--sky` that differs is written into the row and the
/// row's sky line printed.
#[test]
fn place_camera_writes_a_stated_sky_and_refuses_one_that_is_not() {
    let dir = tmp("place");
    let camp = dir.join("campaign");
    campaign(&camp, None);
    let candidates = dir.join("candidates.json");
    std::fs::write(
        &candidates,
        serde_json::to_vec_pretty(
            &serde_json::json!({"campaign_id": "mini", "cameras": [a_camera(None)]}),
        )
        .unwrap(),
    )
    .unwrap();
    let place = |sky: &str| {
        Command::new(BIN)
            .arg("place-camera")
            .arg(&camp)
            .args(["--name", "gate", "--candidates"])
            .arg(&candidates)
            .args(["--pick", "gate", "--sky", sky])
            .output()
            .unwrap()
    };
    let record = camp.join("design/cameras.json");
    for (bad, says) in [
        ("dusk", "is not a sky"),
        ("teatime,rain", "`teatime` is not a time of day"),
        ("dusk,snow", "`snow` is not a weather"),
        ("dusk,rain", "REMOVE the `sky` field"),
    ] {
        let r = place(bad);
        let said = log(&r);
        assert_eq!(r.status.code(), Some(2), "{bad}: {said}");
        assert!(
            said.contains("DW0721") && said.contains(says),
            "{bad}: {said}"
        );
        assert!(said.contains("camera `gate`"), "{bad}: {said}");
        assert!(!record.exists(), "{bad}: a refused sky wrote the record");
    }
    let r = place("noon,clear");
    let said = log(&r);
    assert_eq!(r.status.code(), Some(0), "{said}");
    assert!(
        said.contains("sky: gate noon+clear stated class high"),
        "{said}"
    );
    let written: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&record).unwrap()).unwrap();
    assert_eq!(
        written["cameras"][0]["sky"],
        serde_json::json!({"time": "noon", "weather": "clear"})
    );
}

/// **Criterion 7 — the gallery reaches every weather and every class.** Every
/// camera of `gallery/design/cameras.json` is emitted in process against the
/// rows of `gallery/design.json`, under a render plan carrying the gallery's own
/// declared sky (`gallery/world.json`); the weathers emitted are `{clear, rain,
/// thunder}` and the daylight classes `{high, low, below}`, read off each
/// emitted scene, and exactly one camera states its own sky. Changing the stated
/// sky's weather from `clear` to `rain`, and the answered row's weather of a
/// derived camera, each moves that camera's scene bytes.
///
/// The plan is not the gallery's BUILT `render-plan.json`: a cargo test cannot
/// build the gallery, whose pieces the gallery job generates. A camera's sky
/// reads no byte of the plan but its campaign id; the built-plan half is
/// `tools/ci/check-whole-map-render.py`, which runs `delvec cameras` over the
/// gallery's build and asserts the same two sets.
#[test]
fn the_gallery_reaches_every_weather_and_every_class() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../gallery");
    let record = std::fs::read(root.join("design/cameras.json")).unwrap();
    let sheet = camera::parse_sheet(&record).unwrap();
    let design = std::fs::read(root.join("design.json")).unwrap();
    let rows = camera::reference_rows(&design).unwrap();
    let world: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("world.json")).unwrap()).unwrap();
    let time: WorldTime = serde_json::from_value(world["content"]["time"].clone()).unwrap();
    let plan = serde_json::to_vec(&serde_json::json!({
        "campaign_id": sheet.campaign_id,
        "layout_aabb": {"min": [0, 60, 0], "max": [64, 90, 64]},
        "sky": {
            "time": time.keyword(),
            "daytime_ticks": time.daytime_ticks(),
            "weather": world["content"]["weather"],
        },
        "shots": [],
    }))
    .unwrap();

    let emission = camera::emit(&plan, &sheet, &rows, &stood(&sheet)).unwrap();
    assert_eq!(emission.scenes.len(), sheet.cameras.len());
    let mut weathers = std::collections::BTreeMap::<&str, usize>::new();
    let mut classes = std::collections::BTreeMap::<&str, usize>::new();
    for (c, (_, bytes)) in sheet.cameras.iter().zip(&emission.scenes) {
        let v: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        let resolved = camera::resolve_sky(c, &rows).unwrap();
        // Read off the scene: the overcast cell names its weather by its values.
        let weather = match v["sky"]["skyLight"].as_f64() {
            None => "clear",
            Some(light) => {
                let class = resolved.class;
                let rain = scene::overcast_cell(class, WorldWeather::Rain).unwrap();
                let thunder = scene::overcast_cell(class, WorldWeather::Thunder).unwrap();
                if light == rain.sky_light {
                    "rain"
                } else {
                    assert_eq!(light, thunder.sky_light, "{}", c.name);
                    "thunder"
                }
            }
        };
        assert_eq!(weather, resolved.sky.weather.keyword(), "{}", c.name);
        let altitude = v["sun"]["altitude"].as_f64().unwrap().to_degrees();
        let class = if altitude >= 20.0 {
            "high"
        } else if altitude >= 0.0 {
            "low"
        } else {
            "below"
        };
        *weathers.entry(weather).or_default() += 1;
        *classes.entry(class).or_default() += 1;
    }
    let stated = sheet.cameras.iter().filter(|c| c.sky.is_some()).count();
    eprintln!(
        "gallery skies: {} camera(s); weathers {weathers:?}; classes {classes:?}; {stated} stated",
        sheet.cameras.len()
    );
    assert_eq!(
        weathers.keys().copied().collect::<Vec<_>>(),
        ["clear", "rain", "thunder"]
    );
    assert_eq!(
        classes.keys().copied().collect::<Vec<_>>(),
        ["below", "high", "low"]
    );
    assert_eq!(stated, 1, "one gallery camera states its own sky");

    // Perturbation: the stated sky's weather `clear` -> `rain` moves its bytes.
    let i = sheet.cameras.iter().position(|c| c.sky.is_some()).unwrap();
    let mut moved = sheet.clone();
    moved.cameras[i].sky.as_mut().unwrap().weather = WorldWeather::Rain;
    let after = camera::emit(&plan, &moved, &rows, &stood(&moved)).unwrap();
    assert_ne!(
        after.scenes[i], emission.scenes[i],
        "the stated sky reaches a byte"
    );
    // …and so does the answered row's weather, for a derived camera.
    let j = sheet.cameras.iter().position(|c| c.sky.is_none()).unwrap();
    let mut rows2 = rows.clone();
    let r = rows2
        .iter_mut()
        .find(|r| r.name == sheet.cameras[j].answers)
        .unwrap();
    let s = r.sky.as_mut().unwrap();
    s.weather = if s.weather == WorldWeather::Clear {
        WorldWeather::Rain
    } else {
        WorldWeather::Clear
    };
    let after = camera::emit(&plan, &sheet, &rows2, &stood(&sheet)).unwrap();
    assert_ne!(
        after.scenes[j], emission.scenes[j],
        "the row's sky reaches a byte"
    );
}
