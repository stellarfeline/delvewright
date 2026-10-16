//! **A picture states its sky** (spec-0079, spec-0112 §6.1 item 6): a showcase
//! camera is drawn under the `time` and `weather` of the `design.json` row it
//! answers, or under a `sky` it states; the panorama under the plan's; a review
//! frame under its shot's own; and the record's sky rule is one answer from
//! `delvec cameras` and `delvec place-camera` (the build's half is in
//! `remedy_reachability.rs`). The renderer is handed the engine's sky of that
//! tick and weather and derives none of its own.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use delvec::compiler::view::camera::{
    self, ApprovedRow, Camera, CameraFrame, CameraSheet, CameraSky, EmitOptions, Source,
};
use delvec::compiler::view::frames::{self, FrameSky};
use delvec::compiler::view::{panorama, scene};
use delvewright_dsl::{WorldTime, WorldWeather};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");
/// A plan of a delve played at `day` in the `clear`.
const OCEAN: &[u8] = include_bytes!("fixtures/view/render-plan-ocean.json");
/// The mini fixture played at `dusk` in the `clear`.
const MINI: &[u8] = include_bytes!("fixtures/view/render-plan-mini.json");

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

/// Whether a frame is taken under the sky of `tick` under `weather`, and the
/// renderer is handed the engine's reading of it: the client's sky and fog
/// colours and lightmap factor in quick mode ([`scene::sky_attributes`]), the
/// cast light in hq mode ([`scene::sky_cell`]), and a sun drawn only while its
/// disc is up under a clear sky.
fn carries_sky(sky: FrameSky, tick: i64, weather: WorldWeather) -> bool {
    let at = scene::sky_attributes(tick, weather);
    let cell = scene::sky_cell(tick, weather);
    let quick = frames::quick_sky(sky);
    let hq = frames::hq_sky(sky);
    let near = |a: f32, b: f64| (f64::from(a) - b).abs() < 1e-6;
    sky == FrameSky {
        daytime_ticks: tick,
        weather,
    } && near(quick.sky_factor, at.sky_light_factor)
        && near(quick.sky.x, f64::from(at.sky_color[0]) / 255.0)
        && near(quick.fog.z, f64::from(at.fog_color[2]) / 255.0)
        && near(hq.zenith.z, cell.sky_color.blue * cell.sky_light)
        && (hq.sun_color.x > 0.0) == (cell.sun_draw_texture && hq.sun_dir.y > 0.0)
}

fn sky_of(f: &CameraFrame) -> FrameSky {
    FrameSky {
        daytime_ticks: f.sky.daytime_ticks,
        weather: f.sky.weather,
    }
}

fn cam(name: &str, answers: &str, sky: Option<CameraSky>) -> Camera {
    Camera {
        after: None,
        answers: answers.into(),
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

fn frame_of(plan: &[u8], c: Camera, rows: &[ApprovedRow]) -> CameraFrame {
    let sheet = CameraSheet {
        campaign_id: camera::plan_campaign_id(plan).unwrap(),
        cameras: vec![c],
    };
    camera::frames_of(plan, &sheet, rows, &EmitOptions::default())
        .unwrap()
        .remove(0)
}

/// A plan with its `sky.weather` rewritten.
fn with_weather(plan: &[u8], weather: &str) -> Vec<u8> {
    let mut v: serde_json::Value = serde_json::from_slice(plan).unwrap();
    v["sky"]["weather"] = serde_json::json!(weather);
    serde_json::to_vec(&v).unwrap()
}

/// **Criterion 2 — default and override.** A camera with no `sky` answering a
/// `midnight`+`thunder` row, in a world played at `day` in the `clear`, is drawn
/// under the game's midnight sky under thunder; a camera stating `noon`+`clear`
/// against that row under noon and nothing of the row's.
#[test]
fn a_camera_takes_its_pictures_sky_or_the_one_it_states() {
    let rows = [row(
        "concept/crypt",
        WorldTime::Midnight,
        WorldWeather::Thunder,
    )];
    let derived = frame_of(OCEAN, cam("crypt", "concept/crypt", None), &rows);
    assert!(
        carries_sky(
            sky_of(&derived),
            WorldTime::Midnight.daytime_ticks(),
            WorldWeather::Thunder
        ),
        "{derived:?}"
    );

    let stated = frame_of(
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
    assert!(
        carries_sky(
            sky_of(&stated),
            WorldTime::Noon.daytime_ticks(),
            WorldWeather::Clear
        ),
        "{stated:?}"
    );

    // A stated rain sky against a clear row: the stated pair, wholly.
    let stormy = frame_of(
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
    assert!(
        carries_sky(
            sky_of(&stormy),
            WorldTime::Dusk.daytime_ticks(),
            WorldWeather::Rain
        ),
        "{stormy:?}"
    );
}

/// **Criterion 2 — each frame kind reads its sky from its own source** (§5): of
/// one plan played at `dusk` in the `rain`, the panorama is under its rain, and
/// a showcase camera stating `dusk`+`clear` — against a row drawn in that rain —
/// is not; a camera deriving a `clear` row is not either, because a camera's
/// sky is its picture's, not the plan's. A review frame's sky is its shot's own
/// (`delvec frames` reads it off the shot, `index::plan_shots`).
#[test]
fn each_frame_kind_reads_its_sky_from_its_own_source() {
    let rain = with_weather(MINI, "rain");
    let pano =
        panorama::panorama_from_plan(&rain, &[], &panorama::PanoramaOptions::default()).unwrap();
    let dusk = WorldTime::Dusk.daytime_ticks();
    assert!(
        carries_sky(
            FrameSky {
                daytime_ticks: pano.sky.daytime_ticks,
                weather: pano.sky.weather
            },
            dusk,
            WorldWeather::Rain
        ),
        "{pano:?}"
    );

    let stated_clear = frame_of(
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
    assert!(
        carries_sky(sky_of(&stated_clear), dusk, WorldWeather::Clear),
        "{stated_clear:?}"
    );

    let derived_clear = frame_of(
        &rain,
        cam("gate", "concept/gate", None),
        &[row("concept/gate", WorldTime::Dusk, WorldWeather::Clear)],
    );
    assert!(
        carries_sky(sky_of(&derived_clear), dusk, WorldWeather::Clear),
        "{derived_clear:?}"
    );

    // A review frame: the sky the shot states, whatever the plan's.
    let shot = serde_json::json!({"campaign_id": "mini", "shots": [{
        "id": "pov/leg0/wp0", "kind": "pov", "world": "after-obj-x",
        "camera": {"pos": [4.5, 70.0, 4.5], "yaw": 0.0, "pitch": 5.0, "fov": 70.0},
        "sky": {"time": "midnight", "daytime_ticks": 18000, "weather": "thunder",
                "set_by": "obj/x"},
        "mode": "quick"}]});
    let (_, shots) =
        delvec::compiler::view::index::plan_shots(&serde_json::to_vec(&shot).unwrap()).unwrap();
    assert!(carries_sky(shots[0].sky, 18000, WorldWeather::Thunder));
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
        "answers": "concept/gate", "fov": 55.0, "height": 90, "name": "gate",
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
        said.contains("sky: gate noon+clear stated — clear sky at daytime 6000"),
        "{said}"
    );
    let written: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&record).unwrap()).unwrap();
    assert_eq!(
        written["cameras"][0]["sky"],
        serde_json::json!({"time": "noon", "weather": "clear"})
    );
}

/// **Criterion 7 — the gallery reaches every weather and every sun band.** Every
/// camera of `gallery/design/cameras.json` is emitted in process against the
/// rows of `gallery/design.json`, under a render plan carrying the gallery's own
/// declared sky (`gallery/world.json`); the weathers emitted are `{clear, rain,
/// thunder}` and the sun stands in each band `{high, low, below}` (at or above
/// 20°, from the horizon up to 20°, under it), read off each emitted scene, and
/// exactly one camera states its own sky. Changing the stated
/// sky's weather from `clear` to `rain`, its time (the clear cell's tick), and
/// the answered row's weather of a derived camera, each moves that camera's
/// scene bytes.
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

    let emission = camera::frames_of(&plan, &sheet, &rows, &EmitOptions::default()).unwrap();
    assert_eq!(emission.len(), sheet.cameras.len());
    let mut weathers = std::collections::BTreeMap::<&str, usize>::new();
    let mut classes = std::collections::BTreeMap::<&str, usize>::new();
    for (c, f) in sheet.cameras.iter().zip(&emission) {
        let resolved = camera::resolve_sky(c, &rows).unwrap();
        let weather = f.sky.weather.keyword();
        assert!(
            carries_sky(sky_of(f), resolved.sky.time.daytime_ticks(), f.sky.weather),
            "{}: {f:?}",
            c.name
        );
        assert_eq!(weather, resolved.sky.weather.keyword(), "{}", c.name);
        let altitude = scene::sun_at(f.sky.daytime_ticks).altitude.to_degrees();
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

    // Perturbation: the stated sky's weather `clear` -> `rain` moves its frame's sky.
    let i = sheet.cameras.iter().position(|c| c.sky.is_some()).unwrap();
    let mut moved = sheet.clone();
    moved.cameras[i].sky.as_mut().unwrap().weather = WorldWeather::Rain;
    let after = camera::frames_of(&plan, &moved, &rows, &EmitOptions::default()).unwrap();
    assert_ne!(after[i].sky, emission[i].sky, "the stated sky reaches the frame");
    assert_ne!(
        frames::quick_sky(sky_of(&after[i])).fog,
        frames::quick_sky(sky_of(&emission[i])).fog,
        "and the colours the renderer is handed"
    );
    // The stated clear camera: its tick moved (`just-risen` -> `noon`) moves
    // the light the renderer is handed.
    assert_eq!(sheet.cameras[i].name, "hall-exterior");
    let mut ticked = sheet.clone();
    ticked.cameras[i].sky.as_mut().unwrap().time = WorldTime::Noon;
    let noon = camera::frames_of(&plan, &ticked, &rows, &EmitOptions::default()).unwrap();
    assert_ne!(
        frames::hq_sky(sky_of(&noon[i])).zenith,
        frames::hq_sky(sky_of(&emission[i])).zenith,
        "the stated sky's tick reaches the light"
    );
    assert!(carries_sky(
        sky_of(&noon[i]),
        WorldTime::Noon.daytime_ticks(),
        WorldWeather::Clear
    ));
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
    let after = camera::frames_of(&plan, &sheet, &rows2, &EmitOptions::default()).unwrap();
    assert_ne!(after[j].sky, emission[j].sky, "the row's sky reaches the frame");
}

/// **A night picture renders the night** (spec-0079, departure 1). The
/// Treehouse Camp's `lantern-night` shape: a camera answering a row drawn
/// under `{"moon": "high"}` in the clear, in a delve whose world opens at
/// dusk. The camera's sun is the row's hour — the moon high puts the sun at
/// the nadir — so the renderer is handed the night: the sky's lightmap factor
/// the night's, and no sun. The same camera answering a clear dusk row is
/// handed a lit sky and the sun's disc above the horizon.
#[test]
fn a_camera_at_a_night_beat_renders_the_night_sky() {
    let moon_high: WorldTime = serde_json::from_value(serde_json::json!({"moon": "high"})).unwrap();
    let rows = [
        row("concept/lantern-night", moon_high, WorldWeather::Clear),
        row("concept/evening", WorldTime::Dusk, WorldWeather::Clear),
    ];
    let night = frame_of(
        MINI,
        cam("lantern-night", "concept/lantern-night", None),
        &rows,
    );
    assert!(
        scene::sun_at(night.sky.daytime_ticks).altitude < 0.0,
        "the moon high puts the sun under the horizon"
    );
    assert!(carries_sky(sky_of(&night), 18000, WorldWeather::Clear));
    let n = frames::quick_sky(sky_of(&night));
    assert!(n.sun_dir.y < 0.0, "no sun drawn: {n:?}");
    assert_eq!(frames::hq_sky(sky_of(&night)).sun_color.x, 0.0);

    let dusk = frame_of(MINI, cam("evening", "concept/evening", None), &rows);
    assert!(carries_sky(
        sky_of(&dusk),
        WorldTime::Dusk.daytime_ticks(),
        WorldWeather::Clear
    ));
    let d = frames::quick_sky(sky_of(&dusk));
    assert!(d.sun_dir.y > 0.0 && d.sky_factor > n.sky_factor, "{d:?}");
}
