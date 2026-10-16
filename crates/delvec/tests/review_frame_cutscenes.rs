//! **Cutscene frames** (spec-0112 §7.2; criterion 7): a cutscene a critical
//! step fires gets a frame where each shot starts and, for a moving shot, where
//! it ends, standing in the configuration the firing step arrives under; one
//! no critical step fires gets no frame and is named.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn json(p: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap()
}

/// hello-world whose talk fires a two-shot cutscene — a dolly, then a still —
/// and whose keeper's stand carries an approach trigger no step needs, firing
/// a cutscene of its own.
fn campaign(dir: &Path) -> PathBuf {
    let camp = dir.join("campaign");
    common::copy_dir_all(&common::hello_world_dir(), &camp);
    common::patch_file(&camp.join("quests.json"), |v| {
        v["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"type": "cutscene", "shots": [
                {"path": [
                    {"anchor": "anchor/exit", "offset": [0, 2, 0]},
                    {"anchor": "anchor/exit", "offset": [1, 2, 0]}
                ], "seconds": 2},
                {"path": [{"anchor": "anchor/exit", "offset": [-1, 2, 0]}], "seconds": 1}
            ]}));
        v["content"]["triggers"] = serde_json::json!([{
            "id": "trigger/peek",
            "at": "anchor/keeper-stand",
            "on": {"on": "approach", "range": 2},
            "effects": [{"type": "cutscene", "shots": [
                {"path": [{"anchor": "anchor/keeper-stand", "offset": [0, 2, 0]}], "seconds": 1}
            ]}]
        }]);
    });
    camp
}

#[test]
fn a_fired_cutscene_is_framed_where_each_shot_starts_and_a_dolly_ends() {
    let dir = tmp("rfc");
    let camp = campaign(&dir);
    let out = dir.join("build");
    let o = Command::new(BIN)
        .arg("build")
        .arg(&camp)
        .arg("-o")
        .arg(&out)
        .arg("--prefabs")
        .arg(common::prefabs_dir())
        .output()
        .unwrap();
    let log = String::from_utf8_lossy(&o.stderr).into_owned();
    assert!(o.status.success(), "{log}");
    let plan = json(&out.join("render-plan.json"));
    let cs: Vec<&serde_json::Value> = plan["shots"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["kind"] == "cutscene")
        .collect();
    let ids: Vec<&str> = cs.iter().map(|s| s["id"].as_str().unwrap()).collect();
    // The dolly: its first tick and its last; the still: its first alone.
    assert_eq!(ids.len(), 3, "{ids:?}");
    assert_eq!(ids[0], "cutscene/obj-talk/0/shot0/t0", "{ids:?}");
    assert!(
        ids[1].starts_with("cutscene/obj-talk/0/shot0/t") && ids[1] != ids[0],
        "{ids:?}"
    );
    assert_eq!(ids[2], "cutscene/obj-talk/0/shot1/t0", "{ids:?}");
    for s in &cs {
        assert_eq!(s["camera"]["fov"], 70.0, "{s}");
        assert!(
            s["camera"]["yaw"].is_number() && s["camera"]["pitch"].is_number(),
            "{s}"
        );
        // The configuration the talk's completion arrives under.
        assert_eq!(s["world"], "after-obj-talk", "{s}");
        assert_eq!(s["after"]["step"], "obj/talk", "{s}");
        assert!(!s["expect"].as_array().unwrap().is_empty(), "{s}");
        assert_eq!(s["step"], "obj/talk", "{s}");
    }
    // The dolly's two frames stand where its keyframes stand: one block
    // apart along x.
    let x = |s: &serde_json::Value| s["camera"]["pos"][0].as_f64().unwrap();
    assert!(
        (x(cs[1]) - x(cs[0]) - 1.0).abs() < 1e-6,
        "{} -> {}",
        x(cs[0]),
        x(cs[1])
    );
    // The approach trigger's cutscene: no frame, named on the binding line.
    let line = log
        .lines()
        .find(|l| l.starts_with("review frames: "))
        .unwrap_or_else(|| panic!("{log}"));
    assert!(
        line.contains("3 cutscene frame(s) of 2 shot(s) in 1 cutscene(s), 1 cutscene(s) no critical step fires: /content/triggers/0/effects/0"),
        "{line}"
    );
    let record = json(&out.join("validation/review-frames.json"));
    assert_eq!(
        record["cutscenes"]["unfired"][0],
        "/content/triggers/0/effects/0"
    );

    // `delvec frames --shot` draws one alone.
    let frames = dir.join("frames");
    let o = Command::new(BIN)
        .arg("frames")
        .arg(&out)
        .arg("-o")
        .arg(&frames)
        .args([
            "--shot",
            "cutscene/obj-talk/0/shot0/t0",
            "--size",
            "32",
            "--backend",
            "cpu",
        ])
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let index = json(&frames.join("shot-index.json"));
    assert_eq!(index["count"], 1);
    let e = &index["shots"][0];
    assert_eq!(e["kind"], "cutscene");
    for k in ["step", "shot", "tick", "seconds"] {
        assert!(!e["cutscene"][k].is_null(), "`{k}` in {e}");
    }
}

/// A campaign with no cutscene prints the measured zero.
#[test]
fn a_campaign_with_no_cutscene_says_zero() {
    let dir = tmp("rfc-none");
    let out = dir.join("build");
    let o = Command::new(BIN)
        .arg("build")
        .arg(common::hello_world_dir())
        .arg("-o")
        .arg(&out)
        .arg("--prefabs")
        .arg(common::prefabs_dir())
        .output()
        .unwrap();
    let log = String::from_utf8_lossy(&o.stderr).into_owned();
    assert!(o.status.success(), "{log}");
    assert!(
        log.contains("0 cutscene frame(s) of 0 shot(s) in 0 cutscene(s), 0 cutscene(s) no critical step fires;"),
        "{log}"
    );
}
