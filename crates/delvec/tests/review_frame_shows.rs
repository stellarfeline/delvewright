//! **What a frame shows, and what the renderer did not draw — one sight rule**
//! (spec-0112 §7.5, §8.1; criteria 12 and 13, and criterion 4's mannequin).
//!
//! The fixture is the gallery's primary: its curator is a mannequin, and its
//! marked `reach` objectives stand a lantern marker (an `item_display`) in
//! frame on the legs to them — a body the pinned renderer does not draw yet.

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

/// The objective whose lantern marker the report is asked about.
const MARKED: &str = "obj/board-the-ferry";
const MARKER: &str = "dw_r_board_the_ferry";

/// Build the gallery's primary under `dir` (with `edit` applied to its
/// quests), and draw every frame on the legs to [`MARKED`] and to the
/// curator's first leg. Returns the frames directory and `delvec frames`' log.
fn gallery_frames(dir: &Path, edit: impl FnOnce(&mut serde_json::Value)) -> (PathBuf, String) {
    let (campaign, prefabs) = common::gallery_primary_at(&dir.join("gallery"));
    common::patch_file(&campaign.join("quests.json"), edit);
    let out = dir.join("build");
    let o = Command::new(BIN)
        .arg("build")
        .arg(&campaign)
        .arg("-o")
        .arg(&out)
        .arg("--prefabs")
        .arg(&prefabs)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let plan = json(&out.join("render-plan.json"));
    let shots: Vec<String> = plan["shots"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| {
            s["kind"] == "pov"
                && (s["objective"] == MARKED || s["objective"] == "obj/hear-the-curator")
        })
        .map(|s| s["id"].as_str().unwrap().to_string())
        .collect();
    assert!(!shots.is_empty());
    let frames = dir.join("frames");
    let mut cmd = Command::new(BIN);
    cmd.arg("frames")
        .arg(&out)
        .arg("-o")
        .arg(&frames)
        .args(["--size", "96", "--backend", "cpu"]);
    for s in &shots {
        cmd.arg("--shot").arg(s);
    }
    let o = cmd.output().unwrap();
    let log = String::from_utf8_lossy(&o.stderr).into_owned();
    assert!(o.status.success(), "{log}");
    (frames, log)
}

fn marker_frames(index: &serde_json::Value) -> Vec<&serde_json::Value> {
    index["shots"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["objective"] == MARKED)
        .collect()
}

#[test]
fn a_body_the_renderer_does_not_draw_is_reported_and_one_it_draws_is_not() {
    let dir = tmp("rfs-shows");
    let (frames, log) = gallery_frames(&dir, |_| {});
    let index = json(&frames.join("shot-index.json"));

    // Criterion 12: every entry's `shows` and `out_of_frame` are the frame
    // manifest's, target for target.
    for s in index["shots"].as_array().unwrap() {
        let m = json(
            &frames.join(
                s["image"]
                    .as_str()
                    .unwrap()
                    .replace(".png", ".manifest.json"),
            ),
        );
        assert_eq!(s["shows"], m["targets"], "{}", s["id"]);
        assert_eq!(s["out_of_frame"], m["out_of_frame"], "{}", s["id"]);
        assert_eq!(s["undrawn"], m["undrawn"], "{}", s["id"]);
    }

    // Criterion 4: the curator, a mannequin, is in frame and drawn.
    let curator =
        index["shots"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| {
                s["shows"].as_array().unwrap().iter().any(|t| {
                    t["kind"] == "body" && t["body"] == "mannequin" && t["occluded"] == false
                })
            })
            .expect("a frame with the curator in it");
    assert!(
        curator["undrawn"]
            .as_array()
            .unwrap()
            .iter()
            .all(|u| u["body"] != "mannequin"),
        "{curator}"
    );

    // Criterion 13: a frame on the leg to the marked objective whose only
    // undrawn body is its lantern marker.
    let only = marker_frames(&index)
        .into_iter()
        .find(|s| {
            let u = s["undrawn"].as_array().unwrap();
            u.len() == 1 && u[0]["id"].as_str().unwrap().starts_with(MARKER)
        })
        .expect("a frame whose one undrawn body is the marker");
    let u = &only["undrawn"][0];
    assert_eq!(u["reason"], "items", "{u}");
    assert!(u["judge_by"].as_str().is_some_and(|j| !j.is_empty()), "{u}");
    assert_eq!(u["body"], "item_display", "{u}");
    let id = only["id"].as_str().unwrap();
    let reports = log
        .lines()
        .filter(|l| l.starts_with("DW1039") && l.contains(&format!("shot `{id}`:")))
        .count();
    assert_eq!(reports, 1, "DW1039 once for `{id}`:\n{log}");
    assert!(
        log.contains("not drawn by the renderer (DW1039) in "),
        "{log}"
    );

    // The same build with the objective unmarked: nothing undrawn in that
    // frame.
    let dir2 = tmp("rfs-shows-unmarked");
    let (frames2, _) = gallery_frames(&dir2, |v| {
        for q in v["content"]["quests"].as_array_mut().unwrap() {
            for o in q["objectives"].as_array_mut().unwrap() {
                if o["id"] == MARKED {
                    o["marker"] = serde_json::json!("hidden");
                }
            }
        }
    });
    let index2 = json(&frames2.join("shot-index.json"));
    let same = index2["shots"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == id)
        .expect("the same shot");
    assert_eq!(same["undrawn"], serde_json::json!([]), "{same}");
}

/// Criterion 12: one sight rule — one `fn occluded_target`, in
/// `compiler::framing`, and the index's `shows` read off the build's review
/// record, which `framing::resolve_targets` wrote.
#[test]
fn one_sight_rule() {
    let src = common::repo_root().join("crates/delvec/src");
    let mut defs = Vec::new();
    let mut stack = vec![src.clone()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                let text = std::fs::read_to_string(&p).unwrap();
                for l in text.lines() {
                    if l.contains("fn occluded_target") {
                        defs.push(p.strip_prefix(&src).unwrap().display().to_string());
                    }
                }
            }
        }
    }
    assert_eq!(defs, ["compiler/framing.rs"], "{defs:?}");
    let review = std::fs::read_to_string(src.join("compiler/view/review.rs")).unwrap();
    assert!(review.contains("framing::resolve_targets("));
    let frameset = std::fs::read_to_string(src.join("compiler/view/frameset.rs")).unwrap();
    assert!(
        !frameset.contains("resolve_targets(") && !frameset.contains("occluded_target("),
        "the index re-derives what a frame shows"
    );
}

/// A campaign with no body the renderer cannot draw prints the measured zero:
/// hello-world with its exit unmarked, so the keeper is its one body.
#[test]
fn a_campaign_whose_bodies_are_all_drawn_says_zero() {
    let dir = tmp("rfs-zero");
    let camp = dir.join("campaign");
    common::copy_dir_all(&common::hello_world_dir(), &camp);
    common::patch_file(&camp.join("quests.json"), |v| {
        v["content"]["quests"][0]["objectives"][1]["marker"] = serde_json::json!("hidden");
    });
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
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let o = Command::new(BIN)
        .arg("frames")
        .arg(&out)
        .arg("-o")
        .arg(dir.join("frames"))
        .args(["--size", "32", "--backend", "cpu"])
        .output()
        .unwrap();
    let log = String::from_utf8_lossy(&o.stderr).into_owned();
    assert!(o.status.success(), "{log}");
    assert!(
        log.contains(", 0 not drawn by the renderer (DW1039) in 0 frame(s)"),
        "{log}"
    );
    assert!(!log.contains("DW1039 ["), "{log}");
}
