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
