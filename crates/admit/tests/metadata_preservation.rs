//! **No admission step may lose part of the document it edits.**
//!
//! A prefab's metadata is written once by a generator and then rewritten by
//! every admission step the procedure calls for. Each of those steps owns some
//! part of the document and must leave the rest exactly as it found it. The
//! failure this binds is not hypothetical and not specific to any one field: a
//! step that models fewer fields than the document has deletes the remainder on
//! write, silently, while every test it has passes.
//!
//! It has cost two fields so far. `license.generated_by` — the ADR-0006 row
//! naming the four inputs that regenerate the `.nbt` byte for byte — was written
//! by `delve-grammar expand` and removed by `delve-admit lighting --write`, the
//! very next step of `docs/reference/prefab-procedure.md`. And `waterline_y`,
//! which five island prefabs carry and the ocean-datum invariant `DW0344` keys
//! off, was deleted by every step that wrote the document at all, which would
//! have left that invariant binding to nothing and green.
//!
//! So the assertion here is deliberately not "`generated_by` survives
//! `lighting`". It is **every key, every step**: the document that comes out of
//! an admission step is the document that went in, except at the paths that step
//! exists to write. A future field gets the same protection without anyone
//! remembering to ask for it.
//!
//! # Ownership is a path, not a top-level block
//!
//! The granularity is the point, and getting it wrong is how this file was
//! itself vacuous about the anchor block. A step that owns "`anchors`" is
//! licensed by that phrasing to do anything it likes inside `anchors` — and
//! `anchor` did: re-annotating an anchor that already existed replaced the whole
//! object, deleting the dispenser cell and trigger block the prefab had wired at
//! it, the contract element the exporter had resolved it into, and any key this
//! version of the tool has never heard of. Every one of those is a property of
//! the anchor and none of them is something the operator typed.
//!
//! So a step declares the **paths** it writes, as deep as it really writes them,
//! and `anchor` owns the place and the role of one named anchor rather than the
//! anchor map.
//!
//! # The fixture has to carry the fields at risk
//!
//! A preservation test over a document with nothing to lose is green forever.
//! The fixture is therefore a real `delve-grammar` export — a fixture written by
//! hand would be a fourth copy of the shape and would go stale in exactly the
//! way this test exists to catch — of the one library program that declares a
//! spatial contract, so its anchors carry a real `resolves_to`; plus the
//! annotations the admission procedure adds by hand afterwards (trap hardware,
//! a declared waterline) and one key at each level that no version of this tool
//! models. Every at-risk path is counted, and a count of zero is a failure.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use delvewright_grammar::library::spatial_contract;
use delvewright_grammar::{Box3, ExpandOptions, export_prefab};

const ADMIT: &str = env!("CARGO_BIN_EXE_delve-admit");

/// The anchor the fixture hangs its hand-annotated hardware on, and the one
/// `anchor` re-annotates.
const HARDWARE_ANCHOR: &str = "anchor/watcher-1";

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

/// The paths a document carries that no admission step declares — the ones the
/// losses were on, and the ones a future field will be on. Each is a path from
/// the document root; every one must be present in the fixture, or this file is
/// proving nothing.
const AT_RISK: &[&[&str]] = &[
    &["license", "generated_by"],
    &["waterline_y"],
    &["spatial_contract"],
    &["from_the_future"],
    &["anchors", HARDWARE_ANCHOR, "resolves_to"],
    &["anchors", HARDWARE_ANCHOR, "dispenser"],
    &["anchors", HARDWARE_ANCHOR, "trigger_block"],
    &["anchors", HARDWARE_ANCHOR, "acoustics"],
];

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-admit-preserve-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The piece the procedure produces at §4 — a grammar expansion, frozen, with
/// its provenance row and its resolved contract on it — plus the annotations
/// §5–§7 add to it by hand.
fn exported_piece(tag: &str) -> (PathBuf, PathBuf) {
    let dir = scratch(tag);
    let export = export_prefab(
        &spatial_contract(),
        Box3::at_origin([11, 6, 13]),
        &ExpandOptions::seeded(3),
        "contract-room",
    )
    .expect("the library's contract program exports");
    export.write_to_dir(&dir).unwrap();
    let (nbt, json) = (
        dir.join(&export.structure_file),
        dir.join(&export.metadata_file),
    );

    // The hand half of the procedure: a declared waterline, the hardware a trap
    // anchor carries, and a key from a version of the document newer than this
    // tool. Written as JSON rather than through the typed API on purpose — the
    // unknown keys have no typed API, which is the whole situation being
    // modelled.
    let mut doc = read_json(&json);
    let obj = doc.as_object_mut().unwrap();
    obj.insert("waterline_y".to_string(), serde_json::json!(2));
    obj.insert(
        "from_the_future".to_string(),
        serde_json::json!({ "nested": [1, 2, 3] }),
    );
    let anchor = obj
        .get_mut("anchors")
        .and_then(|a| a.as_object_mut())
        .and_then(|a| a.get_mut(HARDWARE_ANCHOR))
        .unwrap_or_else(|| panic!("the export must declare `{HARDWARE_ANCHOR}`"))
        .as_object_mut()
        .unwrap();
    anchor.insert("dispenser".to_string(), serde_json::json!([3, 2, 4]));
    anchor.insert(
        "trigger_block".to_string(),
        serde_json::json!("minecraft:oak_pressure_plate[powered=false]"),
    );
    anchor.insert("acoustics".to_string(), serde_json::json!("reverberant"));
    std::fs::write(&json, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();

    (nbt, json)
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

/// Every leaf of a document, keyed by its path from the root.
///
/// Leaves rather than keys: a step that keeps a key and changes its value has
/// still changed the document, and `generated_by` is exactly the kind of block
/// whose loss could hide behind a surviving key.
fn leaves(value: &serde_json::Value) -> BTreeMap<Vec<String>, serde_json::Value> {
    fn walk(
        value: &serde_json::Value,
        path: &mut Vec<String>,
        out: &mut BTreeMap<Vec<String>, serde_json::Value>,
    ) {
        match value {
            serde_json::Value::Object(map) if !map.is_empty() => {
                for (key, child) in map {
                    path.push(key.clone());
                    walk(child, path, out);
                    path.pop();
                }
            }
            other => {
                out.insert(path.clone(), other.clone());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(value, &mut Vec::new(), &mut out);
    out
}

fn under(path: &[String], owned: &[&[&str]]) -> bool {
    owned
        .iter()
        .any(|prefix| path.len() >= prefix.len() && path[..prefix.len()] == **prefix)
}

/// Everything outside the paths the step declares is identical; those paths may
/// change freely.
fn assert_only_touches(
    command: &str,
    before: &serde_json::Value,
    after: &serde_json::Value,
    owned: &[&[&str]],
) {
    let (b, a) = (leaves(before), leaves(after));
    for (path, value) in &b {
        if under(path, owned) {
            continue;
        }
        assert_eq!(
            a.get(path),
            Some(value),
            "`delve-admit {command}` changed or dropped `{}`, which it does not write. It \
             declares {owned:?}",
            path.join("/")
        );
    }
    for path in a.keys() {
        assert!(
            under(path, owned) || b.contains_key(path),
            "`delve-admit {command}` invented `{}`, which it does not write",
            path.join("/")
        );
    }
}

/// The whole point, said once per writing subcommand — and, for `anchor`, once
/// per case, because re-annotating an anchor that already exists is a different
/// operation on the document from adding one.
#[test]
fn every_metadata_writing_step_preserves_the_rest_of_the_document() {
    // Binding count: how many (step, at-risk path) pairs were really examined.
    let mut examined = 0usize;

    for command in WRITES_METADATA {
        let cases: &[&str] = match *command {
            "anchor" => &["anchor:new", "anchor:existing", "anchor:role"],
            other => &[other],
        };
        for case in cases {
            let (nbt, json) = exported_piece(case);
            let before = read_json(&json);
            for path in AT_RISK {
                let owned: Vec<String> = path.iter().map(|s| s.to_string()).collect();
                assert!(
                    leaves(&before).keys().any(|p| p.starts_with(&owned)),
                    "the fixture does not carry `{}`, so this step's round trip is not \
                     exercising it — the gate would be green over nothing",
                    path.join("/")
                );
                examined += 1;
            }

            let nbt_s = nbt.to_str().unwrap();
            // Each step declares the paths it writes, as deep as it writes them.
            let owned: Vec<Vec<&str>> = match *case {
                "socket" => {
                    run(&[
                        "socket",
                        nbt_s,
                        "--pos",
                        "5,1,0",
                        "--facing",
                        "north",
                        "--opening",
                        "3,3",
                    ]);
                    vec![vec!["connectors"]]
                }
                "anchor:new" => {
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
                    vec![vec!["anchors", "anchor/test"]]
                }
                "anchor:existing" => {
                    // The live loss: an operator nudges an anchor the piece
                    // already carries. The command names where it is; the
                    // hardware at it, the contract element it resolves into and
                    // the keys this tool has never heard of are the anchor's.
                    run(&[
                        "anchor",
                        nbt_s,
                        "--name",
                        HARDWARE_ANCHOR,
                        "--pos",
                        "1,3,2",
                        "--facing",
                        "south",
                    ]);
                    vec![
                        vec!["anchors", HARDWARE_ANCHOR, "pos"],
                        vec!["anchors", HARDWARE_ANCHOR, "facing"],
                        vec!["anchors", HARDWARE_ANCHOR, "region"],
                        vec!["anchors", HARDWARE_ANCHOR, "block"],
                    ]
                }
                "anchor:role" => {
                    // The same edit saying what the place is FOR. The role is a
                    // fifth field this command owns; everything else about the
                    // anchor is still the anchor's.
                    run(&[
                        "anchor",
                        nbt_s,
                        "--name",
                        HARDWARE_ANCHOR,
                        "--pos",
                        "1,3,2",
                        "--facing",
                        "south",
                        "--role",
                        "entry",
                    ]);
                    vec![
                        vec!["anchors", HARDWARE_ANCHOR, "pos"],
                        vec!["anchors", HARDWARE_ANCHOR, "facing"],
                        vec!["anchors", HARDWARE_ANCHOR, "region"],
                        vec!["anchors", HARDWARE_ANCHOR, "block"],
                        vec!["anchors", HARDWARE_ANCHOR, "role"],
                    ]
                }
                "lighting" => {
                    run(&["lighting", nbt_s, "--write"]);
                    vec![vec!["lighting"]]
                }
                other => panic!("no invocation written for `{other}`"),
            };
            let owned: Vec<&[&str]> = owned.iter().map(|v| v.as_slice()).collect();

            let after = read_json(&json);
            assert_ne!(
                before, after,
                "`{case}` claims to write the document and did not — the check below would then \
                 be vacuous"
            );
            assert_only_touches(case, &before, &after, &owned);

            // Named explicitly as well as covered generally: the two fields this
            // has already cost, and the trap hardware whose loss is silent until
            // a flag-gated trap puts back the wrong block.
            assert_eq!(
                after["license"]["generated_by"], before["license"]["generated_by"],
                "`{case}` dropped or altered the regeneration inputs"
            );
            assert_eq!(
                after["waterline_y"], before["waterline_y"],
                "`{case}` dropped the declared waterline — `DW0344` would then examine one piece \
                 fewer and stay green"
            );
            assert_eq!(
                after["anchors"][HARDWARE_ANCHOR]["trigger_block"],
                before["anchors"][HARDWARE_ANCHOR]["trigger_block"],
                "`{case}` dropped the block a flag-gated trap has to restore"
            );
        }
    }

    assert!(
        examined >= 32,
        "only {examined} (step, at-risk path) pair(s) were examined — this gate is bound to \
         almost nothing"
    );
    eprintln!(
        "preservation examined {examined} (step, at-risk path) pair(s) over {} at-risk path(s)",
        AT_RISK.len()
    );
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
             WRITES_METADATA (with an invocation and the paths it writes), otherwise to \
             DOES_NOT_WRITE_METADATA"
        );
    }
    for c in WRITES_METADATA.iter().chain(DOES_NOT_WRITE_METADATA) {
        assert!(
            commands.iter().any(|x| x == c),
            "`{c}` is classified but `delve-admit` no longer has it"
        );
    }
}

/// The steps classified as not writing the document really do not: the sibling
/// `.json` is byte-identical after each of them.
///
/// Read off the source, this is obvious — none of them calls the writer. That is
/// exactly why it is asserted instead: the classification above is what licenses
/// the gate to skip these commands, and a classification nothing checks is a
/// list of claims.
#[test]
fn every_step_classified_as_not_writing_metadata_leaves_it_byte_identical() {
    let mut checked = 0usize;
    for command in DOES_NOT_WRITE_METADATA {
        // `help` takes no piece; the catalog commands and the log harvesters
        // operate on documents that are not this one and are covered where those
        // documents live.
        let args: Vec<String> = match *command {
            "audit" => vec!["audit".to_string()],
            "resolve-jigsaw" => vec!["resolve-jigsaw".to_string()],
            "gallery" => vec!["gallery".to_string()],
            _ => continue,
        };
        let (nbt, json) = exported_piece(command);
        let before = std::fs::read(&json).unwrap();

        let mut argv: Vec<String> = args;
        let out_dir = nbt.parent().unwrap().join("gallery-out");
        if command == &"gallery" {
            argv.push(nbt.parent().unwrap().to_str().unwrap().to_string());
            argv.push("-o".to_string());
            argv.push(out_dir.to_str().unwrap().to_string());
        } else {
            argv.push(nbt.to_str().unwrap().to_string());
        }
        // The verdict is not what is under test here — `audit` may well fail the
        // piece — only whether the document moved.
        let _ = Command::new(ADMIT).args(&argv).output().unwrap();

        assert_eq!(
            std::fs::read(&json).unwrap(),
            before,
            "`delve-admit {command}` is classified as not writing prefab metadata and wrote it"
        );
        checked += 1;
    }
    assert!(
        checked >= 3,
        "only {checked} non-writing step(s) were exercised"
    );
    eprintln!("{checked} non-writing step(s) left the document byte-identical");
}
