//! CLI-level tests for the CPU render arms `delvec` carries (ADR-0021 §1):
//! the `scene` and `panorama` subcommands' input/output error paths, the world
//! save they name, and their Chunky-cache invalidation. Neither needs a GPU
//! adapter nor the (never-committed, EULA-gated) 1.21.11 client jar.
//!
//! The GPU arms' own CLI paths — texture resolution failing before any
//! rendering is attempted, and `--view` spec parsing — stayed with the binary
//! that has them, in `crates/delvec/tests/render_cli.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn tmp(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("delvec-view-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn render_plan_mini() -> &'static [u8] {
    include_bytes!("fixtures/view/render-plan-mini.json")
}

/// The shape of a world save the arms accept: `level.dat` and one region file.
/// These arms judge that the save is there, never what is in it — Chunky reads
/// the bytes — so the files' contents are immaterial here.
fn stub_world(dir: &Path) {
    std::fs::create_dir_all(dir.join("region")).unwrap();
    std::fs::write(dir.join("level.dat"), b"stub").unwrap();
    std::fs::write(dir.join("region").join("r.0.0.mca"), b"stub").unwrap();
}

fn scene_json(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn scene_missing_render_plan_is_dw0721_exit2() {
    let build_dir = tmp("scene-missing-plan");
    let out = build_dir.join("out");

    let result = Command::new(BIN)
        .args(["scene"])
        .arg(&build_dir)
        .args(["-o"])
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2), "{result:?}");
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("DW0721"), "expected DW0721: {stderr}");
}

#[test]
fn scene_output_blocked_is_dw0722_exit3() {
    let build_dir = tmp("scene-blocked-out");
    std::fs::write(build_dir.join("render-plan.json"), render_plan_mini()).unwrap();
    stub_world(&build_dir.join("world"));
    // Block the output path with a regular file so `create_dir_all(out)` fails.
    let out = build_dir.join("out-is-a-file");
    std::fs::write(&out, b"blocker").unwrap();

    let result = Command::new(BIN)
        .args(["scene"])
        .arg(&build_dir)
        .args(["-o"])
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(3), "{result:?}");
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("DW0722"), "expected DW0722: {stderr}");
}

/// A scene over a world that does not exist renders as an empty frame at exit 0
/// in Chunky, so both arms refuse it before a scene is written — by default
/// (`<build-dir>/world`) and when `--world` names a directory that is not a save.
#[test]
fn a_scene_over_no_world_save_is_refused_and_nothing_is_written() {
    let mut judged = 0;
    for arm in ["scene", "panorama"] {
        for shape in ["absent", "no-region", "no-level-dat", "given-elsewhere"] {
            let build_dir = tmp(&format!("{arm}-no-world-{shape}"));
            std::fs::write(build_dir.join("render-plan.json"), render_plan_mini()).unwrap();
            let world = build_dir.join("world");
            match shape {
                "no-region" => {
                    std::fs::create_dir_all(&world).unwrap();
                    std::fs::write(world.join("level.dat"), b"stub").unwrap();
                }
                "no-level-dat" => {
                    std::fs::create_dir_all(world.join("region")).unwrap();
                    std::fs::write(world.join("region").join("r.0.0.mca"), b"stub").unwrap();
                }
                // A good save at the default, and `--world` naming somewhere else.
                "given-elsewhere" => stub_world(&world),
                _ => {}
            }
            let out = build_dir.join("scenes");
            let mut cmd = Command::new(BIN);
            cmd.arg(arm).arg(&build_dir).arg("-o").arg(&out);
            if shape == "given-elsewhere" {
                cmd.arg("--world").arg(build_dir.join("not-a-save"));
            }
            let result = cmd.output().unwrap();
            let stderr = String::from_utf8_lossy(&result.stderr);
            assert_eq!(result.status.code(), Some(2), "{arm} {shape}: {stderr}");
            assert!(stderr.contains("DW0721"), "{arm} {shape}: {stderr}");
            assert!(
                stderr.contains("world-save.sh"),
                "{arm} {shape}: no remedy: {stderr}"
            );
            assert!(!out.exists(), "{arm} {shape}: a scene dir was written");
            judged += 1;
        }
    }
    assert_eq!(judged, 8);
}

/// Chunky resolves a scene's world against the rendering process's working
/// directory, so the path written is absolute: `<build-dir>/world` by default,
/// the `--world` directory when one is given.
#[test]
fn the_world_a_scene_names_is_absolute() {
    let mut judged = 0;
    for arm in ["scene", "panorama"] {
        let build_dir = tmp(&format!("{arm}-world-abs"));
        std::fs::write(build_dir.join("render-plan.json"), render_plan_mini()).unwrap();
        stub_world(&build_dir.join("world"));
        let elsewhere = build_dir.join("saves").join("kept");
        stub_world(&elsewhere);
        for given in [None, Some(&elsewhere)] {
            let out = build_dir.join(if given.is_some() { "given" } else { "default" });
            let mut cmd = Command::new(BIN);
            // Run from the build dir with a RELATIVE build path, so an unresolved
            // relative world would show up as a relative string.
            cmd.current_dir(&build_dir)
                .arg(arm)
                .arg(".")
                .arg("-o")
                .arg(&out);
            if given.is_some() {
                cmd.arg("--world").arg("saves/kept");
            }
            let result = cmd.output().unwrap();
            assert_eq!(result.status.code(), Some(0), "{arm}: {result:?}");
            let files: Vec<PathBuf> = std::fs::read_dir(&out)
                .unwrap()
                .map(|e| e.unwrap().path())
                .filter(|p| p.extension().is_some_and(|x| x == "json"))
                .collect();
            assert!(!files.is_empty(), "{arm}: no scene emitted");
            let want =
                std::path::absolute(given.cloned().unwrap_or(build_dir.join("world"))).unwrap();
            for f in &files {
                let v = scene_json(f);
                let path = v["world"]["path"].as_str().unwrap();
                assert!(
                    Path::new(path).is_absolute(),
                    "{arm}: relative world {path}"
                );
                assert_eq!(
                    std::fs::canonicalize(path).unwrap(),
                    std::fs::canonicalize(&want).unwrap(),
                    "{arm}: {}",
                    f.display()
                );
                judged += 1;
            }
        }
    }
    assert!(judged >= 4, "judged {judged} scene file(s)");
}

/// Chunky caches a scene's loaded chunks in `<scene>.octree2` / `<scene>.dump`
/// siblings. Re-emitting the scene (new chunkList, camera, sun or water
/// settings) and re-rendering silently reuses the STALE cache — a whole
/// debugging session was paid for this. Emission must delete the caches it is
/// invalidating, so the pitfall cannot recur.
#[test]
fn scene_emission_purges_stale_chunky_caches() {
    let build_dir = tmp("scene-purge");
    std::fs::write(build_dir.join("render-plan.json"), render_plan_mini()).unwrap();
    stub_world(&build_dir.join("world"));
    let out = build_dir.join("scenes");
    std::fs::create_dir_all(&out).unwrap();
    // A previous render's caches for the scenes this emission replaces.
    // Chunky keys its caches on the scene's `name`, which is exactly the file
    // stem `delvec render` emits (campaign-qualified).
    let stale = [
        "mini_spawn.octree2",
        "mini_spawn.dump",
        "mini_spawn.dump.backup",
        "mini_spawn.emittergrid",
        "mini_interior_entry_0.octree2",
    ];
    for f in stale {
        std::fs::write(out.join(f), b"stale").unwrap();
    }
    // An unrelated scene's cache must survive.
    std::fs::write(out.join("someone-elses.octree2"), b"keep").unwrap();

    let result = Command::new(BIN)
        .args(["scene"])
        .arg(&build_dir)
        .args(["-o"])
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    for f in stale {
        assert!(
            !out.join(f).exists(),
            "stale cache {f} survived scene emission"
        );
    }
    assert!(
        out.join("someone-elses.octree2").exists(),
        "an unrelated scene's cache must not be touched"
    );
}

/// The storybook exterior: an oblique camera framing the placed areas, emitted
/// first-class instead of hand-edited into a scene JSON, at the default frame.
#[test]
fn panorama_emits_a_framed_scene_of_the_built_place() {
    let build_dir = tmp("panorama-ok");
    std::fs::write(build_dir.join("render-plan.json"), render_plan_mini()).unwrap();
    stub_world(&build_dir.join("world"));
    let out = build_dir.join("scenes");

    let result = Command::new(BIN)
        .args(["panorama"])
        .arg(&build_dir)
        .args(["-o"])
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    let scene = out.join("mini_panorama_se_45.json");
    assert!(scene.exists(), "panorama scene not emitted");
    let v = scene_json(&scene);
    // Default sample target and frame for a final panorama.
    assert_eq!(v["sppTarget"], serde_json::json!(300));
    assert_eq!(v["width"], serde_json::json!(1600));
    assert_eq!(v["height"], serde_json::json!(900));
    // 45° oblique from the south-east: the camera sits +X/+Z of the layout and
    // above it, looking north-west and down.
    let pos = v["camera"]["position"].clone();
    assert!(
        pos["x"].as_f64().unwrap() > 17.0,
        "camera east of the layout"
    );
    assert!(
        pos["z"].as_f64().unwrap() > 10.0,
        "camera south of the layout"
    );
    assert!(pos["y"].as_f64().unwrap() > 69.0, "camera above the layout");
    // Layout-only chunk list (an ocean seam appears the moment pure-ocean
    // chunks are included).
    let chunks = v["chunkList"].as_array().unwrap();
    assert_eq!(chunks.len(), 2, "mini layout spans 2 chunks: {chunks:?}");
    // A sun is placed explicitly, not left to the Chunky default.
    assert!(v["sun"]["altitude"].is_number(), "no sun in {v}");
    // The measurement of how much of the frame the subject covers is printed.
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("covers") && stderr.contains("% of the frame"),
        "{stderr}"
    );
}

/// The creator's choices — side, pitch and frame — reach the scene and its name,
/// and a pitch outside the range is refused at the flag.
#[test]
fn panorama_takes_its_bearing_pitch_and_frame_from_the_flags() {
    let build_dir = tmp("panorama-flags");
    std::fs::write(build_dir.join("render-plan.json"), render_plan_mini()).unwrap();
    stub_world(&build_dir.join("world"));
    let out = build_dir.join("scenes");

    let result = Command::new(BIN)
        .args(["panorama"])
        .arg(&build_dir)
        .args(["-o"])
        .arg(&out)
        .args([
            "--bearing",
            "n",
            "--pitch",
            "30",
            "--width",
            "1024",
            "--height",
            "768",
        ])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    let v = scene_json(&out.join("mini_panorama_n_30.json"));
    assert_eq!(v["width"], serde_json::json!(1024));
    assert_eq!(v["height"], serde_json::json!(768));
    // Looking south: the camera is north of the layout (−Z).
    assert!(v["camera"]["position"]["z"].as_f64().unwrap() < 0.0, "{v}");

    for bad in [
        ["--pitch", "90"],
        ["--pitch", "4"],
        ["--width", "0"],
        ["--bearing", "up"],
    ] {
        let result = Command::new(BIN)
            .args(["panorama"])
            .arg(&build_dir)
            .args(["-o"])
            .arg(&out)
            .args(bad)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2), "{bad:?}: {result:?}");
    }
}

/// The panorama gets the same stale-cache purge as `scene`.
#[test]
fn panorama_purges_stale_chunky_caches() {
    let build_dir = tmp("panorama-purge");
    std::fs::write(build_dir.join("render-plan.json"), render_plan_mini()).unwrap();
    stub_world(&build_dir.join("world"));
    let out = build_dir.join("scenes");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("mini_panorama_se_45.octree2"), b"stale").unwrap();
    std::fs::write(out.join("mini_panorama_se_45.dump"), b"stale").unwrap();

    let result = Command::new(BIN)
        .args(["panorama"])
        .arg(&build_dir)
        .args(["-o"])
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    assert!(!out.join("mini_panorama_se_45.octree2").exists());
    assert!(!out.join("mini_panorama_se_45.dump").exists());
}

/// A campaign directory holding only what `delvec cameras` reads: `design.json`
/// rows and a `design/cameras.json` record.
fn camera_campaign(dir: &Path, cameras: serde_json::Value) {
    std::fs::create_dir_all(dir.join("design")).unwrap();
    std::fs::write(
        dir.join("design.json"),
        br#"{"campaign_id":"mini","content":{"references":[
            {"name":"concept/gate","shows":"the gate","time":"dusk","weather":"clear"},
            {"name":"concept/hall","shows":"the hall","time":"dusk","weather":"clear"}]},
            "stage":"design"}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("design/cameras.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"campaign_id": "mini", "cameras": cameras}))
            .unwrap(),
    )
    .unwrap();
}

fn a_camera(name: &str, answers: &str) -> serde_json::Value {
    serde_json::json!({
        "answers": answers, "exposure": 2.0, "fov": 55.0, "height": 450, "name": name,
        "pitch": 12.0, "pos": [9.5, 72.0, -6.0], "source": "estimated", "spp": 64, "width": 800, "yaw": 20.0
    })
}

/// `delvec cameras` emits one scene per stated camera, verbatim, names the
/// approved images no camera answers, and with `--bracket` writes every
/// candidate back out in the record format.
#[test]
fn cameras_emits_the_stated_cameras_and_their_candidates() {
    let build_dir = tmp("cameras-ok");
    std::fs::write(build_dir.join("render-plan.json"), render_plan_mini()).unwrap();
    stub_world(&build_dir.join("world"));
    let campaign = build_dir.join("campaign");
    camera_campaign(
        &campaign,
        serde_json::json!([a_camera("gate", "concept/gate")]),
    );
    let out = build_dir.join("scenes");

    let result = Command::new(BIN)
        .args(["cameras"])
        .arg(&build_dir)
        .arg("--campaign")
        .arg(&campaign)
        .arg("-o")
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    let v = scene_json(&out.join("mini_camera_gate.json"));
    assert_eq!(v["name"], "mini_camera_gate");
    assert_eq!(v["width"], 800);
    assert_eq!(v["sppTarget"], 64);
    assert_eq!(v["exposure"], 2.0);
    assert_eq!(v["camera"]["fov"], 55.0);
    assert_eq!(v["camera"]["position"]["z"], -6.0);
    assert!(v["sun"]["altitude"].is_number(), "{v}");
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("1 of 2 approved image(s)") && stderr.contains("concept/hall"),
        "{stderr}"
    );

    let bracketed = build_dir.join("bracketed");
    let result = Command::new(BIN)
        .args(["cameras"])
        .arg(&build_dir)
        .arg("--campaign")
        .arg(&campaign)
        .arg("-o")
        .arg(&bracketed)
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
            "gate.truck-2"
        ]
    );
    for n in &names {
        let draft = scene_json(&bracketed.join(format!("mini_camera_{n}_draft.json")));
        assert_eq!(draft["width"], 200, "{n}");
    }
}

/// A camera answering no approved image, or a record that is not the record, is
/// refused before anything is written.
#[test]
fn cameras_refuses_a_camera_that_answers_nothing() {
    let build_dir = tmp("cameras-refused");
    std::fs::write(build_dir.join("render-plan.json"), render_plan_mini()).unwrap();
    stub_world(&build_dir.join("world"));
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
        let result = Command::new(BIN)
            .args(["cameras"])
            .arg(&build_dir)
            .arg("--campaign")
            .arg(&campaign)
            .arg("-o")
            .arg(&out)
            .output()
            .unwrap();
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

/// A panorama fitted to the building its anchors stand in is framed on their
/// span, not on the whole layout, and says so in its name.
#[test]
fn panorama_frames_the_building_its_anchors_name() {
    let build_dir = tmp("panorama-anchors");
    std::fs::write(build_dir.join("render-plan.json"), render_plan_mini()).unwrap();
    stub_world(&build_dir.join("world"));
    std::fs::create_dir_all(build_dir.join("creator-datapack")).unwrap();
    std::fs::write(
        build_dir.join("creator-datapack/layout.json"),
        br#"{"anchors":[{"id":"anchor/a","area":"area/entry","kind":"point","pos":[2,65,2]},
                        {"id":"anchor/b","area":"area/entry","kind":"point","pos":[6,65,5]}]}"#,
    )
    .unwrap();
    let out = build_dir.join("scenes");
    let run = |subject: &str| {
        Command::new(BIN)
            .args(["panorama"])
            .arg(&build_dir)
            .arg("-o")
            .arg(&out)
            .args(["--subject", subject])
            .output()
            .unwrap()
    };
    let result = run("anchor/a,anchor/b");
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("[2, 64, 2]..[6, 69, 5]"), "{stderr}");
    assert!(stderr.contains("camera record"), "{stderr}");
    assert!(out.join("mini_panorama_se_45_anchors.json").exists());
    let layout = run("layout");
    assert_eq!(layout.status.code(), Some(0), "{layout:?}");
    let narrow = scene_json(&out.join("mini_panorama_se_45_anchors.json"));
    let wide = scene_json(&out.join("mini_panorama_se_45.json"));
    // A smaller subject from the same side is a closer camera.
    assert!(
        narrow["camera"]["position"]["y"].as_f64().unwrap()
            < wide["camera"]["position"]["y"].as_f64().unwrap(),
        "{narrow} vs {wide}"
    );
    let refused = run("anchor/nowhere");
    assert_eq!(refused.status.code(), Some(2), "{refused:?}");
}
