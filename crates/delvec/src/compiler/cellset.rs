//! Copy-on-write cell maps and sets for the navigation model.
//!
//! A [`crate::compiler::nav::World`] holds several cell sets the size of the
//! assembled world (millions of cells on a large campaign), and the proofs
//! derive many short-lived variants of it: one per quest configuration, per
//! sealed gate, per counterfactual. Each variant differs from the world it was
//! derived from by a handful of cells. A [`CellMap`] therefore keeps the large
//! map behind a shared [`Arc`] and records a variant's edits in a small overlay,
//! so deriving a variant costs the size of its edits rather than the size of
//! the world.
//!
//! Iteration is in key order (`[i32; 3]` lexicographic, the order a
//! `BTreeMap<[i32; 3], _>` iterates in): the overlay and the shared map are
//! merged by key, so no caller can observe the layering (ADR-0006).

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, btree_map};
use std::iter::Peekable;
use std::sync::Arc;

type Cell = [i32; 3];

/// A cell→value map whose bulk is shared between copies; see the module docs.
#[derive(Debug)]
pub struct CellMap<V> {
    /// The shared map. Never mutated once shared.
    base: Arc<BTreeMap<Cell, V>>,
    /// This copy's edits: `Some(v)` sets a cell, `None` removes a cell `base`
    /// holds. A key whose overlay entry would restate `base` is not stored.
    over: BTreeMap<Cell, Option<V>>,
    /// The number of cells this map holds.
    len: usize,
}

impl<V> Clone for CellMap<V>
where
    V: Clone,
{
    fn clone(&self) -> Self {
        CellMap {
            base: Arc::clone(&self.base),
            over: self.over.clone(),
            len: self.len,
        }
    }
}

impl<V> Default for CellMap<V> {
    fn default() -> Self {
        CellMap {
            base: Arc::new(BTreeMap::new()),
            over: BTreeMap::new(),
            len: 0,
        }
    }
}

impl<V> From<BTreeMap<Cell, V>> for CellMap<V> {
    fn from(map: BTreeMap<Cell, V>) -> Self {
        CellMap {
            len: map.len(),
            base: Arc::new(map),
            over: BTreeMap::new(),
        }
    }
}

impl<V> CellMap<V>
where
    V: Clone + PartialEq,
{
    /// An empty map.
    pub fn new() -> Self {
        Self::default()
    }

    /// The value at `c`, if the map holds `c`.
    pub fn get(&self, c: &Cell) -> Option<&V> {
        if !self.over.is_empty()
            && let Some(edit) = self.over.get(c)
        {
            return edit.as_ref();
        }
        self.base.get(c)
    }

    /// Whether the map holds `c`.
    pub fn contains_key(&self, c: &Cell) -> bool {
        self.get(c).is_some()
    }

    /// Set `c` to `v`, returning the value it held before.
    pub fn insert(&mut self, c: Cell, v: V) -> Option<V> {
        let before = self.get(&c).cloned();
        if self.base.get(&c) == Some(&v) {
            self.over.remove(&c);
        } else {
            self.over.insert(c, Some(v));
        }
        if before.is_none() {
            self.len += 1;
        }
        before
    }

    /// Remove `c`, returning the value it held.
    pub fn remove(&mut self, c: &Cell) -> Option<V> {
        let before = self.get(c).cloned()?;
        if self.base.contains_key(c) {
            self.over.insert(*c, None);
        } else {
            self.over.remove(c);
        }
        self.len -= 1;
        Some(before)
    }

    /// The number of cells held.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether no cell is held.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Every `(cell, value)`, in cell order.
    pub fn iter(&self) -> Iter<'_, V> {
        Iter {
            base: self.base.iter().peekable(),
            over: self.over.iter().peekable(),
        }
    }

    /// Every cell, in cell order.
    pub fn keys(&self) -> impl Iterator<Item = &Cell> {
        self.iter().map(|(c, _)| c)
    }

    /// Fold this copy's edits into a map of its own, so later copies share
    /// them instead of each carrying them in its overlay.
    pub fn compact(&mut self) {
        if self.over.is_empty() {
            return;
        }
        let map: BTreeMap<Cell, V> = self.iter().map(|(c, v)| (*c, v.clone())).collect();
        *self = CellMap::from(map);
    }
}

impl<V> PartialEq for CellMap<V>
where
    V: Clone + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && self.iter().eq(other.iter())
    }
}

/// The merged, cell-ordered iterator of a [`CellMap`].
pub struct Iter<'a, V> {
    base: Peekable<btree_map::Iter<'a, Cell, V>>,
    over: Peekable<btree_map::Iter<'a, Cell, Option<V>>>,
}

impl<'a, V> Iterator for Iter<'a, V> {
    type Item = (&'a Cell, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let order = match (self.base.peek(), self.over.peek()) {
                (None, None) => return None,
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (Some((b, _)), Some((o, _))) => b.cmp(o),
            };
            if order == Ordering::Less {
                return self.base.next();
            }
            if order == Ordering::Equal {
                self.base.next(); // shadowed by the overlay
            }
            let (c, edit) = self.over.next().expect("peeked");
            if let Some(v) = edit {
                return Some((c, v));
            }
        }
    }
}

/// A cell set whose bulk is shared between copies; see the module docs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CellSet(CellMap<()>);

impl CellSet {
    /// An empty set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the set holds `c`.
    pub fn contains(&self, c: &Cell) -> bool {
        self.0.contains_key(c)
    }

    /// Add `c`; whether it was absent.
    pub fn insert(&mut self, c: Cell) -> bool {
        self.0.insert(c, ()).is_none()
    }

    /// Remove `c`; whether it was present.
    pub fn remove(&mut self, c: &Cell) -> bool {
        self.0.remove(c).is_some()
    }

    /// The number of cells held.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether no cell is held.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Every cell, in cell order.
    pub fn iter(&self) -> impl Iterator<Item = &Cell> {
        self.0.keys()
    }

    /// See [`CellMap::compact`].
    pub fn compact(&mut self) {
        self.0.compact();
    }
}

impl From<BTreeSet<Cell>> for CellSet {
    fn from(set: BTreeSet<Cell>) -> Self {
        CellSet(CellMap::from(
            set.into_iter()
                .map(|c| (c, ()))
                .collect::<BTreeMap<Cell, ()>>(),
        ))
    }
}

impl FromIterator<Cell> for CellSet {
    fn from_iter<I: IntoIterator<Item = Cell>>(iter: I) -> Self {
        CellSet::from(iter.into_iter().collect::<BTreeSet<Cell>>())
    }
}

impl Extend<Cell> for CellSet {
    fn extend<I: IntoIterator<Item = Cell>>(&mut self, iter: I) {
        for c in iter {
            self.insert(c);
        }
    }
}

impl<'a> IntoIterator for &'a CellSet {
    type Item = &'a Cell;
    type IntoIter = std::iter::Map<Iter<'a, ()>, fn((&'a Cell, &'a ())) -> &'a Cell>;

    fn into_iter(self) -> Self::IntoIter {
        fn key<'b>((c, _): (&'b Cell, &'b ())) -> &'b Cell {
            c
        }
        self.0.iter().map(key as fn((&'a Cell, &'a ())) -> &'a Cell)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A copy's edits never reach the original, and both iterate in cell order
    /// exactly as a `BTreeSet` holding the same cells would.
    #[test]
    fn a_copy_edits_alone_and_iterates_in_cell_order() {
        let base: BTreeSet<Cell> = [[0, 0, 0], [0, 1, 0], [2, 0, 0], [1, 5, 5]].into();
        let a = CellSet::from(base.clone());
        let mut b = a.clone();
        assert!(b.remove(&[0, 1, 0]));
        assert!(!b.remove(&[0, 1, 0]));
        assert!(b.insert([1, 0, 0]));
        assert!(!b.insert([2, 0, 0]));
        assert!(b.insert([0, 1, 0]));
        assert!(b.remove(&[0, 1, 0]));
        b.extend([[9, 9, 9], [-1, 0, 0]]);

        let mut want_b = base.clone();
        want_b.remove(&[0, 1, 0]);
        want_b.extend([[1, 0, 0], [9, 9, 9], [-1, 0, 0]]);

        assert_eq!(
            a.iter().copied().collect::<Vec<_>>(),
            base.iter().copied().collect::<Vec<_>>()
        );
        assert_eq!(
            b.iter().copied().collect::<Vec<_>>(),
            want_b.iter().copied().collect::<Vec<_>>()
        );
        assert_eq!(b.len(), want_b.len());
        assert_eq!(a.len(), base.len());
        for c in want_b.iter().chain(base.iter()) {
            assert_eq!(b.contains(c), want_b.contains(c));
            assert_eq!(a.contains(c), base.contains(c));
        }
    }

    /// A map overlay holds a changed value, and restating the shared value
    /// leaves no overlay entry behind.
    #[test]
    fn a_map_overlay_holds_values_and_drops_restatements() {
        let mut m = CellMap::from(BTreeMap::from([([0, 0, 0], 8u8), ([0, 0, 1], 4u8)]));
        let mut n = m.clone();
        assert_eq!(n.insert([0, 0, 0], 2), Some(8));
        assert_eq!(n.get(&[0, 0, 0]), Some(&2));
        assert_eq!(m.get(&[0, 0, 0]), Some(&8));
        assert_eq!(n.insert([0, 0, 0], 8), Some(2));
        assert!(n.over.is_empty());
        assert_eq!(m.remove(&[0, 0, 1]), Some(4));
        assert_eq!(m.len(), 1);
        assert_eq!(
            m.iter().map(|(c, v)| (*c, *v)).collect::<Vec<_>>(),
            vec![([0, 0, 0], 8)]
        );
        assert!(m != n);
    }
}
