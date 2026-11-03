//! **The reached states of a closure** (ADR-0037 §2): one bit per window cell,
//! a second bit per window cell for the plain-sea states the closure flooded
//! rather than expanded (`World::plain_sea`), and per block of the window the
//! kinds of the other states its bits hold, one byte each in bit order — never
//! a map from cell to kind, and no byte for a plain-sea state. A state outside
//! the window (the open sea past its margin) is kept by name.
//!
//! What it answers is what a `BTreeMap<[i32; 3], RestKind>` of the same states
//! answers, in the same order: iteration is by cell (`x`, then `y`, then `z`),
//! whatever order the states were reached in.

use super::{Dense, RestKind};
use std::collections::BTreeMap;

/// The kinds a state can hold, by their byte.
const KINDS: [RestKind; 4] = [
    RestKind::Standing,
    RestKind::Afloat,
    RestKind::Submerged,
    RestKind::Holding,
];

fn byte_of(k: RestKind) -> u8 {
    match k {
        RestKind::Standing => 0,
        RestKind::Afloat => 1,
        RestKind::Submerged => 2,
        RestKind::Holding => 3,
    }
}

/// A block of the window holds this many cells' bits: 64 words.
const BLOCK_BITS: u32 = 12;
const BLOCK_WORDS: usize = 1 << (BLOCK_BITS - 6);

/// Every reached state: a bitmap over the window and the kinds by block.
#[derive(Debug, Clone, Default)]
pub struct States {
    /// The window's least corner and size ([`Dense`]'s), once a state inside
    /// it was kept.
    lo: [i32; 3],
    dims: [i32; 3],
    /// One bit per window cell, in [`Dense::index`] order; empty until a
    /// state inside the window is kept.
    bits: Vec<u64>,
    /// One bit per window cell: set where the state is a plain-sea state,
    /// submerged and summarised ([`States::insert_plain`]); a subset of
    /// `bits`, with no byte in `kinds`.
    plain: Vec<u64>,
    /// Per block of [`BLOCK_WORDS`] words, the kind of each state its bits
    /// hold that is not plain, in bit order.
    kinds: Vec<Vec<u8>>,
    /// The states outside the window, by name.
    outside: BTreeMap<[i32; 3], u8>,
    len: usize,
}

impl States {
    /// The window index of `c`, once the window is known.
    #[inline]
    fn index(&self, c: [i32; 3]) -> Option<usize> {
        if self.bits.is_empty() {
            return None;
        }
        let x = c[0].wrapping_sub(self.lo[0]) as u32;
        let y = c[1].wrapping_sub(self.lo[1]) as u32;
        let z = c[2].wrapping_sub(self.lo[2]) as u32;
        (x < self.dims[0] as u32 && y < self.dims[1] as u32 && z < self.dims[2] as u32).then(|| {
            ((x as usize * self.dims[2] as usize) + z as usize) * self.dims[1] as usize + y as usize
        })
    }

    /// The rank of bit `i` among the set bits of its block that are not
    /// plain: its byte's place in the block's `kinds`.
    #[inline]
    fn rank(&self, i: usize) -> usize {
        let w = i / 64;
        let first = w & !(BLOCK_WORDS - 1);
        let below: u32 = (first..w)
            .map(|j| (self.bits[j] & !self.plain[j]).count_ones())
            .sum();
        let mask = (1u64 << (i % 64)) - 1;
        (below + (self.bits[w] & !self.plain[w] & mask).count_ones()) as usize
    }

    /// Open the window's bitmaps over `d`, once a state inside it is kept.
    fn open(&mut self, d: &Dense, c: [i32; 3]) {
        if self.bits.is_empty() && d.index(c).is_some() {
            self.lo = d.lo;
            self.dims = d.dims;
            self.bits = vec![0; d.cells.len().div_ceil(64)];
            self.plain = vec![0; self.bits.len()];
            self.kinds = vec![Vec::new(); self.bits.len().div_ceil(BLOCK_WORDS)];
        }
    }

    /// Keep `c` as a state of kind `k`, over `d`'s window; a state kept
    /// already takes the new kind, a plain-sea state included (it is plain no
    /// longer).
    pub(in crate::compiler::nav) fn insert(&mut self, d: &Dense, c: [i32; 3], k: RestKind) {
        self.open(d, c);
        match self.index(c) {
            Some(i) => {
                let (w, bit) = (i / 64, 1u64 << (i % 64));
                let was_plain = self.plain[w] & bit != 0;
                self.plain[w] &= !bit;
                let r = self.rank(i);
                let block = &mut self.kinds[i >> BLOCK_BITS];
                if self.bits[w] & bit != 0 && !was_plain {
                    block[r] = byte_of(k);
                } else {
                    self.bits[w] |= bit;
                    block.insert(r, byte_of(k));
                    if !was_plain {
                        self.len += 1;
                    }
                }
            }
            None => {
                if self.outside.insert(c, byte_of(k)).is_none() {
                    self.len += 1;
                }
            }
        }
    }

    /// Keep `c` as a **plain-sea state** (`World::plain_sea`): submerged, with
    /// no byte of its own, told apart by its bit. A cell outside the window is
    /// kept by name as submerged (no plain-sea cell lies there, since each of
    /// its six neighbours is inside the window). A state kept already is left
    /// as it was.
    pub(in crate::compiler::nav) fn insert_plain(&mut self, d: &Dense, c: [i32; 3]) {
        self.open(d, c);
        match self.index(c) {
            Some(i) => {
                let (w, bit) = (i / 64, 1u64 << (i % 64));
                if self.bits[w] & bit == 0 {
                    self.bits[w] |= bit;
                    self.plain[w] |= bit;
                    self.len += 1;
                }
            }
            None => {
                if let std::collections::btree_map::Entry::Vacant(e) = self.outside.entry(c) {
                    e.insert(byte_of(RestKind::Submerged));
                    self.len += 1;
                }
            }
        }
    }

    /// Whether `c` is a plain-sea state.
    #[inline]
    pub fn plain(&self, c: &[i32; 3]) -> bool {
        match self.index(*c) {
            Some(i) => self.plain[i / 64] & (1 << (i % 64)) != 0,
            None => false,
        }
    }

    /// How many plain-sea states.
    pub fn plain_len(&self) -> usize {
        self.plain.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Whether `c` is a state.
    #[inline]
    pub fn contains_key(&self, c: &[i32; 3]) -> bool {
        match self.index(*c) {
            Some(i) => self.bits[i / 64] & (1 << (i % 64)) != 0,
            None => self.outside.contains_key(c),
        }
    }

    /// The kind of state `c` holds.
    pub fn get(&self, c: &[i32; 3]) -> Option<&'static RestKind> {
        match self.index(*c) {
            Some(i) => {
                let (w, bit) = (i / 64, 1u64 << (i % 64));
                if self.bits[w] & bit == 0 {
                    None
                } else if self.plain[w] & bit != 0 {
                    Some(&RestKind::Submerged)
                } else {
                    Some(&KINDS[self.kinds[i >> BLOCK_BITS][self.rank(i)] as usize])
                }
            }
            None => self.outside.get(c).map(|b| &KINDS[*b as usize]),
        }
    }

    /// How many states.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Every state and its kind, in cell order.
    pub fn iter(&self) -> Iter<'_> {
        Iter {
            s: self,
            x: 0,
            slab: Vec::new(),
            at: 0,
            outside: self.outside.iter().peekable(),
        }
    }

    /// Every state, in cell order.
    pub fn keys(&self) -> impl Iterator<Item = [i32; 3]> + '_ {
        self.iter().map(|(c, _)| c)
    }

    /// Every state's kind, in cell order.
    pub fn values(&self) -> impl Iterator<Item = RestKind> + '_ {
        self.iter().map(|(_, k)| k)
    }

    /// The bytes held, beyond the struct.
    pub fn heap(&self) -> usize {
        self.bits.capacity() * 8
            + self.plain.capacity() * 8
            + self.kinds.capacity() * std::mem::size_of::<Vec<u8>>()
            + self.kinds.iter().map(Vec::capacity).sum::<usize>()
            + self.outside.len() * 16
    }
}

/// The states in cell order: the window's, one `x` slab at a time, merged
/// with the ones outside it.
pub struct Iter<'a> {
    s: &'a States,
    /// The next slab of the window to read, as an offset from its least `x`.
    x: i32,
    /// The current slab's states, in cell order.
    slab: Vec<([i32; 3], RestKind)>,
    at: usize,
    outside: std::iter::Peekable<std::collections::btree_map::Iter<'a, [i32; 3], u8>>,
}

impl Iter<'_> {
    /// Read the next window slab that holds a state into `slab`.
    fn fill(&mut self) -> bool {
        let s = self.s;
        if s.bits.is_empty() {
            return false;
        }
        let (dy, dz) = (s.dims[1] as usize, s.dims[2] as usize);
        let per = dy * dz;
        while self.x < s.dims[0] {
            let x = self.x as usize;
            self.x += 1;
            let (from, to) = (x * per, (x + 1) * per);
            self.slab.clear();
            self.at = 0;
            let mut w = from / 64;
            while w * 64 < to {
                let mut b = s.bits[w];
                if w * 64 < from {
                    b &= !0u64 << (from - w * 64);
                }
                if (w + 1) * 64 > to {
                    b &= (1u64 << (to - w * 64)) - 1;
                }
                while b != 0 {
                    let i = w * 64 + b.trailing_zeros() as usize;
                    b &= b - 1;
                    let k = if s.plain[w] & (1 << (i % 64)) != 0 {
                        RestKind::Submerged
                    } else {
                        KINDS[s.kinds[i >> BLOCK_BITS][s.rank(i)] as usize]
                    };
                    let c = [
                        s.lo[0] + x as i32,
                        s.lo[1] + (i % dy) as i32,
                        s.lo[2] + ((i / dy) % dz) as i32,
                    ];
                    self.slab.push((c, k));
                }
                w += 1;
            }
            if !self.slab.is_empty() {
                self.slab.sort_unstable_by_key(|(c, _)| *c);
                return true;
            }
        }
        false
    }
}

impl Iterator for Iter<'_> {
    type Item = ([i32; 3], RestKind);

    fn next(&mut self) -> Option<Self::Item> {
        if self.at >= self.slab.len() && !self.fill() {
            self.slab.clear();
            self.at = 0;
        }
        let inside = self.slab.get(self.at).copied();
        let out = self.outside.peek().map(|(c, b)| (**c, KINDS[**b as usize]));
        match (inside, out) {
            (Some(a), Some(b)) if b.0 < a.0 => {
                self.outside.next();
                Some(b)
            }
            (Some(a), _) => {
                self.at += 1;
                Some(a)
            }
            (None, Some(b)) => {
                self.outside.next();
                Some(b)
            }
            (None, None) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Chunked, Dense, Outside, RestKind};
    use super::{KINDS, States};
    use std::collections::{BTreeMap, BTreeSet};

    fn window(lo: [i32; 3], dims: [i32; 3]) -> Dense {
        let n = dims.iter().map(|d| *d as usize).product::<usize>();
        Dense {
            lo,
            dims,
            cells: Chunked::new(&vec![0; n]),
            outside: Outside::Void,
            wet: false,
        }
    }

    /// **The states answer what a map of them answers** (ADR-0037 §2): on
    /// windows whose slabs straddle words and blocks, states kept in a
    /// generated order — inside the window and round it, some kept twice
    /// with a new kind, one in five kept as a plain-sea state (submerged, no
    /// byte) and some of those kept again with a kind — iterate, count, look
    /// up and test membership exactly as a `BTreeMap<[i32; 3], RestKind>` of
    /// the same inserts does, and answer plain-sea membership as a set of the
    /// cells last kept plain does, over every cell of the window grown by
    /// two. Red on any difference; perturbed by leaving a slab unsorted.
    #[test]
    fn the_states_answer_what_a_map_of_them_answers() {
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = |n: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % n
        };
        let (mut kept, mut outside, mut twice, mut plain, mut unplained) =
            (0usize, 0usize, 0usize, 0usize, 0usize);
        for (lo, dims) in [
            ([0, 0, 0], [1, 1, 1]),
            ([-3, 5, 2], [5, 13, 7]),
            ([10, -64, -10], [3, 64, 2]),
            ([-7, 0, 4], [4, 70, 33]),
            ([0, 0, 0], [9, 200, 11]),
        ] {
            let d = window(lo, dims);
            let mut s = States::default();
            let mut m: BTreeMap<[i32; 3], RestKind> = BTreeMap::new();
            let mut p: BTreeSet<[i32; 3]> = BTreeSet::new();
            let cells: usize = dims.iter().map(|x| *x as usize).product();
            for _ in 0..(cells / 3 + 20) {
                let c = [
                    lo[0] - 2 + next(dims[0] as u64 + 4) as i32,
                    lo[1] - 2 + next(dims[1] as u64 + 4) as i32,
                    lo[2] - 2 + next(dims[2] as u64 + 4) as i32,
                ];
                if d.index(c).is_none() {
                    outside += 1;
                }
                let roll = next(5) as usize;
                if roll == 4 {
                    // A plain-sea state: submerged, no byte; one kept already
                    // is left as it was.
                    if let std::collections::btree_map::Entry::Vacant(e) = m.entry(c) {
                        e.insert(RestKind::Submerged);
                        if d.index(c).is_some() {
                            p.insert(c);
                            plain += 1;
                        }
                    }
                    s.insert_plain(&d, c);
                } else {
                    let k = KINDS[roll];
                    if m.insert(c, k).is_some() {
                        twice += 1;
                    }
                    if p.remove(&c) {
                        unplained += 1;
                    }
                    s.insert(&d, c, k);
                }
            }
            kept += m.len();
            assert_eq!(s.len(), m.len(), "{dims:?}: the count");
            assert_eq!(s.plain_len(), p.len(), "{dims:?}: the plain-sea count");
            assert_eq!(
                s.iter().collect::<Vec<_>>(),
                m.iter().map(|(c, k)| (*c, *k)).collect::<Vec<_>>(),
                "{dims:?}: the states in cell order"
            );
            for x in lo[0] - 2..lo[0] + dims[0] + 2 {
                for y in lo[1] - 2..lo[1] + dims[1] + 2 {
                    for z in lo[2] - 2..lo[2] + dims[2] + 2 {
                        let c = [x, y, z];
                        assert_eq!(s.get(&c), m.get(&c), "{c:?}: the kind");
                        assert_eq!(s.contains_key(&c), m.contains_key(&c), "{c:?}");
                        assert_eq!(s.plain(&c), p.contains(&c), "{c:?}: plain");
                    }
                }
            }
        }
        eprintln!(
            "states: {kept} state(s) kept over five windows, {outside} insert(s) outside the \
             window, {twice} kept twice, {plain} kept plain, {unplained} of them kept again with \
             a kind"
        );
        assert!(
            kept > 0 && outside > 0 && twice > 0 && plain > 0 && unplained > 0,
            "the cases bind nothing"
        );
    }
}
