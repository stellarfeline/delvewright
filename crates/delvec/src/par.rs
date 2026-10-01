//! Ordered parallel map: the one way `delvec` spreads work over cores.
//!
//! [`map`] runs a function over a slice on several threads and returns the
//! results **in the slice's order**, whatever order the threads finished in.
//! Each result is computed from its own item alone; nothing a worker computes
//! reaches another worker or depends on which thread ran it. A caller then
//! folds the results sequentially, in that fixed order, so its output is the
//! output of the sequential loop it replaced (ADR-0006), at any thread count.
//!
//! **Thread count.** [`threads`]: the `DELVEC_THREADS` environment variable
//! when it is a positive integer, otherwise the host's available parallelism
//! (`std::thread::available_parallelism`), otherwise 1. `DELVEC_THREADS=1`
//! runs every map on the calling thread. The count changes how long a build
//! takes and never what it emits.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The number of threads [`map`] uses; see the module docs.
pub fn threads() -> usize {
    static THREADS: OnceLock<usize> = OnceLock::new();
    *THREADS.get_or_init(|| {
        std::env::var("DELVEC_THREADS")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .filter(|&n| n > 0)
            .or_else(|| std::thread::available_parallelism().ok().map(|n| n.get()))
            .unwrap_or(1)
    })
}

/// `items.iter().map(f).collect()`, computed on up to [`threads`] threads.
///
/// Items are handed out one at a time from a shared counter, so a slow item
/// does not hold up a fixed share of the rest. A panic in `f` propagates to
/// the caller once every thread has stopped.
pub fn map<T, R, F>(items: &[T], f: F) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> R + Sync,
{
    let workers = threads().min(items.len());
    if workers <= 1 {
        return items.iter().map(f).collect();
    }
    let next = AtomicUsize::new(0);
    let mut parts: Vec<Vec<(usize, R)>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut done = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(i) else {
                            break;
                        };
                        done.push((i, f(item)));
                    }
                    done
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_else(|e| std::panic::resume_unwind(e)))
            .collect()
    });
    let mut slots: Vec<Option<R>> = (0..items.len()).map(|_| None).collect();
    for (i, r) in parts.drain(..).flatten() {
        slots[i] = Some(r);
    }
    slots
        .into_iter()
        .map(|r| r.expect("every item is computed exactly once"))
        .collect()
}

/// Compute `f` over `items` on up to [`threads`] threads and hand each result
/// to `sink` on the calling thread, **in item order**, stopping at the first
/// `Err` the sink returns.
///
/// Unlike [`map`], at most a few results per thread are held at once: the
/// items are taken in windows, each window computed in parallel and drained
/// into `sink` before the next starts. This is the form for results that are
/// large, such as a decoded template's cells.
pub fn try_for_each_ordered<T, R, E, F, S>(items: &[T], f: F, mut sink: S) -> Result<(), E>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> R + Sync,
    S: FnMut(R) -> Result<(), E>,
{
    let window = (threads() * 2).max(1);
    for chunk in items.chunks(window) {
        for r in map(chunk, &f) {
            sink(r)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    /// The results come back in item order however the work was scheduled:
    /// items finish out of order on purpose (later items are cheaper).
    #[test]
    fn results_come_back_in_item_order() {
        let items: Vec<u64> = (0..200).collect();
        let got = super::map(&items, |&i| {
            let mut acc = i;
            for k in 0..(200 - i) * 500 {
                acc = acc.wrapping_mul(6364136223846793005).wrapping_add(k);
            }
            (i, acc)
        });
        let want: Vec<(u64, u64)> = items
            .iter()
            .map(|&i| {
                let mut acc = i;
                for k in 0..(200 - i) * 500 {
                    acc = acc.wrapping_mul(6364136223846793005).wrapping_add(k);
                }
                (i, acc)
            })
            .collect();
        assert_eq!(got, want);
    }

    /// The sink sees every result in item order and nothing after its first
    /// refusal.
    #[test]
    fn the_sink_sees_results_in_order_and_stops_at_its_refusal() {
        let items: Vec<usize> = (0..97).collect();
        let mut seen = Vec::new();
        let r = super::try_for_each_ordered(
            &items,
            |&i| i * 3,
            |v| {
                if v == 150 {
                    return Err(v);
                }
                seen.push(v);
                Ok(())
            },
        );
        assert_eq!(r, Err(150));
        assert_eq!(seen, (0..50).map(|i| i * 3).collect::<Vec<_>>());
    }
}
