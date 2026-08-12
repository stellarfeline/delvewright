//! `delve-grammar expand` at the command line — the surface an author actually
//! touches.
//!
//! The library tests prove `export_zone` tiles correctly. They do not prove the
//! binary calls it: `run_expand` used `export_prefab` for as long as the export
//! existed, and a region past the structure-template cap was a refusal *at the
//! CLI* with a suggestion that the author re-author their design as several
//! jigsaw-socketed prefabs. That is exactly the shape a doc line cannot fix, so
//! the binary gets its own test.

use std::path::PathBuf;
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_delve-grammar")
}

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("delve-grammar-cli-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A region past the cap on two axes expands, gates and freezes — no refusal,
/// no flag to pass, nothing about 48 in the author's way.
#[test]
fn a_region_past_the_structure_cap_expands_into_a_tile_set() {
    let dir = tmp("oversize");
    let out = Command::new(bin())
        .args(["expand", "--program", "castle", "--region", "90x14x130"])
        .args(["--seed", "7", "--id", "grammar-keep", "-o"])
        .arg(&dir)
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "a giant zone is essential content, never a refusal: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // The manifest, and every tile it names, are on disk under the names it
    // gives.
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("grammar-keep.json")).unwrap())
            .unwrap();
    let set = &manifest["structure_set"];
    assert_eq!(set["size"], serde_json::json!([90, 14, 130]));
    assert_eq!(set["grid"], serde_json::json!([2, 1, 3]));
    let parts = set["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 6);
    for part in parts {
        let file = part["file"].as_str().unwrap();
        assert!(dir.join(file).exists(), "{file} was named and not written");
        for axis in 0..3 {
            assert!(
                part["size"][axis].as_i64().unwrap() <= 48,
                "{file} is past the cap on axis {axis}"
            );
        }
    }

    // The gate report is about the zone, and it is still written beside the
    // pieces: packaging changed, judgement did not.
    let report: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("grammar-keep.report.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(report["verdict"], "pass");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("tile(s) in a 2x1x3 grid"),
        "the operator is told the packaging happened: {stderr}"
    );
    assert!(
        stderr.contains("judged the whole zone"),
        "...and that a tile was not what was judged: {stderr}"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

/// A region that fits still writes the two files it always wrote, under the
/// names it always used. The tiling path must be invisible here.
#[test]
fn a_region_that_fits_writes_one_structure_and_its_metadata() {
    let dir = tmp("fits");
    let out = Command::new(bin())
        .args(["expand", "--program", "castle", "--region", "41x14x25"])
        .args(["--seed", "7", "--id", "grammar-castle", "-o"])
        .arg(&dir)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{out:?}");

    assert!(dir.join("grammar-castle.nbt").exists());
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("grammar-castle.json")).unwrap())
            .unwrap();
    assert_eq!(meta["structure"]["file"], "grammar-castle.nbt");
    assert!(
        meta.get("structure_set").is_none(),
        "a prefab that fits one template is not a tile set"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}
