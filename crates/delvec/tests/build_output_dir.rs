//! A build directory holds exactly the build last written into it
//! (`delvec::outdir`, `DW0967`).
//!
//! The defect this guards: `delvec build -o <dir>` only ever wrote, so a reused
//! directory kept every file an earlier build emitted and the current one does
//! not — live advancements and the PackTests of removed objectives among them —
//! and the server booted from it loaded both.

mod common;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("build-output-dir")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn build(campaign: &Path, out: &Path) -> Output {
    Command::new(BIN)
        .args([
            "build",
            campaign.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--prefabs",
            common::prefabs_dir().to_str().unwrap(),
        ])
        .output()
        .expect("run delvec")
}

fn ok(o: &Output) {
    assert_eq!(
        o.status.code(),
        Some(0),
        "build: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}

/// Every file under `root`, `/`-joined, sorted.
fn files(root: &Path) -> BTreeSet<String> {
    fn walk(root: &Path, dir: &Path, acc: &mut BTreeSet<String>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(root, &p, acc);
            } else {
                acc.insert(
                    p.strip_prefix(root)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .replace('\\', "/"),
                );
            }
        }
    }
    let mut acc = BTreeSet::new();
    walk(root, root, &mut acc);
    acc
}

/// What the build's own manifest says it emitted, plus the manifest.
fn emitted(root: &Path) -> BTreeSet<String> {
    let m: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("manifest.json")).unwrap()).unwrap();
    let mut s: BTreeSet<String> = m["outputs"].as_object().unwrap().keys().cloned().collect();
    s.insert("manifest.json".into());
    s
}

/// hello-world with `obj/exit` removed: a second build of the same campaign
/// that emits strictly less.
fn without_exit(at: &Path) -> PathBuf {
    let src = common::hello_world_dir();
    let dst = at.join("hello-world-b");
    std::fs::create_dir_all(&dst).unwrap();
    for e in std::fs::read_dir(&src).unwrap() {
        let p = e.unwrap().path();
        std::fs::copy(&p, dst.join(p.file_name().unwrap())).unwrap();
    }
    let qp = dst.join("quests.json");
    let mut q: serde_json::Value = serde_json::from_slice(&std::fs::read(&qp).unwrap()).unwrap();
    let objs = q["content"]["quests"][0]["objectives"]
        .as_array_mut()
        .unwrap();
    let before = objs.len();
    objs.retain(|o| o["id"] != "obj/exit");
    assert_eq!(
        objs.len(),
        before - 1,
        "the fixture carries obj/exit to remove"
    );
    std::fs::write(&qp, serde_json::to_vec_pretty(&q).unwrap()).unwrap();
    dst
}

#[test]
fn a_rebuild_with_an_objective_removed_leaves_exactly_the_second_build() {
    let work = scratch("rebuild");
    let b_src = without_exit(&work);

    // B alone, into a fresh directory: the reference.
    let fresh = work.join("fresh-b");
    ok(&build(&b_src, &fresh));

    // A, then B, into one directory.
    let out = work.join("out");
    ok(&build(&common::hello_world_dir(), &out));
    let a_files = files(&out);
    let r = build(&b_src, &out);
    ok(&r);

    let left = files(&out);
    let named = emitted(&out);
    let stale: Vec<_> = left.difference(&named).collect();
    assert!(
        stale.is_empty(),
        "files of the first build survive the second: {stale:?}"
    );
    assert_eq!(
        left,
        files(&fresh),
        "the reused directory differs from a fresh build of B"
    );
    for f in &left {
        assert_eq!(
            std::fs::read(out.join(f)).unwrap(),
            std::fs::read(fresh.join(f)).unwrap(),
            "{f}: bytes differ from a fresh build"
        );
    }
    // The removal is real, not vacuous: A emitted files B does not.
    let removed: Vec<_> = a_files.difference(&left).collect();
    assert!(
        removed.iter().any(|f| f.contains("exit")),
        "the first build emitted nothing for obj/exit that the second removed: {removed:?}"
    );
    let said = String::from_utf8_lossy(&r.stderr);
    assert!(
        said.contains(&format!(
            "removed {} file(s) the previous tree emitted",
            removed.len()
        )),
        "the rebuild states what it removed: {said}"
    );
}

#[test]
fn a_directory_the_build_does_not_own_is_refused_and_untouched() {
    let work = scratch("unowned");
    let out = work.join("out");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("thesis.tex"), "years of work").unwrap();
    let r = build(&common::hello_world_dir(), &out);
    assert_eq!(r.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&r.stderr).contains("DW0967"));
    assert_eq!(files(&out), BTreeSet::from(["thesis.tex".to_string()]));
}

#[test]
fn a_file_beside_a_build_that_its_manifest_does_not_name_is_refused() {
    let work = scratch("foreign-file");
    let out = work.join("out");
    ok(&build(&common::hello_world_dir(), &out));
    std::fs::write(out.join("datapack/notes.txt"), "mine").unwrap();
    let before = files(&out);
    let r = build(&common::hello_world_dir(), &out);
    assert_eq!(r.status.code(), Some(3));
    let said = String::from_utf8_lossy(&r.stderr);
    assert!(
        said.contains("DW0967") && said.contains("`datapack/notes.txt`"),
        "{said}"
    );
    assert_eq!(files(&out), before, "a refusal removes and writes nothing");
}

#[test]
fn the_previous_builds_world_save_is_removed_by_a_rebuild() {
    let work = scratch("world");
    let out = work.join("out");
    ok(&build(&common::hello_world_dir(), &out));
    std::fs::create_dir_all(out.join("world/region")).unwrap();
    std::fs::write(out.join("world/level.dat"), "the previous geometry").unwrap();
    ok(&build(&common::hello_world_dir(), &out));
    assert!(
        !out.join("world").exists(),
        "a world save of the previous build survived"
    );
    assert_eq!(files(&out), emitted(&out));
}
