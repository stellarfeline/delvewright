//! **A campaign part-way through being written is refused, not crashed at**
//! (`DW0874`).
//!
//! A campaign directory missing one of the six stage documents used to be
//! `internal error: cannot read campaign dir: npcs.json`, exit 10, no `DW` code —
//! the phrasing this compiler reserves for its own bugs, printed at the state the
//! authoring skill tells an author to be in on purpose.
//!
//! Three things are asserted here and each was a way the old shape could come
//! back.
//!
//! 1. **The population**, taken from
//!    [`delvewright_compiler::load::STAGE_FILES`] rather than written out, so a
//!    seventh required document cannot join the six and quietly keep the old
//!    behaviour. Binding count is stated by the test itself: six of six.
//! 2. **Every entry point.** Four verbs read a campaign directory and each
//!    carried its own copy of the old error arm. A rule bound at `validate`
//!    alone would leave the other three saying `internal error` about the same
//!    state, which is the UNRUN shape one call site in.
//! 3. **What must NOT move.** An unreadable document and a path that is not a
//!    campaign directory are genuinely conditions this process should stop hard
//!    on, and both still exit 10 with no code. `DW0874` is a claim that the
//!    directory is there and incomplete; making it fire for a mistyped path
//!    would be a true sentence about a campaign that does not exist.

mod common;

use std::path::Path;
use std::process::{Command, Output};

use delvewright_compiler::load::{
    OPTIONAL_FILES, STAGE_FILES, missing_stage_documents, missing_stage_documents_diagnostic,
};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn run(args: &[&str]) -> Output {
    Command::new(BIN).args(args).output().expect("run delvec")
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn exit(out: &Output) -> i32 {
    out.status.code().unwrap_or(-1)
}

/// hello-world at `dst`, minus the documents named.
fn campaign_without(tag: &str, drop: &[&str]) -> std::path::PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("missing-stage-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(&common::hello_world_dir(), &dir);
    for f in drop {
        std::fs::remove_file(dir.join(f)).unwrap_or_else(|e| panic!("remove {f}: {e}"));
    }
    dir
}

fn prefabs() -> String {
    common::prefabs_dir().display().to_string()
}

/// **Every one of the six, and the count is computed from the six.**
///
/// The old behaviour was uniform over the population — the loader reads in
/// document order and returns the first `NotFound` — so a repair that fixed the
/// document somebody happened to try would look identical on that one document
/// and identical to the old shape on the other five.
#[test]
fn each_of_the_six_stage_documents_is_dw0874_at_exit_1() {
    let mut bound = 0;
    for f in STAGE_FILES {
        let dir = campaign_without(f.trim_end_matches(".json"), &[f]);
        let out = run(&[
            "validate",
            dir.to_str().unwrap(),
            "--prefabs",
            &prefabs(),
            "--json",
        ]);
        let s = text(&out);
        assert_eq!(exit(&out), 1, "{f}: expected the validation tier\n{s}");
        assert!(s.contains("DW0874"), "{f}: {s}");
        assert!(
            !s.contains("internal error"),
            "{f}: an authoring state must not be reported as a compiler fault\n{s}"
        );
        bound += 1;
    }
    assert_eq!(
        bound,
        STAGE_FILES.len(),
        "binding count: every stage document must have been examined"
    );
}

/// The message names **all** of them, not the first one the loader tripped on.
///
/// Before this, an author starting from `world.json` alone learned the remaining
/// five filenames by running `validate` five more times, one filename per run.
#[test]
fn the_refusal_names_every_missing_document_at_once() {
    let dir = campaign_without("all-but-world", &STAGE_FILES[1..]);
    let d = missing_stage_documents_diagnostic(&dir).expect("five documents are missing");
    assert_eq!(d.code, "DW0874");
    assert_eq!(missing_stage_documents(&dir).len(), STAGE_FILES.len() - 1);
    for f in &STAGE_FILES[1..] {
        assert!(d.message.contains(f), "must name `{f}`: {}", d.message);
    }
    // And the whole six, so the author learns the set rather than the remainder.
    assert!(d.message.contains("world.json"), "{}", d.message);
    // The remedy has to be runnable as written: a stub, and the command that
    // prints the shape of the document being stubbed.
    assert!(
        d.message.contains("delvec schema --stage"),
        "{}",
        d.message
    );
    // The optional documents are named as optional, so their absence is not
    // mistaken for the next thing owed.
    for f in OPTIONAL_FILES {
        assert!(d.message.contains(f), "must name `{f}` as optional: {}", d.message);
    }
}

/// **Every entry point.** `validate` is one of four verbs that read a campaign
/// directory, and the old uncoded error was written out separately at all four.
#[test]
fn every_verb_that_reads_a_campaign_directory_raises_it() {
    let dir = campaign_without("entry-points", &["npcs.json"]);
    let p = dir.to_str().unwrap().to_string();
    let pf = prefabs();
    let shots = dir.join("edit-shots").display().to_string();
    let invocations: [Vec<&str>; 6] = [
        vec!["validate", &p, "--prefabs", &pf],
        vec!["analyze", &p, "--prefabs", &pf],
        vec!["build", &p, "--prefabs", &pf, "--out", &shots],
        vec!["l10n-inventory", &p],
        vec!["allocation", &p, "--all"],
        vec!["edit", "preview", &p, "--prefabs", &pf, "--out", &shots],
    ];
    let mut bound = 0;
    for args in &invocations {
        let out = run(args);
        let s = text(&out);
        assert!(s.contains("DW0874"), "{args:?}: {s}");
        assert!(!s.contains("internal error"), "{args:?}: {s}");
        assert_eq!(exit(&out), 1, "{args:?}: {s}");
        bound += 1;
    }
    assert_eq!(bound, invocations.len(), "binding count over the verbs");
}

/// **What must not move.** A document that is there and cannot be read is not an
/// authoring state, and a probe built on `is_file()` would have called it absent.
#[test]
fn an_unreadable_document_is_still_an_internal_error() {
    let dir = campaign_without("unreadable", &["npcs.json"]);
    std::fs::create_dir(dir.join("npcs.json")).unwrap();
    assert!(
        missing_stage_documents(&dir).is_empty(),
        "a directory standing in a document's place is present, not absent"
    );
    let out = run(&["validate", dir.to_str().unwrap(), "--prefabs", &prefabs()]);
    let s = text(&out);
    assert_eq!(exit(&out), 10, "{s}");
    assert!(s.contains("internal error"), "{s}");
    assert!(!s.contains("DW0874"), "{s}");
}

/// A path that is not a campaign directory has six absent stage documents by
/// arithmetic and nothing an author can do about it. Refusing it as an
/// incomplete campaign would be a true sentence about a campaign that is not
/// there, and a remedy (write six documents) that does nothing.
#[test]
fn a_path_that_is_not_a_campaign_directory_raises_nothing_here() {
    let missing = Path::new(env!("CARGO_TARGET_TMPDIR")).join("missing-stage-no-such-dir");
    let _ = std::fs::remove_dir_all(&missing);
    assert!(missing_stage_documents(&missing).is_empty());
    assert!(missing_stage_documents_diagnostic(&missing).is_none());

    let file = common::hello_world_dir().join("world.json");
    assert!(missing_stage_documents(&file).is_empty());
    assert!(missing_stage_documents_diagnostic(&file).is_none());

    let out = run(&[
        "validate",
        missing.to_str().unwrap(),
        "--prefabs",
        &prefabs(),
    ]);
    assert_eq!(exit(&out), 10);
    assert!(text(&out).contains("internal error"));
}

/// A complete campaign directory answers with nothing, so the rule cannot start
/// charging a campaign that has all six.
#[test]
fn a_complete_campaign_directory_raises_nothing() {
    let dir = common::hello_world_dir();
    assert!(missing_stage_documents(&dir).is_empty());
    assert!(missing_stage_documents_diagnostic(&dir).is_none());
}
