//! Process-level `delvec calibrate` tests (spec-0019 §4): the write-back half of
//! the rehearsal loop — its exit-code / diagnostic matrix and the lossless
//! round trip from a harvested proposal to an applied DSL patch.

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

const LAYOUT: &str = r#"{
  "campaign_id": "island",
  "anchors": [
    { "id": "anchor/fire-pit", "area": "area/island", "pos": [9, 69, -56] },
    { "id": "anchor/exit", "area": "area/island", "pos": [5, 67, 8] }
  ]
}"#;

/// A report naming cells within reach of the anchors above.
const REPORT: &str = r#"{
  "version": "0.1.0",
  "campaign_id": "island",
  "shots": [
    { "shot": 1, "beat": 1, "pointer": "/content/quests/0/on_complete/0/shots/0",
      "path": [[6, 67, 7], [12, 70, -50]], "look_at": [9, 69, -56], "seconds": 7 }
  ]
}"#;

fn fixture(dir: &Path, report: &str, layout: &str) -> (PathBuf, PathBuf, PathBuf) {
    let r = dir.join("rehearsal-report.json");
    let l = dir.join("layout.json");
    let o = dir.join("shot-patch.json");
    std::fs::write(&r, report).unwrap();
    std::fs::write(&l, layout).unwrap();
    (r, l, o)
}

fn run(dir: &Path, report: &str, layout: &str) -> (Output, PathBuf) {
    let (r, l, o) = fixture(dir, report, layout);
    let out = delvec(&[
        "calibrate",
        r.to_str().unwrap(),
        "--layout",
        l.to_str().unwrap(),
        "-o",
        o.to_str().unwrap(),
    ]);
    (out, o)
}

/// The happy path: every proposal snaps, the patch is written in the DSL's own
/// `anchor + offset` vocabulary, and `seconds` carries through untouched.
#[test]
fn snappable_proposals_become_an_anchor_offset_patch() {
    let dir = tmp("calibrate-ok");
    let (out, patch) = run(&dir, REPORT, LAYOUT);
    assert_eq!(code(&out), 0, "{}", String::from_utf8_lossy(&out.stdout));
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&patch).unwrap()).unwrap();
    assert_eq!(v["version"], "0.1.0");
    assert_eq!(v["campaign_id"], "island");
    let p = &v["patches"][0];
    assert_eq!(p["pointer"], "/content/quests/0/on_complete/0/shots/0");
    assert_eq!(p["patch"]["seconds"], 7);
    assert_eq!(p["patch"]["path"][0]["anchor"], "anchor/exit");
    assert_eq!(
        p["patch"]["path"][0]["offset"],
        serde_json::json!([1, 0, -1])
    );
    // A zero offset is spelled the way the DSL spells it: a bare anchor.
    assert_eq!(
        p["patch"]["look_at"],
        serde_json::json!({ "anchor": "anchor/fire-pit" })
    );
    assert!(v["unsnappable"].as_array().unwrap().is_empty());
}

/// `DW0390`: a proposal with no declared anchor within the snap radius is
/// reported and the shot is left un-patched — the converter never invents an
/// anchor and never writes a raw world coordinate (spec-0019 §5). The exit is
/// non-zero, and the patch file is still written so the snappable shots of the
/// same session are not thrown away.
#[test]
fn dw0390_far_proposal_is_reported_not_snapped() {
    let dir = tmp("calibrate-far");
    let report = r#"{
      "version": "0.1.0", "campaign_id": "island",
      "shots": [
        { "shot": 1, "beat": 1, "pointer": "/a", "path": [[6, 67, 7]], "seconds": 5 },
        { "shot": 2, "beat": 1, "pointer": "/b", "path": [[400, 90, 400]], "seconds": 5 }
      ]
    }"#;
    let (out, patch) = run(&dir, report, LAYOUT);
    assert_eq!(code(&out), 3);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("DW0390"), "{stderr}");
    assert!(
        stderr.contains("NOT widen the radius"),
        "the diagnostic names the tempting wrong fix: {stderr}"
    );
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&patch).unwrap()).unwrap();
    assert_eq!(
        v["patches"].as_array().unwrap().len(),
        1,
        "shot 1 still patched"
    );
    assert_eq!(v["unsnappable"][0]["shot"], 2);
}

/// `DW0391`: calibrating one build's proposals against another build's anchors
/// would silently relocate every camera. Refused before any snapping happens.
#[test]
fn dw0391_campaign_mismatch_is_refused() {
    let dir = tmp("calibrate-mismatch");
    let layout = LAYOUT.replace("\"island\"", "\"some-other-delve\"");
    let (out, _) = run(&dir, REPORT, &layout);
    assert_eq!(code(&out), 1);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("DW0391"), "{stderr}");
    assert!(stderr.contains("some-other-delve"), "{stderr}");
}

/// `DW0392`: a missing, malformed or wrong-version report is refused with the
/// prescription to re-harvest rather than hand-edit.
#[test]
fn dw0392_unreadable_report_is_refused() {
    let dir = tmp("calibrate-bad");
    // Missing file.
    let missing = delvec(&[
        "calibrate",
        dir.join("nope.json").to_str().unwrap(),
        "--layout",
        dir.join("layout.json").to_str().unwrap(),
    ]);
    assert_eq!(code(&missing), 1);
    assert!(
        String::from_utf8_lossy(&missing.stderr).contains("DW0392"),
        "{}",
        String::from_utf8_lossy(&missing.stderr)
    );
    // Not a report at all.
    let (out, _) = run(&dir, "{\"hello\":true}", LAYOUT);
    assert_eq!(code(&out), 1);
    assert!(String::from_utf8_lossy(&out.stderr).contains("DW0392"));
    // A schema version this delvec does not understand.
    let (out, _) = run(&dir, &REPORT.replace("\"0.1.0\"", "\"9.9.9\""), LAYOUT);
    assert_eq!(code(&out), 1);
    assert!(String::from_utf8_lossy(&out.stderr).contains("DW0392"));
}

/// `--json` emits the same one-object-per-line diagnostic shape the rest of the
/// CLI does, so the agent loop parses calibration failures like any other.
#[test]
fn json_diagnostics_match_the_cli_contract() {
    let dir = tmp("calibrate-json");
    let (r, l, o) = fixture(
        &dir,
        r#"{ "version": "0.1.0", "campaign_id": "island",
             "shots": [ { "shot": 1, "beat": 1, "pointer": "/a",
                          "path": [[400, 90, 400]], "seconds": 5 } ] }"#,
        LAYOUT,
    );
    let out = delvec(&[
        "--json",
        "calibrate",
        r.to_str().unwrap(),
        "--layout",
        l.to_str().unwrap(),
        "-o",
        o.to_str().unwrap(),
    ]);
    assert_eq!(code(&out), 3);
    let line = String::from_utf8_lossy(&out.stdout)
        .lines()
        .find(|l| l.contains("DW0390"))
        .map(str::to_string)
        .expect("a JSON diagnostic line");
    let d: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(d["code"], "DW0390");
    assert_eq!(d["severity"], "error");
    assert_eq!(d["stage"], "build");
}

/// The converter is a pure function of its two inputs: two runs produce
/// byte-identical patches (ADR-0006).
#[test]
fn calibration_is_deterministic() {
    let a = tmp("calibrate-det-a");
    let b = tmp("calibrate-det-b");
    let (_, pa) = run(&a, REPORT, LAYOUT);
    let (_, pb) = run(&b, REPORT, LAYOUT);
    assert_eq!(std::fs::read(&pa).unwrap(), std::fs::read(&pb).unwrap());
}
