//! CLI-level tests for the frame-drawing arms `delvec` carries: `frames`,
//! `panorama` and `cameras` — their input and output error paths, the world
//! a frame names, the manifest beside every frame, and the record's sky lines.
//! Every frame here is drawn on the CPU backend with the pinned client jar's
//! textures (`$DELVEWRIGHT_CLIENT_JAR`, or Init I5's location).

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn tmp(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("delvec-view-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn scene_json(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn frames_missing_render_plan_is_dw0721_exit2() {
    let build_dir = tmp("frames-missing-plan");
    let result = Command::new(BIN)
        .args(["frames"])
        .arg(&build_dir)
        .args(["-o"])
        .arg(build_dir.join("out"))
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2), "{result:?}");
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("DW0721"), "expected DW0721: {stderr}");
}

#[test]
fn frames_output_blocked_is_dw0722_exit3() {
    let dir = tmp("frames-blocked-out");
    let built = hello_build(&dir);
    // Block the output path with a regular file so `create_dir_all(out)` fails.
    let out = dir.join("out-is-a-file");
    std::fs::write(&out, b"blocker").unwrap();
    let result = Command::new(BIN)
        .args(["frames"])
        .arg(&built)
        .args(["-o"])
        .arg(&out)
        .args(["--size", "16", "--backend", "cpu"])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(3), "{result:?}");
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("DW0722"), "expected DW0722: {stderr}");
}

/// A frame over a world the build did not write is refused before anything is
/// drawn, naming the key and the rebuild — for a review frame and for the
/// panorama, which stands in the load world.
#[test]
fn a_frame_over_no_written_world_is_refused_and_nothing_is_written() {
    let mut judged = 0;
    for arm in ["frames", "panorama"] {
        let dir = tmp(&format!("{arm}-no-world"));
        let built = hello_build(&dir);
        std::fs::remove_dir_all(built.join("validation/worlds/at-load")).unwrap();
        let out = dir.join("frames");
        let mut cmd = Command::new(BIN);
        cmd.arg(arm)
            .arg(&built)
            .arg("-o")
            .arg(&out)
            .args(["--backend", "cpu"]);
        let result = cmd.output().unwrap();
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert_eq!(result.status.code(), Some(2), "{arm}: {stderr}");
        assert!(stderr.contains("DW0721"), "{arm}: {stderr}");
        assert!(stderr.contains("at-load"), "{arm}: names the key: {stderr}");
        assert!(
            stderr.contains("delvec build"),
            "{arm}: no remedy: {stderr}"
        );
        assert!(!out.exists(), "{arm}: a frame dir was written");
        judged += 1;
    }
    assert_eq!(judged, 2);
}

fn panorama(built: &Path, out: &Path, extra: &[&str]) -> std::process::Output {
    Command::new(BIN)
        .args(["panorama"])
        .arg(built)
        .args(["-o"])
        .arg(out)
        .args([
            "--width",
            "64",
            "--height",
            "36",
            "--spp",
            "1",
            "--backend",
            "cpu",
        ])
        .args(extra)
        .output()
        .unwrap()
}

/// The storybook exterior: an oblique camera framing the placed areas, drawn
/// hq from the load world the build wrote, with its manifest beside it.
#[test]
fn panorama_draws_a_framed_frame_of_the_built_place() {
    let dir = tmp("panorama-ok");
    let built = hello_build(&dir);
    let out = dir.join("frames");
    let result = panorama(&built, &out, &[]);
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    assert!(out.join("hello-world_panorama_se_45.png").is_file());
    let v = scene_json(&out.join("hello-world_panorama_se_45.manifest.json"));
    assert_eq!(v["mode"], "hq");
    assert_eq!(v["spp"], 1);
    assert_eq!(v["world"], "at-load");
    assert_eq!(v["image"]["width"], 64);
    let plan = scene_json(&built.join("render-plan.json"));
    let max = &plan["layout_aabb"]["max"];
    // 45° oblique from the south-east: the camera sits +X/+Z of the layout and
    // above it, looking north-west and down.
    let pos = &v["camera"]["pos"];
    assert!(pos[0].as_f64().unwrap() > max[0].as_f64().unwrap(), "{v}");
    assert!(pos[2].as_f64().unwrap() > max[2].as_f64().unwrap(), "{v}");
    assert!(pos[1].as_f64().unwrap() > max[1].as_f64().unwrap(), "{v}");
    assert!(v["subject"]["span"]["fill"].as_f64().unwrap() > 0.0, "{v}");
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.starts_with("renderer: lumenwright "), "{stderr}");
    assert!(
        stderr.contains("covers") && stderr.contains("% of the frame"),
        "{stderr}"
    );
}

/// The creator's choices — side, pitch and frame — reach the frame and its
/// name, and a pitch outside the range is refused at the flag.
#[test]
fn panorama_takes_its_bearing_pitch_and_frame_from_the_flags() {
    let dir = tmp("panorama-flags");
    let built = hello_build(&dir);
    let out = dir.join("frames");
    let result = panorama(&built, &out, &["--bearing", "n", "--pitch", "30"]);
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    let v = scene_json(&out.join("hello-world_panorama_n_30.manifest.json"));
    let plan = scene_json(&built.join("render-plan.json"));
    // Looking south: the camera is north of the layout (−Z).
    assert!(
        v["camera"]["pos"][2].as_f64().unwrap() < plan["layout_aabb"]["min"][2].as_f64().unwrap(),
        "{v}"
    );
    for bad in [
        ["--pitch", "90"],
        ["--pitch", "4"],
        ["--width", "0"],
        ["--bearing", "up"],
    ] {
        let result = panorama(&built, &out, &bad);
        assert_eq!(result.status.code(), Some(2), "{bad:?}: {result:?}");
    }
}

/// A hello-world copy carrying what `delvec cameras` reads beside the stage
/// documents: `design.json` rows and a `design/cameras.json` record. `delvec
/// cameras` assembles the campaign to write the world each camera stands in
/// (spec-0089), so the record stands in a campaign that plans.
fn camera_campaign(dir: &Path, cameras: serde_json::Value) {
    common::copy_dir_all(&common::hello_world_dir(), dir);
    std::fs::create_dir_all(dir.join("design")).unwrap();
    std::fs::write(
        dir.join("design.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "dsl_version": delvewright_dsl::DSL_VERSION,
            "campaign_id": "hello-world",
            "stage": "design",
            "content": {"references": [
                {"name": "concept/gate", "shows": "the gate", "time": "dusk", "weather": "clear"},
                {"name": "concept/hall", "shows": "the hall", "time": "dusk", "weather": "clear"}
            ]}
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::write(
        dir.join("design/cameras.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"campaign_id": "hello-world", "cameras": cameras}),
        )
        .unwrap(),
    )
    .unwrap();
}

/// hello-world built once into `<dir>/build`, with no camera record: the
/// render plan `delvec cameras` reads.
fn hello_build(dir: &Path) -> PathBuf {
    let out = dir.join("build");
    let r = Command::new(BIN)
        .arg("build")
        .arg(common::hello_world_dir())
        .arg("-o")
        .arg(&out)
        .arg("--prefabs")
        .arg(common::prefabs_dir())
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    out
}

/// `delvec cameras` over `built`, the campaign assembled from the pinned
/// library.
fn cameras_cmd(built: &Path, campaign: &Path, out: &Path) -> Command {
    let mut c = Command::new(BIN);
    c.arg("--prefabs")
        .arg(common::prefabs_dir())
        .arg("cameras")
        .arg(built)
        .arg("--campaign")
        .arg(campaign)
        .arg("-o")
        .arg(out)
        .args(["--backend", "cpu"]);
    c
}

fn a_camera(name: &str, answers: &str) -> serde_json::Value {
    serde_json::json!({
        "answers": answers, "fov": 55.0, "height": 90, "name": name,
        "pitch": 12.0, "pos": [9.5, 72.0, -6.0], "source": "estimated", "spp": 4, "width": 160, "yaw": 20.0
    })
}

/// **A stated exposure is refused where the record is read** (`DW1010`):
/// `delvec cameras` exits 2, naming the camera and the remedy, and writes no
/// scene. The build's half is `design_record.rs`.
#[test]
fn a_record_that_states_an_exposure_is_refused_by_the_cameras_command() {
    let dir = tmp("cameras-exposure");
    let built = hello_build(&dir);
    let campaign = dir.join("campaign");
    let mut hall = a_camera("hall", "concept/hall");
    hall["exposure"] = 16.0.into();
    camera_campaign(
        &campaign,
        serde_json::json!([a_camera("gate", "concept/gate"), hall]),
    );
    let out = dir.join("scenes");
    let r = cameras_cmd(&built, &campaign, &out).output().unwrap();
    let stderr = String::from_utf8_lossy(&r.stderr);
    assert_eq!(r.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("DW1010"), "{stderr}");
    assert!(stderr.contains("camera `hall`"), "{stderr}");
    assert!(stderr.contains("light the room"), "{stderr}");
    assert!(!out.join("hello-world_camera_gate.png").exists());
}

/// `delvec cameras` emits one scene per stated camera, verbatim, names the
/// approved images no camera answers, and with `--bracket` writes every
/// candidate back out in the record format.
#[test]
fn cameras_emits_the_stated_cameras_and_their_candidates() {
    let build_dir = tmp("cameras-ok");
    let built = hello_build(&build_dir);
    let campaign = build_dir.join("campaign");
    // Every approved image is answered, or nothing is emitted (spec-0070): the
    // emitting fixture answers both rows.
    camera_campaign(
        &campaign,
        serde_json::json!([
            a_camera("gate", "concept/gate"),
            a_camera("hall", "concept/hall")
        ]),
    );
    let out = build_dir.join("scenes");

    let result = cameras_cmd(&built, &campaign, &out).output().unwrap();
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    let v = scene_json(&out.join("hello-world_camera_gate.manifest.json"));
    assert_eq!(v["image"]["path"], "hello-world_camera_gate.png");
    assert_eq!(v["image"]["width"], 160);
    assert_eq!(v["mode"], "hq");
    assert_eq!(v["camera"]["fov"], 55.0);
    assert_eq!(v["camera"]["pos"][2], -6.0);
    assert_eq!(v["world"]["key"], "at-load");
    assert!(out.join("hello-world_camera_gate.png").is_file());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("answers: 2 of 2 approved image(s)"),
        "{stderr}"
    );
    assert!(
        !stderr.contains("none answers"),
        "a complete record names nothing unanswered: {stderr}"
    );

    let bracketed = build_dir.join("bracketed");
    let result = cameras_cmd(&built, &campaign, &bracketed)
        .args(["--bracket", "yaw=10,truck=2", "--draft"])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    let candidates = scene_json(&bracketed.join("candidates.json"));
    let names: Vec<&str> = candidates["cameras"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "gate",
            "gate.yaw+10",
            "gate.truck+2",
            "gate.yaw-10",
            "gate.truck-2",
            "hall",
            "hall.yaw+10",
            "hall.truck+2",
            "hall.yaw-10",
            "hall.truck-2"
        ]
    );
    for n in &names {
        let draft =
            scene_json(&bracketed.join(format!("hello-world_camera_{n}_draft.manifest.json")));
        assert_eq!(draft["image"]["width"], 40, "{n}");
    }
}

/// A camera answering no approved image, or a record that is not the record, is
/// refused before anything is written.
#[test]
fn cameras_refuses_a_camera_that_answers_nothing() {
    let build_dir = tmp("cameras-refused");
    let built = hello_build(&build_dir);
    for (tag, cameras) in [
        (
            "stray",
            serde_json::json!([a_camera("gate", "concept/cellar")]),
        ),
        ("none", serde_json::json!([])),
        (
            "bad-name",
            serde_json::json!([a_camera("Gate", "concept/gate")]),
        ),
    ] {
        let campaign = build_dir.join(tag);
        camera_campaign(&campaign, cameras);
        let out = build_dir.join(format!("out-{tag}"));
        let result = cameras_cmd(&built, &campaign, &out).output().unwrap();
        assert_eq!(result.status.code(), Some(2), "{tag}: {result:?}");
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("DW0721"),
            "{tag}: {result:?}"
        );
        assert!(
            !out.exists(),
            "{tag}: a refused record wrote {}",
            out.display()
        );
    }
}

/// **Scene emission is held while an approved image is unanswered**
/// (`DW0900`, spec-0070 criterion 6): with one camera for two rows, `delvec
/// cameras` exits 2 and writes no scene — with `--only` naming the one camera
/// that IS answered too, because the record is what is judged and `--only`
/// exempts nothing.
#[test]
fn cameras_refuses_a_record_that_leaves_an_approved_image_unanswered() {
    let build_dir = tmp("cameras-unanswered");
    let built = hello_build(&build_dir);
    let campaign = build_dir.join("campaign");
    camera_campaign(
        &campaign,
        serde_json::json!([a_camera("gate", "concept/gate")]),
    );
    for (tag, extra) in [("all", vec![]), ("only", vec!["--only", "gate"])] {
        let out = build_dir.join(format!("out-{tag}"));
        let result = cameras_cmd(&built, &campaign, &out)
            .args(&extra)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2), "{tag}: {result:?}");
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(stderr.contains("DW0900"), "{tag}: {stderr}");
        assert!(
            stderr.contains("`concept/hall`") && stderr.contains("the hall"),
            "{tag}: the refusal names the row and what it shows: {stderr}"
        );
        assert!(
            !out.exists(),
            "{tag}: a refused record wrote {}",
            out.display()
        );
    }
}

/// **The sky binding line** (spec-0079 §6, criterion 9): `delvec cameras` prints
/// one `sky:` line per camera and the `skies:` summary, every run. A record that
/// states no sky prints `0 stated`, and — its rows being clear — the empty set of
/// weathers, which is the line a creator whose rain delve renders clear reads it
/// on; a record whose camera states a rain sky prints `1 stated` and `{rain}`.
/// Refused or not: a camera restating its row's sky is counted in the
/// denominator as refused.
#[test]
fn cameras_prints_the_sky_binding_line() {
    let build_dir = tmp("cameras-sky-line");
    let built = hello_build(&build_dir);
    let mut rain = a_camera("hall", "concept/hall");
    rain["sky"] = serde_json::json!({"time": "dusk", "weather": "rain"});
    let mut restated = a_camera("hall", "concept/hall");
    restated["sky"] = serde_json::json!({"time": "dusk", "weather": "clear"});
    for (tag, hall, code, lines) in [
        (
            "derived",
            a_camera("hall", "concept/hall"),
            0,
            vec![
                "sky: gate dusk+clear derived from concept/gate — clear sky at daytime 12000: the \
                 client's sky colours, its light 0.690126 (the lightmap's, peak channel), sun drawn",
                "sky: hall dusk+clear derived from concept/hall —",
                "skies: 2 derived, 0 stated, over 2 camera(s); weathers emitted: {}",
            ],
        ),
        (
            "stated",
            rain,
            0,
            vec![
                "sky: hall dusk+rain stated — rain sky at daytime 12000: the client's sky colours, \
                 its light 0.55433 (the lightmap's, peak channel), sun not drawn",
                "skies: 1 derived, 1 stated, over 2 camera(s); weathers emitted: {rain}",
            ],
        ),
        (
            "refused",
            restated,
            2,
            vec!["skies: 1 derived, 0 stated, over 2 camera(s) (1 refused); weathers emitted: {}"],
        ),
    ] {
        let campaign = build_dir.join(tag);
        camera_campaign(
            &campaign,
            serde_json::json!([a_camera("gate", "concept/gate"), hall]),
        );
        let result = cameras_cmd(&built, &campaign, &build_dir.join(format!("out-{tag}")))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert_eq!(result.status.code(), Some(code), "{tag}: {stderr}");
        for line in lines {
            assert!(stderr.contains(line), "{tag}: `{line}` in:\n{stderr}");
        }
    }
}

/// A panorama fitted to the building its anchors stand in is framed on their
/// span, not on the whole layout, and says so in its name.
#[test]
fn panorama_frames_the_building_its_anchors_name() {
    let dir = tmp("panorama-anchors");
    let built = hello_build(&dir);
    let out = dir.join("frames");
    let result = panorama(
        &built,
        &out,
        &["--subject", "anchor/keeper-stand,anchor/exit"],
    );
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("camera record"), "{stderr}");
    assert!(out.join("hello-world_panorama_se_45_anchors.png").is_file());
    let layout = panorama(&built, &out, &[]);
    assert_eq!(layout.status.code(), Some(0), "{layout:?}");
    let narrow = scene_json(&out.join("hello-world_panorama_se_45_anchors.manifest.json"));
    let wide = scene_json(&out.join("hello-world_panorama_se_45.manifest.json"));
    // A smaller subject from the same side is a closer camera.
    assert!(
        narrow["camera"]["pos"][1].as_f64().unwrap() < wide["camera"]["pos"][1].as_f64().unwrap(),
        "{narrow} vs {wide}"
    );
    let refused = panorama(&built, &out, &["--subject", "anchor/nowhere"]);
    assert_eq!(refused.status.code(), Some(2), "{refused:?}");
}

/// spec-0080 §5.3: a palette for a scene inside a place is derived under the
/// biome the build's map says that place stands in, read from the build's own
/// datapack — and a place no atmosphere reaches stands in the ground biome.
#[test]
fn a_palette_inside_a_place_is_derived_under_that_places_biome() {
    let build = tmp("palette-place");
    std::fs::create_dir_all(build.join("validation")).unwrap();
    std::fs::write(
        build.join("validation/biome-map.json"),
        serde_json::to_string(&serde_json::json!({
            "ground": "demo:void",
            "places": [{ "place": "node/far", "atmosphere": "atmosphere/wrong-place",
                         "biome": "demo:atmosphere/wrong-place", "precipitates": false,
                         "cells": [[0, 64, 0], [7, 71, 7]] }]
        }))
        .unwrap(),
    )
    .unwrap();
    let biomes = build.join("datapack/data/demo/worldgen/biome");
    std::fs::create_dir_all(biomes.join("atmosphere")).unwrap();
    std::fs::write(
        biomes.join("atmosphere/wrong-place.json"),
        r##"{"effects":{"grass_color":"#6b6a2a"},"temperature":0.8,"downfall":0.4}"##,
    )
    .unwrap();
    std::fs::write(
        biomes.join("void.json"),
        r##"{"effects":{"water_color":"#3f76e4"}}"##,
    )
    .unwrap();
    // An empty asset source: the table's biome is what is asserted, not a texture.
    let textures = tmp("palette-place-textures");
    let piece = common_prefab();
    let run = |place: &str, out: &Path| {
        Command::new(BIN)
            .arg("palette")
            .arg(&piece)
            .arg("-o")
            .arg(out)
            .arg("--build")
            .arg(&build)
            .arg("--place")
            .arg(place)
            .arg("--textures")
            .arg(&textures)
            .output()
            .unwrap()
    };
    let far = build.join("far.json");
    let o = run("node/far", &far);
    let stderr = String::from_utf8_lossy(&o.stderr);
    assert!(o.status.success(), "{stderr}");
    assert!(
        stderr.contains("read from the build's datapack"),
        "{stderr}"
    );
    assert_eq!(scene_json(&far)["biome"], "demo:atmosphere/wrong-place");
    let near = build.join("near.json");
    let o = run("node/near", &near);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(
        scene_json(&near)["biome"],
        "demo:void",
        "a place no atmosphere reaches"
    );
}

/// The hello-room piece every fixture binds, from the pinned library.
fn common_prefab() -> PathBuf {
    common::prefabs_dir().join("hello-room.nbt")
}
