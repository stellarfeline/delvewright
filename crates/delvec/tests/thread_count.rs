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

mod common;

fn tmp(name: &str) -> PathBuf {
    let p = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn read_tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
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
    cmd.output().expect("delvec runs")
}

#[test]
fn the_output_is_the_same_at_every_thread_count() {
    let root = tmp("thread-count");
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
        c.insert("horizon".into(), serde_json::json!("valley"));
        c.insert("boundary".into(), serde_json::json!({ "margin": 20 }));
    });

    let runs: Vec<(&str, Option<&str>)> = vec![("1", Some("1")), ("3", Some("3")), ("host", None)];
    let mut built: Vec<(&str, BTreeMap<String, Vec<u8>>, Vec<u8>)> = Vec::new();
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
            String::from_utf8_lossy(&built[0].2),
            String::from_utf8_lossy(stderr),
            "the same diagnostics at 1 and {name} thread(s)"
        );
    }
}
