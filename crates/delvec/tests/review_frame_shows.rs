//! **What a frame shows, and what the renderer did not draw — one sight rule**
//! (spec-0112 §7.5, §8.1; criteria 12, 13 and 17, and criterion 4's mannequin).
//!
//! The fixture is the gallery's primary: its curator is a mannequin, and the
//! hall's hardware stands glowing `item_display`s (a lever on the trap, the
//! side door and the inner door) in frame on the legs to `obj/reach-the-end`
//! — a holder the pinned renderer draws, with its item, so it leaves
//! `undrawn` (criterion 17). The standard bearer's banner, an item the pinned
//! renderer cannot draw, is taken out of its hand by the test's one declared
//! edit, so a frame's `undrawn` is about the holders alone.

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

/// The objective on whose legs the hall's hardware once stood in frame.
const MARKED: &str = "obj/reach-the-end";
const MARKER: &str = "dw_hw_";

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
fn a_holder_the_renderer_draws_leaves_undrawn_and_the_curator_is_drawn() {
    let dir = tmp("rfs-shows");
    let (frames, log) = gallery_frames(&dir, |v| {
        let bearer = v["content"]["actors"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|a| a["id"] == "actor/standard-bearer")
            .expect("the gallery's standard bearer");
        bearer["equipment"]
            .as_object_mut()
            .unwrap()
            .remove("main_hand")
            .expect("the bearer holds its banner");
    });
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

    let cid = curator["id"].as_str().unwrap();
    assert!(
        !log.lines()
            .any(|l| l.starts_with("DW1039") && l.contains(&format!("shot `{cid}`:"))),
        "no DW1039 for the curator's frame `{cid}`:\n{log}"
    );

    // spec-0117: no interaction point carries compiler-summoned hardware, so
    // no frame on the leg to the objective shows one.
    let frames_seen = marker_frames(&index);
    assert!(!frames_seen.is_empty(), "the leg to {MARKED} is framed");
    assert!(
        frames_seen.iter().all(|s| s["shows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| { t["id"].as_str().is_none_or(|id| !id.starts_with(MARKER)) })),
        "no hardware display is in any frame"
    );
    assert!(
        log.contains("not drawn by the renderer (DW1039) in "),
        "{log}"
    );
}

/// A campaign with no body the renderer cannot draw prints the measured zero:
/// hello-world, whose keeper is its one body (an objective stands no marker,
/// spec-0104 §6).
#[test]
fn a_campaign_whose_bodies_are_all_drawn_says_zero() {
    let dir = tmp("rfs-zero");
    let camp = dir.join("campaign");
    common::copy_dir_all(&common::hello_world_dir(), &camp);
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
