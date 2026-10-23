//! **A commit's writes cannot be seen half done** (spec-0122 §6.4). Each file
//! `delvec detail` writes goes to a temporary file in its own directory and is
//! moved into place with `rename`, so a `--check` reading the whole while a
//! commit runs reads the old file or the new one, never a torn one — and no
//! temporary file outlives the commit.

mod common;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn tempdir(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("dw-detail-atomic-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// Every path under `dir`, recursively, relative to it.
fn paths(dir: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            out.insert(p.strip_prefix(dir).unwrap().display().to_string());
            if p.is_dir() {
                stack.push(p);
            }
        }
    }
    out
}

#[test]
fn a_commit_leaves_only_its_targets_and_a_reader_never_sees_a_torn_plan() {
    let tmp = tempdir("commit");
    let (campaign, prefabs) = common::gallery_site_plan_detailed(&tmp);

    // The annex is taken out of the committed whole — its piece, its report
    // and its row — so the commit below writes every one of its targets anew.
    let targets = [
        "gallery-annex.nbt",
        "gallery-annex.json",
        "gallery-annex.report.json",
    ];
    for f in targets {
        std::fs::remove_file(prefabs.join(f)).unwrap_or_else(|e| panic!("{f}: {e}"));
    }
    common::patch_file(&campaign.join("detail-plan.json"), |v| {
        v["content"]["details"]
            .as_array_mut()
            .unwrap()
            .retain(|r| r["place"] != "node/annex");
    });
    let before = (paths(&prefabs), paths(&campaign));

    // A reader looping on parse for the commit's whole life.
    let plan = campaign.join("detail-plan.json");
    let stop = Arc::new(AtomicBool::new(false));
    let reads = Arc::new(AtomicUsize::new(0));
    let reader = {
        let (stop, reads, plan) = (stop.clone(), reads.clone(), plan.clone());
        std::thread::spawn(move || -> Result<(), String> {
            while !stop.load(Ordering::Relaxed) {
                let text = std::fs::read_to_string(&plan)
                    .map_err(|e| format!("the plan could not be opened mid-commit: {e}"))?;
                serde_json::from_str::<serde_json::Value>(&text)
                    .map_err(|e| format!("a torn plan was read mid-commit: {e}"))?;
                reads.fetch_add(1, Ordering::Relaxed);
            }
            Ok(())
        })
    };
    let out = Command::new(BIN)
        .arg("--prefabs")
        .arg(&prefabs)
        .arg("detail")
        .arg(&campaign)
        .arg("node/annex")
        .output()
        .unwrap();
    stop.store(true, Ordering::Relaxed);
    let read = reader.join().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(read, Ok(()));
    assert!(reads.load(Ordering::Relaxed) > 0, "the reader read nothing");

    // Exactly the targets are new, and no temporary file is left behind.
    let after = (paths(&prefabs), paths(&campaign));
    let new_pieces: BTreeSet<String> = after.0.difference(&before.0).cloned().collect();
    let expected: BTreeSet<String> = targets.iter().map(|s| (*s).to_string()).collect();
    assert_eq!(new_pieces, expected, "the library gained exactly the targets");
    assert_eq!(before.0.difference(&after.0).count(), 0);
    assert_eq!(after.1, before.1, "the campaign gained and lost no path");
    assert!(
        !after.0.iter().chain(&after.1).any(|p| p.ends_with(".tmp")),
        "a temporary file outlived the commit"
    );
    let rows = serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(&plan).unwrap())
        .unwrap()["content"]["details"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["place"] == "node/annex")
        .count();
    assert_eq!(rows, 1, "the row was written");
}
