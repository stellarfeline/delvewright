//! CLI-level tests for the `delvec render` GPU arms that need neither a GPU
//! adapter nor the (never-committed, EULA-gated) 1.21.11 client jar: texture
//! resolution failing before any rendering is attempted, and `--view` spec
//! parsing being refused before the GPU is touched. Behaviour that genuinely
//! needs an adapter (the fidelity gate's missing-texture detection, actual
//! rendering) lives in `tests/gpu.rs` (`#[ignore]`d by default).
//!
//! The CPU arms these used to sit beside are `delvec` subcommands now
//! (ADR-0021 §1); their CLI tests moved with them, to
//! `crates/delvec/tests/view_cli.rs`.

use std::path::PathBuf;
use std::process::Command;

/// `delvec render …`: the one binary, entered at the GPU-render surface.
fn render() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_delvec"));
    cmd.arg("render");
    cmd
}

fn tmp(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("delvec render-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A prefab from the content repo, or `None` on a checkout without the symlink.
fn prefab(name: &str) -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../campaigns/prefabs")
        .join(name);
    p.exists().then_some(p)
}

#[test]
fn piece_without_textures_is_dw0723_exit5() {
    // No --textures, and an empty HOME so `~/.chunky/resources/minecraft.jar`
    // cannot exist; DELVEWRIGHT_CLIENT_JAR is stripped too.
    //
    // **The input is a real, measured library piece**, and it has to be. This
    // test used to pass `nonexistent.nbt` on the reading that texture resolution
    // happens before the input is ever opened. That order moved: the arm now
    // judges the piece — measured light, and the roofed floor nothing can reach
    // (`DW0894`/`DW0895`) — before it goes looking for a client jar, for the
    // same reason the `--view` test below gives for parsing specs first. A
    // creator who typed the wrong filename, or handed over a piece nobody
    // measured, is owed that answer and not *I could not find your textures*.
    // So the piece is one that passes both, and what is left for this test to
    // assert is exactly what it always asserted: with no textures anywhere, the
    // arm exits 5 on `DW0723`.
    let Some(nbt) = prefab("keep-gate-room.nbt") else {
        eprintln!("skip: no content symlink");
        return;
    };
    let empty_home = tmp("piece-no-textures-home");
    let out = tmp("piece-no-textures-out");

    let result = render()
        .args(["piece"])
        .arg(&nbt)
        .args(["--out"])
        .arg(&out)
        .env("HOME", &empty_home)
        .env_remove("DELVEWRIGHT_CLIENT_JAR")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert_eq!(result.status.code(), Some(5), "{stderr}");
    assert!(stderr.contains("DW0723"), "expected DW0723: {stderr}");
    // ...and it got past the piece, which is what makes the assertion above
    // about textures rather than about the piece.
    assert!(!stderr.contains("DW0894"), "{stderr}");
}

/// A malformed `--view` is a usage error, and it is worth nothing to discover it
/// after a GPU has been initialised and twenty-eight frames have been written —
/// so it is refused *before* texture resolution, which is why this test can
/// assert `DW0721`/exit 2 on a machine with no client jar at all.
#[test]
fn a_malformed_view_is_dw0721_exit2_before_the_gpu_is_touched() {
    let empty_home = tmp("view-bad-spec-home");
    let out = tmp("view-bad-spec-out");

    for (spec, needle) in [
        ("fov=60", "no bearing"),
        ("face=northeast", "not a face"),
        ("face=north,yaw=10", "one bearing"),
        ("face=north,tilt=3", "not a view key"),
        ("face=north,name=West Front", "not usable as a file stem"),
    ] {
        let result = render()
            .args(["piece", "nonexistent.nbt", "--out"])
            .arg(&out)
            .args(["--view", spec])
            .env("HOME", &empty_home)
            .env_remove("DELVEWRIGHT_CLIENT_JAR")
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2), "spec `{spec}`: {result:?}");
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(stderr.contains("DW0721"), "spec `{spec}`: {stderr}");
        assert!(stderr.contains(needle), "spec `{spec}`: {stderr}");
        // Nothing was rendered, so nothing about textures was ever reported.
        assert!(!stderr.contains("DW0723"), "spec `{spec}`: {stderr}");
    }
}

/// `--view` is on `batch` as well as `piece`: aiming a camera belongs to
/// photographing a piece, and `batch` photographs pieces.
#[test]
fn batch_accepts_the_same_view_specs() {
    let empty_home = tmp("view-batch-home");
    let out = tmp("view-batch-out");
    let result = render()
        .args(["batch", "nonexistent-dir", "--out"])
        .arg(&out)
        .args(["--view", "face=northeast"])
        .env("HOME", &empty_home)
        .env_remove("DELVEWRIGHT_CLIENT_JAR")
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2), "{result:?}");
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("DW0721"), "{stderr}");
    assert!(stderr.contains("not a face"), "{stderr}");
}
