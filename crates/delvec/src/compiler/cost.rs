//! **The engine states a build's cost before the whole-map stages start**
//! (ADR-0037 §3): one line, `resource cost:`, computed from the documents and
//! the assembled store alone — the site plan's region, the horizon, the judged
//! columns, the store's sections — and written as
//! `validation/resource-cost.json`. It holds what the engine computes, never
//! what it measures: a measured peak is the second instrument that cross-checks
//! it (`tools/maintenance/rss-sample.sh`, GNU `time -v`) and goes in commit
//! bodies and the record, so two builds of one campaign write one file
//! (ADR-0006). The hosting side meets the stated cost; the engine never refuses
//! a map for it (CLAUDE.md "Host hardware never caps a capability").
//!
//! What is stated: the working set every paged structure asks for before any
//! walk is measured (the stores' ring, the snapshot's pages for the window's
//! height, the lists' ring of buffered pushes), each summary's bound (the
//! reached bitmap, the portal edges over the window's chunk-face cells, the
//! leak floods' face labels, the configurations, the plain-sea summaries), and
//! the scratch files' size (the stores' arrays, and the lists' largest chunk at
//! most). Every bound is an upper bound a build cannot exceed; what a build
//! reached is printed as it ends (`reach budget:`, `reach lists:`,
//! `reach portals:`, `world store pages:`).

use crate::compiler::assembled::{Assembled, Judged};
use crate::compiler::nav::Ambient;
use crate::compiler::plan::Plan;

/// A build's computed cost; see the module docs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceCost {
    /// The site plan's region, inclusive corners, when one is declared.
    pub region: Option<([i64; 3], [i64; 3])>,
    /// The horizon's ambient, by name.
    pub horizon: &'static str,
    /// The judged columns' inclusive `[x, z]` corners, when the boundary cuts.
    pub judged: Option<([i32; 2], [i32; 2])>,
    /// The reach window at most: the judged columns (or the store's bounds)
    /// with the window's margins and rounding, inclusive corners.
    pub window: Option<([i32; 3], [i32; 3])>,
    /// Reach chunks across the window in x and z, at the chunk side.
    pub chunks: [usize; 2],
    pub chunk_side: usize,
    /// Cells on the window's chunk faces: one end of every portal edge.
    pub face_cells: usize,
    /// The most edges one node holds.
    pub edges_per_node: usize,
    /// The stores: sections and bytes of the whole and of the judged copy.
    pub store_sections: usize,
    pub store_bytes: usize,
    pub judged_sections: usize,
    pub judged_bytes: usize,
    /// The working set each paged structure states before any walk.
    pub store_ring: usize,
    pub snapshot_need: usize,
    pub lists_floor: usize,
    /// The most one reach chunk's lists decode to, by the window's height.
    pub lists_chunk_bound: usize,
    /// How many region writes the path carries.
    pub region_writes: usize,
}

/// The most distinct labels one 16 × 16 section face can carry: a
/// checkerboard, half its cells.
const FACE_LABELS_AT_MOST: usize = 128;

impl ResourceCost {
    /// The cost of building `plan` over `whole`, cut to `judged` when the
    /// boundary cuts it.
    pub fn of(plan: &Plan, whole: &Assembled, judged: Option<&(Assembled, Judged)>) -> Self {
        let ambient = Ambient::of_plan(plan);
        let region = plan
            .campaign
            .site_plan
            .as_ref()
            .map(|p| (p.content.region.min, p.content.region.max()));
        let cut = judged.map(|(_, j)| (j.lo, j.hi));
        let store = judged.map_or(whole, |(a, _)| a);
        let window = store.blocks.section_bounds().map(|(slo, shi)| {
            let (mut lo, mut hi) = (slo, shi);
            if let Some((jlo, jhi)) = cut {
                lo[0] = jlo[0] - 1;
                lo[2] = jlo[1] - 1;
                hi[0] = jhi[0] + 1;
                hi[2] = jhi[1] + 1;
            }
            let sea = match &ambient {
                Ambient::Ocean(s) => Some((s.floor_top, s.level)),
                Ambient::Void => None,
            };
            crate::compiler::nav::window_box(lo, hi, sea)
        });
        let side = crate::compiler::nav::chunk_side() as usize;
        let dims = window.map_or([0; 3], |(lo, hi)| {
            [0, 1, 2].map(|a| usize::try_from(hi[a] - lo[a] + 1).unwrap_or(0))
        });
        let chunks = [dims[0].div_ceil(side), dims[2].div_ceil(side)];
        let face_cells = (chunks[0].saturating_sub(1) * dims[2]
            + chunks[1].saturating_sub(1) * dims[0])
            * dims[1];
        let edges_per_node = crate::compiler::nav::edges_per_node_at_most();
        let (lists_floor, lists_chunk_bound) =
            crate::compiler::nav::lists_floor_and_bound(dims[1], edges_per_node);
        ResourceCost {
            region,
            horizon: ambient.name(),
            judged: cut,
            window,
            chunks,
            chunk_side: side,
            face_cells,
            edges_per_node,
            store_sections: whole.blocks.sections(),
            store_bytes: whole.blocks.bytes(),
            judged_sections: judged.map_or(0, |(a, _)| a.blocks.sections()),
            judged_bytes: judged.map_or(0, |(a, _)| a.blocks.bytes()),
            store_ring: crate::compiler::sections::floor(),
            snapshot_need: crate::compiler::nav::snapshot_need_for(dims[1]),
            lists_floor,
            lists_chunk_bound,
            region_writes: plan.region_events.writes(),
        }
    }

    /// The window's dimensions, zero without one.
    pub fn window_dims(&self) -> [usize; 3] {
        self.window.map_or([0; 3], |(lo, hi)| {
            [0, 1, 2].map(|a| usize::try_from(hi[a] - lo[a] + 1).unwrap_or(0))
        })
    }

    /// Cells of the window.
    pub fn window_cells(&self) -> usize {
        self.window_dims().iter().product()
    }

    /// The reached bitmap: one bit per window cell.
    pub fn reached_bitmap_bytes(&self) -> usize {
        self.window_cells().div_ceil(8)
    }

    /// The most portal edges a closure carries: every face cell a node with
    /// every edge crossing.
    pub fn portal_edges_at_most(&self) -> usize {
        self.face_cells.saturating_mul(self.edges_per_node)
    }

    /// The most face labels the leak floods' union-find holds: every section
    /// of the judged store, six faces, a checkerboard each.
    pub fn face_labels_at_most(&self) -> usize {
        let sections = if self.judged.is_some() {
            self.judged_sections
        } else {
            self.store_sections
        };
        sections * 6 * FACE_LABELS_AT_MOST
    }

    /// The most configurations the proofs judge: one more than the region
    /// writes.
    pub fn configurations_at_most(&self) -> usize {
        self.region_writes + 1
    }

    /// Plain-sea summaries at most: one per configuration under an ocean,
    /// none under a void.
    pub fn plain_sea_summaries_at_most(&self) -> usize {
        if self.horizon == "ocean" {
            self.configurations_at_most()
        } else {
            0
        }
    }

    /// The working set the paged structures state before any walk: the sum
    /// the budget starts at.
    pub fn working_set_floor(&self) -> usize {
        self.store_ring + self.snapshot_need + self.lists_floor
    }

    /// The stores' scratch file at most: every array of both stores given up.
    pub fn store_scratch_bytes(&self) -> usize {
        self.store_bytes + self.judged_bytes
    }

    /// The one line, printed before the whole-map stages start.
    pub fn line(&self) -> String {
        let region = match self.region {
            Some((lo, hi)) => format!(
                "region {} x {} x {} cell(s)",
                hi[0] - lo[0] + 1,
                hi[1] - lo[1] + 1,
                hi[2] - lo[2] + 1
            ),
            None => "no site plan region".to_string(),
        };
        let judged = match self.judged {
            Some((lo, hi)) => format!(
                "judged columns [{}, {}]..[{}, {}]",
                lo[0], lo[1], hi[0], hi[1]
            ),
            None => "every column judged".to_string(),
        };
        let d = self.window_dims();
        format!(
            "resource cost: {region} under a {} horizon; {judged}; reach window at most {} cell(s) \
             ({} x {} x {}), its reached bitmap {} byte(s), {} x {} chunk(s) of side {} with {} \
             face cell(s), at most {} portal edge(s) at {} edge(s) per node; working set before \
             any walk: the stores' ring {} + the snapshot's pages {} + the lists' buffered pushes \
             {} = {} byte(s), the lists' largest chunk at most {} byte(s) by the window's height; \
             summaries at most: {} face label(s), {} configuration(s), {} plain-sea summar(y/ies); \
             scratch: the stores' {} byte(s) over {} section(s) of the whole and {} of the judged \
             copy (ADR-0037 §3)",
            self.horizon,
            self.window_cells(),
            d[0],
            d[1],
            d[2],
            self.reached_bitmap_bytes(),
            self.chunks[0],
            self.chunks[1],
            self.chunk_side,
            self.face_cells,
            self.portal_edges_at_most(),
            self.edges_per_node,
            self.store_ring,
            self.snapshot_need,
            self.lists_floor,
            self.working_set_floor(),
            self.lists_chunk_bound,
            self.face_labels_at_most(),
            self.configurations_at_most(),
            self.plain_sea_summaries_at_most(),
            self.store_scratch_bytes(),
            self.store_sections,
            self.judged_sections,
        )
    }

    /// The same cost as `validation/resource-cost.json`.
    pub fn to_json(&self) -> serde_json::Value {
        let d = self.window_dims();
        serde_json::json!({
            "adr": "ADR-0037 §3",
            "computed_never_measured": true,
            "region": self.region.map(|(lo, hi)| serde_json::json!({"min": lo, "max": hi})),
            "horizon": self.horizon,
            "judged_columns": self.judged.map(|(lo, hi)| serde_json::json!({"min": lo, "max": hi})),
            "reach_window": self.window.map(|(lo, hi)| serde_json::json!({
                "min": lo, "max": hi, "dims": d, "cells": self.window_cells(),
                "chunk_side": self.chunk_side, "chunks": self.chunks,
            })),
            "working_set_bytes": {
                "stores_ring": self.store_ring,
                "snapshot_pages": self.snapshot_need,
                "lists_buffered_pushes": self.lists_floor,
                "total_before_any_walk": self.working_set_floor(),
                "lists_largest_chunk_at_most": self.lists_chunk_bound,
            },
            "summaries_at_most": {
                "reached_bitmap_bytes": self.reached_bitmap_bytes(),
                "portal_face_cells": self.face_cells,
                "portal_edges": self.portal_edges_at_most(),
                "edges_per_node": self.edges_per_node,
                "face_labels": self.face_labels_at_most(),
                "configurations": self.configurations_at_most(),
                "plain_sea_summaries": self.plain_sea_summaries_at_most(),
            },
            "scratch_bytes": {
                "stores": self.store_scratch_bytes(),
                "store_sections": self.store_sections,
                "store_bytes": self.store_bytes,
                "judged_sections": self.judged_sections,
                "judged_bytes": self.judged_bytes,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The bounds are arithmetic over the window** and the line and the
    /// ledger carry the same numbers.
    #[test]
    fn the_bounds_follow_the_window() {
        let c = ResourceCost {
            region: Some(([0, 0, 0], [99, 49, 199])),
            horizon: "ocean",
            judged: Some(([0, 0], [99, 199])),
            window: Some(([-9, 52, -9], [108, 63, 208])),
            chunks: [4, 7],
            chunk_side: 32,
            face_cells: (3 * 218 + 6 * 118) * 12,
            edges_per_node: 1112,
            store_sections: 10,
            store_bytes: 40_960,
            judged_sections: 8,
            judged_bytes: 32_768,
            store_ring: 221_184,
            snapshot_need: 1_000,
            lists_floor: 2_000,
            lists_chunk_bound: 3_000,
            region_writes: 4,
        };
        assert_eq!(c.window_dims(), [118, 12, 218]);
        assert_eq!(c.window_cells(), 118 * 12 * 218);
        assert_eq!(c.reached_bitmap_bytes(), (118 * 12 * 218usize).div_ceil(8));
        assert_eq!(c.portal_edges_at_most(), c.face_cells * 1112);
        assert_eq!(c.face_labels_at_most(), 8 * 6 * 128);
        assert_eq!(c.configurations_at_most(), 5);
        assert_eq!(c.plain_sea_summaries_at_most(), 5);
        assert_eq!(c.working_set_floor(), 224_184);
        assert_eq!(c.store_scratch_bytes(), 73_728);
        let line = c.line();
        let json = c.to_json();
        for (word, n) in [
            ("reached bitmap", c.reached_bitmap_bytes()),
            ("portal edge(s)", c.portal_edges_at_most()),
            ("face label(s)", c.face_labels_at_most()),
        ] {
            assert!(
                line.contains(&n.to_string()),
                "{word} {n} missing from {line}"
            );
        }
        assert_eq!(
            json["summaries_at_most"]["portal_edges"],
            c.portal_edges_at_most()
        );
        assert_eq!(json["working_set_bytes"]["total_before_any_walk"], 224_184);
        assert_eq!(json["computed_never_measured"], true);
    }
}
