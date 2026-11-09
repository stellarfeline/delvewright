//! **What a build emits does not depend on how many threads built it**
//! (ADR-0006).
//!
//! `delvec build` spreads independent work over cores through `delvec::par`,
//! and every result is merged back in a fixed order, so the output must be the
//! sequential build's at any thread count. `DELVEC_THREADS` sets the count; at
//! 1 every map runs on the calling thread, which is the sequential build.
//!
//! The subject reaches every parallel site a small campaign can: a piece cut
//! into several templates (decoded per template, placed in template order) under
//! a `valley` horizon (its tile stacks built per footprint and numbered in
//! footprint order). The test states both counts, so a subject that stopped
//! reaching them would fail here rather than pass over nothing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::common;

fn tmp(name: &str) -> PathBuf {
    let p = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// An output tree: relative path to bytes.
type Tree = BTreeMap<String, Vec<u8>>;

fn read_tree(root: &Path) -> Tree {
    fn walk(base: &Path, dir: &Path, map: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(base, &path, map);
            } else {
                let rel = path
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                map.insert(rel, std::fs::read(&path).unwrap());
            }
        }
    }
    let mut map = BTreeMap::new();
    walk(root, root, &mut map);
    map
}

/// Build `campaign` into `out` with `DELVEC_THREADS` set to `threads`, or unset
/// for `None` (the host's own parallelism).
fn build(campaign: &Path, prefabs: &Path, out: &Path, threads: Option<&str>) -> Output {
    build_with(campaign, prefabs, out, threads, false)
}

/// [`build`], with `DELVEC_PHASE_TIMING` set when `timing` and removed
/// otherwise.
fn build_with(
    campaign: &Path,
    prefabs: &Path,
    out: &Path,
    threads: Option<&str>,
    timing: bool,
) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_delvec"));
    cmd.args([
        "build",
        campaign.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        prefabs.to_str().unwrap(),
    ]);
    match threads {
        Some(n) => cmd.env("DELVEC_THREADS", n),
        None => cmd.env_remove("DELVEC_THREADS"),
    };
    if timing {
        cmd.env("DELVEC_PHASE_TIMING", "1");
    } else {
        cmd.env_remove("DELVEC_PHASE_TIMING");
    }
    cmd.output().expect("delvec runs")
}

/// The subject: a piece cut into several templates under a `valley` horizon.
fn subject(root: &Path) -> (PathBuf, PathBuf) {
    let prefabs = root.join("prefabs");
    common::write_tiled_zone(
        &prefabs,
        "tiled-corridor",
        serde_json::json!({
            "spawn": { "pos": [4, 1, 2], "facing": "south", "role": "entry" },
            "anchor/keeper-stand": { "pos": [4, 1, 4], "facing": "north" },
            "anchor/door": {
                "region": { "from": [4, 1, 30], "to": [4, 3, 30] },
                "block": "minecraft:iron_bars"
            },
            "anchor/exit": { "pos": [4, 1, 55] }
        }),
        &[],
    );
    let campaign = common::campaign_bound_to(&root.join("campaign"), "tiled-corridor");
    common::patch_file(&campaign.join("world.json"), |v| {
        let c = v["content"].as_object_mut().unwrap();
        c.insert("horizon".into(), common::horizon("valley"));
        c.insert("boundary".into(), serde_json::json!({ "margin": 20 }));
    });
    (campaign, prefabs)
}

#[test]
fn the_output_is_the_same_at_every_thread_count() {
    let root = tmp("thread-count");
    let (campaign, prefabs) = subject(&root);

    let runs: Vec<(&str, Option<&str>)> = vec![("1", Some("1")), ("3", Some("3")), ("host", None)];
    let mut built: Vec<(&str, Tree, Vec<u8>)> = Vec::new();
    for (name, threads) in runs {
        let out = root.join(format!("out-{name}"));
        let r = build(&campaign, &prefabs, &out, threads);
        assert!(
            r.status.success(),
            "the subject builds at {name} thread(s):\n{}",
            String::from_utf8_lossy(&r.stderr)
        );
        built.push((name, read_tree(&out), r.stderr));
    }

    // The subject reaches the parallel sites: several templates of one piece,
    // and several horizon tiles.
    let (_, first, _) = &built[0];
    let tiles = first
        .keys()
        .filter(|p| p.contains("/structure/tiled-corridor"))
        .count();
    let horizon = first
        .keys()
        .filter(|p| p.contains("/structure/horizon/valley/"))
        .count();
    eprintln!(
        "thread-count binding: {} file(s) compared across {} build(s); {tiles} template(s) of \
         the piece, {horizon} horizon tile(s)",
        first.len(),
        built.len()
    );
    assert!(tiles >= 2, "the piece ships as several templates: {tiles}");
    assert!(
        horizon >= 2,
        "the horizon ships as several tiles: {horizon}"
    );

    for (name, tree, stderr) in &built[1..] {
        assert_eq!(
            first.keys().collect::<Vec<_>>(),
            tree.keys().collect::<Vec<_>>(),
            "the same files at 1 and {name} thread(s)"
        );
        for (path, bytes) in first {
            assert!(
                bytes == &tree[path],
                "{path} differs between 1 and {name} thread(s)"
            );
        }
        assert_eq!(
            without_wall_time(&String::from_utf8_lossy(&built[0].2)),
            without_wall_time(&String::from_utf8_lossy(stderr)),
            "the same diagnostics at 1 and {name} thread(s)"
        );
    }
}

/// A diagnostic stream with the three measurements of the machine taken out:
/// each written world's wall time (`written in <n> ms`, spec-0112 §9), the
/// snapshot's count of page reads served by a thread's recent pages
/// (`reach snapshot: … H of T page read(s) served …`, ADR-0037 §2), which a
/// per-thread cache answers differently at every thread count, and the
/// section stores' paging (`world store pages: …`, ADR-0037 §1), whose
/// give-ups, read-backs and measured need fall as the threads' reads fall.
/// The need is the machine's in every line that states it: each raise
/// (`world store need: raised to …`) is said as it happens, on a clock of the
/// arrivals over every thread's stores, so where walks run beside each other
/// how many raises a run makes is the schedule's — those lines are dropped —
/// and the budget's line (`reach budget: …`), which states that need and what
/// is resident at the end, is normalised; so is the scratch files' line
/// (`reach scratch: …`, ADR-0037 §2), whose lengths depend on the order the
/// threads free records and write the next ones into their room. Every other byte is compared, and
/// the count of lines normalised is asserted equal by the comparison itself.
fn without_wall_time(text: &str) -> String {
    text.lines()
        .filter(|l| !l.starts_with("world store need: "))
        .map(|l| match l.rfind(", written in ") {
            Some(i) if l.ends_with(" ms") && l.starts_with("world: ") => {
                format!("{}, written in <ms>", &l[..i])
            }
            _ if l.starts_with("world store pages: ") => {
                "world store pages: <the machine's> (ADR-0037 §1)".to_string()
            }
            _ if l.starts_with("reach budget: ") => {
                "reach budget: <the machine's> (ADR-0037 §1)".to_string()
            }
            _ if l.starts_with("reach scratch: ") => {
                "reach scratch: <the machine's> (ADR-0037 §2)".to_string()
            }
            _ if l.starts_with("reach snapshot: ") => match l.rfind("; ") {
                Some(i) if l[i..].contains(" served by a thread's recent pages") => {
                    format!(
                        "{}; <hits> served by a thread's recent pages (ADR-0037 §2)",
                        &l[..i]
                    )
                }
                _ => l.to_string(),
            },
            _ => l.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// **Phase timing is the log's alone** (`DELVEC_PHASE_TIMING`,
/// `compiler/notes.rs`): the subject built with it set and unset writes the
/// same tree, and the stamped stderr is the plain one once each note's stamp
/// and each `phase:` mark are taken out. The marks are counted, so a timing
/// switch that stopped stamping would fail here rather than compare nothing.
#[test]
fn phase_timing_moves_no_output_byte_and_only_stamps_the_log() {
    let root = tmp("phase-timing");
    let (campaign, prefabs) = subject(&root);
    let plain_out = root.join("out-plain");
    let timed_out = root.join("out-timed");
    let plain = build_with(&campaign, &prefabs, &plain_out, Some("1"), false);
    let timed = build_with(&campaign, &prefabs, &timed_out, Some("1"), true);
    for (name, r) in [("plain", &plain), ("timed", &timed)] {
        assert!(
            r.status.success(),
            "the subject builds {name}:\n{}",
            String::from_utf8_lossy(&r.stderr)
        );
    }
    let (a, b) = (read_tree(&plain_out), read_tree(&timed_out));
    assert_eq!(
        a.keys().collect::<Vec<_>>(),
        b.keys().collect::<Vec<_>>(),
        "the same files with phase timing set and unset"
    );
    for (path, bytes) in &a {
        assert!(bytes == &b[path], "{path} differs with phase timing set");
    }
    let plain_err = String::from_utf8_lossy(&plain.stderr);
    let timed_err = String::from_utf8_lossy(&timed.stderr);
    assert!(!plain_err.contains("phase: "), "unset, no mark is printed");
    let unstamp = |l: &str| -> String {
        match l.strip_prefix("[t=+").and_then(|r| r.split_once("] ")) {
            Some((_, rest)) => rest.to_string(),
            None => l.to_string(),
        }
    };
    let marks = timed_err
        .lines()
        .filter(|l| unstamp(l).starts_with("phase: "))
        .count();
    let stamped = timed_err.lines().filter(|l| l.starts_with("[t=+")).count();
    eprintln!(
        "phase-timing binding: {} file(s) compared, {marks} mark(s), {stamped} stamped line(s)",
        a.len()
    );
    assert!(
        marks >= 2,
        "the timed build prints its phase marks: {marks}"
    );
    let timed_plain: Vec<String> = timed_err
        .lines()
        .map(unstamp)
        .filter(|l| !l.starts_with("phase: "))
        .collect();
    assert_eq!(
        without_wall_time(&plain_err),
        without_wall_time(&timed_plain.join("\n")),
        "the stamped log is the plain one, stamps and marks taken out"
    );
}
