//! **The window snapshot, paged** (ADR-0037 §2): every window-long array a
//! graph keeps ([`super::Chunked`]: the snapshot's codes, the raw membership)
//! is held as pages of [`PAGE_CELLS`] values, each distinct page once in the
//! process — hash-consed, so the configurations of one campaign, which differ
//! in a few cells, and the open sea, which repeats column after column, hold
//! one copy of a page however many arrays hold it.
//!
//! A page is written to a scratch file ([`super::spill`]) when it is first
//! made, and at most a budget of pages is **resident** ([`budget`]); past it
//! the least recently read is given up and read back from the file when it is
//! next asked for. The budget is the working set an expansion needs: a node's
//! launches read its reach chunk and the eight round it, so the budget holds
//! that 3 × 3 ring of the tallest window built, several times over for the
//! threads that expand chunks at once. Each thread keeps the last few pages it
//! read, so a launch that reads round one cell does not take the lock.
//!
//! What an array answers never depends on the budget or on which pages are
//! resident: a page read back is the bytes it was written as. A store whose
//! file cannot be opened keeps every page in memory.

use super::spill::Spill;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// A page holds `1 << PAGE_BITS` values.
pub(super) const PAGE_BITS: u32 = 12;
pub(super) const PAGE_CELLS: usize = 1 << PAGE_BITS;

/// A value a paged array holds: a fixed-width little-endian record.
pub(in crate::compiler::nav) trait Paged:
    Copy + PartialEq + std::hash::Hash + Send + Sync + 'static
{
    const BYTES: usize;
    fn store() -> &'static Store<Self>;
    fn put(v: &[Self], out: &mut Vec<u8>);
    fn take(b: &[u8]) -> Vec<Self>;
    /// Run `f` over this thread's recently read pages.
    fn recent<R>(f: impl FnOnce(&mut Recent<Self>) -> R) -> R;
}

/// The pages a thread read last, by id.
pub(in crate::compiler::nav) struct Recent<T> {
    slots: [(u32, Option<Arc<[T]>>); RECENT],
    next: usize,
}

const RECENT: usize = 8;

impl<T> Default for Recent<T> {
    fn default() -> Self {
        Recent {
            slots: std::array::from_fn(|_| (u32::MAX, None)),
            next: 0,
        }
    }
}

impl<T> Recent<T> {
    fn get(&self, id: u32) -> Option<&Arc<[T]>> {
        self.slots
            .iter()
            .find(|(i, p)| *i == id && p.is_some())
            .and_then(|(_, p)| p.as_ref())
    }

    fn keep(&mut self, id: u32, p: Arc<[T]>) {
        self.slots[self.next] = (id, Some(p));
        self.next = (self.next + 1) % RECENT;
    }
}

/// Every page of one value type.
pub(in crate::compiler::nav) struct Store<T> {
    inner: Mutex<Inner<T>>,
}

struct Inner<T> {
    /// The ids of the pages by the hash of their values.
    by_hash: HashMap<u64, Vec<u32>>,
    /// Each page's offset in the file.
    at: Vec<u64>,
    /// The resident pages and when each was last read.
    resident: HashMap<u32, (Arc<[T]>, u64)>,
    tick: u64,
    /// The file; `None` when it could not be opened, and then every page
    /// stays resident.
    spill: Option<Spill>,
    opened: bool,
}

impl<T> Default for Store<T> {
    fn default() -> Self {
        Store {
            inner: Mutex::new(Inner {
                by_hash: HashMap::new(),
                at: Vec::new(),
                resident: HashMap::new(),
                tick: 0,
                spill: None,
                opened: false,
            }),
        }
    }
}

/// Pages read back from the file, over the run.
static LOADS: AtomicU64 = AtomicU64::new(0);
/// The resident budget, in pages of either type; `0` until a window sets it.
static BUDGET: AtomicUsize = AtomicUsize::new(0);
/// Pages written, and the bytes, over the run.
static PAGES: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

/// The fewest pages the budget holds.
const FLOOR: usize = 64;
/// How many 3 × 3 rings the budget holds: one per thread expanding a chunk.
const RINGS: usize = 4;

/// A budget a test set, in force on every thread while it is set: the pages
/// are read on the threads that expand nodes, and no answer depends on it.
static OVERRIDE: AtomicUsize = AtomicUsize::new(0);

/// The resident budget in force, in pages per value type.
pub(super) fn budget() -> usize {
    match OVERRIDE.load(Ordering::Relaxed) {
        0 => BUDGET.load(Ordering::Relaxed).max(FLOOR),
        b => b,
    }
}

/// Run `f` with the resident budget at `pages` on every thread.
#[cfg(test)]
pub(super) fn with_budget<R>(pages: usize, f: impl FnOnce() -> R) -> R {
    let was = OVERRIDE.swap(pages, Ordering::Relaxed);
    let out = f();
    OVERRIDE.store(was, Ordering::Relaxed);
    out
}

/// Note a window `height` cells tall: the budget holds [`RINGS`] of its 3 × 3
/// rings of reach chunks of side `side`.
pub(super) fn window(height: usize, side: usize) {
    let ring = (3 * side) * (3 * side) * height;
    BUDGET.fetch_max(RINGS * ring.div_ceil(PAGE_CELLS), Ordering::Relaxed);
}

/// Pages written, bytes written, pages read back, and the budget.
pub(super) fn traffic() -> (u64, u64, u64, usize) {
    (
        PAGES.load(Ordering::Relaxed),
        BYTES.load(Ordering::Relaxed),
        LOADS.load(Ordering::Relaxed),
        budget(),
    )
}

/// The pages resident now, over both stores.
pub(super) fn resident() -> usize {
    resident_of::<u32>() + resident_of::<u16>()
}

fn resident_of<T: Paged>() -> usize {
    T::store().inner.lock().expect("pages").resident.len()
}

impl<T: Paged> Inner<T> {
    fn evict(&mut self) {
        if self.spill.is_none() {
            return;
        }
        let budget = budget();
        while self.resident.len() > budget {
            let oldest = self
                .resident
                .iter()
                .min_by_key(|(id, (_, t))| (*t, **id))
                .map(|(id, _)| *id)
                .expect("over budget means non-empty");
            self.resident.remove(&oldest);
        }
    }

    /// Page `id`, resident or read back.
    fn page(&mut self, id: u32) -> Arc<[T]> {
        self.tick += 1;
        let tick = self.tick;
        if let Some((p, t)) = self.resident.get_mut(&id) {
            *t = tick;
            return Arc::clone(p);
        }
        let sp = self
            .spill
            .as_ref()
            .expect("a page given up was written to the file");
        let bytes = sp
            .read(self.at[id as usize], (PAGE_CELLS * T::BYTES) as u32)
            .expect("the snapshot's scratch file reads back");
        LOADS.fetch_add(1, Ordering::Relaxed);
        let p: Arc<[T]> = Arc::from(T::take(&bytes));
        self.resident.insert(id, (Arc::clone(&p), tick));
        self.evict();
        p
    }
}

impl<T: Paged> Store<T> {
    /// The id of the page holding `values` (at most [`PAGE_CELLS`]; a short
    /// last page is padded with its first value), made when none holds them.
    pub(super) fn intern(&self, values: &[T]) -> u32 {
        use std::hash::{BuildHasher, BuildHasherDefault};
        let mut v = values.to_vec();
        v.resize(PAGE_CELLS, values[0]);
        let h = BuildHasherDefault::<super::flights::Mix>::default().hash_one(&v);
        let mut g = self.inner.lock().expect("pages");
        if !g.opened {
            g.opened = true;
            g.spill = Spill::open("snapshot").ok();
        }
        let ids = g.by_hash.get(&h).cloned().unwrap_or_default();
        for id in ids {
            if *g.page(id) == v[..] {
                return id;
            }
        }
        let id = u32::try_from(g.at.len()).expect("under 4 G pages");
        let mut bytes = Vec::with_capacity(PAGE_CELLS * T::BYTES);
        T::put(&v, &mut bytes);
        let at = match g.spill.as_mut().map(|s| s.append(&bytes)) {
            Some(Ok((at, _))) => at,
            Some(Err(_)) => {
                // The file failed: every page stays resident from here.
                g.spill = None;
                0
            }
            None => 0,
        };
        PAGES.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(bytes.len() as u64, Ordering::Relaxed);
        g.at.push(at);
        g.by_hash.entry(h).or_default().push(id);
        g.tick += 1;
        let tick = g.tick;
        g.resident.insert(id, (Arc::from(v), tick));
        g.evict();
        id
    }

    /// Page `id`: from this thread's recent pages, or the store.
    #[inline]
    pub(super) fn page(&self, id: u32) -> Arc<[T]> {
        T::recent(|r| {
            if let Some(p) = r.get(id) {
                return Arc::clone(p);
            }
            let p = self.inner.lock().expect("pages").page(id);
            r.keep(id, Arc::clone(&p));
            p
        })
    }

    /// The value at `j` of page `id`.
    #[inline]
    pub(super) fn at(&self, id: u32, j: usize) -> T {
        T::recent(|r| {
            if let Some(p) = r.get(id) {
                return p[j];
            }
            let p = self.inner.lock().expect("pages").page(id);
            let v = p[j];
            r.keep(id, p);
            v
        })
    }
}

macro_rules! paged {
    ($t:ty, $n:expr) => {
        impl Paged for $t {
            const BYTES: usize = $n;
            fn store() -> &'static Store<$t> {
                static S: std::sync::OnceLock<Store<$t>> = std::sync::OnceLock::new();
                S.get_or_init(Store::default)
            }
            fn put(v: &[$t], out: &mut Vec<u8>) {
                for x in v {
                    out.extend_from_slice(&x.to_le_bytes());
                }
            }
            fn take(b: &[u8]) -> Vec<$t> {
                b.chunks_exact($n)
                    .map(|c| <$t>::from_le_bytes(c.try_into().expect("a whole value")))
                    .collect()
            }
            fn recent<R>(f: impl FnOnce(&mut Recent<$t>) -> R) -> R {
                thread_local! {
                    static R: std::cell::RefCell<Recent<$t>> = std::cell::RefCell::new(Recent::default());
                }
                R.with(|r| f(&mut r.borrow_mut()))
            }
        }
    };
}

paged!(u32, 4);
paged!(u16, 2);
