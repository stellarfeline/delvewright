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
//! reached and plain-sea bitmaps, the portal edges over the window's chunk-face
//! cells, the leak floods' face labels, the configurations), and the scratch
//! files' size (the stores' arrays; the reach graph's snapshot pages, node
//! tables and one closure's lists per configuration; and the lists' largest
//! chunk at most). Every bound is an upper bound a build cannot exceed; what a
//! build reached is printed as it ends (`reach budget:`, `reach lists:`,
//! `reach portals:`, `reach scratch:`, `world store pages:`).

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
    /// The sections a build write covers, at every height
    /// (`Reached::sections_written`): what the write sequence judge's stores
    /// lie over.
    pub written_sections: usize,
    /// The sections the path's region writes cover: what a configuration's
    /// own arrays lie over.
    pub region_sections: usize,
    /// The launches flown out of one node, the plan's declared jumps and its
    /// killing volumes: what a bank variant's record holds beside the cells
    /// its arcs read.
    pub launches: usize,
    pub jumps: usize,
    pub lethal_volumes: usize,
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
/// trapdoor) whose cell above holds no such block puts one standing candidate
/// in that cell — a surface; every climbable cell is a holding candidate; every
/// fluid cell, and every cell of the horizon's sea band that holds no ground
/// (a full cube, a partial floor, a fence or wall), is a water candidate — the
/// terrain's ground in the band is no water (`World::ambient_water`). The
/// count is an upper bound: a surface under a torch, a slab under a
/// carpet, or a ledge a body cannot fit on counts a rest cell no body takes,
/// and nothing a body rests in goes uncounted.
///
/// A node's edges are at most the launch set's landings and the cell moves
/// ([`crate::compiler::nav::edges_per_node_at_most`]), and no more than the rest
/// candidates within [`crate::compiler::nav::LAUNCH_REACH`] columns of it in
/// `x` and `z`, which is where every launch lands; a water node has at most
/// its six dives. Summed over a reach chunk, that bounds what its lists decode
/// to; summed over the nodes within reach of a chunk face — each with the
/// candidates in the strips across the faces it can reach, which is where a
/// crossing edge lands — it bounds the portal edges one closure joins, a water
/// cell on the face line crossing it at most once.
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
    /// The most edges over the window: every candidate's envelope, summed —
    /// what a graph's tables and a closure's lists hold at most.
    pub items_at_most: usize,
    /// The most edges one reach chunk's candidates hold.
    pub chunk_items_at_most: usize,
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

/// Whether a block is ground: a cell of the sea's band holding one is no
/// water (`World::ambient_water` refuses an occupied cell), so it leaves the
/// band's count. A door or trapdoor is not: the nav world may still read the
/// cell as water, and the census stays an upper bound.
fn ground(name: &str) -> bool {
    matches!(
        collision_class(name),
        Collision::PartialFloor(_) | Collision::FullCube | Collision::TallBarrier
    )
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
        let in_band = |y: i32| sea.is_some_and(|(floor_top, level)| y > floor_top && y <= level);
        let mut standing = vec![0u32; w * d];
        let mut water = vec![band; w * d];
        // The supports of each column, one bit per height, so a surface — a
        // support with no support over it — is counted once the column is read.
        let words = usize::try_from(hi[1] - lo[1] + 1).unwrap_or(0).div_ceil(64);
        let mut supports = vec![0u64; w * d * words];
        // Every reading is a function of the block state alone, made once per
        // distinct state.
        let mut kinds: BTreeMap<BlockState, (Rest, bool)> = BTreeMap::new();
        for (c, b) in store.in_box(lo, hi) {
            let (k, ground) = *kinds
                .entry(*b)
                .or_insert_with(|| (rest_of(b.as_str()), ground(b.as_str())));
            match k {
                Rest::Support => {
                    let y = (c[1] - lo[1]) as usize;
                    supports[at(c[0], c[2]) * words + y / 64] |= 1 << (y % 64);
                    // Ground in the sea's band is no water.
                    if ground && in_band(c[1]) {
                        water[at(c[0], c[2])] -= 1;
                    }
                }
                Rest::Climb => standing[at(c[0], c[2])] += 1,
                // A fluid cell in the band is the band's own candidate.
                Rest::Water if !in_band(c[1]) => water[at(c[0], c[2])] += 1,
                Rest::Water | Rest::None => {}
            }
        }
        for col in 0..w * d {
            let bits = &supports[col * words..(col + 1) * words];
            for (i, word) in bits.iter().enumerate() {
                // The support one height up: the next bit, the next word's
                // lowest bit carried in at the top.
                let above = (word >> 1) | bits.get(i + 1).map_or(0, |n| n << 63);
                standing[col] += (word & !above).count_ones();
            }
        }
        drop(supports);
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
        // Rest candidates in the inclusive column box `x0..=x1`, `z0..=z1`,
        // clipped to the window (an empty box when it lies outside).
        let boxed = |x0: i64, x1: i64, z0: i64, z1: i64| -> u64 {
            let (x0, x1) = (x0.max(0) as usize, (x1 + 1).clamp(0, w as i64) as usize);
            let (z0, z1) = (z0.max(0) as usize, (z1 + 1).clamp(0, d as i64) as usize);
            if x0 >= x1 || z0 >= z1 {
                return 0;
            }
            sat[x1 + z1 * (w + 1)] + sat[x0 + z0 * (w + 1)]
                - sat[x0 + z1 * (w + 1)]
                - sat[x1 + z0 * (w + 1)]
        };
        let near = |x: usize, z: usize| -> u64 {
            let (x, z, r) = (x as i64, z as i64, r as i64);
            boxed(x - r, x + r, z - r, z + r)
        };
        // The candidates across the chunk faces a column's launches reach:
        // for a column `i` cells into its chunk on an axis, the strip past the
        // low face is the `reach - i` columns before the chunk, past the high
        // face the `reach - (side - 1 - i)` columns after it, each `2 reach + 1`
        // wide on the other axis.
        let across = |x: usize, z: usize, ix: i32, iz: i32| -> u64 {
            let (x, z, r, side) = (x as i64, z as i64, r as i64, i64::from(side));
            let (ix, iz) = (i64::from(ix), i64::from(iz));
            let mut n = 0u64;
            if ix < r {
                n += boxed(x - r, x - ix - 1, z - r, z + r);
            }
            if side - 1 - ix < r {
                n += boxed(x + (side - ix), x + r, z - r, z + r);
            }
            if iz < r {
                n += boxed(x - r, x + r, z - r, z - iz - 1);
            }
            if side - 1 - iz < r {
                n += boxed(x - r, x + r, z + (side - iz), z + r);
            }
            n
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
                let crossing = usize::try_from(across(x, z, ix, iz)).unwrap_or(usize::MAX);
                portal += s * per_node.min(crossing);
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
            out.chunk_items_at_most = out.chunk_items_at_most.max(*items);
            out.items_at_most = out.items_at_most.saturating_add(*items);
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

/// The write sequence judge's stores over the written sections at most
/// (`compiler::attached`): the cells the load leaves, kept; and one replay's
/// world, its read faces and a function's fall cells.
const JUDGE_STORES: usize = 4;

/// How many sections the inclusive boxes `regions` cover between them.
pub fn region_sections(regions: impl IntoIterator<Item = ([i32; 3], [i32; 3])>) -> usize {
    use crate::compiler::sections::SIDE;
    let mut sections: std::collections::BTreeSet<[i32; 3]> = std::collections::BTreeSet::new();
    for (a, b) in regions {
        let lo = [0, 1, 2].map(|i| a[i].min(b[i]).div_euclid(SIDE));
        let hi = [0, 1, 2].map(|i| a[i].max(b[i]).div_euclid(SIDE));
        for sx in lo[0]..=hi[0] {
            for sy in lo[1]..=hi[1] {
                for sz in lo[2]..=hi[2] {
                    sections.insert([sx, sy, sz]);
                }
            }
        }
    }
    sections.len()
}

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
            written_sections: whole.reached.sections_written(),
            region_sections: region_sections(plan.region_events.iter().map(|e| e.region)),
            launches: crate::compiler::nav::launches_per_node(),
            jumps: plan.jumps.len(),
            lethal_volumes: plan.lethal_volumes.len(),
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

    /// The reached states' bitmaps: one bit per window cell for the states
    /// reached, and one for the plain-sea states among them (ADR-0037 §2).
    pub fn reached_bitmap_bytes(&self) -> usize {
        2 * self.window_cells().div_ceil(8)
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

    /// The working set the paged structures state before any walk: the sum
    /// the budget starts at.
    pub fn working_set_floor(&self) -> usize {
        self.store_ring + self.snapshot_need + self.lists_floor
    }

    /// **The stores' scratch file at most** (ADR-0037 §1, §3): the file holds
    /// one region of [`crate::compiler::sections::REGION`] bytes per array
    /// alive at once over every store, so its size is bounded by the most
    /// arrays the build holds at once — [`Self::arrays_at_most`] — and never
    /// by the give-ups a run made.
    pub fn store_scratch_bytes(&self) -> usize {
        self.arrays_at_most()
            .saturating_mul(crate::compiler::sections::REGION)
    }

    /// **The reach graph's scratch files for one configuration, at most**
    /// (ADR-0037 §3, `nav::scratch_bound_of`): the snapshot's pages over the
    /// window, once; the two node tables, an entry per rest candidate and an
    /// edge per candidate's envelope; one closure's two lists, a record per
    /// edge, with one chunk's records once more for a chunk rewritten whole
    /// beside its pieces; and the bank's variants, the most a node keeps for
    /// every candidate, each holding every cell its arcs could read.
    pub fn reach_scratch_per_configuration(&self) -> crate::compiler::nav::ScratchBound {
        crate::compiler::nav::scratch_bound_of(
            self.census.states_at_most(),
            self.census.items_at_most,
            self.census.chunk_items_at_most,
            self.window_cells(),
            self.window_dims()[1],
            self.launches,
            self.jumps,
            self.lethal_volumes,
        )
    }

    /// **The reach graph's scratch files at most**: one configuration's
    /// ([`Self::reach_scratch_per_configuration`]) for each configuration
    /// the proofs judge. What the terms do not count by name is covered by
    /// their slack, held so by test on generated cases and on the gallery:
    /// the premise variants and counterfactuals the proofs build beside the
    /// configurations (worlds of their own, whose pages, tables, closures and
    /// variants the snapshot's, tables', lists' and bank's terms cover), a
    /// second closure taken over one configuration from other roots (the
    /// lists' term) and a node's second table entry under another set of
    /// launch quadrants (the tables' term). The `reach scratch:` line states
    /// each file as the run ends.
    pub fn reach_scratch_bytes(&self) -> usize {
        self.reach_scratch_per_configuration()
            .total()
            .saturating_mul(self.configurations_at_most())
    }

    /// **The most index arrays alive at once over every store**, from the
    /// stores the stages lay over the written world: the whole and the judged
    /// copy, held throughout; the write sequence judge's stores — the cells
    /// the load leaves, kept for the runtime arm, and one replay's world, its
    /// read faces and a function's fall cells, each over the sections a build
    /// write covers at most (a replay grown to the whole load holds them
    /// all); and each configuration's own arrays, which lie over the sections
    /// its region writes cover. A copy of a store shares every array it does
    /// not write (ADR-0037 §1), so a configuration costs its writes and a
    /// light model or review world over a shared picture costs nothing here.
    /// Not counted, declared: the premise variants and counterfactuals the
    /// proofs build beside the configurations (`reach graphs:`), each owning
    /// at most the region sections it writes — a residue of the same kind the
    /// configurations figure declares.
    pub fn arrays_at_most(&self) -> usize {
        self.store_sections
            + self.judged_sections
            + JUDGE_STORES * self.written_sections
            + self.configurations_at_most() * self.region_sections
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
        let reach = self.reach_scratch_per_configuration();
        format!(
            "resource cost: {region} under a {} horizon; {judged}; reach window at most {} cell(s) \
             ({} x {} x {}), its reached and plain-sea bitmaps {} byte(s), {} x {} chunk(s) of \
             side {} with {} \
             face cell(s); rest census: {} standing and {} water candidate(s) over {} column(s), \
             at most {} node(s) in one chunk, {} edge(s) out of one node and every landing within \
             {} column(s); at most {} portal edge(s) in one closure; working set before any \
             walk: the stores' ring {} + the snapshot's pages {} + the lists' buffered pushes \
             {} = {} byte(s), the lists' largest chunk at most {} byte(s) by its candidates; \
             summaries at most: {} face label(s), {} configuration(s); \
             scratch: the stores' {} byte(s), one region of {} per array alive at once, at most {} \
             — {} section(s) of the whole, {} of the judged copy, {} written section(s) under the \
             write sequence judge's {} stores, and {} region section(s) per configuration; the \
             reach graph's {} byte(s) over the configuration(s), {} per configuration — the \
             snapshot's pages {}, the node tables {} over {} edge(s) at most, one closure's lists \
             {}, the bank's variants {} ({} launch(es), {} declared jump(s), {} killing \
             volume(s) per variant) — the variants' worlds, a second closure and a second table \
             entry covered by those terms' slack (ADR-0037 §3)",
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
            self.store_scratch_bytes(),
            crate::compiler::sections::REGION,
            self.arrays_at_most(),
            self.store_sections,
            self.judged_sections,
            self.written_sections,
            JUDGE_STORES,
            self.region_sections,
            self.reach_scratch_bytes(),
            reach.total(),
            reach.snapshot,
            reach.tables,
            self.census.items_at_most,
            reach.lists,
            reach.bank,
            self.launches,
            self.jumps,
            self.lethal_volumes,
        )
    }

    /// The same cost as `validation/resource-cost.json`.
    pub fn to_json(&self) -> serde_json::Value {
        let d = self.window_dims();
        let reach = self.reach_scratch_per_configuration();
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
                "edges_at_most": self.census.items_at_most,
                "chunk_edges_at_most": self.census.chunk_items_at_most,
                "launch_reach_columns": self.census.reach,
            },
            "summaries_at_most": {
                "reached_bitmap_bytes": self.reached_bitmap_bytes(),
                "portal_face_cells": self.face_cells,
                "portal_edges": self.portal_edges_at_most(),
                "edges_per_node": self.edges_per_node,
                "face_labels": self.face_labels_at_most(),
                "configurations": self.configurations_at_most(),
            },
            "scratch_bytes": {
                "stores": self.store_scratch_bytes(),
                "region_bytes": crate::compiler::sections::REGION,
                "arrays_at_most": self.arrays_at_most(),
                "store_sections": self.store_sections,
                "store_bytes": self.store_bytes,
                "judged_sections": self.judged_sections,
                "judged_bytes": self.judged_bytes,
                "written_sections": self.written_sections,
                "judge_stores": JUDGE_STORES,
                "region_sections": self.region_sections,
                "reach_graph": self.reach_scratch_bytes(),
                "reach_graph_per_configuration": {
                    "total": reach.total(),
                    "snapshot_pages": reach.snapshot,
                    "node_tables": reach.tables,
                    "one_closures_lists": reach.lists,
                    "bank_variants": reach.bank,
                    "launches_per_node": self.launches,
                    "declared_jumps": self.jumps,
                    "killing_volumes": self.lethal_volumes,
                },
                "reach_graph_covered_by_slack": [
                    "the premise variants' and counterfactuals' worlds (every term)",
                    "a second closure over one configuration (the lists)",
                    "a node's second table entry under other launch quadrants (the tables)",
                ],
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
            written_sections: 12,
            region_sections: 3,
            launches: 100,
            jumps: 3,
            lethal_volumes: 2,
            census: RestCensus {
                columns: 118 * 218,
                standing: 500,
                water: 20_000,
                chunk_nodes_at_most: 4_000,
                lists_chunk_bound: 2_500,
                items_at_most: 120_000,
                chunk_items_at_most: 30_000,
                portal_edges_at_most: 7_000,
                reach: 5,
            },
        };
        assert_eq!(c.window_dims(), [118, 12, 218]);
        assert_eq!(c.window_cells(), 118 * 12 * 218);
        assert_eq!(
            c.reached_bitmap_bytes(),
            2 * (118 * 12 * 218usize).div_ceil(8)
        );
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
        assert_eq!(c.working_set_floor(), 224_184);
        assert_eq!(
            c.arrays_at_most(),
            10 + 8 + 4 * 12 + 5 * 3,
            "the whole, the judged copy, the judge's four stores over the written sections, \
             a configuration's region sections each"
        );
        assert_eq!(
            c.store_scratch_bytes(),
            81 * crate::compiler::sections::REGION
        );
        let reach = c.reach_scratch_per_configuration();
        assert_eq!(
            reach,
            crate::compiler::nav::scratch_bound_of(
                20_500,
                120_000,
                30_000,
                118 * 12 * 218,
                12,
                100,
                3,
                2
            ),
            "the census's candidates and edges over the window's cells and height, the \
             launches, the plan's jumps and volumes"
        );
        // A variant: the counts, the head box and the tallies (57), a
        // dependency per cell of the envelope box (17 x 17 columns at every
        // height, 16 bytes each), the jumps (12 each), the volumes (24 each),
        // a run per launch (76 and one edge of 17 each) and the node's edges
        // (53); eight per candidate, and the node's edges once more per edge.
        let variant = 57 + 17 * 17 * 12 * 16 + 3 * 12 + 2 * 24 + 100 * (76 + 17) + 53;
        assert_eq!(
            (
                reach.snapshot,
                reach.tables,
                reach.lists,
                reach.bank,
                reach.total()
            ),
            (
                (118 * 12 * 218usize).div_ceil(4096) * 4096 * 6,
                // The cell moves in one table are at most the edges (120,000
                // under 20,500 x 32).
                2 * 20_500 * 55 + (120_000 + 120_000) * 17,
                (120_000 + 30_000) * (29 + 41),
                20_500 * 8 * variant + 8 * 120_000 * 17,
                reach.snapshot + reach.tables + reach.lists + reach.bank
            ),
            "every page of the two arrays; two tables of an entry per candidate, the cell moves \
             in one and every edge in the other; a record per edge in each of two lists and one \
             chunk's once more; eight variants per candidate over the envelope box"
        );
        assert_eq!(
            c.reach_scratch_bytes(),
            5 * reach.total(),
            "one configuration's scratch for each configuration"
        );
        assert_eq!(
            region_sections([
                ([0, 0, 0], [15, 15, 15]),
                ([15, 0, 15], [16, 0, 16]),
                ([3, 3, 3], [4, 4, 4])
            ]),
            4,
            "two boxes sharing a section, one straddling three more"
        );
        let line = c.line();
        let json = c.to_json();
        for (word, n) in [
            ("reached and plain-sea bitmaps", c.reached_bitmap_bytes()),
            ("portal edge(s)", c.portal_edges_at_most()),
            ("face label(s)", c.face_labels_at_most()),
            ("scratch: the stores'", c.store_scratch_bytes()),
            ("the reach graph's", c.reach_scratch_bytes()),
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
        assert_eq!(json["scratch_bytes"]["stores"], c.store_scratch_bytes());
        assert_eq!(json["scratch_bytes"]["arrays_at_most"], 81);
        assert_eq!(
            json["scratch_bytes"]["reach_graph"],
            c.reach_scratch_bytes()
        );
        assert_eq!(
            json["scratch_bytes"]["reach_graph_per_configuration"]["total"],
            reach.total()
        );
        assert_eq!(json["rest_census"]["edges_at_most"], 120_000);
        assert_eq!(
            json["scratch_bytes"]["reach_graph_per_configuration"]["bank_variants"],
            reach.bank
        );
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
            let hi = [lo[0] + 70 + 3 * round, 50 + 60 + 9 * round, lo[2] + 66];
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
            let in_band = |y: i32| sea.is_some_and(|(f, l)| y > f && y <= l);
            let col = |x: i32, z: i32| -> (usize, usize) {
                let (mut s, mut w) = (0usize, band as usize);
                let kind = |y: i32| store.get(&[x, y, z]).map(|b| rest_of(b.as_str()));
                let is_ground = |y: i32| store.get(&[x, y, z]).is_some_and(|b| ground(b.as_str()));
                for y in lo[1]..=hi[1] {
                    match kind(y) {
                        // A surface: a support with no support over it.
                        Some(Rest::Support) if kind(y + 1) != Some(Rest::Support) => s += 1,
                        Some(Rest::Climb) => s += 1,
                        // A fluid cell outside the band; inside it, the band's own.
                        Some(Rest::Water) if !in_band(y) => w += 1,
                        _ => {}
                    }
                    // Ground in the band is no water.
                    if in_band(y) && is_ground(y) {
                        w -= 1;
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
                    // Candidates in the neighbourhood that lie in another chunk
                    // across a face this column's launches reach, strip by strip
                    // (a corner column counts its corner twice, as the census does).
                    let mut crossing = 0usize;
                    for dx in -reach..=reach {
                        for dz in -reach..=reach {
                            let (nx, nz) = (x + dx, z + dz);
                            if nx < lo[0] || nx > hi[0] || nz < lo[2] || nz > hi[2] {
                                continue;
                            }
                            let (a, b) = col(nx, nz);
                            let past_x = nx.div_euclid(side) != x.div_euclid(side);
                            let past_z = nz.div_euclid(side) != z.div_euclid(side);
                            crossing += (a + b) * (usize::from(past_x) + usize::from(past_z));
                        }
                    }
                    portal += s * per.min(crossing);
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
            let items: usize = chunks.values().map(|(_, i)| *i).sum();
            let chunk_items = chunks.values().map(|(_, i)| *i).max().unwrap_or(0);
            assert_eq!(
                (
                    got.standing,
                    got.water,
                    got.chunk_nodes_at_most,
                    got.lists_chunk_bound,
                    got.items_at_most,
                    got.chunk_items_at_most,
                    got.portal_edges_at_most
                ),
                (standing, water, nodes, lists, items, chunk_items, portal),
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
