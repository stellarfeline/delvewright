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
use crate::compiler::blockstate::{BlockMap, BlockState};
use crate::compiler::nav::Ambient;
use crate::compiler::plan::Plan;
use delvewright_dsl::blockshape::{Collision, collision_class};
use std::collections::BTreeMap;

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
    /// The rest census over the store: where a body can rest at most, per
    /// column, and the bounds it gives the lists and the portal edges.
    pub census: RestCensus,
}

/// **The rest census** (ADR-0037 §3): over the reach window, column by column,
/// how many cells a body can rest in at most — read from the store section by
/// section, by the one block classification every model reads
/// ([`collision_class`]) — and the bounds that follow for the structures that
/// grow with the reached cells.
///
/// A cell is a **rest candidate** when a body could rest in it under
/// `World::rest_kind`'s rules, counted from above: every cell holding a block a
/// body stands on (a full cube, a partial floor, a fence or wall top, a door or
/// trapdoor) puts one standing candidate in the cell over it; every climbable
/// cell is a holding candidate; every fluid cell, and every cell of the
/// horizon's sea band, is a water candidate. The count is an upper bound: a
/// support whose cell above is filled, or a wall two cells thick, counts rest
/// cells no body takes, and nothing a body rests in goes uncounted.
///
/// A node's edges are at most the launch set's landings and the cell moves
/// ([`crate::compiler::nav::edges_per_node_at_most`]), and no more than the rest
/// candidates within [`crate::compiler::nav::LAUNCH_REACH`] columns of it in
/// `x` and `z`, which is where every launch lands; a water node has at most
/// its six dives. Summed over a reach chunk, that bounds what its lists decode
/// to; summed over the nodes within reach of a chunk face, it bounds the
/// portal edges one closure joins, a water cell on the face line crossing it
/// at most once.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RestCensus {
    /// Columns of the window.
    pub columns: usize,
    /// Standing and holding candidates over the window.
    pub standing: usize,
    /// Water candidates over the window, the sea band included.
    pub water: usize,
    /// The most rest candidates one reach chunk holds: its nodes at most.
    pub chunk_nodes_at_most: usize,
    /// The most one reach chunk's lists decode to, by its candidates.
    pub lists_chunk_bound: usize,
    /// The most portal edges one closure joins.
    pub portal_edges_at_most: usize,
    /// How far a launch lands, in columns ([`crate::compiler::nav::LAUNCH_REACH`]).
    pub reach: i32,
}

/// What a block does for the census.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Rest {
    /// A body stands on it: the cell over it is a standing candidate.
    Support,
    /// A body holds it: the cell is a holding candidate.
    Climb,
    /// A body swims in it.
    Water,
    /// Nothing a body rests on or in.
    None,
}

fn rest_of(name: &str) -> Rest {
    match collision_class(name) {
        Collision::Air | Collision::Thin(_) => Rest::None,
        Collision::Fluid => Rest::Water,
        Collision::Climbable => Rest::Climb,
        Collision::PartialFloor(_)
        | Collision::FullCube
        | Collision::TallBarrier
        | Collision::OpensByHand => Rest::Support,
    }
}

impl RestCensus {
    /// The census of `store` over `window` (inclusive corners), under a sea
    /// whose ambient water fills `floor_top + 1 ..= level` when one is given,
    /// for reach chunks of `side` columns, launches landing within `reach`
    /// columns, and at most `edges` edges out of one node.
    pub fn of(
        store: &BlockMap,
        window: ([i32; 3], [i32; 3]),
        sea: Option<(i32, i32)>,
        side: i32,
        reach: i32,
        edges: usize,
    ) -> RestCensus {
        let (lo, hi) = window;
        let w = usize::try_from(hi[0] - lo[0] + 1).unwrap_or(0);
        let d = usize::try_from(hi[2] - lo[2] + 1).unwrap_or(0);
        if w == 0 || d == 0 {
            return RestCensus {
                reach,
                ..RestCensus::default()
            };
        }
        let at = |x: i32, z: i32| (x - lo[0]) as usize + (z - lo[2]) as usize * w;
        let band = sea.map_or(0u32, |(floor_top, level)| {
            let (a, b) = ((floor_top + 1).max(lo[1]), level.min(hi[1]));
            u32::try_from(b - a + 1).unwrap_or(0)
        });
        let mut standing = vec![0u32; w * d];
        let mut water = vec![band; w * d];
        // Every reading is a function of the block state alone, made once per
        // distinct state.
        let mut kinds: BTreeMap<BlockState, Rest> = BTreeMap::new();
        for (c, b) in store.in_box(lo, hi) {
            let k = *kinds.entry(*b).or_insert_with(|| rest_of(b.as_str()));
            match k {
                Rest::Support | Rest::Climb => standing[at(c[0], c[2])] += 1,
                Rest::Water => water[at(c[0], c[2])] += 1,
                Rest::None => {}
            }
        }
        // The rest candidates within `reach` columns of each column, by a
        // summed-area table.
        let mut sat = vec![0u64; (w + 1) * (d + 1)];
        for z in 0..d {
            for x in 0..w {
                let i = x + z * w;
                sat[(x + 1) + (z + 1) * (w + 1)] = u64::from(standing[i])
                    + u64::from(water[i])
                    + sat[x + (z + 1) * (w + 1)]
                    + sat[(x + 1) + z * (w + 1)]
                    - sat[x + z * (w + 1)];
            }
        }
        let r = usize::try_from(reach).unwrap_or(0);
        let near = |x: usize, z: usize| -> u64 {
            let (x0, x1) = (x.saturating_sub(r), (x + r + 1).min(w));
            let (z0, z1) = (z.saturating_sub(r), (z + r + 1).min(d));
            sat[x1 + z1 * (w + 1)] + sat[x0 + z0 * (w + 1)]
                - sat[x0 + z1 * (w + 1)]
                - sat[x1 + z0 * (w + 1)]
        };
        let mut chunks: BTreeMap<(i32, i32), (usize, usize)> = BTreeMap::new();
        let mut portal = 0usize;
        let (mut all_standing, mut all_water) = (0usize, 0usize);
        for z in 0..d {
            for x in 0..w {
                let i = x + z * w;
                let (s, wt) = (standing[i] as usize, water[i] as usize);
                all_standing += s;
                all_water += wt;
                let per_node = edges.min(usize::try_from(near(x, z)).unwrap_or(usize::MAX));
                let items = s * per_node + wt * 6;
                let (ax, az) = (lo[0] + x as i32, lo[2] + z as i32);
                let e = chunks
                    .entry((ax.div_euclid(side), az.div_euclid(side)))
                    .or_insert((0, 0));
                e.0 += s + wt;
                e.1 += items;
                let (ix, iz) = (ax.rem_euclid(side), az.rem_euclid(side));
                let near_face = |i: i32| i < reach || i >= side - reach;
                if near_face(ix) || near_face(iz) {
                    portal += s * per_node;
                }
                let on_line = |i: i32| usize::from(i == 0 || i == side - 1);
                portal += wt * (on_line(ix) + on_line(iz));
            }
        }
        let mut out = RestCensus {
            columns: w * d,
            standing: all_standing,
            water: all_water,
            reach,
            ..RestCensus::default()
        };
        for (nodes, items) in chunks.values() {
            out.chunk_nodes_at_most = out.chunk_nodes_at_most.max(*nodes);
            out.lists_chunk_bound = out
                .lists_chunk_bound
                .max(crate::compiler::nav::lists_bound_of(*nodes, *items));
        }
        out.portal_edges_at_most = portal;
        out
    }

    /// Rest states at most, over the window: every candidate.
    pub fn states_at_most(&self) -> usize {
        self.standing + self.water
    }
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
        let sea = match &ambient {
            Ambient::Ocean(s) => Some((s.floor_top, s.level)),
            Ambient::Void => None,
        };
        let census = window.map_or_else(RestCensus::default, |w| {
            RestCensus::of(
                &store.blocks,
                w,
                sea,
                side as i32,
                crate::compiler::nav::LAUNCH_REACH,
                edges_per_node,
            )
        });
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
            census,
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

    /// The most portal edges a closure carries: the nodes within a launch's
    /// reach of a chunk face, each with the edges its envelope allows, and
    /// never more than every face cell a node with every edge crossing.
    pub fn portal_edges_at_most(&self) -> usize {
        self.face_cells
            .saturating_mul(self.edges_per_node)
            .min(self.census.portal_edges_at_most)
    }

    /// The most one reach chunk's lists decode to: by its rest candidates and
    /// their envelopes, and never more than every cell a node with every edge.
    pub fn lists_chunk_at_most(&self) -> usize {
        self.lists_chunk_bound.min(self.census.lists_chunk_bound)
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
             face cell(s); rest census: {} standing and {} water candidate(s) over {} column(s), \
             at most {} node(s) in one chunk, {} edge(s) out of one node and every landing within \
             {} column(s); at most {} portal edge(s) in one closure; working set before any \
             walk: the stores' ring {} + the snapshot's pages {} + the lists' buffered pushes \
             {} = {} byte(s), the lists' largest chunk at most {} byte(s) by its candidates; \
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
            self.census.standing,
            self.census.water,
            self.census.columns,
            self.census.chunk_nodes_at_most,
            self.edges_per_node,
            self.census.reach,
            self.portal_edges_at_most(),
            self.store_ring,
            self.snapshot_need,
            self.lists_floor,
            self.working_set_floor(),
            self.lists_chunk_at_most(),
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
                "lists_largest_chunk_at_most": self.lists_chunk_at_most(),
            },
            "rest_census": {
                "columns": self.census.columns,
                "standing_candidates": self.census.standing,
                "water_candidates": self.census.water,
                "states_at_most": self.census.states_at_most(),
                "chunk_nodes_at_most": self.census.chunk_nodes_at_most,
                "launch_reach_columns": self.census.reach,
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
            census: RestCensus {
                columns: 118 * 218,
                standing: 500,
                water: 20_000,
                chunk_nodes_at_most: 4_000,
                lists_chunk_bound: 2_500,
                portal_edges_at_most: 7_000,
                reach: 5,
            },
        };
        assert_eq!(c.window_dims(), [118, 12, 218]);
        assert_eq!(c.window_cells(), 118 * 12 * 218);
        assert_eq!(c.reached_bitmap_bytes(), (118 * 12 * 218usize).div_ceil(8));
        assert_eq!(
            c.portal_edges_at_most(),
            7_000,
            "the census's bound, the tighter"
        );
        assert_eq!(
            c.lists_chunk_at_most(),
            2_500,
            "the census's bound, the tighter"
        );
        assert_eq!(c.census.states_at_most(), 20_500);
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

    /// **The census counts what a column walk counts**: a generated store of
    /// supports, climbables, fluids and thin blocks across chunk and section
    /// borders under a sea band, its census against a reference that walks
    /// every column and its neighbourhood cell by cell — the candidates, the
    /// chunk's most nodes, the lists' bound and the portal bound.
    #[test]
    fn the_census_counts_what_a_column_walk_counts() {
        let mut seed = 0x7a3f_19c2_5b8e_d401u64;
        let mut next = |n: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % n
        };
        let names = [
            "minecraft:stone",
            "minecraft:oak_fence",
            "minecraft:ladder[facing=north]",
            "minecraft:water",
            "minecraft:torch",
            "minecraft:oak_trapdoor[facing=north,half=bottom,open=false]",
            "minecraft:stone_slab[type=bottom]",
        ];
        for round in 0..8i32 {
            let mut store = BlockMap::new();
            let lo = [-37 - round, 50, -29];
            let hi = [lo[0] + 70 + 3 * round, 75, lo[2] + 66];
            for _ in 0..(400 + round * 300) {
                let c = [
                    lo[0] + next((hi[0] - lo[0] + 1) as u64) as i32,
                    lo[1] + next((hi[1] - lo[1] + 1) as u64) as i32,
                    lo[2] + next((hi[2] - lo[2] + 1) as u64) as i32,
                ];
                store.insert(c, BlockState::new(names[next(7) as usize]));
            }
            let sea = (round % 2 == 0).then_some((54, 62));
            let (side, reach, edges) = (32, 5, 1138usize);
            let got = RestCensus::of(&store, (lo, hi), sea, side, reach, edges);
            // The reference: every column, cell by cell.
            let band = sea.map_or(0, |(f, l)| (l.min(hi[1]) - (f + 1).max(lo[1]) + 1).max(0));
            let col = |x: i32, z: i32| -> (usize, usize) {
                let (mut s, mut w) = (0usize, band as usize);
                for y in lo[1]..=hi[1] {
                    match store.get(&[x, y, z]).map(|b| rest_of(b.as_str())) {
                        Some(Rest::Support | Rest::Climb) => s += 1,
                        Some(Rest::Water) => w += 1,
                        _ => {}
                    }
                }
                (s, w)
            };
            let (mut standing, mut water, mut portal) = (0, 0, 0);
            let mut chunks: BTreeMap<(i32, i32), (usize, usize)> = BTreeMap::new();
            for x in lo[0]..=hi[0] {
                for z in lo[2]..=hi[2] {
                    let (s, w) = col(x, z);
                    standing += s;
                    water += w;
                    let mut near = 0usize;
                    for dx in -reach..=reach {
                        for dz in -reach..=reach {
                            let (nx, nz) = (x + dx, z + dz);
                            if nx >= lo[0] && nx <= hi[0] && nz >= lo[2] && nz <= hi[2] {
                                let (a, b) = col(nx, nz);
                                near += a + b;
                            }
                        }
                    }
                    let per = edges.min(near);
                    let e = chunks
                        .entry((x.div_euclid(side), z.div_euclid(side)))
                        .or_insert((0, 0));
                    e.0 += s + w;
                    e.1 += s * per + 6 * w;
                    let (ix, iz) = (x.rem_euclid(side), z.rem_euclid(side));
                    if ix < reach || ix >= side - reach || iz < reach || iz >= side - reach {
                        portal += s * per;
                    }
                    portal += w
                        * (usize::from(ix == 0 || ix == side - 1)
                            + usize::from(iz == 0 || iz == side - 1));
                }
            }
            let nodes = chunks.values().map(|(n, _)| *n).max().unwrap_or(0);
            let lists = chunks
                .values()
                .map(|(n, i)| crate::compiler::nav::lists_bound_of(*n, *i))
                .max()
                .unwrap_or(0);
            assert_eq!(
                (
                    got.standing,
                    got.water,
                    got.chunk_nodes_at_most,
                    got.lists_chunk_bound,
                    got.portal_edges_at_most
                ),
                (standing, water, nodes, lists, portal),
                "round {round}"
            );
            assert_eq!(
                got.columns,
                ((hi[0] - lo[0] + 1) * (hi[2] - lo[2] + 1)) as usize
            );
            assert!(
                standing > 0 && water > 0 && portal > 0,
                "round {round} binds nothing"
            );
        }
    }
}
