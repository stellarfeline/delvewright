//! **The author's line comes first.**
//!
//! A run's diagnostics used to reach the terminal in the order the passes
//! happened to produce them. Measured on every site-plan run before this:
//! `DW0813` ("the metrics gym has not walked them"), `DW0822` ("carries NO
//! threshold and refuses nothing") and the per-run binding lines all printed
//! **ahead** of the first refusal — four to six paragraphs saying *this is fine*
//! or *the engine's own table is provisional*, in front of the one line the
//! author was there to act on.
//!
//! What is asserted here is the ORDER and the count, never that anything went
//! quiet: every code that reported before reports now, at the same tier, and the
//! run's binding counts are all still stated. A test that let a line disappear
//! would be the vacuity this repository names by name.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use delvewright_dsl::{Group, Subject};

fn tempdir(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("dw-order-{name}"));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// `delvec validate`'s stdout, which is where diagnostics go.
fn validate_stdout(campaign: &Path, prefabs: &Path) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_delvec"))
        .arg("--prefabs")
        .arg(prefabs)
        .arg("validate")
        .arg(campaign)
        .output()
        .expect("`delvec validate` runs");
    String::from_utf8(out.stdout).expect("stdout is utf-8")
}

/// The blockout fixture — a site-plan campaign, which is the shape that produced
/// the measurement — with its region shortened so that boxes leave it. That is
/// one edit and it puts all three groups on the screen at once: a refusal
/// (`DW0826`), an advisory about the campaign (`DW0822`), and a notice about the
/// engine (`DW0813`).
fn a_run_with_all_three_groups() -> String {
    let tmp = tempdir("three-groups");
    let campaign = tmp.join("campaign");
    let prefabs = tmp.join("prefabs");
    std::fs::create_dir_all(&prefabs).unwrap();
    common::copy_dir_all(
        &common::repo_root().join("crates/compiler/tests/fixtures/blockout"),
        &campaign,
    );
    common::patch_file(&campaign.join("site-plan.json"), |v| {
        let y = v["content"]["region"]["extent"][1].as_i64().unwrap();
        v["content"]["region"]["extent"][1] = serde_json::json!(y / 2);
    });
    validate_stdout(&campaign, &prefabs)
}

/// Position of the first line whose code is `code`, or `None`.
fn line_of(out: &str, code: &str) -> Option<usize> {
    out.lines().position(|l| l.starts_with(code))
}

#[test]
fn a_runs_lines_are_grouped_with_the_authors_first() {
    let out = a_run_with_all_three_groups();

    let refusal = out
        .lines()
        .position(|l| l.contains("refusal(s): these are yours to act on"))
        .unwrap_or_else(|| panic!("no refusal heading in:\n{out}"));
    let campaign = out
        .lines()
        .position(|l| l.contains("advisory(ies) about this campaign"))
        .unwrap_or_else(|| panic!("no campaign heading in:\n{out}"));
    let engine = out
        .lines()
        .position(|l| l.contains("notice(s) about this engine"))
        .unwrap_or_else(|| panic!("no engine heading in:\n{out}"));
    assert!(
        refusal < campaign && campaign < engine,
        "author-actionable first, then the campaign, then the engine:\n{out}"
    );

    // And the diagnostics themselves land in their own group's stretch.
    let refusal_line = line_of(&out, "DW0826").expect("the shortened region refuses");
    let campaign_line = line_of(&out, "DW0822").expect("the pacing figure is an advisory");
    let engine_line = line_of(&out, "DW0813").expect("the provisional table is an engine notice");
    assert!(
        refusal < refusal_line
            && refusal_line < campaign
            && campaign < campaign_line
            && campaign_line < engine
            && engine < engine_line,
        "every line is under its own heading:\n{out}"
    );
}

/// **Nothing went quiet.** Each of the three prints exactly once per run — the
/// count is the assertion, because "print it once" is satisfiable by printing it
/// zero times and a heading count is not.
#[test]
fn each_advisory_prints_exactly_once_per_run() {
    let out = a_run_with_all_three_groups();
    for code in ["DW0822", "DW0813"] {
        let n = out.lines().filter(|l| l.starts_with(code)).count();
        assert_eq!(n, 1, "{code} appears {n} time(s) in:\n{out}");
    }
    // The refusal is present too — an ordering test over a run with no refusal
    // would assert the interesting half of nothing.
    assert!(
        out.lines().any(|l| l.starts_with("DW0826")),
        "the perturbation still refuses:\n{out}"
    );
}

/// The green run has no refusal group at all, and the two advisories still print
/// in their own order. This is the case an author meets most often.
#[test]
fn a_green_run_prints_the_campaign_before_the_engine() {
    let tmp = tempdir("green");
    let prefabs = tmp.join("prefabs");
    std::fs::create_dir_all(&prefabs).unwrap();
    let out = validate_stdout(
        &common::repo_root().join("crates/compiler/tests/fixtures/blockout"),
        &prefabs,
    );
    assert!(
        !out.contains("refusal(s): these are yours to act on"),
        "a green run has no refusals to head:\n{out}"
    );
    let campaign = line_of(&out, "DW0822").expect("the pacing figure");
    let engine = line_of(&out, "DW0813").expect("the provisional table");
    assert!(campaign < engine, "{out}");
}

/// **Which group a code belongs to is a property of the code**, declared where
/// the code is, not a list some printer keeps. The test an author's line is
/// sorted by: *if the campaign changed nothing and the engine's own tables were
/// finished, would this go away?*
#[test]
fn the_subject_of_a_code_is_declared_beside_it() {
    assert_eq!(
        delvewright_dsl::metrics::DW_METRIC_PROVISIONAL.subject(),
        Subject::Engine,
        "`DW0813` reports that an ENGINE table is still seeded; no campaign can move it"
    );
    assert_eq!(
        delvewright_dsl::layout::DW_PACING.subject(),
        Subject::Campaign,
        "`DW0822` is a measurement of THIS graph's critical path"
    );
    assert_eq!(
        delvewright_compiler::faces::DW_FACE_UNBOUND.subject(),
        Subject::Campaign,
        "`DW0781` counts THIS world's pieces and faces"
    );
    // The default is the campaign, so a code that says nothing about its subject
    // is addressed to the author — the safe direction.
    assert_eq!(Subject::default(), Subject::Campaign);
    assert!(Group::Refusal < Group::AboutTheCampaign);
    assert!(Group::AboutTheCampaign < Group::AboutTheEngine);
}
