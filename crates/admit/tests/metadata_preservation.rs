//! **No admission step may lose part of the document it edits.**
//!
//! A prefab's metadata is written once by a generator and then rewritten by
//! every admission step the procedure calls for. Each of those steps owns one
//! block of the document and must leave the rest exactly as it found it. The
//! failure this binds is not hypothetical and not specific to any one field: a
//! step that models fewer fields than the document has deletes the remainder on
//! write, silently, while every test it has passes.
//!
//! The field it cost was `license.generated_by` — the ADR-0006 row naming the
//! four inputs that regenerate the `.nbt` byte for byte. `delve-grammar expand`
//! wrote it and `delve-admit lighting --write`, the very next step of
//! `docs/reference/prefab-procedure.md`, removed it, leaving the regeneration
//! inputs legible to a human in the prose `provenance` sentence and to no tool
//! at all.
//!
//! So the assertion here is deliberately not "`generated_by` survives
//! `lighting`". It is **every key, every step**: the document that comes out of
//! an admission step is the document that went in, except under the one
//! top-level key that step exists to change. A future field gets the same
//! protection without anyone remembering to ask for it.
//!
//! The input is a real `delve-grammar` export rather than a fixture, because a
//! fixture is a fourth copy of the shape and would go stale in exactly the way
//! this test exists to catch.

use std::path::{Path, PathBuf};
use std::process::Command;

use delvewright_grammar::library::store_room;
use delvewright_grammar::{Box3, ExpandOptions, export_prefab};

const ADMIT: &str = env!("CARGO_BIN_EXE_delve-admit");

/// Every `delve-admit` subcommand, and whether it rewrites a prefab's metadata.
///
/// This list is checked against the binary's own `--help` below, so a new
/// subcommand fails this test until someone classifies it — the classification
/// cannot be forgotten, because there is nowhere to forget it.
const WRITES_METADATA: &[&str] = &["socket", "anchor", "lighting"];
const DOES_NOT_WRITE_METADATA: &[&str] = &[
    "audit",          // reads the `.nbt`, writes only a report
    "resolve-jigsaw", // rewrites the `.nbt`, never the metadata
    "catalog",        // reads/writes `catalog/<id>.json`, a different document
    "gallery",        // reads metadata, writes a world
    "curate",         // reads a server log, writes a report
    "curate-merge",   // writes catalog cards, a different document
    "help",
];

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-admit-preserve-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The piece the procedure produces at §4: a grammar expansion, frozen, with its
/// provenance row on it.
fn exported_piece(tag: &str) -> (PathBuf, PathBuf) {
    let dir = scratch(tag);
    let export = export_prefab(
        &store_room(),
        Box3::at_origin([7, 5, 14]),
        &ExpandOptions::seeded(3),
        "store-room",
    )
    .expect("the library store-room exports");
    export.write_to_dir(&dir).unwrap();
    (
        dir.join(&export.structure_file),
        dir.join(&export.metadata_file),
    )
}

fn read_json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn run(args: &[&str]) {
    let out = Command::new(ADMIT).args(args).output().unwrap();
    assert!(
        out.status.success(),
        "delve-admit {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Everything outside `owned` is identical; `owned` itself is allowed to change.
fn assert_only_touches(before: &serde_json::Value, after: &serde_json::Value, owned: &str) {
    let (b, a) = (
        before.as_object().expect("metadata is an object"),
        after.as_object().expect("metadata is an object"),
    );
    for (key, value) in b {
        if key == owned {
            continue;
        }
        assert_eq!(
            a.get(key),
            Some(value),
            "`{key}` was changed or dropped by a step that only owns `{owned}`"
        );
    }
    for key in a.keys() {
        assert!(
            key == owned || b.contains_key(key),
            "`{key}` appeared from nowhere"
        );
    }
}

/// The whole point, said once per writing subcommand.
#[test]
fn every_metadata_writing_step_preserves_the_rest_of_the_document() {
    for command in WRITES_METADATA {
        let (nbt, json) = exported_piece(command);
        let before = read_json(&json);
        assert!(
            before["license"]["generated_by"].is_object(),
            "the fixture must arrive carrying a provenance row, or this proves nothing"
        );

        let nbt_s = nbt.to_str().unwrap();
        let owned = match *command {
            "socket" => {
                run(&[
                    "socket",
                    nbt_s,
                    "--pos",
                    "3,1,0",
                    "--facing",
                    "north",
                    "--opening",
                    "3,3",
                ]);
                "connectors"
            }
            "anchor" => {
                run(&[
                    "anchor",
                    nbt_s,
                    "--name",
                    "anchor/test",
                    "--pos",
                    "3,1,7",
                    "--facing",
                    "north",
                ]);
                "anchors"
            }
            "lighting" => {
                run(&["lighting", nbt_s, "--write"]);
                "lighting"
            }
            other => panic!("no invocation written for `{other}`"),
        };

        let after = read_json(&json);
        assert_ne!(
            before[owned], after[owned],
            "`{command}` claims to write `{owned}` and did not — the check below would then be \
             vacuous"
        );
        assert_only_touches(&before, &after, owned);

        // Named explicitly as well as covered generally: this is the row the
        // whole procedure's ADR-0006 claim rests on, and the one that was lost.
        assert_eq!(
            after["license"]["generated_by"], before["license"]["generated_by"],
            "`{command}` dropped or altered the regeneration inputs"
        );
    }
}

/// A new subcommand cannot quietly join the tool unclassified — the enumeration
/// above is bound to the binary's own command list, not to a doc line.
#[test]
fn every_subcommand_is_classified_as_writing_metadata_or_not() {
    let help = Command::new(ADMIT).arg("--help").output().unwrap();
    let text = String::from_utf8(help.stdout).unwrap();
    let commands: Vec<String> = text
        .lines()
        .skip_while(|l| !l.starts_with("Commands:"))
        .skip(1)
        .take_while(|l| !l.trim().is_empty())
        .filter_map(|l| l.split_whitespace().next().map(str::to_string))
        .collect();
    assert!(
        commands.len() >= 9,
        "could not read the command list out of --help: {commands:?}"
    );
    for c in &commands {
        assert!(
            WRITES_METADATA.contains(&c.as_str()) || DOES_NOT_WRITE_METADATA.contains(&c.as_str()),
            "`delve-admit {c}` is not classified: if it rewrites prefab metadata add it to \
             WRITES_METADATA (with an invocation), otherwise to DOES_NOT_WRITE_METADATA"
        );
    }
    for c in WRITES_METADATA.iter().chain(DOES_NOT_WRITE_METADATA) {
        assert!(
            commands.iter().any(|x| x == c),
            "`{c}` is classified but `delve-admit` no longer has it"
        );
    }
}
