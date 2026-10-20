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
//! The bank is an accelerator, never a premise: a node no variant holds for is
//! flown, and a variant that holds answers what flying would.

use super::{Code, Dense, NodeEdges};
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// The window cells one node's expansion read, as a bitmap over the window.
pub(super) struct Track<'a> {
    dense: &'a Dense,
    bits: Vec<Cell<u64>>,
}

impl<'a> Track<'a> {
    pub(super) fn new(dense: &'a Dense) -> Track<'a> {
        Track {
            dense,
            bits: (0..dense.cells.len().div_ceil(64))
                .map(|_| Cell::new(0))
                .collect(),
        }
    }

    /// Note the window cell at index `i`.
    #[inline]
    pub(super) fn mark_index(&self, i: usize) {
        let w = &self.bits[i / 64];
        w.set(w.get() | 1 << (i % 64));
    }

    /// Note every window cell of the inclusive box `lo..=hi`.
    pub(super) fn mark_box(&self, lo: [i32; 3], hi: [i32; 3]) {
        for x in lo[0]..=hi[0] {
            for z in lo[2]..=hi[2] {
                for y in lo[1]..=hi[1] {
                    if let Some(i) = self.dense.index([x, y, z]) {
                        self.mark_index(i);
                    }
                }
            }
        }
    }

    /// The cells noted, each with its snapshot code and raw membership.
    fn deps(self, extra: &[u16]) -> Vec<(u32, Code, u16)> {
        let mut out = Vec::new();
        for (w, word) in self.bits.into_iter().enumerate() {
            let mut b = word.into_inner();
            while b != 0 {
                let i = w * 64 + b.trailing_zeros() as usize;
                b &= b - 1;
                out.push((i as u32, self.dense.cells[i], extra[i]));
            }
        }
        out
    }
}

/// The killing volumes near a box, as the bank compares them.
pub(super) type Volumes = Vec<([i32; 3], [i32; 3])>;

/// One recorded expansion of a node.
struct Variant {
    deps: Vec<(u32, Code, u16)>,
    volumes: Volumes,
    edges: Arc<NodeEdges>,
}

/// The bank of one window, ambient, launch set and non-local world facts.
#[derive(Default)]
pub(super) struct Bank {
    nodes: Mutex<HashMap<super::FlownKey, Vec<Arc<Variant>>>>,
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
        volumes: &dyn Fn(([i32; 3], [i32; 3])) -> Volumes,
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

    /// Record an expansion of `key` that read what `track` noted.
    pub(super) fn keep(
        &self,
        key: super::FlownKey,
        track: Track<'_>,
        extra: &[u16],
        volumes: Volumes,
        edges: &Arc<NodeEdges>,
    ) {
        let deps = track.deps(extra);
        let mut nodes = self.nodes.lock().expect("bank");
        let vs = nodes.entry(key).or_default();
        if vs.len() < VARIANTS {
            vs.push(Arc::new(Variant {
                deps,
                volumes,
                edges: Arc::clone(edges),
            }));
        }
    }
}

/// The bank shared by every graph of `key`.
pub(super) fn bank(key: String) -> Arc<Bank> {
    static BANKS: std::sync::OnceLock<Mutex<HashMap<String, Arc<Bank>>>> =
        std::sync::OnceLock::new();
    let banks = BANKS.get_or_init(|| Mutex::new(HashMap::new()));
    Arc::clone(banks.lock().expect("banks").entry(key).or_default())
}
