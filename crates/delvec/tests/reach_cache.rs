//! The node bank across runs (`DELVEC_REACH_CACHE`): a build that reads what
//! an earlier run's bank holds emits the same bytes as one that flies every
//! node, a run that grew a bank leaves its file, a run that grew nothing
//! leaves the file as it found it, and a file that does not decode is read as
//! empty (ADR-0006).

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn build(out: &Path, cache: Option<&Path>) -> Output {
    let pf = common::prefabs_dir();
    let mut cmd = Command::new(BIN);
    cmd.args(["build"])
        .arg(common::hello_world_dir())
        .arg("-o")
        .arg(out)
        .arg("--prefabs")
        .arg(&pf)
        .env_remove("DELVEC_REACH_CACHE");
    if let Some(c) = cache {
        cmd.env("DELVEC_REACH_CACHE", c);
    }
    let o = cmd.output().expect("run delvec");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    o
}

/// Every file under `dir`, by its path relative to `dir`, with its bytes.
fn tree(dir: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(root: &Path, at: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        for e in std::fs::read_dir(at).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                let rel = p.strip_prefix(root).unwrap().to_string_lossy().into_owned();
                out.push((rel, std::fs::read(&p).unwrap()));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn banks(cache: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out: Vec<(PathBuf, Vec<u8>)> = std::fs::read_dir(cache)
        .map(|d| {
            d.map(|e| e.unwrap().path())
                .filter(|p| p.extension().is_some_and(|x| x == "bank"))
                .map(|p| {
                    let b = std::fs::read(&p).unwrap();
                    (p, b)
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

#[test]
fn a_build_from_an_earlier_run_s_bank_emits_what_flying_emits() {
    let root = tmp("reach-cache");
    let cache = root.join("cache");
    let flown = root.join("flown");
    build(&flown, None);
    assert!(banks(&cache).is_empty(), "no cache named, none written");

    let cold = root.join("cold");
    build(&cold, Some(&cache));
    let written = banks(&cache);
    assert!(
        !written.is_empty(),
        "a run that grew a bank leaves its file"
    );
    assert!(written.iter().all(|(_, b)| b.len() > 8));

    let warm = root.join("warm");
    build(&warm, Some(&cache));
    assert_eq!(
        banks(&cache),
        written,
        "a run that took every node from the file grew nothing and wrote nothing"
    );

    // A file cut short is read as empty, and the run writes a whole one back.
    for (p, b) in &written {
        std::fs::write(p, &b[..b.len() / 2]).unwrap();
    }
    let cut = root.join("cut");
    build(&cut, Some(&cache));
    let rewritten = banks(&cache);
    assert_eq!(rewritten.len(), written.len());
    for ((p, b), (_, was)) in rewritten.iter().zip(&written) {
        assert!(
            b.len() > was.len() / 2,
            "{p:?}: the cut file is replaced by a whole one"
        );
    }

    let want = tree(&flown);
    assert!(!want.is_empty());
    for (name, dir) in [("cold", &cold), ("warm", &warm), ("cut", &cut)] {
        assert!(
            tree(dir) == want,
            "the {name} build's tree differs from the one that flew every node"
        );
    }
}
