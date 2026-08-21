//! **`DW0848` at the admission event** (spec-0050 §5): a piece's declared
//! footprint class judged against its own bytes.
//!
//! The rule lives once, in `delvewright_dsl::prefab::check_footprint_class`, and
//! two doors ask it: `delve-admit audit`, where the library's integrity lives,
//! and the compiler wherever a `detail-plan` row consumes the piece. This file
//! proves the admission door; `crates/compiler/tests/detail.rs` proves the
//! consumer door. Two doors, one rule — a second implementation is what agrees
//! until it does not.
//!
//! Every red here is a **perturbed declaration on a piece that audited green**,
//! so what is measured is the check rather than a hand-built defect. The piece
//! is synthesised rather than exported from a grammar program for one reason:
//! the property under test is a relation between a declared class and a
//! FOOTPRINT, so the footprint has to be chosen, and no program in the corpus
//! happens to expand to one on the kit grid.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use fastnbt::Value;

const ADMIT: &str = env!("CARGO_BIN_EXE_delve-admit");

/// An 8×8 footprint, five cells tall: exactly a frame of the `alcove` rung
/// (footprint 4..=8 on both axes, clearance 3, plus the one floor course a piece
/// owns), and a multiple of the kit grid's quantum of 4.
const SIZE: [i32; 3] = [8, 5, 8];

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-admit-fp-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The two states this fixture writes, judged against the pinned registry before
/// a byte is serialised.
///
/// A fixture that authors a palette owes the same rule a generator does: an id
/// or a property value the pinned version does not have loads as **air**, with
/// no error anywhere, so the piece silently loses those cells and the test that
/// measured them measured a hole. `tools/check-structure-emitters.py` binds
/// this, and an exemption would have been the weaker answer to a check that is
/// right.
const STATES: [&str; 2] = ["minecraft:stone_bricks", "minecraft:air"];

fn judge_the_palette() {
    let registry = delvewright_schem::blocks::BlockRegistry::v1_21_11();
    let none: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    for name in STATES {
        let verdict = registry.validate(name, &none);
        verdict.unwrap_or_else(|e| panic!("the fixture names an impossible state: {e}"));
    }
}

/// Gzip-framed vanilla structure NBT: a sealed stone shell of `SIZE`, which is
/// all the palette audit needs to see and all the doors need to measure.
fn structure_nbt() -> Vec<u8> {
    judge_the_palette();
    let [sx, sy, sz] = SIZE;
    let mut blocks: Vec<Value> = Vec::new();
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let solid = x == 0 || x == sx - 1 || y == 0 || y == sy - 1 || z == 0 || z == sz - 1;
                let mut b = HashMap::new();
                b.insert(
                    "pos".to_string(),
                    Value::List(vec![Value::Int(x), Value::Int(y), Value::Int(z)]),
                );
                b.insert("state".to_string(), Value::Int(i32::from(!solid)));
                blocks.push(Value::Compound(b));
            }
        }
    }
    let palette = Value::List(
        STATES
            .iter()
            .map(|n| {
                let mut c = HashMap::new();
                c.insert("Name".to_string(), Value::String((*n).to_string()));
                Value::Compound(c)
            })
            .collect(),
    );
    let mut root = HashMap::new();
    root.insert(
        "size".to_string(),
        Value::List(SIZE.iter().map(|v| Value::Int(*v)).collect()),
    );
    root.insert("DataVersion".to_string(), Value::Int(4671));
    root.insert("palette".to_string(), palette);
    root.insert("blocks".to_string(), Value::List(blocks));
    let raw = fastnbt::to_bytes(&Value::Compound(root)).unwrap();
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(&raw).unwrap();
    enc.finish().unwrap()
}

/// One piece on disk, with its sibling declaration document and no
/// `spatial_contract` — which is a legitimate library state (`DW0783` states
/// its own binding over it) and keeps this file measuring one thing.
fn piece(tag: &str) -> (PathBuf, PathBuf) {
    let dir = scratch(tag);
    std::fs::write(dir.join("room.nbt"), structure_nbt()).unwrap();
    let meta = serde_json::json!({
        "prefab_id": "prefab/room",
        "structure": {
            "file": "room.nbt",
            "id": "room",
            "size": SIZE,
            "data_version": 4671,
            "generator": "crates/admit/tests/footprint_class.rs",
        },
        "anchors": {},
        "connectors": [],
        "lighting": { "profile": "unmeasured" },
        "license": {
            "source": "original",
            "spdx": "GPL-3.0-or-later",
            "note": "Test fixture.",
            "provenance": "Synthesised by crates/admit/tests/footprint_class.rs."
        }
    });
    std::fs::write(
        dir.join("room.json"),
        serde_json::to_string_pretty(&meta).unwrap() + "\n",
    )
    .unwrap();
    (dir.join("room.nbt"), dir.join("room.json"))
}

struct Audit {
    ok: bool,
    stderr: String,
}

fn audit(path: &Path) -> Audit {
    let out = Command::new(ADMIT)
        .args(["audit", path.to_str().unwrap()])
        .output()
        .expect("delve-admit runs");
    Audit {
        ok: out.status.success(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
    }
}

fn declare(meta: &Path, class: serde_json::Value) {
    let mut v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(meta).unwrap()).unwrap();
    v["footprint_class"] = class;
    std::fs::write(meta, serde_json::to_string_pretty(&v).unwrap() + "\n").unwrap();
}

/// A piece that declares nothing has withheld a claim rather than made one of
/// none — and the binding line says so instead of going quiet.
#[test]
fn a_piece_that_declares_no_class_states_its_zero_and_is_not_refused() {
    let (nbt, _meta) = piece("undeclared");
    let a = audit(&nbt);
    assert!(a.ok, "{}", a.stderr);
    assert!(
        a.stderr
            .contains("footprint class binding: 0 declared class(es) judged over 1"),
        "the zero is stated against what it was measured over: {}",
        a.stderr
    );
    assert!(!a.stderr.contains("DW0848"), "{}", a.stderr);
}

/// The claim this piece can actually honour.
#[test]
fn an_honest_class_passes_and_is_counted() {
    let (nbt, meta) = piece("honest");
    declare(&meta, serde_json::json!("alcove"));
    let a = audit(&nbt);
    assert!(a.ok, "{}", a.stderr);
    assert!(
        a.stderr
            .contains("footprint class binding: 1 declared class(es) judged over 1"),
        "and it was judged rather than skipped: {}",
        a.stderr
    );
}

/// **The refusal, and it fails the audit rather than annotating it.**
#[test]
fn dw0848_refuses_a_class_the_bytes_contradict() {
    let (nbt, meta) = piece("oversize-claim");
    // `expanse` starts at 64×64; this piece is 8×8.
    declare(&meta, serde_json::json!("expanse"));
    let a = audit(&nbt);
    assert!(!a.ok, "the admission is refused: {}", a.stderr);
    assert!(a.stderr.contains("DW0848"), "{}", a.stderr);
    assert!(
        a.stderr.contains("could serve no box of that class"),
        "{}",
        a.stderr
    );
    assert!(
        a.stderr.contains("x extent is 8"),
        "and names the measurement, not just the verdict: {}",
        a.stderr
    );
}

/// The height half of the rule: a piece of any footprint that leaves its class
/// no clearance could fill no box of it either.
#[test]
fn dw0848_refuses_a_class_whose_clearance_the_piece_is_too_short_for() {
    let (nbt, meta) = piece("too-short");
    // `hall` wants 16..=32 on both axes and 8 of clearance; this piece is 8×8×5.
    declare(&meta, serde_json::json!("hall"));
    let a = audit(&nbt);
    assert!(!a.ok, "{}", a.stderr);
    assert!(a.stderr.contains("DW0848"), "{}", a.stderr);
    assert!(
        a.stderr.contains("the shallowest `hall` frame is 9"),
        "clearance plus the one floor course a piece owns: {}",
        a.stderr
    );
}

/// A name the metrics table does not define is `DW0812`, as for any document
/// naming a table entry — not a second spelling of `DW0848`.
#[test]
fn dw0812_refuses_a_class_name_the_table_does_not_define() {
    let (nbt, meta) = piece("unknown-name");
    declare(&meta, serde_json::json!("cathedral"));
    let a = audit(&nbt);
    assert!(!a.ok, "{}", a.stderr);
    assert!(a.stderr.contains("DW0812"), "{}", a.stderr);
}
