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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct Key([u64; 12]);

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
        Key([
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
        ])
    }

    /// A spread of the key over the shards; any function of the key will do.
    fn shard(&self) -> usize {
        let mut h = Mix::default();
        for w in self.0 {
            std::hash::Hasher::write_u64(&mut h, w);
        }
        (std::hash::Hasher::finish(&h) >> 40) as usize % SHARDS
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
/// The tag on [`Node::child`] that makes the node a leaf: the rest of the word
/// indexes the shard's leaves.
const LEAF: u32 = 1 << 31;

/// One node of a start's tree: a cell to read (relative to the launch cell)
/// and its first child, or a leaf; `code` is the code the parent's cell held
/// on the way here, `sibling` the parent's next child.
#[derive(Clone, Copy)]
struct Node {
    cell: [i16; 3],
    code: Code,
    child: u32,
    sibling: u32,
}

/// The nodes a chunk of a shard's arena holds: a path is pushed whole, so a
/// walk down a fresh path reads consecutive nodes.
const CHUNK_BITS: u32 = 14;
const CHUNK: usize = 1 << CHUNK_BITS;

/// One shard of the memo: every start that hashes here, the nodes of their
/// trees in one arena, their leaves, and the outcomes the leaves name.
#[derive(Default)]
struct Shard {
    roots: HashMap<Key, u32, std::hash::BuildHasherDefault<Mix>>,
    chunks: Vec<Box<[Node]>>,
    len: u32,
    leaves: Vec<Leaf>,
    outcomes: Vec<Outcome>,
    interned: HashMap<Outcome, u32, std::hash::BuildHasherDefault<Mix>>,
}

impl Shard {
    #[inline]
    fn node(&self, n: u32) -> &Node {
        &self.chunks[(n >> CHUNK_BITS) as usize][n as usize & (CHUNK - 1)]
    }

    #[inline]
    fn node_mut(&mut self, n: u32) -> &mut Node {
        &mut self.chunks[(n >> CHUNK_BITS) as usize][n as usize & (CHUNK - 1)]
    }

    fn push(&mut self, node: Node) -> u32 {
        let n = self.len;
        if (n as usize).is_multiple_of(CHUNK) {
            self.chunks.push(
                vec![
                    Node {
                        cell: [0; 3],
                        code: 0,
                        child: NONE,
                        sibling: NONE,
                    };
                    CHUNK
                ]
                .into_boxed_slice(),
            );
        }
        self.len += 1;
        *self.node_mut(n) = node;
        n
    }

    /// The child of `n` reached by `code`, if any.
    #[inline]
    fn child(&self, n: u32, code: Code) -> Option<u32> {
        let mut k = self.node(n).child;
        while k != NONE {
            let node = self.node(k);
            if node.code == code {
                return Some(k);
            }
            k = node.sibling;
        }
        None
    }
}

const SHARDS: usize = 256;
/// The nodes the memo holds before it stops recording (sixteen bytes a node,
/// with a leaf's seventy-two beside): a bound on its memory, never on what a
/// build answers.
const MEMO_NODES: u64 = 1 << 25;

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

/// The recorded flight of `key` over `codes`, if one met the same cells.
pub(super) fn lookup<C: Codes + ?Sized>(key: &Key, codes: &C) -> Option<Kept> {
    let shard = shards()[key.shard()].read().expect("flight memo");
    let mut n = *shard.roots.get(key)?;
    loop {
        let node = *shard.node(n);
        if node.child != NONE && node.child & LEAF != 0 {
            let leaf = shard.leaves[(node.child & !LEAF) as usize];
            return Some(Kept {
                outcome: shard.outcomes[leaf.outcome as usize].clone(),
                swept: leaf.swept,
                floor: leaf.floor,
                left_at: leaf.left_at,
            });
        }
        let c = node.cell;
        let v = codes.code([i32::from(c[0]), i32::from(c[1]), i32::from(c[2])]);
        n = shard.child(n, v)?;
    }
}

/// A [`Codes`] that writes down each cell the first time it is asked about,
/// with what it held.
pub(super) struct Reads<'a, C: Codes + ?Sized> {
    inner: &'a C,
    seen: std::cell::RefCell<ReadSeq>,
    stamp: u32,
}

/// The cells a flight read, in the order it first read them, with their codes.
type ReadSeq = Vec<([i32; 3], Code)>;

/// The cells the flight a thread is recording has read: an open-addressed
/// table of cells, each slot stamped with the flight it belongs to, so a new
/// flight starts with an empty table without clearing it. A flight reads a
/// few hundred cells, most of them many times.
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
    static SEEN: std::cell::RefCell<Seen> = const {
        std::cell::RefCell::new(Seen {
            stamp: 0,
            slots: Vec::new(),
            held: 0,
        })
    };
    static STAMP: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
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
        // A stamp no slot carries yet: the table is wiped when they wrap.
        let stamp = STAMP.with(|s| {
            let v = s.get().wrapping_add(1).max(1);
            if v == 1 {
                SEEN.with(|t| t.borrow_mut().slots.iter_mut().for_each(|x| x.0 = 0));
            }
            s.set(v);
            v
        });
        Reads {
            inner,
            seen: std::cell::RefCell::new(Vec::new()),
            stamp,
        }
    }
}

impl<C: Codes + ?Sized> Codes for Reads<'_, C> {
    fn code(&self, c: [i32; 3]) -> Code {
        let v = self.inner.code(c);
        if SEEN.with(|t| t.borrow_mut().insert(self.stamp, c)) {
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
    if HELD.load(Relaxed) > MEMO_NODES {
        return;
    }
    let seq = reads.seen.into_inner();
    let Some(seq) = seq
        .iter()
        .map(|(c, v)| small(*c).map(|c| (c, *v)))
        .collect::<Option<Vec<_>>>()
    else {
        return;
    };
    let mut guard = shards()[key.shard()].write().expect("flight memo");
    let shard = &mut *guard;
    let before = shard.len;
    // The node after `i` cells of the flight were read: the next cell to
    // read, or the flight's leaf.
    let fresh = |shard: &mut Shard, i: usize, code: Code| -> u32 {
        let node = match seq.get(i) {
            Some((cell, _)) => Node {
                cell: *cell,
                code,
                child: NONE,
                sibling: NONE,
            },
            None => {
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
                Node {
                    cell: [0; 3],
                    code,
                    child: LEAF | (shard.leaves.len() - 1) as u32,
                    sibling: NONE,
                }
            }
        };
        shard.push(node)
    };
    let mut n = match shard.roots.get(&key) {
        Some(&n) => n,
        None => {
            let n = fresh(shard, 0, 0);
            shard.roots.insert(key, n);
            n
        }
    };
    for (i, (cell, v)) in seq.iter().enumerate() {
        let node = *shard.node(n);
        if (node.child != NONE && node.child & LEAF != 0) || node.cell != *cell {
            break;
        }
        let k = match shard.child(n, *v) {
            Some(k) => k,
            None => {
                let k = fresh(shard, i + 1, *v);
                shard.node_mut(k).sibling = node.child;
                shard.node_mut(n).child = k;
                k
            }
        };
        n = k;
    }
    // The walk ends on this flight's leaf, new or already recorded; a cell to
    // read there is a flight that read on past where this one stopped, and
    // the flight is not kept.
    HELD.fetch_add(u64::from(shard.len - before), Relaxed);
}
