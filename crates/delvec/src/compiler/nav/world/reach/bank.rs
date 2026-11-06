//! **The node bank**: every node a graph flew, kept with the cells its launches
//! and its world queries read and what each held, for every later graph over a
//! world of the same window, ambient and launch set (spec-0113 §7).
//!
//! A flown node's edges are a deterministic function of what it read: the
//! snapshot codes its launches, edge positions and template checks asked for,
//! the world's own cell sets around the node and around every cell a launch
//! came to rest in (the rest states and the walk relation the edges are
//! judged by), and the killing volumes near the box its launches swept. A node
//! is written down with each of those cells — its snapshot code and its raw
//! cell-set membership — and the volumes near its box. A later graph whose
//! world holds the same at every one of them has the same node, whatever else
//! differs; the lineage ([`super::GraphMemo::lineage`]) asks the same question
//! of one earlier graph over a box, and this asks it of every earlier graph
//! over the cells themselves.
//!
//! A door's or a trapdoor's leaf is read cell by cell, and every point read of
//! one goes through [`leaf_read`]: within an expansion, the cell is noted as
//! read in the run (or scope) it was read in, and the box of the leaf cells a
//! run read joins the box it grew the node's read box by. A cell's recorded
//! fact ([`super::Raw::at`]) carries whether it holds a leaf and, where it
//! does, the leaf's ordinal, so a world whose leaf differs at a cell a node
//! read misses that node, and one whose leaves differ anywhere else takes it.
//! What a world query reads beyond a cell's neighbourhood is part of the
//! bank's own key (whether the world holds any climb or leaf, a hand's use of
//! a leaf, and every leaf outside the window), or of the node's variant (where
//! the declared jumps out of it land), so two worlds that differ in it never
//! share a node.
//!
//! A node no variant holds for is flown again — but not all of it. Its
//! launches are taken in [`GROUPS`] runs of template arcs, in their order, and
//! each run is written down with the box of the cells it read and what it did
//! (its edges as first named, its leaving the world and first death, its
//! counts, the box it grew the node's read box by). Against the variant that
//! differs from this world in the fewest of its cells, a run whose box holds
//! none of those cells did the same here and is taken as written; the rest
//! are flown. Runs merge in their order as the launches do, first naming
//! first, so the node is the node one pass over every launch gives. Each
//! cell a variant records carries the runs that read it, and a run taken as
//! written notes those cells and no others: what a node records is the cells
//! its head and its runs read, whichever variant (if any) it took runs from,
//! so a variant is a function of the world and the node alone and never of
//! which graph banked first.
//!
//! A node keeps at most [`VARIANTS`] variants, the same ones whatever order
//! they arrive in: one equal to a kept variant is not kept again, and past
//! the bound the variants kept are those first in the order of their bytes'
//! digest. The bank's census and its file are therefore the same in every run
//! that asks the same nodes of the same worlds, at any thread count.
//!
//! The bank is an accelerator, never a premise: what no variant answers is
//! flown, and a variant that holds answers what flying would.

use super::{Code, Death, Dense, EdgeKind, NodeEdges, Raw};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// An inclusive cell box.
pub(super) type Box3 = ([i32; 3], [i32; 3]);

pub(super) fn union(a: Option<Box3>, b: Option<Box3>) -> Option<Box3> {
    match (a, b) {
        (Some(a), Some(b)) => Some((
            [a.0[0].min(b.0[0]), a.0[1].min(b.0[1]), a.0[2].min(b.0[2])],
            [a.1[0].max(b.1[0]), a.1[1].max(b.1[1]), a.1[2].max(b.1[2])],
        )),
        (a, None) => a,
        (None, b) => b,
    }
}

/// Whether the killing volume `v` lies near the cell box `b`: within a cell
/// of its faces — every volume [`super::World::lethal_near`] or a body's box
/// inside `b` could meet.
pub(super) fn near(v: &([i32; 3], [i32; 3]), b: &Box3) -> bool {
    (0..3).all(|i| {
        f64::from(v.0[i]) <= f64::from(b.1[i]) + 2.0
            && f64::from(v.1[i]) + 1.0 >= f64::from(b.0[i]) - 1.0
    })
}

fn inside(c: [i32; 3], b: &Box3) -> bool {
    (0..3).all(|i| b.0[i] <= c[i] && c[i] <= b.1[i])
}

/// How many runs a node's launches are taken in: one bit each in a
/// recorded cell's [`Dep`] runs.
pub(super) const GROUPS: usize = 32;
const _: () = assert!(GROUPS <= u32::BITS as usize);

/// One cell a variant read: its window index, its snapshot code, its raw
/// fact, and the runs that read it (bit `g` for run `g`; none for a cell only
/// the node's head read).
pub(super) type Dep = (u32, Code, u32, u32);

/// The window cells one node's expansion read, as a bitmap over the window
/// with the words it set, and the box of the cells the current run read. The
/// bitmap is the thread's, taken clear and handed back clear.
pub(super) struct Track<'a> {
    dense: &'a Dense,
    bits: Vec<Cell<u64>>,
    touched: RefCell<Vec<u32>>,
    /// The cells noted since the current run began, repeats included.
    run_cells: RefCell<Vec<u32>>,
    /// Each run's cells, with the run's bit.
    run_reads: RefCell<Vec<(u32, u32)>>,
    /// The current run's box, empty while `lo > hi`.
    lo: [Cell<i32>; 3],
    hi: [Cell<i32>; 3],
}

thread_local! {
    static BITS: RefCell<Vec<Cell<u64>>> = const { RefCell::new(Vec::new()) };
    static LEAF_READS: RefCell<Option<LeafReads>> = const { RefCell::new(None) };
}

/// The leaf cells the world was asked about within the expansion running on
/// this thread: those its track has not yet noted, and the box of those read
/// since the expansion last asked.
#[derive(Default)]
struct LeafReads {
    tracked: bool,
    pending: Vec<[i32; 3]>,
    since: Option<Box3>,
}

/// Note that the world was asked for the leaf at `c` ([`super::World`]'s one
/// point read of a leaf). Nothing outside an expansion.
pub(super) fn leaf_read(c: [i32; 3]) {
    LEAF_READS.with(|r| {
        if let Some(r) = r.borrow_mut().as_mut() {
            if r.tracked {
                r.pending.push(c);
            }
            r.since = union(r.since, Some((c, c)));
        }
    });
}

/// One expansion's watch over the leaf cells the world is asked about, on
/// its thread, for as long as it lives; `tracked` when a [`Track`] notes
/// them. Expansions on one thread never nest.
pub(super) struct LeafWatch;

impl LeafWatch {
    pub(super) fn open(tracked: bool) -> LeafWatch {
        LEAF_READS.with(|r| {
            let mut r = r.borrow_mut();
            assert!(
                r.is_none(),
                "an expansion runs inside another on one thread"
            );
            *r = Some(LeafReads {
                tracked,
                ..LeafReads::default()
            });
        });
        LeafWatch
    }

    /// The box of the leaf cells read since the last call, emptied.
    pub(super) fn since(&self) -> Option<Box3> {
        LEAF_READS.with(|r| r.borrow_mut().as_mut().and_then(|r| r.since.take()))
    }
}

impl Drop for LeafWatch {
    fn drop(&mut self) {
        LEAF_READS.with(|r| *r.borrow_mut() = None);
    }
}

/// The leaf cells read that no track has noted yet, emptied.
fn pending_leaf_reads() -> Vec<[i32; 3]> {
    LEAF_READS.with(|r| {
        r.borrow_mut()
            .as_mut()
            .map(|r| std::mem::take(&mut r.pending))
            .unwrap_or_default()
    })
}

impl Drop for Track<'_> {
    fn drop(&mut self) {
        for &w in self.touched.borrow().iter() {
            self.bits[w as usize].set(0);
        }
        let bits = std::mem::take(&mut self.bits);
        BITS.with(|b| *b.borrow_mut() = bits);
    }
}

impl<'a> Track<'a> {
    pub(super) fn new(dense: &'a Dense) -> Track<'a> {
        let n = dense.cells.len().div_ceil(64);
        let mut bits = BITS.with(|b| std::mem::take(&mut *b.borrow_mut()));
        if bits.len() < n {
            bits.resize_with(n, || Cell::new(0));
        }
        Track {
            dense,
            bits,
            touched: RefCell::new(Vec::new()),
            run_cells: RefCell::new(Vec::new()),
            run_reads: RefCell::new(Vec::new()),
            lo: [
                Cell::new(i32::MAX),
                Cell::new(i32::MAX),
                Cell::new(i32::MAX),
            ],
            hi: [
                Cell::new(i32::MIN),
                Cell::new(i32::MIN),
                Cell::new(i32::MIN),
            ],
        }
    }

    #[inline]
    fn set(&self, i: usize) {
        let w = &self.bits[i / 64];
        let old = w.get();
        if old == 0 {
            self.touched.borrow_mut().push((i / 64) as u32);
        }
        w.set(old | 1 << (i % 64));
        self.run_cells.borrow_mut().push(i as u32);
    }

    /// Note the window cell `c`, at index `i`.
    #[inline(always)]
    pub(super) fn note(&self, i: usize, c: [i32; 3]) {
        self.set(i);
        for ((lo, hi), v) in self.lo.iter().zip(&self.hi).zip(c) {
            lo.set(lo.get().min(v));
            hi.set(hi.get().max(v));
        }
    }

    /// Grow the current run's box by `b`.
    #[inline]
    pub(super) fn extend(&self, b: Box3) {
        for k in 0..3 {
            self.lo[k].set(self.lo[k].get().min(b.0[k]));
            self.hi[k].set(self.hi[k].get().max(b.1[k]));
        }
    }

    /// Note every leaf cell read since the last look: a window cell as read,
    /// and any cell into the current box (a leaf outside the window is the
    /// bank's key's).
    fn note_leaf_reads(&self) {
        for c in pending_leaf_reads() {
            match self.dense.index(c) {
                Some(i) => self.note(i, c),
                None => self.extend((c, c)),
            }
        }
    }

    /// The current run's box, emptied.
    fn take(&self) -> Option<Box3> {
        self.note_leaf_reads();
        let b = (
            [self.lo[0].get(), self.lo[1].get(), self.lo[2].get()],
            [self.hi[0].get(), self.hi[1].get(), self.hi[2].get()],
        );
        for k in 0..3 {
            self.lo[k].set(i32::MAX);
            self.hi[k].set(i32::MIN);
        }
        (b.0[0] <= b.1[0]).then_some(b)
    }

    /// Note every window cell of the inclusive box `lo..=hi`.
    pub(super) fn mark_box(&self, lo: [i32; 3], hi: [i32; 3]) {
        for x in lo[0]..=hi[0] {
            for z in lo[2]..=hi[2] {
                for y in lo[1]..=hi[1] {
                    if let Some(i) = self.dense.index([x, y, z]) {
                        self.set(i);
                    }
                }
            }
        }
        self.extend((lo, hi));
    }

    /// Start a run: its box and its cells are empty. What was read before it.
    pub(super) fn begin_run(&self) -> Option<Box3> {
        let b = self.take();
        self.run_cells.borrow_mut().clear();
        b
    }

    /// End a run: its box.
    pub(super) fn end_run(&self) -> Option<Box3> {
        self.take()
    }

    /// File the cells noted since the current run began as run `g`'s.
    fn file_run(&self, g: usize) {
        let mut cells = std::mem::take(&mut *self.run_cells.borrow_mut());
        cells.sort_unstable();
        cells.dedup();
        self.run_reads
            .borrow_mut()
            .extend(cells.into_iter().map(|i| (i, 1u32 << g)));
    }

    /// Set the current box aside to measure what one computation reads.
    pub(super) fn begin_scope(&self) -> Option<Box3> {
        self.take()
    }

    /// The box the computation since [`Track::begin_scope`] read; the run's
    /// box takes it in.
    pub(super) fn end_scope(&self, saved: Option<Box3>) -> Option<Box3> {
        let inner = self.take();
        if let Some(b) = union(saved, inner) {
            self.extend(b);
        }
        inner
    }

    /// The cells noted, each with its snapshot code, its raw fact and the
    /// runs filed as reading it.
    fn deps(&self, raw: &Raw) -> Vec<Dep> {
        self.note_leaf_reads();
        let mut reads = self.run_reads.borrow().clone();
        reads.sort_unstable();
        let mut reads = reads.into_iter().peekable();
        let mut out = Vec::new();
        let mut words = self.touched.borrow().clone();
        words.sort_unstable();
        for w in words {
            let w = w as usize;
            let mut b = self.bits[w].get();
            while b != 0 {
                let i = w * 64 + b.trailing_zeros() as usize;
                b &= b - 1;
                let mut runs = 0;
                while let Some(&(j, g)) = reads.peek() {
                    if j as usize > i {
                        break;
                    }
                    runs |= g;
                    reads.next();
                }
                out.push((i as u32, self.dense.cells.get(i), raw.at(i), runs));
            }
        }
        out
    }
}

/// What one run of a node's launches did: the edges it named first (in
/// edge order), whether a launch left the world, its first death, its counts
/// and the box it grew the node's read box by.
#[derive(Debug, Clone, Default)]
pub(super) struct Summary {
    pub(super) edges: Vec<NamedEdge>,
    pub(super) leaves_world: bool,
    pub(super) dies: Option<Death>,
    pub(super) simulated: u64,
    pub(super) credited: u64,
    pub(super) grown: Option<Box3>,
}

/// An edge as a run names it: its end and kind, and the facing and gait of
/// the launch that named it first.
pub(super) type NamedEdge = (([i32; 3], EdgeKind), (Option<i16>, bool));

/// One run of a recorded expansion.
#[derive(Debug, Clone)]
pub(super) struct Run {
    /// Every window cell the run read lies in this box.
    pub(super) bbox: Option<Box3>,
    pub(super) summary: Summary,
}

/// The killing volumes near a box, as the bank compares them.
pub(super) type Volumes = Vec<([i32; 3], [i32; 3])>;

/// One recorded expansion of a node.
pub(super) struct Variant {
    pub(super) deps: Vec<Dep>,
    /// Every cell the expansion read before its first run lies in this box:
    /// the node's own queries.
    pub(super) head: Option<Box3>,
    /// Where the declared jumps out of the node land.
    pub(super) jumps: Vec<[i32; 3]>,
    pub(super) volumes: Volumes,
    pub(super) runs: Vec<Run>,
    pub(super) edges: Arc<NodeEdges>,
}

/// One expansion as the bank follows it: the cells it reads, the variant it
/// may take runs from, and the runs it takes.
pub(super) struct Expansion<'a> {
    pub(super) track: Track<'a>,
    prior: Option<Prior>,
    bank: &'a Bank,
    runs: RefCell<Vec<Run>>,
    head: Cell<Option<Option<Box3>>>,
}

/// The recorded variant an expansion takes runs from, and the window cells
/// this world holds otherwise than it.
struct Prior {
    variant: Arc<Variant>,
    diff: Vec<[i32; 3]>,
    /// The volumes near the variant's read box in this world.
    volumes: Volumes,
}

impl Expansion<'_> {
    /// Start a run; the first keeps what was read before it.
    pub(super) fn begin_run(&self) {
        let b = self.track.begin_run();
        if self.head.get().is_none() {
            self.head.set(Some(b));
        }
    }

    /// The recorded run `g` of `n`, if it holds here; the cells it read are
    /// noted as read and its box made the current run's.
    pub(super) fn reuse(&self, g: usize, n: usize) -> Option<Summary> {
        let p = self.prior.as_ref()?;
        if p.variant.runs.len() != n {
            return None;
        }
        let run = &p.variant.runs[g];
        // A run's launches meet a volume only near the box they grew the read
        // box by: the volumes near it must be the same here.
        if let Some(gb) = run.summary.grown {
            let here =
                |vs: &Volumes| -> Volumes { vs.iter().filter(|v| near(v, &gb)).copied().collect() };
            if here(&p.volumes) != here(&p.variant.volumes) {
                return None;
            }
        }
        if let Some(b) = run.bbox {
            if p.diff.iter().any(|d| inside(*d, &b)) {
                return None;
            }
            // The cells run `g` read, and no other cell of its box: one the
            // variant's other runs read there is this node's only where its
            // own runs read it, so what this expansion records is the same
            // whichever variant it took runs from.
            for &(i, _, _, runs) in &p.variant.deps {
                if runs & 1 << g != 0 {
                    self.track
                        .note(i as usize, self.track.dense.coords(i as usize));
                }
            }
            self.track.extend(b);
        }
        self.bank
            .reused
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Some(run.summary.clone())
    }

    /// Write down the next run, which ended with box `bbox`, and the cells
    /// it read.
    pub(super) fn ran(&self, bbox: Option<Box3>, summary: Summary) {
        let mut runs = self.runs.borrow_mut();
        self.track.file_run(runs.len());
        runs.push(Run { bbox, summary });
    }
}

/// One kept variant: where it lies in the spill ([`super::spill`]), or the
/// variant itself for a bank whose spill could not be opened (a spill that
/// opened and then fails stops the run: one policy for every scratch file,
/// `spill::Spill::fail`). What a node's
/// expansion read is the bulk of a variant — every window cell, at twelve
/// bytes — and a build keeps one variant per node it flew for the life of the
/// process, so the bank keeps a slot per variant in memory and the file
/// holds the rest.
/// A slot on disk shares its record with every copy taken to read it, so a
/// variant that leaves the bank frees its room in the file only once no read
/// of it is under way.
#[derive(Clone)]
enum Slot {
    Disk {
        ext: Arc<super::spill::Extent>,
        digest: u64,
    },
    Held {
        v: Arc<Variant>,
        len: u32,
        digest: u64,
    },
}

impl Slot {
    fn digest(&self) -> u64 {
        match self {
            Slot::Disk { digest, .. } | Slot::Held { digest, .. } => *digest,
        }
    }
    fn len(&self) -> u64 {
        match self {
            Slot::Disk { ext, .. } => u64::from(ext.at().1),
            Slot::Held { len, .. } => u64::from(*len),
        }
    }
}

/// The first eight bytes of a variant's encoding's sha256: the order a
/// node's variants are kept in, and the test of two being one.
fn digest(bytes: &[u8]) -> u64 {
    use sha2::{Digest, Sha256};
    let h = Sha256::digest(bytes);
    u64::from_le_bytes(h[..8].try_into().expect("eight bytes"))
}

/// The bank of one window, ambient, launch set and non-local world facts.
pub(super) struct Bank {
    nodes: Mutex<HashMap<super::FlownKey, Vec<Slot>>>,
    spill: Mutex<Option<super::spill::Spill>>,
    /// Runs taken as written, for the agreement test's binding.
    reused: std::sync::atomic::AtomicU64,
    /// The bank's key, which names its file ([`super::persist`]).
    key: String,
    /// Whether the variants kept changed since the bank was read from its
    /// file: a bank that did not is not written back.
    changed: std::sync::atomic::AtomicBool,
}

/// How many variants of one node the bank keeps: a bound on its memory, never
/// on what a build answers.
pub(super) const VARIANTS: usize = 8;

/// **The bank's cap** (ADR-0037 §3): the bytes every bank of the process may
/// keep in its file between them, set by the build from its cost before the
/// proofs run ([`set_cap`]: the graph's own tables' and lists' bound) and
/// unbounded where no build set one. A variant that would pass it is not
/// kept — the expansion's answer is the same, the next world that asks
/// re-flies the node, and no output byte reads the bank — so the file is
/// bounded by what the plan states, never by the nodes a run flew. Under a
/// test's cap ([`with_cap`], carried to the workers that expand for the
/// thread) each variant is judged alone against it, and what was refused
/// is counted for the test over the process.
static CAP: AtomicU64 = AtomicU64::new(u64::MAX);
/// Bytes the variants kept hold, over every bank, under the cap.
static KEPT: AtomicU64 = AtomicU64::new(0);
/// Variants refused at the cap, over the run.
static REFUSED: AtomicU64 = AtomicU64::new(0);
/// Variants refused under a test's cap, over the process: the expansions
/// run on worker threads, so a test reads the count before and after.
static REFUSED_UNDER_TEST: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static CAP_HERE: Cell<u64> = const { Cell::new(0) };
}

/// The build's cap on what the banks keep, in bytes.
pub(super) fn set_cap(bytes: u64) {
    CAP.store(bytes, Ordering::Relaxed);
}

/// The cap stated and the variants refused at it over the run; `None` where
/// no build set one.
pub(super) fn cap_stated() -> Option<(u64, u64)> {
    let c = CAP.load(Ordering::Relaxed);
    (c != u64::MAX).then(|| (c, REFUSED.load(Ordering::Relaxed)))
}

/// A test's cap in force on this thread, `0` for none.
pub(super) fn cap_here() -> u64 {
    CAP_HERE.with(Cell::get)
}

/// `f` with a test's cap of `bytes` in force on this thread (`0`: none).
pub(super) fn with_cap<R>(bytes: u64, f: impl FnOnce() -> R) -> R {
    let was = CAP_HERE.with(|c| c.replace(bytes));
    let out = f();
    CAP_HERE.with(|c| c.set(was));
    out
}

/// Variants refused under a test's cap so far, over the process.
#[cfg(test)]
pub(super) fn refused_under_test() -> u64 {
    REFUSED_UNDER_TEST.load(Ordering::Relaxed)
}

/// Whether a variant of `len` bytes may be kept now: counted against the
/// cap in force, and counted back if it may not.
fn admit(len: u64) -> bool {
    let here = cap_here();
    if here != 0 {
        if len > here {
            REFUSED_UNDER_TEST.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        return true;
    }
    let cap = CAP.load(Ordering::Relaxed);
    if KEPT.fetch_add(len, Ordering::Relaxed) + len > cap {
        KEPT.fetch_sub(len, Ordering::Relaxed);
        REFUSED.fetch_add(1, Ordering::Relaxed);
        return false;
    }
    true
}

/// A variant kept under the cap let go: its bytes counted back.
fn released(len: u64) {
    if cap_here() == 0 {
        KEPT.fetch_sub(len, Ordering::Relaxed);
    }
}

impl Bank {
    /// A bank keyed `key` holding `nodes`, spilled.
    fn new(key: String, nodes: Vec<(super::FlownKey, Vec<Arc<Variant>>)>) -> Bank {
        let b = Bank {
            nodes: Mutex::new(HashMap::new()),
            spill: Mutex::new(super::spill::Spill::open("bank").ok()),
            reused: Default::default(),
            key,
            changed: Default::default(),
        };
        for (k, vs) in nodes {
            for v in vs {
                b.put(k, &v);
            }
        }
        b
    }

    /// Spill `v` and record it under `key`, unless the node holds it already
    /// or holds [`VARIANTS`] that come before it in digest order; past the
    /// bound, the last in that order leaves.
    fn put(&self, key: super::FlownKey, v: &Arc<Variant>) {
        // Judged and written under the one lock, so two threads keeping one
        // node at once cannot pass the bound between them. A node's slots
        // stand in digest order: which variants a node keeps, and the order
        // its file lists them in, are the same whatever order they arrive in.
        let bytes = super::persist::encode_variant(v);
        let d = digest(&bytes);
        let mut nodes = self.nodes.lock().expect("bank");
        let vs = nodes.entry(key).or_default();
        let Err(at) = vs.binary_search_by_key(&d, Slot::digest) else {
            return;
        };
        if at >= VARIANTS {
            return;
        }
        if !admit(bytes.len() as u64) {
            return;
        }
        let slot = {
            let mut spill = self.spill.lock().expect("spill");
            match spill.as_mut() {
                Some(s) => {
                    let r = s.put(&bytes);
                    Slot::Disk {
                        ext: Arc::new(s.or_fail("take a write", r)),
                        digest: d,
                    }
                }
                None => Slot::Held {
                    v: Arc::clone(v),
                    len: u32::try_from(bytes.len()).expect("a variant under 4 GiB"),
                    digest: d,
                },
            }
        };
        vs.insert(at, slot);
        if vs.len() > VARIANTS {
            for gone in vs.drain(VARIANTS..) {
                released(gone.len());
            }
        }
        self.changed
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Every variant kept under `key`, read back; none for a node never kept.
    fn variants(&self, key: &super::FlownKey) -> Vec<Arc<Variant>> {
        let slots = match self.nodes.lock().expect("bank").get(key) {
            Some(s) => s.clone(),
            None => return Vec::new(),
        };
        let mut out = Vec::with_capacity(slots.len());
        for s in slots {
            match s {
                Slot::Held { v, .. } => out.push(v),
                Slot::Disk { ext, .. } => {
                    let (at, len) = ext.at();
                    let bytes = {
                        let g = self.spill.lock().expect("spill");
                        let sp = g.as_ref().expect("a variant on disk has its spill");
                        sp.or_fail("read a variant back", sp.read(at, len))
                    };
                    let v = super::persist::decode_variant(&bytes)
                        .expect("a variant reads back as it was written");
                    out.push(Arc::new(v));
                }
            }
        }
        out
    }

    /// The edges a recorded expansion of `key` gives a world whose snapshot is
    /// `dense` with raw membership `extra`, if one holds; `volumes` says which
    /// killing volumes lie near a read box.
    pub(super) fn find(
        &self,
        key: &super::FlownKey,
        dense: &Dense,
        raw: &Raw,
        jumps: &[[i32; 3]],
        volumes: &dyn Fn(Box3) -> Volumes,
    ) -> Option<Arc<NodeEdges>> {
        let variants = self.variants(key);
        variants.iter().find_map(|v| {
            let held = v.jumps == jumps
                && v.deps.iter().all(|&(i, code, x, _)| {
                    dense.cells.get(i as usize) == code && raw.at(i as usize) == x
                });
            (held && v.edges.read.is_some_and(|b| volumes(b) == v.volumes))
                .then(|| Arc::clone(&v.edges))
        })
    }

    /// A new expansion of `key`, taking runs from the recorded variant that
    /// differs from this world in the fewest cells — when none of those
    /// cells is one the node's own queries read (`node`) and the volumes near
    /// it are the same.
    pub(super) fn expansion<'a>(
        &'a self,
        key: &super::FlownKey,
        dense: &'a Dense,
        raw: &Raw,
        node: &[Box3],
        jumps: &[[i32; 3]],
        volumes: &dyn Fn(Box3) -> Volumes,
    ) -> Expansion<'a> {
        let variants = self.variants(key);
        let mut best: Option<Prior> = None;
        for v in variants {
            if v.jumps != jumps {
                continue;
            }
            let diff: Vec<[i32; 3]> = v
                .deps
                .iter()
                .filter(|&&(i, code, x, _)| {
                    dense.cells.get(i as usize) != code || raw.at(i as usize) != x
                })
                .map(|&(i, _, _, _)| dense.coords(i as usize))
                .collect();
            let Some(read) = v.edges.read else {
                continue;
            };
            // The node's own queries: no changed cell among them or in what
            // it read before its first run, and the volumes near them the
            // same.
            let here = volumes(read);
            let at_node = |vs: &Volumes| -> Volumes {
                vs.iter()
                    .filter(|x| node.iter().any(|b| near(x, b)))
                    .copied()
                    .collect()
            };
            if diff.iter().any(|d| {
                node.iter().any(|b| inside(*d, b)) || v.head.is_some_and(|h| inside(*d, &h))
            }) || at_node(&here) != at_node(&v.volumes)
            {
                continue;
            }
            if best.as_ref().is_none_or(|b| diff.len() < b.diff.len()) {
                best = Some(Prior {
                    variant: v,
                    diff,
                    volumes: here,
                });
            }
        }
        Expansion {
            track: Track::new(dense),
            prior: best,
            bank: self,
            runs: RefCell::new(Vec::new()),
            head: Cell::new(None),
        }
    }

    /// Record an expansion of `key`: the variant it makes.
    pub(super) fn keep(
        &self,
        key: super::FlownKey,
        expansion: Expansion<'_>,
        raw: &Raw,
        jumps: Vec<[i32; 3]>,
        volumes: Volumes,
        edges: &Arc<NodeEdges>,
    ) -> Arc<Variant> {
        let deps = expansion.track.deps(raw);
        let head = expansion.head.get().flatten();
        let runs = expansion.runs.into_inner();
        let v = Arc::new(Variant {
            deps,
            head,
            jumps,
            volumes,
            runs,
            edges: Arc::clone(edges),
        });
        self.put(key, &v);
        v
    }

    /// Every variant the bank holds, read back, in key order.
    /// Every variant, chunk by chunk ([`super::chunks::SIDE`]) and in key
    /// order within a chunk, read back from the spill one chunk at a time.
    fn held_by_chunk(&self) -> impl Iterator<Item = super::persist::Section> + '_ {
        let mut keys: Vec<super::FlownKey> =
            self.nodes.lock().expect("bank").keys().copied().collect();
        let side = super::chunks::SIDE;
        keys.sort_by_key(|k| (super::chunks::chunk_of(k.0, side), *k));
        let mut groups: Vec<Vec<super::FlownKey>> = Vec::new();
        for k in keys {
            match groups.last_mut() {
                Some(g)
                    if super::chunks::chunk_of(g[0].0, side)
                        == super::chunks::chunk_of(k.0, side) =>
                {
                    g.push(k)
                }
                _ => groups.push(vec![k]),
            }
        }
        groups
            .into_iter()
            .map(|g| g.into_iter().map(|k| (k, self.variants(&k))).collect())
    }

    #[cfg(test)]
    fn held_all(&self) -> Vec<(super::FlownKey, Vec<Arc<Variant>>)> {
        let keys: Vec<super::FlownKey> = self.nodes.lock().expect("bank").keys().copied().collect();
        let mut v: Vec<_> = keys.into_iter().map(|k| (k, self.variants(&k))).collect();
        v.sort_by_key(|n| n.0);
        v
    }

    /// How many variants the bank holds, and the bytes they take in the spill
    /// (a variant a node let go past its bound is not counted).
    pub(super) fn census(&self) -> (usize, u64) {
        let nodes = self.nodes.lock().expect("bank");
        let n = nodes.values().map(Vec::len).sum();
        let bytes = nodes.values().flatten().map(Slot::len).sum();
        (n, bytes)
    }
}

impl Bank {
    /// How many runs this bank's expansions took as written.
    #[cfg(test)]
    pub(super) fn reused_runs(&self) -> u64 {
        self.reused.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// How many nodes it holds two or more variants of.
    #[cfg(test)]
    pub(super) fn revisited(&self) -> usize {
        self.nodes
            .lock()
            .expect("bank")
            .values()
            .filter(|v| v.len() > 1)
            .count()
    }
}

type Banks = Mutex<HashMap<String, Arc<Bank>>>;

fn banks() -> &'static Banks {
    static BANKS: std::sync::OnceLock<Banks> = std::sync::OnceLock::new();
    BANKS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The bank shared by every graph of `key`: on first asking, what the cache
/// directory holds for it ([`super::persist`]).
pub(super) fn bank(key: String) -> Arc<Bank> {
    let mut banks = banks().lock().expect("banks");
    if let Some(b) = banks.get(&key) {
        return Arc::clone(b);
    }
    // Read chunk by chunk into the bank's spill; a file that does not
    // decode whole leaves an empty bank, never part of one.
    let mut b = Bank::new(key.clone(), Vec::new());
    let mut read = 0usize;
    let whole = super::persist::load(&key, &mut |section| {
        for (k, vs) in section {
            read += vs.len();
            for v in vs {
                b.put(k, &v);
            }
        }
    });
    if !whole && read > 0 {
        b = Bank::new(key.clone(), Vec::new());
    }
    if whole {
        b.changed.store(false, std::sync::atomic::Ordering::Relaxed);
    }
    let b = Arc::new(b);
    banks.insert(key, Arc::clone(&b));
    b
}

/// Write every bank whose kept variants changed this run to the cache directory,
/// in key order; nothing where the cache is off.
pub(super) fn persist_all() {
    let banks: Vec<Arc<Bank>> = banks().lock().expect("banks").values().cloned().collect();
    for b in banks {
        if super::persist::file_of(&b.key).is_none() {
            continue;
        }
        if !b.changed.load(std::sync::atomic::Ordering::Relaxed) {
            continue;
        }
        super::persist::store(&b.key, &mut b.held_by_chunk());
    }
}

impl Bank {
    /// What this bank's scratch file has held.
    #[cfg(test)]
    pub(super) fn usage(&self) -> super::spill::Usage {
        self.spill
            .lock()
            .expect("spill")
            .as_ref()
            .map_or(Default::default(), super::spill::Spill::usage)
    }
}

/// Every bank's census, summed: variants kept and bytes spilled.
/// What every bank's scratch file has held.
pub(super) fn usage_all() -> super::spill::Usage {
    let banks: Vec<Arc<Bank>> = banks().lock().expect("banks").values().cloned().collect();
    banks
        .iter()
        .filter_map(|b| {
            b.spill
                .lock()
                .expect("spill")
                .as_ref()
                .map(super::spill::Spill::usage)
        })
        .fold(Default::default(), |a, b| a + b)
}

pub(super) fn census_all() -> (usize, u64) {
    let banks: Vec<Arc<Bank>> = banks().lock().expect("banks").values().cloned().collect();
    banks
        .iter()
        .map(|b| b.census())
        .fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1))
}

#[cfg(test)]
impl Bank {
    /// Every variant the bank holds, in key order.
    pub(super) fn held(&self) -> Vec<(super::FlownKey, Vec<Arc<Variant>>)> {
        self.held_all()
    }

    /// A bank holding `nodes`, as one read from a file is.
    pub(super) fn holding(nodes: Vec<(super::FlownKey, Vec<Arc<Variant>>)>) -> Bank {
        Bank::new(String::from("test"), nodes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A node keeps its [`VARIANTS`] and no more, the same ones in the same
    /// order whatever order they arrive in: a variant equal to one kept is
    /// not kept again, and past the bound the first in digest order stay.
    /// The census counts the variants kept and their bytes, so two runs that
    /// are offered the same variants report the same figures.
    #[test]
    fn a_node_keeps_the_same_variants_in_any_order() {
        let key: super::super::FlownKey = ([0, 0, 0], 0);
        let offered: Vec<Arc<Variant>> = (0..VARIANTS as u32 + 3)
            .map(|k| {
                Arc::new(Variant {
                    deps: (0..=k).map(|i| (i, 0, 2, 1 << (i % 32))).collect(),
                    head: None,
                    jumps: vec![[1, 2, 3]],
                    volumes: Vec::new(),
                    runs: Vec::new(),
                    edges: Arc::new(NodeEdges::default()),
                })
            })
            .collect();
        let bytes = |v: &Variant| super::super::persist::encode_variant(v);
        let mut first: Vec<&Arc<Variant>> = offered.iter().collect();
        first.sort_by_key(|v| digest(&bytes(v)));
        first.truncate(VARIANTS);
        let want: Vec<Vec<u8>> = first.iter().map(|v| bytes(v)).collect();
        let want_bytes: u64 = want.iter().map(|b| b.len() as u64).sum();
        let orders: [Vec<usize>; 3] = [
            (0..offered.len()).collect(),
            (0..offered.len()).rev().collect(),
            (0..offered.len())
                .map(|i| (i * 5) % offered.len())
                .collect(),
        ];
        for order in &orders {
            let b = Bank::holding(Vec::new());
            for &i in order {
                b.put(key, &offered[i]);
                // Offered twice: kept once.
                b.put(key, &offered[i]);
            }
            assert_eq!(b.census(), (VARIANTS, want_bytes), "{order:?}");
            let kept: Vec<Vec<u8>> = b.variants(&key).iter().map(|v| bytes(v)).collect();
            assert_eq!(kept, want, "{order:?}");
        }
    }

    /// A leaf cell the world is asked about is a cell the expansion read,
    /// wherever it lies: noted in the run it was read in, its box the run's,
    /// and its fact among the deps — whether or not any box the expansion
    /// marked holds it. Outside an expansion, nothing is noted.
    #[test]
    fn a_leaf_read_is_a_cell_the_run_read() {
        let w = super::super::tests::bell_yard();
        let dense = Dense::of(&w);
        let raw = Raw::of(&w, &dense);
        let at = [3, 1, 3];
        let i = dense.index(at).expect("inside the window");
        leaf_read(at);
        let track = Track::new(&dense);
        let watch = LeafWatch::open(true);
        track.begin_run();
        leaf_read(at);
        assert_eq!(
            track.end_run(),
            Some((at, at)),
            "the run's box is the leaf cell"
        );
        assert_eq!(watch.since(), Some((at, at)));
        let deps = track.deps(&raw);
        assert_eq!(deps, vec![(i as u32, dense.cells.get(i), raw.at(i), 0)]);
        drop(watch);
        leaf_read([4, 1, 4]);
        assert_eq!(
            track.deps(&raw).len(),
            1,
            "a read outside an expansion is noted"
        );
    }
}
