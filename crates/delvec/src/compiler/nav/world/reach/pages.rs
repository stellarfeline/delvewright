//! **The window snapshot, paged** (ADR-0037 §2): every window-long array a
//! graph keeps ([`super::Chunked`]: the snapshot's codes, the raw membership)
//! is held as pages of [`PAGE_CELLS`] values, each distinct page once in the
//! process — hash-consed, so the configurations of one campaign, which differ
//! in a few cells, and the open sea, which repeats column after column, hold
//! one copy of a page however many arrays hold it.
//!
//! A page is written to a scratch file ([`super::spill`]) when it is first
//! made, and the pages **resident** count against the one resident budget
//! ([`super::budget`]); past it the least recently read is given up and read
//! back from the file when it is next asked for. The store's stated need is
//! the working set an expansion needs: a node's launches read its reach chunk
//! and the eight round it, so the need holds that 3 × 3 ring of the tallest
//! window built, [`RINGS`] times over for the threads that expand chunks at
//! once. Each thread keeps the last few pages it read ([`Recent`]), so a
//! launch that reads round one cell does not take the lock; how often that
//! serves a read is counted and printed with the run.
//!
//! What an array answers never depends on the budget or on which pages are
//! resident: a page read back is the bytes it was written as. A store whose
//! file cannot be opened keeps every page in memory.

use super::budget;
use super::spill::Spill;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};

/// A page holds `1 << PAGE_BITS` values.
pub(super) const PAGE_BITS: u32 = 12;
pub(super) const PAGE_CELLS: usize = 1 << PAGE_BITS;

/// A value a paged array holds: a fixed-width little-endian record.
pub(in crate::compiler::nav) trait Paged:
    Copy + PartialEq + std::hash::Hash + Send + Sync + 'static
{
    const BYTES: usize;
    fn store() -> &'static Arc<Store<Self>>;
    fn put(v: &[Self], out: &mut Vec<u8>);
    fn take(b: &[u8]) -> Vec<Self>;
    /// Run `f` over this thread's recently read pages.
    fn recent<R>(f: impl FnOnce(&mut Recent<Self>) -> R) -> R;
}

/// The pages a thread read last, by id, and how often they served a read
/// ([`Recent::hits`]) against how often the store was asked
/// ([`Recent::misses`]) — counted here, on the thread, and folded into the
/// run's figures when the thread ends or the run reports ([`traffic`]).
pub(in crate::compiler::nav) struct Recent<T> {
    slots: [(u32, Option<Arc<[T]>>); RECENT],
    next: usize,
    hits: u64,
    misses: u64,
}

const RECENT: usize = 8;

impl<T> Default for Recent<T> {
    fn default() -> Self {
        Recent {
            slots: std::array::from_fn(|_| (u32::MAX, None)),
            next: 0,
            hits: 0,
            misses: 0,
        }
    }
}

impl<T> Drop for Recent<T> {
    fn drop(&mut self) {
        self.flush();
    }
}

impl<T> Recent<T> {
    fn get(&mut self, id: u32) -> Option<&Arc<[T]>> {
        let hit = self.slots.iter().position(|(i, p)| *i == id && p.is_some());
        match hit {
            Some(i) => {
                self.hits += 1;
                self.slots[i].1.as_ref()
            }
            None => {
                self.misses += 1;
                None
            }
        }
    }

    fn keep(&mut self, id: u32, p: Arc<[T]>) {
        self.slots[self.next] = (id, Some(p));
        self.next = (self.next + 1) % RECENT;
    }

    /// Fold this thread's counts into the run's.
    fn flush(&mut self) {
        HITS.fetch_add(std::mem::take(&mut self.hits), Ordering::Relaxed);
        MISSES.fetch_add(std::mem::take(&mut self.misses), Ordering::Relaxed);
    }
}

/// Reads served by a thread's recent pages, and reads that went to the store,
/// over the run, as folded so far.
static HITS: AtomicU64 = AtomicU64::new(0);
static MISSES: AtomicU64 = AtomicU64::new(0);

/// Every page of one value type.
pub(in crate::compiler::nav) struct Store<T> {
    inner: Mutex<Inner<T>>,
}

struct Inner<T> {
    /// The ids of the pages by the hash of their values.
    by_hash: HashMap<u64, Vec<u32>>,
    /// Each page's offset in the file; [`HELD`] for a page the file did
    /// not take, which is never given up.
    at: Vec<u64>,
    /// The resident pages and when each was last read.
    resident: HashMap<u32, (Arc<[T]>, u64)>,
    tick: u64,
    /// The file; `None` when it could not be opened, and then every page
    /// stays resident ([`HELD`]).
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

/// The offset of a page the file did not take.
const HELD: u64 = u64::MAX;

/// Pages read back from the file, over the run.
static LOADS: AtomicU64 = AtomicU64::new(0);
/// Pages written, and the bytes, over the run.
static PAGES: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

/// The fewest pages the need holds, per value type.
const FLOOR: usize = 64;
/// How many 3 × 3 rings the need holds: one per thread expanding a chunk.
const RINGS: usize = 4;
/// The bytes a cell costs over both value types.
pub(super) const CELL_BYTES: usize = 4 + 2;

/// The snapshot's stated need: [`RINGS`] 3 × 3 rings of the tallest window,
/// over both value types.
static NEED: budget::Need = budget::Need::new(
    "the window snapshot's pages",
    FLOOR * PAGE_CELLS * CELL_BYTES,
);

/// Note a window `height` cells tall: the need holds [`RINGS`] of its 3 × 3
/// rings of reach chunks of side `side`.
pub(super) fn window(height: usize, side: usize) {
    NEED.raise(need_for(height, side));
}

/// The need a window `height` cells tall states: [`RINGS`] of its 3 × 3 rings
/// of reach chunks of side `side`, in whole pages, over both value types.
pub(super) fn need_for(height: usize, side: usize) -> usize {
    let ring = (3 * side) * (3 * side) * height;
    RINGS * ring.div_ceil(PAGE_CELLS) * PAGE_CELLS * CELL_BYTES
}

/// The snapshot's stated need, in bytes.
pub(super) fn need() -> usize {
    NEED.bytes()
}

/// Pages written, bytes written, pages read back, reads served by a thread's
/// recent pages, and reads that went to the store — this thread's counts
/// folded in first (the threads that expand chunks fold theirs as they end).
pub(super) fn traffic() -> (u64, u64, u64, u64, u64) {
    <u32 as Paged>::recent(Recent::flush);
    <u16 as Paged>::recent(Recent::flush);
    (
        PAGES.load(Ordering::Relaxed),
        BYTES.load(Ordering::Relaxed),
        LOADS.load(Ordering::Relaxed),
        HITS.load(Ordering::Relaxed),
        MISSES.load(Ordering::Relaxed),
    )
}

/// The pages resident now, over both stores.
pub(super) fn resident() -> usize {
    resident_of::<u32>() + resident_of::<u16>()
}

/// What the snapshot's scratch files have held, over both stores.
pub(super) fn usage() -> super::spill::Usage {
    usage_of::<u32>() + usage_of::<u16>()
}

fn usage_of<T: Paged>() -> super::spill::Usage {
    T::store()
        .inner
        .lock()
        .expect("pages")
        .spill
        .as_ref()
        .map_or_else(Default::default, super::spill::Spill::usage)
}

fn resident_of<T: Paged>() -> usize {
    T::store().inner.lock().expect("pages").resident.len()
}

impl<T: Paged> Inner<T> {
    /// The bytes the resident pages hold.
    fn held(&self) -> usize {
        self.resident.len() * PAGE_CELLS * T::BYTES
    }

    /// Give up the least recently read pages the file holds until `want`
    /// bytes are freed; the bytes freed.
    fn evict(&mut self, want: usize) -> usize {
        let mut freed = 0;
        while freed < want {
            let at = &self.at;
            let Some(oldest) = self
                .resident
                .iter()
                .filter(|(id, _)| at[**id as usize] != HELD)
                .min_by_key(|(id, (_, t))| (*t, **id))
                .map(|(id, _)| *id)
            else {
                // Every resident page is one the file did not take.
                break;
            };
            self.resident.remove(&oldest);
            freed += PAGE_CELLS * T::BYTES;
        }
        budget::release(freed);
        freed
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
        let bytes = sp.or_fail(
            "read a page back",
            sp.read(self.at[id as usize], (PAGE_CELLS * T::BYTES) as u32),
        );
        LOADS.fetch_add(1, Ordering::Relaxed);
        let p: Arc<[T]> = Arc::from(T::take(&bytes));
        self.resident.insert(id, (Arc::clone(&p), tick));
        budget::hold(PAGE_CELLS * T::BYTES);
        p
    }
}

impl<T: Paged> budget::Pageable for Store<T> {
    fn give_up(&self, want: usize) -> usize {
        match self.inner.try_lock() {
            Ok(mut g) => g.evict(want),
            Err(_) => 0,
        }
    }
}

impl<T: Paged> Store<T> {
    /// The store of this value type, registered with the budget when made.
    fn shared() -> Arc<Store<T>> {
        let s = Arc::new(Store::default());
        let w: Weak<dyn budget::Pageable> = {
            let w: Weak<Store<T>> = Arc::downgrade(&s);
            w
        };
        budget::join(budget::id_of(&*s), w);
        s
    }

    /// Bring the resident bytes under the budget, this store first
    /// ([`budget::relieve`]).
    fn relieve(&self, g: &mut Inner<T>) {
        let held = g.held();
        budget::relieve(budget::id_of(self), held, need(), |want| g.evict(want));
    }

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
        // A page the file does not take (none opened, or a write failed)
        // stays resident; the pages it took before can still be read back.
        // A page the file does not take because none opened stays resident;
        // a file that opened and refuses the write stops the run
        // (`spill::Spill::fail`).
        let at = match g.spill.as_mut() {
            Some(s) => {
                let r = s.append(&bytes);
                s.or_fail("take a page", r).0
            }
            None => HELD,
        };
        PAGES.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(bytes.len() as u64, Ordering::Relaxed);
        g.at.push(at);
        g.by_hash.entry(h).or_default().push(id);
        g.tick += 1;
        let tick = g.tick;
        g.resident.insert(id, (Arc::from(v), tick));
        budget::hold(PAGE_CELLS * T::BYTES);
        self.relieve(&mut g);
        id
    }

    /// Page `id`: from this thread's recent pages, or the store.
    #[inline]
    pub(super) fn page(&self, id: u32) -> Arc<[T]> {
        T::recent(|r| {
            if let Some(p) = r.get(id) {
                return Arc::clone(p);
            }
            let p = self.fetch(id);
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
            let p = self.fetch(id);
            let v = p[j];
            r.keep(id, p);
            v
        })
    }

    /// Page `id` from the store, under its lock, the budget relieved after.
    fn fetch(&self, id: u32) -> Arc<[T]> {
        let mut g = self.inner.lock().expect("pages");
        let p = g.page(id);
        self.relieve(&mut g);
        p
    }
}

macro_rules! paged {
    ($t:ty, $n:expr) => {
        impl Paged for $t {
            const BYTES: usize = $n;
            fn store() -> &'static Arc<Store<$t>> {
                static S: std::sync::OnceLock<Arc<Store<$t>>> = std::sync::OnceLock::new();
                S.get_or_init(Store::shared)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// **A page the file did not take is never given up**: a store with no
    /// file, under a budget of one byte, keeps every page it made and answers
    /// each as made, hash-consing a repeat; with a file, the same pages are
    /// given up past the budget and read back as made.
    #[test]
    fn a_page_is_given_up_only_when_the_file_holds_it() {
        let pages: Vec<Vec<u16>> = (0..5u16)
            .map(|k| {
                (0..PAGE_CELLS as u16)
                    .map(|i| i.wrapping_mul(k + 1))
                    .collect()
            })
            .collect();
        for file in [false, true] {
            let store: Arc<Store<u16>> = Store::shared();
            {
                let mut g = store.inner.lock().expect("pages");
                g.opened = true;
                g.spill = if file {
                    Spill::open("test-pages").ok()
                } else {
                    None
                };
            }
            let (ids, resident, back) = budget::with_budget(1, || {
                let loads = LOADS.load(Ordering::Relaxed);
                let ids: Vec<u32> = pages.iter().map(|p| store.intern(p)).collect();
                assert_eq!(store.intern(&pages[2]), ids[2], "a repeat is the page made");
                for (id, p) in ids.iter().zip(&pages) {
                    assert_eq!(&*store.fetch(*id), &p[..], "page {id} (file: {file})");
                }
                let resident = store.inner.lock().expect("pages").resident.len();
                (ids, resident, LOADS.load(Ordering::Relaxed) - loads)
            });
            assert_eq!(ids, vec![0, 1, 2, 3, 4]);
            if file {
                assert_eq!(
                    resident, 0,
                    "with a file, a budget of one byte holds nothing"
                );
                assert!(back > 0, "with a file, pages are read back");
            } else {
                assert_eq!(resident, 5, "without a file, every page stays");
            }
        }
    }
}
