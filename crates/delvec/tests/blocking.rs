//! `delvec blocking-chart` — the spec-0015 pillar-3 cutaway floor plans, tested
//! at the process level: the slice set, the index contract, multi-storey band
//! detection on a real vertical campaign, the partial-build guarantee, and the
//! ADR-0006 double-run byte-identity gate over every written file.

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn delvec(args: &[&str]) -> Output {
    Command::new(BIN).args(args).output().expect("run delvec")
}

fn code(out: &Output) -> i32 {
    out.status.code().unwrap_or(-1)
}

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Chart `campaign` into `out`.
fn chart(campaign: &Path, out: &Path) -> Output {
    let pf = common::prefabs_dir();
    delvec(&[
        "blocking-chart",
        campaign.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
    ])
}

fn index_of(dir: &Path) -> serde_json::Value {
    let bytes = std::fs::read(dir.join("blocking-chart.json")).expect("index written");
    serde_json::from_slice(&bytes).expect("index is JSON")
}

/// Every file in `dir`, sorted, as `(name, bytes)`.
fn files(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out: Vec<(String, Vec<u8>)> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| {
            let p = e.unwrap().path();
            (
                p.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read(&p).unwrap(),
            )
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[test]
fn writes_one_png_per_band_plus_an_index() {
    let dir = tmp("chart-basic");
    let out = chart(&common::keep_vertical_dir(), &dir);
    assert_eq!(
        code(&out),
        0,
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let idx = index_of(&dir);
    assert_eq!(idx["chart_version"], 1);
    assert!(idx["campaign_id"].as_str().is_some_and(|s| !s.is_empty()));
    assert!(
        idx["orientation"]
            .as_str()
            .expect("orientation documented in-band")
            .contains("north up"),
        "the index must state its own orientation"
    );
    assert!(idx["cut"].as_str().is_some_and(|s| s.contains("dollhouse")));

    let areas = idx["areas"].as_array().expect("areas array");
    assert!(!areas.is_empty());
    let mut total_bands = 0;
    for a in areas {
        assert!(a["area"].as_str().is_some());
        assert!(a["bounds"]["min"].is_array() && a["bounds"]["max"].is_array());
        for b in a["bands"].as_array().expect("bands array") {
            total_bands += 1;
            let file = b["file"].as_str().expect("file name");
            let png = std::fs::read(dir.join(file)).expect("slice PNG written");
            assert_eq!(
                &png[..8],
                &[137, 80, 78, 71, 13, 10, 26, 10],
                "PNG signature"
            );
            assert!(b["floor_y"].is_number());
            assert!(b["walkable_cells"].as_u64().unwrap_or(0) > 0);
            let yr = b["y_range"].as_array().expect("y_range");
            let (lo, hi) = (yr[0].as_i64().unwrap(), yr[1].as_i64().unwrap());
            assert_eq!(
                lo,
                b["floor_y"].as_i64().unwrap() - 1,
                "cut starts one below"
            );
            assert!(hi > lo);
            assert!(b["width"].as_u64().unwrap_or(0) > 0);
            assert!(b["height"].as_u64().unwrap_or(0) > 0);
            assert!(b["labelled"].is_array());
        }
    }
    assert!(total_bands > 0, "at least one slice");
    // Nothing but the slices and the index.
    assert_eq!(files(&dir).len(), total_bands + 1);
}

#[test]
fn a_vertical_campaign_produces_more_than_one_elevation_band() {
    // The acceptance shape of spec-0015 pillar 3: a layout with real vertical
    // structure must be cut into separate storeys, not flattened into one plan
    // where the upper floor hides the lower.
    let dir = tmp("chart-vertical");
    assert_eq!(code(&chart(&common::keep_vertical_dir(), &dir)), 0);
    let idx = index_of(&dir);
    let bands: Vec<i64> = idx["areas"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|a| a["bands"].as_array().unwrap().clone())
        .map(|b| b["floor_y"].as_i64().unwrap())
        .collect();
    assert!(
        bands.len() >= 2,
        "a vertical campaign must yield ≥2 slices, got {bands:?}"
    );
    // Distinct, ascending elevations per area.
    for a in idx["areas"].as_array().unwrap() {
        let ys: Vec<i64> = a["bands"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b["floor_y"].as_i64().unwrap())
            .collect();
        let mut sorted = ys.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(ys, sorted, "ascending, distinct: {ys:?}");
    }
}

#[test]
fn every_walkable_elevation_appears_on_some_slice() {
    // The coverage invariant: relief finds the storeys, and the coverage pass
    // guarantees no populated elevation is left off every chart. Read straight
    // off the index — each band covers `[floor-1, floor+2]`, and their union
    // must be gapless across the elevations the index reports.
    let dir = tmp("chart-coverage");
    assert_eq!(code(&chart(&common::keep_vertical_dir(), &dir)), 0);
    for a in index_of(&dir)["areas"].as_array().unwrap() {
        let bands = a["bands"].as_array().unwrap();
        let ranges: Vec<(i64, i64)> = bands
            .iter()
            .map(|b| {
                let r = b["y_range"].as_array().unwrap();
                (r[0].as_i64().unwrap(), r[1].as_i64().unwrap())
            })
            .collect();
        let (lo, hi) = (
            ranges.iter().map(|r| r.0).min().unwrap(),
            ranges.iter().map(|r| r.1).max().unwrap(),
        );
        for y in lo..=hi {
            assert!(
                ranges.iter().any(|r| y >= r.0 && y <= r.1),
                "elevation {y} falls in no slice's cut: {ranges:?}"
            );
        }
    }
}

#[test]
fn chart_is_byte_identical_across_runs() {
    // ADR-0006 for the chart tier, over the PNGs and the index alike.
    let a = tmp("chart-det-a");
    let b = tmp("chart-det-b");
    assert_eq!(code(&chart(&common::keep_vertical_dir(), &a)), 0);
    assert_eq!(code(&chart(&common::keep_vertical_dir(), &b)), 0);
    let (fa, fb) = (files(&a), files(&b));
    assert_eq!(
        fa.iter().map(|f| f.0.clone()).collect::<Vec<_>>(),
        fb.iter().map(|f| f.0.clone()).collect::<Vec<_>>(),
        "the same file set"
    );
    for ((na, ba), (_, bb)) in fa.iter().zip(fb.iter()) {
        assert_eq!(ba, bb, "{na} differs between runs");
    }
}

#[test]
fn chart_needs_only_placement_not_emission() {
    // Like `snapshot`: a campaign that fails quest-graph analysis still charts,
    // because blocking is a question about geometry, not about the quest graph.
    let dir = tmp("chart-partial");
    let campaign = dir.join("campaign");
    let patch: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(common::compiler_fixtures_dir().join("unreachable-finale.json"))
            .unwrap(),
    )
    .unwrap();
    common::materialize(&patch, &campaign);
    let pf = common::prefabs_dir();
    assert_eq!(
        code(&delvec(&[
            "analyze",
            campaign.to_str().unwrap(),
            "--prefabs",
            pf.to_str().unwrap(),
        ])),
        2,
        "the fixture must fail analysis"
    );
    let out_dir = dir.join("chart");
    let out = chart(&campaign, &out_dir);
    assert_eq!(
        code(&out),
        0,
        "chart must render an unanalyzable campaign: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out_dir.join("blocking-chart.json").is_file());
}

#[test]
fn labels_cover_the_campaign_markers() {
    // Anchors, NPC posts and interact markers must actually reach the chart —
    // an unlabelled plan is a picture, not a blocking chart.
    let dir = tmp("chart-labels");
    assert_eq!(code(&chart(&common::keep_vertical_dir(), &dir)), 0);
    let idx = index_of(&dir);
    let labelled: Vec<String> = idx["areas"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|a| a["bands"].as_array().unwrap().clone())
        .flat_map(|b| b["labelled"].as_array().unwrap().clone())
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(!labelled.is_empty(), "some markers must be labelled");
    assert!(
        labelled.iter().any(|id| id.starts_with("npc/")),
        "NPC posts must be charted: {labelled:?}"
    );
    assert!(
        labelled
            .iter()
            .any(|id| id.contains("anchor") || id == "spawn"),
        "anchors must be charted: {labelled:?}"
    );
}

#[test]
fn json_mode_reports_the_slices() {
    let dir = tmp("chart-json");
    let pf = common::prefabs_dir();
    let campaign = common::keep_vertical_dir();
    let out = delvec(&[
        "blocking-chart",
        campaign.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "-o",
        dir.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code(&out), 0);
    let last = String::from_utf8_lossy(&out.stdout)
        .lines()
        .last()
        .expect("a summary line")
        .to_string();
    let v: serde_json::Value = serde_json::from_str(&last).expect("json summary");
    assert!(v["dir"].as_str().is_some());
    let slices = v["slices"].as_array().expect("slices");
    assert!(!slices.is_empty());
    for s in slices {
        assert!(s["file"].as_str().unwrap().ends_with(".png"));
        assert!(s["area"].as_str().is_some());
        assert!(s["floor_y"].is_number());
    }
}
