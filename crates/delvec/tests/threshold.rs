//! **A trigger the path relies on cannot be walked past** (`DW1044`).
//!
//! The fixture is the chapel-door shape (`common::threshold`): the keep's
//! two-wide doorway, opened by the Keeper, with an `approach` trigger on the
//! doorway's west column that pays the flag the exit objective reads. The bot
//! walks onto the anchor cell; a player walks through the east column.
//!
//! * range 1 — refused with `DW1044`, naming the trigger, the flag, the reader
//!   and the least range every walk enters (2);
//! * range 2 — builds, and the binding line says the one judged trigger is
//!   entered by every walk;
//! * range 2 with a second doorway carved through the same wall — refused
//!   again, with every other proof green: the way round is a door, and only
//!   this proof sees it.
//!
//! The remedy rows (widen the range, move the anchor into the only way
//! through, close the way round) are in `remedy_reachability.rs`.

mod common;

use common::threshold::{approach, doorway_campaign, second_door};
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("threshold-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn build(tag: &str, on: serde_json::Value, edits: Option<Vec<serde_json::Value>>) -> (i32, String) {
    let camp = tmp(&format!("camp-{tag}"));
    doorway_campaign(&camp, on, "anchor/door", edits);
    let out = tmp(&format!("out-{tag}"));
    let r = Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args(["build", camp.to_str().unwrap(), "-o", out.to_str().unwrap()])
        .arg("--prefabs")
        .arg(common::prefabs_dir())
        .output()
        .unwrap();
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
    (r.status.code().unwrap_or(-1), log)
}

#[test]
fn a_range_one_trigger_in_a_two_wide_doorway_is_refused() {
    let (code, log) = build("r1", approach(1), None);
    assert_ne!(code, 0, "{log}");
    assert!(log.contains("DW1044 [error]"), "{log}");
    assert!(
        log.lines()
            .filter(|l| l.contains("[error]"))
            .all(|l| l.starts_with("DW1044")),
        "the one refusal is this proof's, every other proof green:\n{log}"
    );
    assert!(
        log.contains(
            "trigger `trigger/through-the-door` (approach) pays `flag/through-the-door`, which \
             `obj/exit` reads"
        ),
        "{log}"
    );
    assert!(
        log.contains("widen `range` from 1 to 2, the least range every walk enters"),
        "{log}"
    );
    assert!(
        log.contains("move the anchor into the only way through"),
        "{log}"
    );
    assert!(log.contains("close the way round it"), "{log}");
    assert!(
        log.contains(
            "DW1044 binding: 1 trigger(s) performed by the critical path; 1 judged (walk-fired, \
             paying a flag a later step reads), 0 entered by every walk"
        ),
        "{log}"
    );
}

#[test]
fn the_range_the_refusal_names_builds() {
    let (code, log) = build("r2", approach(2), None);
    assert_eq!(code, 0, "{log}");
    assert!(!log.contains("DW1044 [error]"), "{log}");
    assert!(
        log.contains(
            "DW1044 binding: 1 trigger(s) performed by the critical path; 1 judged (walk-fired, \
             paying a flag a later step reads), 1 entered by every walk to the step that needs it"
        ),
        "{log}"
    );
    // The ledger states the line's counts, `judged` also as `examined` — the key
    // the gallery's coverage gate refuses at zero.
    let ledger: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_TARGET_TMPDIR"))
                .join("threshold-out-r2")
                .join("validation/threshold-gate.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        ledger,
        serde_json::json!({
            "code": "DW1044", "performed": 1, "judged": 1, "examined": 1, "entered": 1,
            "no_debt": 0, "by_hand": 0, "carried": 0
        })
    );
}

/// The perturbation only this proof catches: a second doorway round the
/// trigger. The walk proof routes either door; this one is refused.
#[test]
fn a_second_door_round_the_widened_trigger_is_refused() {
    let (code, log) = build("r2-door", approach(2), Some(second_door()));
    assert_ne!(code, 0, "{log}");
    assert!(log.contains("DW1044 [error]"), "{log}");
    assert!(log.contains("DW0311 binding: 3 leg(s); 3 walked"), "{log}");
    assert!(log.contains("0 entered by every walk"), "{log}");
}
