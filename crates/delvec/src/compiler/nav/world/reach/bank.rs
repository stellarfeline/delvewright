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
//! What a world query reads beyond a cell's neighbourhood — the declared
//! jumps, the climb holds — is part of the bank's own key, so two worlds that
//! differ in it never share a bank.
//!
//! A node no variant holds for is flown again — but not all of it. Its
//! launches are taken in [`GROUPS`] runs of template arcs, in their order, and
//! each run is written down with the box of the cells it read and what it did
//! (its edges as first named, its leaving the world and first death, its
//! counts, the box it grew the node's read box by). Against the variant that
//! differs from this world in the fewest of its cells, a run whose box holds
//! none of those cells did the same here and is taken as written; the rest
//! are flown. Runs merge in their order as the launches do, first naming
//! first, so the node is the node one pass over every launch gives.
//!
//! The bank is an accelerator, never a premise: what no variant answers is
//! flown, and a variant that holds answers what flying would.

use super::{Code, Death, Dense, EdgeKind, NodeEdges};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// An inclusive cell box.
pub(super) type Box3 = ([i32; 3], [i32; 3]);

fn union(a: Option<Box3>, b: Option<Box3>) -> Option<Box3> {
    match (a, b) {
        (Some(a), Some(b)) => Some((
            [a.0[0].min(b.0[0]), a.0[1].min(b.0[1]), a.0[2].min(b.0[2])],
            [a.1[0].max(b.1[0]), a.1[1].max(b.1[1]), a.1[2].max(b.1[2])],
        )),
        (a, None) => a,
        (None, b) => b,
    }
}

fn inside(c: [i32; 3], b: &Box3) -> bool {
    (0..3).all(|i| b.0[i] <= c[i] && c[i] <= b.1[i])
}

/// How many runs a node's launches are taken in.
pub(super) const GROUPS: usize = 32;

/// The window cells one node's expansion read, as a bitmap over the window,
/// and the box of the cells the current run read.
pub(super) struct Track<'a> {
    dense: &'a Dense,
    bits: Vec<Cell<u64>>,
    run: Cell<Option<Box3>>,
}

impl<'a> Track<'a> {
    pub(super) fn new(dense: &'a Dense) -> Track<'a> {
        Track {
            dense,
            bits: (0..dense.cells.len().div_ceil(64))
                .map(|_| Cell::new(0))
                .collect(),
            run: Cell::new(None),
        }
    }

    /// Note the window cell `c`, at index `i`.
    #[inline]
    pub(super) fn note(&self, i: usize, c: [i32; 3]) {
        let w = &self.bits[i / 64];
        w.set(w.get() | 1 << (i % 64));
        self.extend((c, c));
    }

    /// Grow the current run's box by `b`.
    #[inline]
    pub(super) fn extend(&self, b: Box3) {
        self.run.set(union(self.run.get(), Some(b)));
    }

    /// Note every window cell of the inclusive box `lo..=hi`.
    pub(super) fn mark_box(&self, lo: [i32; 3], hi: [i32; 3]) {
        for x in lo[0]..=hi[0] {
            for z in lo[2]..=hi[2] {
                for y in lo[1]..=hi[1] {
                    if let Some(i) = self.dense.index([x, y, z]) {
                        let w = &self.bits[i / 64];
                        w.set(w.get() | 1 << (i % 64));
                    }
                }
            }
        }
        self.extend((lo, hi));
    }

    /// Start a run: its box is empty.
    pub(super) fn begin_run(&self) {
        self.run.set(None);
    }

    /// End a run: its box.
    pub(super) fn end_run(&self) -> Option<Box3> {
        self.run.take()
    }

    /// Set the current box aside to measure what one computation reads.
    pub(super) fn begin_scope(&self) -> Option<Box3> {
        self.run.take()
    }

    /// The box the computation since [`Track::begin_scope`] read; the run's
    /// box takes it in.
    pub(super) fn end_scope(&self, saved: Option<Box3>) -> Option<Box3> {
        let inner = self.run.get();
        self.run.set(union(saved, inner));
        inner
    }

    /// The cells noted, each with its snapshot code and raw membership.
    fn deps(&self, extra: &[u16]) -> Vec<(u32, Code, u16)> {
        let mut out = Vec::new();
        for (w, word) in self.bits.iter().enumerate() {
            let mut b = word.get();
            while b != 0 {
                let i = w * 64 + b.trailing_zeros() as usize;
                b &= b - 1;
                out.push((i as u32, self.dense.cells[i], extra[i]));
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
    bbox: Option<Box3>,
    pub(super) summary: Summary,
}

/// The killing volumes near a box, as the bank compares them.
pub(super) type Volumes = Vec<([i32; 3], [i32; 3])>;

/// One recorded expansion of a node.
struct Variant {
    deps: Vec<(u32, Code, u16)>,
    volumes: Volumes,
    runs: Vec<Run>,
    edges: Arc<NodeEdges>,
}

/// One expansion as the bank follows it: the cells it reads, the variant it
/// may take runs from, and the runs it takes.
pub(super) struct Expansion<'a> {
    pub(super) track: Track<'a>,
    prior: Option<Prior>,
    bank: &'a Bank,
    runs: RefCell<Vec<Run>>,
}

/// The recorded variant an expansion takes runs from, and the window cells
/// this world holds otherwise than it.
struct Prior {
    variant: Arc<Variant>,
    diff: Vec<[i32; 3]>,
}

impl Expansion<'_> {
    /// The recorded run `g` of `n`, if it holds here; its cells are noted as
    /// read and its box made the current run's.
    pub(super) fn reuse(&self, g: usize, n: usize) -> Option<Summary> {
        let p = self.prior.as_ref()?;
        if p.variant.runs.len() != n {
            return None;
        }
        let run = &p.variant.runs[g];
        if let Some(b) = run.bbox {
            if p.diff.iter().any(|d| inside(*d, &b)) {
                return None;
            }
            for &(i, _, _) in &p.variant.deps {
                let c = self.track.dense.coords(i as usize);
                if inside(c, &b) {
                    self.track.note(i as usize, c);
                }
            }
            self.track.extend(b);
        }
        self.bank
            .reused
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Some(run.summary.clone())
    }

    /// Write down run `g`, which ended with box `bbox`.
    pub(super) fn ran(&self, bbox: Option<Box3>, summary: Summary) {
        self.runs.borrow_mut().push(Run { bbox, summary });
    }
}

/// The bank of one window, ambient, launch set and non-local world facts.
#[derive(Default)]
pub(super) struct Bank {
    nodes: Mutex<HashMap<super::FlownKey, Vec<Arc<Variant>>>>,
    /// Runs taken as written, for the agreement test's binding.
    reused: std::sync::atomic::AtomicU64,
}

/// How many variants of one node the bank keeps: a bound on its memory, never
/// on what a build answers.
const VARIANTS: usize = 8;

impl Bank {
    /// The edges a recorded expansion of `key` gives a world whose snapshot is
    /// `dense` with raw membership `extra`, if one holds; `volumes` says which
    /// killing volumes lie near a read box.
    pub(super) fn find(
        &self,
        key: &super::FlownKey,
        dense: &Dense,
        extra: &[u16],
        volumes: &dyn Fn(Box3) -> Volumes,
    ) -> Option<Arc<NodeEdges>> {
        let variants = self.nodes.lock().expect("bank").get(key)?.clone();
        variants.iter().find_map(|v| {
            let held = v
                .deps
                .iter()
                .all(|&(i, code, x)| dense.cells[i as usize] == code && extra[i as usize] == x);
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
        extra: &[u16],
        node: &[Box3],
        volumes: &dyn Fn(Box3) -> Volumes,
    ) -> Expansion<'a> {
        let variants = self
            .nodes
            .lock()
            .expect("bank")
            .get(key)
            .cloned()
            .unwrap_or_default();
        let mut best: Option<Prior> = None;
        for v in variants {
            let diff: Vec<[i32; 3]> = v
                .deps
                .iter()
                .filter(|&&(i, code, x)| dense.cells[i as usize] != code || extra[i as usize] != x)
                .map(|&(i, _, _)| dense.coords(i as usize))
                .collect();
            if diff.iter().any(|d| node.iter().any(|b| inside(*d, b)))
                || v.edges.read.is_none_or(|b| volumes(b) != v.volumes)
            {
                continue;
            }
            if best.as_ref().is_none_or(|b| diff.len() < b.diff.len()) {
                best = Some(Prior { variant: v, diff });
            }
        }
        Expansion {
            track: Track::new(dense),
            prior: best,
            bank: self,
            runs: RefCell::new(Vec::new()),
        }
    }

    /// Record an expansion of `key`.
    pub(super) fn keep(
        &self,
        key: super::FlownKey,
        expansion: Expansion<'_>,
        extra: &[u16],
        volumes: Volumes,
        edges: &Arc<NodeEdges>,
    ) {
        let deps = expansion.track.deps(extra);
        let runs = expansion.runs.into_inner();
        let mut nodes = self.nodes.lock().expect("bank");
        let vs = nodes.entry(key).or_default();
        if vs.len() < VARIANTS {
            vs.push(Arc::new(Variant {
                deps,
                volumes,
                runs,
                edges: Arc::clone(edges),
            }));
        }
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

/// The bank shared by every graph of `key`.
pub(super) fn bank(key: String) -> Arc<Bank> {
    static BANKS: std::sync::OnceLock<Mutex<HashMap<String, Arc<Bank>>>> =
        std::sync::OnceLock::new();
    let banks = BANKS.get_or_init(|| Mutex::new(HashMap::new()));
    Arc::clone(banks.lock().expect("banks").entry(key).or_default())
}
