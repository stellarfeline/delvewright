//! **A campaign document that cannot be read names itself.**
//!
//! The loader reads twelve documents plus any `l10n/` sidecar, and every one of
//! them used to fail with a bare `No such file or directory (os error 2)` that
//! named nothing: the author of a six-document campaign missing one of them was
//! told only that *something* under the directory could not be read, and had to
//! guess which. Worse, an absent directory and a directory missing one document
//! produced the **byte-identical** message.
//!
//! These tests bind the two halves of the repair, and they are written so that
//! nothing except the repair can satisfy them:
//!
//! * they call [`load_campaign_dir`] directly, so the message under assertion is
//!   the return value of the one function being repaired — no other layer is in
//!   the picture and no other mechanism can produce the red;
//! * they assert the failing name is present **and that the names of the
//!   documents which are fine are absent**, so a "fix" that lists every
//!   filename it might have read does not pass;
//! * they assert the [`std::io::ErrorKind`] survives, so *absent* and
//!   *unreadable* stay different findings.

mod common;

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use delvewright_compiler::load::{
    DETAIL_PLAN_FILE, GEOMETRY_BRIEF_FILE, LAYOUT_GRAPH_FILE, LoadedCampaign, SITE_PLAN_FILE,
    STAGE_FILES, WALK_RECORD_FILE, WORLD_EDITS_FILE, load_campaign_dir,
};

/// Every optional stage document, in load order — the siblings of
/// [`STAGE_FILES`] whose absence is deliberately not an error.
const OPTIONAL_FILES: [&str; 5] = [
    WORLD_EDITS_FILE,
    GEOMETRY_BRIEF_FILE,
    LAYOUT_GRAPH_FILE,
    SITE_PLAN_FILE,
    DETAIL_PLAN_FILE,
];

/// **The binding count of this path**: every document name the loader can put
/// in a message. Six required + five optional + the walk record. The `l10n/`
/// sidecars are unbounded in principle and are bound separately below.
const NAMEABLE: usize = STAGE_FILES.len() + OPTIONAL_FILES.len() + 1;

/// `Result::expect_err` needs `T: Debug`, and [`LoadedCampaign`] is not a
/// debuggable type — deriving one on a production struct to suit a test would be
/// the tail wagging the dog. This says the same thing and costs nothing.
fn err_of(r: std::io::Result<LoadedCampaign>, what: &str) -> std::io::Error {
    match r {
        Ok(_) => panic!("{what}"),
        Err(e) => e,
    }
}

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("load-names-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A complete, loadable copy of hello-world at a fresh temp path.
fn campaign(name: &str) -> PathBuf {
    let dir = tmp(name);
    for f in STAGE_FILES {
        std::fs::copy(common::hello_world_dir().join(f), dir.join(f)).unwrap();
    }
    common::copy_l10n_dir(&common::hello_world_dir(), &dir);
    dir
}

/// Put a **directory** where a document belongs. Root-immune, unlike `chmod
/// 000`: it makes the read fail for a reason that is not `NotFound` on every
/// platform and under every uid, which is what pins "absent and unreadable are
/// different findings".
fn shadow_with_dir(path: &Path) {
    let _ = std::fs::remove_file(path);
    std::fs::create_dir_all(path).unwrap();
}

/// The whole campaign loads before anything is perturbed — otherwise a red
/// below could be about the fixture rather than about the repair.
#[test]
fn the_unperturbed_fixture_loads() {
    let dir = campaign("baseline");
    assert!(load_campaign_dir(&dir).is_ok(), "fixture must load clean");
}

#[test]
fn a_missing_required_document_names_itself_and_only_itself() {
    for missing in STAGE_FILES {
        let dir = campaign(&format!("missing-{missing}"));
        std::fs::remove_file(dir.join(missing)).unwrap();

        let err = err_of(
            load_campaign_dir(&dir),
            "a missing stage document must fail",
        );
        let msg = err.to_string();

        assert!(
            msg.contains(missing),
            "removing {missing} must produce a message naming it, got: {msg}"
        );
        assert_eq!(
            err.kind(),
            ErrorKind::NotFound,
            "an absent document is NotFound, not flattened: {msg}"
        );
        // The five that are still there must not be named. A message that
        // listed every filename the loader might read would satisfy the
        // assertion above and answer nothing.
        for present in STAGE_FILES.iter().filter(|f| **f != missing) {
            assert!(
                !msg.contains(present),
                "{missing} is the missing one, but the message also names {present}: {msg}"
            );
        }
    }
}

#[test]
fn an_unreadable_required_document_names_itself_and_keeps_its_kind() {
    for broken in STAGE_FILES {
        let dir = campaign(&format!("unreadable-{broken}"));
        shadow_with_dir(&dir.join(broken));

        let err = err_of(
            load_campaign_dir(&dir),
            "an unreadable stage document must fail",
        );
        let msg = err.to_string();

        assert!(msg.contains(broken), "message must name {broken}: {msg}");
        assert_ne!(
            err.kind(),
            ErrorKind::NotFound,
            "unreadable is not absent — the kind must not be flattened: {msg}"
        );
    }
}

/// An optional document that is **there and cannot be read** is a finding, not
/// an absence. Reading it and treating only `NotFound` as absence is what makes
/// that true; probing with `is_file()` first reports "absent" for anything whose
/// metadata does not resolve to a regular file, and a build then silently ships
/// a campaign missing a stage document, byte-identical to one that never had it.
#[test]
fn an_unreadable_optional_document_is_a_named_finding_not_an_absence() {
    for opt in OPTIONAL_FILES {
        let dir = campaign(&format!("opt-{opt}"));
        shadow_with_dir(&dir.join(opt));

        let err = err_of(
            load_campaign_dir(&dir),
            "an optional document that exists and cannot be read must fail",
        );
        let msg = err.to_string();

        assert!(msg.contains(opt), "message must name {opt}: {msg}");
        assert_ne!(err.kind(), ErrorKind::NotFound, "{msg}");
    }
}

/// The complement, and it is what keeps the test above honest: a *genuinely*
/// absent optional document is still not an error.
#[test]
fn an_absent_optional_document_is_still_not_an_error() {
    let dir = campaign("opt-absent");
    for opt in OPTIONAL_FILES {
        assert!(!dir.join(opt).exists());
    }
    assert!(!dir.join(WALK_RECORD_FILE).exists());
    let loaded = load_campaign_dir(&dir).expect("a campaign with no optional documents loads");
    assert!(loaded.raw.world_edits.is_none());
    assert!(loaded.walk_record.is_none());
}

#[test]
fn an_unreadable_walk_record_names_itself() {
    let dir = campaign("walk-record");
    shadow_with_dir(&dir.join(WALK_RECORD_FILE));

    let err = err_of(
        load_campaign_dir(&dir),
        "an unreadable walk record must fail",
    );
    let msg = err.to_string();
    assert!(msg.contains(WALK_RECORD_FILE), "must name it: {msg}");
    assert_ne!(err.kind(), ErrorKind::NotFound, "{msg}");
}

#[test]
fn an_unreadable_l10n_sidecar_names_itself() {
    let dir = campaign("l10n");
    let l10n = dir.join("l10n");
    std::fs::create_dir_all(&l10n).unwrap();
    shadow_with_dir(&l10n.join("zh.json"));

    let err = err_of(load_campaign_dir(&dir), "an unreadable sidecar must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("l10n/zh.json"),
        "must name the sidecar the way the manifest keys it: {msg}"
    );
}

/// **The half the repair itself would otherwise break.** Naming the document
/// makes an absent *directory* report a missing `world.json` — sending the
/// author after one file inside a directory that is not there. So the directory
/// is established before any document is read, and the two findings say
/// different things.
#[test]
fn an_absent_campaign_directory_names_the_directory_not_a_document() {
    let dir = tmp("absent").join("no-such-campaign");
    assert!(!dir.exists());

    let err = err_of(
        load_campaign_dir(&dir),
        "an absent campaign directory must fail",
    );
    let msg = err.to_string();

    assert!(
        msg.contains("no-such-campaign"),
        "must name the directory: {msg}"
    );
    assert_eq!(err.kind(), ErrorKind::NotFound, "{msg}");
    for f in STAGE_FILES {
        assert!(
            !msg.contains(f),
            "an absent directory is not a missing {f}: {msg}"
        );
    }
}

/// A path that exists but is a file rather than a directory is its own finding,
/// and it too must not read as a missing `world.json`.
#[test]
fn a_campaign_path_that_is_not_a_directory_says_so() {
    let dir = tmp("not-a-dir");
    let path = dir.join("campaign");
    std::fs::write(&path, b"not a campaign").unwrap();

    let err = err_of(
        load_campaign_dir(&path),
        "a non-directory campaign path must fail",
    );
    let msg = err.to_string();
    assert!(msg.contains("campaign"), "must name the path: {msg}");
    for f in STAGE_FILES {
        assert!(!msg.contains(f), "{msg}");
    }
}

/// **State the binding count** (CLAUDE.md: a green gate that binds to nothing is
/// vacuous, not a pass) — and state it as a *measurement*, not as a constant.
///
/// Every one of the twelve documents is perturbed here and the ones whose
/// message names them are counted, so the number printed is what the loader
/// actually did rather than what this file asserts it should do. A constant
/// compared against itself would be green on a loader that names nothing.
#[test]
fn the_binding_count_is_measured_and_is_not_zero() {
    let mut named_itself = Vec::new();
    for doc in STAGE_FILES.iter().chain(&OPTIONAL_FILES) {
        let dir = campaign(&format!("count-{doc}"));
        shadow_with_dir(&dir.join(doc));
        if let Err(e) = load_campaign_dir(&dir)
            && e.to_string().contains(*doc)
        {
            named_itself.push(*doc);
        }
    }
    let dir = campaign("count-walk-record");
    shadow_with_dir(&dir.join(WALK_RECORD_FILE));
    if let Err(e) = load_campaign_dir(&dir)
        && e.to_string().contains(WALK_RECORD_FILE)
    {
        named_itself.push(WALK_RECORD_FILE);
    }

    eprintln!(
        "binding count: {} of {NAMEABLE} campaign documents named themselves — {}",
        named_itself.len(),
        named_itself.join(", ")
    );
    assert_eq!(
        named_itself.len(),
        NAMEABLE,
        "every document this loader reads must be able to name itself; \
         the ones that did were {named_itself:?}"
    );
}
