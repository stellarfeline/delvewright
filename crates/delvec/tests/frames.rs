//! `delvec frames` — the review frames of a build, and with `--campaign` the
//! authoring loop's viewport (spec-0015, spec-0112 §9), tested at the process
//! level: the manifest contract, the binding lines, the refusals, the
//! partial-build guarantee and the ADR-0006 byte-identity of every artifact a
//! frame writes, across runs and across thread counts on the CPU backend.
//!
//! Every frame here is drawn by Lumenwright with the pinned client jar's
//! textures, found the way a creator's run finds them
//! (`$DELVEWRIGHT_CLIENT_JAR`, or Init I5's location).

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

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// One frame of the keep-vertical fixture's load world (a 3D multi-piece
/// layout, so the frame sees real geometry at several elevations), on the CPU
/// backend so the bytes are a function of the inputs alone.
fn snap(out_png: &Path, extra: &[&str]) -> Output {
    let campaign = common::keep_vertical_dir();
    let pf = common::prefabs_dir();
    let mut args: Vec<String> = vec![
        "frames".into(),
        "--campaign".into(),
        campaign.to_string_lossy().into_owned(),
        "--prefabs".into(),
        pf.to_string_lossy().into_owned(),
        "-o".into(),
        out_png.to_string_lossy().into_owned(),
        // Small frames keep the suite fast; every code path is size-independent.
        "--size".into(),
        "96".into(),
        "--backend".into(),
        "cpu".into(),
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
fn a_campaign_frame_writes_a_png_its_manifest_and_its_world() {
    let dir = tmp("frames-default");
    let png = dir.join("frame.png");
    let out = snap(&png, &[]);
    assert_eq!(code(&out), 0, "stderr: {}", stderr(&out));
    let bytes = std::fs::read(&png).expect("png written");
    assert_eq!(
        &bytes[..8],
        &[137, 80, 78, 71, 13, 10, 26, 10],
        "PNG signature"
    );
    let m = manifest_of(&png);
    assert_eq!(m["manifest_version"], 3);
    assert_eq!(m["image"]["width"], 96);
    assert_eq!(m["image"]["height"], 96);
    assert_eq!(m["image"]["path"], "frame.png");
    assert_eq!(m["mode"], "quick");
    assert_eq!(m["world"]["key"], "at-load");
    assert!(
        m["renderer"]
            .as_str()
            .is_some_and(|r| r.starts_with("lumenwright ")),
        "the manifest names the renderer: {m}"
    );
    assert!(
        m["camera"]["convention"]
            .as_str()
            .expect("convention documented in-band")
            .contains("yaw 0 = south"),
        "the manifest must carry its own camera convention"
    );
    // The world the frame was drawn from is written beside it.
    assert!(dir.join("world/level.dat").is_file());
    let e = stderr(&out);
    let first = e
        .lines()
        .find(|l| l.starts_with("renderer: "))
        .expect("a renderer line");
    assert!(first.contains("backend"), "{first}");
    assert!(
        e.contains("world: at-load "),
        "the world's binding line: {e}"
    );
}

/// The manifest's **layout** half: every placed piece, with the exact inputs a
/// `piece-local` edit frame resolves against — the per-area index, the prefab
/// guard, and the placement transform (`origin` + `rotation`) plus the
/// resulting AABB.
#[test]
fn manifest_lists_every_placed_piece_with_its_frame_inputs() {
    let dir = tmp("frames-pieces");
    let png = dir.join("frame.png");
    assert_eq!(code(&snap(&png, &[])), 0);
    let m = manifest_of(&png);
    let pieces = m["pieces"].as_array().expect("pieces array");
    assert!(
        pieces.len() > 1,
        "keep-vertical is a multi-piece layout: {pieces:?}"
    );
    let mut next: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for p in pieces {
        let area = p["area"].as_str().expect("area id").to_string();
        let index = p["index"].as_u64().expect("index") as usize;
        let want = next.entry(area.clone()).or_insert(0);
        assert_eq!(index, *want, "area `{area}` piece indices are contiguous");
        *want += 1;
        assert!(
            p["prefab"]
                .as_str()
                .is_some_and(|s| s.starts_with("prefab/")),
            "the prefab guard value: {p}"
        );
        let arr = |v: &serde_json::Value| -> [i64; 3] {
            let a = v.as_array().expect("cell");
            [
                a[0].as_i64().unwrap(),
                a[1].as_i64().unwrap(),
                a[2].as_i64().unwrap(),
            ]
        };
        let origin = arr(&p["origin"]);
        let size = arr(&p["size"]);
        let min = arr(&p["box"]["min"]);
        let max = arr(&p["box"]["max"]);
        for a in 0..3 {
            assert!(min[a] <= max[a], "inclusive AABB: {p}");
            assert!(
                min[a] <= origin[a] && origin[a] <= max[a],
                "the /place template origin is inside the placed box: {p}"
            );
        }
        assert_eq!(max[1] - min[1] + 1, size[1], "unrotated y extent: {p}");
        let (ex, ez) = (max[0] - min[0] + 1, max[2] - min[2] + 1);
        assert!(
            (ex, ez) == (size[0], size[2]) || (ex, ez) == (size[2], size[0]),
            "the placed extent is the prefab size, quarter-turned at most: {p}"
        );
    }
}

#[test]
fn manifest_targets_carry_position_screen_box_and_occlusion() {
    let dir = tmp("frames-targets");
    let png = dir.join("frame.png");
    assert_eq!(code(&snap(&png, &[])), 0);
    let m = manifest_of(&png);
    let targets = m["targets"].as_array().expect("targets array");
    assert!(!targets.is_empty(), "the overview must frame something");
    for t in targets {
        assert!(t["id"].as_str().is_some_and(|s| !s.is_empty()), "{t}");
        assert!(
            t["pos"].is_array() != t["box"].is_object(),
            "a target is a point or a box, never both: {t}"
        );
        let b = &t["screen_bbox"];
        assert!(
            b["w"].as_i64().unwrap_or(0) >= 1 && b["h"].as_i64().unwrap_or(0) >= 1,
            "{t}"
        );
        assert!(t["occluded"].is_boolean(), "{t}");
        assert!(t["distance"].is_number(), "{t}");
    }
    let out_of_frame = m["out_of_frame"].as_array().expect("out_of_frame array");
    for t in out_of_frame {
        assert!(
            t["screen_bbox"].is_null(),
            "an absent subject has no box: {t}"
        );
    }
    assert!(
        targets
            .iter()
            .chain(out_of_frame.iter())
            .any(|t| t["kind"] == "anchor"),
        "anchors must appear in the manifest"
    );
}

#[test]
fn a_campaign_frame_is_byte_identical_across_runs() {
    // ADR-0006 for the visual tier: the same DSL and the same camera produce
    // the same PNG bytes AND the same manifest bytes. Same file name in two
    // directories: the manifest records the image's basename.
    let a = tmp("frames-determinism-a").join("frame.png");
    let b = tmp("frames-determinism-b").join("frame.png");
    for p in [&a, &b] {
        assert_eq!(code(&snap(p, &["--labels"])), 0);
    }
    assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
    assert_eq!(
        std::fs::read(a.with_extension("manifest.json")).unwrap(),
        std::fs::read(b.with_extension("manifest.json")).unwrap(),
    );
}

#[test]
fn labels_change_the_frame_but_not_the_manifest() {
    let dir = tmp("frames-labels");
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
    for m in [&mut a, &mut b] {
        m["image"]["path"] = serde_json::Value::Null;
        // The renderer's own timing is no fact of the scene.
        m["renderer_stats"] = serde_json::Value::Null;
        m["frame"]["featureless"] = serde_json::Value::Null;
    }
    assert_eq!(a, b, "--labels must not change the manifest");
}

#[test]
fn explicit_camera_flag_places_the_eye_exactly() {
    let dir = tmp("frames-camera");
    let png = dir.join("frame.png");
    let out = snap(&png, &["--camera", "12.5,80,-4.25,135,20,50"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let m = manifest_of(&png);
    assert_eq!(m["camera"]["pos"][0], 12.5);
    assert_eq!(m["camera"]["pos"][1], 80.0);
    assert_eq!(m["camera"]["pos"][2], -4.25);
    assert_eq!(m["camera"]["yaw"], 135.0);
    assert_eq!(m["camera"]["pitch"], 20.0);
    assert_eq!(m["camera"]["fov"], 50.0);
    let png2 = dir.join("nofov.png");
    assert_eq!(code(&snap(&png2, &["--camera", "0,80,0,0,10"])), 0);
    assert_eq!(manifest_of(&png2)["camera"]["fov"], 70.0);
}

#[test]
fn malformed_camera_and_unknown_anchor_fail_with_a_useful_message() {
    let dir = tmp("frames-errors");
    let png = dir.join("frame.png");
    let out = snap(&png, &["--camera", "1,2,3"]);
    assert_eq!(code(&out), 1);
    assert!(stderr(&out).contains("x,y,z,yaw,pitch"), "names the shape");
    let out = snap(&png, &["--camera", "1,2,3,4,five"]);
    assert_eq!(code(&out), 1);
    assert!(stderr(&out).contains("not a number"), "names the bad field");
    let out = snap(&png, &["--at", "anchor/does-not-exist"]);
    assert_eq!(code(&out), 1);
    assert!(
        stderr(&out).contains("Known anchors"),
        "an unknown anchor must list the real ones"
    );
}

#[test]
fn at_anchor_frames_that_anchor_unoccluded() {
    let dir = tmp("frames-at");
    let png = dir.join("frame.png");
    let out = snap(&png, &["--at", "spawn"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
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
    let dir = tmp("frames-orbit");
    let south = dir.join("s.png");
    let west = dir.join("w.png");
    for (p, b) in [(&south, "0"), (&west, "90")] {
        assert_eq!(
            code(&snap(p, &["--at", "spawn", "--orbit", b, "--dist", "10"])),
            0
        );
    }
    let (a, b) = (manifest_of(&south), manifest_of(&west));
    let (ya, yb) = (
        a["camera"]["yaw"].as_f64().unwrap(),
        b["camera"]["yaw"].as_f64().unwrap(),
    );
    assert!((ya - yb).abs() > 1.0, "{ya} vs {yb}");
    assert_ne!(a["camera"]["pos"], b["camera"]["pos"]);
}

#[test]
fn a_campaign_frame_needs_only_placement_not_emission() {
    // spec-0015: the loop works on a partial build. A campaign whose quest
    // graph fails analysis still draws.
    let dir = tmp("frames-partial");
    let campaign = dir.join("campaign");
    let patch: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(common::compiler_fixtures_dir().join("unreachable-finale.json"))
            .unwrap(),
    )
    .unwrap();
    common::materialize(&patch, &campaign);
    common::declare_story_dir(&campaign);
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
        "frames",
        "--campaign",
        campaign.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "-o",
        png.to_str().unwrap(),
        "--size",
        "48",
        "--backend",
        "cpu",
    ]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert!(png.is_file() && png.with_extension("manifest.json").is_file());
}

#[test]
fn a_campaign_frame_writes_no_datapack() {
    let dir = tmp("frames-noemit");
    let png = dir.join("frame.png");
    assert_eq!(code(&snap(&png, &[])), 0);
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, vec!["frame.manifest.json", "frame.png", "world"]);
}

#[test]
fn json_mode_reports_the_written_paths() {
    let dir = tmp("frames-json");
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
    assert!(v["in_frame"].is_number() && v["undrawn"].is_number());
}

// --------------------------------------------------------- a build's frames

/// Build hello-world under `dir/build`.
fn build_hello(dir: &Path) -> PathBuf {
    let out = dir.join("build");
    let o = delvec(&[
        "build",
        common::hello_world_dir().to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        common::prefabs_dir().to_str().unwrap(),
    ]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    out
}

fn frames_of(build: &Path, out: &Path, extra: &[&str]) -> Output {
    let mut args = vec![
        "frames",
        build.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--size",
        "48",
        "--backend",
        "cpu",
    ];
    args.extend_from_slice(extra);
    delvec(&args)
}

/// spec-0112 criterion 5: `delvec frames` draws one PNG per shot of the plan
/// with `shot-index.json` beside them, names the renderer and the backend on
/// its first line, and prints the set's binding lines.
#[test]
fn frames_draws_every_shot_and_says_what_it_did() {
    let dir = tmp("frames-build");
    let build = build_hello(&dir);
    let out = dir.join("frames");
    let o = frames_of(&build, &out, &[]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let e = stderr(&o);
    let first = e.lines().next().unwrap_or("");
    assert!(
        first.starts_with("renderer: lumenwright ") && first.contains(", backend cpu"),
        "the first line names the renderer and backend: {first}"
    );
    assert!(e.contains("frames: "), "{e}");
    assert!(e.contains("shows: "), "{e}");
    assert!(e.contains("not drawn by the renderer (DW1039)"), "{e}");
    let plan: serde_json::Value =
        serde_json::from_slice(&std::fs::read(build.join("render-plan.json")).unwrap()).unwrap();
    let n = plan["shots"].as_array().unwrap().len();
    assert!(n > 0);
    let index: serde_json::Value =
        serde_json::from_slice(&std::fs::read(out.join("shot-index.json")).unwrap()).unwrap();
    assert_eq!(index["count"], n);
    for s in index["shots"].as_array().unwrap() {
        assert!(out.join(s["image"].as_str().unwrap()).is_file(), "{s}");
        for k in [
            "world",
            "sky",
            "mode",
            "backend",
            "renderer",
            "shows",
            "undrawn",
            "out_of_frame",
            "unchecked",
        ] {
            assert!(!s[k].is_null(), "entry carries `{k}`: {s}");
        }
    }
}

/// A shot whose world the build directory does not hold is refused by name,
/// with the rebuild as its remedy.
#[test]
fn a_shot_whose_world_is_missing_is_refused_by_name() {
    let dir = tmp("frames-missing-world");
    let build = build_hello(&dir);
    std::fs::remove_dir_all(build.join("validation/worlds/at-load")).unwrap();
    let o = frames_of(&build, &dir.join("frames"), &[]);
    assert_ne!(code(&o), 0);
    let e = stderr(&o);
    assert!(e.contains("at-load"), "names the key: {e}");
    assert!(e.contains("delvec build"), "names the remedy: {e}");
}

/// `DW0723` names Init I5 when no jar resolves.
#[test]
fn no_jar_is_dw0723_naming_init_i5() {
    let dir = tmp("frames-no-jar");
    let build = build_hello(&dir);
    let o = Command::new(BIN)
        .args([
            "frames",
            build.to_str().unwrap(),
            "-o",
            dir.join("frames").to_str().unwrap(),
            "--backend",
            "cpu",
        ])
        .env_remove("DELVEWRIGHT_CLIENT_JAR")
        .env("HOME", &dir)
        .output()
        .unwrap();
    assert_ne!(code(&o), 0);
    let e = stderr(&o);
    assert!(e.contains("DW0723"), "{e}");
    assert!(e.contains("I5"), "names Init I5: {e}");
}

/// spec-0112 criterion 9: two runs on one build with the CPU backend at one
/// thread and at ten write byte-identical PNGs, indices and manifests, and
/// neither the index nor a manifest carries a pixel hash.
#[test]
fn the_cpu_backend_draws_the_same_bytes_at_every_thread_count() {
    let dir = tmp("frames-threads");
    let build = build_hello(&dir);
    let (a, b) = (dir.join("t1"), dir.join("t10"));
    assert_eq!(code(&frames_of(&build, &a, &["--threads", "1"])), 0);
    assert_eq!(code(&frames_of(&build, &b, &["--threads", "10"])), 0);
    let mut names: Vec<String> = std::fs::read_dir(&a)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert!(names.iter().any(|n| n.ends_with(".png")));
    for n in &names {
        let (x, y) = (
            std::fs::read(a.join(n)).unwrap(),
            std::fs::read(b.join(n)).unwrap(),
        );
        if n.ends_with(".manifest.json") {
            // The renderer's own wall time is no fact of the frame.
            let strip = |bytes: &[u8]| {
                let mut v: serde_json::Value = serde_json::from_slice(bytes).unwrap();
                v["renderer_stats"]["seconds"] = serde_json::Value::Null;
                v
            };
            assert_eq!(strip(&x), strip(&y), "{n}");
            let text = String::from_utf8_lossy(&x);
            assert!(!text.contains("sha256"), "no pixel hash in {n}");
        } else {
            assert_eq!(x, y, "{n} differs between --threads 1 and --threads 10");
        }
    }
    let index = std::fs::read_to_string(a.join("shot-index.json")).unwrap();
    assert!(!index.contains("sha256"), "no pixel hash in the index");
}

/// The engine's dependency graph holds no network client: a frame is drawn
/// from the jar on disk and the worlds the build wrote, and nothing in the
/// binary can fetch.
#[test]
fn the_renderer_holds_no_network_client() {
    let lock =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock"))
            .unwrap();
    for client in [
        "reqwest",
        "ureq",
        "hyper",
        "curl",
        "isahc",
        "attohttpc",
        "surf",
        "tokio",
    ] {
        assert!(
            !lock.contains(&format!("name = \"{client}\"")),
            "`{client}` is in the lock: the renderer must not be able to fetch"
        );
    }
}

/// `delvec cameras --preview` draws a stated camera as `delvec frames
/// --campaign --camera` draws the same numbers: the preview of a record
/// camera is byte for byte the campaign frame of the same camera at half the
/// stated frame, in the same world.
#[test]
fn a_camera_preview_is_the_campaign_frame_of_the_same_camera() {
    let dir = tmp("camera-preview");
    let campaign = dir.join("keep-vertical");
    std::fs::create_dir_all(&campaign).unwrap();
    let src = common::keep_vertical_dir();
    for entry in std::fs::read_dir(&src).unwrap() {
        let p = entry.unwrap().path();
        if p.is_file() {
            std::fs::copy(&p, campaign.join(p.file_name().unwrap())).unwrap();
        }
    }
    std::fs::create_dir_all(campaign.join("design/concept")).unwrap();
    std::fs::write(
        campaign.join("design/concept/keep.png"),
        b"an approved picture",
    )
    .unwrap();
    std::fs::write(
        campaign.join("design.json"),
        format!(
            r#"{{"campaign_id":"keep-vertical","content":{{"references":[
            {{"name":"concept/keep","shows":"the keep","time":"noon","weather":"clear"}}]}},
            "dsl_version":"{}","stage":"design"}}"#,
            delvewright_dsl::DSL_VERSION
        ),
    )
    .unwrap();
    let camera = serde_json::json!({
        "answers": "concept/keep", "fov": 60.0, "height": 96,
        "name": "keep", "pitch": 20.0, "pos": [-6.5, 74.0, -8.0], "source": "estimated", "spp": 16,
        "width": 96, "yaw": -35.0
    });
    std::fs::write(
        campaign.join("design/cameras.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"campaign_id": "keep-vertical", "cameras": [camera]}),
        )
        .unwrap(),
    )
    .unwrap();
    let build = dir.join("build");
    std::fs::create_dir_all(&build).unwrap();
    std::fs::write(
        build.join("render-plan.json"),
        br#"{"campaign_id":"keep-vertical","layout_aabb":{"min":[0,64,0],"max":[1,65,1]},
            "sky":{"time":"noon","daytime_ticks":6000,"weather":"clear"},"shots":[]}"#,
    )
    .unwrap();
    let pf = common::prefabs_dir();
    let previews = dir.join("previews");
    let preview_of = || {
        let out = Command::new(BIN)
            .arg("--prefabs")
            .arg(&pf)
            .arg("cameras")
            .arg(&build)
            .arg("--campaign")
            .arg(&campaign)
            .arg("-o")
            .arg(&previews)
            .arg("--preview")
            .arg("--backend")
            .arg("cpu")
            .output()
            .unwrap();
        assert_eq!(code(&out), 0, "{}", stderr(&out));
        std::fs::read(previews.join("keep-vertical_camera_keep_preview.png")).unwrap()
    };
    let preview = preview_of();
    let frame = dir.join("frame.png");
    let out = delvec(&[
        "frames",
        "--campaign",
        &campaign.to_string_lossy(),
        "--prefabs",
        &pf.to_string_lossy(),
        "-o",
        &frame.to_string_lossy(),
        "--camera=-6.5,74,-8,-35,20,60",
        "--size",
        "48",
        "--backend",
        "cpu",
    ]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert_eq!(
        preview,
        std::fs::read(&frame).unwrap(),
        "the preview of a record camera is not the campaign frame of the same numbers"
    );
    let moved = std::fs::read_to_string(campaign.join("design/cameras.json"))
        .unwrap()
        .replace("-35.0", "-25.0");
    std::fs::write(campaign.join("design/cameras.json"), moved).unwrap();
    assert_ne!(
        preview,
        preview_of(),
        "a changed yaw did not move the preview"
    );
}
