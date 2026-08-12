//! `delve-grammar expand`, driven the way an author drives it.
//!
//! The property under test is about ORDER, so it cannot be tested through the
//! library: `export_prefab` has always refused an unusable id. What shipped was
//! a CLI that refused it *after* printing a complete gate report headed
//! `<id>: pass` and writing `<id>.report.json`, then exited non-zero with no
//! `.nbt`. A verdict a reader stops at, above a failure, is worse than no
//! verdict.
//!
//! Two claims, and the second is the general one:
//!
//! 1. An input that cannot produce an artifact is refused **before** any work
//!    whose output could be mistaken for success — nothing expanded, nothing
//!    written, nothing printed but the refusal.
//! 2. **No `pass` verdict may precede a non-zero exit**, whatever the cause.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use delvewright_grammar::Program;
use delvewright_grammar::block::BlockState;
use delvewright_grammar::ir::{Material, Node};
use delvewright_grammar::library::store_room;

const GRAMMAR: &str = env!("CARGO_BIN_EXE_delve-grammar");

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-grammar-cli-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Write `program` to `<dir>/<filename>` — the filename matters, because it is
/// where the default prefab id comes from.
fn program_file(dir: &Path, filename: &str, program: &Program) -> PathBuf {
    let path = dir.join(filename);
    std::fs::write(&path, serde_json::to_vec_pretty(program).unwrap()).unwrap();
    path
}

fn expand(file: &Path, out: &Path, extra: &[&str]) -> Output {
    Command::new(GRAMMAR)
        .args(["expand", "--file"])
        .arg(file)
        .args(["--region", "7x5x14", "--seed", "3"])
        .args(extra)
        .arg("-o")
        .arg(out)
        .output()
        .unwrap()
}

fn combined(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// Nothing that looks like a verdict of success may appear.
fn assert_no_pass_verdict(out: &Output) {
    let text = combined(out);
    for line in text.lines() {
        assert!(
            !line.ends_with(": pass"),
            "a `pass` verdict was printed above a non-zero exit:\n{text}"
        );
        assert!(
            !line.contains("  pass  "),
            "a passing gate line was printed above a non-zero exit:\n{text}"
        );
    }
}

/// The reproduction: a program in `var-B.json`. The id defaults to the file
/// stem, and `var-B` is not a usable structure id.
#[test]
fn an_unusable_id_is_refused_before_anything_is_expanded_or_written() {
    let dir = scratch("bad-id");
    let out_dir = dir.join("out");
    let file = program_file(&dir, "var-B.json", &store_room());

    let out = expand(&file, &out_dir, &[]);

    assert_eq!(out.status.code(), Some(2), "{}", combined(&out));
    assert_no_pass_verdict(&out);

    let text = combined(&out);
    assert!(
        text.contains("not a usable structure id"),
        "the refusal must say what is wrong:\n{text}"
    );
    // ...and where the id came from, which no document the author read mentions.
    assert!(
        text.contains("the stem of the input filename \"var-B.json\""),
        "the refusal must name where the id came from:\n{text}"
    );
    assert!(
        text.contains("--id"),
        "the refusal must name the way out:\n{text}"
    );

    assert!(
        !out_dir.exists() || std::fs::read_dir(&out_dir).unwrap().next().is_none(),
        "a refused expansion must leave no report and no prefab: {:?}",
        std::fs::read_dir(&out_dir).map(|d| d.count())
    );
}

/// The same program under a usable name produces the artifact — so the refusal
/// above is about the id and nothing else.
#[test]
fn the_same_program_under_a_usable_id_writes_the_prefab() {
    let dir = scratch("good-id");
    let out_dir = dir.join("out");
    let file = program_file(&dir, "var-B.json", &store_room());

    let out = expand(&file, &out_dir, &["--id", "var-b"]);

    assert!(out.status.success(), "{}", combined(&out));
    assert!(out_dir.join("var-b.nbt").is_file());
    assert!(out_dir.join("var-b.json").is_file());
    assert!(out_dir.join("var-b.report.json").is_file());
    // The provenance row reaches the file the operator ends up with.
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out_dir.join("var-b.json")).unwrap())
            .unwrap();
    assert_eq!(meta["license"]["generated_by"]["seed"], 3);
    assert!(
        meta["license"]["generated_by"]["program_hash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
}

/// An `--id` the author supplied by hand is refused the same way, and the
/// refusal names `--id` rather than the filename it did not come from.
#[test]
fn an_explicit_bad_id_names_the_flag_it_came_from() {
    let dir = scratch("bad-flag-id");
    let out_dir = dir.join("out");
    let file = program_file(&dir, "store-room.json", &store_room());

    let out = expand(&file, &out_dir, &["--id", "Store_Room"]);

    assert_eq!(out.status.code(), Some(2), "{}", combined(&out));
    assert_no_pass_verdict(&out);
    let text = combined(&out);
    assert!(text.contains("the id came from --id"), "{text}");
    assert!(!out_dir.exists() || std::fs::read_dir(&out_dir).unwrap().next().is_none());
}

/// The general claim, on the one failure that is NOT an input property: every
/// gate passes and freezing still refuses, because a structure template may not
/// carry a command block. The refusal must not wear a `pass`.
#[test]
fn a_freeze_refusal_after_green_gates_prints_no_pass_verdict() {
    let dir = scratch("forbidden");
    let out_dir = dir.join("out");
    let program = Program::new("armed", "all").rule(
        "all",
        Node::Fill {
            material: Material::block(BlockState::simple("minecraft:command_block")),
        },
    );
    let file = program_file(&dir, "armed.json", &program);

    let out = expand(&file, &out_dir, &[]);

    assert_ne!(out.status.code(), Some(0), "{}", combined(&out));
    assert_no_pass_verdict(&out);
    let text = combined(&out);
    assert!(text.contains("command_block"), "{text}");
    assert!(
        text.contains("this refusal is not a gate"),
        "the reader must be told the gates are not what refused:\n{text}"
    );
    assert!(
        !out_dir.join("armed.nbt").exists(),
        "no prefab may be written"
    );
}

/// A region no structure template can hold is an input property too: refused
/// before the expansion, not after it.
#[test]
fn an_oversize_region_is_refused_before_expanding_it() {
    let dir = scratch("oversize");
    let out_dir = dir.join("out");
    let file = program_file(&dir, "store-room.json", &store_room());

    let out = Command::new(GRAMMAR)
        .args(["expand", "--file"])
        .arg(&file)
        .args(["--region", "49x5x14", "--seed", "3"])
        .arg("-o")
        .arg(&out_dir)
        .output()
        .unwrap();

    assert_eq!(out.status.code(), Some(2), "{}", combined(&out));
    assert_no_pass_verdict(&out);
    assert!(combined(&out).contains("caps every axis at 48"));
    assert!(!out_dir.exists() || std::fs::read_dir(&out_dir).unwrap().next().is_none());
}
