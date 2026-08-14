//! `delve-grammar audit` — the walk that makes the gates INVOKED.
//!
//! The gates themselves were correct and bound long before this existed. What
//! did not exist was anything that ran them over a corpus: `expand` judges the
//! one program an operator names, and a campaign's zone programs — the artifacts
//! of record under `campaigns/<c>/design/programs/` — had no caller at all. That
//! is CLAUDE.md's fourth vacuity mode, a gate nothing invokes, and this file is
//! about the ways this command must refuse rather than the gates it runs (those
//! are `library.rs`, `zones.rs` and `shape_orient.rs`).
//!
//! Every refusal below is watched going red on a tree written for it. The three
//! that matter are the ones that keep the audit from going quietly dark: a
//! programs directory with no manifest, a program file no manifest names, and a
//! corpus that turned out to be empty.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const GRAMMAR: &str = env!("CARGO_BIN_EXE_delve-grammar");

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-grammar-audit-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn combined(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn audit(args: &[&str]) -> Output {
    Command::new(GRAMMAR)
        .arg("audit")
        .args(args)
        .output()
        .unwrap()
}

/// A content root holding one campaign with one program, complete and green.
fn content_root(dir: &Path, program_json: &str, manifest: Option<&str>) -> PathBuf {
    let programs = dir.join("campaigns/demo/design/programs");
    fs::create_dir_all(&programs).unwrap();
    fs::write(programs.join("zone-a.json"), program_json).unwrap();
    if let Some(m) = manifest {
        fs::write(programs.join("zones.json"), m).unwrap();
    }
    dir.to_path_buf()
}

/// A minimal program that fills a box with a fully specified state.
const GREEN_PROGRAM: &str = r#"{
  "name": "demo",
  "start": "all",
  "rules": { "all": [ { "weight": 1, "body": {
    "op": "split", "axis": "y",
    "sizes": [ {"size":"absolute","blocks":{"expr":"int","value":1}},
               {"size":"relative","weight":{"expr":"int","value":1}} ],
    "children": [ {"op":"fill","material":"minecraft:stone"}, {"op":"void"} ] } } ] }
}"#;

/// The same program painting a wall with no connections written — `DW0735` and
/// `DW0737` both, and the shape a bare palette role takes in a real campaign.
const RED_PROGRAM: &str = r#"{
  "name": "demo",
  "start": "all",
  "rules": { "all": [ { "weight": 1, "body": {
    "op": "split", "axis": "y",
    "sizes": [ {"size":"absolute","blocks":{"expr":"int","value":1}},
               {"size":"relative","weight":{"expr":"int","value":1}} ],
    "children": [ {"op":"fill","material":"minecraft:cobblestone_wall"}, {"op":"void"} ] } } ] }
}"#;

const MANIFEST: &str = r#"{ "zones": [
  { "id": "zone-a", "program": "zone-a.json", "region": [5, 4, 5], "seed": 1 }
] }"#;

// ---------------------------------------------------------------------------
// The three ways this walk could go quietly dark
// ---------------------------------------------------------------------------

/// **A programs directory with no manifest is a red, not a skip.**
///
/// This is the whole point of the manifest. A zone program cannot be expanded
/// without a region, so without this refusal "add a zone program" and "add a
/// zone program nothing will ever check" are the same action — which is exactly
/// how the eight bell zones came to have no caller.
#[test]
fn a_programs_directory_with_no_manifest_is_a_finding() {
    let dir = scratch("no-manifest");
    let root = content_root(&dir, GREEN_PROGRAM, None);
    let out = audit(&["--campaign-root", root.to_str().unwrap()]);
    assert!(!out.status.success(), "{}", combined(&out));
    let text = combined(&out);
    assert!(text.contains("and no manifest"), "{text}");
    assert!(text.contains("zones.json"), "{text}");
}

/// **A program file no manifest entry names is a red.**
///
/// The manifest could otherwise be satisfied by naming one zone of eight, and
/// the other seven would be audited by nothing while the board stayed green.
#[test]
fn a_program_no_manifest_entry_names_is_a_finding() {
    let dir = scratch("unnamed");
    let root = content_root(&dir, GREEN_PROGRAM, Some(MANIFEST));
    fs::write(
        root.join("campaigns/demo/design/programs/zone-b.json"),
        GREEN_PROGRAM,
    )
    .unwrap();
    let out = audit(&["--campaign-root", root.to_str().unwrap()]);
    assert!(!out.status.success(), "{}", combined(&out));
    assert!(
        combined(&out).contains("no entry in zones.json names it"),
        "{}",
        combined(&out)
    );
}

/// **A manifest entry naming a file that is not there is a red.**
#[test]
fn a_manifest_entry_with_no_file_is_a_finding() {
    let dir = scratch("missing-file");
    let root = content_root(&dir, GREEN_PROGRAM, Some(MANIFEST));
    fs::remove_file(root.join("campaigns/demo/design/programs/zone-a.json")).unwrap();
    let out = audit(&["--campaign-root", root.to_str().unwrap()]);
    assert!(!out.status.success(), "{}", combined(&out));
    assert!(combined(&out).contains("zone-a.json"), "{}", combined(&out));
}

/// **An audit that found nothing is a red.**
///
/// A content checkout that silently landed empty — the shape
/// `build-every-campaign.py` guards against for the same reason — would
/// otherwise report a clean board having judged nought.
#[test]
fn an_audit_that_finds_no_programs_is_a_finding() {
    let dir = scratch("empty");
    fs::create_dir_all(dir.join("campaigns")).unwrap();
    let out = audit(&["--campaign-root", dir.to_str().unwrap()]);
    assert!(!out.status.success(), "{}", combined(&out));
    assert!(
        combined(&out).contains("found 0 programs"),
        "{}",
        combined(&out)
    );
}

/// **An audit told to look nowhere refuses.**
#[test]
fn an_audit_with_no_corpus_named_refuses() {
    let out = audit(&[]);
    assert!(!out.status.success());
    assert!(
        combined(&out).contains("an audit of nothing is not a pass"),
        "{}",
        combined(&out)
    );
}

// ---------------------------------------------------------------------------
// The verdict, and the binding counts it must state
// ---------------------------------------------------------------------------

/// A complete manifest over a green program passes, and says what bound.
#[test]
fn a_green_campaign_passes_and_states_every_gates_binding_count() {
    let dir = scratch("green");
    let root = content_root(&dir, GREEN_PROGRAM, Some(MANIFEST));
    let out = audit(&["--campaign-root", root.to_str().unwrap()]);
    assert!(out.status.success(), "{}", combined(&out));
    let text = combined(&out);
    for gate in [
        "blocks-exist",
        "shape-complete",
        "states-complete",
        "oriented-fills",
        "non-empty",
    ] {
        assert!(text.contains(gate), "no `{gate}` line:\n{text}");
    }
    assert!(text.contains("audited 1 program(s)"), "{text}");
}

/// A campaign program that reds does red — the audit is not decorative.
#[test]
fn a_red_campaign_program_reds_the_audit() {
    let dir = scratch("red");
    let root = content_root(&dir, RED_PROGRAM, Some(MANIFEST));
    let out = audit(&["--campaign-root", root.to_str().unwrap()]);
    assert!(!out.status.success(), "{}", combined(&out));
    let text = combined(&out);
    assert!(text.contains("DW0735"), "{text}");
    assert!(text.contains("DW0737"), "{text}");
}

// ---------------------------------------------------------------------------
// The exclusion inverts the assertion; it never removes it
// ---------------------------------------------------------------------------

fn write_exclusions(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("exclusions.json");
    fs::write(&path, body).unwrap();
    path
}

/// A recorded red is held, and the record says what is missing.
#[test]
fn a_recorded_red_is_held_with_its_codes() {
    let dir = scratch("recorded");
    let root = content_root(&dir, RED_PROGRAM, Some(MANIFEST));
    let ex = write_exclusions(
        &dir,
        r#"{ "exclusion": [ { "id": "demo/zone-a",
             "capability_gap": "an oriented state cannot be a palette role",
             "expect_codes": ["DW0735", "DW0737"] } ] }"#,
    );
    let out = audit(&[
        "--campaign-root",
        root.to_str().unwrap(),
        "--exclusions",
        ex.to_str().unwrap(),
    ]);
    assert!(out.status.success(), "{}", combined(&out));
    let text = combined(&out);
    assert!(text.contains("known-red"), "{text}");
    assert!(text.contains("1 of them held known-red"), "{text}");
}

/// **A record that no longer reproduces is a finding.** This is the half that
/// makes the mechanism an inversion rather than a skip: a defect that got fixed
/// must not leave the assertion inverted forever.
#[test]
fn a_recorded_red_that_now_passes_is_a_finding() {
    let dir = scratch("expired");
    let root = content_root(&dir, GREEN_PROGRAM, Some(MANIFEST));
    let ex = write_exclusions(
        &dir,
        r#"{ "exclusion": [ { "id": "demo/zone-a",
             "capability_gap": "an oriented state cannot be a palette role",
             "expect_codes": ["DW0735"] } ] }"#,
    );
    let out = audit(&[
        "--campaign-root",
        root.to_str().unwrap(),
        "--exclusions",
        ex.to_str().unwrap(),
    ]);
    assert!(!out.status.success(), "{}", combined(&out));
    assert!(
        combined(&out).contains("the record has expired"),
        "{}",
        combined(&out)
    );
}

/// **A second defect hiding behind a recorded one is a finding.** The recorded
/// codes are matched as a SET, so one more code is a red — which is the property
/// a plain skip cannot have, because the defect it excuses would supply it.
#[test]
fn a_recorded_red_that_fails_with_another_code_is_a_finding() {
    let dir = scratch("extra-code");
    let root = content_root(&dir, RED_PROGRAM, Some(MANIFEST));
    let ex = write_exclusions(
        &dir,
        r#"{ "exclusion": [ { "id": "demo/zone-a",
             "capability_gap": "an oriented state cannot be a palette role",
             "expect_codes": ["DW0735"] } ] }"#,
    );
    let out = audit(&[
        "--campaign-root",
        root.to_str().unwrap(),
        "--exclusions",
        ex.to_str().unwrap(),
    ]);
    assert!(!out.status.success(), "{}", combined(&out));
    assert!(
        combined(&out).contains("RECORD WRONG"),
        "{}",
        combined(&out)
    );
}

/// **A record naming a program the audit never saw is a finding** — otherwise an
/// inversion outlives the zone it was written for.
#[test]
fn a_stale_record_naming_nothing_is_a_finding() {
    let dir = scratch("stale");
    let root = content_root(&dir, GREEN_PROGRAM, Some(MANIFEST));
    let ex = write_exclusions(
        &dir,
        r#"{ "exclusion": [ { "id": "demo/zone-gone",
             "capability_gap": "an oriented state cannot be a palette role",
             "expect_codes": ["DW0735"] } ] }"#,
    );
    let out = audit(&[
        "--campaign-root",
        root.to_str().unwrap(),
        "--exclusions",
        ex.to_str().unwrap(),
    ]);
    assert!(!out.status.success(), "{}", combined(&out));
    assert!(
        combined(&out).contains("RECORD STALE"),
        "{}",
        combined(&out)
    );
}

/// **An exclusion naming no codes is refused at read time** — it would be
/// satisfied by any failure at all, which is a skip.
#[test]
fn an_exclusion_with_no_expected_codes_is_refused() {
    let dir = scratch("no-codes");
    let root = content_root(&dir, RED_PROGRAM, Some(MANIFEST));
    let ex = write_exclusions(
        &dir,
        r#"{ "exclusion": [ { "id": "demo/zone-a",
             "capability_gap": "something", "expect_codes": [] } ] }"#,
    );
    let out = audit(&[
        "--campaign-root",
        root.to_str().unwrap(),
        "--exclusions",
        ex.to_str().unwrap(),
    ]);
    assert!(!out.status.success());
    assert!(
        combined(&out).contains("names no expect_codes"),
        "{}",
        combined(&out)
    );
}

/// **An exclusion naming no capability gap is refused.** A record exists to say
/// what is MISSING; one that says nothing is a permission slip.
#[test]
fn an_exclusion_with_no_capability_gap_is_refused() {
    let dir = scratch("no-gap");
    let root = content_root(&dir, RED_PROGRAM, Some(MANIFEST));
    let ex = write_exclusions(
        &dir,
        r#"{ "exclusion": [ { "id": "demo/zone-a",
             "capability_gap": "  ", "expect_codes": ["DW0735"] } ] }"#,
    );
    let out = audit(&[
        "--campaign-root",
        root.to_str().unwrap(),
        "--exclusions",
        ex.to_str().unwrap(),
    ]);
    assert!(!out.status.success());
    assert!(
        combined(&out).contains("names no capability gap"),
        "{}",
        combined(&out)
    );
}

// ---------------------------------------------------------------------------
// The live corpora
// ---------------------------------------------------------------------------

/// **The rule library passes its own audit**, and the run states a binding count
/// per gate over all thirty-three programs.
#[test]
fn the_rule_library_passes_its_own_audit() {
    let out = audit(&["--library"]);
    assert!(out.status.success(), "{}", combined(&out));
    let text = combined(&out);
    assert!(text.contains("audited 33 program(s)"), "{text}");
    // No gate may report a zero binding over the whole corpus.
    for line in text.lines() {
        assert!(
            !line.contains("zero binding"),
            "a gate bound to nothing:\n{text}"
        );
    }
}
