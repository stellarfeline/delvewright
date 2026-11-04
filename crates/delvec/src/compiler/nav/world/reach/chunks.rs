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
//! counts against the one resident budget ([`super::budget`]); past it, the
//! least recently used chunks give their decoded maps up, and the records not
//! yet written go to the file first. Records pushed to a chunk that is not
//! resident are buffered and written once they pass [`PENDING`] bytes, so a
//! closure being built never decodes the chunks it only adds to.
//!
//! The lists' stated need is the working set a walk by chunk needs
//! ([`working_set`]): a closure is taken, and a proof walks it, one chunk at
//! a time, writing what crosses into the eight chunks round it; so the need
//! holds the 3 × 3 ring of chunks as large as the largest one any list has
//! been decoded to, and the ring's buffered pushes in the two lists a closure
//! writes. It is measured from the lists, never set: a map's largest chunk
//! depends on how densely its columns are reached, never on how many columns
//! it has.
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

use super::budget;
use super::persist::{Dec, Enc};
use super::spill::{Extent, Reader, Spill, Usage};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};

/// A reach chunk's side, in columns: a multiple of the game's 16 and at least
/// the window margin, so a node's launches land in its chunk or the eight
/// round it.
pub(super) const SIDE: i32 = 32;

/// Records not yet written are written once a chunk buffers this many bytes.
const PENDING: usize = 64 << 10;

/// The chunks a walk by chunk holds at once: the one in hand and the eight
/// round it, which what crosses its faces is written to.
const RING: usize = 9;

/// The largest any chunk of any list has been decoded to, in bytes.
static LARGEST: AtomicUsize = AtomicUsize::new(0);

/// The largest any chunk of any list has been decoded to so far, in bytes.
pub(super) fn largest() -> usize {
    LARGEST.load(Ordering::Relaxed)
}

/// **The working set a walk by chunk needs**, in bytes: [`RING`] chunks as
/// large as the largest decoded so far, and [`RING`] chunks' buffered pushes
/// in each of the two lists a closure writes (`succ` and `preds`).
pub(super) fn working_set() -> usize {
    RING * largest() + RING * 2 * PENDING
}

/// The lists' stated need: their working set ([`working_set`]).
static NEED: budget::Need = budget::Need::new("the closures' edge lists", RING * 2 * PENDING);

/// The lists' need before anything is decoded: the ring's buffered pushes.
pub(super) fn floor() -> usize {
    RING * 2 * PENDING
}

/// The most a chunk of `states` nodes with `edges` edges each decodes to:
/// each node an entry and its edges as items ([`ENTRY`], the edge's size).
pub(super) fn chunk_bound(states: usize, edges: usize) -> usize {
    decoded_bound(states, states.saturating_mul(edges))
}

/// The most a chunk of `states` nodes holding `items` edges in all decodes to:
/// each node an entry ([`ENTRY`]) and each edge an item (the edge's size).
pub(super) fn decoded_bound(states: usize, items: usize) -> usize {
    states
        .saturating_mul(ENTRY)
        .saturating_add(items.saturating_mul(std::mem::size_of::<super::Edge>()))
}

/// Note a chunk decoded to `bytes`: the largest, and the need, grow with it.
fn decoded_to(bytes: usize) {
    if LARGEST.fetch_max(bytes, Ordering::Relaxed) < bytes {
        NEED.raise(working_set());
    }
}

/// The one scratch file every list writes to, behind the writer's lock, and
/// a reader every reader reads it by at an offset, unlocked; both `None` when
/// it could not be opened, and then every list keeps its records in memory.
struct Shared {
    write: Mutex<Option<Spill>>,
    read: Option<Reader>,
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

/// What the lists' and tables' scratch file has held ([`Usage`]).
pub(super) fn spilled() -> Usage {
    spill()
        .write
        .lock()
        .expect("spill")
        .as_ref()
        .map_or(Usage::default(), Spill::usage)
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
    /// The edges this closure joined whose two ends lie in different chunks.
    portals: u64,
    /// The cells those edges land on.
    faces: std::collections::HashSet<[i32; 3]>,
}

impl Visits {
    pub(super) fn visit(&mut self, k: Key) {
        *self.counts.entry(k).or_insert(0) += 1;
    }

    /// Note an edge joined from `from` to `to`: a **portal edge** when the
    /// two lie in different chunks of side `side` (ADR-0037 §2).
    pub(super) fn joined(&mut self, from: [i32; 3], to: [i32; 3], side: i32) {
        if chunk_of(from, side) != chunk_of(to, side) {
            self.portals += 1;
            self.faces.insert(to);
        }
    }

    /// Fold this closure's visits and portal edges into the run's figures;
    /// its portal edges and the face cells they land on.
    pub(super) fn report(&mut self) -> [u64; 2] {
        for n in std::mem::take(&mut self.counts).into_values() {
            let b = (n as usize).min(VISIT_BUCKETS);
            VISIT_HIST[b].fetch_add(1, Ordering::Relaxed);
            VISIT_MAX.fetch_max(n, Ordering::Relaxed);
        }
        let out = [
            std::mem::take(&mut self.portals),
            std::mem::take(&mut self.faces).len() as u64,
        ];
        PORTALS.fetch_add(out[0], Ordering::Relaxed);
        FACES.fetch_add(out[1], Ordering::Relaxed);
        PORTALS_MAX.fetch_max(out[0], Ordering::Relaxed);
        out
    }
}

static PORTALS: AtomicU64 = AtomicU64::new(0);
static FACES: AtomicU64 = AtomicU64::new(0);
/// The most portal edges one closure joined: the figure the cost's bound is
/// read against (`compiler::cost`), which is per closure.
static PORTALS_MAX: AtomicU64 = AtomicU64::new(0);

/// The run's portal edges and the face cells they land on, summed over the
/// closures taken, and the most portal edges one closure joined: what crosses
/// a chunk border (ADR-0037 §2).
pub(super) fn portals() -> (u64, u64, u64) {
    (
        PORTALS.load(Ordering::Relaxed),
        FACES.load(Ordering::Relaxed),
        PORTALS_MAX.load(Ordering::Relaxed),
    )
}

/// How many (closure, chunk) pairs the run's closures visited.
pub(super) fn visited() -> u64 {
    VISIT_HIST.iter().map(|b| b.load(Ordering::Relaxed)).sum()
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
    /// The records written to the file, in order: shared with every copy of
    /// the list, and free in the file once none holds them.
    segments: Vec<Arc<Extent>>,
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
            for seg in &self.segments {
                let (at, len) = seg.at();
                take(&file.read(at, len));
            }
        }
        take(&self.pending);
        self.resident = Some(map);
        self.decoded = decoded;
        decoded_to(decoded);
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
            let r = sp.put(&self.pending);
            self.segments.push(Arc::new(sp.or_fail("take a write", r)));
            self.pending = Vec::new();
        }
    }

    /// Give up what the chunk holds in memory that the file can hold: a chunk
    /// written in more than [`PIECES`] pieces is written whole, cell by cell
    /// in cell order and each cell's items in their order (never in the
    /// resident map's order, which is a hash's), and read back from that one
    /// piece.
    fn release(&mut self) {
        let pieces = self.segments.len() + usize::from(!self.pending.is_empty());
        if let Some(map) = self.resident.as_ref().filter(|_| pieces > PIECES) {
            let mut cells: Vec<&[i32; 3]> = map.keys().collect();
            cells.sort_unstable();
            let mut e = Enc::default();
            for c in cells {
                for v in &map[c] {
                    e.cell(*c);
                    v.put(&mut e);
                }
            }
            let mut sp = spill().write.lock().expect("spill");
            if let Some(sp) = sp.as_mut() {
                let r = sp.put(&e.0);
                let whole = Arc::new(sp.or_fail("take a write", r));
                // The pieces it replaces are free once no copy holds them.
                drop(std::mem::replace(&mut self.segments, vec![whole]));
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
    /// Bytes this list counts as held ([`budget::hold`]).
    held: usize,
    items: usize,
}

impl<V> Drop for Inner<V> {
    fn drop(&mut self) {
        budget::release(self.held);
    }
}

impl<V: Item> Inner<V> {
    /// Push `v` onto `c`'s list; the chunk it went to.
    fn push(&mut self, c: [i32; 3], v: V) -> Key {
        let k = chunk_of(c, self.side);
        let tick = budget::tick();
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
            decoded_to(ch.decoded);
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
            budget::hold(after - before);
            self.held += after - before;
        } else {
            budget::release(before - after);
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

impl<V: Item> budget::Pageable for Mutex<Inner<V>> {
    fn give_up(&self, want: usize) -> usize {
        match self.try_lock() {
            Ok(mut g) => g.evict(want, None),
            Err(_) => 0,
        }
    }
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
        let weak: Weak<dyn budget::Pageable> = {
            let w: Weak<Mutex<Inner<V>>> = Arc::downgrade(&inner);
            w
        };
        budget::join(budget::id_of(&*inner), weak);
        Lists { inner }
    }

    /// Bring the resident bytes under the budget, this list first, keeping
    /// chunk `keep` ([`budget::relieve`]).
    fn relieve(&self, g: &mut Inner<V>, keep: Key) {
        let held = g.held;
        budget::relieve(budget::id_of(&*self.inner), held, NEED.bytes(), |want| {
            g.evict(want, Some(keep))
        });
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
        let tick = budget::tick();
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

    /// The chunk side the list is kept at.
    pub(super) fn side(&self) -> i32 {
        self.inner.lock().expect("lists").side
    }

    /// Every record of every chunk written to the file now.
    #[cfg(test)]
    fn segments(&self) -> Vec<Weak<Extent>> {
        let g = self.inner.lock().expect("lists");
        g.chunks
            .values()
            .flat_map(|ch| ch.segments.iter().map(Arc::downgrade))
            .collect()
    }

    /// How many pieces `c`'s chunk is written in, buffered records counted
    /// as one.
    #[cfg(test)]
    fn pieces_of(&self, c: [i32; 3]) -> usize {
        let g = self.inner.lock().expect("lists");
        g.chunks.get(&chunk_of(c, g.side)).map_or(0, |ch| {
            ch.segments.len() + usize::from(!ch.pending.is_empty())
        })
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
        budget::hold(held);
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
    /// Each cell's first entry, kept by reach chunk of [`TABLE_SIDE`]: the
    /// cell packed within its chunk ([`local`]) and the slot packed in one
    /// word ([`pack`]) — almost every cell has one.
    first: HashMap<Key, HashMap<u64, u64>>,
    /// A first entry whose slot does not pack, and each cell's later entries,
    /// in order.
    more: HashMap<[i32; 3], Vec<Slot>>,
    /// The records, when the scratch file could not be opened.
    held: Vec<u8>,
}

/// The chunk side a node table keys its entries by: what it answers never
/// depends on it.
const TABLE_SIDE: i32 = 32;

/// A cell as a word within its chunk of [`TABLE_SIDE`]: its `x` and `z`
/// offsets in the chunk and its whole `y`.
fn local(c: [i32; 3]) -> u64 {
    let lx = c[0].rem_euclid(TABLE_SIDE) as u64;
    let lz = c[2].rem_euclid(TABLE_SIDE) as u64;
    (lx << 37) | (lz << 32) | u64::from(c[1] as u32)
}

/// A slot as one word, offset in the low 40 bits and length above; `None`
/// past a terabyte of file or a record of 16 MiB.
fn pack(s: Slot) -> Option<u64> {
    (s.at < 1 << 40 && s.len < 1 << 24).then(|| s.at | (u64::from(s.len) << 40))
}

fn unpack(w: u64) -> Slot {
    Slot {
        at: w & ((1 << 40) - 1),
        len: (w >> 40) as u32,
    }
}

impl Drop for TableIndex {
    /// A table dropped gives its records' room in the file back: nothing
    /// reads them again, every read going through the table.
    fn drop(&mut self) {
        let Some(file) = spill().read.as_ref().filter(|_| self.held.is_empty()) else {
            return;
        };
        for m in self.first.values() {
            for w in m.values() {
                let s = unpack(*w);
                file.give_back(s.at, s.len);
            }
        }
        for slots in self.more.values() {
            for s in slots {
                file.give_back(s.at, s.len);
            }
        }
    }
}

impl TableIndex {
    fn first_of(&self, c: [i32; 3]) -> Option<Slot> {
        self.first
            .get(&chunk_of(c, TABLE_SIDE))
            .and_then(|m| m.get(&local(c)))
            .map(|w| unpack(*w))
    }

    fn slots(&self, c: [i32; 3]) -> Vec<Slot> {
        let mut out: Vec<Slot> = self.first_of(c).into_iter().collect();
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
                    let r = sp.append(bytes);
                    let (at, len) = sp.or_fail("take a write", r);
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
        // A cell's entries are its first, then `more` in order; a first
        // entry that does not pack opens `more` instead, and is first there.
        let fresh = self.first_of(c).is_none() && !self.more.contains_key(&c);
        match pack(slot).filter(|_| fresh) {
            Some(w) => {
                self.first
                    .entry(chunk_of(c, TABLE_SIDE))
                    .or_default()
                    .insert(local(c), w);
            }
            None => self.more.entry(c).or_default().push(slot),
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
        Some(file) if held.is_empty() => decode(&file.read(s.at, s.len)),
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
        let mut cells: Vec<[i32; 3]> = ix
            .first
            .iter()
            .flat_map(|((kx, kz), m)| {
                m.keys().map(move |w| {
                    [
                        kx * TABLE_SIDE + ((w >> 37) & 31) as i32,
                        *w as u32 as i32,
                        kz * TABLE_SIDE + ((w >> 32) & 31) as i32,
                    ]
                })
            })
            .chain(ix.more.keys().copied())
            .collect();
        cells.sort_unstable();
        cells.dedup();
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

#[cfg(test)]
mod table_tests {
    use super::{Dec, Enc, Extent, Item, Lists, PENDING, PIECES, Table, Weak};
    use std::collections::BTreeMap;

    #[derive(Clone, Debug, PartialEq)]
    struct Rec([i32; 3]);

    impl Item for Rec {
        fn put(&self, e: &mut Enc) {
            e.cell(self.0);
        }
        fn take(d: &mut Dec<'_>) -> Option<Self> {
            Some(Rec(d.cell()?))
        }
    }

    /// A list beside the map of the same pushes it is held to.
    type Pair = (Lists<Fat>, BTreeMap<[i32; 3], Vec<Fat>>);

    /// A record with a payload, so a chunk's buffered bytes pass [`PENDING`].
    #[derive(Clone, Debug, PartialEq)]
    struct Fat(u16, Vec<i32>);

    impl Item for Fat {
        fn put(&self, e: &mut Enc) {
            e.u16(self.0);
            e.u16(self.1.len() as u16);
            for v in &self.1 {
                e.cell([*v, 0, 0]);
            }
        }
        fn take(d: &mut Dec<'_>) -> Option<Self> {
            let tag = d.u16()?;
            let n = d.u16()?;
            let mut v = Vec::with_capacity(n as usize);
            for _ in 0..n {
                v.push(d.cell()?[0]);
            }
            Some(Fat(tag, v))
        }
        fn heap(&self) -> usize {
            self.1.capacity() * 4
        }
    }

    /// **A list kept by chunk answers what a map of its pushes answers**
    /// (ADR-0037 §2): records pushed in a generated order at cells on both
    /// sides of zero and across two-column chunk faces, several per cell and
    /// more than [`PENDING`] bytes per chunk, read back between pushes so
    /// every chunk is given up and written out more than [`PIECES`] times —
    /// taking the rewrite branch of `Chunk::release` — with a copy taken
    /// mid-stream and pushed to on both sides; under the default budget and
    /// under a budget of one byte. Every present cell and a cell beside each
    /// answer exactly as a `BTreeMap<[i32; 3], Vec<_>>` of the same pushes.
    /// Red on any difference, and on a run whose chunks never passed
    /// [`PIECES`] pieces or were never given up (which would bind nothing).
    #[test]
    fn a_list_kept_by_chunk_answers_what_a_map_answers() {
        for cut in [false, true] {
            let run = || {
                let mut seed = 0x9e37_79b9_7f4a_7c15u64;
                let mut next = |n: u64| {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    seed % n
                };
                let l: Lists<Fat> = Lists::new(2);
                let mut m: BTreeMap<[i32; 3], Vec<Fat>> = BTreeMap::new();
                let mut copy: Option<Pair> = None;
                let (mut most_pieces, releases0) = (0usize, super::traffic().2);
                let cells: Vec<[i32; 3]> = (0..24)
                    .map(|i| [i % 5 - 2, (i * 7) % 4 - 1, (i * 3) % 5 - 2])
                    .collect();
                for step in 0..6000u32 {
                    let c = cells[next(cells.len() as u64) as usize];
                    let v = Fat(
                        next(1000) as u16,
                        (0..next(40)).map(|k| k as i32 - 20).collect(),
                    );
                    l.push(c, v.clone());
                    m.entry(c).or_default().push(v.clone());
                    if let Some((cl, cm)) = copy.as_mut() {
                        // The copy takes a different stream.
                        if next(2) == 0 {
                            let v2 = Fat(v.0 ^ 1, v.1.clone());
                            cl.push(c, v2.clone());
                            cm.entry(c).or_default().push(v2);
                        }
                    }
                    if step == 2500 {
                        copy = Some((l.clone(), m.clone()));
                    }
                    // Read a cell back now and then, so chunks are loaded,
                    // given up and written out piece by piece.
                    if next(5) == 0 {
                        let q = cells[next(cells.len() as u64) as usize];
                        most_pieces = most_pieces.max(l.pieces_of(q));
                        assert_eq!(&l.get(q), m.get(&q).map_or(&[][..], Vec::as_slice), "{q:?}");
                        if let Some((cl, cm)) = copy.as_ref() {
                            assert_eq!(&cl.get(q), cm.get(&q).map_or(&[][..], Vec::as_slice));
                        }
                    }
                }
                let check = |l: &Lists<Fat>, m: &BTreeMap<[i32; 3], Vec<Fat>>| {
                    for (c, vs) in m {
                        assert_eq!(&l.get(*c), vs, "{c:?}");
                        let beside = [c[0] + 100, c[1], c[2]];
                        assert!(l.get(beside).is_empty());
                    }
                    let (first, items) = m.first_key_value().expect("a push was made");
                    l.with(*first, |x| assert_eq!(x, items.as_slice()));
                };
                check(&l, &m);
                let (cl, cm) = copy.expect("a copy was taken");
                check(&cl, &cm);
                let bytes: usize = m.values().flatten().map(|f| 4 + 12 * f.1.len()).sum();
                (
                    most_pieces,
                    super::traffic().2 - releases0,
                    bytes
                        / m.keys()
                            .map(|c| super::chunk_of(*c, 2))
                            .collect::<std::collections::BTreeSet<_>>()
                            .len(),
                    m.values().map(Vec::len).sum::<usize>(),
                )
            };
            let (pieces, releases, per_chunk, records) = if cut {
                super::budget::with_budget(1, run)
            } else {
                run()
            };
            eprintln!(
                "lists (budget {}): {records} record(s), about {per_chunk} byte(s) per chunk, at most \
                 {pieces} piece(s) before a read, {releases} chunk(s) given up",
                if cut { "one byte" } else { "default" }
            );
            assert!(
                per_chunk > PENDING,
                "a chunk never passed its buffer: the case binds nothing"
            );
            if cut {
                assert!(
                    pieces > PIECES,
                    "no chunk passed {PIECES} pieces: the rewrite branch was never taken"
                );
                assert!(releases > 0, "no chunk was given up");
            }
        }
    }

    /// **A list's file holds what the list holds, not what it wrote**
    /// (ADR-0037 §2): pushes to a few cells under a budget of one byte, read
    /// back between pushes, so chunks are given up, written out piece by
    /// piece and rewritten whole past [`PIECES`] pieces; a copy taken
    /// mid-stream shares the pieces written so far. A piece the list no
    /// longer holds is free in the file once no copy holds it, and every
    /// piece is free once the list and its copy are gone. Red on a piece
    /// kept after its chunk was rewritten (the file would grow with every
    /// rewrite), and on a run that rewrote nothing (which would bind
    /// nothing).
    #[test]
    fn a_lists_file_holds_what_the_list_holds_not_what_it_wrote() {
        super::budget::with_budget(1, || {
            let l: Lists<Fat> = Lists::new(2);
            let cells = [[0, 0, 0], [4, 1, 0], [8, 0, 1], [1, 2, 4]];
            let mut seen: Vec<Weak<Extent>> = Vec::new();
            let mut copy = None;
            for step in 0..3000u32 {
                let c = cells[step as usize % cells.len()];
                l.push(c, Fat(step as u16, (0..30).collect()));
                if step % 7 == 0 {
                    l.get(cells[(step as usize / 7) % cells.len()]);
                    seen.extend(l.segments());
                }
                if step == 1500 {
                    copy = Some(l.clone());
                }
            }
            seen.extend(l.segments());
            let alive = |w: &[Weak<Extent>]| w.iter().filter(|w| w.strong_count() > 0).count();
            let ptrs = |ws: Vec<Weak<Extent>>| -> std::collections::BTreeSet<usize> {
                ws.iter().map(|w| w.as_ptr() as usize).collect()
            };
            let held: std::collections::BTreeSet<usize> = ptrs(l.segments())
                .union(&ptrs(copy.as_ref().expect("a copy").segments()))
                .copied()
                .collect();
            let distinct = ptrs(seen.clone());
            let stray = seen
                .iter()
                .filter(|w| w.strong_count() > 0 && !held.contains(&(w.as_ptr() as usize)))
                .map(|w| w.as_ptr() as usize)
                .collect::<std::collections::BTreeSet<_>>()
                .len();
            eprintln!(
                "lists' file: {} distinct piece(s) written over the run, {} held by the list \
                 and its copy, {stray} held by neither",
                distinct.len(),
                held.len()
            );
            assert!(
                distinct.len() > held.len() + cells.len(),
                "no chunk was rewritten: the case binds nothing"
            );
            assert_eq!(stray, 0, "a piece nothing holds is still kept in the file");
            drop(l);
            drop(copy);
            assert_eq!(alive(&seen), 0, "a piece outlived its list");
        });
    }

    /// **A node table keyed by chunk answers what a map of its entries
    /// answers** (ADR-0037 §2): entries kept in a generated order at cells on
    /// both sides of zero, chunk faces and the `y` extremes included, some
    /// cells kept several times and some refused as already held, read back
    /// per cell in the order kept and listed whole in cell order exactly as a
    /// `BTreeMap<[i32; 3], Vec<_>>` of the same keeps. Red on any difference.
    #[test]
    fn a_table_keyed_by_chunk_answers_what_a_map_answers() {
        let mut seed = 0x6a09_e667_f3bc_c909u64;
        let mut next = |n: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % n
        };
        let t: Table<Rec> = Table::default();
        let mut m: BTreeMap<[i32; 3], Vec<Rec>> = BTreeMap::new();
        let ys = [i32::MIN, -65, -1, 0, 63, 319, i32::MAX];
        let (mut kept, mut refused) = (0usize, 0usize);
        for _ in 0..4000 {
            let faces = [-33, -32, -31, -1, 0, 1, 31, 32, 33];
            let mut h = || {
                if next(2) == 0 {
                    faces[next(9) as usize]
                } else {
                    next(12) as i32 - 6
                }
            };
            let (x, z) = (h(), h());
            let y = if next(4) == 0 {
                ys[next(7) as usize]
            } else {
                next(6) as i32 - 3
            };
            let c = [x, y, z];
            let v = Rec([next(3) as i32, 0, 0]);
            let have = m.entry(c).or_default();
            let got = t.find_or_push(c, |x| *x == v, v.clone());
            assert_eq!(got, v);
            if have.contains(&v) {
                refused += 1;
            } else {
                have.push(v);
                kept += 1;
            }
        }
        for (c, vs) in &m {
            assert_eq!(&t.with(*c, |l| l.to_vec()), vs, "{c:?}");
        }
        assert_eq!(t.with([1000, 0, 1000], |l| l.len()), 0);
        assert_eq!(
            t.entries(),
            m.iter().map(|(c, v)| (*c, v.clone())).collect::<Vec<_>>()
        );
        let several = m.values().filter(|v| v.len() > 1).count();
        eprintln!(
            "table: {kept} entr(ies) kept over {} cell(s), {several} with several, {refused} refused as held",
            m.len()
        );
        assert!(
            kept > 0 && refused > 0 && several > 0,
            "the cases bind nothing"
        );
    }
}
