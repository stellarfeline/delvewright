//! **`delvec detail --check` and the commit are two runs of one verb**
//! (spec-0122 §6.1). `--check` is the commit's own `prepare` and `judge_run`
//! with the write and the battery not called, so over every place and every
//! refusal the two must say the same thing, code for code, and the check must
//! leave the library and `detail-plan.json` byte-identical.
//!
//! What is compared: the DW codes each run prints up to the point the commit
//! writes — the commit's `detail: N place(s) detailed` line, after which only
//! the battery speaks. The battery is the one stage `--check` does not run by
//! the spec's own definition, so the codes it adds are not part of the
//! agreement; a commit that refuses before writing prints no such line and is
//! compared whole.

mod common;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn delvec(args: &[&str]) -> Output {
    Command::new(BIN).args(args).output().expect("run delvec")
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn tempdir(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("dw-detail-check-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// Every file under `dir`, recursively, with its bytes' hash.
fn snapshot(dir: &Path) -> BTreeSet<(String, String)> {
    use sha2::{Digest, Sha256};
    let mut out = BTreeSet::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let h = format!("{:x}", Sha256::digest(std::fs::read(&p).unwrap()));
                out.insert((p.strip_prefix(dir).unwrap().display().to_string(), h));
            }
        }
    }
    out
}

/// The DW codes a run printed, in order, up to the commit's write line.
fn codes(out: &Output) -> Vec<String> {
    let stderr = String::from_utf8_lossy(&out.stderr);
    let mut found = Vec::new();
    for line in stderr.lines() {
        if line.starts_with("detail: ") && line.contains("place(s) detailed of") {
            break;
        }
        let bytes = line.as_bytes();
        let mut i = 0;
        while i + 6 <= bytes.len() {
            if &bytes[i..i + 2] == b"DW" && bytes[i + 2..i + 6].iter().all(u8::is_ascii_digit) {
                found.push(line[i..i + 6].to_string());
                i += 6;
            } else {
                i += 1;
            }
        }
    }
    found
}

/// One place, checked then committed: the check writes nothing, and the two
/// print the same codes. Returns the shared code list.
fn agree(campaign: &Path, prefabs: &Path, place: &str) -> (Vec<String>, i32, i32, String) {
    let cs = campaign.to_str().unwrap();
    let ps = prefabs.to_str().unwrap();
    let before = (snapshot(prefabs), snapshot(campaign));
    let check = delvec(&["--prefabs", ps, "detail", "--check", cs, place]);
    assert_eq!(
        before,
        (snapshot(prefabs), snapshot(campaign)),
        "`--check {place}` wrote something: {}",
        text(&check)
    );
    let commit = delvec(&["--prefabs", ps, "detail", cs, place]);
    let (c, k) = (codes(&check), codes(&commit));
    assert_eq!(
        c,
        k,
        "{place}: `--check` and the commit disagree\n--check: {}\ncommit: {}",
        text(&check),
        text(&commit)
    );
    let check_exit = check.status.code().unwrap_or(-1);
    let commit_exit = commit.status.code().unwrap_or(-1);
    if check_exit == 0 {
        assert!(
            text(&check).contains("peak rss: "),
            "the check states its peak: {}",
            text(&check)
        );
    } else {
        // A refusal before the write is the same refusal, at the same exit.
        assert_eq!(check_exit, commit_exit, "{place}");
    }
    let shown = format!("--check: {}\ncommit: {}", text(&check), text(&commit));
    (c, check_exit, commit_exit, shown)
}

fn stems(campaign: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(campaign.join("programs"))
        .unwrap()
        .filter_map(|e| {
            let p = e.unwrap().path();
            (p.extension().and_then(|x| x.to_str()) == Some("json"))
                .then(|| p.file_stem().unwrap().to_string_lossy().to_string())
        })
        .collect();
    out.sort();
    out
}

fn patch(path: &Path, f: impl FnOnce(&mut Value)) {
    common::patch_file(path, f);
}

#[test]
fn check_and_commit_agree_over_every_gallery_place_and_three_refusals() {
    let tmp = tempdir("agree");
    let (campaign, prefabs) = common::gallery_site_plan_detailed(&tmp);
    let places = stems(&campaign);
    assert!(places.len() >= 4, "the binding: {places:?}");

    // Green: every program place of the gallery's site-plan point.
    for stem in &places {
        let (_, check, commit, shown) = agree(&campaign, &prefabs, &format!("node/{stem}"));
        assert_eq!((check, commit), (0, 0), "node/{stem}: {shown}");
    }
    eprintln!(
        "agreement binding: {n} of {n} program place(s) checked and committed",
        n = places.len()
    );

    // Each refusal over its own copy of the committed whole.
    let copy = |name: &str| -> (PathBuf, PathBuf) {
        let root = tmp.join(name);
        let (c, p) = (root.join("site-plan"), root.join("prefabs"));
        common::copy_dir_all(&campaign, &c);
        common::copy_dir_all(&prefabs, &p);
        (c, p)
    };

    // DW0843 — an undersize piece: the perch's box shrinks under its
    // committed piece, and the annex's check judges the whole that holds it.
    let (c, p) = copy("dw0843");
    patch(&c.join("site-plan.json"), |v| {
        let boxes = v["content"]["boxes"].as_array_mut().unwrap();
        let b = boxes
            .iter_mut()
            .find(|b| b["node"] == "node/perch")
            .unwrap();
        b["extent"] = json!([8, 6]);
    });
    let (k, _, _, shown) = agree(&c, &p, "node/annex");
    assert!(k.contains(&"DW0843".to_string()), "{k:?}\n{shown}");

    // DW0844 — an unanswered seam: the annex's arch cut one cell along the
    // wall from where the plan hands it.
    let (c, p) = copy("dw0844");
    patch(&c.join("programs/annex.json"), |v| {
        let size = &mut v["rules"]["east_band"][0]["body"]["sizes"][0]["blocks"];
        *size = json!({"expr": "arith", "lhs": size.clone(), "op": "add",
                       "rhs": {"expr": "int", "value": 1}});
    });
    let (k, _, _, shown) = agree(&c, &p, "node/annex");
    assert!(k.contains(&"DW0844".to_string()), "{k:?}\n{shown}");

    // DW0845 — an owed name unbound: the annex's own place mark removed.
    let (c, p) = copy("dw0845");
    patch(&c.join("programs/annex.json"), |v| {
        let body = v["rules"]["room"][0]["body"]["body"].clone();
        v["rules"]["room"][0]["body"] = body;
    });
    let (k, _, _, shown) = agree(&c, &p, "node/annex");
    assert!(k.contains(&"DW0845".to_string()), "{k:?}\n{shown}");
}

#[test]
fn check_beside_all_is_refused_naming_the_pair_and_writes_nothing() {
    let tmp = tempdir("check-all");
    let campaign = tmp.join("site-plan");
    common::gallery_site_plan_at(&campaign);
    let prefabs = tmp.join("prefabs");
    std::fs::create_dir_all(&prefabs).unwrap();
    let before = (snapshot(&prefabs), snapshot(&campaign));
    let out = delvec(&[
        "--prefabs",
        prefabs.to_str().unwrap(),
        "detail",
        "--check",
        "--all",
        campaign.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    let t = text(&out);
    assert!(t.contains("`--check` and `--all`"), "{t}");
    assert_eq!(before, (snapshot(&prefabs), snapshot(&campaign)));
}
