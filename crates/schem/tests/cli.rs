//! End-to-end tests for the `delve-schem` binary: it reads a real `.schem` file,
//! writes structure `.nbt` output, and names split parts + a manifest correctly.

use std::path::PathBuf;
use std::process::Command;

use delvewright_schem::convert::read_structure;
use delvewright_schem::fixtures;

const BIN: &str = env!("CARGO_BIN_EXE_delve-schem");

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("delve-schem-{}-{}", std::process::id(), tag));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn cli_converts_single_structure() {
    let dir = tmp_dir("single");
    let input = dir.join("room.schem");
    let out = dir.join("room.nbt");
    std::fs::write(&input, fixtures::v3_basic()).unwrap();

    let status = Command::new(BIN)
        .args(["convert"])
        .arg(&input)
        .arg("--out")
        .arg(&out)
        .status()
        .unwrap();
    assert!(status.success());

    let view = read_structure(&std::fs::read(&out).unwrap()).unwrap();
    assert_eq!(view.size, [3, 3, 3]);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cli_splits_oversize_and_writes_manifest() {
    let dir = tmp_dir("split");
    let input = dir.join("castle.schem");
    let out = dir.join("castle.nbt");
    std::fs::write(&input, fixtures::v2_oversize()).unwrap();

    let status = Command::new(BIN)
        .args(["convert"])
        .arg(&input)
        .arg("--out")
        .arg(&out)
        .status()
        .unwrap();
    assert!(status.success());

    // 2x1x2 grid of parts + a manifest; the un-suffixed `--out` is not written.
    assert!(!out.exists());
    assert!(dir.join("castle.split.json").exists());
    for name in [
        "castle.x0y0z0.nbt",
        "castle.x0y0z1.nbt",
        "castle.x1y0z0.nbt",
        "castle.x1y0z1.nbt",
    ] {
        assert!(dir.join(name).exists(), "missing part {name}");
    }
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("castle.split.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["grid"], serde_json::json!([2, 1, 2]));
    assert_eq!(manifest["parts"].as_array().unwrap().len(), 4);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cli_reports_bad_input_with_exit_2() {
    let dir = tmp_dir("bad");
    let input = dir.join("junk.schem");
    let out = dir.join("junk.nbt");
    std::fs::write(&input, b"not nbt at all").unwrap();

    let code = Command::new(BIN)
        .args(["convert"])
        .arg(&input)
        .arg("--out")
        .arg(&out)
        .status()
        .unwrap()
        .code();
    assert_eq!(code, Some(2));

    std::fs::remove_dir_all(&dir).ok();
}
