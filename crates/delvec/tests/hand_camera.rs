//! **A showcase camera placed by hand** (spec-0069): the harvest pass, the
//! record writer, and the build's proof that every camera in
//! `design/cameras.json` photographs the scene.
//!
//! Every test drives the real binary over real files: the loop this surface
//! serves is an agent running `delvec harvest`, `delvec place-camera` and
//! `delvec build` one after another, so the files between them are the thing
//! under test.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

mod common;

/// A scratch directory under the test binary's own target tmp.
fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("hand-camera-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn delvec(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args(args)
        .output()
        .expect("delvec runs")
}

fn log(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn json(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

/// One overlay layout, enough for the harvester.
fn layout(dir: &Path) -> PathBuf {
    let p = dir.join("layout.json");
    std::fs::write(
        &p,
        serde_json::to_vec_pretty(&serde_json::json!({
            "version": delvewright_dsl::DSL_VERSION,
            "campaign_id": "hello-world",
            "areas": [{"id": "area/keep", "prefab": "keep", "origin": [0, 64, 0], "size": [11, 6, 11]}],
            "objectives": [{"id": "obj/talk", "quest": "quest/open-the-door"}],
            "npcs": [],
            "anchors": [],
            "shots": []
        }))
        .unwrap(),
    )
    .unwrap();
    p
}

const NOTE: &str = "[08:57:05] [Server thread/INFO]: [Not Secure] [delve-creator] [DelveNote] pos=[5,65,2] area=area/keep quests=obj/talk:0 nearest_npc=none\n\
[08:57:09] [Server thread/INFO]: <delve-creator> the door sticks\n";
const SHOT: &str = "[08:58:00] [Server thread/INFO]: [Not Secure] [delve-creator] [DelveShot] shot=1 beat=1 ptr=/content/quests/0/on_complete/0 idx=0 seconds=5 look_at=5,67,4 path=5,67,8\n";
const CAMERA: &str = "[08:59:00] [Server thread/INFO]: [Not Secure] [delve-creator] [DelveCamera] slot=1 eye=5500,66620,2500 yaw=1130 pitch=1000 in=air\n\
[08:59:30] [Server thread/INFO]: [Not Secure] [delve-creator] [DelveCamera] slot=1 eye=5500,66620,3500 yaw=1130 pitch=1250 in=air\n";

/// **Criterion 4 — one harvest pass.** A log holding a note, a shot and camera
/// stamps yields all three reports from one run; two stamps on one slot keep
/// the last and count 2; a log with no camera stamp writes no camera report.
#[test]
fn one_harvest_pass_writes_every_report_the_log_carries() {
    let dir = tmp("harvest");
    let manifest = layout(&dir);
    let server_log = dir.join("server.log");
    std::fs::write(&server_log, format!("{NOTE}{SHOT}{CAMERA}")).unwrap();
    let (notes, shots, cams) = (
        dir.join("playtest-report.json"),
        dir.join("rehearsal-report.json"),
        dir.join("camera-report.json"),
    );
    let r = delvec(&[
        "harvest",
        server_log.to_str().unwrap(),
        manifest.to_str().unwrap(),
        "-o",
        notes.to_str().unwrap(),
        "--rehearsal-out",
        shots.to_str().unwrap(),
        "--camera-out",
        cams.to_str().unwrap(),
    ]);
    assert!(r.status.success(), "{}", log(&r));
    assert_eq!(json(&notes)["notes"].as_array().unwrap().len(), 1);
    assert_eq!(json(&shots)["shots"].as_array().unwrap().len(), 1);
    let report = json(&cams);
    assert_eq!(report["campaign_id"], "hello-world");
    let cameras = report["cameras"].as_array().unwrap();
    assert_eq!(cameras.len(), 1);
    assert_eq!(cameras[0]["stamps"], 2);
    assert_eq!(cameras[0]["eye"], serde_json::json!([5.5, 66.62, 3.5]));
    assert_eq!(cameras[0]["pitch"], 12.5);

    // A note-only session writes no camera report.
    let quiet = tmp("harvest-quiet");
    let quiet_log = quiet.join("server.log");
    std::fs::write(&quiet_log, format!("{NOTE}{SHOT}")).unwrap();
    let quiet_cams = quiet.join("camera-report.json");
    let r = delvec(&[
        "harvest",
        quiet_log.to_str().unwrap(),
        manifest.to_str().unwrap(),
        "-o",
        quiet.join("playtest-report.json").to_str().unwrap(),
        "--rehearsal-out",
        quiet.join("rehearsal-report.json").to_str().unwrap(),
        "--camera-out",
        quiet_cams.to_str().unwrap(),
    ]);
    assert!(r.status.success(), "{}", log(&r));
    assert!(
        !quiet_cams.exists(),
        "a log with no camera stamp writes no camera report"
    );
}

/// A campaign directory holding what `place-camera` reads: `design.json` rows.
fn design_rows(dir: &Path) {
    std::fs::write(
        dir.join("design.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "dsl_version": delvewright_dsl::DSL_VERSION,
            "campaign_id": "hello-world",
            "stage": "design",
            "content": {"references": [
                {"name": "concept/keep", "shows": "x", "time": "noon", "weather": "clear"}
            ]}
        }))
        .unwrap(),
    )
    .unwrap();
}

fn harvested_report(dir: &Path) -> PathBuf {
    let manifest = layout(dir);
    let server_log = dir.join("server.log");
    std::fs::write(&server_log, CAMERA).unwrap();
    let report = dir.join("camera-report.json");
    let r = delvec(&[
        "harvest",
        server_log.to_str().unwrap(),
        manifest.to_str().unwrap(),
        "-o",
        dir.join("playtest-report.json").to_str().unwrap(),
        "--camera-out",
        report.to_str().unwrap(),
    ]);
    assert!(r.status.success(), "{}", log(&r));
    report
}

/// **Criteria 5 and 7 — no conversion, and precedence.** A harvested pose is
/// written into the record exactly as stamped (the record speaks Minecraft's
/// rotation); an estimate written over the hand row is refused naming the row;
/// once the row is deleted the same write succeeds.
#[test]
fn a_hand_row_is_written_as_stamped_and_no_estimate_replaces_it() {
    let camp = tmp("place");
    design_rows(&camp);
    let report = harvested_report(&camp);
    let place = |extra: &[&str]| {
        let mut args = vec![
            "place-camera",
            camp.to_str().unwrap(),
            "--name",
            "keep-hero",
        ];
        args.extend_from_slice(extra);
        delvec(&args)
    };
    let hand = [
        "--report",
        report.to_str().unwrap(),
        "--slot",
        "1",
        "--fov",
        "62",
        "--answers",
        "concept/keep",
    ];
    let r = place(&hand);
    assert!(r.status.success(), "{}", log(&r));
    let record = json(&camp.join("design/cameras.json"));
    let row = &record["cameras"][0];
    assert_eq!(row["source"], "hand");
    assert_eq!(row["pos"], serde_json::json!([5.5, 66.62, 3.5]));
    assert_eq!(
        (row["yaw"].as_f64(), row["pitch"].as_f64()),
        (Some(11.3), Some(12.5))
    );
    assert_eq!(row["fov"], 62.0);

    // The estimate the bracket would have handed over.
    let cands = camp.join("candidates.json");
    let mut estimate = record.clone();
    estimate["cameras"][0]["source"] = serde_json::json!("estimated");
    estimate["cameras"][0]["name"] = serde_json::json!("keep-hero.yaw+8");
    estimate["cameras"][0]["yaw"] = serde_json::json!(19.3);
    std::fs::write(&cands, serde_json::to_vec_pretty(&estimate).unwrap()).unwrap();
    let from_estimate = [
        "--candidates",
        cands.to_str().unwrap(),
        "--pick",
        "keep-hero.yaw+8",
    ];
    let r = place(&from_estimate);
    assert_eq!(r.status.code(), Some(2), "{}", log(&r));
    assert!(log(&r).contains("DW0721"), "{}", log(&r));
    assert!(
        log(&r).contains("`keep-hero` was placed by hand"),
        "{}",
        log(&r)
    );
    assert_eq!(
        json(&camp.join("design/cameras.json")),
        record,
        "a refusal writes nothing"
    );

    let r = place(&["--delete"]);
    assert!(r.status.success(), "{}", log(&r));
    assert!(!camp.join("design/cameras.json").exists());
    let r = place(&from_estimate);
    assert!(r.status.success(), "{}", log(&r));
    let row = &json(&camp.join("design/cameras.json"))["cameras"][0];
    assert_eq!(row["source"], "estimated");
    assert_eq!(row["yaw"], 19.3);

    // A slot nobody stamped, and a new hand row with no image to answer.
    let r = place(&[
        "--report",
        report.to_str().unwrap(),
        "--slot",
        "4",
        "--fov",
        "70",
    ]);
    assert_eq!(r.status.code(), Some(2), "{}", log(&r));
    assert!(log(&r).contains("slot 4"), "{}", log(&r));
    let r = delvec(&[
        "place-camera",
        camp.to_str().unwrap(),
        "--name",
        "cellar",
        "--report",
        report.to_str().unwrap(),
        "--slot",
        "1",
        "--fov",
        "70",
    ]);
    assert_eq!(r.status.code(), Some(2), "{}", log(&r));
    assert!(log(&r).contains("--answers"), "{}", log(&r));
    // `--fov` is asked, never assumed.
    let r = place(&["--report", report.to_str().unwrap(), "--slot", "1"]);
    assert!(!r.status.success(), "{}", log(&r));
}

// ---------------------------------------------------------------------------
// The build's proof (criterion 6) and byte identity (criterion 11)
// ---------------------------------------------------------------------------

fn camera_row(name: &str, source: &str, pos: [f64; 3], yaw: f64, pitch: f64) -> serde_json::Value {
    serde_json::json!({
        "answers": "concept/keep", "exposure": 1.0, "fov": 70.0, "height": 90,
        "name": name, "pitch": pitch, "pos": pos, "source": source, "spp": 16,
        "width": 160, "yaw": yaw
    })
}

fn hello_with(tag: &str, cameras: Option<serde_json::Value>) -> PathBuf {
    let camp = tmp(&format!("camp-{tag}"));
    common::copy_dir_all(&common::hello_world_dir(), &camp);
    if let Some(cams) = cameras {
        std::fs::create_dir_all(camp.join("design")).unwrap();
        std::fs::write(
            camp.join("design/cameras.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "campaign_id": "hello-world", "cameras": cams
            }))
            .unwrap(),
        )
        .unwrap();
    }
    camp
}

fn build(tag: &str, camp: &Path) -> (i32, String, PathBuf) {
    let out = tmp(&format!("out-{tag}"));
    let r = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        common::prefabs_dir().to_str().unwrap(),
    ]);
    (r.status.code().unwrap_or(-1), log(&r), out)
}

/// The eye of hello-world's first player-POV shot: a point the build itself
/// proves clear, and the floor cell under the body standing there.
fn clear_eye_and_floor(tag: &str) -> ([f64; 3], [f64; 3], serde_json::Value) {
    let (code, out, dir) = build(tag, &hello_with(tag, None));
    assert_eq!(code, 0, "{out}");
    let plan = json(&dir.join("render-plan.json"));
    assert!(
        plan["camera_eye_proof"].get("showcase").is_none(),
        "a campaign with no camera record states no showcase count"
    );
    let pov = plan["shots"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["kind"] == "pov")
        .expect("hello-world has a walked POV")
        .clone();
    let pos: Vec<f64> = serde_json::from_value(pov["camera"]["pos"].clone()).unwrap();
    let stand: Vec<i32> = serde_json::from_value(pov["standing_cell"].clone()).unwrap();
    let floor = [
        f64::from(stand[0]) + 0.5,
        f64::from(stand[1]) - 0.5,
        f64::from(stand[2]) + 0.5,
    ];
    (pos.try_into().unwrap(), floor, plan)
}

#[test]
fn the_build_proves_every_showcase_camera_photographs_the_scene() {
    let (eye, floor, plain) = clear_eye_and_floor("plain-proof");

    // Green: a hand camera at a proven-clear eye looking into the keep.
    let camp = hello_with(
        "green",
        Some(serde_json::json!([camera_row(
            "hero", "hand", eye, 0.0, 10.0
        )])),
    );
    let (code, out, dir) = build("green", &camp);
    assert_eq!(code, 0, "{out}");
    let plan = json(&dir.join("render-plan.json"));
    assert_eq!(plan["camera_eye_proof"]["showcase"], 1);
    // Never pulled in: the record's camera is not a plan shot, so nothing moved
    // it and the plan's own cameras are the plain build's.
    assert_eq!(plan["shots"], plain["shots"]);
    assert_eq!(
        plan["camera_eye_proof"]["pulled_in"],
        plain["camera_eye_proof"]["pulled_in"]
    );

    // Red: the same row with its eye inside the floor, placed by hand — and the
    // estimate in the same cell reds identically, with its own remedy.
    for (source, remedy) in [
        ("hand", "fire `/trigger dw.cam` again"),
        ("estimated", "move the camera"),
    ] {
        let camp = hello_with(
            &format!("buried-{source}"),
            Some(serde_json::json!([camera_row(
                "hero", source, floor, 0.0, 10.0
            )])),
        );
        let (code, out, _) = build(&format!("buried-{source}"), &camp);
        assert_eq!(code, 3, "{source}: {out}");
        assert!(out.contains("DW0724"), "{source}: {out}");
        assert!(out.contains("showcase camera `hero`"), "{source}: {out}");
        assert!(out.contains(remedy), "{source}: {out}");
    }

    // Red: a clear eye above the keep, looking up at empty sky.
    let sky = [eye[0], 120.0, eye[2]];
    let camp = hello_with(
        "sky",
        Some(serde_json::json!([camera_row(
            "hero", "hand", sky, 0.0, -45.0
        )])),
    );
    let (code, out, _) = build("sky", &camp);
    assert_eq!(code, 3, "{out}");
    assert!(
        out.contains("DW0724") && out.contains("framed extent"),
        "{out}"
    );

    // Red: a record that is not a record stops the build under its own code.
    let camp = hello_with("bad-record", Some(serde_json::json!([{"name": "hero"}])));
    let (code, out, _) = build("bad-record", &camp);
    assert_eq!(code, 3, "{out}");
    assert!(out.contains("DW0721"), "{out}");
}

/// **Criterion 11 — byte identity.** Two builds of a campaign with a hand row
/// write byte-identical `render-plan.json` and `manifest.json`, and `delvec
/// cameras` over each writes byte-identical scenes.
#[test]
fn a_hand_row_builds_byte_identically() {
    let (eye, _, _) = clear_eye_and_floor("plain-twice");
    let rows = serde_json::json!([camera_row("hero", "hand", eye, 0.0, 10.0)]);
    // `delvec cameras` refuses a world save that is not there; the scene bytes
    // name the save by path and depend on nothing it holds.
    let world = tmp("world");
    std::fs::create_dir_all(world.join("region")).unwrap();
    std::fs::write(world.join("level.dat"), b"x").unwrap();
    std::fs::write(world.join("region/r.0.0.mca"), b"x").unwrap();
    let mut outs = Vec::new();
    for run in ["a", "b"] {
        let camp = hello_with(&format!("twice-{run}"), Some(rows.clone()));
        design_rows(&camp);
        std::fs::create_dir_all(camp.join("design/concept")).unwrap();
        std::fs::write(camp.join("design/concept/keep.png"), b"not read").unwrap();
        let (code, out, dir) = build(&format!("twice-{run}"), &camp);
        assert_eq!(code, 0, "{out}");
        let scenes = tmp(&format!("scenes-{run}"));
        let r = delvec(&[
            "cameras",
            dir.to_str().unwrap(),
            "--campaign",
            camp.to_str().unwrap(),
            "-o",
            scenes.to_str().unwrap(),
            "--world",
            world.to_str().unwrap(),
        ]);
        assert!(r.status.success(), "{}", log(&r));
        let index = scenes.join("shot-index.json");
        let r = delvec(&[
            "index",
            dir.to_str().unwrap(),
            "-o",
            index.to_str().unwrap(),
        ]);
        assert!(r.status.success(), "{}", log(&r));
        outs.push((
            std::fs::read(dir.join("render-plan.json")).unwrap(),
            json(&dir.join("manifest.json")),
            std::fs::read(scenes.join("hello-world_camera_hero.json")).unwrap(),
            std::fs::read(&index).unwrap(),
        ));
    }
    assert_eq!(outs[0].0, outs[1].0, "render-plan.json");
    assert_eq!(outs[0].2, outs[1].2, "the hand row's scene");
    assert_eq!(outs[0].3, outs[1].3, "the shot index");
    let inputs = &outs[0].1["inputs"];
    assert!(
        inputs.to_string().contains("design/cameras.json"),
        "the record is a hashed build input: {inputs}"
    );
}
