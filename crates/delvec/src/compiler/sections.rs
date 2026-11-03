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
//! coordinates and palettes past the narrow index, under the default budget
//! and under a budget of one byte.
//!
//! **Paged under the one budget.** A section's index array is a [`Paged`]
//! holding: resident, it counts against the one resident budget every paged
//! structure of the compiler shares ([`budget`]); past the budget the least
//! recently used arrays over every store in the process are written to the
//! one scratch file of the stores ([`spill`], the same policy and place as the
//! reach graph's) and read back when next asked. The palette, the cell count
//! and the section index stay resident: a section's cost in memory while it
//! is given up is its palette. What a store answers never depends on what is
//! resident. **The stated need** is measured from what the walks have had to
//! hold: an array read back within one turnover of the resident arrays —
//! fewer arrays, over every store, having come resident since it was given up
//! (made, or read back: [`arrivals`]) than the stores hold resident or the
//! need states, whichever is more — and given up under the need now stated,
//! was given up while the walk was still using it, and each time the stores
//! are brought under the budget, a sample of read-backs ([`SAMPLE`]) of which
//! half or more were such re-references doubles the need, and the budget with
//! it, until the walk's working set stays. A walk that reads back an array
//! given up long before — a sparse write into a spilled world at assembly, a
//! second sequential pass over it, a store another stage's growth pushed out
//! and a later stage reads again — raises nothing, however few cells it
//! reads; nor does one given up under an earlier, smaller need, which says
//! nothing about the need now in force. The clock is the arrivals over every
//! store, since the stores share one least-recently-used order: a store's
//! own give-ups stop while another store grows, and a clock of those would
//! read every later reload of it as thrashing. The floor is a 3 × 3 × 3 ring
//! of sections at the wide index.

use crate::compiler::nav::{budget, spill};
use std::collections::{BTreeMap, btree_map};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};

/// A cell: `[x, y, z]`.
type Cell = [i32; 3];

/// A section's side, in cells.
pub const SIDE: i32 = 16;
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
    /// This store's paging clock and its read-backs, thrashing or cold.
    tally: Arc<Tally>,
    /// The number of cells held, over every section.
    len: usize,
}

/// One section: a palette and one index per cell into it (`0`: absent;
/// `i`: `palette[i - 1]`), the index array a paged holding.
struct Section<V> {
    palette: Vec<V>,
    idx: Arc<Paged>,
    /// Cells held: indices that are not `0`.
    count: u32,
}

impl<V: Clone> Section<V> {
    /// A copy of this section sharing its index array: a copy of a store costs
    /// its palettes and its section index, and an array is copied when one of
    /// the two writes it ([`Section::own`]).
    fn share(&self) -> Self {
        Section {
            palette: self.palette.clone(),
            idx: Arc::clone(&self.idx),
            count: self.count,
        }
    }

    /// Make this section's index array its own before a write: an array a
    /// copy shares is duplicated for a store whose clock is `tally` — its own
    /// array when resident, the same record when given up — so neither store
    /// sees the other's writes.
    fn own(&mut self, tally: &Arc<Tally>) {
        if Arc::strong_count(&self.idx) > 1 {
            self.idx = self.idx.duplicate(tally);
        }
    }
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

    /// The array as a record: its bytes, little-endian for the wide form,
    /// so the record's length says which form it is.
    fn encode(&self) -> Vec<u8> {
        match self {
            Idx::Narrow(a) => a.to_vec(),
            Idx::Wide(a) => a.iter().flat_map(|v| v.to_le_bytes()).collect(),
        }
    }

    /// The array a record was written as.
    fn decode(b: &[u8]) -> Idx {
        match b.len() {
            CELLS => {
                let mut a = Box::new([0u8; CELLS]);
                a.copy_from_slice(b);
                Idx::Narrow(a)
            }
            n if n == 2 * CELLS => {
                let mut a = Box::new([0u16; CELLS]);
                for (to, from) in a.iter_mut().zip(b.chunks_exact(2)) {
                    *to = u16::from_le_bytes([from[0], from[1]]);
                }
                Idx::Wide(a)
            }
            n => panic!(
                "a section's record is {n} byte(s), neither {CELLS} nor {}",
                2 * CELLS
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Paging: the index arrays under the one budget, in the one scratch file.

/// **A section's index array as a paged holding**: resident, or given up to
/// the stores' scratch file and read back when next asked. Every access goes
/// through its lock, so a store's owner reading one cell and the budget
/// giving the array up on another thread never see half an array.
struct Paged {
    slot: Mutex<Slot>,
    /// The use clock ([`budget::tick`]) when the array was last asked for:
    /// every use advances it, so the arrays given up first are the least
    /// recently used.
    used: AtomicU64,
    /// The clock of the store this array belongs to.
    tally: Arc<Tally>,
}

enum Slot {
    Resident(Arc<Idx>),
    /// Given up: where the record lies and its length (which says its form),
    /// and when it was given up — the arrivals clock ([`arrivals`]), the
    /// need's generation ([`generation`]) and whether a test's budget was in
    /// force, so the read-back is set against the same clock — `None` for a
    /// record a copy shares and never held, whose first read is cold.
    Spilled {
        at: u64,
        len: u32,
        since: Option<(u64, u64, bool)>,
    },
}

/// **One store's paging tally** (the module docs): what it holds resident,
/// and of its arrays read back, how many came back within one turnover of
/// the resident arrays (thrashing) and how many did not (cold). Each store
/// keeps its own, so what one walk's read-backs say is not mixed with another
/// store's traffic; the need is judged over every store's. A test's budget
/// (`budget::with_budget`) is kept over the one store relieving itself under
/// it, never over another thread's.
#[derive(Default)]
struct Tally {
    /// Bytes of this store's arrays resident now.
    held: AtomicUsize,
    read_back: AtomicU64,
    thrashed: AtomicU64,
}

impl Tally {
    /// Read-backs so far and how many of them were thrashing.
    #[cfg(test)]
    fn read_back(&self) -> (u64, u64) {
        (
            self.read_back.load(Ordering::Relaxed),
            self.thrashed.load(Ordering::Relaxed),
        )
    }
}

/// A 3 × 3 × 3 ring of sections at the wide index: the fewest bytes the
/// stores' need states.
const FLOOR: usize = 27 * 2 * CELLS;

/// The stores' stated need: measured, as the module docs say.
static NEED: budget::Need = budget::Need::new("the world store's sections", FLOOR);

/// The stores' need before any walk is measured: the ring.
pub fn floor() -> usize {
    FLOOR
}

/// Bytes of index arrays resident now, over every store in the process.
static HELD: AtomicUsize = AtomicUsize::new(0);
/// Sections whose index array is resident now, over every store.
static RESIDENT: AtomicUsize = AtomicUsize::new(0);
/// Holds so far (an array made, or read back).
static EVENTS: AtomicU64 = AtomicU64::new(0);
/// **The arrivals clock** (the module docs): arrays come resident, made or
/// read back, over every store — what a read-back's distance from its
/// give-up is counted in. Under a test's budget, the thread's own.
static ARRIVALS: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static ARRIVALS_HERE: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// The arrivals clock in force on this thread: the process's, or under a
/// test's budget the thread's own, as the budget binds each store alone.
fn arrivals() -> u64 {
    match budget::override_here() {
        0 => ARRIVALS.load(Ordering::Relaxed),
        _ => ARRIVALS_HERE.with(std::cell::Cell::get),
    }
}

thread_local! {
    static RAISED_HERE: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// The need's generation in force on this thread: the count of the need's
/// raises ([`RAISED`]), or under a test's budget the thread's own, which no
/// other thread's raise moves — as the arrivals clock is.
fn generation() -> u64 {
    match budget::override_here() {
        0 => RAISED.load(Ordering::Relaxed),
        _ => RAISED_HERE.with(std::cell::Cell::get),
    }
}

/// Advance the arrivals clock in force on this thread.
fn arrive() {
    match budget::override_here() {
        0 => {
            ARRIVALS.fetch_add(1, Ordering::Relaxed);
        }
        _ => ARRIVALS_HERE.with(|a| a.set(a.get() + 1)),
    }
}
/// Arrays given up and read back over the run, how many of the read-backs
/// were thrashing (the module docs), and how often the need was raised.
static GIVEN_UP: AtomicU64 = AtomicU64::new(0);
static READ_BACK: AtomicU64 = AtomicU64::new(0);
static THRASHED: AtomicU64 = AtomicU64::new(0);
static RAISED: AtomicU64 = AtomicU64::new(0);
/// The read-backs the need is judged over — made under no test budget — and
/// the thrashing ones among them, and both as they stood at the last
/// judgement.
static JUDGED_READ_BACK: AtomicU64 = AtomicU64::new(0);
static JUDGED_THRASHED: AtomicU64 = AtomicU64::new(0);
static READ_BACK_JUDGED: AtomicU64 = AtomicU64::new(0);
static THRASHED_JUDGED: AtomicU64 = AtomicU64::new(0);
/// Read-backs between two judgements of the need.
const SAMPLE: u64 = 64;

/// The one structure the budget asks: every store's paged arrays, weakly.
struct Store {
    pages: Mutex<Vec<Weak<Paged>>>,
}

fn store() -> &'static Arc<Store> {
    static S: OnceLock<Arc<Store>> = OnceLock::new();
    S.get_or_init(|| {
        let s = Arc::new(Store {
            pages: Mutex::new(Vec::new()),
        });
        let weak: Weak<dyn budget::Pageable> = {
            let w: Weak<Store> = Arc::downgrade(&s);
            w
        };
        budget::join(budget::id_of(&*s), weak);
        // The need stands at its floor from the first section, so the budget
        // holds a ring before any walk is measured.
        NEED.raise(0);
        s
    })
}

/// The one scratch file every store writes to, behind the writer's lock, and
/// a reader every read-back reads it by, unlocked; both `None` when it could
/// not be opened, and then every array stays resident.
struct Shared {
    write: Mutex<Option<spill::Spill>>,
    read: Option<spill::Reader>,
}

fn shared() -> &'static Shared {
    static SPILL: OnceLock<Shared> = OnceLock::new();
    SPILL.get_or_init(|| {
        let opened = spill::Spill::open("sections").ok();
        let read = opened.as_ref().and_then(|s| s.reader().ok());
        Shared {
            write: Mutex::new(opened.filter(|_| read.is_some())),
            read,
        }
    })
}

impl Store {
    /// Give up the least recently used resident arrays, over every store (or
    /// over the one store `only` names), until `want` bytes are freed or none
    /// is left to give; the bytes freed. An array in use is skipped, never
    /// waited for; the list of arrays is waited for only on the stores' own
    /// relief (`wait`), never when another structure asks.
    fn evict(&self, want: usize, wait: bool, only: Option<&Arc<Tally>>) -> usize {
        let pages = if wait {
            self.pages.lock().ok()
        } else {
            self.pages.try_lock().ok()
        };
        let Some(mut pages) = pages else {
            return 0;
        };
        pages.retain(|w| w.strong_count() > 0);
        let mut order: Vec<(u64, Arc<Paged>)> = pages
            .iter()
            .filter_map(Weak::upgrade)
            .filter(|p| only.is_none_or(|t| Arc::ptr_eq(&p.tally, t)))
            .map(|p| (p.used.load(Ordering::Relaxed), p))
            .collect();
        order.sort_unstable_by_key(|(used, _)| *used);
        if wait {
            Store::judge();
        }
        let mut freed = 0;
        for (_, p) in order {
            if freed >= want {
                break;
            }
            freed += p.give_up();
        }
        freed
    }

    /// **Measure the need** (the module docs): over the read-backs since the
    /// last judgement, half or more thrashing — read back within one turnover
    /// of the resident arrays ([`thrashing`]) — says the walk was given up
    /// arrays it was still using, and the need doubles. Judged once per
    /// [`SAMPLE`] read-backs, never under a test's budget.
    fn judge() {
        if budget::override_here() != 0 {
            return;
        }
        let read = JUDGED_READ_BACK.load(Ordering::Relaxed);
        let read_since = read.saturating_sub(READ_BACK_JUDGED.load(Ordering::Relaxed));
        if read_since < SAMPLE {
            return;
        }
        let thrashed = JUDGED_THRASHED.load(Ordering::Relaxed);
        let thrashed_since = thrashed.saturating_sub(THRASHED_JUDGED.load(Ordering::Relaxed));
        READ_BACK_JUDGED.store(read, Ordering::Relaxed);
        THRASHED_JUDGED.store(thrashed, Ordering::Relaxed);
        if found_thrashing(read_since, thrashed_since) {
            NEED.raise(NEED.bytes().saturating_mul(2));
            RAISED.fetch_add(1, Ordering::Relaxed);
            // Said as it happens, so the stage lines around it name the walk
            // whose working set the need now states.
            eprintln!(
                "world store need: raised to {} byte(s); {} of the last {} read-back(s) came back \
                 within one turnover of the resident arrays",
                NEED.bytes(),
                thrashed_since,
                read_since
            );
        }
    }
}

/// **A sample of read-backs says the walk was found thrashing** when it is
/// a sample's worth and half or more of them were thrashing (the module
/// docs): what doubles the need.
fn found_thrashing(read_since: u64, thrashed_since: u64) -> bool {
    read_since >= SAMPLE && thrashed_since.saturating_mul(2) >= read_since
}

impl budget::Pageable for Store {
    /// Asked by another structure's relief: the stores give up what they
    /// hold over three quarters of their need ([`budget::spare`]) and no
    /// more, so a walk within the stores' need is not emptied for the
    /// reach graph's growth.
    fn give_up(&self, want: usize) -> usize {
        let spare = budget::spare(HELD.load(Ordering::Relaxed), NEED.bytes());
        self.evict(want.min(spare), false, None)
    }
}

/// Bring the stores' resident bytes under the budget, these stores first.
/// Under a test's budget (`budget::with_budget`), the one store whose array
/// just came resident is brought under it alone, out of its own arrays.
fn relieve(tally: &Arc<Tally>) {
    let s = store();
    if budget::override_here() != 0 {
        budget::relieve(
            budget::id_of(&**s),
            tally.held.load(Ordering::Relaxed),
            0,
            |want| s.evict(want, true, Some(tally)),
        );
        return;
    }
    budget::relieve(
        budget::id_of(&**s),
        HELD.load(Ordering::Relaxed),
        NEED.bytes(),
        |want| s.evict(want, true, None),
    );
}

/// Count an array of `bytes` as resident: an arrival.
fn hold(tally: &Tally, bytes: usize) {
    arrive();
    budget::hold(bytes);
    HELD.fetch_add(bytes, Ordering::Relaxed);
    tally.held.fetch_add(bytes, Ordering::Relaxed);
    RESIDENT.fetch_add(1, Ordering::Relaxed);
    EVENTS.fetch_add(1, Ordering::Relaxed);
}

/// Count an array of `bytes` as no longer resident.
fn release(tally: &Tally, bytes: usize) {
    budget::release(bytes);
    HELD.fetch_sub(bytes, Ordering::Relaxed);
    tally.held.fetch_sub(bytes, Ordering::Relaxed);
    RESIDENT.fetch_sub(1, Ordering::Relaxed);
}

/// **Was a read-back thrashing?** An array given up when the arrivals clock
/// stood at `since` and read back at `now` came back within one turnover of
/// the resident arrays when fewer arrays came resident in between than
/// `capacity` — the need stated (under a test's budget, that budget), in
/// arrays of its size: a store about twice as large would have kept it, so
/// the walk was still using it. One that waited longer fell off the
/// least-recently-used end in the ordinary course of a walk that moved on,
/// and is cold. The caller first asks whether the array was given up under
/// the need now stated and on the clock now in force ([`Paged::resident`]).
fn thrashing(since: u64, now: u64, capacity: usize) -> bool {
    now.saturating_sub(since) < capacity as u64
}

/// The need stated — under a test's budget, that budget — in arrays of
/// `bytes` each: the turnover a read-back is judged against.
fn capacity(bytes: usize) -> usize {
    match budget::override_here() {
        0 => NEED.bytes() / bytes.max(1),
        b => b / bytes.max(1),
    }
}

impl Paged {
    /// A fresh resident array, counted and listed with the store.
    fn new(idx: Idx, tally: &Arc<Tally>) -> Arc<Paged> {
        let bytes = idx.bytes();
        let p = Arc::new(Paged {
            slot: Mutex::new(Slot::Resident(Arc::new(idx))),
            used: AtomicU64::new(budget::tick()),
            tally: Arc::clone(tally),
        });
        hold(tally, bytes);
        store()
            .pages
            .lock()
            .expect("pages")
            .push(Arc::downgrade(&p));
        relieve(tally);
        p
    }

    /// A copy: its own array when resident, the same record when given up
    /// (a record is written once and never changed, so two sections may read
    /// it).
    /// A copy for a store whose clock is `tally`: a resident array is copied,
    /// a spilled one shares its record, which the copy never held, so its
    /// first read back is cold.
    fn duplicate(&self, tally: &Arc<Tally>) -> Arc<Paged> {
        let g = self.slot.lock().expect("a section's slot");
        match &*g {
            Slot::Resident(a) => {
                let idx = (**a).clone();
                drop(g);
                Paged::new(idx, tally)
            }
            Slot::Spilled { at, len, .. } => {
                let p = Arc::new(Paged {
                    slot: Mutex::new(Slot::Spilled {
                        at: *at,
                        len: *len,
                        since: None,
                    }),
                    used: AtomicU64::new(budget::tick()),
                    tally: Arc::clone(tally),
                });
                drop(g);
                store()
                    .pages
                    .lock()
                    .expect("pages")
                    .push(Arc::downgrade(&p));
                p
            }
        }
    }

    /// Write the array to the scratch file and drop it, when resident and
    /// not in use; the bytes freed.
    fn give_up(&self) -> usize {
        let Ok(mut g) = self.slot.try_lock() else {
            return 0;
        };
        let Slot::Resident(a) = &*g else {
            return 0;
        };
        let bytes = a.bytes();
        let record = a.encode();
        let mut w = shared().write.lock().expect("the stores' spill");
        let Some(sp) = w.as_mut() else {
            return 0;
        };
        let r = sp.append(&record);
        let (at, len) = sp.or_fail("take a write", r);
        drop(w);
        let since = Some((arrivals(), generation(), budget::override_here() != 0));
        *g = Slot::Spilled { at, len, since };
        release(&self.tally, bytes);
        GIVEN_UP.fetch_add(1, Ordering::Relaxed);
        bytes
    }

    /// The array, resident: read back from the file when it was given up.
    /// The second answer says whether it was read back, so the caller
    /// relieves the budget once its own use of the array is done.
    /// The array, resident: read back from the record when it was given up,
    /// and the read-back counted, thrashing or cold ([`thrashing`]).
    fn resident(&self, g: &mut Slot) -> (Arc<Idx>, bool) {
        match &*g {
            Slot::Resident(a) => (Arc::clone(a), false),
            Slot::Spilled { at, len, since } => {
                let record = shared()
                    .read
                    .as_ref()
                    .expect("an array given up was written to an open file")
                    .read(*at, *len);
                let idx = Arc::new(Idx::decode(&record));
                let now = arrivals();
                let judged = budget::override_here() == 0;
                let generation = generation();
                // Given up on another clock — by a thread under no test
                // budget while this one is under one, or the reverse — it was
                // not this walk's relief that gave it up: cold.
                if since.is_some_and(|(since, under, test)| {
                    test == !judged
                        && under == generation
                        && thrashing(since, now, capacity(idx.bytes()))
                }) {
                    self.tally.thrashed.fetch_add(1, Ordering::Relaxed);
                    THRASHED.fetch_add(1, Ordering::Relaxed);
                    if judged {
                        JUDGED_THRASHED.fetch_add(1, Ordering::Relaxed);
                    }
                }
                hold(&self.tally, idx.bytes());
                self.tally.read_back.fetch_add(1, Ordering::Relaxed);
                READ_BACK.fetch_add(1, Ordering::Relaxed);
                if judged {
                    JUDGED_READ_BACK.fetch_add(1, Ordering::Relaxed);
                }
                *g = Slot::Resident(Arc::clone(&idx));
                (idx, true)
            }
        }
    }

    /// What `f` makes of the array.
    #[inline]
    fn with<R>(&self, f: impl FnOnce(&Idx) -> R) -> R {
        let mut g = self.slot.lock().expect("a section's slot");
        self.used.store(budget::tick(), Ordering::Relaxed);
        let (idx, read_back) = self.resident(&mut g);
        let out = f(&idx);
        if read_back {
            relieve(&self.tally);
        }
        out
    }

    /// What `f` makes of the array, changing it; the bytes it holds are
    /// re-counted when its form changes.
    fn with_mut<R>(&self, f: impl FnOnce(&mut Idx) -> R) -> R {
        let mut g = self.slot.lock().expect("a section's slot");
        self.used.store(budget::tick(), Ordering::Relaxed);
        let (_, read_back) = self.resident(&mut g);
        let Slot::Resident(a) = &mut *g else {
            unreachable!("resident after a read-back")
        };
        let before = a.bytes();
        let out = f(Arc::make_mut(a));
        let after = a.bytes();
        if after > before {
            budget::hold(after - before);
            HELD.fetch_add(after - before, Ordering::Relaxed);
            self.tally.held.fetch_add(after - before, Ordering::Relaxed);
        } else if before > after {
            budget::release(before - after);
            HELD.fetch_sub(before - after, Ordering::Relaxed);
            self.tally.held.fetch_sub(before - after, Ordering::Relaxed);
        }
        if read_back || after > before {
            relieve(&self.tally);
        }
        out
    }

    /// The array pinned for a walk that will read `cells` of it: a reader
    /// iterating a section holds this and reads it without the lock.
    fn pin(&self) -> Arc<Idx> {
        let mut g = self.slot.lock().expect("a section's slot");
        self.used.store(budget::tick(), Ordering::Relaxed);
        let (idx, read_back) = self.resident(&mut g);
        if read_back {
            relieve(&self.tally);
        }
        idx
    }

    /// Bytes the array holds, resident or in the file.
    fn bytes(&self) -> usize {
        match &*self.slot.lock().expect("a section's slot") {
            Slot::Resident(a) => a.bytes(),
            Slot::Spilled { len, .. } => *len as usize,
        }
    }

    /// Whether the array is resident.
    fn is_resident(&self) -> bool {
        matches!(
            &*self.slot.lock().expect("a section's slot"),
            Slot::Resident(_)
        )
    }
}

impl Drop for Paged {
    fn drop(&mut self) {
        if let Ok(g) = self.slot.lock()
            && let Slot::Resident(a) = &*g
        {
            release(&self.tally, a.bytes());
        }
    }
}

/// **The stores' paging, stated** (ADR-0037 §1): what every store in the
/// process holds resident, what was given up and read back, the need stated
/// and the scratch file's size; `None` when no store ever held a section.
pub fn report() -> Option<String> {
    if EVENTS.load(Ordering::Relaxed) == 0 {
        return None;
    }
    let spilled = shared()
        .write
        .lock()
        .expect("the stores' spill")
        .as_ref()
        .map_or(0, spill::Spill::len);
    Some(format!(
        "world store pages: {} byte(s) resident over {} section(s) of every store; {} array(s) \
         given up, {} read back, {} of them within one turnover of the resident arrays; the \
         need raised {} time(s) for a walk found thrashing, to {} byte(s); {} byte(s) in the \
         scratch file (ADR-0037 §1)",
        HELD.load(Ordering::Relaxed),
        RESIDENT.load(Ordering::Relaxed),
        GIVEN_UP.load(Ordering::Relaxed),
        READ_BACK.load(Ordering::Relaxed),
        THRASHED.load(Ordering::Relaxed),
        RAISED.load(Ordering::Relaxed),
        NEED.bytes(),
        spilled
    ))
}

/// Arrays given up and read back so far, over the run.
#[cfg(test)]
fn traffic() -> (u64, u64, u64) {
    (
        GIVEN_UP.load(Ordering::Relaxed),
        READ_BACK.load(Ordering::Relaxed),
        RAISED.load(Ordering::Relaxed),
    )
}

// ---------------------------------------------------------------------------

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
    fn new(tally: &Arc<Tally>) -> Self {
        Section {
            palette: Vec::new(),
            idx: Paged::new(Idx::Narrow(Box::new([0u8; CELLS])), tally),
            count: 0,
        }
    }

    #[inline]
    fn get(&self, at: usize) -> Option<&V> {
        match self.idx.with(|idx| idx.get(at)) {
            0 => None,
            i => Some(&self.palette[i - 1]),
        }
    }

    /// The value palette index `i` (1-based) names.
    #[inline]
    fn value(&self, i: usize) -> &V {
        &self.palette[i - 1]
    }

    /// The palette index of `v` (1-based), entering it when new.
    fn index_of(&mut self, v: V) -> usize {
        if let Some(p) = self.palette.iter().position(|p| *p == v) {
            return p + 1;
        }
        if self.palette.len() == NARROW_PALETTE && self.idx.with(|i| matches!(i, Idx::Narrow(_))) {
            self.compact_palette();
            if self.palette.len() == NARROW_PALETTE {
                self.idx.with_mut(Idx::widen);
            }
        }
        self.palette.push(v);
        self.palette.len()
    }

    /// Drop the palette entries no cell uses, re-indexing the cells.
    fn compact_palette(&mut self) {
        let palette = &mut self.palette;
        self.idx.with_mut(|idx| {
            let mut used = vec![false; palette.len() + 1];
            for at in 0..CELLS {
                used[idx.get(at)] = true;
            }
            let mut renumber = vec![0usize; palette.len() + 1];
            let mut kept = Vec::with_capacity(palette.len());
            for (old, v) in palette.iter().enumerate() {
                if used[old + 1] {
                    kept.push(*v);
                    renumber[old + 1] = kept.len();
                }
            }
            if kept.len() == palette.len() {
                return;
            }
            for at in 0..CELLS {
                let old = idx.get(at);
                if old != 0 {
                    idx.set(at, renumber[old]);
                }
            }
            *palette = kept;
        });
    }

    /// Bytes this section holds: its indices (resident or in the file) and
    /// its palette.
    fn bytes(&self) -> usize {
        self.idx.bytes() + self.palette.capacity() * std::mem::size_of::<V>()
    }
}

impl<V> Default for SectionMap<V> {
    fn default() -> Self {
        SectionMap {
            sections: BTreeMap::new(),
            tally: Arc::default(),
            len: 0,
        }
    }
}

impl<V: Clone> Clone for SectionMap<V> {
    /// A copy sharing every index array with the original until one of the two
    /// writes it (ADR-0037 §2): a whole-world copy costs its palettes and its
    /// section index, and the arrays a copy's sparse writes touch, never the
    /// map. An array shared this way keeps the original store's clock until it
    /// is written.
    fn clone(&self) -> Self {
        SectionMap {
            sections: self.sections.iter().map(|(k, s)| (*k, s.share())).collect(),
            tally: Arc::default(),
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
        let tally = &self.tally;
        let s = self
            .sections
            .entry(section_of(&c))
            .or_insert_with(|| Section::new(tally));
        let at = local_of(&c);
        let before = s.get(at).copied();
        if before == Some(v) {
            return before;
        }
        s.own(tally);
        let i = s.index_of(v);
        s.idx.with_mut(|idx| idx.set(at, i));
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
        s.own(&self.tally);
        s.idx.with_mut(|idx| idx.set(at, 0));
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

    /// **Apply `writes` section by section** (ADR-0037 §2, assembly): the
    /// writes grouped by the section each lies in, the sections in key order,
    /// the writes of one section in the order listed (a stable sort), so a
    /// cell written twice keeps its two writes in order and the map written is
    /// the one the listed order writes — while the store's working set is one
    /// section at a time, whatever order the writes were listed in. `None`
    /// removes the cell.
    pub fn write_by_section(&mut self, writes: impl IntoIterator<Item = (Cell, Option<V>)>) {
        let mut writes: Vec<(Cell, Option<V>)> = writes.into_iter().collect();
        writes.sort_by_key(|(c, _)| section_of(c));
        for (c, v) in writes {
            match v {
                Some(v) => {
                    self.insert(c, v);
                }
                None => {
                    self.remove(&c);
                }
            }
        }
    }

    /// The section columns holding a cell — `(sx, sz)` — in key order, each
    /// with the inclusive `sy` range its sections span: the unit a walk that
    /// settles or sweeps columns takes up one at a time.
    pub fn section_columns(&self) -> Vec<([i32; 2], [i32; 2])> {
        let mut by: BTreeMap<[i32; 2], [i32; 2]> = BTreeMap::new();
        for k in self.sections.keys() {
            let e = by.entry([k[0], k[2]]).or_insert([k[1], k[1]]);
            e[0] = e[0].min(k[1]);
            e[1] = e[1].max(k[1]);
        }
        by.into_iter().collect()
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

    /// The bytes the sections hold: every index array (resident or in the
    /// scratch file) and palette. The store's cost, stated by the build.
    pub fn bytes(&self) -> usize {
        self.sections.values().map(Section::bytes).sum()
    }

    /// The inclusive box the store's sections span, in cells (section-aligned,
    /// so every cell held lies in it); `None` when no cell is held.
    pub fn section_bounds(&self) -> Option<(Cell, Cell)> {
        let mut lo = [i32::MAX; 3];
        let mut hi = [i32::MIN; 3];
        for k in self.sections.keys() {
            for a in 0..3 {
                lo[a] = lo[a].min(k[a] * SIDE);
                hi[a] = hi[a].max(k[a] * SIDE + SIDE - 1);
            }
        }
        (!self.sections.is_empty()).then_some((lo, hi))
    }

    /// The sections whose index array is resident now.
    pub fn resident_sections(&self) -> usize {
        self.sections
            .values()
            .filter(|s| s.idx.is_resident())
            .count()
    }

    /// This store's arrays read back so far, and how many of them came back
    /// within one turnover of the resident arrays (the module docs).
    #[cfg(test)]
    fn paging(&self) -> (u64, u64) {
        self.tally.read_back()
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

/// One section of a slab: its key, the section, its index array pinned for
/// the walk, and the local `z` range the box clips it to.
type Col<'a, V> = (Cell, &'a Section<V>, Arc<Idx>, usize, usize);

/// The sections of one section-`x` slab, in `(sy, sz)` order, with the local
/// ranges the iterator's box clips each to and each one's index array pinned
/// for the walk.
#[derive(Clone)]
struct Slab<'a, V> {
    /// `(section key, section, its pinned index array, lz0, lz1)`, in
    /// `(sy, sz)` order.
    cols: Vec<Col<'a, V>>,
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
                slab.cols.push((*key, sec, sec.idx.pin(), lz0, lz1));
            }
            if slab.cols.is_empty() {
                continue;
            }
            slab.lx = slab.lx0;
            slab.g = 0;
            slab.ly = slab.groups[0].2;
            slab.s = slab.groups[0].0;
            slab.lz = slab.cols[slab.s].3;
            slab.started = true;
            return true;
        }
    }

    /// Advance the slab's position by one cell; `false` when the slab is done.
    fn step(&mut self) -> bool {
        let slab = &mut self.slab;
        slab.lz += 1;
        if slab.lz <= slab.cols[slab.s].4 {
            return true;
        }
        slab.s += 1;
        let g = slab.groups[slab.g];
        if slab.s < g.1 {
            slab.lz = slab.cols[slab.s].3;
            return true;
        }
        slab.ly += 1;
        if slab.ly <= g.3 {
            slab.s = g.0;
            slab.lz = slab.cols[slab.s].3;
            return true;
        }
        slab.g += 1;
        if slab.g < slab.groups.len() {
            let g = slab.groups[slab.g];
            slab.ly = g.2;
            slab.s = g.0;
            slab.lz = slab.cols[slab.s].3;
            return true;
        }
        slab.lx += 1;
        if slab.lx <= slab.lx1 {
            slab.g = 0;
            let g = slab.groups[0];
            slab.ly = g.2;
            slab.s = g.0;
            slab.lz = slab.cols[slab.s].3;
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
            // The position names a cell; read it off the pinned array, then
            // advance.
            let slab = &self.slab;
            let (key, sec, idx, _, _) = &slab.cols[slab.s];
            let at = (slab.lx * 16 + slab.ly) * 16 + slab.lz;
            let (lx, ly, lz) = (slab.lx, slab.ly, slab.lz);
            let hit = match idx.get(at) {
                0 => None,
                i => Some((
                    [
                        key[0] * SIDE + lx as i32,
                        key[1] * SIDE + ly as i32,
                        key[2] * SIDE + lz as i32,
                    ],
                    sec.value(i),
                )),
            };
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
    /// every shape read back in order — under the default budget, and under
    /// a budget of one byte, where every array but the one in use is given up
    /// to the scratch file and read back when next asked (ADR-0037 §1).
    #[test]
    fn a_section_store_answers_what_a_map_answers() {
        for cut in [false, true] {
            let run = || {
                let (given0, read0, raised0) = traffic();
                let mut seed = 0x9e37_79b9_7f4a_7c15u64;
                let mut next = |n: u64| xorshift(&mut seed) % n;
                // Three cell pools: one section's worth, a span of sections
                // across the origin, and a wide sparse spread.
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
                            // 300 distinct values, so a dense section outgrows
                            // the narrow palette.
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
                    if s.idx.with(|i| matches!(i, Idx::Wide(_))) {
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
                let (given, read, raised) = traffic();
                (
                    given - given0,
                    read - read0,
                    raised - raised0,
                    store.sections(),
                    store.resident_sections(),
                )
            };
            let (given_up, read_back, raised, sections, resident) = if cut {
                budget::with_budget(1, run)
            } else {
                run()
            };
            eprintln!(
                "section store (budget {}): {sections} section(s), {resident} resident at the end, \
                 {given_up} array(s) given up, {read_back} read back, the need raised {raised} \
                 time(s), standing at {} byte(s)",
                if cut { "one byte" } else { "default" },
                NEED.bytes()
            );
            if cut {
                assert!(
                    given_up > 0,
                    "no array was given up: the case binds nothing"
                );
                assert!(read_back > 0, "no array was read back");
                assert!(
                    resident <= 2,
                    "{resident} arrays resident under a budget of one byte"
                );
            }
        }
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
            sec.idx.with(|i| matches!(i, Idx::Narrow(_))),
            "one live value keeps a narrow index"
        );
        assert!(sec.palette.len() <= NARROW_PALETTE);
        assert_eq!(s.bytes(), CELLS + sec.palette.capacity() * 4);
    }

    /// An array given up is read back as it was written, in both forms, and
    /// a copy of a section given up reads the same record.
    #[test]
    fn an_array_given_up_reads_back_as_written_in_both_forms() {
        budget::with_budget(1, || {
            let mut s: SectionMap<u32> = SectionMap::new();
            // A narrow section and a wide one.
            for i in 0..4096u32 {
                s.insert([i as i32 % 16, (i as i32 / 16) % 16, i as i32 / 256], i % 7);
                s.insert(
                    [32 + i as i32 % 16, (i as i32 / 16) % 16, i as i32 / 256],
                    i % 1000,
                );
            }
            // Touch a third, so both are given up.
            s.insert([64, 0, 0], 1);
            let narrow = s.sections.get(&[0, 0, 0]).expect("the narrow section");
            let wide = s.sections.get(&[2, 0, 0]).expect("the wide section");
            assert!(
                !narrow.idx.is_resident() && !wide.idx.is_resident(),
                "both sections given up under a budget of one byte"
            );
            assert!(wide.idx.bytes() == 2 * CELLS && narrow.idx.bytes() == CELLS);
            let copy = s.clone();
            for i in 0..4096u32 {
                let c = [i as i32 % 16, (i as i32 / 16) % 16, i as i32 / 256];
                assert_eq!(s.get(&c), Some(&(i % 7)));
                assert_eq!(copy.get(&c), Some(&(i % 7)));
                let w = [32 + i as i32 % 16, (i as i32 / 16) % 16, i as i32 / 256];
                assert_eq!(s.get(&w), Some(&(i % 1000)));
                assert_eq!(copy.get(&w), Some(&(i % 1000)));
            }
        });
    }

    /// **A walk found thrashing is judged so, and such a sample raises the
    /// need** (ADR-0037 §1): a store twice the budget in force (this
    /// thread's own, so no other test's traffic enters), read cell by cell
    /// at random, reads arrays back with few arrivals between them — half
    /// or more of its read-backs are within one turnover — and a sample of
    /// read-backs of which half or more are such doubles the need
    /// (`found_thrashing`, the one rule the judgement applies), while a
    /// smaller share, or fewer than a sample, does not. The store is read
    /// through once in order first, so the read-backs judged are the random
    /// walk's own, not the cold reload of a store written past the budget.
    /// The walk is judged in the thread's own generation of the need, so no
    /// other thread's raise enters either: taken again with the process's
    /// need raised every 32 reads, it finds the same.
    #[test]
    fn a_walk_found_thrashing_raises_the_need() {
        let n = 512i32;
        let budget = (n as usize / 2) * CELLS;
        // The walk, on a fresh store; with `raises`, the process's need is
        // raised every 32 reads, as another thread's walk would raise it.
        let walk = |raises: bool| {
            let mut s: SectionMap<u32> = SectionMap::new();
            budget::with_budget(budget, || {
                for k in 0..n {
                    s.insert([k * 16, 0, 0], 1);
                }
                for k in 0..n {
                    assert_eq!(s.get(&[k * 16, 0, 0]), Some(&1));
                }
                let (read0, thrashed0) = s.paging();
                let mut seed = 0x1234_5678_9abc_def1u64;
                let mut reads = 0u64;
                for _ in 0..10 {
                    for _ in 0..n {
                        seed ^= seed << 13;
                        seed ^= seed >> 7;
                        seed ^= seed << 17;
                        let k = (seed % n as u64) as i32;
                        assert_eq!(s.get(&[k * 16, 0, 0]), Some(&1));
                        reads += 1;
                        if raises && reads.is_multiple_of(32) {
                            RAISED.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
                let (read, thrashed) = s.paging();
                (read - read0, thrashed - thrashed0, reads)
            })
        };
        let (own_read, own_thrashed, reads) = walk(true);
        // No other thread's raise moves what a walk under a test's budget
        // finds: the generation it is judged in is the thread's own.
        assert_eq!(
            walk(false),
            (own_read, own_thrashed, reads),
            "the process's raises moved a walk under a test's budget"
        );
        eprintln!(
            "thrash: {n} section(s) over a budget of {budget} byte(s); {reads} read(s), \
             {own_read} read back, {own_thrashed} of them within one turnover"
        );
        assert!(
            own_read > 0,
            "no array was read back: the case binds nothing"
        );
        assert!(
            own_thrashed * 2 >= own_read,
            "{own_thrashed} of {own_read} read-backs of a store read at random over twice the \
             budget were judged thrashing"
        );
        assert!(found_thrashing(SAMPLE, SAMPLE / 2));
        assert!(found_thrashing(3 * SAMPLE, 3 * SAMPLE));
        assert!(!found_thrashing(SAMPLE, SAMPLE / 2 - 1));
        assert!(!found_thrashing(SAMPLE - 1, SAMPLE - 1));
    }

    /// **The capacity a read-back is judged against is the need in arrays**
    /// (the module docs) — under a test's budget, that budget — so a need
    /// the walks raise is what the next read-back is set against, whatever
    /// the stores happen to hold while nothing else asks.
    #[test]
    fn the_capacity_is_the_need_in_arrays() {
        assert_eq!(capacity(CELLS), NEED.bytes() / CELLS);
        assert_eq!(capacity(2 * CELLS), NEED.bytes() / (2 * CELLS));
        assert_eq!(budget::with_budget(8 * CELLS, || capacity(CELLS)), 8);
        assert!(thrashing(100, 107, 8));
        assert!(!thrashing(100, 108, 8));
    }

    /// **A sparse walk over a spilled store raises nothing** (the module
    /// docs): a store four times the budget in force, every section written
    /// once and then one cell of each read and one written, round after round
    /// in one order, reads every array back cold — given up long before,
    /// however few cells of it the walk touches — so none of its read-backs
    /// is thrashing. The assembly's sliver writes into sections placed
    /// earlier, and a second sequential pass over the world, are this walk.
    #[test]
    fn a_sparse_walk_over_a_spilled_store_is_cold() {
        // The store under a budget of a quarter of itself, this thread's own
        // (`budget::with_budget`, kept over this store alone), so what the
        // other tests hold cannot change what this walk's read-backs say: a
        // round gives up every array it reads back, so each comes back at
        // least three quarters of the store later.
        let n = 2048i32;
        let budget = (n as usize / 4) * CELLS;
        let mut s: SectionMap<u32> = SectionMap::new();
        let (own_read, own_thrashed) = budget::with_budget(budget, || {
            for k in 0..n {
                s.insert([k * 16, 0, 0], 1);
            }
            for round in 0..3u32 {
                for k in 0..n {
                    assert_eq!(s.get(&[k * 16, 0, 0]), Some(&1));
                    s.insert([k * 16, 1, 0], round + 2);
                }
            }
            // A copy shares the records given up, which it never held: its
            // first read of each is cold too.
            // A copy shares the arrays, so its walk reads them back on the
            // original's clock, cold as the original's own walk would be, and
            // the copy owns no array until it writes one.
            let (before_read, before_thrashed) = s.paging();
            let copy = s.clone();
            for k in 0..n {
                assert_eq!(copy.get(&[k * 16, 1, 0]), Some(&4));
            }
            let (after_read, after_thrashed) = s.paging();
            assert!(
                after_read > before_read,
                "the copy's walk read no array back: the case binds nothing"
            );
            assert_eq!(
                after_thrashed,
                before_thrashed,
                "{} of the copy's {} reads were judged thrashing",
                after_thrashed - before_thrashed,
                after_read - before_read
            );
            assert_eq!(
                copy.paging(),
                (0, 0),
                "a copy owns no array until it writes"
            );
            s.paging()
        });
        eprintln!(
            "sparse: {n} section(s) over a budget of {budget} byte(s); {own_read} read back, \
             {own_thrashed} of them within one turnover"
        );
        assert!(
            own_read > 0,
            "no array was read back: the case binds nothing"
        );
        assert_eq!(
            own_thrashed, 0,
            "{own_thrashed} read-back(s) of a walk that touched each of {n} sections once a round \
             were judged thrashing"
        );
    }

    /// **A store given up before another store's arrivals is read back
    /// cold** (the module docs): under this thread's budget, a store of
    /// twice it written once, whose later arrivals give its earlier arrays
    /// up; then a second store of twice the budget written once, every
    /// arrival of it advancing the one clock; then the first read through
    /// once more, its last-given-up arrays first: every read-back of it
    /// comes after more arrivals than the budget holds, and none is
    /// thrashing. The judged copy's reload after the write judge has grown
    /// its own world is this walk. A clock of the first store's own
    /// give-ups, which stood still while the second grew, would call its
    /// last-given-up arrays thrashing.
    #[test]
    fn a_store_another_stages_growth_pushed_out_is_read_back_cold() {
        let n = 512i32;
        let budget = (n as usize / 2) * CELLS;
        let mut first: SectionMap<u32> = SectionMap::new();
        let mut second: SectionMap<u32> = SectionMap::new();
        let (given_up, read, thrashed) = budget::with_budget(budget, || {
            for k in 0..n {
                first.insert([k * 16, 0, 0], 1);
            }
            let given_up = n as usize - first.resident_sections();
            for k in 0..n {
                second.insert([k * 16, 0, 64], 2);
            }
            let (read_before, thrashed_before) = first.paging();
            for k in (0..n).rev() {
                assert_eq!(first.get(&[k * 16, 0, 0]), Some(&1));
            }
            let (read, thrashed) = first.paging();
            (given_up, read - read_before, thrashed - thrashed_before)
        });
        eprintln!(
            "reload: {n} section(s) twice over a budget of {budget} byte(s), {given_up} of the \
             first given up; {read} read back, {thrashed} of them within one turnover"
        );
        assert!(
            given_up > 0 && read >= given_up as u64,
            "{read} of {given_up} arrays given up were read back: the case binds nothing"
        );
        assert_eq!(
            thrashed, 0,
            "{thrashed} read-back(s) of a store given up before another store's arrivals were \
             judged thrashing"
        );
    }

    /// **An array given up under an earlier need is read back cold** (the
    /// module docs): under this thread's budget, a store of twice it
    /// written once gives up its early arrays as its later ones arrive; this
    /// thread's generation of the need then moves on, and the arrays given
    /// up last — fewest arrivals since — are read back first: none is thrashing,
    /// since the need they were given up under is not the need in force.
    /// Without the generation, the last given up come back within the
    /// turnover and are thrashing.
    #[test]
    fn an_array_given_up_under_an_earlier_need_is_read_back_cold() {
        let n = 512i32;
        let budget = (n as usize / 2) * CELLS;
        let mut s: SectionMap<u32> = SectionMap::new();
        let (given_up, read, thrashed) = budget::with_budget(budget, || {
            for k in 0..n {
                s.insert([k * 16, 0, 0], 1);
            }
            let given_up = n as usize - s.resident_sections();
            RAISED_HERE.with(|r| r.set(r.get() + 1));
            let (read_before, thrashed_before) = s.paging();
            for k in (0..n).rev() {
                assert_eq!(s.get(&[k * 16, 0, 0]), Some(&1));
            }
            let (read, thrashed) = s.paging();
            (given_up, read - read_before, thrashed - thrashed_before)
        });
        eprintln!(
            "earlier need: {n} section(s) over a budget of {budget} byte(s), {given_up} given \
             up; {read} read back after the raise, {thrashed} of them within one turnover"
        );
        assert!(
            given_up > 0 && read >= given_up as u64,
            "{read} of {given_up} arrays given up were read back: the case binds nothing"
        );
        assert_eq!(
            thrashed, 0,
            "{thrashed} read-back(s) of arrays given up under an earlier need were judged \
             thrashing"
        );
    }

    /// **Writes by section lay the map the listed order lays**: generated
    /// writes over several sections, cells written more than once (a later
    /// write winning, a removal among them), applied section by section and
    /// in the order listed, read alike cell for cell; and the section columns
    /// listed are the sections' columns with their `sy` spans.
    #[test]
    fn writes_by_section_lay_the_map_the_listed_order_lays() {
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut next = |n: u64| xorshift(&mut seed) % n;
        for round in 0..20 {
            let mut writes: Vec<(Cell, Option<u32>)> = Vec::new();
            let n = 50 + round * 40;
            for _ in 0..n {
                let c = [
                    next(70) as i32 - 35,
                    next(40) as i32 - 20,
                    next(70) as i32 - 35,
                ];
                let v = if next(4) == 0 {
                    None
                } else {
                    Some(next(6) as u32)
                };
                writes.push((c, v));
            }
            // A base the writes land on: some cells held before.
            let mut listed: SectionMap<u32> = SectionMap::new();
            let mut tree: BTreeMap<Cell, u32> = BTreeMap::new();
            for _ in 0..n / 2 {
                let c = [
                    next(70) as i32 - 35,
                    next(40) as i32 - 20,
                    next(70) as i32 - 35,
                ];
                listed.insert(c, 9);
                tree.insert(c, 9);
            }
            let mut by_section = listed.clone();
            for (c, v) in &writes {
                match v {
                    Some(v) => {
                        listed.insert(*c, *v);
                        tree.insert(*c, *v);
                    }
                    None => {
                        listed.remove(c);
                        tree.remove(c);
                    }
                }
            }
            by_section.write_by_section(writes.iter().copied());
            assert_eq!(
                by_section.iter().collect::<Vec<_>>(),
                listed.iter().collect::<Vec<_>>(),
                "round {round}"
            );
            assert_eq!(
                by_section.iter().map(|(c, v)| (c, *v)).collect::<Vec<_>>(),
                tree.iter().map(|(c, v)| (*c, *v)).collect::<Vec<_>>(),
                "round {round} against the tree"
            );
            let mut columns: BTreeMap<[i32; 2], [i32; 2]> = BTreeMap::new();
            for c in tree.keys() {
                let k = section_of(c);
                let e = columns.entry([k[0], k[2]]).or_insert([k[1], k[1]]);
                e[0] = e[0].min(k[1]);
                e[1] = e[1].max(k[1]);
            }
            assert_eq!(
                by_section.section_columns(),
                columns.into_iter().collect::<Vec<_>>(),
                "round {round}: the section columns"
            );
        }
    }

    /// **A copy shares its arrays until one side writes**: the copy's sections
    /// hold the original's arrays; a write into the copy gives that one section
    /// its own array and no other, the original reads as before; a write into
    /// the original after that touches no array the copy holds.
    #[test]
    fn a_copy_shares_its_arrays_until_one_side_writes() {
        let mut a: SectionMap<u32> = SectionMap::new();
        for i in 0..10 {
            a.insert([i * 16, 0, 0], 1);
            a.insert([i * 16 + 3, 5, 7], 2);
        }
        let mut b = a.clone();
        let shared = |a: &SectionMap<u32>, b: &SectionMap<u32>| {
            a.sections
                .iter()
                .filter(|(k, s)| {
                    b.sections
                        .get(*k)
                        .is_some_and(|t| Arc::ptr_eq(&s.idx, &t.idx))
                })
                .count()
        };
        assert_eq!(shared(&a, &b), 10, "every array shared after the copy");
        b.insert([3, 5, 7], 9);
        assert_eq!(shared(&a, &b), 9, "the written section has its own array");
        assert_eq!(a.get(&[3, 5, 7]), Some(&2), "the original reads as before");
        assert_eq!(b.get(&[3, 5, 7]), Some(&9));
        b.remove(&[16, 0, 0]);
        assert_eq!(shared(&a, &b), 8);
        assert_eq!(a.get(&[16, 0, 0]), Some(&1));
        assert_eq!(b.get(&[16, 0, 0]), None);
        a.insert([32, 0, 0], 7);
        assert_eq!(
            shared(&a, &b),
            7,
            "a write into the original parts that section"
        );
        assert_eq!(b.get(&[32, 0, 0]), Some(&1), "the copy reads as before");
        assert_eq!(a.get(&[32, 0, 0]), Some(&7));
        assert_eq!(
            a.iter().count() + 1,
            b.iter().count() + 2,
            "the copy lost one cell, the original gained none"
        );
    }
}
