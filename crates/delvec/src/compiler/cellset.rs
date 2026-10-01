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

impl<V> CellMap<V> {
    /// A map whose shared part is `base`, with no edits of its own.
    pub fn from_shared(base: Arc<BTreeMap<Cell, V>>) -> Self {
        CellMap {
            len: base.len(),
            base,
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
///
/// The shared part is a bitset over its cells' bounding box when the cells
/// fill enough of that box ([`DENSE_CELLS_PER_BIT`]), and a `BTreeSet`
/// otherwise. Either way it iterates in cell order: a bit's index within the
/// box is `((x - x0) * ny + (y - y0)) * nz + (z - z0)`, which orders cells
/// exactly as `[i32; 3]` compares.
#[derive(Debug, Clone)]
pub struct CellSet {
    base: Arc<Base>,
    /// This copy's edits: `true` adds a cell `base` lacks, `false` removes one
    /// `base` holds. A key whose edit would restate `base` is not stored.
    over: BTreeMap<Cell, bool>,
    len: usize,
}

/// The shared part of a [`CellSet`].
#[derive(Debug)]
enum Base {
    Sparse(BTreeSet<Cell>),
    Dense(Bits),
}

/// A set takes the dense form when its bounding box holds at most this many
/// cells per member: at one bit per box cell that is a quarter of the 32
/// bytes a `BTreeSet` spends per member before its node overhead.
const DENSE_CELLS_PER_BIT: usize = 64;

/// A bitset over an inclusive box.
#[derive(Debug)]
struct Bits {
    lo: Cell,
    /// Box extent along x, y, z.
    dims: [usize; 3],
    words: Vec<u64>,
}

impl Bits {
    /// The bit index of `c`, or `None` outside the box.
    #[inline]
    fn index(&self, c: &Cell) -> Option<usize> {
        let mut idx = 0usize;
        for ((&v, &lo), &dim) in c.iter().zip(&self.lo).zip(&self.dims) {
            let d = i64::from(v) - i64::from(lo);
            if d < 0 || d as u64 >= dim as u64 {
                return None;
            }
            idx = idx * dim + d as usize;
        }
        Some(idx)
    }

    #[inline]
    fn contains(&self, c: &Cell) -> bool {
        self.index(c)
            .is_some_and(|i| self.words[i / 64] & (1u64 << (i % 64)) != 0)
    }

    fn cell(&self, idx: usize) -> Cell {
        let z = idx % self.dims[2];
        let rest = idx / self.dims[2];
        let y = rest % self.dims[1];
        let x = rest / self.dims[1];
        [
            self.lo[0] + x as i32,
            self.lo[1] + y as i32,
            self.lo[2] + z as i32,
        ]
    }
}

impl Base {
    fn of(set: BTreeSet<Cell>) -> Base {
        let (Some(first), Some(last)) = (set.first(), set.last()) else {
            return Base::Sparse(set);
        };
        // x is the leading key, so the first and last cells bound x; y and z
        // need the whole scan.
        let mut lo = [first[0], i32::MAX, i32::MAX];
        let mut hi = [last[0], i32::MIN, i32::MIN];
        for c in &set {
            for a in 1..3 {
                lo[a] = lo[a].min(c[a]);
                hi[a] = hi[a].max(c[a]);
            }
        }
        let dims = [0, 1, 2].map(|a| (i64::from(hi[a]) - i64::from(lo[a]) + 1) as u64);
        let volume = dims[0]
            .checked_mul(dims[1])
            .and_then(|v| v.checked_mul(dims[2]));
        match volume {
            Some(v) if v <= (set.len() as u64).saturating_mul(DENSE_CELLS_PER_BIT as u64) => {
                let mut bits = Bits {
                    lo,
                    dims: dims.map(|d| d as usize),
                    words: vec![0u64; (v as usize).div_ceil(64)],
                };
                for c in &set {
                    let i = bits.index(c).expect("inside its own bounding box");
                    bits.words[i / 64] |= 1u64 << (i % 64);
                }
                Base::Dense(bits)
            }
            _ => Base::Sparse(set),
        }
    }

    #[inline]
    fn contains(&self, c: &Cell) -> bool {
        match self {
            Base::Sparse(s) => s.contains(c),
            Base::Dense(b) => b.contains(c),
        }
    }

    fn len(&self) -> usize {
        match self {
            Base::Sparse(s) => s.len(),
            Base::Dense(b) => b.words.iter().map(|w| w.count_ones() as usize).sum(),
        }
    }

    fn iter(&self) -> BaseIter<'_> {
        match self {
            Base::Sparse(s) => BaseIter::Sparse(s.iter()),
            Base::Dense(b) => BaseIter::Dense {
                bits: b,
                word: 0,
                cur: b.words.first().copied().unwrap_or(0),
            },
        }
    }
}

/// The cell-ordered iterator of a [`Base`].
enum BaseIter<'a> {
    Sparse(std::collections::btree_set::Iter<'a, Cell>),
    Dense {
        bits: &'a Bits,
        word: usize,
        cur: u64,
    },
}

impl Iterator for BaseIter<'_> {
    type Item = Cell;

    fn next(&mut self) -> Option<Cell> {
        match self {
            BaseIter::Sparse(it) => it.next().copied(),
            BaseIter::Dense { bits, word, cur } => {
                while *cur == 0 {
                    *word += 1;
                    *cur = *bits.words.get(*word)?;
                }
                let bit = cur.trailing_zeros() as usize;
                *cur &= *cur - 1;
                Some(bits.cell(*word * 64 + bit))
            }
        }
    }
}

impl Default for CellSet {
    fn default() -> Self {
        CellSet {
            base: Arc::new(Base::Sparse(BTreeSet::new())),
            over: BTreeMap::new(),
            len: 0,
        }
    }
}

impl PartialEq for CellSet {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && self.iter().eq(other.iter())
    }
}

impl CellSet {
    /// An empty set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the set holds `c`.
    #[inline]
    pub fn contains(&self, c: &Cell) -> bool {
        if !self.over.is_empty()
            && let Some(present) = self.over.get(c)
        {
            return *present;
        }
        self.base.contains(c)
    }

    /// Add `c`; whether it was absent.
    pub fn insert(&mut self, c: Cell) -> bool {
        if self.contains(&c) {
            return false;
        }
        if self.base.contains(&c) {
            self.over.remove(&c);
        } else {
            self.over.insert(c, true);
        }
        self.len += 1;
        true
    }

    /// Remove `c`; whether it was present.
    pub fn remove(&mut self, c: &Cell) -> bool {
        if !self.contains(c) {
            return false;
        }
        if self.base.contains(c) {
            self.over.insert(*c, false);
        } else {
            self.over.remove(c);
        }
        self.len -= 1;
        true
    }

    /// The number of cells held.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether no cell is held.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Every cell, in cell order.
    pub fn iter(&self) -> SetIter<'_> {
        SetIter {
            base: self.base.iter().peekable(),
            over: self.over.iter().peekable(),
        }
    }

    /// Fold this copy's edits into a shared part of its own, so later copies
    /// share them instead of each carrying them in its overlay.
    pub fn compact(&mut self) {
        if self.over.is_empty() {
            return;
        }
        *self = CellSet::from(self.iter().collect::<BTreeSet<Cell>>());
    }
}

/// The merged, cell-ordered iterator of a [`CellSet`].
pub struct SetIter<'a> {
    base: Peekable<BaseIter<'a>>,
    over: Peekable<btree_map::Iter<'a, Cell, bool>>,
}

impl Iterator for SetIter<'_> {
    type Item = Cell;

    fn next(&mut self) -> Option<Cell> {
        loop {
            let order = match (self.base.peek(), self.over.peek()) {
                (None, None) => return None,
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (Some(b), Some((o, _))) => b.cmp(o),
            };
            if order == Ordering::Less {
                return self.base.next();
            }
            if order == Ordering::Equal {
                self.base.next(); // shadowed by the overlay
            }
            let (c, present) = self.over.next().expect("peeked");
            if *present {
                return Some(*c);
            }
        }
    }
}

impl From<BTreeSet<Cell>> for CellSet {
    fn from(set: BTreeSet<Cell>) -> Self {
        let base = Base::of(set);
        CellSet {
            len: base.len(),
            base: Arc::new(base),
            over: BTreeMap::new(),
        }
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
    type Item = Cell;
    type IntoIter = SetIter<'a>;

    fn into_iter(self) -> SetIter<'a> {
        self.iter()
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
            a.iter().collect::<Vec<_>>(),
            base.iter().copied().collect::<Vec<_>>()
        );
        assert_eq!(
            b.iter().collect::<Vec<_>>(),
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

    /// A set that fills its box takes the dense form, and a copy of it with
    /// edits iterates, counts and answers membership exactly as a `BTreeSet`
    /// holding the same cells, including at the box edges and outside it.
    #[test]
    fn the_dense_form_reads_as_the_set_it_holds() {
        let mut want: BTreeSet<Cell> = BTreeSet::new();
        for x in -3..4 {
            for y in -2..3 {
                for z in 60..66 {
                    if (x * 7 + y * 3 + z) % 5 != 0 {
                        want.insert([x, y, z]);
                    }
                }
            }
        }
        let set = CellSet::from(want.clone());
        assert!(matches!(*set.base, Base::Dense(_)), "dense form taken");
        let sparse = CellSet::from(BTreeSet::from([[0, 0, 0], [1000, 1000, 1000]]));
        assert!(matches!(*sparse.base, Base::Sparse(_)), "sparse form kept");

        let mut copy = set.clone();
        let mut want_copy = want.clone();
        for c in [
            [-3, -2, 60],
            [3, 2, 65],
            [0, 0, 62],
            [9, 9, 9],
            [-9, 0, 61],
            [0, 0, 60],
        ] {
            assert_eq!(copy.remove(&c), want_copy.remove(&c), "remove {c:?}");
            assert_eq!(
                copy.insert([c[0], c[1] + 10, c[2]]),
                want_copy.insert([c[0], c[1] + 10, c[2]])
            );
        }
        for (s, w) in [(&set, &want), (&copy, &want_copy)] {
            assert_eq!(
                s.iter().collect::<Vec<_>>(),
                w.iter().copied().collect::<Vec<_>>()
            );
            assert_eq!(s.len(), w.len());
            for x in -5..6 {
                for y in -4..15 {
                    for z in 58..68 {
                        assert_eq!(s.contains(&[x, y, z]), w.contains(&[x, y, z]));
                    }
                }
            }
        }
        let mut compacted = copy.clone();
        compacted.compact();
        assert!(compacted.over.is_empty());
        assert!(compacted == copy);
    }
}
