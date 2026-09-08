//! **The piece's own waterline, measured from its own bytes** (spec-0060 §4).
//!
//! [`walkplane`](crate::walkplane) is this module's pair: one says where a body
//! stands, this one says where the piece meets a sea, and both are numbers the
//! generator that laid the blocks reads back out of them rather than states.
//!
//! # Why a constant here is the defect the field exists to prevent
//!
//! `waterline_y` is a claim about the bytes — the local y of the piece's top
//! authored water block — and `DW0887` holds a document to that claim. Every
//! generator that wrote the field wrote a **constant**: the island tileset's
//! authoring convention (`2`), copied into two generators and a third's shore
//! lift. A constant is right for exactly the pieces it was measured over on the
//! day it was typed, and a regeneration reinstates it whatever the blocks have
//! since become — which is how a library came to hold three declarations of a
//! waterline over pieces with no water block anywhere in them. Deleting those
//! three by hand and leaving the constant in the generator puts them straight
//! back.
//!
//! # What it answers, and what `None` says
//!
//! The topmost local y holding a `minecraft:water` cell, or `None` when the
//! piece authors none. `None` is a document with **no `waterline_y` key at
//! all**, which is the honest statement for an interior: a piece with no water
//! has no waterline to state, and stating one is `DW0887`.
//!
//! Waterlogging is deliberately not water here, for the reason
//! `delvewright_dsl::blockshape::is_fluid` gives: a `waterlogged=true` stair is
//! a cell occupied by its host block. A piece whose only water is waterlogging
//! authors no free surface for a sea to meet, and the engine's own reader
//! (`compiler::seating::PieceFacts`) counts the same cells, so the number a
//! generator writes and the number `DW0887` checks it against are one
//! measurement taken twice.

use crate::invariants::Cells;

/// The block a waterline is a claim about. Spelled once here and once in the
/// engine's reader, and the two are held equal by `DW0887` refusing any
/// document whose declaration its own bytes do not bear out.
const WATER: &str = "minecraft:water";

/// **The local y of this piece's top authored water block**, or `None` when the
/// piece authors no water at all — the number a generator writes as
/// `waterline_y`, and writes no key for when this is `None`.
///
/// Reported with the cell count it was drawn from by [`census`], so a generator
/// can print what it measured rather than what it intended.
pub fn measure_waterline_y(cells: &Cells) -> Option<i32> {
    census(cells).0
}

/// The measurement and its denominator: the top water y, and how many water
/// cells the piece holds.
///
/// A generator that prints `Some(2)` beside `0 cell(s)` has printed an
/// impossible pair, so the two travel together.
pub fn census(cells: &Cells) -> (Option<i32>, usize) {
    let mut top: Option<i32> = None;
    let mut count = 0usize;
    for (pos, (name, _)) in cells {
        if name == WATER {
            count += 1;
            top = Some(top.map_or(pos[1], |t: i32| t.max(pos[1])));
        }
    }
    (top, count)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn put(cells: &mut Cells, p: [i32; 3], name: &str) {
        cells.insert(p, (name.to_string(), BTreeMap::new()));
    }

    /// The measurement is the TOP water block, not the first one found nor the
    /// bottom of the body — a shore meets the sea at its surface.
    #[test]
    fn the_waterline_is_the_top_water_block() {
        let mut cells = Cells::new();
        put(&mut cells, [0, 0, 0], WATER);
        put(&mut cells, [0, 1, 0], WATER);
        put(&mut cells, [1, 2, 0], WATER);
        put(&mut cells, [2, 5, 0], "minecraft:stone");
        assert_eq!(census(&cells), (Some(2), 3));
    }

    /// A piece with no water has no waterline, and the generator writes no key.
    #[test]
    fn no_water_is_no_waterline() {
        let mut cells = Cells::new();
        put(&mut cells, [0, 0, 0], "minecraft:stone");
        assert_eq!(census(&cells), (None, 0));
    }

    /// A waterlogged host block is its host: it authors no free surface, so it
    /// is not a waterline. The engine's `DW0887` reader counts the same cells,
    /// and a generator that counted these would write a number that check
    /// refuses.
    #[test]
    fn waterlogging_is_not_a_waterline() {
        let mut cells = Cells::new();
        cells.insert(
            [0, 3, 0],
            (
                "minecraft:oak_stairs".to_string(),
                BTreeMap::from([("waterlogged".to_string(), "true".to_string())]),
            ),
        );
        assert_eq!(census(&cells), (None, 0));
    }
}
