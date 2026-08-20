//! **A door that did not open is not a door that opened and found nothing.**
//!
//! `tests/spatial_contract.rs` proves the second door's *verdicts* — that the
//! checker `delve-grammar expand` runs and the checker `delve-admit audit` runs
//! are one checker, agreeing on a piece that holds and refusing one that does
//! not. This file proves the other half, which nothing proved: **that the door
//! opens at all**, and that every way it can stay shut says so with the count of
//! what it therefore did not examine.
//!
//! The shape it binds was five conditions in one `if let` chain, any of which
//! fell through to a `contract_failed` still `false`, a `"verdict": "pass"`
//! report and exit 0:
//!
//! 1. **a tile-set manifest** — which is what a composed or tiled zone IS, so
//!    the door never opened on a composed building at all. A 3-tile zone with
//!    nine bar cells ripped out of its barred door audited clean.
//! 2. bytes that cannot be read, and
//! 3. bytes that do not parse — both already refused by the command, but as its
//!    own input errors, and the door fell through them in silence.
//! 4. **no metadata beside the piece, or metadata that does not parse.** The
//!    second is a piece whose contract nobody can read, reported as a pass.
//! 5. **metadata that declares no contract** — the hatch, and the one a DROPPED
//!    declaration produces byte for byte.
//!
//! Condition 5 is why the outcomes here are four rather than two. "This piece
//! declares no contract, so there is nothing to audit" is an opt-out the defect
//! can supply — an admission step that loses a top-level key on write produces
//! exactly that document — so the absence is corroborated against something the
//! dropper cannot fake: `resolves_to`, which only an exporter writes and only
//! out of a contract. A document with no contract and resolved anchors
//! contradicts itself and is refused; a document with neither states its zero
//! binding and is never silent. The corroboration's own binding count is stated
//! too, because a document with no anchors is one nothing here could contradict.

use std::path::{Path, PathBuf};
use std::process::Command;

use delvewright_admit::structure::{PaletteEntry, Structure};
use delvewright_grammar::export::export_zone;
use delvewright_grammar::library::spatial_contract::spatial_contract;
use delvewright_grammar::{Box3, ExpandOptions, export_prefab};

const ADMIT: &str = env!("CARGO_BIN_EXE_delve-admit");

/// A region past the 48-per-axis structure cap on X, so the export tiles.
const ZONE: Box3 = Box3::at_origin([101, 6, 15]);
/// A region that fits one template — the same program, same seed.
const PIECE: Box3 = Box3::at_origin([11, 6, 15]);

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-admit-door-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The corpus contract program, frozen as a TILED zone: several `.nbt` files and
/// one manifest carrying the zone-relative contract.
fn tiled_zone(tag: &str) -> (PathBuf, usize) {
    let dir = scratch(tag);
    let export =
        export_zone(&spatial_contract(), ZONE, &ExpandOptions::seeded(1), "zone").expect("exports");
    export.write_to_dir(&dir).unwrap();
    let tiles = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("nbt"))
        .count();
    assert!(
        tiles > 1,
        "the fixture must actually tile, or this file is about the case it is not testing"
    );
    (dir.join("zone.json"), tiles)
}

/// The same program frozen as ONE structure template, with its sibling document.
fn single_piece(tag: &str) -> (PathBuf, PathBuf) {
    let dir = scratch(tag);
    export_prefab(
        &spatial_contract(),
        PIECE,
        &ExpandOptions::seeded(1),
        "room",
    )
    .expect("exports")
    .write_to_dir(&dir)
    .unwrap();
    (dir.join("room.nbt"), dir.join("room.json"))
}

struct Audit {
    ok: bool,
    stderr: String,
    report: serde_json::Value,
}

fn audit(path: &Path) -> Audit {
    let out = Command::new(ADMIT)
        .args(["audit", path.to_str().unwrap()])
        .output()
        .expect("delve-admit runs");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let report: serde_json::Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("a machine-readable report on stdout: {e}\nstderr: {stderr}"));
    Audit {
        ok: out.status.success(),
        stderr,
        report,
    }
}

/// Take every iron bar out of every `.nbt` in `dir` — the same perturbation
/// `spatial_contract.rs` makes to one template, made to a zone spread over
/// several files. The bars are the barred edge's whole proof.
fn rip_the_bars_out(dir: &Path) -> usize {
    let mut ripped = 0usize;
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("nbt"))
        .collect();
    files.sort();
    for path in files {
        let mut s = Structure::read(&std::fs::read(&path).unwrap()).unwrap();
        let cells: Vec<[i32; 3]> = s
            .blocks
            .iter()
            .filter(|b| s.palette[b.state as usize].name.contains("iron_bars"))
            .map(|b| b.pos)
            .collect();
        if cells.is_empty() {
            continue;
        }
        for cell in cells {
            s.set_cell(cell, PaletteEntry::simple("minecraft:air"), None);
            ripped += 1;
        }
        s.prune_palette();
        std::fs::write(&path, s.write()).unwrap();
    }
    ripped
}

fn contract(report: &serde_json::Value) -> &serde_json::Value {
    let block = &report["contract"];
    assert!(
        block.is_object(),
        "every report states what the door did: {report}"
    );
    block
}

// ---------------------------------------------------------------------------
// Condition 1 — the manifest, which is the one that mattered
// ---------------------------------------------------------------------------

/// **The green, at zone scale.** A tiled zone's manifest is a first-class input
/// to the door: the contract it declares is zone-relative, so the checker's two
/// arguments exist there exactly as they do for one template — and the report
/// says how many files and how many cells that verdict covers.
#[test]
fn a_tiled_zone_is_judged_and_the_report_says_over_how_many_files() {
    let (manifest, tiles) = tiled_zone("zone-green");
    let a = audit(&manifest);
    assert!(a.ok, "{}", a.stderr);
    let c = contract(&a.report);
    assert_eq!(c["state"], "judged", "{c}");
    assert_eq!(
        c["files"].as_u64().unwrap() as usize,
        tiles,
        "the binding names every file the verdict covers: {c}"
    );
    assert_eq!(
        c["cells"].as_u64().unwrap(),
        (ZONE.size[0] as u64) * (ZONE.size[1] as u64) * (ZONE.size[2] as u64),
        "and the whole assembled zone, not a tile: {c}"
    );
    assert!(c["gates"].as_u64().unwrap() >= 9, "{c}");
    assert!(
        c["objects"].as_u64().unwrap() > 0,
        "obligations that examined nothing are not a pass: {c}"
    );
    assert_eq!(c["failed_gates"], 0, "{c}");
    assert!(
        a.stderr.contains("DW0782") && a.stderr.contains("judged"),
        "the enumeration and the binding reach the operator: {}",
        a.stderr
    );
}

/// **The red the old shape shipped.** Nine bar cells out of a three-tile zone,
/// nothing else changed: the manifest still describes a building it is no longer
/// true of. Before, this audited clean and exited 0 — the door's first condition
/// was `extension() != "json"`, and a composed zone is addressed by its manifest.
#[test]
fn a_tiled_zone_whose_blocks_no_longer_match_its_contract_is_refused_as_dw0782() {
    let (manifest, _) = tiled_zone("zone-red");
    let ripped = rip_the_bars_out(manifest.parent().unwrap());
    assert!(
        ripped > 0,
        "the perturbation must actually change the bytes"
    );

    let a = audit(&manifest);
    assert!(!a.ok, "an unbarred zone must not be admitted: {}", a.stderr);
    assert!(a.stderr.contains("DW0782"), "{}", a.stderr);
    assert!(
        a.stderr.contains("contract-edge-proof FAILED"),
        "the gate that disagreed is named: {}",
        a.stderr
    );
    assert!(
        a.stderr.contains("does not bar anything"),
        "and what it found: {}",
        a.stderr
    );
    let c = contract(&a.report);
    assert_eq!(c["state"], "judged", "{c}");
    assert!(c["failed_gates"].as_u64().unwrap() >= 1, "{c}");
    assert_eq!(
        a.report["verdict"], "fail",
        "the saved artifact agrees with the exit code: {}",
        a.report
    );
}

// ---------------------------------------------------------------------------
// Conditions 4 and 5 — the document
// ---------------------------------------------------------------------------

/// A declaration document that does not parse is not a piece without a
/// contract; it is a piece whose contract nobody can read. `DW0783`, exit 1 —
/// where it was exit 0 with an empty stderr.
#[test]
fn a_declaration_document_that_does_not_parse_is_refused_as_dw0783() {
    let (nbt, json) = single_piece("badmeta");
    std::fs::write(&json, "{ this is not a document").unwrap();

    let a = audit(&nbt);
    assert!(!a.ok, "{}", a.stderr);
    assert!(a.stderr.contains("DW0783"), "{}", a.stderr);
    assert!(
        a.stderr.contains("COULD NOT judge"),
        "the refusal says which kind it is: {}",
        a.stderr
    );
    assert_eq!(contract(&a.report)["state"], "refused");
    assert_eq!(a.report["verdict"], "fail");
}

/// **The opt-out the defect cannot supply.** Delete `spatial_contract` from a
/// document and nothing else: the piece now "declares no contract", which is the
/// legitimate case byte for byte. It is refused anyway, because its anchors
/// still carry the `resolves_to` an exporter writes only out of a contract — so
/// the document contradicts itself, and the declaration was dropped rather than
/// never made.
#[test]
fn a_dropped_contract_is_not_an_absent_one_and_is_refused_as_dw0783() {
    let (nbt, json) = single_piece("dropped");
    let mut doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
    doc.as_object_mut().unwrap().remove("spatial_contract");
    std::fs::write(&json, serde_json::to_string_pretty(&doc).unwrap()).unwrap();

    let a = audit(&nbt);
    assert!(!a.ok, "{}", a.stderr);
    assert!(a.stderr.contains("DW0783"), "{}", a.stderr);
    assert!(
        a.stderr.contains("resolves_to"),
        "the refusal names the corroboration that contradicted the absence: {}",
        a.stderr
    );
    let c = contract(&a.report);
    assert_eq!(c["state"], "refused", "{c}");
    assert!(
        c["resolved_anchors"].as_u64().unwrap() > 0,
        "the drop detector states its own binding count: {c}"
    );
}

/// A document that declares no contract and whose anchors resolve into nothing
/// is the legitimate case: no refusal, and **never a silence**. The zero binding
/// is stated with what it means, and the corroboration reports how many anchors
/// it examined — a document with none is one nothing could have contradicted.
#[test]
fn a_document_that_legitimately_declares_none_states_its_zero_binding_as_dw0783() {
    let (nbt, json) = single_piece("undeclared");
    let mut doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
    doc.as_object_mut().unwrap().remove("spatial_contract");
    for anchor in doc["anchors"].as_object_mut().unwrap().values_mut() {
        anchor.as_object_mut().unwrap().remove("resolves_to");
    }
    std::fs::write(&json, serde_json::to_string_pretty(&doc).unwrap()).unwrap();

    let a = audit(&nbt);
    assert!(
        a.ok,
        "a piece that declares none is not a failure: {}",
        a.stderr
    );
    assert!(a.stderr.contains("DW0783"), "{}", a.stderr);
    assert!(
        a.stderr.contains("examined NOTHING"),
        "the zero binding is stated, not implied by an absence: {}",
        a.stderr
    );
    let c = contract(&a.report);
    assert_eq!(c["state"], "undeclared", "{c}");
    assert_eq!(c["gates"], 0, "{c}");
    assert_eq!(c["objects"], 0, "{c}");
    assert_eq!(c["resolved_anchors"], 0, "{c}");
    assert!(
        c["anchors"].as_u64().unwrap() > 0,
        "and it says how many anchors could have contradicted it: {c}"
    );
}

/// No document at all — an ingested piece audited before the admission steps
/// write one. Legitimate, stated, counted, and distinguishable from every other
/// outcome by `state` alone.
#[test]
fn a_piece_with_no_declaration_document_states_its_zero_binding_as_dw0783() {
    let (nbt, json) = single_piece("nodoc");
    std::fs::remove_file(&json).unwrap();

    let a = audit(&nbt);
    assert!(a.ok, "{}", a.stderr);
    assert!(a.stderr.contains("DW0783"), "{}", a.stderr);
    let c = contract(&a.report);
    assert_eq!(c["state"], "no-document", "{c}");
    assert_eq!(c["cells"].as_u64().unwrap(), 11 * 6 * 15, "{c}");
    assert_eq!(c["files"], 1, "{c}");
}

// ---------------------------------------------------------------------------
// The set, and the binding count of this file itself
// ---------------------------------------------------------------------------

/// **Every outcome is reachable, distinct, and named in the report.**
///
/// The individual tests above each pin one arm. This one asks the question a
/// list of arms cannot: are they actually different to a reader? A `state` that
/// two situations share is the silence coming back under a new name — which is
/// exactly what the old report did, where all four of these produced the same
/// bytes.
#[test]
fn the_four_outcomes_are_distinguishable_in_the_report() {
    let mut seen: Vec<(String, String)> = Vec::new();

    let (manifest, _) = tiled_zone("set-judged");
    seen.push((
        "a tiled zone with a contract".into(),
        contract(&audit(&manifest).report)["state"]
            .as_str()
            .unwrap()
            .to_string(),
    ));

    let (nbt, json) = single_piece("set-none");
    std::fs::remove_file(&json).unwrap();
    seen.push((
        "no document".into(),
        contract(&audit(&nbt).report)["state"]
            .as_str()
            .unwrap()
            .to_string(),
    ));

    let (nbt, json) = single_piece("set-undeclared");
    let mut doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
    doc.as_object_mut().unwrap().remove("spatial_contract");
    for anchor in doc["anchors"].as_object_mut().unwrap().values_mut() {
        anchor.as_object_mut().unwrap().remove("resolves_to");
    }
    std::fs::write(&json, serde_json::to_string_pretty(&doc).unwrap()).unwrap();
    seen.push((
        "declares none".into(),
        contract(&audit(&nbt).report)["state"]
            .as_str()
            .unwrap()
            .to_string(),
    ));

    let (nbt, json) = single_piece("set-refused");
    std::fs::write(&json, "{").unwrap();
    seen.push((
        "unreadable document".into(),
        contract(&audit(&nbt).report)["state"]
            .as_str()
            .unwrap()
            .to_string(),
    ));

    let mut states: Vec<&str> = seen.iter().map(|(_, s)| s.as_str()).collect();
    states.sort_unstable();
    states.dedup();
    assert_eq!(
        states.len(),
        seen.len(),
        "two situations report the same state, so the report cannot tell them apart: {seen:?}"
    );
    eprintln!("the door's outcomes, examined: {seen:?}");
}

/// A report the **command** did not produce says the door was never opened, in
/// the same field and by the same word.
///
/// `delvewright_admit::audit::audit` is the palette half on its own — a library
/// entry point with no files and no document, so it has no door to open. Left
/// as a default of zeroes it would have claimed the door opened and examined
/// nothing, which is the exact confusion this field exists to end.
#[test]
fn a_report_whose_door_was_never_opened_says_unopened() {
    let (report, _) = delvewright_admit::audit::audit(
        "fixture",
        &delvewright_admit::fixtures::clean_room(),
        &delvewright_admit::allowlist::Allowlist::default_building(),
    );
    assert_eq!(report.contract.state, delvewright_admit::spatial::UNOPENED);
    let v: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(v["contract"]["state"], "unopened", "{v}");
}
