//! `world.demo_level`: a demo level says so, and its build carries the
//! declaration in `manifest.json` for the staging gate to read.
//!
//! One campaign is built twice, once as written and once with the declaration,
//! and the two manifests are compared: the declared build carries
//! `demo_level: true`, the other carries no such key, and every emitted path
//! hashes the same — nothing a player meets moves.

mod common;

use std::path::Path;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn build(campaign: &Path, out: &Path) -> serde_json::Value {
    let _ = std::fs::remove_dir_all(out);
    let r = Command::new(BIN)
        .arg("build")
        .arg(campaign)
        .arg("-o")
        .arg(out)
        .arg("--prefabs")
        .arg(common::prefabs_dir())
        .output()
        .expect("run delvec build");
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    serde_json::from_slice(&std::fs::read(out.join("manifest.json")).expect("manifest"))
        .expect("manifest parses")
}

fn campaign_at(dir: &Path, demo_level: Option<bool>) {
    let _ = std::fs::remove_dir_all(dir);
    common::copy_dir_all(&common::hello_world_dir(), dir);
    if let Some(v) = demo_level {
        common::patch_file(&dir.join("world.json"), |w| {
            w["content"]["demo_level"] = serde_json::json!(v);
        });
    }
}

#[test]
fn a_declared_demo_level_is_carried_in_the_manifest_and_moves_nothing_else() {
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR")).join("demo-level");
    let (plain, demo) = (tmp.join("plain"), tmp.join("demo"));
    campaign_at(&plain, None);
    campaign_at(&demo, Some(true));
    let a = build(&plain, &tmp.join("plain.out"));
    let b = build(&demo, &tmp.join("demo.out"));

    assert!(a.get("demo_level").is_none(), "{a}");
    assert_eq!(b["demo_level"], serde_json::json!(true), "{b}");
    assert_eq!(a["outputs"], b["outputs"], "a demo level emits the same world");
    let (ai, bi) = (a["inputs"].as_object().unwrap(), b["inputs"].as_object().unwrap());
    assert_eq!(
        ai.keys().collect::<Vec<_>>(),
        bi.keys().collect::<Vec<_>>()
    );
    let moved: Vec<&String> = ai.keys().filter(|k| ai[*k] != bi[*k]).collect();
    assert_eq!(moved, vec!["world.json"], "only the declaring document moves");
}

#[test]
fn demo_level_false_is_the_same_build_as_no_declaration() {
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR")).join("demo-level-false");
    let (plain, off) = (tmp.join("plain"), tmp.join("off"));
    campaign_at(&plain, None);
    campaign_at(&off, Some(false));
    let a = build(&plain, &tmp.join("plain.out"));
    let b = build(&off, &tmp.join("off.out"));
    assert!(b.get("demo_level").is_none(), "{b}");
    assert_eq!(a["outputs"], b["outputs"]);
}
