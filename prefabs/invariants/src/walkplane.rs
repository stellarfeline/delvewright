//! **The piece's own walk plane, measured from its own bytes** (spec-0060 §4).
//!
//! One module because one number: `walk_y` is written by every generator, read
//! by the seating derivation, and refused by `DW0886` when it is absent — and a
//! measurement taken seven times is seven measurements.

use crate::invariants::{blockshape, Cells};

/// **The local y of this piece's walk plane** — the number the prefab document
/// declares as `walk_y`, and the number a walk-plane horizon derives an area's
/// origin from (spec-0060 §3.2).
///
/// # Why this is measured and not typed
///
/// `walk_y` has no default, because a default is the retired global ocean datum
/// wearing a different name: right for the one tileset it was copied from and
/// silently wrong for every other, with the piece that lands under the sea
/// flooding on boot and nothing looking. What replaces the default is not a
/// per-tileset constant either — a constant is a hand-written census of an
/// object that can be read, and it drifts the first time a piece changes a
/// course. So the generator that just laid the blocks reads the answer back out
/// of them.
///
/// # The definition, stated once
///
/// The walk plane is **the lowest local y that holds a standable cell**: a cell
/// a body's feet can occupy, meaning the cell and the one above it pass a body
/// and the cell beneath supports one. It is the plane a body first stands on
/// when it is put down inside the piece, which is exactly the relationship a
/// horizon's datum is about — one block above the sea, or on the valley's gap
/// floor.
///
/// An absent cell is air (`/place template` leaves what the piece does not
/// write), so walking `size` rather than the written cells is not a
/// convenience: a piece's topmost floor with open sky above it is standable,
/// and a walk restricted to written cells would miss the air the body stands
/// in.
///
/// # It refuses rather than reporting a number it did not measure
///
/// A piece with no standable cell anywhere has no walk plane, and returning
/// some number for it would be inventing the measurement this function exists
/// to take. That is a generator defect — a solid block, or a piece whose every
/// floor is fluid — so it panics, which is how every gate in this crate
/// reports.
pub fn measure_walk_y(id: &str, size: [i32; 3], cells: &Cells) -> i32 {
    let (standable, examined) = scan(size, cells);
    assert!(
        examined > 0,
        "{id}: the walk-plane measurement examined ZERO cells — a piece of extent {size:?} has \
         no interior for a body to stand in, so `walk_y` would be invented rather than measured"
    );
    standable.unwrap_or_else(|| {
        panic!(
            "{id}: no standable cell anywhere in {examined} cell(s) of extent {size:?}, so this \
             piece has no walk plane to declare. A body's feet need a cell that passes a body, \
             open above, over a block that supports one; a piece that offers none is solid, \
             flooded, or floored in something a body falls through"
        )
    })
}

/// **The same measurement, for a producer that writes pieces a body cannot
/// stand in** — a solid fragment source, a block of material a later step cuts
/// from.
///
/// `None` says the piece has no walk plane, and a generator that gets it writes
/// no `walk_y` key. That is the honest document rather than a convenience: a
/// piece with no walk plane cannot be seated on a horizon that derives an
/// origin from one, and `DW0886` is where a campaign that tries learns it.
/// Every generator that writes a piece a party walks in uses
/// [`measure_walk_y`], which refuses instead.
pub fn walk_y(size: [i32; 3], cells: &Cells) -> Option<i32> {
    scan(size, cells).0
}

/// The scan behind [`measure_walk_y`]: the lowest standable local y, and how
/// many cells were examined.
///
/// Returned as a pair so the refusal above is stated over a number rather than
/// over a feeling.
fn scan(size: [i32; 3], cells: &Cells) -> (Option<i32>, usize) {
    let name_at = |p: [i32; 3]| cells.get(&p).map(|(n, _)| n.as_str());
    // An absent cell is air: it passes a body and supports nothing.
    let passes = |p: [i32; 3]| name_at(p).is_none_or(blockshape::passes_body);
    let supports = |p: [i32; 3]| name_at(p).is_some_and(blockshape::supports_body);
    let mut examined = 0usize;
    for y in 0..size[1] {
        let mut found = false;
        for x in 0..size[0] {
            for z in 0..size[2] {
                examined += 1;
                if supports([x, y - 1, z]) && passes([x, y, z]) && passes([x, y + 1, z]) {
                    found = true;
                }
            }
        }
        if found {
            return (Some(y), examined);
        }
    }
    (None, examined)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn stone(cells: &mut Cells, p: [i32; 3]) {
        cells.insert(p, ("minecraft:stone".to_string(), BTreeMap::new()));
    }

    /// A room whose floor course is local y=0 stands its body at y=1, and that
    /// is the number the document declares.
    #[test]
    fn a_floor_at_zero_walks_at_one() {
        let mut cells = Cells::new();
        for x in 0..3 {
            for z in 0..3 {
                stone(&mut cells, [x, 0, z]);
            }
        }
        assert_eq!(measure_walk_y("room", [3, 4, 3], &cells), 1);
    }

    /// The gallery's shore lift: two courses of plinth under the same room put
    /// the walk plane at 3, which is the island convention read forwards and
    /// one block above a waterline of 2.
    #[test]
    fn a_plinth_lifts_the_walk_plane_by_its_own_height() {
        let mut cells = Cells::new();
        for x in 0..3 {
            for z in 0..3 {
                for y in 0..3 {
                    stone(&mut cells, [x, y, z]);
                }
            }
        }
        assert_eq!(measure_walk_y("shore", [3, 6, 3], &cells), 3);
    }

    /// The measurement takes the LOWEST standable plane, so a cellar under a
    /// hall is the walk plane rather than the hall floor — which is the fact
    /// that makes a cellar visible to a sea rather than hidden from it.
    #[test]
    fn a_cellar_is_the_walk_plane_not_the_storey_above_it() {
        let mut cells = Cells::new();
        for x in 0..3 {
            for z in 0..3 {
                stone(&mut cells, [x, 0, z]);
                stone(&mut cells, [x, 3, z]);
            }
        }
        assert_eq!(measure_walk_y("cellar", [3, 8, 3], &cells), 1);
    }

    /// A piece with no standable cell has no walk plane, and inventing one is
    /// the failure this refuses.
    #[test]
    #[should_panic(expected = "no standable cell")]
    fn a_solid_block_has_no_walk_plane() {
        let mut cells = Cells::new();
        for x in 0..2 {
            for y in 0..2 {
                for z in 0..2 {
                    stone(&mut cells, [x, y, z]);
                }
            }
        }
        measure_walk_y("solid", [2, 2, 2], &cells);
    }
}
