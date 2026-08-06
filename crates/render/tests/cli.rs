//! CLI-level tests for `delve-render` that need neither a GPU adapter nor the
//! (never-committed, EULA-gated) 1.21.11 client jar: the `scene` subcommand's
//! input/output error paths, and texture resolution failing before any
//! rendering is attempted. GPU-dependent behavior (the fidelity gate's
//! missing-texture detection, actual rendering) lives in `tests/gpu.rs`
//! (`#[ignore]`d by default).

use std::path::PathBuf;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_delve-render");

fn tmp(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("delve-render-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn render_plan_mini() -> &'static [u8] {
    include_bytes!("fixtures/render-plan-mini.json")
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

/// Chunky caches a scene's loaded chunks in `<scene>.octree2` / `<scene>.dump`
/// siblings. Re-emitting the scene (new chunkList, camera, sun or water
/// settings) and re-rendering silently reuses the STALE cache — a whole
/// debugging session was paid for this. Emission must delete the caches it is
/// invalidating, so the pitfall cannot recur.
#[test]
fn scene_emission_purges_stale_chunky_caches() {
    let build_dir = tmp("scene-purge");
    std::fs::write(build_dir.join("render-plan.json"), render_plan_mini()).unwrap();
    let out = build_dir.join("scenes");
    std::fs::create_dir_all(&out).unwrap();
    // A previous render's caches for the scenes this emission replaces.
    // Chunky keys its caches on the scene's `name`, which is exactly the file
    // stem `delve-render` emits (campaign-qualified).
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

/// The whole-map release panorama: a 45° oblique camera framing the entire
/// layout, emitted first-class instead of hand-edited into a scene JSON.
#[test]
fn panorama_emits_a_framed_whole_map_scene() {
    let build_dir = tmp("panorama-ok");
    std::fs::write(build_dir.join("render-plan.json"), render_plan_mini()).unwrap();
    let out = build_dir.join("scenes");

    let result = Command::new(BIN)
        .args(["panorama"])
        .arg(&build_dir)
        .args(["-o"])
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    let scene = out.join("mini_panorama_se.json");
    assert!(scene.exists(), "panorama scene not emitted");
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&scene).unwrap()).unwrap();
    // Default sample target for a final panorama.
    assert_eq!(v["sppTarget"], serde_json::json!(300));
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
}

/// The panorama gets the same stale-cache purge as `scene`.
#[test]
fn panorama_purges_stale_chunky_caches() {
    let build_dir = tmp("panorama-purge");
    std::fs::write(build_dir.join("render-plan.json"), render_plan_mini()).unwrap();
    let out = build_dir.join("scenes");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("mini_panorama_se.octree2"), b"stale").unwrap();
    std::fs::write(out.join("mini_panorama_se.dump"), b"stale").unwrap();

    let result = Command::new(BIN)
        .args(["panorama"])
        .arg(&build_dir)
        .args(["-o"])
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    assert!(!out.join("mini_panorama_se.octree2").exists());
    assert!(!out.join("mini_panorama_se.dump").exists());
}

#[test]
fn piece_without_textures_is_dw0723_exit5() {
    // No --textures, and an empty HOME so `~/.chunky/resources/minecraft.jar`
    // cannot exist; DELVEWRIGHT_CLIENT_JAR is stripped too. Texture resolution
    // fails before the (nonexistent) input .nbt is ever read.
    let empty_home = tmp("piece-no-textures-home");
    let out = tmp("piece-no-textures-out");

    let result = Command::new(BIN)
        .args(["piece"])
        .arg("nonexistent.nbt")
        .args(["--out"])
        .arg(&out)
        .env("HOME", &empty_home)
        .env_remove("DELVEWRIGHT_CLIENT_JAR")
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(5), "{result:?}");
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("DW0723"), "expected DW0723: {stderr}");
}
