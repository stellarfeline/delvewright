//! **The one resident budget** every structure the reach graph pages is held
//! to (ADR-0037 §1): the closures' edge lists by chunk ([`super::chunks`]),
//! the window snapshot's pages ([`super::pages`]) and whatever pages next.
//!
//! Each paged structure **states the working set its walk needs** as a
//! [`Need`], in bytes, measured from what it has had to hold — never set by a
//! host — and the budget is the sum of the needs stated. Every structure
//! counts the bytes it holds in memory against one process-wide total
//! ([`resident`]); past the budget, the structure that just grew gives up its
//! least recently used holdings first, then every other registered structure
//! is asked ([`relieve`]), until the total stands at three quarters of the
//! budget. What a structure answers never depends on what is resident: a
//! holding given up is read back from its scratch file when next asked.
//!
//! A test may put a budget of its own in force on its thread
//! ([`with_budget`]): it then binds each structure that thread (and the
//! workers the graph spawns from it, [`super::World`]'s parallel maps carry
//! it) grows, **each alone**, to that many bytes, so one test paging its
//! graph to nothing moves no other test's holdings. A structure every graph
//! in the process shares (the snapshot's store) still gives up the pages
//! others hold under it; they are read back when next asked.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock, Weak};

/// **A paged structure's stated working set**, in bytes: what its walk needs
/// resident, with a floor below which no measurement takes it.
pub(super) struct Need {
    bytes: AtomicUsize,
    floor: usize,
    name: &'static str,
    listed: AtomicBool,
}

impl Need {
    pub(super) const fn new(name: &'static str, floor: usize) -> Need {
        Need {
            bytes: AtomicUsize::new(0),
            floor,
            name,
            listed: AtomicBool::new(false),
        }
    }

    /// State the need as `bytes` when that is more than stated so far.
    pub(super) fn raise(&'static self, bytes: usize) {
        self.list();
        if self.bytes.fetch_max(bytes, Ordering::Relaxed) < bytes {
            let total = needs()
                .lock()
                .expect("needs")
                .iter()
                .map(|n| n.bytes())
                .sum();
            TOTAL.store(total, Ordering::Relaxed);
        }
    }

    /// The need as stated, at least its floor.
    pub(super) fn bytes(&self) -> usize {
        self.bytes.load(Ordering::Relaxed).max(self.floor)
    }

    fn list(&'static self) {
        if !self.listed.swap(true, Ordering::Relaxed) {
            let mut n = needs().lock().expect("needs");
            n.push(self);
            TOTAL.store(n.iter().map(|n| n.bytes()).sum(), Ordering::Relaxed);
        }
    }
}

/// The sum of the needs stated, kept as they are raised so the budget is one
/// load on the paths that push and read.
static TOTAL: AtomicUsize = AtomicUsize::new(0);

fn needs() -> &'static Mutex<Vec<&'static Need>> {
    static N: OnceLock<Mutex<Vec<&'static Need>>> = OnceLock::new();
    N.get_or_init(|| Mutex::new(Vec::new()))
}

/// Every need stated so far, by name, in the order first stated.
pub(super) fn needs_stated() -> Vec<(&'static str, usize)> {
    needs()
        .lock()
        .expect("needs")
        .iter()
        .map(|n| (n.name, n.bytes()))
        .collect()
}

/// **The resident budget in force on this thread**, in bytes: the sum of the
/// needs stated, or what a test put in force.
pub(super) fn budget() -> usize {
    match override_here() {
        0 => TOTAL.load(Ordering::Relaxed),
        b => b,
    }
}

thread_local! {
    static OVERRIDE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The budget a test put in force on this thread; `0` for none.
pub(super) fn override_here() -> usize {
    OVERRIDE.with(std::cell::Cell::get)
}

/// Run `f` with the resident budget at `bytes` on this thread (`0`: none),
/// binding each structure this thread grows, alone, to it.
pub(super) fn with_budget<R>(bytes: usize, f: impl FnOnce() -> R) -> R {
    let was = OVERRIDE.with(|b| b.replace(bytes));
    let out = f();
    OVERRIDE.with(|b| b.set(was));
    out
}

/// Bytes every registered structure holds in memory now.
static RESIDENT: AtomicUsize = AtomicUsize::new(0);
/// The use clock every structure orders its holdings by.
static TICK: AtomicU64 = AtomicU64::new(0);

/// The bytes every registered structure holds in memory now.
pub(super) fn resident() -> usize {
    RESIDENT.load(Ordering::Relaxed)
}

/// Count `bytes` as held.
pub(super) fn hold(bytes: usize) {
    RESIDENT.fetch_add(bytes, Ordering::Relaxed);
}

/// Count `bytes` as given up.
pub(super) fn release(bytes: usize) {
    RESIDENT.fetch_sub(bytes, Ordering::Relaxed);
}

/// The next tick of the use clock.
pub(super) fn tick() -> u64 {
    TICK.fetch_add(1, Ordering::Relaxed)
}

/// **A structure that can give memory up**: asked for `want` bytes, it gives
/// up its least recently used holdings until that many are freed or none is
/// left to give, without waiting on a lock another thread holds, and answers
/// the bytes freed.
pub(super) trait Pageable: Send + Sync {
    fn give_up(&self, want: usize) -> usize;
}

/// A registered structure: its identity, which [`relieve`] skips when asking
/// the others.
pub(super) type Id = usize;

type Registry = Mutex<Vec<(Id, Weak<dyn Pageable>)>>;

fn registry() -> &'static Registry {
    static R: OnceLock<Registry> = OnceLock::new();
    R.get_or_init(|| Mutex::new(Vec::new()))
}

/// Register a structure under `id` (its address, [`id_of`]); the registry
/// forgets a structure once it is dropped.
pub(super) fn join(id: Id, w: Weak<dyn Pageable>) {
    let mut r = registry().lock().expect("registry");
    r.retain(|(_, w)| w.strong_count() > 0);
    r.push((id, w));
}

/// The identity a structure registers under: its address.
pub(super) fn id_of<T: ?Sized>(t: &T) -> Id {
    t as *const T as *const () as usize
}

/// **Bring the resident bytes under the budget**, after structure `me`,
/// holding `held` bytes, grew: `own(want)` gives up `me`'s own holdings (the
/// caller holds `me`'s lock, so it is asked directly), then every other
/// registered structure is asked, until the total stands at three quarters
/// of the budget. Under a test's budget, `me` alone is held to it.
pub(super) fn relieve(me: Id, held: usize, mut own: impl FnMut(usize) -> usize) {
    let b = override_here();
    if b != 0 {
        if held > b {
            own(held - b);
        }
        return;
    }
    let b = budget();
    let over = |b: usize| resident().saturating_sub(b);
    if over(b) == 0 {
        return;
    }
    let target = b / 4 * 3;
    own(over(target));
    if over(target) == 0 {
        return;
    }
    let others: Vec<std::sync::Arc<dyn Pageable>> = registry()
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
        o.give_up(want);
    }
}

/// How many registered structures are alive.
#[cfg(test)]
pub(super) fn registered() -> usize {
    registry()
        .lock()
        .expect("registry")
        .iter()
        .filter(|(_, w)| w.strong_count() > 0)
        .count()
}
