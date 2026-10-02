//! spec-0086 — **an endless corridor.**
//!
//! `tests/fixtures/long-gallery` is the primary: one area, one corridor of six
//! identical bays ([`common::corridor`]) whose view closes inside one bay behind
//! two staggered baffles, and one loop whose slab stands across bay 3's mouth
//! and lands a body on bay 2's, released on the count it raises. Every probe
//! below is that primary plus one declared edit, built through the real
//! `delvec build` entry point, so the validation funnel (`DW0949`) and the build
//! (`DW0945`–`DW0948`, `DW0311`) are each reached the way an author reaches them.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

use common::corridor::{self, Cuts, FLOOR_Y, bay};

/// A private copy of the primary with `quests.json` edited by `patch`.
fn campaign(who: &str, patch: impl FnOnce(&mut Value)) -> PathBuf {
    campaign_with(who, patch, |_| {})
}

/// [`campaign`] with a second edit to any other stage document, by name.
fn campaign_with(who: &str, quests: impl FnOnce(&mut Value), other: impl FnOnce(&Path)) -> PathBuf {
    let src = common::compiler_fixtures_dir().join("long-gallery");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("endless-corridor-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for f in common::STAGE_FILES {
        std::fs::copy(src.join(f), dir.join(f)).unwrap();
    }
    common::patch_file(&dir.join("quests.json"), quests);
    other(&dir);
    dir
}

/// What one `delvec build` run said.
struct Run {
    status: i32,
    stderr: String,
    out: PathBuf,
}

impl Run {
    fn green(&self) -> &Self {
        assert_eq!(self.status, 0, "expected a green build:\n{}", self.stderr);
        self
    }

    fn refused(&self, code: &str) -> String {
        assert_ne!(self.status, 0, "expected {code}, got a green build");
        let line = self
            .stderr
            .lines()
            .find(|l| l.starts_with(code) && l.contains("[error]"))
            .unwrap_or_else(|| panic!("expected {code}:\n{}", self.stderr));
        line.to_string()
    }

    fn json(&self, rel: &str) -> Value {
        serde_json::from_str(
            &std::fs::read_to_string(self.out.join(rel))
                .unwrap_or_else(|e| panic!("{rel}: {e}\n{}", self.stderr)),
        )
        .unwrap()
    }

    fn text(&self, rel: &str) -> String {
        std::fs::read_to_string(self.out.join(rel))
            .unwrap_or_else(|e| panic!("{rel}: {e}\n{}", self.stderr))
    }

    fn binding(&self) -> String {
        self.stderr
            .lines()
            .find(|l| l.starts_with("loop binding:"))
            .unwrap_or_else(|| panic!("no loop binding line:\n{}", self.stderr))
            .to_string()
    }
}

fn build_over(dir: &Path, prefabs: &Path) -> Run {
    let out = dir.with_extension("out");
    let _ = std::fs::remove_dir_all(&out);
    let o = Command::new(env!("CARGO_BIN_EXE_delvec"))
        .arg("build")
        .arg(dir)
        .arg("--out")
        .arg(&out)
        .arg("--prefabs")
        .arg(prefabs)
        .output()
        .expect("delvec runs");
    Run {
        status: o.status.code().unwrap_or(-1),
        stderr: format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
        out,
    }
}

fn build(dir: &Path) -> Run {
    let tag = dir.file_name().unwrap().to_string_lossy().to_string();
    build_over(dir, &corridor::gallery_prefabs(&tag, &Cuts::default()))
}

fn build_cut(dir: &Path, cuts: &Cuts) -> Run {
    let tag = dir.file_name().unwrap().to_string_lossy().to_string();
    build_over(dir, &corridor::gallery_prefabs(&tag, cuts))
}

fn loop_mut(q: &mut Value) -> &mut Value {
    &mut q["content"]["loops"][0]
}

/// The loop step `critical-path.json` carries.
fn loop_step(path: &Value) -> Option<Value> {
    path["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["action"] == "loop")
        .cloned()
}

#[test]
fn the_primary_builds_and_exercises_the_loop() {
    let run = build(&campaign("primary", |_| {}));
    run.green();
    let path = run.json("critical-path.json");
    let step = loop_step(&path).expect("the path exercises the loop");
    assert_eq!(step["loop"], "loop/gallery");
    assert_eq!(step["offset"], json!([0, 0, -6]));
    assert_eq!(step["times"], 2);
    let b = run.binding();
    assert!(b.contains("open faces 0"), "{b}");
    assert!(b.contains("forced route meets 1 of 1 holding"), "{b}");
    let _ = (FLOOR_Y, bay(0));
}
