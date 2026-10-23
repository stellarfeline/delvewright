//! **The per-place handout script runs against the engine it was written for**
//! (spec-0121).
//!
//! `scripts/handout.py` derives everything a fresh agent is handed from
//! `delvec allocation`, and its Python tests judge it against a RECORDED copy of
//! that output (`tools/tests/fixtures/handout/allocations.json`). A recorded
//! copy is an instrument's output frozen at one revision, so this test is what
//! keeps it honest: the built binary's own answer over the fixture campaign must
//! equal the recording byte for byte, and the handout the script writes with
//! this binary must carry the place's allocation verbatim. A change to what the
//! engine hands a place reds here, and the fixture is re-recorded with the same
//! command the fixture's README names.

mod common;

use std::path::PathBuf;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn fixture() -> PathBuf {
    common::repo_root().join("tools/tests/fixtures/handout")
}

fn script() -> PathBuf {
    common::repo_root().join(".claude/skills/delvewright/skills/new-delve/scripts/handout.py")
}

fn tempdir(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("dw-handout-script-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    // The script prints every path canonical; the temp root is a symlink on
    // macOS, so the expected paths are canonical too.
    p.canonicalize().unwrap()
}

#[test]
fn the_recorded_allocations_are_this_binary_s_own_answer() {
    let out = Command::new(BIN)
        .args([
            "allocation",
            fixture().join("campaign").to_str().unwrap(),
            "--all",
        ])
        .output()
        .expect("run delvec");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let recorded = std::fs::read_to_string(fixture().join("allocations.json")).unwrap();
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        recorded,
        "tools/tests/fixtures/handout/allocations.json is not what this binary prints; re-record it: \
         `delvec allocation tools/tests/fixtures/handout/campaign --all > \
         tools/tests/fixtures/handout/allocations.json`"
    );
}

#[test]
fn the_handout_carries_the_place_s_allocation_verbatim() {
    let tmp = tempdir("handout");
    let campaign = tmp.join("campaign");
    common::copy_dir_all(&fixture().join("campaign"), &campaign);
    let prefabs = tmp.join("prefabs");
    std::fs::create_dir_all(&prefabs).unwrap();
    let out = Command::new("python3")
        .arg("-I")
        .arg(script())
        .arg(&campaign)
        .arg("node/hall")
        .arg("--engine")
        .arg(common::repo_root())
        .arg("--prefabs")
        .arg(&prefabs)
        .arg("--delvec")
        .arg(BIN)
        .output()
        .expect("python3 runs the script");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{stderr}");
    assert!(
        stderr.contains("handout binding: `node/hall` attempt 1"),
        "{stderr}"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let path = stdout.lines().next().expect("the handout's path on stdout");
    assert!(
        path.ends_with("/.out/handouts/hall/handout-1.md"),
        "{stdout}"
    );
    let handout = std::fs::read_to_string(path).unwrap();

    // The allocation the engine hands the hall, as this binary prints it, with
    // the three per-cell lists the script renders one entry per line taken out.
    let alloc = Command::new(BIN)
        .args(["allocation", campaign.to_str().unwrap(), "node/hall"])
        .output()
        .expect("run delvec");
    assert!(alloc.status.success());
    let mut a: serde_json::Value = serde_json::from_slice(&alloc.stdout).unwrap();
    let ground = a["ground"].as_object_mut().unwrap();
    let columns = ground.remove("columns").unwrap();
    let perimeter = ground.remove("perimeter").unwrap();
    let fixed = ground.remove("fixed").unwrap();
    let embedded = serde_json::to_string_pretty(&a).unwrap();
    assert!(
        handout.contains(&embedded),
        "the handout does not carry the allocation verbatim:\n{embedded}"
    );
    assert!(handout.contains(&format!(
        "ground.columns ({};",
        columns.as_array().unwrap().len()
    )));
    assert!(handout.contains(&format!(
        "ground.perimeter ({};",
        perimeter.as_array().unwrap().len()
    )));
    assert!(handout.contains(&format!(
        "ground.fixed ({};",
        fixed.as_array().unwrap().len()
    )));
    // The frame and the check command name this binary by its absolute path.
    assert!(handout.contains("extent **14×10×18**"), "{handout}");
    assert!(handout.contains(&format!(
        "\"{BIN}\" --prefabs \"{}\" detail",
        prefabs.display()
    )));
    // The Agent prompt is the second stdout line.
    assert_eq!(
        stdout.lines().nth(1),
        Some(format!("Read and execute {path}. Report, then exit.").as_str())
    );
}
