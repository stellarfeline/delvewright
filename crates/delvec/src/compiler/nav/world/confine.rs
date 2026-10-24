//! **The shared confinement**: what keeps a body in part of a [`World`] while
//! [`World::body_flood`] walks the reach graph, stated as one of three shapes
//! every confined reader takes.
//!
//! - **The box** of one piece ([`World::of_piece`]): no move leads out of it.
//!   It rides on the world, so no reader can forget it.
//! - **Walls** ([`World::walls_over`]): floor taken out of a body's reach is
//!   walled up in the world the flood runs over, because an arc that lands on
//!   kept floor flies over left-out floor without standing in it.
//! - **A licence** ([`Licence`]): the cells are split into regions, and a move
//!   from one region into another is taken only where a declared relation
//!   between the two names it — a walk either way it is declared, a fall only
//!   in the direction a drop is declared. A wall cannot say this: a wall is
//!   one cell and has no direction, and the question is about the pair of
//!   regions a move joins and whether the move falls.
//!
//! [`Confinement`] carries the cells a body may rest in and, when there is
//! one, the licence: the only per-move rule a flood may state, so no reader
//! keeps an admission of its own beside the graph's.

use super::World;
use std::collections::{BTreeMap, BTreeSet};

/// **A declared relation between regions** of a world: which region a cell
/// belongs to, and which ordered pairs of regions a body may move between, by
/// a walk and by a fall. A move inside one region is always licensed; a move
/// from a cell of no region, or into one, never is.
///
/// A move **falls** when it ends more than one course below where it began:
/// a fall across regions needs a fall licence (a `drop`'s), and a walk
/// licence does not cover it.
#[derive(Debug, Clone, Default)]
pub struct Licence {
    /// Cell → the regions holding it, in region order.
    of_cell: BTreeMap<[i32; 3], Vec<usize>>,
    /// Ordered pairs a walk is licensed between.
    walk: BTreeSet<(usize, usize)>,
    /// Ordered pairs a fall is licensed between.
    fall: BTreeSet<(usize, usize)>,
}

impl Licence {
    /// A licence over `regions` (region `i` is `regions[i]`) that licenses
    /// the `walk` pairs for any move and the `fall` pairs for a fall.
    pub fn new(
        regions: &[&BTreeSet<[i32; 3]>],
        walk: impl IntoIterator<Item = (usize, usize)>,
        fall: impl IntoIterator<Item = (usize, usize)>,
    ) -> Licence {
        let mut of_cell: BTreeMap<[i32; 3], Vec<usize>> = BTreeMap::new();
        for (id, cells) in regions.iter().enumerate() {
            for &c in cells.iter() {
                of_cell.entry(c).or_default().push(id);
            }
        }
        Licence {
            of_cell,
            walk: walk.into_iter().collect(),
            fall: fall.into_iter().collect(),
        }
    }

    /// Whether a move from `from` to `to` is licensed: some region of `from`
    /// and some region of `to` are one region, or are a pair the move's kind
    /// is licensed between.
    pub fn admits(&self, from: [i32; 3], to: [i32; 3]) -> bool {
        let (Some(a), Some(b)) = (self.of_cell.get(&from), self.of_cell.get(&to)) else {
            return false;
        };
        let falling = from[1] - to[1] > 1;
        let pairs = if falling { &self.fall } else { &self.walk };
        a.iter()
            .any(|&f| b.iter().any(|&t| f == t || pairs.contains(&(f, t))))
    }
}

/// **What keeps a [`World::body_flood`] in part of the world**: the cells a
/// body may come to rest in, and the licence between regions when there is
/// one. The piece's box rides on the world and walls are laid in it, so
/// neither is stated here.
#[derive(Clone, Copy)]
pub struct Confinement<'a> {
    keep: Option<&'a dyn Fn([i32; 3]) -> bool>,
    licence: Option<&'a Licence>,
}

impl<'a> Confinement<'a> {
    /// No confinement: every move of the graph is taken.
    pub fn none() -> Confinement<'static> {
        Confinement {
            keep: None,
            licence: None,
        }
    }

    /// A body comes to rest only in a cell `keep` holds.
    pub fn cells(keep: &'a dyn Fn([i32; 3]) -> bool) -> Confinement<'a> {
        Confinement {
            keep: Some(keep),
            licence: None,
        }
    }

    /// A body comes to rest only in a cell `keep` holds, and crosses between
    /// regions only where `licence` admits the move.
    pub fn licensed(keep: &'a dyn Fn([i32; 3]) -> bool, licence: &'a Licence) -> Confinement<'a> {
        Confinement {
            keep: Some(keep),
            licence: Some(licence),
        }
    }

    /// Whether the move from `from` to `to` is taken.
    pub(in crate::compiler::nav) fn admits(&self, from: [i32; 3], to: [i32; 3]) -> bool {
        self.keep.is_none_or(|k| k(to)) && self.licence.is_none_or(|l| l.admits(from, to))
    }
}

impl World {
    /// [`World::body_flood`] from each cell of `cells` in turn, kept to
    /// `cells`: the cells of `cells` a body in which reaches some cell of
    /// `goal` without coming to rest outside `cells`. The cells of `goal`
    /// that are in `cells` are in it.
    pub fn cells_reaching(
        &self,
        cells: &BTreeSet<[i32; 3]>,
        goal: &BTreeSet<[i32; 3]>,
    ) -> BTreeSet<[i32; 3]> {
        let keep = |c: [i32; 3]| cells.contains(&c);
        let mut out: BTreeSet<[i32; 3]> = BTreeSet::new();
        for &c in cells {
            if goal.contains(&c)
                || !self
                    .body_flood(&[c], &Confinement::cells(&keep))
                    .is_disjoint(goal)
            {
                out.insert(c);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A drop is licensed one way: a fall from the high region into the low
    /// one is admitted, the climb back is not, and a fall the walk licence
    /// alone covers is not.
    #[test]
    fn a_licence_takes_a_fall_only_where_a_drop_is_declared() {
        let high: BTreeSet<[i32; 3]> = BTreeSet::from([[0, 5, 0]]);
        let low: BTreeSet<[i32; 3]> = BTreeSet::from([[1, 1, 0]]);
        let beside: BTreeSet<[i32; 3]> = BTreeSet::from([[1, 5, 0]]);
        let dropped = Licence::new(&[&high, &low, &beside], [(0, 2), (2, 0)], [(0, 1)]);
        assert!(dropped.admits([0, 5, 0], [1, 1, 0]), "the drop falls");
        assert!(!dropped.admits([1, 1, 0], [0, 5, 0]), "no way back up");
        assert!(dropped.admits([0, 5, 0], [1, 5, 0]), "the walk either way");
        assert!(dropped.admits([1, 5, 0], [0, 5, 0]));
        let walked = Licence::new(&[&high, &low, &beside], [(0, 1), (1, 0)], []);
        assert!(
            !walked.admits([0, 5, 0], [1, 1, 0]),
            "a walk licence does not cover a fall of four"
        );
        assert!(!walked.admits([0, 5, 0], [9, 9, 9]), "no region, no move");
        assert!(walked.admits([0, 5, 0], [0, 5, 0]), "inside one region");
    }
}
