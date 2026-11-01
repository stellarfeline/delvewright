//! **Where the reach graph keeps what it keeps for the life of the process**
//! (ADR-0037 §2): every closure's successor and predecessor lists, by **reach
//! chunk** ([`Lists`]), and every graph's node tables, one slot per entry
//! ([`Table`]) — both in one scratch file ([`super::spill`]), read back when a
//! proof asks.
//!
//! A list is keyed by a cell and holds its items in the order they were
//! pushed; the cell's chunk — a full-height column of [`SIDE`] × [`SIDE`]
//! cells — holds it. A chunk is **resident** while it is being read: its
//! records decoded into a map from cell to items. Every chunk of every list
//! counts against one process-wide budget of resident bytes ([`budget`]);
//! past it, the least recently used chunks give their decoded maps up, and the
//! records not yet written go to the file first. Records pushed to a chunk that
//! is not resident are buffered and written once they pass [`PENDING`] bytes,
//! so a closure being built never decodes the chunks it only adds to.
//!
//! What a list answers never depends on the chunk side, the budget or which
//! chunks are resident: a chunk's items are its spilled records, in the order
//! they were written, then its buffered ones. A copy of a list
//! ([`Lists::clone`]) shares the spilled records, which are never rewritten,
//! and copies only the buffered ones.
//!
//! A node table is read one entry at a time by many threads at once, so its
//! entries are not chunked: each is one record at an offset, read back alone
//! outside the table's lock.

use super::persist::{Dec, Enc};
use super::spill::Spill;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};

/// A reach chunk's side, in columns: a multiple of the game's 16 and at least
/// the window margin, so a node's launches land in its chunk or the eight
/// round it.
pub(super) const SIDE: i32 = 32;

/// Records not yet written are written once a chunk buffers this many bytes.
const PENDING: usize = 64 << 10;

/// The resident budget the engine ships: decoded chunks of every list,
/// together, in bytes — a constant of the engine, never of the map.
const BUDGET: usize = 256 << 20;

/// The resident budget in force on this thread ([`BUDGET`], or what a test
/// set).
pub(super) fn budget() -> usize {
    match BUDGET_HERE.with(std::cell::Cell::get) {
        0 => BUDGET,
        b => b,
    }
}

thread_local! {
    static BUDGET_HERE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
/// Bytes every list holds in memory now: decoded chunks and buffered records.
static RESIDENT: AtomicUsize = AtomicUsize::new(0);
/// The use clock.
static TICK: AtomicU64 = AtomicU64::new(0);

/// Run `f` with the resident budget at `bytes` on this thread: every list
/// this thread uses gives up what it holds past it.
#[cfg(test)]
pub(super) fn with_budget<R>(bytes: usize, f: impl FnOnce() -> R) -> R {
    let was = BUDGET_HERE.with(|b| b.replace(bytes));
    let out = f();
    BUDGET_HERE.with(|b| b.set(was));
    out
}

/// The bytes every list holds in memory now.
pub(super) fn resident() -> usize {
    RESIDENT.load(Ordering::Relaxed)
}

/// The one scratch file every list writes to, behind the writer's lock, and
/// a handle every reader reads it by at an offset, unlocked; both `None` when
/// it could not be opened, and then every list keeps its records in memory.
struct Shared {
    write: Mutex<Option<Spill>>,
    read: Option<std::fs::File>,
}

fn spill() -> &'static Shared {
    static SPILL: OnceLock<Shared> = OnceLock::new();
    SPILL.get_or_init(|| {
        let opened = Spill::open("lists").ok();
        let read = opened.as_ref().and_then(|s| s.reader().ok());
        Shared {
            write: Mutex::new(opened.filter(|_| read.is_some())),
            read,
        }
    })
}

/// The bytes the lists' scratch file holds.
pub(super) fn spilled() -> u64 {
    spill()
        .write
        .lock()
        .expect("spill")
        .as_ref()
        .map_or(0, Spill::len)
}

/// Chunks decoded, the records they held, and chunks given up, over the run.
static LOADS: AtomicU64 = AtomicU64::new(0);
static RECORDS: AtomicU64 = AtomicU64::new(0);
static RELEASES: AtomicU64 = AtomicU64::new(0);

/// How often chunks were decoded, the records decoded, and how often chunks
/// were given up, over the run.
pub(super) fn traffic() -> (u64, u64, u64) {
    (
        LOADS.load(Ordering::Relaxed),
        RECORDS.load(Ordering::Relaxed),
        RELEASES.load(Ordering::Relaxed),
    )
}

/// A chunk written in more pieces than this is written whole when next given
/// up, so a chunk read back reads a few pieces.
const PIECES: usize = 4;

/// What a list holds per cell: written as a record and read back as written.
pub(super) trait Item: Clone + Send + 'static {
    fn put(&self, e: &mut Enc);
    fn take(d: &mut Dec<'_>) -> Option<Self>;
    /// The heap bytes one decoded item holds beyond its own size.
    fn heap(&self) -> usize {
        0
    }
}

/// A reach chunk: its column coordinates, the cell's x and z over the side.
pub(super) type Key = (i32, i32);

/// The chunk a cell lies in, at side `side`.
pub(super) fn chunk_of(c: [i32; 3], side: i32) -> Key {
    (c[0].div_euclid(side), c[2].div_euclid(side))
}

// ------------------------------------------------------------ the visits

/// Visits counted per chunk, past which they are counted together.
const VISIT_BUCKETS: usize = 64;

/// Over the run: how many (closure, chunk) pairs saw each number of visits,
/// the last bucket holding every count from [`VISIT_BUCKETS`] up; and the
/// most visits one chunk had in one closure.
static VISIT_HIST: [AtomicU64; VISIT_BUCKETS + 1] =
    [const { AtomicU64::new(0) }; VISIT_BUCKETS + 1];
static VISIT_MAX: AtomicU64 = AtomicU64::new(0);

/// **How often one closure took up each reach chunk**: a chunk is visited
/// each time the closure closes its frontier ([`super::World::close`]) or
/// flies its nodes' launches ([`super::World::fly_chunks`]). Folded into the
/// run's distribution when the closure ends ([`Visits::report`]).
#[derive(Default)]
pub(super) struct Visits {
    counts: HashMap<Key, u64>,
}

impl Visits {
    pub(super) fn visit(&mut self, k: Key) {
        *self.counts.entry(k).or_insert(0) += 1;
    }

    /// Fold this closure's visits into the run's distribution.
    pub(super) fn report(&mut self) {
        for n in std::mem::take(&mut self.counts).into_values() {
            let b = (n as usize).min(VISIT_BUCKETS);
            VISIT_HIST[b].fetch_add(1, Ordering::Relaxed);
            VISIT_MAX.fetch_max(n, Ordering::Relaxed);
        }
    }
}

/// The run's visit distribution as a line: `n×k` for k (closure, chunk) pairs
/// visited n times, and the most.
pub(super) fn visits() -> String {
    let mut parts = Vec::new();
    let mut pairs = 0u64;
    let mut total = 0u64;
    for (n, b) in VISIT_HIST.iter().enumerate() {
        let k = b.load(Ordering::Relaxed);
        if k > 0 {
            pairs += k;
            total += k * n as u64;
            let plus = if n == VISIT_BUCKETS { "+" } else { "" };
            parts.push(format!("{n}{plus}x{k}"));
        }
    }
    format!(
        "{total} chunk visit(s) over {pairs} (closure, chunk) pair(s), at most {} for one chunk \
         in one closure [{}]",
        VISIT_MAX.load(Ordering::Relaxed),
        parts.join(" ")
    )
}

/// One chunk of a list.
struct Chunk<V> {
    /// The records written to the file, in order.
    segments: Vec<(u64, u32)>,
    /// The records pushed since, encoded.
    pending: Vec<u8>,
    /// Every record decoded, while the chunk is resident.
    resident: Option<HashMap<[i32; 3], Vec<V>>>,
    /// The bytes `resident` holds, estimated.
    decoded: usize,
    /// When it was last used.
    used: u64,
}

impl<V> Default for Chunk<V> {
    fn default() -> Self {
        Chunk {
            segments: Vec::new(),
            pending: Vec::new(),
            resident: None,
            decoded: 0,
            used: 0,
        }
    }
}

/// Bytes a decoded entry costs beyond its items: the key, the vector and the
/// map's slot.
const ENTRY: usize = 64;

impl<V: Item> Chunk<V> {
    fn held(&self) -> usize {
        self.pending.capacity() + self.decoded
    }

    /// Decode every record into the resident map.
    fn load(&mut self) {
        if self.resident.is_some() {
            return;
        }
        let mut map: HashMap<[i32; 3], Vec<V>> = HashMap::new();
        let mut decoded = 0;
        let mut records = 0;
        let mut take = |bytes: &[u8]| {
            let mut d = Dec { b: bytes, at: 0 };
            while d.at < bytes.len() {
                let c = d.cell().expect("a list record's cell reads back");
                let v = V::take(&mut d).expect("a list record reads back");
                records += 1;
                let list = map.entry(c).or_default();
                if list.is_empty() {
                    decoded += ENTRY;
                }
                decoded += std::mem::size_of::<V>() + v.heap();
                list.push(v);
            }
        };
        if !self.segments.is_empty() {
            let file = spill()
                .read
                .as_ref()
                .expect("a list with spilled records has its file");
            for &(at, len) in &self.segments {
                let bytes = super::spill::read_at(file, at, len)
                    .expect("the lists' scratch file reads back");
                take(&bytes);
            }
        }
        take(&self.pending);
        self.resident = Some(map);
        self.decoded = decoded;
        LOADS.fetch_add(1, Ordering::Relaxed);
        RECORDS.fetch_add(records, Ordering::Relaxed);
    }

    /// Write the buffered records to the file, when there is one.
    fn write(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let mut sp = spill().write.lock().expect("spill");
        if let Some(sp) = sp.as_mut() {
            let at = sp
                .append(&self.pending)
                .expect("the lists' scratch file takes a write");
            self.segments.push(at);
            self.pending = Vec::new();
        }
    }

    /// Give up what the chunk holds in memory that the file can hold: a chunk
    /// written in more than [`PIECES`] pieces is written whole, each cell's
    /// items in their order, and read back from that one piece.
    fn release(&mut self) {
        let pieces = self.segments.len() + usize::from(!self.pending.is_empty());
        if let Some(map) = self.resident.as_ref().filter(|_| pieces > PIECES) {
            let mut e = Enc::default();
            for (c, items) in map {
                for v in items {
                    e.cell(*c);
                    v.put(&mut e);
                }
            }
            let mut sp = spill().write.lock().expect("spill");
            if let Some(sp) = sp.as_mut() {
                let at = sp
                    .append(&e.0)
                    .expect("the lists' scratch file takes a write");
                self.segments = vec![at];
                self.pending = Vec::new();
            }
        } else {
            self.write();
        }
        self.resident = None;
        self.decoded = 0;
        RELEASES.fetch_add(1, Ordering::Relaxed);
    }
}

struct Inner<V> {
    side: i32,
    chunks: HashMap<Key, Chunk<V>>,
    /// Bytes counted against [`RESIDENT`] for this list.
    held: usize,
    items: usize,
}

impl<V> Drop for Inner<V> {
    fn drop(&mut self) {
        RESIDENT.fetch_sub(self.held, Ordering::Relaxed);
    }
}

impl<V: Item> Inner<V> {
    /// Push `v` onto `c`'s list; the chunk it went to.
    fn push(&mut self, c: [i32; 3], v: V) -> Key {
        let k = chunk_of(c, self.side);
        let tick = TICK.fetch_add(1, Ordering::Relaxed);
        let ch = self.chunks.entry(k).or_default();
        let before = ch.held();
        let mut e = Enc(std::mem::take(&mut ch.pending));
        e.cell(c);
        v.put(&mut e);
        ch.pending = e.0;
        ch.used = tick;
        if let Some(map) = ch.resident.as_mut() {
            let list = map.entry(c).or_default();
            if list.is_empty() {
                ch.decoded += ENTRY;
            }
            ch.decoded += std::mem::size_of::<V>() + v.heap();
            list.push(v);
        } else if ch.pending.len() >= PENDING {
            ch.write();
        }
        self.items += 1;
        self.recount(k, before);
        k
    }

    /// Re-count chunk `k`'s bytes after a change.
    fn recount(&mut self, k: Key, before: usize) {
        let after = self.chunks.get(&k).map_or(0, Chunk::held);
        if after >= before {
            RESIDENT.fetch_add(after - before, Ordering::Relaxed);
            self.held += after - before;
        } else {
            RESIDENT.fetch_sub(before - after, Ordering::Relaxed);
            self.held -= before - after;
        }
    }

    /// Release the least recently used chunks but `keep` until `want` bytes
    /// are freed; the bytes freed.
    fn evict(&mut self, want: usize, keep: Option<Key>) -> usize {
        let mut order: Vec<(u64, Key)> = self
            .chunks
            .iter()
            .filter(|(k, ch)| Some(**k) != keep && ch.held() > 0)
            .map(|(k, ch)| (ch.used, *k))
            .collect();
        order.sort_unstable();
        let mut freed = 0;
        for (_, k) in order {
            if freed >= want {
                break;
            }
            let ch = self.chunks.get_mut(&k).expect("listed");
            let before = ch.held();
            ch.release();
            let after = ch.held();
            freed += before - after;
            self.recount(k, before);
        }
        freed
    }
}

/// A list another list can ask to give memory up.
trait Evict: Send + Sync {
    fn evict(&self, want: usize) -> usize;
}

impl<V: Item> Evict for Mutex<Inner<V>> {
    fn evict(&self, want: usize) -> usize {
        match self.try_lock() {
            Ok(mut g) => g.evict(want, None),
            Err(_) => 0,
        }
    }
}

type Registry = Mutex<Vec<(usize, Weak<dyn Evict>)>>;

fn registry() -> &'static Registry {
    static R: OnceLock<Registry> = OnceLock::new();
    R.get_or_init(|| Mutex::new(Vec::new()))
}

/// **A list per cell, kept by reach chunk.**
pub(super) struct Lists<V: Item> {
    inner: Arc<Mutex<Inner<V>>>,
}

impl<V: Item> Lists<V> {
    /// An empty list at chunk side `side`.
    pub(super) fn new(side: i32) -> Lists<V> {
        Self::of(Inner {
            side,
            chunks: HashMap::new(),
            held: 0,
            items: 0,
        })
    }

    fn of(inner: Inner<V>) -> Lists<V> {
        let inner = Arc::new(Mutex::new(inner));
        let weak: Weak<dyn Evict> = {
            let w: Weak<Mutex<Inner<V>>> = Arc::downgrade(&inner);
            w
        };
        let id = Arc::as_ptr(&inner) as usize;
        let mut r = registry().lock().expect("registry");
        r.retain(|(_, w)| w.strong_count() > 0);
        r.push((id, weak));
        Lists { inner }
    }

    /// Ask every list, this one first, to give up bytes past the budget.
    fn relieve(&self, g: &mut Inner<V>, keep: Key) {
        let over = |b: usize| RESIDENT.load(Ordering::Relaxed).saturating_sub(b);
        if over(budget()) == 0 {
            return;
        }
        let target = budget() / 4 * 3;
        g.evict(over(target), Some(keep));
        if over(target) == 0 {
            return;
        }
        let me = Arc::as_ptr(&self.inner) as usize;
        let others: Vec<Arc<dyn Evict>> = registry()
            .lock()
            .expect("registry")
            .iter()
            .filter(|(id, _)| *id != me)
            .filter_map(|(_, w)| w.upgrade())
            .collect();
        for o in others {
            let want = over(target);
            if want == 0 {
                break;
            }
            o.evict(want);
        }
    }

    /// Push `v` onto `c`'s list.
    pub(super) fn push(&self, c: [i32; 3], v: V) {
        let mut g = self.inner.lock().expect("lists");
        let k = g.push(c, v);
        self.relieve(&mut g, k);
    }

    /// What `f` makes of `c`'s list.
    pub(super) fn with<R>(&self, c: [i32; 3], f: impl FnOnce(&[V]) -> R) -> R {
        let mut g = self.inner.lock().expect("lists");
        let k = chunk_of(c, g.side);
        let tick = TICK.fetch_add(1, Ordering::Relaxed);
        let Some(ch) = g.chunks.get_mut(&k) else {
            return f(&[]);
        };
        let before = ch.held();
        ch.load();
        ch.used = tick;
        let out = f(ch
            .resident
            .as_ref()
            .and_then(|m| m.get(&c))
            .map_or(&[][..], Vec::as_slice));
        g.recount(k, before);
        self.relieve(&mut g, k);
        out
    }

    /// `c`'s list.
    pub(super) fn get(&self, c: [i32; 3]) -> Vec<V> {
        self.with(c, <[V]>::to_vec)
    }
}

impl<V: Item> Clone for Lists<V> {
    /// A copy sharing the spilled records and copying the buffered ones; no
    /// chunk of it is resident.
    fn clone(&self) -> Lists<V> {
        let g = self.inner.lock().expect("lists");
        let mut held = 0;
        let chunks = g
            .chunks
            .iter()
            .map(|(k, ch)| {
                let c = Chunk {
                    segments: ch.segments.clone(),
                    pending: ch.pending.clone(),
                    resident: None,
                    decoded: 0,
                    used: 0,
                };
                held += c.held();
                (*k, c)
            })
            .collect();
        RESIDENT.fetch_add(held, Ordering::Relaxed);
        let inner = Inner {
            side: g.side,
            chunks,
            held,
            items: g.items,
        };
        drop(g);
        Lists::of(inner)
    }
}

impl<V: Item> std::fmt::Debug for Lists<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let g = self.inner.lock().expect("lists");
        write!(
            f,
            "Lists {{ side: {}, chunks: {}, items: {} }}",
            g.side,
            g.chunks.len(),
            g.items
        )
    }
}

thread_local! {
    /// The chunk side new lists on this thread take.
    static SIDE_HERE: std::cell::Cell<i32> = const { std::cell::Cell::new(SIDE) };
}

/// The chunk side new lists on this thread take: [`SIDE`], or what a test
/// set.
pub(super) fn side_here() -> i32 {
    SIDE_HERE.with(std::cell::Cell::get)
}

/// Run `f` with new lists on this thread at chunk side `side`.
#[cfg(test)]
pub(super) fn at_side<R>(side: i32, f: impl FnOnce() -> R) -> R {
    let was = SIDE_HERE.with(|s| s.replace(side));
    let out = f();
    SIDE_HERE.with(|s| s.set(was));
    out
}

impl<V: Item> Default for Lists<V> {
    /// An empty list at this thread's side ([`side_here`]).
    fn default() -> Lists<V> {
        Lists::new(side_here())
    }
}

// ---------------------------------------------------------- the node tables

/// Where one record lies in the scratch file, or in memory when the file
/// could not be opened.
#[derive(Clone, Copy)]
struct Slot {
    at: u64,
    len: u32,
}

/// **A node table**: what a graph's expansion of a node gave, one record per
/// entry in the scratch file and one slot per entry in memory — the shape the
/// node bank keeps its variants in ([`super::bank`]). A table is read one entry
/// at a time from many threads at once (a closure's expansions run in
/// parallel), so an entry is read back by itself, at its offset, outside the
/// table's lock; a chunk of a table is never decoded whole.
pub(super) struct Table<V: Item> {
    index: Mutex<TableIndex>,
    _items: std::marker::PhantomData<fn() -> V>,
}

#[derive(Default)]
struct TableIndex {
    /// Each cell's first entry: almost every cell has one.
    first: HashMap<[i32; 3], Slot>,
    /// Each cell's later entries, in order.
    more: HashMap<[i32; 3], Vec<Slot>>,
    /// The records, when the scratch file could not be opened.
    held: Vec<u8>,
}

impl TableIndex {
    fn slots(&self, c: [i32; 3]) -> Vec<Slot> {
        let mut out: Vec<Slot> = self.first.get(&c).copied().into_iter().collect();
        if let Some(m) = self.more.get(&c) {
            out.extend_from_slice(m);
        }
        out
    }

    fn put(&mut self, c: [i32; 3], bytes: &[u8]) {
        let slot = {
            let mut sp = spill().write.lock().expect("spill");
            match sp.as_mut() {
                Some(sp) => {
                    let (at, len) = sp
                        .append(bytes)
                        .expect("the lists' scratch file takes a write");
                    Slot { at, len }
                }
                None => {
                    let at = self.held.len() as u64;
                    self.held.extend_from_slice(bytes);
                    Slot {
                        at,
                        len: u32::try_from(bytes.len()).expect("one record under 4 GiB"),
                    }
                }
            }
        };
        match self.first.entry(c) {
            std::collections::hash_map::Entry::Vacant(v) => {
                v.insert(slot);
            }
            std::collections::hash_map::Entry::Occupied(_) => {
                self.more.entry(c).or_default().push(slot);
            }
        }
    }

    /// The records at `slots`, read back: from the held bytes, or from the
    /// file at their offsets.
    fn read<V: Item>(&self, slots: &[Slot]) -> Vec<V> {
        slots.iter().map(|s| read_slot(&self.held, *s)).collect()
    }
}

fn read_slot<V: Item>(held: &[u8], s: Slot) -> V {
    let decode = |b: &[u8]| {
        let mut d = Dec { b, at: 0 };
        V::take(&mut d).expect("a node-table record reads back")
    };
    match spill().read.as_ref() {
        Some(file) if held.is_empty() => decode(
            &super::spill::read_at(file, s.at, s.len).expect("the lists' scratch file reads back"),
        ),
        _ => decode(&held[s.at as usize..s.at as usize + s.len as usize]),
    }
}

impl<V: Item> Default for Table<V> {
    fn default() -> Table<V> {
        Table {
            index: Mutex::new(TableIndex::default()),
            _items: std::marker::PhantomData,
        }
    }
}

impl<V: Item> Table<V> {
    /// What `f` makes of `c`'s entries, in the order they were kept.
    pub(super) fn with<R>(&self, c: [i32; 3], f: impl FnOnce(&[V]) -> R) -> R {
        let slots = {
            let ix = self.index.lock().expect("table");
            let slots = ix.slots(c);
            if !ix.held.is_empty() {
                // No file: the records are the index's own bytes.
                let items: Vec<V> = ix.read(&slots);
                drop(ix);
                return f(&items);
            }
            slots
        };
        // Read outside the lock, each record at its offset.
        let items: Vec<V> = slots.iter().map(|s| read_slot(&[], *s)).collect();
        f(&items)
    }

    /// The first of `c`'s entries `matches` holds for, or `v` kept as its next
    /// one when none does — one step under the table's lock, so two threads
    /// keeping one entry at once keep it once.
    pub(super) fn find_or_push(&self, c: [i32; 3], matches: impl Fn(&V) -> bool, v: V) -> V {
        let mut ix = self.index.lock().expect("table");
        let slots = ix.slots(c);
        if let Some(h) = ix.read::<V>(&slots).into_iter().find(|x| matches(x)) {
            return h;
        }
        let mut e = Enc::default();
        v.put(&mut e);
        ix.put(c, &e.0);
        v
    }

    /// Every cell's entries, in cell order.
    #[cfg(test)]
    pub(super) fn entries(&self) -> Vec<([i32; 3], Vec<V>)> {
        let ix = self.index.lock().expect("table");
        let mut cells: Vec<[i32; 3]> = ix.first.keys().copied().collect();
        cells.sort_unstable();
        cells
            .into_iter()
            .map(|c| (c, ix.read(&ix.slots(c))))
            .collect()
    }
}

// ------------------------------------------------------------- the records

impl Item for super::Edge {
    fn put(&self, e: &mut Enc) {
        e.edge(self);
    }
    fn take(d: &mut Dec<'_>) -> Option<Self> {
        d.edge()
    }
}

impl Item for ([i32; 3], super::Edge) {
    fn put(&self, e: &mut Enc) {
        e.cell(self.0);
        e.edge(&self.1);
    }
    fn take(d: &mut Dec<'_>) -> Option<Self> {
        Some((d.cell()?, d.edge()?))
    }
}

impl Item for (super::Quadrants, super::NodeEdges) {
    fn put(&self, e: &mut Enc) {
        e.u16(self.0);
        e.node_edges(&self.1);
    }
    fn take(d: &mut Dec<'_>) -> Option<Self> {
        Some((d.u16()?, d.node_edges()?))
    }
    fn heap(&self) -> usize {
        self.1.edges.capacity() * std::mem::size_of::<super::Edge>()
    }
}
