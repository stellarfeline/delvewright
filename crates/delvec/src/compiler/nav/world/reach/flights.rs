//! **The flight memo**: every launch flown over a real world, kept by the cells
//! it read and what each held, so a launch from another cell — of this world or
//! of any other — that starts the same and meets the same cells is answered
//! without a tick.
//!
//! A flight is a deterministic function of its start (the launch's input, the
//! body's state, the jump's tick, whether the world can hold a fluid, whether
//! a fall kills) and of the codes the cells it asks about hold. The cells it
//! asks about are themselves decided by the codes it has already read: the
//! first cell it reads is fixed by its start, and each next cell it has not
//! read before by the codes of the cells before it. So the flights of one
//! start form a tree — at each node the cell the flight reads next, a branch
//! per code it held, and at each leaf what the flight did — and walking the
//! tree over a world reads the cells the flight would read, in its order,
//! until it either reaches the flight's outcome or meets a code no flight
//! before it met.
//!
//! Two things a flight meets are not cells, and a leaf answers for them
//! separately: a killing volume, which a leaf is never recorded through (a
//! flight a volume killed is not kept) and is reused only where no volume lies
//! near the box it swept, exactly as a template arc is credited
//! (spec-0113 §3.3); and the floor of the world's window, which a leaf records
//! as the lowest feet height its check passed (and, for a flight that left the
//! world, the height it left at), so another floor reuses it only where the
//! check answers the same on every tick.
//!
//! The memo is an accelerator, never a premise: a walk that misses flies the
//! launch, and every answer is the answer a flight would give. Nothing in it
//! depends on which thread asked first, so the order threads fill it in
//! changes how long a build takes and never what it emits (ADR-0006).

use super::{Code, Codes, Flight, Launch, Mode, Outcome};
use delvewright_dsl::metrics::body::{Aabb, State};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{OnceLock, RwLock};

/// What a flight's start is, exactly, as bits: every input to the flight but
/// the cells it reads.
#[derive(Debug, Clone)]
pub(super) struct Key {
    words: [u64; 12],
    /// The words' hash, taken once: it picks the shard and keys the map.
    hash: u64,
}

impl PartialEq for Key {
    fn eq(&self, o: &Key) -> bool {
        self.words == o.words
    }
}

impl Eq for Key {}

impl std::hash::Hash for Key {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        h.write_u64(self.hash);
    }
}

impl Key {
    pub(super) fn new(
        launch: &Launch,
        st: &State,
        press: Option<u8>,
        wet: bool,
        unbounded_fall: bool,
    ) -> Key {
        let sprint = !matches!(launch.mode, Mode::WalkOff | Mode::Creep);
        let flags = u64::from(wet)
            | u64::from(unbounded_fall) << 1
            | u64::from(st.on_ground) << 2
            | u64::from(st.in_water) << 3
            | u64::from(st.in_lava) << 4
            | u64::from(st.in_web) << 5
            | u64::from(st.collided_h) << 6
            | u64::from(st.collided_v) << 7
            | u64::from(st.sprinting) << 8
            | u64::from(sprint) << 9
            | u64::from(press.map_or(0x1ff, u16::from)) << 16
            | u64::from(st.jump_ticks) << 32;
        let words = [
            flags,
            launch.yaw.to_bits(),
            launch.strafe.to_bits(),
            st.pos[0].to_bits(),
            st.pos[1].to_bits(),
            st.pos[2].to_bits(),
            st.vel[0].to_bits(),
            st.vel[1].to_bits(),
            st.vel[2].to_bits(),
            st.fall.to_bits(),
            st.landing_damage.to_bits(),
            st.height.to_bits(),
        ];
        let mut h = Mix::default();
        for w in words {
            std::hash::Hasher::write_u64(&mut h, w);
        }
        Key {
            words,
            hash: std::hash::Hasher::finish(&h),
        }
    }

    /// A spread of the key over the shards; any function of the key will do.
    fn shard(&self) -> usize {
        (self.hash >> 40) as usize % SHARDS
    }
}

/// What a recorded flight did, with what reusing it under another floor or
/// beside a killing volume needs.
#[derive(Clone, Copy)]
struct Leaf {
    /// The outcome, interned in its shard.
    outcome: u32,
    swept: Option<Aabb>,
    floor: f64,
    left_at: f64,
}

/// A recorded flight, as a lookup hands it out.
pub(super) struct Kept {
    pub(super) outcome: Outcome,
    pub(super) swept: Option<Aabb>,
    floor: f64,
    left_at: f64,
}

impl Kept {
    /// Whether the floor check at `bottom` answers on every tick as it did
    /// when the flight was recorded.
    pub(super) fn holds_at(&self, bottom: i32) -> bool {
        let b = f64::from(bottom);
        match self.outcome {
            Outcome::LeftWorld => self.floor >= b && self.left_at < b,
            _ => self.floor >= b,
        }
    }
}

const NONE: u32 = u32::MAX;

/// One read of a flight: a cell (relative to the launch cell) and the code
/// it held.
#[derive(Clone, Copy, Default)]
struct Entry {
    cell: [i16; 3],
    code: Code,
}

/// One node of a start's tree: a run of cells a flight reads in order
/// (`len` entries from `start`) — every one but the last must hold the code
/// beside it, and the last one's code picks a child — or, with `len == 0`, a
/// leaf (`child` is then the leaf's index). `code` is the code the parent's
/// last cell held on the way here; `sibling` the parent's next child. A run
/// is split only where two flights' reads part, so a walk reads long runs out
/// of one block of memory.
#[derive(Clone, Copy)]
struct Node {
    start: u32,
    len: u32,
    child: u32,
    sibling: u32,
    code: Code,
}

/// The entries a chunk of a shard's arena holds. A run never crosses a
/// chunk; a flight that read more cells than a chunk holds is not kept.
const CHUNK_BITS: u32 = 14;
const CHUNK: usize = 1 << CHUNK_BITS;

/// One shard of the memo: every start that hashes here, their trees' nodes
/// and entries, the leaves, and the outcomes the leaves name.
#[derive(Default)]
struct Shard {
    roots: HashMap<Key, u32, std::hash::BuildHasherDefault<Mix>>,
    nodes: Vec<Node>,
    chunks: Vec<Box<[Entry]>>,
    /// Entries used in the last chunk.
    used: usize,
    leaves: Vec<Leaf>,
    outcomes: Vec<Outcome>,
    interned: HashMap<Outcome, u32, std::hash::BuildHasherDefault<Mix>>,
}

impl Shard {
    #[inline]
    fn entry(&self, i: u32) -> Entry {
        self.chunks[(i >> CHUNK_BITS) as usize][i as usize & (CHUNK - 1)]
    }

    /// Store `run` contiguously; its first entry's index.
    fn push_run(&mut self, run: &[([i16; 3], Code)]) -> u32 {
        if self.chunks.is_empty() || self.used + run.len() > CHUNK {
            self.chunks
                .push(vec![Entry::default(); CHUNK].into_boxed_slice());
            self.used = 0;
        }
        let c = self.chunks.len() - 1;
        let at = self.used;
        for (k, (cell, code)) in run.iter().enumerate() {
            self.chunks[c][at + k] = Entry {
                cell: *cell,
                code: *code,
            };
        }
        self.used += run.len();
        ((c << CHUNK_BITS) | at) as u32
    }

    /// The child of `n` reached by `code`, if any.
    #[inline]
    fn child(&self, n: u32, code: Code) -> Option<u32> {
        let mut k = self.nodes[n as usize].child;
        while k != NONE {
            let node = &self.nodes[k as usize];
            if node.code == code {
                return Some(k);
            }
            k = node.sibling;
        }
        None
    }
}

const SHARDS: usize = 256;
/// The entries and nodes the memo holds before it stops recording (eight
/// bytes an entry, twenty a node, a leaf's seventy-two beside): a bound on
/// its memory, never on what a build answers.
const MEMO_ENTRIES: u64 = 1 << 25;

fn shards() -> &'static [RwLock<Shard>] {
    static S: OnceLock<Vec<RwLock<Shard>>> = OnceLock::new();
    S.get_or_init(|| (0..SHARDS).map(|_| RwLock::new(Shard::default())).collect())
}

static HELD: AtomicU64 = AtomicU64::new(0);

fn small(c: [i32; 3]) -> Option<[i16; 3]> {
    Some([
        i16::try_from(c[0]).ok()?,
        i16::try_from(c[1]).ok()?,
        i16::try_from(c[2]).ok()?,
    ])
}

#[inline]
fn read<C: Codes + ?Sized>(codes: &C, c: [i16; 3]) -> Code {
    codes.code([i32::from(c[0]), i32::from(c[1]), i32::from(c[2])])
}

/// The recorded flight of `key` over `codes`, if one met the same cells.
pub(super) fn lookup<C: Codes + ?Sized>(key: &Key, codes: &C) -> Option<Kept> {
    let shard = shards()[key.shard()].read().expect("flight memo");
    let mut n = *shard.roots.get(key)?;
    loop {
        let node = shard.nodes[n as usize];
        if node.len == 0 {
            let leaf = shard.leaves[node.child as usize];
            return Some(Kept {
                outcome: shard.outcomes[leaf.outcome as usize].clone(),
                swept: leaf.swept,
                floor: leaf.floor,
                left_at: leaf.left_at,
            });
        }
        for k in 0..node.len - 1 {
            let e = shard.entry(node.start + k);
            if read(codes, e.cell) != e.code {
                return None;
            }
        }
        let last = shard.entry(node.start + node.len - 1);
        n = shard.child(n, read(codes, last.cell))?;
    }
}

/// A [`Codes`] that writes down each cell the first time it is asked about,
/// with what it held.
pub(super) struct Reads<'a, C: Codes + ?Sized> {
    inner: &'a C,
    seen: std::cell::RefCell<ReadSeq>,
    stamp: u32,
    /// The thread's tables, taken for this flight and handed back after.
    tables: std::cell::RefCell<Tables>,
}

/// The cells near the launch cell a flight has read, with their codes, each
/// slot stamped with the flight it belongs to, and [`Seen`] for the cells
/// further off. A flight reads each cell many times, the first through the
/// world and every later one from here.
#[derive(Default)]
struct Tables {
    near: Vec<std::cell::Cell<(u32, Code)>>,
    seen: Seen,
}

/// The box round the launch cell [`Tables::near`] holds: `±NEAR_XZ` across,
/// `NEAR_DOWN` under and `NEAR_UP` over.
const NEAR_XZ: i32 = 12;
const NEAR_DOWN: i32 = 32;
const NEAR_UP: i32 = 12;
const NEAR_W: usize = (2 * NEAR_XZ + 1) as usize;
const NEAR_H: usize = (NEAR_DOWN + NEAR_UP + 1) as usize;

#[inline]
fn near_slot(c: [i32; 3]) -> Option<usize> {
    let x = (c[0] + NEAR_XZ) as u32;
    let y = (c[1] + NEAR_DOWN) as u32;
    let z = (c[2] + NEAR_XZ) as u32;
    (x < NEAR_W as u32 && y < NEAR_H as u32 && z < NEAR_W as u32)
        .then(|| (x as usize * NEAR_W + z as usize) * NEAR_H + y as usize)
}

/// The cells a flight read, in the order it first read them, with their codes.
type ReadSeq = Vec<([i32; 3], Code)>;

/// The cells the flight a thread is recording has read: an open-addressed
/// table of cells, each slot stamped with the flight it belongs to, so a new
/// flight starts with an empty table without clearing it. A flight reads a
/// few hundred cells, most of them many times.
#[derive(Default)]
struct Seen {
    stamp: u32,
    slots: Vec<(u32, [i32; 3])>,
    held: usize,
}

impl Seen {
    fn slot(slots: &[(u32, [i32; 3])], c: [i32; 3]) -> usize {
        let mut h = Mix::default();
        for v in c {
            std::hash::Hasher::write_i32(&mut h, v);
        }
        (std::hash::Hasher::finish(&h) as usize) & (slots.len() - 1)
    }

    /// Whether `c` is new to the flight stamped `stamp`.
    fn insert(&mut self, stamp: u32, c: [i32; 3]) -> bool {
        if self.stamp != stamp {
            self.stamp = stamp;
            self.held = 0;
        }
        if (self.held + 1) * 2 > self.slots.len() {
            let n = (self.slots.len() * 2).max(1 << 12);
            let old = std::mem::replace(&mut self.slots, vec![(0, [0; 3]); n]);
            for (s, c) in old {
                if s == stamp {
                    let mut i = Seen::slot(&self.slots, c);
                    while self.slots[i].0 == stamp {
                        i = (i + 1) & (n - 1);
                    }
                    self.slots[i] = (stamp, c);
                }
            }
        }
        let n = self.slots.len();
        let mut i = Seen::slot(&self.slots, c);
        loop {
            let (s, held) = self.slots[i];
            if s != stamp {
                self.slots[i] = (stamp, c);
                self.held += 1;
                return true;
            }
            if held == c {
                return false;
            }
            i = (i + 1) & (n - 1);
        }
    }
}

thread_local! {
    static TABLES: std::cell::RefCell<Tables> = std::cell::RefCell::new(Tables::default());
    static STAMP: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

impl<C: Codes + ?Sized> Drop for Reads<'_, C> {
    fn drop(&mut self) {
        let t = std::mem::take(&mut *self.tables.borrow_mut());
        TABLES.with(|x| *x.borrow_mut() = t);
    }
}

/// A multiply-rotate hash. Nothing it orders reaches an output: it keys the
/// memo's maps, which are only ever looked up.
#[derive(Default)]
pub(super) struct Mix(u64);

impl std::hash::Hasher for Mix {
    fn finish(&self) -> u64 {
        self.0 ^ (self.0 >> 29)
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes.chunks(8) {
            let mut w = [0u8; 8];
            w[..b.len()].copy_from_slice(b);
            self.write_u64(u64::from_le_bytes(w));
        }
    }
    fn write_u64(&mut self, v: u64) {
        self.0 = (self.0.rotate_left(23) ^ v).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    }
    fn write_u32(&mut self, v: u32) {
        self.write_u64(u64::from(v));
    }
    fn write_i32(&mut self, v: i32) {
        self.write_u64(u64::from(v as u32));
    }
    fn write_usize(&mut self, v: usize) {
        self.write_u64(v as u64);
    }
}

impl<'a, C: Codes + ?Sized> Reads<'a, C> {
    pub(super) fn new(inner: &'a C) -> Self {
        let mut tables = TABLES.with(|x| std::mem::take(&mut *x.borrow_mut()));
        if tables.near.is_empty() {
            tables.near = (0..NEAR_W * NEAR_W * NEAR_H)
                .map(|_| std::cell::Cell::new((0, 0)))
                .collect();
        }
        // A stamp no slot carries yet: the tables are wiped when they wrap.
        let stamp = STAMP.with(|s| {
            let v = s.get().wrapping_add(1).max(1);
            if v == 1 {
                tables.seen.slots.iter_mut().for_each(|x| x.0 = 0);
                tables.near.iter().for_each(|x| x.set((0, 0)));
            }
            s.set(v);
            v
        });
        Reads {
            inner,
            seen: std::cell::RefCell::new(Vec::new()),
            stamp,
            tables: std::cell::RefCell::new(tables),
        }
    }
}

impl<C: Codes + ?Sized> Codes for Reads<'_, C> {
    fn code(&self, c: [i32; 3]) -> Code {
        let tables = self.tables.borrow();
        if let Some(i) = near_slot(c) {
            let slot = &tables.near[i];
            let (s, v) = slot.get();
            if s == self.stamp {
                return v;
            }
            let v = self.inner.code(c);
            slot.set((self.stamp, v));
            self.seen.borrow_mut().push((c, v));
            return v;
        }
        drop(tables);
        let v = self.inner.code(c);
        if self.tables.borrow_mut().seen.insert(self.stamp, c) {
            self.seen.borrow_mut().push((c, v));
        }
        v
    }
    fn wet(&self) -> bool {
        self.inner.wet()
    }
}

/// Record `flight`, flown from `key` reading `reads`. A flight a killing volume
/// stopped is not kept; nor is one whose reads part from the tree where no
/// code says they should — a floor that ended it early, or one that let it
/// fly on.
pub(super) fn record<C: Codes + ?Sized>(key: Key, reads: Reads<'_, C>, flight: &Flight) {
    if matches!(flight.outcome, Outcome::Dead(super::Death::Volume)) {
        return;
    }
    if HELD.load(Relaxed) > MEMO_ENTRIES {
        return;
    }
    let seq = std::mem::take(&mut *reads.seen.borrow_mut());
    let Some(seq) = seq
        .iter()
        .map(|(c, v)| small(*c).map(|c| (c, *v)))
        .collect::<Option<Vec<_>>>()
    else {
        return;
    };
    if seq.len() > CHUNK {
        return;
    }
    let mut guard = shards()[key.shard()].write().expect("flight memo");
    let shard = &mut *guard;
    // A new path for the reads from `i` on: a run of them and the flight's
    // leaf, or the leaf alone when there are none.
    let path = |shard: &mut Shard, i: usize, code: Code| -> u32 {
        let outcome = match shard.interned.get(&flight.outcome) {
            Some(&o) => o,
            None => {
                shard.outcomes.push(flight.outcome.clone());
                let o = (shard.outcomes.len() - 1) as u32;
                shard.interned.insert(flight.outcome.clone(), o);
                o
            }
        };
        shard.leaves.push(Leaf {
            outcome,
            swept: flight.swept,
            floor: flight.floor,
            left_at: flight.left_at,
        });
        let leaf = (shard.leaves.len() - 1) as u32;
        let rest = &seq[i..];
        let leaf_code = rest.last().map_or(code, |e| e.1);
        shard.nodes.push(Node {
            start: 0,
            len: 0,
            child: leaf,
            sibling: NONE,
            code: if rest.is_empty() { code } else { leaf_code },
        });
        let leaf_node = (shard.nodes.len() - 1) as u32;
        if rest.is_empty() {
            return leaf_node;
        }
        let start = shard.push_run(rest);
        shard.nodes.push(Node {
            start,
            len: rest.len() as u32,
            child: leaf_node,
            sibling: NONE,
            code,
        });
        (shard.nodes.len() - 1) as u32
    };
    let mut n = match shard.roots.get(&key) {
        Some(&n) => n,
        None => {
            let n = path(shard, 0, 0);
            shard.roots.insert(key, n);
            HELD.fetch_add(seq.len() as u64 + 2, Relaxed);
            return;
        }
    };
    let mut i = 0usize;
    loop {
        let node = shard.nodes[n as usize];
        if node.len == 0 {
            // The flight's leaf already, or a flight that read on past where
            // this one stopped: either way nothing is added.
            break;
        }
        let len = node.len as usize;
        for j in 0..len {
            let e = shard.entry(node.start + j as u32);
            let Some(&(cell, code)) = seq.get(i + j) else {
                return;
            };
            if cell != e.cell {
                return;
            }
            if j + 1 < len && code != e.code {
                // The reads part here: the run ends at this cell, its old
                // rest and the new path become its two children.
                let old = Node {
                    start: node.start + j as u32 + 1,
                    len: (len - j - 1) as u32,
                    child: node.child,
                    sibling: NONE,
                    code: e.code,
                };
                shard.nodes.push(old);
                let a = (shard.nodes.len() - 1) as u32;
                let b = path(shard, i + j + 1, code);
                shard.nodes[b as usize].sibling = a;
                let m = &mut shard.nodes[n as usize];
                m.len = j as u32 + 1;
                m.child = b;
                HELD.fetch_add((seq.len() - i - j) as u64 + 3, Relaxed);
                return;
            }
        }
        let v = seq[i + len - 1].1;
        match shard.child(n, v) {
            Some(k) => {
                n = k;
                i += len;
            }
            None => {
                let k = path(shard, i + len, v);
                shard.nodes[k as usize].sibling = shard.nodes[n as usize].child;
                shard.nodes[n as usize].child = k;
                HELD.fetch_add((seq.len() - i - len) as u64 + 2, Relaxed);
                return;
            }
        }
    }
}
