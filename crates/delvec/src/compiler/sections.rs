//! **The section store** (ADR-0037 §1): a cell→value map held as 16 × 16 × 16
//! sections, each a palette of the values it holds and one index per cell.
//!
//! The assembled world names a block in every placed cell — millions on a
//! large campaign — and a `BTreeMap<[i32; 3], _>` spends some forty bytes per
//! cell on the key, the value and the tree's own nodes. A section spends one
//! byte per cell (two once it holds more than 255 distinct values), and a
//! section nothing wrote costs nothing: the store's size is the volume
//! written, not the number of cells.
//!
//! The store reads as the map it replaces: `get`, `insert` and `remove`
//! answer what a `BTreeMap` answers, and iteration is in key order (`[i32; 3]`
//! lexicographic, the order a `BTreeMap<[i32; 3], _>` iterates in), so no
//! consumer can observe the sectioning and no emitted byte moves (ADR-0006).
//! [`SectionMap::in_box`] walks one inclusive box in the same order. The
//! agreement with a map is held by `a_section_store_answers_what_a_map_answers`
//! below, on generated edit sequences across section borders, negative
//! coordinates and palettes past the narrow index.

use std::collections::{BTreeMap, btree_map};

/// A cell: `[x, y, z]`.
type Cell = [i32; 3];

/// A section's side, in cells.
const SIDE: i32 = 16;
/// Cells in a section.
const CELLS: usize = 4096;
/// The widest palette a narrow (one-byte) index addresses: index `0` is
/// "absent", so 255 values.
const NARROW_PALETTE: usize = 255;

/// A cell→value map stored by section; see the module docs.
pub struct SectionMap<V> {
    /// Sections by section coordinate `[sx, sy, sz]`, each holding at least
    /// one cell: a section emptied by removals is dropped.
    sections: BTreeMap<Cell, Section<V>>,
    /// The number of cells held, over every section.
    len: usize,
}

/// One section: a palette and one index per cell into it (`0`: absent;
/// `i`: `palette[i - 1]`).
#[derive(Clone)]
struct Section<V> {
    palette: Vec<V>,
    idx: Idx,
    /// Cells held: indices that are not `0`.
    count: u32,
}

/// A section's indices: one byte per cell while the palette fits, two after.
#[derive(Clone)]
enum Idx {
    Narrow(Box<[u8; CELLS]>),
    Wide(Box<[u16; CELLS]>),
}

impl Idx {
    #[inline]
    fn get(&self, at: usize) -> usize {
        match self {
            Idx::Narrow(a) => usize::from(a[at]),
            Idx::Wide(a) => usize::from(a[at]),
        }
    }

    #[inline]
    fn set(&mut self, at: usize, to: usize) {
        match self {
            Idx::Narrow(a) => a[at] = u8::try_from(to).expect("a narrow index fits its palette"),
            Idx::Wide(a) => a[at] = u16::try_from(to).expect("a wide index fits its palette"),
        }
    }

    fn widen(&mut self) {
        if let Idx::Narrow(a) = self {
            let mut w = Box::new([0u16; CELLS]);
            for (to, from) in w.iter_mut().zip(a.iter()) {
                *to = u16::from(*from);
            }
            *self = Idx::Wide(w);
        }
    }

    /// Bytes this index array holds.
    fn bytes(&self) -> usize {
        match self {
            Idx::Narrow(_) => CELLS,
            Idx::Wide(_) => CELLS * 2,
        }
    }
}

/// The section a cell lies in.
#[inline]
fn section_of(c: &Cell) -> Cell {
    [
        c[0].div_euclid(SIDE),
        c[1].div_euclid(SIDE),
        c[2].div_euclid(SIDE),
    ]
}

/// The index of a cell within its section: `x` leading, then `y`, then `z`,
/// so that the index orders a section's cells exactly as `[i32; 3]` compares.
#[inline]
fn local_of(c: &Cell) -> usize {
    let lx = c[0].rem_euclid(SIDE) as usize;
    let ly = c[1].rem_euclid(SIDE) as usize;
    let lz = c[2].rem_euclid(SIDE) as usize;
    (lx * 16 + ly) * 16 + lz
}

impl<V: Copy + Eq> Section<V> {
    fn new() -> Self {
        Section {
            palette: Vec::new(),
            idx: Idx::Narrow(Box::new([0u8; CELLS])),
            count: 0,
        }
    }

    #[inline]
    fn get(&self, at: usize) -> Option<&V> {
        match self.idx.get(at) {
            0 => None,
            i => Some(&self.palette[i - 1]),
        }
    }

    /// The palette index of `v` (1-based), entering it when new.
    fn index_of(&mut self, v: V) -> usize {
        if let Some(p) = self.palette.iter().position(|p| *p == v) {
            return p + 1;
        }
        if self.palette.len() == NARROW_PALETTE && matches!(self.idx, Idx::Narrow(_)) {
            self.compact_palette();
            if self.palette.len() == NARROW_PALETTE {
                self.idx.widen();
            }
        }
        self.palette.push(v);
        self.palette.len()
    }

    /// Drop the palette entries no cell uses, re-indexing the cells.
    fn compact_palette(&mut self) {
        let mut used = vec![false; self.palette.len() + 1];
        for at in 0..CELLS {
            used[self.idx.get(at)] = true;
        }
        let mut renumber = vec![0usize; self.palette.len() + 1];
        let mut palette = Vec::with_capacity(self.palette.len());
        for (old, v) in self.palette.iter().enumerate() {
            if used[old + 1] {
                palette.push(*v);
                renumber[old + 1] = palette.len();
            }
        }
        if palette.len() == self.palette.len() {
            return;
        }
        for at in 0..CELLS {
            let old = self.idx.get(at);
            if old != 0 {
                self.idx.set(at, renumber[old]);
            }
        }
        self.palette = palette;
    }

    /// Bytes this section holds: its indices and its palette.
    fn bytes(&self) -> usize {
        self.idx.bytes() + self.palette.capacity() * std::mem::size_of::<V>()
    }
}

impl<V> Default for SectionMap<V> {
    fn default() -> Self {
        SectionMap {
            sections: BTreeMap::new(),
            len: 0,
        }
    }
}

impl<V: Clone> Clone for SectionMap<V> {
    fn clone(&self) -> Self {
        SectionMap {
            sections: self.sections.clone(),
            len: self.len,
        }
    }
}

impl<V: Copy + Eq> SectionMap<V> {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// The value at `c`, if the store holds `c`.
    #[inline]
    pub fn get(&self, c: &Cell) -> Option<&V> {
        self.sections.get(&section_of(c))?.get(local_of(c))
    }

    /// Whether the store holds `c`.
    #[inline]
    pub fn contains_key(&self, c: &Cell) -> bool {
        self.get(c).is_some()
    }

    /// Set `c` to `v`, returning the value it held before.
    pub fn insert(&mut self, c: Cell, v: V) -> Option<V> {
        let s = self
            .sections
            .entry(section_of(&c))
            .or_insert_with(Section::new);
        let at = local_of(&c);
        let before = s.get(at).copied();
        if before == Some(v) {
            return before;
        }
        let i = s.index_of(v);
        s.idx.set(at, i);
        if before.is_none() {
            s.count += 1;
            self.len += 1;
        }
        before
    }

    /// Remove `c`, returning the value it held.
    pub fn remove(&mut self, c: &Cell) -> Option<V> {
        let key = section_of(c);
        let s = self.sections.get_mut(&key)?;
        let at = local_of(c);
        let before = s.get(at).copied()?;
        s.idx.set(at, 0);
        s.count -= 1;
        self.len -= 1;
        if s.count == 0 {
            self.sections.remove(&key);
        }
        Some(before)
    }

    /// Keep the cells `keep` answers `true` for.
    pub fn retain(&mut self, mut keep: impl FnMut(&Cell, &V) -> bool) {
        let gone: Vec<Cell> = self
            .iter()
            .filter(|(c, v)| !keep(c, v))
            .map(|(c, _)| c)
            .collect();
        for c in &gone {
            self.remove(c);
        }
    }

    /// The number of cells held.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether no cell is held.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The number of sections holding a cell.
    pub fn sections(&self) -> usize {
        self.sections.len()
    }

    /// The bytes the sections hold: every index array and palette. The
    /// store's resident cost, stated by the build.
    pub fn bytes(&self) -> usize {
        self.sections.values().map(Section::bytes).sum()
    }

    /// Every `(cell, value)`, in cell order.
    pub fn iter(&self) -> Iter<'_, V> {
        self.in_box([i32::MIN; 3], [i32::MAX; 3])
    }

    /// Every cell, in cell order.
    pub fn keys(&self) -> impl Iterator<Item = Cell> + '_ {
        self.iter().map(|(c, _)| c)
    }

    /// Every value, in cell order.
    pub fn values(&self) -> impl Iterator<Item = &V> + '_ {
        self.iter().map(|(_, v)| v)
    }

    /// Every `(cell, value)` with the cell in the inclusive box `lo..=hi`
    /// (each axis taken `min..=max`), in cell order.
    pub fn in_box(&self, lo: Cell, hi: Cell) -> Iter<'_, V> {
        let lo_c = [lo[0].min(hi[0]), lo[1].min(hi[1]), lo[2].min(hi[2])];
        let hi_c = [lo[0].max(hi[0]), lo[1].max(hi[1]), lo[2].max(hi[2])];
        Iter {
            map: self,
            lo: lo_c,
            hi: hi_c,
            slo: section_of(&lo_c),
            shi: section_of(&hi_c),
            next_sx: Some(lo_c[0].div_euclid(SIDE)),
            slab: Slab::default(),
        }
    }
}

impl<V: Copy + Eq> FromIterator<(Cell, V)> for SectionMap<V> {
    fn from_iter<I: IntoIterator<Item = (Cell, V)>>(iter: I) -> Self {
        let mut m = SectionMap::new();
        m.extend(iter);
        m
    }
}

impl<V: Copy + Eq> Extend<(Cell, V)> for SectionMap<V> {
    fn extend<I: IntoIterator<Item = (Cell, V)>>(&mut self, iter: I) {
        for (c, v) in iter {
            self.insert(c, v);
        }
    }
}

impl<V: Copy + Eq> From<BTreeMap<Cell, V>> for SectionMap<V> {
    fn from(map: BTreeMap<Cell, V>) -> Self {
        map.into_iter().collect()
    }
}

impl<'a, V: Copy + Eq> IntoIterator for &'a SectionMap<V> {
    type Item = (Cell, &'a V);
    type IntoIter = Iter<'a, V>;
    fn into_iter(self) -> Iter<'a, V> {
        self.iter()
    }
}

impl<V: Copy + Eq> PartialEq for SectionMap<V> {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && self.iter().eq(other.iter())
    }
}

impl<V: Copy + Eq> Eq for SectionMap<V> {}

impl<V: Copy + Eq + std::fmt::Debug> std::fmt::Debug for SectionMap<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

/// The sections of one section-`x` slab, in `(sy, sz)` order, with the local
/// ranges the iterator's box clips each to.
#[derive(Clone)]
struct Slab<'a, V> {
    /// `(section key, section, lz0, lz1)`, in `(sy, sz)` order.
    cols: Vec<(Cell, &'a Section<V>, usize, usize)>,
    /// Runs of `cols` with one `sy`: `(start, end, ly0, ly1)`.
    groups: Vec<(usize, usize, usize, usize)>,
    /// The slab's `x` range, local.
    lx0: usize,
    lx1: usize,
    // The walk's position: `lx`, then the `sy` group, then `ly`, then the
    // section within the group, then `lz`.
    lx: usize,
    g: usize,
    ly: usize,
    s: usize,
    lz: usize,
    /// Whether the position is set (a fresh slab starts at its first cell).
    started: bool,
}

impl<V> Default for Slab<'_, V> {
    fn default() -> Self {
        Slab {
            cols: Vec::new(),
            groups: Vec::new(),
            lx0: 0,
            lx1: 0,
            lx: 0,
            g: 0,
            ly: 0,
            s: 0,
            lz: 0,
            started: false,
        }
    }
}

/// The cell-ordered iterator of a [`SectionMap`], over one box.
#[derive(Clone)]
pub struct Iter<'a, V> {
    map: &'a SectionMap<V>,
    lo: Cell,
    hi: Cell,
    slo: Cell,
    shi: Cell,
    /// The section `x` the next slab is sought from; `None` once past the box.
    next_sx: Option<i32>,
    slab: Slab<'a, V>,
}

impl<'a, V: Copy + Eq> Iter<'a, V> {
    /// Load the next non-empty slab at or after `next_sx`; `false` at the end.
    fn load_slab(&mut self) -> bool {
        loop {
            let Some(from) = self.next_sx else {
                return false;
            };
            if from > self.shi[0] {
                self.next_sx = None;
                return false;
            }
            // The first section at or after `from` says which `sx` comes next.
            let Some((first, _)) = self
                .map
                .sections
                .range([from, i32::MIN, i32::MIN]..=[self.shi[0], i32::MAX, i32::MAX])
                .next()
            else {
                self.next_sx = None;
                return false;
            };
            let sx = first[0];
            self.next_sx = sx.checked_add(1);
            let slab = &mut self.slab;
            slab.cols.clear();
            slab.groups.clear();
            let base_x = i64::from(sx) * i64::from(SIDE);
            slab.lx0 = (i64::from(self.lo[0]) - base_x).max(0) as usize;
            slab.lx1 = (i64::from(self.hi[0]) - base_x).min(15) as usize;
            for (key, sec) in self
                .map
                .sections
                .range([sx, self.slo[1], i32::MIN]..=[sx, self.shi[1], i32::MAX])
            {
                if key[2] < self.slo[2] || key[2] > self.shi[2] {
                    continue;
                }
                let base_z = i64::from(key[2]) * i64::from(SIDE);
                let lz0 = (i64::from(self.lo[2]) - base_z).max(0) as usize;
                let lz1 = (i64::from(self.hi[2]) - base_z).min(15) as usize;
                match slab.groups.last_mut() {
                    Some(g) if slab.cols[g.0].0[1] == key[1] => g.1 += 1,
                    _ => {
                        let base_y = i64::from(key[1]) * i64::from(SIDE);
                        let ly0 = (i64::from(self.lo[1]) - base_y).max(0) as usize;
                        let ly1 = (i64::from(self.hi[1]) - base_y).min(15) as usize;
                        slab.groups
                            .push((slab.cols.len(), slab.cols.len() + 1, ly0, ly1));
                    }
                }
                slab.cols.push((*key, sec, lz0, lz1));
            }
            if slab.cols.is_empty() {
                continue;
            }
            slab.lx = slab.lx0;
            slab.g = 0;
            slab.ly = slab.groups[0].2;
            slab.s = slab.groups[0].0;
            slab.lz = slab.cols[slab.s].2;
            slab.started = true;
            return true;
        }
    }

    /// Advance the slab's position by one cell; `false` when the slab is done.
    fn step(&mut self) -> bool {
        let slab = &mut self.slab;
        slab.lz += 1;
        if slab.lz <= slab.cols[slab.s].3 {
            return true;
        }
        slab.s += 1;
        let g = slab.groups[slab.g];
        if slab.s < g.1 {
            slab.lz = slab.cols[slab.s].2;
            return true;
        }
        slab.ly += 1;
        if slab.ly <= g.3 {
            slab.s = g.0;
            slab.lz = slab.cols[slab.s].2;
            return true;
        }
        slab.g += 1;
        if slab.g < slab.groups.len() {
            let g = slab.groups[slab.g];
            slab.ly = g.2;
            slab.s = g.0;
            slab.lz = slab.cols[slab.s].2;
            return true;
        }
        slab.lx += 1;
        if slab.lx <= slab.lx1 {
            slab.g = 0;
            let g = slab.groups[0];
            slab.ly = g.2;
            slab.s = g.0;
            slab.lz = slab.cols[slab.s].2;
            return true;
        }
        slab.started = false;
        false
    }
}

impl<'a, V: Copy + Eq> Iterator for Iter<'a, V> {
    type Item = (Cell, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if !self.slab.started && !self.load_slab() {
                return None;
            }
            // The position names a cell; read it, then advance.
            let slab = &self.slab;
            let (key, sec, _, _) = slab.cols[slab.s];
            let at = (slab.lx * 16 + slab.ly) * 16 + slab.lz;
            let (lx, ly, lz) = (slab.lx, slab.ly, slab.lz);
            let hit = sec.get(at).map(|v| {
                (
                    [
                        key[0] * SIDE + lx as i32,
                        key[1] * SIDE + ly as i32,
                        key[2] * SIDE + lz as i32,
                    ],
                    v,
                )
            });
            // A group's `ly` range and a section's `lz` range can be empty
            // only when the box misses it, which the section range excluded.
            let _ = self.step();
            if hit.is_some() {
                return hit;
            }
        }
    }
}

/// The BTreeMap iterator type the overlay of a [`super::cellset::CellMap`]
/// merges with the store's.
pub type OverlayIter<'a, V> = btree_map::Iter<'a, Cell, Option<V>>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn xorshift(seed: &mut u64) -> u64 {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        *seed
    }

    /// Every answer of the store equals the map's, over generated edit
    /// sequences: cells across section borders and negative coordinates,
    /// values past the narrow palette, copies taken mid-stream, and boxes of
    /// every shape read back in order.
    #[test]
    fn a_section_store_answers_what_a_map_answers() {
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = |n: u64| xorshift(&mut seed) % n;
        // Three cell pools: one section's worth, a span of sections across
        // the origin, and a wide sparse spread.
        let pool = |k: u64, next: &mut dyn FnMut(u64) -> u64| -> Cell {
            match k {
                0 => [next(16) as i32, next(16) as i32, next(16) as i32],
                1 => [
                    next(40) as i32 - 20,
                    next(40) as i32 - 20,
                    next(40) as i32 - 20,
                ],
                _ => [
                    next(400) as i32 - 200,
                    next(100) as i32 - 50,
                    next(400) as i32 - 200,
                ],
            }
        };
        let mut store: SectionMap<u32> = SectionMap::new();
        let mut map: BTreeMap<Cell, u32> = BTreeMap::new();
        let mut copy: Option<(SectionMap<u32>, BTreeMap<Cell, u32>)> = None;
        let mut widened = 0usize;
        for step in 0..60_000u32 {
            let c = pool(next(3), &mut next);
            match next(4) {
                0 => {
                    assert_eq!(store.remove(&c), map.remove(&c), "remove {c:?}");
                }
                _ => {
                    // 300 distinct values, so a dense section outgrows the
                    // narrow palette.
                    let v = next(300) as u32;
                    assert_eq!(store.insert(c, v), map.insert(c, v), "insert {c:?}");
                }
            }
            assert_eq!(store.get(&c), map.get(&c));
            assert_eq!(store.contains_key(&c), map.contains_key(&c));
            assert_eq!(store.len(), map.len());
            if step == 30_000 {
                copy = Some((store.clone(), map.clone()));
            }
            if step % 5_000 == 4_999 {
                assert!(
                    store.iter().eq(map.iter().map(|(c, v)| (*c, v))),
                    "iter at {step}"
                );
                for _ in 0..20 {
                    let a = pool(next(3), &mut next);
                    let b = pool(next(3), &mut next);
                    let lo = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
                    let hi = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];
                    let expect: Vec<(Cell, &u32)> = map
                        .range(lo..=hi)
                        .filter(|(c, _)| (0..3).all(|i| lo[i] <= c[i] && c[i] <= hi[i]))
                        .map(|(c, v)| (*c, v))
                        .collect();
                    let got: Vec<(Cell, &u32)> = store.in_box(lo, hi).collect();
                    assert_eq!(got, expect, "in_box {lo:?}..={hi:?} at {step}");
                }
            }
        }
        for s in store.sections.values() {
            if matches!(s.idx, Idx::Wide(_)) {
                widened += 1;
            }
        }
        assert!(widened > 0, "no section outgrew the narrow palette");
        assert!(store.iter().eq(map.iter().map(|(c, v)| (*c, v))));
        assert_eq!(
            store.keys().collect::<Vec<_>>(),
            map.keys().copied().collect::<Vec<_>>()
        );
        assert!(store.values().eq(map.values()));
        assert_eq!(store.sections(), {
            let s: std::collections::BTreeSet<Cell> = map.keys().map(section_of).collect();
            s.len()
        });
        let (cs, cm) = copy.expect("a copy was taken");
        assert!(
            cs.iter().eq(cm.iter().map(|(c, v)| (*c, v))),
            "the copy kept its own cells"
        );
        assert_ne!(cs, store);
        let again: SectionMap<u32> = map.iter().map(|(c, v)| (*c, *v)).collect();
        assert_eq!(again, store);
        assert_eq!(SectionMap::from(map.clone()), store);
        // The box a slab, a column and a single cell.
        let whole: Vec<(Cell, &u32)> = store.iter().collect();
        let x_slab: Vec<(Cell, &u32)> = store
            .in_box([-3, i32::MIN, i32::MIN], [17, i32::MAX, i32::MAX])
            .collect();
        assert_eq!(
            x_slab,
            whole
                .iter()
                .copied()
                .filter(|(c, _)| (-3..=17).contains(&c[0]))
                .collect::<Vec<_>>()
        );
        let one = whole[whole.len() / 2];
        assert_eq!(store.in_box(one.0, one.0).collect::<Vec<_>>(), vec![one]);
        assert!(store.in_box([1000, 0, 0], [1001, 0, 0]).next().is_none());
        // Retain, then empty.
        let mut r = store.clone();
        let mut rm = map.clone();
        r.retain(|c, v| (c[0] + *v as i32) % 3 != 0);
        rm.retain(|c, v| (c[0] + *v as i32) % 3 != 0);
        assert!(r.iter().eq(rm.iter().map(|(c, v)| (*c, v))));
        assert_eq!(r.len(), rm.len());
        for c in rm.keys() {
            r.remove(c);
        }
        assert!(r.is_empty());
        assert_eq!(r.sections(), 0);
        assert_eq!(r.bytes(), 0);
    }

    /// A section's palette is compacted before its index widens, so a
    /// section that cycled through many values keeps a narrow index while it
    /// holds few.
    #[test]
    fn a_palette_forgets_the_values_no_cell_holds() {
        let mut s: SectionMap<u32> = SectionMap::new();
        for v in 0..1000u32 {
            s.insert([3, 3, 3], v);
        }
        assert_eq!(s.get(&[3, 3, 3]), Some(&999));
        assert_eq!(s.len(), 1);
        let sec = s.sections.values().next().expect("one section");
        assert!(
            matches!(sec.idx, Idx::Narrow(_)),
            "one live value keeps a narrow index"
        );
        assert!(sec.palette.len() <= NARROW_PALETTE);
        assert_eq!(s.bytes(), CELLS + sec.palette.capacity() * 4);
    }
}
