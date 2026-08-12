//! Audit fixtures (spec-0007 quality bar): a clean piece, a command-block piece,
//! a disallowed-palette piece, and an NBT-bearing spawner.

use delvewright_admit::allowlist::Allowlist;
use delvewright_admit::audit::audit;
use delvewright_admit::fixtures;
use delvewright_admit::meta::{License, PrefabMeta};
use delvewright_admit::socket::{self, SocketDecl};

#[test]
fn carved_jigsaw_socket_passes_the_audit() {
    // A library prefab's jigsaw sockets are legitimate (the solver mates them) and
    // must NOT be hard-forbidden — only command/structure blocks + NBT spawners are.
    let mut s = fixtures::clean_room();
    let mut meta = PrefabMeta::skeleton(
        "clean",
        s.size,
        s.data_version,
        License {
            source: "original".into(),
            spdx: "CC0-1.0".into(),
            note: String::new(),
            provenance: String::new(),
        },
    );
    socket::carve(&mut s, &mut meta, &SocketDecl::new([3, 1, 0], "north")).unwrap();
    let (rep, _) = audit("socketed", &s, &Allowlist::default_building());
    assert!(
        rep.is_pass(),
        "carved socket should pass: {:?}",
        rep.findings
    );
    assert!(s.block_names().contains("minecraft:jigsaw"));
}

#[test]
fn clean_piece_passes() {
    let s = fixtures::clean_room();
    let (rep, diags) = audit("clean", &s, &Allowlist::default_building());
    assert!(rep.is_pass(), "clean room should pass: {:?}", rep.findings);
    assert_eq!(rep.forbidden, 0);
    assert_eq!(rep.not_allowlisted, 0);
    assert!(diags.is_empty());
    // palette is reported for reviewer visibility.
    assert!(rep.palette.iter().any(|b| b == "minecraft:stone_bricks"));
}

#[test]
fn command_block_piece_is_hard_rejected() {
    let s = fixtures::command_block_piece();
    let (rep, _) = audit("cb", &s, &Allowlist::default_building());
    assert!(!rep.is_pass());
    assert!(rep.forbidden >= 1, "command block must be forbidden");
    // both the block name and the embedded Command are DW0731.
    assert!(rep.findings.iter().any(|f| f.code == "DW0731"));
    // the offending cell position is reported.
    assert!(rep.findings.iter().any(|f| f.pos == Some([1, 1, 1])));
}

#[test]
fn spawner_with_nbt_is_hard_rejected() {
    let s = fixtures::spawner_piece();
    let (rep, _) = audit("sp", &s, &Allowlist::default_building());
    assert!(!rep.is_pass());
    assert!(rep.findings.iter().any(|f| f.code == "DW0731"));
}

#[test]
fn disallowed_palette_block_fails_allowlist() {
    let s = fixtures::disallowed_palette_piece();
    let (rep, _) = audit("tnt", &s, &Allowlist::default_building());
    assert!(!rep.is_pass());
    assert_eq!(rep.forbidden, 0, "tnt is not a code-injection vector");
    assert!(rep.not_allowlisted >= 1);
    assert!(rep.findings.iter().any(|f| f.code == "DW0730"));
}

#[test]
fn allowlist_override_can_admit_a_surprising_block() {
    let s = fixtures::disallowed_palette_piece();
    let allow = Allowlist::from_file(r#"{ "allow": ["minecraft:air","minecraft:stone_bricks","minecraft:glowstone","minecraft:tnt"] }"#).unwrap();
    let (rep, _) = audit("tnt", &s, &allow);
    assert!(
        rep.is_pass(),
        "override should admit tnt: {:?}",
        rep.findings
    );
}

#[test]
fn report_json_is_machine_readable() {
    let s = fixtures::command_block_piece();
    let (rep, _) = audit("cb", &s, &Allowlist::default_building());
    let json = rep.to_json();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["verdict"], "fail");
    assert!(!v["findings"].as_array().unwrap().is_empty());
    assert!(json.ends_with("}\n"));
}

/// `DW0733`: a block the pinned game does not have.
///
/// The allowlist cannot catch this and the test says why: `minecraft:chain` is
/// in the built-in allowlist to this day, because an allowlist is a list of
/// names somebody once approved and nothing re-checks a name against the game.
#[test]
fn a_block_the_pinned_version_does_not_have_fails_the_audit() {
    let s = fixtures::renamed_block_piece();
    let (rep, _) = audit("ropes", &s, &Allowlist::default_building());
    assert!(!rep.is_pass());
    assert_eq!(rep.unknown_blocks, 1);
    assert_eq!(
        rep.forbidden, 0,
        "a renamed block is not an injection vector"
    );
    let f = rep
        .findings
        .iter()
        .find(|f| f.code == "DW0733")
        .expect("DW0733 must fire");
    assert!(f.message.contains("minecraft:iron_chain"), "{}", f.message);
    assert_eq!(f.severity, "error");
}

/// ...and the rename itself audits clean, so the gate is not simply refusing
/// everything in that shape.
#[test]
fn the_rename_passes_the_audit() {
    let mut s = fixtures::clean_room();
    s.set_cell(
        [2, 1, 2],
        delvewright_admit::structure::PaletteEntry::simple("minecraft:iron_chain"),
        None,
    );
    let (rep, _) = audit("ropes", &s, &Allowlist::default_building());
    assert_eq!(rep.unknown_blocks, 0);
    assert!(rep.is_pass(), "{:?}", rep.findings);
}
