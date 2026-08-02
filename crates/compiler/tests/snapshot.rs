//! `delvec snapshot` — the visual authoring loop's viewport (spec-0015 pillars
//! 1+2), tested at the process level: framing flags, the manifest contract, the
//! partial-build guarantee, and the ADR-0006 double-run byte-identity gate on
//! **both** artifacts (PNG and manifest).

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn delvec(args: &[&str]) -> Output {
    Command::new(BIN).args(args).output().expect("run delvec")
}

fn code(out: &Output) -> i32 {
    out.status.code().unwrap_or(-1)
}

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Run `snapshot` on the keep-vertical fixture (a 3D multi-piece layout, so the
/// raycaster sees real geometry at several elevations) with extra args.
fn snap(out_png: &Path, extra: &[&str]) -> Output {
    let campaign = common::keep_vertical_dir();
    let pf = common::prefabs_dir();
    let mut args: Vec<String> = vec![
        "snapshot".into(),
        campaign.to_string_lossy().into_owned(),
        "--prefabs".into(),
        pf.to_string_lossy().into_owned(),
        "-o".into(),
        out_png.to_string_lossy().into_owned(),
        // Small frames keep the suite fast; every code path is size-independent.
        "--width".into(),
        "192".into(),
        "--height".into(),
        "108".into(),
    ];
    args.extend(extra.iter().map(|s| (*s).to_string()));
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    delvec(&refs)
}

fn manifest_of(png: &Path) -> serde_json::Value {
    let path = png.with_extension("manifest.json");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|e| panic!("manifest at {}: {e}", path.display()));
    serde_json::from_slice(&bytes).expect("manifest is JSON")
}

#[test]
fn default_snapshot_writes_a_png_and_its_manifest_sidecar() {
    let dir = tmp("snap-default");
    let png = dir.join("frame.png");
    let out = snap(&png, &[]);
    assert_eq!(
        code(&out),
        0,
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let bytes = std::fs::read(&png).expect("png written");
    assert_eq!(
        &bytes[..8],
        &[137, 80, 78, 71, 13, 10, 26, 10],
        "PNG signature"
    );
    // The sidecar is the image path with its extension replaced.
    assert!(dir.join("frame.manifest.json").is_file());
    let m = manifest_of(&png);
    assert_eq!(m["manifest_version"], 1);
    assert_eq!(m["image"]["width"], 192);
    assert_eq!(m["image"]["height"], 108);
    assert_eq!(m["image"]["path"], "frame.png");
    assert!(m["campaign_id"].as_str().is_some_and(|s| !s.is_empty()));
    assert!(
        m["camera"]["convention"]
            .as_str()
            .expect("convention documented in-band")
            .contains("yaw 0 = south"),
        "the manifest must carry its own camera convention"
    );
}

#[test]
fn manifest_targets_carry_position_screen_box_and_occlusion() {
    let dir = tmp("snap-targets");
    let png = dir.join("frame.png");
    assert_eq!(code(&snap(&png, &[])), 0);
    let m = manifest_of(&png);
    let targets = m["targets"].as_array().expect("targets array");
    assert!(!targets.is_empty(), "the overview must frame something");
    for t in targets {
        assert!(t["id"].as_str().is_some_and(|s| !s.is_empty()), "{t}");
        assert!(t["kind"].as_str().is_some(), "{t}");
        assert!(
            t["pos"].is_array() || t["box"].is_object(),
            "a target is a point or a box: {t}"
        );
        assert!(
            !(t["pos"].is_array() && t["box"].is_object()),
            "never both: {t}"
        );
        let b = &t["screen_bbox"];
        assert!(
            b["w"].as_i64().unwrap_or(0) >= 1 && b["h"].as_i64().unwrap_or(0) >= 1,
            "{t}"
        );
        assert!(
            b["x"].as_i64().unwrap_or(-1) >= 0 && b["y"].as_i64().unwrap_or(-1) >= 0,
            "{t}"
        );
        assert!(t["occluded"].is_boolean(), "{t}");
        assert!(t["distance"].is_number(), "{t}");
    }
    // Every anchor is accounted for exactly once: in frame or explicitly out.
    let out_of_frame = m["out_of_frame"].as_array().expect("out_of_frame array");
    for t in out_of_frame {
        assert!(
            t["screen_bbox"].is_null(),
            "an absent subject has no box: {t}"
        );
    }
    // The kinds spec-0015 names are all representable; at minimum a layout
    // overview of a keep sees anchors.
    assert!(
        targets
            .iter()
            .chain(out_of_frame.iter())
            .any(|t| t["kind"] == "anchor"),
        "anchors must appear in the manifest"
    );
}

#[test]
fn snapshot_is_byte_identical_across_runs() {
    // ADR-0006 for the visual tier: the same DSL + the same camera must produce
    // the same PNG bytes AND the same manifest bytes.
    // Same *file name* in two directories: the manifest records the image's
    // basename, so a differing name would be a false negative here.
    let a = tmp("snap-determinism-a").join("frame.png");
    let b = tmp("snap-determinism-b").join("frame.png");
    for p in [&a, &b] {
        assert_eq!(code(&snap(p, &["--labels"])), 0);
    }
    assert_eq!(
        std::fs::read(&a).unwrap(),
        std::fs::read(&b).unwrap(),
        "PNG bytes must be identical across runs"
    );
    assert_eq!(
        std::fs::read(a.with_extension("manifest.json")).unwrap(),
        std::fs::read(b.with_extension("manifest.json")).unwrap(),
        "manifest bytes must be identical across runs"
    );
}

#[test]
fn labels_change_the_frame_but_not_the_manifest() {
    // The overlay is a rendering concern; the structured channel must not move
    // when it is toggled (so a labelled review frame and an unlabelled
    // regression frame describe exactly the same scene).
    let dir = tmp("snap-labels");
    let plain = dir.join("plain.png");
    let labelled = dir.join("labelled.png");
    assert_eq!(code(&snap(&plain, &[])), 0);
    assert_eq!(code(&snap(&labelled, &["--labels"])), 0);
    assert_ne!(
        std::fs::read(&plain).unwrap(),
        std::fs::read(&labelled).unwrap(),
        "--labels must visibly change the frame"
    );
    let mut a = manifest_of(&plain);
    let mut b = manifest_of(&labelled);
    // Only the image filename differs.
    a["image"]["path"] = serde_json::Value::Null;
    b["image"]["path"] = serde_json::Value::Null;
    assert_eq!(a, b, "--labels must not change the manifest");
}

#[test]
fn explicit_camera_flag_places_the_eye_exactly() {
    let dir = tmp("snap-camera");
    let png = dir.join("frame.png");
    let out = snap(&png, &["--camera", "12.5,80,-4.25,135,20,50"]);
    assert_eq!(code(&out), 0, "{}", String::from_utf8_lossy(&out.stderr));
    let m = manifest_of(&png);
    assert_eq!(m["camera"]["pos"][0], 12.5);
    assert_eq!(m["camera"]["pos"][1], 80.0);
    assert_eq!(m["camera"]["pos"][2], -4.25);
    assert_eq!(m["camera"]["yaw"], 135.0);
    assert_eq!(m["camera"]["pitch"], 20.0);
    assert_eq!(m["camera"]["fov"], 50.0);
    // fov is optional and defaults.
    let png2 = dir.join("nofov.png");
    assert_eq!(code(&snap(&png2, &["--camera", "0,80,0,0,10"])), 0);
    assert_eq!(manifest_of(&png2)["camera"]["fov"], 70.0);
}

#[test]
fn malformed_camera_and_unknown_anchor_fail_with_a_useful_message() {
    let dir = tmp("snap-errors");
    let png = dir.join("frame.png");
    let out = snap(&png, &["--camera", "1,2,3"]);
    assert_eq!(code(&out), 1);
    let e = String::from_utf8_lossy(&out.stderr);
    assert!(e.contains("x,y,z,yaw,pitch"), "names the shape: {e}");

    let out = snap(&png, &["--camera", "1,2,3,4,five"]);
    assert_eq!(code(&out), 1);
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("not a number"),
        "names the bad field"
    );

    let out = snap(&png, &["--at", "anchor/does-not-exist"]);
    assert_eq!(code(&out), 1);
    let e = String::from_utf8_lossy(&out.stderr);
    assert!(
        e.contains("Known anchors"),
        "an unknown anchor must list the real ones: {e}"
    );

    let out = snap(&png, &["--shot", "no/such/shot"]);
    assert_eq!(code(&out), 1);
    let e = String::from_utf8_lossy(&out.stderr);
    assert!(
        e.contains("Available"),
        "an unknown shot lists the real ones: {e}"
    );
}

#[test]
fn at_anchor_frames_that_anchor_unoccluded() {
    // The acceptance shape of spec-0015 criterion 1: `--at <anchor>` puts that
    // anchor in frame, and (having pulled the eye into open air) sees it.
    let dir = tmp("snap-at");
    let png = dir.join("frame.png");
    let out = snap(&png, &["--at", "spawn"]);
    assert_eq!(code(&out), 0, "{}", String::from_utf8_lossy(&out.stderr));
    let m = manifest_of(&png);
    let subject = m["targets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == "spawn" && t["kind"] == "anchor")
        .expect("the --at subject must be in frame");
    assert_eq!(subject["occluded"], false, "the subject must be visible");
}

#[test]
fn orbit_bearings_move_the_camera_around_the_subject() {
    // `--orbit` is a compass bearing in Minecraft's yaw sense: 0 stands due
    // south of the subject (looking north), 90 due west (looking east).
    let dir = tmp("snap-orbit");
    let south = dir.join("s.png");
    let west = dir.join("w.png");
    assert_eq!(
        code(&snap(
            &south,
            &["--at", "spawn", "--orbit", "0", "--dist", "10"]
        )),
        0
    );
    assert_eq!(
        code(&snap(
            &west,
            &["--at", "spawn", "--orbit", "90", "--dist", "10"]
        )),
        0
    );
    let (a, b) = (manifest_of(&south), manifest_of(&west));
    let (ya, yb) = (
        a["camera"]["yaw"].as_f64().unwrap(),
        b["camera"]["yaw"].as_f64().unwrap(),
    );
    assert!(
        (ya - yb).abs() > 1.0,
        "different bearings must aim differently ({ya} vs {yb})"
    );
    assert_ne!(a["camera"]["pos"], b["camera"]["pos"]);
}

#[test]
fn shot_reuses_a_render_plan_camera() {
    // The bridge between the render-plan (Chunky) yaw convention and Minecraft's:
    // `--shot` reads pos/look_at and re-derives the aim, so the frame points where
    // the planned shot points.
    let dir = tmp("snap-shot");
    let png = dir.join("frame.png");
    let out = snap(&png, &["--shot", "spawn"]);
    assert_eq!(code(&out), 0, "{}", String::from_utf8_lossy(&out.stderr));
    let m = manifest_of(&png);
    assert!(m["camera"]["pos"].is_array());
    assert!(!m["targets"].as_array().unwrap().is_empty());
}

#[test]
fn snapshot_needs_only_placement_not_emission() {
    // spec-0015: the loop must work on a partial build. A campaign whose quest
    // graph fails analysis (exit 2 for `analyze`/`build`) must still render —
    // otherwise the tool is unavailable exactly when authoring is unfinished.
    let dir = tmp("snap-partial");
    let campaign = dir.join("campaign");
    let patch: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(common::compiler_fixtures_dir().join("unreachable-finale.json"))
            .unwrap(),
    )
    .unwrap();
    common::materialize(&patch, &campaign);
    let pf = common::prefabs_dir();

    let analyzed = delvec(&[
        "analyze",
        campaign.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    assert_eq!(code(&analyzed), 2, "the fixture must fail analysis");

    let png = dir.join("frame.png");
    let out = delvec(&[
        "snapshot",
        campaign.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "-o",
        png.to_str().unwrap(),
        "--width",
        "96",
        "--height",
        "54",
    ]);
    assert_eq!(
        code(&out),
        0,
        "snapshot must render an unanalyzable campaign: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(png.is_file() && png.with_extension("manifest.json").is_file());
}

#[test]
fn snapshot_writes_no_datapack() {
    // The command is view-only: nothing but the two artifacts appears.
    let dir = tmp("snap-noemit");
    let png = dir.join("frame.png");
    assert_eq!(code(&snap(&png, &[])), 0);
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, vec!["frame.manifest.json", "frame.png"]);
}

#[test]
fn json_mode_reports_the_written_paths() {
    let dir = tmp("snap-json");
    let png = dir.join("frame.png");
    let out = snap(&png, &["--json"]);
    assert_eq!(code(&out), 0);
    let line = String::from_utf8_lossy(&out.stdout);
    let last = line.lines().last().expect("a summary line");
    let v: serde_json::Value = serde_json::from_str(last).expect("json summary");
    assert!(v["png"].as_str().unwrap().ends_with("frame.png"));
    assert!(
        v["manifest"]
            .as_str()
            .unwrap()
            .ends_with("frame.manifest.json")
    );
    assert!(v["in_frame"].is_number() && v["out_of_frame"].is_number());
}
