//! **A corpus example: a library piece placed by name** (spec-0106 §3.8) — the
//! one program in the library that writes `prefab`.
//!
//! A `prefab` node places an admitted piece at its scope's box, which must be
//! exactly the piece's size. The piece is a kit part — a small stone cell with
//! a doorway and a seat — and the program lays two of them in a row with a
//! party wall between, the second turned a half turn so both doorways face
//! out. Placed twice, the piece's anchor `kit-seat` arrives as
//! `anchor/kit-seat-1` and `anchor/kit-seat-2` (`index: auto`). Where each
//! cell stands is the split's, never a draw: a `prefab` under weighted
//! alternatives is refused (`prefab-placed-by-design`).
//!
//! The piece is not read from a library on disk: the corpus is self-contained,
//! so the entry hands its expansion the piece it places ([`kit_pieces`]),
//! exactly as `--prefabs` hands a campaign's program the library's.
//!
//! Documented at **11 × 4 × 5, seed 1**.

use std::collections::BTreeMap;

use crate::grammar::block::BlockState;
use crate::grammar::geom::{Axis, Box3};
use crate::grammar::ir::{Node, PrefabRotation, Program};
use crate::grammar::model::VoxelModel;
use crate::grammar::pieces::{Piece, Pieces};

use super::{abs, fill, split};

/// The kit part's id.
pub const KIT_CELL: &str = "prefab/kit-cell";

/// The kit part's size: a 5 × 4 × 5 cell, walls one thick, a floor, a lid.
const SIZE: [i32; 3] = [5, 4, 5];

/// Two kit cells and a party wall.
///
/// Roles: `wall`.
pub fn kit_row() -> Program {
    Program::new("kit_row", "row")
        .role("wall", BlockState::simple("stone_bricks"))
        .rule(
            "row",
            split(
                Axis::X,
                vec![abs(5), abs(1), abs(5)],
                vec![
                    Node::Prefab {
                        piece: KIT_CELL.to_string(),
                        rotation: PrefabRotation::None,
                    },
                    fill("wall"),
                    Node::Prefab {
                        piece: KIT_CELL.to_string(),
                        rotation: PrefabRotation::Clockwise180,
                    },
                ],
            ),
        )
}

/// The kit part: a cell of stone bricks with a doorway two high in its
/// north wall and a seat anchor on its floor.
pub fn kit_cell() -> Piece {
    let mut model = VoxelModel::new(Box3::at_origin([5, 4, 5]));
    let wall = BlockState::simple("stone_bricks");
    let floor = BlockState::simple("polished_andesite");
    for x in 0..SIZE[0] {
        for y in 0..SIZE[1] {
            for z in 0..SIZE[2] {
                let edge = x == 0 || x == SIZE[0] - 1 || z == 0 || z == SIZE[2] - 1;
                let door = z == 0 && x == 2 && (1..=2).contains(&y);
                let block = if y == 0 {
                    Some(&floor)
                } else if y == SIZE[1] - 1 || (edge && !door) {
                    Some(&wall)
                } else {
                    None
                };
                if let Some(b) = block {
                    model.set([x, y, z], b).expect("a two-block palette fits");
                }
            }
        }
    }
    let mut anchors = BTreeMap::new();
    anchors.insert(
        "kit-seat".to_string(),
        delvewright_dsl::prefab::Anchor::point([2, 1, 3], "north"),
    );
    Piece {
        id: KIT_CELL.to_string(),
        size: SIZE,
        hash: format!(
            "sha256:{:x}",
            <sha2::Sha256 as sha2::Digest>::digest(model.canonical_bytes())
        ),
        model,
        anchors,
        contract: None,
        provenance: "Written by crates/delvec/src/grammar/library/kit_row.rs as the corpus's \
                     own kit part."
            .to_string(),
        waterline_y: None,
    }
}

/// The pieces [`kit_row`] places.
pub fn kit_pieces() -> Pieces {
    let mut p = Pieces::none();
    p.insert(kit_cell());
    p
}
