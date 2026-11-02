//! spec-0110 criterion 10: every build with a cutscene prints the shot-subject
//! binding line, every figure derived with its denominator, and writes
//! `validation/shot-subjects.json` naming what is unchecked and the digest of
//! the shot list measured; a cutscene-less campaign prints and writes neither.

mod common;

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn build(dir: &std::path::Path, tag: &str) -> (i32, String, std::path::PathBuf) {
    let out = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(tag);
    let _ = std::fs::remove_dir_all(&out);
    let r = Command::new(BIN)
        .args(["build", dir.to_str().unwrap(), "-o", out.to_str().unwrap()])
        .arg("--prefabs")
        .arg(common::prefabs_dir())
        .output()
        .unwrap();
    (
        r.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&r.stderr).to_string(),
        out,
    )
}

#[test]
fn a_build_with_a_cutscene_prints_the_line_and_writes_the_ledger() {
    let (status, stderr, out) = build(&common::cutscene_shots_dir(), "framing-ledger-shots");
    assert_eq!(status, 0, "{stderr}");
    let line = stderr
        .lines()
        .find(|l| l.starts_with("shot subjects: "))
        .unwrap_or_else(|| panic!("no binding line:\n{stderr}"));
    // Two landscape shots in one cutscene: zero objects stated, zero ticks.
    assert!(
        line.starts_with(
            "shot subjects: 2 shot(s) in 1 cutscene(s), 2 declare a subject (2 landscape, 0 \
             marks, 0 bodies, 0 assemblies, 0 places, 0 regions); 0 tick(s) judged, 0 out of \
             frame, 0 occluded, clear lines 0 of 0; bodies: 0 present"
        ),
        "{line}"
    );
    let ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(out.join("validation/shot-subjects.json")).unwrap())
            .unwrap();
    let unchecked = ledger["unchecked"].as_array().unwrap();
    assert_eq!(unchecked.len(), delvec::compiler::framing::UNCHECKED.len());
    assert!(unchecked.iter().any(|u| u == "options.fov"));
    let digest = ledger["cutscenes"][0]["cutscene_digest"].as_str().unwrap();
    assert_eq!(digest.len(), 8, "{digest}");
    assert_eq!(ledger["counts"]["shots"], 2);
}

#[test]
fn a_cutscene_less_campaign_prints_and_writes_neither() {
    let (status, stderr, out) = build(&common::hello_world_dir(), "framing-ledger-none");
    assert_eq!(status, 0, "{stderr}");
    assert!(!stderr.contains("shot subjects:"), "{stderr}");
    assert!(!out.join("validation/shot-subjects.json").exists());
}
