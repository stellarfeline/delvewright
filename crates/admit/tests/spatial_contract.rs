//! **One checker, two doors** (spec-0036 §1c, AC6).
//!
//! `delve-grammar expand` judges a contract against the blocks it just wrote.
//! `delve-admit audit` judges the same contract against the same blocks read
//! back off disk. Same bytes plus same resolved contract must give the same
//! verdict whichever door it came through — which is true here because there is
//! one implementation of the obligations and this crate calls it.

use std::path::{Path, PathBuf};
use std::process::Command;

use delvewright_admit::structure::{PaletteEntry, Structure};
use delvewright_grammar::library::spatial_contract::spatial_contract;
use delvewright_grammar::{Box3, ExpandOptions, export_prefab};

const ADMIT: &str = env!("CARGO_BIN_EXE_delve-admit");

/// The region the corpus program is documented at.
const PIECE: Box3 = Box3::at_origin([11, 6, 15]);

fn library(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-admit-contract-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    export_prefab(
        &spatial_contract(),
        PIECE,
        &ExpandOptions::seeded(1),
        "twin-room",
    )
    .expect("the corpus program exports")
    .write_to_dir(&dir)
    .unwrap();
    dir
}

fn audit(dir: &Path) -> (bool, String) {
    let out = Command::new(ADMIT)
        .args(["audit", dir.join("twin-room.nbt").to_str().unwrap()])
        .output()
        .expect("delve-admit runs");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

/// **The green.** The piece `delve-grammar expand` passed, admitted through the
/// other door, still passes — and says what each opt-out was, by name.
#[test]
fn the_second_door_agrees_with_the_first_on_a_piece_that_holds() {
    let dir = library("green");
    let (ok, stderr) = audit(&dir);
    assert!(ok, "{stderr}");
    assert!(
        stderr.contains("no_body \"shelf\": posted"),
        "the enumeration a reviewer reads is printed by this door too: {stderr}"
    );
    assert!(
        stderr.contains("exterior face"),
        "the face contract is enumerated: {stderr}"
    );
}

/// **The red.** Take the iron out of the barred door and nothing else changes:
/// the same metadata now describes a building it is not true of, and the door
/// the operator actually runs refuses it with the code named.
#[test]
fn a_piece_whose_blocks_no_longer_match_its_contract_is_refused_as_dw0782() {
    let dir = library("red");
    let nbt = dir.join("twin-room.nbt");
    let mut s = Structure::read(&std::fs::read(&nbt).unwrap()).unwrap();
    for y in 1..=3 {
        for x in 4..=6 {
            s.set_cell([x, y, 7], PaletteEntry::simple("minecraft:air"), None);
        }
    }
    s.prune_palette();
    std::fs::write(&nbt, s.write()).unwrap();

    let (ok, stderr) = audit(&dir);
    assert!(!ok, "an unbarred bar must not be admitted: {stderr}");
    assert!(stderr.contains("DW0782"), "{stderr}");
    assert!(
        stderr.contains("contract-edge-proof FAILED"),
        "the gate that disagreed is named: {stderr}"
    );
    assert!(
        stderr.contains("does not bar anything"),
        "and what it found: {stderr}"
    );
    assert!(
        stderr.contains("examined 1 object(s)"),
        "with its binding count: {stderr}"
    );
}
