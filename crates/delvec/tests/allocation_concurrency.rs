//! **The wave bound is the engine's, stated by `delvec allocation --all`**
//! (spec-0122 §6.2): `concurrency: N = min(P, max(1, C / 2))`, P the plan's
//! places with no passing program (no `details[]` row), C the machine's
//! `available_parallelism()`. The line goes to stderr; stdout stays the one
//! JSON document the handout parses.

mod common;

use std::path::PathBuf;
use std::process::{Command, Output};

use delvec::compiler::detail::{concurrency_bound, unauthored_places};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn delvec(args: &[&str]) -> Output {
    Command::new(BIN).args(args).output().expect("run delvec")
}

fn tempdir(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("dw-alloc-conc-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

#[test]
fn the_bound_is_min_of_places_and_half_the_cores_and_never_below_one_author() {
    // Zero places to author is zero authors, whatever the machine.
    for c in [1, 2, 7, 64] {
        assert_eq!(concurrency_bound(0, c), 0, "P = 0, C = {c}");
    }
    // One core still runs one author at a time: N = min(P, 1).
    for p in [1, 2, 9] {
        assert_eq!(concurrency_bound(p, 1), 1.min(p), "C = 1, P = {p}");
    }
    assert_eq!(concurrency_bound(9, 16), 8);
    assert_eq!(concurrency_bound(3, 16), 3);
    assert_eq!(concurrency_bound(9, 5), 2);
}

/// Parse `concurrency: N = min(P, max(1, C / 2))` into `(N, P, C)`.
fn parse(line: &str) -> (usize, usize, usize) {
    let rest = line.strip_prefix("concurrency: ").expect("the prefix");
    let (n, rest) = rest.split_once(" = min(").unwrap();
    let (p, rest) = rest.split_once(", max(1, ").unwrap();
    let c = rest.strip_suffix(" / 2))").unwrap();
    (n.parse().unwrap(), p.parse().unwrap(), c.parse().unwrap())
}

#[test]
fn allocation_all_prints_the_bound_on_stderr_from_the_machine_and_the_plan() {
    let tmp = tempdir("line");
    let dir = tmp.join("site-plan");
    common::gallery_site_plan_at(&dir);
    let ds = dir.to_str().unwrap();
    let cores = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);

    let state = |label: &str| {
        let out = delvec(&["allocation", ds, "--all"]);
        assert!(out.status.success(), "{label}");
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        let lines: Vec<&str> = stderr
            .lines()
            .filter(|l| l.starts_with("concurrency: "))
            .collect();
        assert_eq!(lines.len(), 1, "{label}: exactly one line: {stderr}");
        // stdout is the JSON document and nothing else.
        let _: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        parse(lines[0])
    };

    // The committed point: P counts the boxes with no `details[]` row.
    let c = common::campaign_at(&dir);
    let p_committed = unauthored_places(&c);
    let (n, p, cc) = state("committed");
    assert_eq!((p, cc), (p_committed, cores));
    assert_eq!(n, concurrency_bound(p_committed, cores));

    // No row at all: every box the plan allocates is to be authored.
    common::patch_file(&dir.join("detail-plan.json"), |v| {
        v["content"]["details"] = serde_json::json!([]);
    });
    let boxes = delvec::compiler::detail::frames(&common::campaign_at(&dir)).len();
    assert!(boxes > 1, "the binding: {boxes} box(es)");
    let (n, p, cc) = state("no rows");
    assert_eq!(p, boxes, "every box unauthored");
    assert_eq!(cc, cores);
    assert_eq!(n, concurrency_bound(boxes, cores));
    assert!(n >= 1);

    // A single place is never asked for, and prints no bound.
    let out = delvec(&["allocation", ds, "node/annex"]);
    assert!(out.status.success());
    assert!(!String::from_utf8_lossy(&out.stderr).contains("concurrency: "));
}
