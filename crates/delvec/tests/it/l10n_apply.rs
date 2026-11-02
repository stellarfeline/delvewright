//! `delvec l10n-apply` — the keys are the tool's (spec-0071 §4).
//!
//! The defect: an agent that is itself the translator has to address inventory
//! rows by key, and the effect keys are POSITIONAL
//! (`fx.<quest>.oc.<objective>.<i>`). Insert one effect and every sibling key
//! shifts, so the translations re-attach one line over — present, applied, and
//! wrong, with no key missing for coverage to notice. The tool's contract is
//! that the agent hands over English → translation and never touches a key, so
//! the renumbering is the tool's problem and stops being anybody's.
//!
//! Every assertion here runs the real binary over a real campaign directory and
//! reads the file it wrote: the question is what lands on disk.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");
const LANG: &str = "zh-cn";

/// A private copy of the keep-trial fixture — the valid campaign that declares a
/// language and ships a sidecar with provenance.
fn campaign(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(&common::keep_trial_dir(), &dir);
    dir
}

fn inventory(dir: &Path) -> BTreeMap<String, String> {
    let out = Command::new(BIN)
        .args(["--lang", LANG, "l10n-inventory"])
        .arg(dir)
        .output()
        .expect("run l10n-inventory");
    assert!(out.status.success(), "l10n-inventory failed");
    let doc: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("the inventory is JSON");
    doc["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .map(|e| {
            (
                e["key"].as_str().unwrap().to_string(),
                e["en"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

/// The sidecar as it stands on disk.
fn sidecar(dir: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(dir.join(format!("l10n/{LANG}.json"))).unwrap())
        .expect("the sidecar is JSON")
}

/// A complete table: every English string in the inventory, translated by
/// bracketing it. A real translation is not what is under test — the keys are.
fn full_table(dir: &Path, path: &Path) {
    let table: BTreeMap<String, String> = inventory(dir)
        .into_values()
        .map(|en| (en.clone(), format!("〔{en}〕")))
        .collect();
    std::fs::write(path, serde_json::to_string_pretty(&table).unwrap()).unwrap();
}

fn apply(dir: &Path, table: &Path) -> Output {
    Command::new(BIN)
        .args(["--lang", LANG, "l10n-apply"])
        .arg(dir)
        .arg("--table")
        .arg(table)
        .output()
        .expect("run l10n-apply")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// A complete table yields a sidecar `delvec validate` accepts with no missing
/// key, and the run states the count it covered.
#[test]
fn a_complete_table_yields_a_sidecar_validate_accepts() {
    let dir = campaign("l10n-apply-complete");
    let table = dir.join("table.json");
    full_table(&dir, &table);
    let out = apply(&dir, &table);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let inv = inventory(&dir);
    assert!(
        stdout(&out).contains(&format!("{} of {} rows translated", inv.len(), inv.len())),
        "{}",
        stdout(&out)
    );

    let validated = Command::new(BIN)
        .args(["--lang", LANG, "--prefabs"])
        .arg(common::prefabs_dir())
        .arg("validate")
        .arg(&dir)
        .output()
        .expect("run validate");
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&validated.stdout),
        String::from_utf8_lossy(&validated.stderr)
    );
    for code in ["DW0180", "DW0181", "DW0187", "DW0188"] {
        assert!(!log.contains(code), "{code} in:\n{log}");
    }

    // Every row carries the English it was made from — which is what makes a
    // later edit of that English detectable rather than audited.
    let doc = sidecar(&dir);
    assert_eq!(
        doc["content"].as_object().unwrap().len(),
        doc["source"].as_object().unwrap().len(),
        "every written row records its source"
    );
}

/// Put `lines` into `quest/trial`'s `obj/slay` bundle as narrate effects, in
/// order — the shape whose l10n keys are positional
/// (`fx.trial.oc.slay.<i>.narrate`).
fn narrate_bundle(dir: &Path, lines: &[&str]) {
    common::patch_file(&dir.join("quests.json"), |quests| {
        for q in quests["content"]["quests"].as_array_mut().unwrap() {
            if q["id"] != "quest/trial" {
                continue;
            }
            let bundle = q["on_objective_complete"]["obj/slay"]
                .as_array_mut()
                .unwrap();
            for (i, line) in lines.iter().enumerate() {
                bundle.insert(i, serde_json::json!({ "type": "narrate", "text": line }));
            }
        }
    });
}

/// **The defect this verb exists for.** An effect inserted ahead of an existing
/// one renumbers every sibling key; the same table, with one entry added for the
/// new line and nothing else touched, puts each translation back on the line
/// whose English it was made from.
#[test]
fn an_insertion_renumbers_the_keys_and_the_translations_follow() {
    let dir = campaign("l10n-apply-insert");
    narrate_bundle(&dir, &["The first line.", "The second line."]);
    let table = dir.join("table.json");
    full_table(&dir, &table);
    assert_eq!(apply(&dir, &table).status.code(), Some(0));

    let key_of = |dir: &Path, en: &str| -> String {
        inventory(dir)
            .into_iter()
            .find(|(_, v)| v == en)
            .map(|(k, _)| k)
            .unwrap_or_else(|| panic!("`{en}` is inventoried"))
    };
    let second_before = key_of(&dir, "The second line.");
    let was = sidecar(&dir)["content"][&second_before]
        .as_str()
        .expect("translated")
        .to_string();
    assert!(second_before.starts_with("fx."), "{second_before}");

    narrate_bundle(&dir, &["A line inserted ahead of the others."]);
    let mut t: BTreeMap<String, String> =
        serde_json::from_str(&std::fs::read_to_string(&table).unwrap()).unwrap();
    t.insert(
        "A line inserted ahead of the others.".to_string(),
        "〔inserted〕".to_string(),
    );
    std::fs::write(&table, serde_json::to_string_pretty(&t).unwrap()).unwrap();

    let out = apply(&dir, &table);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let second_after = key_of(&dir, "The second line.");
    assert_ne!(
        second_after, second_before,
        "the insertion renumbered the key"
    );
    assert_eq!(
        sidecar(&dir)["content"][&second_after].as_str(),
        Some(was.as_str()),
        "the translation followed its English onto the new key"
    );
}

/// A table entry that matches no row, and a row the table does not carry, are
/// each printed — and the run says so with a non-zero exit, the same signal
/// `fmt --check` gives.
#[test]
fn an_unmatched_entry_and_a_missing_row_are_both_listed() {
    let dir = campaign("l10n-apply-gaps");
    let table = dir.join("table.json");
    let inv = inventory(&dir);
    let mut t: BTreeMap<String, String> = inv
        .values()
        .map(|en| (en.clone(), format!("〔{en}〕")))
        .collect();
    // One row dropped from the table, one entry that is nobody's English.
    let dropped = inv.values().next().unwrap().clone();
    t.remove(&dropped);
    // …and it is not already in the sidecar either: a row translated from
    // unchanged English is KEPT, which is the tool working, not a gap.
    let side = dir.join(format!("l10n/{LANG}.json"));
    let mut doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&side).unwrap()).unwrap();
    let key = inv
        .iter()
        .find(|(_, en)| **en == dropped)
        .map(|(k, _)| k.clone())
        .unwrap();
    doc["content"].as_object_mut().unwrap().remove(&key);
    doc["source"].as_object_mut().unwrap().remove(&key);
    std::fs::write(&side, serde_json::to_string_pretty(&doc).unwrap()).unwrap();
    t.insert("Nothing in this campaign says this.".into(), "〔x〕".into());
    std::fs::write(&table, serde_json::to_string_pretty(&t).unwrap()).unwrap();

    let out = apply(&dir, &table);
    let log = stdout(&out);
    assert_eq!(out.status.code(), Some(1), "{log}");
    assert!(
        log.contains("1 English string(s) still untranslated"),
        "{log}"
    );
    assert!(log.contains(&dropped), "{log}");
    assert!(
        log.contains("1 table entry(ies) matched no inventory row"),
        "{log}"
    );
    assert!(log.contains("Nothing in this campaign says this."), "{log}");
}

/// Two runs of the same table over the same campaign write the same bytes
/// (ADR-0006), and the formatter — the one authority on canonical form —
/// accepts them.
#[test]
fn two_runs_write_the_same_bytes_and_the_formatter_accepts_them() {
    let dir = campaign("l10n-apply-deterministic");
    let table = dir.join("table.json");
    full_table(&dir, &table);
    assert_eq!(apply(&dir, &table).status.code(), Some(0));
    let first = std::fs::read(dir.join(format!("l10n/{LANG}.json"))).unwrap();
    assert_eq!(apply(&dir, &table).status.code(), Some(0));
    let second = std::fs::read(dir.join(format!("l10n/{LANG}.json"))).unwrap();
    assert_eq!(first, second, "the same inputs write the same bytes");

    let checked = Command::new(BIN)
        .args(["fmt", "--check"])
        .arg(dir.join(format!("l10n/{LANG}.json")))
        .output()
        .expect("run delvec fmt --check");
    assert_eq!(
        checked.status.code(),
        Some(0),
        "the writer wrote a sidecar its own formatter refuses:\n{}",
        String::from_utf8_lossy(&checked.stdout)
    );
}

/// Two keys holding one English string are the named limit: the table form
/// cannot give them different translations, so the run lists them with their
/// keys rather than answering one question twice.
#[test]
fn english_held_by_two_keys_is_named() {
    let dir = campaign("l10n-apply-shared");
    narrate_bundle(&dir, &["One line, said twice.", "One line, said twice."]);
    let table = dir.join("table.json");
    full_table(&dir, &table);
    let log = stdout(&apply(&dir, &table));
    assert!(
        log.contains("held by more than one key"),
        "the limit is named: {log}"
    );
    assert!(log.contains("One line, said twice."), "{log}");
    let keys: Vec<String> = inventory(&dir)
        .into_iter()
        .filter(|(_, en)| en == "One line, said twice.")
        .map(|(k, _)| k)
        .collect();
    assert_eq!(keys.len(), 2, "two keys hold it");
    for k in &keys {
        assert!(log.contains(k), "{k} in:\n{log}");
    }
}
