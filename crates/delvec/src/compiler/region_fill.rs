//! **What one region write of a block leaves in each cell** — the one
//! derivation every writer and every model of a region write asks.
//!
//! A region write names one block state for a box: a gate's declared block
//! (the world-load seal, `close-gate`, a timed gate's closing edge, a
//! shortcut's seal), a `fill-region`'s block, a laid way's block. Most blocks
//! are one cell, so the box holds that state in every cell. A **two-high
//! block** is two states: vanilla's `DoubleBlockHalf` property, spelled
//! `half=lower` / `half=upper` (a door, a tall plant), whose upper half
//! survives only over its own lower half and whose lower half only under its
//! upper. A box written with one leaves its halves **by course**: the box's
//! lowest course the lower half, the course above it the upper half, and so on
//! alternately. Every other property — a door's `facing`, `hinge`, `open`,
//! `powered` — is the declared state's, in both halves, so the two halves of
//! one door agree. Nothing per cell is written by hand: the declared block and
//! the box decide every cell.
//!
//! A state with `half=top` / `half=bottom` (a stair, a slab-like trapdoor) is
//! one cell — the property names where in its cell the block sits — and is
//! untouched.
//!
//! Everything that writes or models a region write takes its cells here:
//! [`state_at`] answers one cell, [`cells`] walks the box, and [`boxes`] cuts
//! it into the uniform boxes one `fill` each can write. A block that is not
//! two-high is one box, the region verbatim, so every emitted command for it
//! is byte-identical to a single `fill` of the declared block.

use std::borrow::Cow;

/// An inclusive world box, any corner order.
pub type Region = ([i32; 3], [i32; 3]);

/// The byte range of the `half` value in `block`'s state list, when that value
/// is `lower` or `upper` — the two halves of a two-high block.
fn half_value(block: &str) -> Option<(usize, usize)> {
    let open = block.find('[')?;
    let close = open + block[open..].find(']')?;
    let mut at = open + 1;
    for pair in block[open + 1..close].split(',') {
        let start = at;
        at += pair.len() + 1;
        let Some((k, v)) = pair.split_once('=') else {
            continue;
        };
        if k.trim() == "half" && matches!(v.trim(), "lower" | "upper") {
            let v_start = start + pair.find('=')? + 1;
            let lead = v.len() - v.trim_start().len();
            let v_start = v_start + lead;
            return Some((v_start, v_start + v.trim().len()));
        }
    }
    None
}

/// Whether `block` is a two-high block: its state names a `half` of `lower`
/// or `upper`.
#[must_use]
pub fn two_high(block: &str) -> bool {
    half_value(block).is_some()
}

/// The block one region write of `block` over `region` (inclusive, any corner
/// order) leaves at `cell`: the declared state, with a two-high block's half
/// set by the cell's course above the region's floor.
#[must_use]
pub fn state_at<'a>(region: ([i32; 3], [i32; 3]), block: &'a str, cell: [i32; 3]) -> Cow<'a, str> {
    let Some((s, e)) = half_value(block) else {
        return Cow::Borrowed(block);
    };
    let floor = region.0[1].min(region.1[1]);
    let half = if (cell[1] - floor).rem_euclid(2) == 0 {
        "lower"
    } else {
        "upper"
    };
    if &block[s..e] == half {
        Cow::Borrowed(block)
    } else {
        Cow::Owned(format!("{}{half}{}", &block[..s], &block[e..]))
    }
}

/// Every cell of `region` with the block one write of `block` leaves there, in
/// [`crate::compiler::assembled::region_cells`] order.
pub fn cells<'a>(
    region: ([i32; 3], [i32; 3]),
    block: &'a str,
) -> impl Iterator<Item = ([i32; 3], Cow<'a, str>)> + 'a {
    crate::compiler::assembled::region_cells(region.0, region.1)
        .map(move |c| (c, state_at(region, block, c)))
}

/// `region` cut into the boxes one `fill` each writes, with the state each
/// holds: the region verbatim (its corner order kept) for a block that is not
/// two-high, else one box per course, lowest first, each keeping the region's
/// `x`/`z` corner order.
#[must_use]
pub fn boxes<'a>(
    region: ([i32; 3], [i32; 3]),
    block: &'a str,
) -> Vec<(Region, Cow<'a, str>)> {
    if !two_high(block) {
        return vec![(region, Cow::Borrowed(block))];
    }
    let (a, b) = region;
    (a[1].min(b[1])..=a[1].max(b[1]))
        .map(|y| {
            let course = ([a[0], y, a[2]], [b[0], y, b[2]]);
            (course, state_at(region, block, course.0))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOOR: &str =
        "minecraft:iron_door[facing=south,half=lower,hinge=left,open=false,powered=false]";
    const DOOR_TOP: &str =
        "minecraft:iron_door[facing=south,half=upper,hinge=left,open=false,powered=false]";

    /// A door over a two-high doorway is its lower half in the lower course and
    /// its upper half in the upper, every other property the declared state's.
    #[test]
    fn a_door_lays_its_halves_by_course() {
        let r = ([4, 1, 6], [5, 2, 6]);
        let got: Vec<([i32; 3], String)> =
            cells(r, DOOR).map(|(c, s)| (c, s.into_owned())).collect();
        assert_eq!(
            got,
            vec![
                ([4, 1, 6], DOOR.to_string()),
                ([5, 1, 6], DOOR.to_string()),
                ([4, 2, 6], DOOR_TOP.to_string()),
                ([5, 2, 6], DOOR_TOP.to_string()),
            ]
        );
        // Declared as its upper half, it is still derived from the floor up.
        assert_eq!(state_at(r, DOOR_TOP, [4, 1, 6]), DOOR);
        assert_eq!(
            boxes(r, DOOR)
                .into_iter()
                .map(|(b, s)| (b, s.into_owned()))
                .collect::<Vec<_>>(),
            vec![
                (([4, 1, 6], [5, 1, 6]), DOOR.to_string()),
                (([4, 2, 6], [5, 2, 6]), DOOR_TOP.to_string()),
            ]
        );
        // Corner order does not move the floor.
        assert_eq!(state_at(([5, 2, 6], [4, 1, 6]), DOOR, [4, 2, 6]), DOOR_TOP);
    }

    /// A block that is not two-high is one box, the region verbatim, and the
    /// same state in every cell: a stair's or a trapdoor's `half` names where in
    /// its one cell it sits.
    #[test]
    fn every_other_block_is_one_box_of_its_declared_state() {
        for block in [
            "minecraft:iron_bars[east=false,north=false,south=false,waterlogged=false,west=false]",
            "minecraft:iron_bars",
            "minecraft:iron_trapdoor[facing=north,half=top,open=false,powered=false,waterlogged=false]",
            "minecraft:stone_brick_stairs[facing=east,half=bottom,shape=straight,waterlogged=false]",
            "minecraft:air",
        ] {
            let r = ([5, 3, 0], [0, 0, 0]);
            assert!(!two_high(block), "{block}");
            assert_eq!(boxes(r, block), vec![(r, Cow::Borrowed(block))]);
            assert!(cells(r, block).all(|(_, s)| s == block), "{block}");
        }
        assert!(two_high("minecraft:tall_grass[half=upper]"));
    }
}
