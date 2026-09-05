//! **`delvec validate` checks the graph as a graph** — the promise the
//! authoring page makes at the site-plan step, asserted at the process level.
//!
//! `crates/delvec/tests/layout_graph.rs` asserts the in-process binding: the
//! validation pass raises the reachability proofs and the analysis pass no
//! longer raises them a second time. This file asserts the thing an author
//! actually does. The page tells them to loop `delvec validate` while the
//! layout graph is being written, so what has to be true is that the VERB
//! refuses — not that a function somewhere would have.
//!
//! The subject is the metrics gym, because it is the engine's own worked
//! example and it is generated rather than checked in: a fixture would be a
//! second copy of a campaign that the generator already writes from the table,
//! and it would go stale the moment the table moved. One perturbation is
//! planted in it — the last edge of its critical path is made to run backwards
//! — and that is a fault of the graph alone, which no other check in the
//! battery can see.
//!
//! Before this moved, `validate` was exit 0 and silent on exactly this
//! campaign: the proofs lived in the analysis pass, and the analysis pass runs
//! only on a campaign that already validates, so nothing on the authoring loop
//! ever asked. That is the red this test would have been.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

mod common;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

/// Every verb here names the prefab library explicitly. The default resolves
/// `campaigns/prefabs` against the process's working directory, which under
/// `cargo test` is the package root and not the repository root — an absent
/// library is `internal error` at exit 10, which is not the refusal any of
/// these tests is about.
fn prefabs() -> String {
    common::prefabs_dir()
        .to_str()
        .expect("utf-8 path")
        .to_string()
}

fn delvec(args: &[&str]) -> Output {
    Command::new(BIN).args(args).output().expect("run delvec")
}

fn code(out: &Output) -> i32 {
    out.status.code().unwrap_or(-1)
}

/// Both streams, joined. `delvec` prints its DIAGNOSTICS on stdout and its
/// BINDING lines on stderr, so a test that reads one of them is asserting over
/// half the run — and the half carrying the binding counts reads convincing
/// enough to hide the absence of the refusal.
fn said(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Generate the gym into `dir` and return the campaign directory.
fn gym(dir: &Path) -> PathBuf {
    let out = delvec(&["metrics", "--gym", dir.to_str().unwrap()]);
    assert_eq!(code(&out), 0, "the gym did not generate: {}", said(&out));
    dir.to_path_buf()
}

/// Turn the last step of the critical path around, and name the two places it
/// was between so the assertions can quote them.
fn reverse_the_last_critical_step(campaign: &Path) -> (String, String) {
    let p = campaign.join("layout-graph.json");
    let mut v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&p).expect("read the graph"))
            .expect("the graph parses");
    let path = v["content"]["critical_path"]
        .as_array()
        .expect("critical_path")
        .clone();
    let from = path[path.len() - 2]
        .as_str()
        .expect("a node id")
        .to_string();
    let to = path[path.len() - 1]
        .as_str()
        .expect("a node id")
        .to_string();
    let mut planted = 0;
    for e in v["content"]["edges"].as_array_mut().expect("edges") {
        if e["a"] == serde_json::json!(from) && e["b"] == serde_json::json!(to) {
            e["one_way"] = serde_json::json!("b-to-a");
            planted += 1;
        }
    }
    assert_eq!(planted, 1, "the perturbation must land on exactly one edge");
    std::fs::write(&p, serde_json::to_string_pretty(&v).unwrap()).expect("write the graph");
    (from, to)
}

/// The unperturbed gym is green under `validate`, so the refusal below is the
/// perturbation's and nothing else's.
#[test]
fn the_gym_validates() {
    let dir = tmp("validate-graph-green");
    let c = gym(&dir);
    let out = delvec(&["--prefabs", &prefabs(), "validate", c.to_str().unwrap()]);
    assert_eq!(
        code(&out),
        0,
        "the generated gym should validate: {}",
        said(&out)
    );
}

/// One backwards edge on the critical path, and `delvec validate` refuses it by
/// name: the goal is unreachable (`DW0816`) and the authored path does not hold
/// (`DW0817`).
#[test]
fn validate_refuses_a_graph_whose_goal_cannot_be_reached() {
    let dir = tmp("validate-graph-red");
    let c = gym(&dir);
    let (from, to) = reverse_the_last_critical_step(&c);

    let out = delvec(&["--prefabs", &prefabs(), "validate", c.to_str().unwrap()]);
    let said = said(&out);
    assert_eq!(code(&out), 1, "validate should refuse it: {said}");
    assert!(
        said.contains("DW0816"),
        "`{to}` is unreachable and validate did not say so: {said}"
    );
    assert!(
        said.contains("DW0817"),
        "the critical path steps from `{from}` to `{to}` over nothing and validate did not say \
         so: {said}"
    );
    // The message names the place, which is what makes the refusal actionable
    // at the step it fires: an author holding a graph and no coordinates has
    // nothing else to look at.
    assert!(
        said.contains(&to),
        "the refusal does not name the place it is about: {said}"
    );
}

/// The verbs downstream refuse it too, and they refuse it at the SAME gate.
///
/// `analyze` and `build` both validate first, so moving the proofs out of the
/// analysis pass cannot open a road to a built world — it only changes which
/// gate the refusal comes from, and therefore the exit code, which is exit 1
/// here rather than the analysis tier's 2. Asserted rather than reasoned,
/// because "nothing downstream regressed" is exactly the claim nobody rechecks.
#[test]
fn analyze_and_build_refuse_it_at_the_validation_gate() {
    let dir = tmp("validate-graph-downstream");
    let c = gym(&dir);
    reverse_the_last_critical_step(&c);
    let outdir = tmp("validate-graph-downstream-out");

    let prefabs = prefabs();
    for args in [
        vec!["--prefabs", &prefabs, "analyze", c.to_str().unwrap()],
        vec![
            "--prefabs",
            &prefabs,
            "build",
            c.to_str().unwrap(),
            "--out",
            outdir.to_str().unwrap(),
        ],
    ] {
        let verb = args[2];
        let out = delvec(&args);
        let said = said(&out);
        assert_eq!(code(&out), 1, "`{verb}` should refuse it at exit 1: {said}");
        assert!(
            said.contains("DW0816") && said.contains("DW0817"),
            "`{verb}` refused without naming the graph fault: {said}"
        );
    }
}
