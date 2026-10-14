//! **The `prefab` operation** (spec-0106 §3.8, criterion 13): a program places
//! an admitted library piece at a scope of its size — its bytes, its anchors
//! under a mark's naming, its contract joined under the scope's claim name,
//! its provenance carried — and nothing the seed draws places one.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use delvec::grammar::geom::{Axis, Box3};
use delvec::grammar::ir::{
    Alternative, Contract, EdgeClass, Envelope, Material, Node, PrefabRotation, Program, Size,
    Split,
};
use delvec::grammar::pieces::Pieces;
use delvec::grammar::{BlockState, ExpandOptions, expand, judge};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

/// The piece: a 5 x 4 x 5 cell, its doorway in the north wall.
const SIZE: [i32; 3] = [5, 4, 5];

fn cell(p: [i32; 3]) -> &'static str {
    let [x, y, z] = p;
    let edge = x == 0 || x == 4 || z == 0 || z == 4;
    let door = z == 0 && x == 2 && (1..=2).contains(&y);
    if y == 0 {
        "minecraft:polished_andesite"
    } else if y == 3 || (edge && !door) {
        "minecraft:stone_bricks"
    } else {
        "minecraft:air"
    }
}

fn library(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-prefab-op-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut cells = Vec::new();
    for x in 0..SIZE[0] {
        for y in 0..SIZE[1] {
            for z in 0..SIZE[2] {
                cells.push(([x, y, z], cell([x, y, z])));
            }
        }
    }
    std::fs::write(dir.join("cell.nbt"), common::structure_nbt(SIZE, &cells)).unwrap();
    let region = |f: [i32; 3], t: [i32; 3]| serde_json::json!({"from": f, "to": t});
    let meta = serde_json::json!({
        "prefab_id": "prefab/cell",
        "structure": {"file": "cell.nbt", "id": "cell", "size": SIZE, "data_version": 4671,
                      "generator": "crates/delvec/tests/grammar_prefab_op.rs"},
        "anchors": {"seat": {"pos": [2, 1, 3], "facing": "north"}},
        "connectors": [],
        "lighting": {"profile": "unmeasured"},
        "license": {"source": "original", "spdx": "GPL-3.0-or-later", "note": "Test fixture.",
                    "provenance": "Written by grammar_prefab_op.rs."},
        "spatial_contract": {
            "entry": "room",
            "spaces": {"room": {"envelope": "enclosed", "boxes": [region([1, 1, 1], [3, 2, 3])]}},
            "no_body": {},
            "edges": [{"a": "room", "b": "exterior", "class": "walk"}],
            "faces": [{"space": "room", "class": "walk", "dir": "north",
                       "opening": region([2, 1, 0], [2, 2, 0])}]
        }
    });
    std::fs::write(
        dir.join("cell.json"),
        serde_json::to_string_pretty(&meta).unwrap(),
    )
    .unwrap();
    dir
}

fn prefab(rotation: PrefabRotation) -> Node {
    Node::Prefab {
        piece: "prefab/cell".to_string(),
        rotation,
    }
}

/// A yard four deep, then the tower: the piece, under the claim `tower`.
fn program(tower: Node) -> Program {
    Program::new("yard_and_tower", "site")
        .role(
            "ground",
            "grass_block[snowy=false]".parse::<BlockState>().unwrap(),
        )
        .rule(
            "site",
            Node::Split(Split {
                axis: Axis::Z,
                sizes: vec![Size::abs(4), Size::rel(1)],
                rounding: None,
                repeat: false,
                orient: Default::default(),
                children: vec![Node::call("yard"), tower],
            }),
        )
        .rule(
            "yard",
            Node::Split(Split {
                axis: Axis::Y,
                sizes: vec![Size::abs(1), Size::rel(1)],
                rounding: None,
                repeat: false,
                orient: Default::default(),
                children: vec![
                    Node::Fill {
                        material: Material::role("ground"),
                    },
                    Node::Claim {
                        region: "yard".to_string(),
                        body: Box::new(Node::Void),
                    },
                ],
            }),
        )
        .contract(Contract::new("yard").space("yard", Envelope::Open).edge(
            "yard",
            "exterior",
            EdgeClass::Walk {
                rise: 0,
                via: None,
                way: None,
            },
        ))
}

fn claimed_tower() -> Node {
    Node::Claim {
        region: "tower".to_string(),
        body: Box::new(prefab(PrefabRotation::None)),
    }
}

fn pieces(dir: &Path, p: &Program) -> Pieces {
    Pieces::load_for(p, dir).unwrap()
}

#[test]
fn the_piece_stands_at_its_scope_byte_for_byte_with_its_names_and_its_contract() {
    let dir = library("bytes");
    let p = program(claimed_tower());
    let opts = ExpandOptions::seeded(1).with_pieces(pieces(&dir, &p));
    let e = expand(&p, Box3::at_origin([5, 4, 9]), &opts).unwrap();
    // The bytes at the scope equal the library piece's.
    let mut compared = 0;
    for x in 0..5 {
        for y in 0..4 {
            for z in 0..5 {
                let got = e
                    .model
                    .get([x, y, z + 4])
                    .map_or("minecraft:air".to_string(), |b| b.to_string());
                assert_eq!(got, cell([x, y, z]), "at piece cell {:?}", [x, y, z]);
                compared += 1;
            }
        }
    }
    assert_eq!(compared, 100);
    // The anchors carry the piece's under a mark's naming.
    assert_eq!(e.anchors["anchor/seat"].pos, [2, 1, 7]);
    // The contract holds the piece's space under the claim name, and its
    // exterior face, meeting the yard one cell out, is an edge to it.
    let c = e.contract.as_ref().unwrap();
    assert!(c.spaces.contains_key("tower/room"), "{:?}", c.spaces.keys());
    assert!(
        c.edges
            .iter()
            .any(|x| x.a == "tower/room" && x.b == "yard" && x.class == "walk"),
        "{:?}",
        c.edges
    );
    // The provenance names the piece.
    assert_eq!(e.prefabs.len(), 1);
    assert_eq!(e.prefabs[0].piece, "prefab/cell");
    let report = judge(&e, Default::default());
    let g = report
        .gates
        .iter()
        .find(|g| g.id == "prefab-placed-by-design")
        .unwrap();
    assert!(g.passed(), "{}", g.detail);
    assert_eq!(g.bound, 1);
}

#[test]
fn placed_twice_the_anchors_and_the_prefix_number_in_expansion_order() {
    let dir = library("twice");
    let p = Program::new("pair", "row").rule(
        "row",
        Node::Split(Split {
            axis: Axis::X,
            sizes: vec![Size::abs(5), Size::abs(5)],
            rounding: None,
            repeat: false,
            orient: Default::default(),
            children: vec![
                prefab(PrefabRotation::None),
                prefab(PrefabRotation::Clockwise180),
            ],
        }),
    );
    let opts = ExpandOptions::seeded(1).with_pieces(pieces(&dir, &p));
    let e = expand(&p, Box3::at_origin([10, 4, 5]), &opts).unwrap();
    assert_eq!(e.anchors["anchor/seat-1"].pos, [2, 1, 3]);
    assert_eq!(
        e.anchors["anchor/seat-2"].pos,
        [7, 1, 1],
        "turned a half turn"
    );
    let prefixes: Vec<&str> = e.prefabs.iter().map(|p| p.prefix.as_str()).collect();
    assert_eq!(prefixes, ["cell-1", "cell-2"]);
}

#[test]
fn a_prefab_under_weighted_alternatives_is_refused_naming_the_rule() {
    let dir = library("drawn");
    let p = program(Node::call("tower")).rule_alts(
        "tower",
        vec![
            Alternative::new(claimed_tower()).weight(1),
            Alternative::new(Node::Void).weight(1),
        ],
    );
    let opts = ExpandOptions::seeded(1).with_pieces(pieces(&dir, &p));
    let e = expand(&p, Box3::at_origin([5, 4, 9]), &opts).unwrap();
    let report = judge(&e, Default::default());
    let g = report
        .gates
        .iter()
        .find(|g| g.id == "prefab-placed-by-design")
        .unwrap();
    assert!(!g.passed());
    assert!(
        g.detail
            .contains("rule \"tower\" draws \"prefab/cell\" by weight"),
        "{}",
        g.detail
    );
}

#[test]
fn a_scope_one_cell_short_is_refused_naming_both_sizes() {
    let dir = library("short");
    let p = program(claimed_tower());
    let opts = ExpandOptions::seeded(1).with_pieces(pieces(&dir, &p));
    let err = expand(&p, Box3::at_origin([5, 4, 8]), &opts)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("scope of 5x4x4") && err.contains("the piece is 5x4x5"),
        "{err}"
    );
}

#[test]
fn a_piece_the_library_lacks_is_refused_naming_it() {
    let p = program(claimed_tower());
    let err = expand(&p, Box3::at_origin([5, 4, 9]), &ExpandOptions::seeded(1))
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("\"prefab/cell\"") && err.contains("holds no such piece"),
        "{err}"
    );
}

#[test]
fn grammar_expand_exports_the_piece_with_its_provenance() {
    let dir = library("cli");
    let p = program(claimed_tower());
    let file = dir.join("yard.json");
    std::fs::write(&file, serde_json::to_string_pretty(&p).unwrap()).unwrap();
    let out = dir.join("out");
    let run = Command::new(BIN)
        .args([
            "--prefabs",
            dir.to_str().unwrap(),
            "grammar",
            "expand",
            "--file",
            file.to_str().unwrap(),
            "--region",
            "5x4x9",
            "--seed",
            "1",
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&run.stderr);
    assert!(
        err.contains("prefab-placed-by-design pass  bound 1"),
        "{err}"
    );
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("yard.json")).unwrap()).unwrap();
    let prov = meta["license"]["provenance"].as_str().unwrap();
    assert!(
        prov.contains("prefab/cell") && prov.contains("Written by grammar_prefab_op.rs."),
        "{prov}"
    );
    assert!(meta["anchors"].get("anchor/seat").is_some(), "{meta}");
    let spaces: BTreeMap<String, serde_json::Value> =
        serde_json::from_value(meta["spatial_contract"]["spaces"].clone()).unwrap();
    assert!(spaces.contains_key("tower/room"), "{spaces:?}");
}
