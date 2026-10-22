//! **A camera is clear in its own world** (spec-0112 §7.4; criterion 8).
//!
//! A cutscene's dolly is judged in the configuration its firing step arrives
//! under, so a dolly across a gate shut at that step is refused `DW0308`
//! naming the world, and the move the message names — the waypoints off the
//! gate — builds. The plan states how many worlds its cameras were proven in.
//!
//! The `DW0724` half of the criterion — a POV eye inside a region an earlier
//! step filled — is reached by no document: a POV eye is 1.62 above a cell
//! the walk proved standable in that same configuration, so the walk routes
//! round the fill before the eye is asked (measured: a fence gate filled at
//! the exit's head cell re-routes the leg; the build is green with every eye
//! clear). The rule's own shape — the shot, the cell, the world key and the
//! block — is held by `compiler::nav::view`'s unit tests.

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

/// hello-world whose exit re-shuts the door and fires a dolly from the
/// keeper's side to the exit's — through the door — or, with `one_side`, a
/// dolly that stays on the exit's side.
fn campaign(dir: &Path, one_side: bool) -> PathBuf {
    let camp = dir.join("campaign");
    common::copy_dir_all(&common::hello_world_dir(), &camp);
    let (from, to) = if one_side {
        ([0, 2, 0], [1, 2, 0])
    } else {
        ([0, 2, -4], [0, 2, 0])
    };
    common::patch_file(&camp.join("quests.json"), |v| {
        v["content"]["quests"][0]["on_objective_complete"]["obj/exit"] = serde_json::json!([
            {"type": "close-gate", "anchor": "anchor/door",
             "sealed_hint": "The bars hold.",
             "happening": {"verb": "seals", "text": "the door shuts behind the party"}},
            {"type": "cutscene", "shots": [{"path": [
                {"anchor": "anchor/exit", "offset": from},
                {"anchor": "anchor/exit", "offset": to}
            ], "seconds": 2}]}
        ]);
    });
    camp
}

fn build(camp: &Path, out: &Path) -> (i32, String) {
    let o = Command::new(BIN)
        .arg("build")
        .arg(camp)
        .arg("-o")
        .arg(out)
        .arg("--prefabs")
        .arg(common::prefabs_dir())
        .output()
        .unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

#[test]
fn a_dolly_across_a_gate_shut_at_its_step_is_refused_naming_the_world() {
    let dir = tmp("rfe-through");
    let (code, log) = build(&campaign(&dir, false), &dir.join("build"));
    assert_eq!(code, 3, "{log}");
    let line = log
        .lines()
        .find(|l| l.starts_with("DW0308"))
        .unwrap_or_else(|| panic!("no DW0308:\n{log}"));
    assert!(line.contains("in world `after-obj-exit`"), "{line}");
    // The door's top course, shut again at the exit.
    assert!(line.contains("clips a solid block at [5, 67, 6]"), "{line}");

    // The move the message names: the waypoints off the gate.
    let dir = tmp("rfe-one-side");
    let out = dir.join("build");
    let (code, log) = build(&campaign(&dir, true), &out);
    assert_eq!(code, 0, "{log}");
    let plan: serde_json::Value =
        serde_json::from_slice(&std::fs::read(out.join("render-plan.json")).unwrap()).unwrap();
    let proof = &plan["camera_eye_proof"];
    let worlds = proof["worlds"].as_u64().unwrap();
    let keys: std::collections::BTreeSet<&str> = plan["shots"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["world"].as_str().unwrap())
        .collect();
    assert_eq!(worlds as usize, keys.len(), "{proof} over {keys:?}");
    assert!(keys.contains("after-obj-exit"), "{keys:?}");
}

/// The other direction: the talk opens the door, so a dolly the talk fires
/// through the doorway flies in the open configuration and builds — judged in
/// the load world, where the door is shut, it would be refused.
#[test]
fn a_dolly_through_a_gate_its_step_opens_flies() {
    let dir = tmp("rfe-open");
    let camp = dir.join("campaign");
    common::copy_dir_all(&common::hello_world_dir(), &camp);
    common::patch_file(&camp.join("quests.json"), |v| {
        v["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"type": "cutscene", "shots": [{"path": [
                {"anchor": "anchor/exit", "offset": [0, 2, -4]},
                {"anchor": "anchor/exit", "offset": [0, 2, 0]}
            ], "seconds": 2}]}));
    });
    let (code, log) = build(&camp, &dir.join("build"));
    assert_eq!(code, 0, "{log}");
    assert!(
        log.contains("2 cutscene frame(s) of 1 shot(s) in 1 cutscene(s)"),
        "{log}"
    );
}
