//! **A place's claim, and who owns a cell** (spec-0098 §2).
//!
//! A place owns everything a body can see of it from outside — the ground
//! under its plot, its floor course, its play space, the one-cell ring its
//! walls may stand in and, roofed, its lid and the roof zone the plan declares —
//! minus the ring's **fixed ground**, which is the whole's terrain continued to
//! the plot's edge, and minus every cell the ownership rule awards to a
//! neighbour. This module is the one derivation of both halves: [`Site::claims`]
//! says what a place would write if nothing stood beside it, [`Site::owner`]
//! says who writes a cell, and [`crate::detailplan::Frame::of`] is built on
//! nothing else.
//!
//! Every reader — the blockout's stand-ins and holes, the handout,
//! `DW0827`/`DW0843`/`DW0844`/`DW0987`/`DW0990` — calls [`Site::owner`] or a
//! function of it. Two of them deciding "whose cell is this" independently is
//! how a builder and its observer come to agree about a world neither
//! describes.

use std::collections::{BTreeMap, BTreeSet};

use crate::envelope::Campaign;
use crate::ids::NodeId;

use super::{Ground, PlacedBox, PlacedSeam};

/// An inclusive world AABB, `(lo, hi)`.
pub type Aabb = ([i64; 3], [i64; 3]);

/// One group of `DW0827`: the places contesting, and the cells no rule awards them.
pub type Contest = (Vec<NodeId>, Vec<[i64; 3]>);

/// Who writes a cell (spec-0098 §2, rules 0–4).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Owner {
    /// The named place — its piece once bound, its stand-in while not.
    Place(NodeId),
    /// The ring's fixed ground (rule 0): the whole's terrain, written by no
    /// piece.
    Ground,
    /// Nobody (rule 4): what a volume declares there, else the site's fill.
    Nobody,
    /// Two or more places claim the cell and no rule awards it (rule 3d) — a
    /// plan `DW0827` refuses. Named so that a refused plan still answers.
    Contested(Vec<NodeId>),
}

impl Owner {
    /// The spelling the handout and the refusals use.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Owner::Place(n) => format!("`{n}`"),
            Owner::Ground => "the whole's fixed ground".to_string(),
            Owner::Nobody => "nobody (the site's fill)".to_string(),
            Owner::Contested(ns) => format!(
                "no one — contested by {}",
                ns.iter()
                    .map(|n| format!("`{n}`"))
                    .collect::<Vec<_>>()
                    .join(" and ")
            ),
        }
    }
}

fn inside(cell: [i64; 3], (lo, hi): Aabb) -> bool {
    (0..3).all(|i| cell[i] >= lo[i] && cell[i] <= hi[i])
}

fn intersect(a: Aabb, b: Aabb) -> Option<Aabb> {
    let lo = [a.0[0].max(b.0[0]), a.0[1].max(b.0[1]), a.0[2].max(b.0[2])];
    let hi = [a.1[0].min(b.1[0]), a.1[1].min(b.1[1]), a.1[2].min(b.1[2])];
    (0..3).all(|i| lo[i] <= hi[i]).then_some((lo, hi))
}

fn cells(a: Aabb) -> impl Iterator<Item = [i64; 3]> {
    let (lo, hi) = a;
    (lo[0]..=hi[0]).flat_map(move |x| {
        (lo[1]..=hi[1]).flat_map(move |y| (lo[2]..=hi[2]).map(move |z| [x, y, z]))
    })
}

impl PlacedBox {
    /// The top course of the play space.
    #[must_use]
    pub fn top(&self) -> i64 {
        self.floor + i64::from(self.clearance) - 1
    }

    /// The highest course of the shell: the lid on a roofed place, the top of
    /// the play space on an open one.
    #[must_use]
    pub fn shell_top(&self) -> i64 {
        if self.open {
            self.top()
        } else {
            self.top() + 1
        }
    }

    /// The roof zone the plan declares, before any neighbour clips its eaves:
    /// the shell footprint grown by `eaves` on every horizontal side, from the
    /// ceiling course up `courses` courses. `None` when no roof is declared or
    /// the place is open.
    #[must_use]
    pub fn roof_zone(&self) -> Option<Aabb> {
        let roof = self.roof?;
        if self.open {
            return None;
        }
        let e = i64::from(roof.eaves);
        let lid = self.top() + 1;
        Some((
            [self.foot[0] - 1 - e, lid, self.foot[2] - 1 - e],
            [
                self.foot[1] + 1 + e,
                lid + i64::from(roof.courses),
                self.foot[3] + 1 + e,
            ],
        ))
    }

    /// The highest course this place could claim.
    #[must_use]
    pub fn claim_top(&self) -> i64 {
        self.roof_zone().map_or(self.shell_top(), |(_, hi)| hi[1])
    }

    /// The floor course's world `y`.
    #[must_use]
    pub fn floor_course_y(&self) -> i64 {
        self.floor - 1
    }

    /// True when `(x, z)` is one of the ring's columns: inside the shell
    /// footprint and outside the play space's.
    #[must_use]
    pub fn is_ring_column(&self, x: i64, z: i64) -> bool {
        let [x0, x1, z0, z1] = self.foot;
        let in_shell = x >= x0 - 1 && x <= x1 + 1 && z >= z0 - 1 && z <= z1 + 1;
        let in_foot = x >= x0 && x <= x1 && z >= z0 && z <= z1;
        in_shell && !in_foot
    }
}

/// The whole plan, resolved for ownership: every place, every seam, the site's
/// ground, and each place's claim bottom.
#[derive(Debug, Clone)]
pub struct Site<'a> {
    /// The plan's places.
    pub boxes: &'a [PlacedBox],
    /// The plan's connections.
    pub seams: &'a [PlacedSeam],
    /// The site's fill.
    pub ground: &'a Ground,
    bottoms: Vec<i64>,
}

impl<'a> Site<'a> {
    /// Resolve the claim bottoms of `boxes` over `ground`.
    ///
    /// A claim reaches down to the lower of its floor course and the lowest
    /// terrain under its footprint and ring, so the ring's fixed ground is in
    /// it (spec-0098 §2) — but never into a place stacked under it: the bottom
    /// stops one course above the highest claim of any place whose claim lies
    /// wholly below this one's floor and under its shell footprint, so the plane
    /// between two stacked places is the upper's floor course (rule 3a) and not
    /// a column of ground through the lower.
    #[must_use]
    pub fn new(boxes: &'a [PlacedBox], seams: &'a [PlacedSeam], ground: &'a Ground) -> Site<'a> {
        let bottoms = boxes
            .iter()
            .map(|b| {
                let [x0, x1, z0, z1] = b.foot;
                let mut bottom = b.floor_course_y();
                if ground.is_open() {
                    for x in x0 - 1..=x1 + 1 {
                        for z in z0 - 1..=z1 + 1 {
                            if let Some(t) = ground.top(x, z) {
                                bottom = bottom.min(t);
                            }
                        }
                    }
                }
                for q in boxes {
                    if q.node == b.node {
                        continue;
                    }
                    let overlaps = q.foot[0] - 1 <= x1 + 1
                        && q.foot[1] + 1 >= x0 - 1
                        && q.foot[2] - 1 <= z1 + 1
                        && q.foot[3] + 1 >= z0 - 1;
                    if overlaps && q.claim_top() < b.floor {
                        bottom = bottom.max(q.claim_top() + 1);
                    }
                }
                bottom.min(b.floor_course_y())
            })
            .collect();
        Site {
            boxes,
            seams,
            ground,
            bottoms,
        }
    }

    /// The index of `node`'s box.
    #[must_use]
    pub fn index_of(&self, node: &NodeId) -> Option<usize> {
        self.boxes.iter().position(|b| &b.node == node)
    }

    /// Place `i`'s claim bottom.
    #[must_use]
    pub fn bottom(&self, i: usize) -> i64 {
        self.bottoms[i]
    }

    /// Place `i`'s shell: the ground under its plot from its claim bottom, its
    /// floor course, its play space and its ring, and on a roofed place its lid.
    #[must_use]
    pub fn shell(&self, i: usize) -> Aabb {
        let b = &self.boxes[i];
        (
            [b.foot[0] - 1, self.bottoms[i], b.foot[2] - 1],
            [b.foot[1] + 1, b.shell_top(), b.foot[3] + 1],
        )
    }

    /// Everything place `i` might claim, as one box — what a reader iterates.
    #[must_use]
    pub fn claim_bounds(&self, i: usize) -> Aabb {
        let (mut lo, mut hi) = self.shell(i);
        if let Some((zlo, zhi)) = self.boxes[i].roof_zone() {
            for a in 0..3 {
                lo[a] = lo[a].min(zlo[a]);
                hi[a] = hi[a].max(zhi[a]);
            }
        }
        (lo, hi)
    }

    /// The ground height `G` of place `i`'s ring column `(x, z)` (spec-0098
    /// §2 rule 0): the terrain's ground — on an `open` site the terrain's
    /// surface `y`, on a `solid` site the floor course — except where a
    /// vertical seam of this place crosses the column at grade: a seam whose
    /// sill a body steps onto from that ground (sill − 1 at most one course
    /// above it, or below it) levels the column to the sill minus one, flat
    /// across the opening's width. A seam aloft — a bridge's deck, a door
    /// high in a wall — fixes no earth under it: the column between the
    /// terrain and the sill is the owner's (§14, a correction).
    #[must_use]
    pub fn ground_height(&self, i: usize, x: i64, z: i64) -> i64 {
        let b = &self.boxes[i];
        let terrain = self.terrain_height(i, x, z);
        for s in self.seams {
            if s.normal_axis == 1 || (s.a != b.node && s.b != b.node) {
                continue;
            }
            let along = if s.normal_axis == 0 { z } else { x };
            let across = if s.normal_axis == 0 { x } else { z };
            let other = if s.normal_axis == 0 { 2 } else { 0 };
            if across == s.plane
                && along >= s.opening.0[other]
                && along <= s.opening.1[other]
                && s.opening.0[1] - 1 <= terrain + 1
            {
                return s.opening.0[1] - 1;
            }
        }
        terrain
    }

    /// The terrain's ground under place `i`'s ring column `(x, z)`: the
    /// surface `y` on an `open` site, the floor course on a `solid` one.
    #[must_use]
    pub fn terrain_height(&self, i: usize, x: i64, z: i64) -> i64 {
        if self.ground.is_open()
            && let Some(t) = self.ground.top(x, z)
        {
            return t;
        }
        self.boxes[i].floor_course_y()
    }

    /// True when `cell` is fixed ground of place `i`: a ring cell of its shell
    /// at or under the column's ground height.
    #[must_use]
    pub fn is_fixed(&self, i: usize, cell: [i64; 3]) -> bool {
        inside(cell, self.shell(i))
            && self.boxes[i].is_ring_column(cell[0], cell[2])
            && cell[1] <= self.ground_height(i, cell[0], cell[2])
    }

    /// True when `cell` is in place `i`'s claim: in its shell, or in its roof
    /// zone — except an eaves cell standing in another place's shell (its play
    /// space, its walls, its lid), where the eave stops (spec-0098 §3).
    #[must_use]
    pub fn claims(&self, i: usize, cell: [i64; 3]) -> bool {
        if inside(cell, self.shell(i)) {
            return true;
        }
        let b = &self.boxes[i];
        let Some(zone) = b.roof_zone() else {
            return false;
        };
        if !inside(cell, zone) {
            return false;
        }
        let over_shell = cell[0] >= b.foot[0] - 1
            && cell[0] <= b.foot[1] + 1
            && cell[2] >= b.foot[2] - 1
            && cell[2] <= b.foot[3] + 1;
        // An eave stops where a neighbour's own shell begins — its play space,
        // its walls, its lid — as a real eave stops at the wall it meets.
        over_shell || !(0..self.boxes.len()).any(|k| k != i && inside(cell, self.shell(k)))
    }

    /// The places whose claim bounds meet `bounds` — the only places a cell
    /// inside `bounds` can be claimed by.
    #[must_use]
    pub fn near(&self, bounds: Aabb) -> Vec<usize> {
        (0..self.boxes.len())
            .filter(|i| intersect(self.claim_bounds(*i), bounds).is_some())
            .collect()
    }

    /// **Who writes `cell`** — spec-0098 §2's rule, in its order, over every
    /// place.
    #[must_use]
    pub fn owner(&self, cell: [i64; 3]) -> Owner {
        let all: Vec<usize> = (0..self.boxes.len()).collect();
        self.owner_among(cell, &all)
    }

    /// [`Site::owner`] over `cand` only — every place whose claim could reach
    /// `cell` must be in it ([`Site::near`]).
    ///
    /// 0. a ring cell at or under its ground height is the whole's fixed ground;
    /// 1. a cell inside a place's play space is that place's;
    /// 2. a cell exactly one claim covers is that place's;
    /// 3. a cell several claims cover: in exactly one of their floor courses,
    ///    that place's (a stacked plane is the upper's floor); else, exactly one
    ///    of them roofed, that one's (the facade onto an open place); else,
    ///    the seams the plan allocates between two of them across a plane
    ///    through the cell agreeing, pair by pair, about which is `a`, the `a`
    ///    of the first of them in plan order (a designed connection is drawn by
    ///    its first-named side); else contested — `DW0827`;
    /// 4. a cell no claim covers is nobody's.
    #[must_use]
    pub fn owner_among(&self, cell: [i64; 3], cand: &[usize]) -> Owner {
        if cand.iter().any(|&i| self.is_fixed(i, cell)) {
            return Owner::Ground;
        }
        if let Some(&i) = cand.iter().find(|&&i| inside(cell, self.boxes[i].space())) {
            return Owner::Place(self.boxes[i].node.clone());
        }
        let claimants: Vec<usize> = cand
            .iter()
            .copied()
            .filter(|&i| self.claims(i, cell))
            .collect();
        match claimants.len() {
            0 => return Owner::Nobody,
            1 => return Owner::Place(self.boxes[claimants[0]].node.clone()),
            _ => {}
        }
        // 3a — a floor course.
        let floors: Vec<usize> = claimants
            .iter()
            .copied()
            .filter(|&i| cell[1] == self.boxes[i].floor_course_y())
            .collect();
        if floors.len() == 1 {
            return Owner::Place(self.boxes[floors[0]].node.clone());
        }
        // 3b — exactly one roofed.
        let roofed: Vec<usize> = claimants
            .iter()
            .copied()
            .filter(|&i| !self.boxes[i].open)
            .collect();
        if roofed.len() == 1 {
            return Owner::Place(self.boxes[roofed[0]].node.clone());
        }
        // 3c — the connection's `a`. The seams the plan allocates between two
        // of the claimants across a plane through this cell: where the seams
        // between one pair disagree about which is `a`, the plan is refused
        // (rule 3d); where every one names the same `a`, the cell is that
        // place's — spec-0098 §2's rule as written for two places. At a corner
        // column where connections led by different places meet (a T-junction
        // of three places), the cell is the `a` of the first of them in the
        // plan's own seam order — the designer's order decides, never the
        // engine (recorded in spec-0098's departures).
        let names_of: Vec<&NodeId> = claimants.iter().map(|&i| &self.boxes[i].node).collect();
        let across: Vec<&PlacedSeam> = self
            .seams
            .iter()
            .filter(|s| {
                names_of.contains(&&s.a)
                    && names_of.contains(&&s.b)
                    && cell[s.normal_axis] == s.plane
            })
            .collect();
        let pair_disagrees = across.iter().any(|s| {
            across.iter().any(|t| {
                let same_pair = (s.a == t.a && s.b == t.b) || (s.a == t.b && s.b == t.a);
                same_pair && s.a != t.a
            })
        });
        if let Some(first) = across.first()
            && !pair_disagrees
        {
            return Owner::Place(first.a.clone());
        }
        let mut names: Vec<NodeId> = claimants
            .iter()
            .map(|&i| self.boxes[i].node.clone())
            .collect();
        names.sort();
        Owner::Contested(names)
    }

    /// Every cell of place `i`'s claim, in a fixed order.
    #[must_use]
    pub fn claim_cells(&self, i: usize) -> Vec<[i64; 3]> {
        cells(self.claim_bounds(i))
            .filter(|c| self.claims(i, *c))
            .collect()
    }

    /// Every fixed ground cell of place `i`, with its column's ground height,
    /// in a fixed order — what the terrain pass writes and the handout hands.
    #[must_use]
    pub fn fixed_cells(&self, i: usize) -> Vec<([i64; 3], i64)> {
        let b = &self.boxes[i];
        let (lo, hi) = self.shell(i);
        let mut out = Vec::new();
        for x in lo[0]..=hi[0] {
            for z in lo[2]..=hi[2] {
                if !b.is_ring_column(x, z) {
                    continue;
                }
                let g = self.ground_height(i, x, z);
                for y in lo[1]..=g.min(hi[1]) {
                    out.push(([x, y, z], g));
                }
            }
        }
        out
    }

    /// The cells several places claim that no rule awards (rule 3d), grouped
    /// by the places contesting them, in plan order — what `DW0827` refuses —
    /// and how many shared cells the rule did award, for the binding.
    #[must_use]
    pub fn contests(&self) -> (Vec<Contest>, usize) {
        let mut by: BTreeMap<Vec<NodeId>, BTreeSet<[i64; 3]>> = BTreeMap::new();
        let mut awarded: BTreeSet<[i64; 3]> = BTreeSet::new();
        for i in 0..self.boxes.len() {
            for j in i + 1..self.boxes.len() {
                let Some(meet) = intersect(self.claim_bounds(i), self.claim_bounds(j)) else {
                    continue;
                };
                let cand = self.near(meet);
                for c in cells(meet) {
                    if !(self.claims(i, c) && self.claims(j, c)) {
                        continue;
                    }
                    match self.owner_among(c, &cand) {
                        Owner::Contested(names) => {
                            by.entry(names).or_default().insert(c);
                        }
                        _ => {
                            awarded.insert(c);
                        }
                    }
                }
            }
        }
        (
            by.into_iter()
                .map(|(n, c)| (n, c.into_iter().collect()))
                .collect(),
            awarded.len(),
        )
    }
}

/// Greedy merge of a cell set into disjoint AABBs, in one fixed order: take the
/// least cell, run along `z`, then grow the run along `y`, then along `x`, each
/// only while every cell of the grown slab is present. Deterministic (ADR-0006)
/// and exact — the union of the result is the input.
#[must_use]
pub fn merge_cells(cells: &BTreeSet<[i64; 3]>) -> Vec<Aabb> {
    let mut left = cells.clone();
    let mut out = Vec::new();
    while let Some(&lo) = left.iter().next() {
        let mut hi = lo;
        while left.contains(&[lo[0], lo[1], hi[2] + 1]) {
            hi[2] += 1;
        }
        'y: loop {
            let ny = hi[1] + 1;
            for z in lo[2]..=hi[2] {
                if !left.contains(&[lo[0], ny, z]) {
                    break 'y;
                }
            }
            hi[1] = ny;
        }
        'x: loop {
            let nx = hi[0] + 1;
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    if !left.contains(&[nx, y, z]) {
                        break 'x;
                    }
                }
            }
            hi[0] = nx;
        }
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    left.remove(&[x, y, z]);
                }
            }
        }
        out.push((lo, hi));
    }
    out
}

/// A run of cells inside a place's frame that the place does not own, and who
/// does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Void {
    /// Low corner, inclusive, in world coordinates.
    pub lo: [i64; 3],
    /// High corner, inclusive.
    pub hi: [i64; 3],
    /// Who writes them.
    pub owner: Owner,
}

/// A place's owned cells and the voids inside their bounding box, derived from
/// the whole plan. The one computation behind `Frame::of`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ownership {
    /// Inclusive low corner of the owned cells' bounding box.
    pub lo: [i64; 3],
    /// Inclusive high corner.
    pub hi: [i64; 3],
    /// The owned cells, merged.
    pub owned: Vec<Aabb>,
    /// How many cells the place owns.
    pub owned_cells: usize,
    /// Every cell of the bounding box the place does not own, merged per owner,
    /// in owner order then cell order.
    pub voids: Vec<Void>,
    /// The fixed ground cells inside the bounding box, each with the block the
    /// whole writes there, in cell order.
    pub fixed: Vec<([i64; 3], String)>,
    /// Eaves cells the plan clipped at a neighbour's shell, merged, with
    /// the neighbour.
    pub clipped: Vec<(Aabb, NodeId)>,
}

impl Site<'_> {
    /// What place `i` owns, and what inside its frame it does not.
    #[must_use]
    pub fn ownership(&self, i: usize) -> Ownership {
        let b = &self.boxes[i];
        let me = Owner::Place(b.node.clone());
        let bounds = self.claim_bounds(i);
        let cand = self.near(bounds);
        let owned: BTreeSet<[i64; 3]> = self
            .claim_cells(i)
            .into_iter()
            .filter(|c| self.owner_among(*c, &cand) == me)
            .collect();
        let (lo, hi) = owned
            .iter()
            .fold(([i64::MAX; 3], [i64::MIN; 3]), |(mut lo, mut hi), c| {
                for a in 0..3 {
                    lo[a] = lo[a].min(c[a]);
                    hi[a] = hi[a].max(c[a]);
                }
                (lo, hi)
            });
        let mut by_owner: BTreeMap<Owner, BTreeSet<[i64; 3]>> = BTreeMap::new();
        let mut fixed: Vec<([i64; 3], String)> = Vec::new();
        if !owned.is_empty() {
            let fcand = self.near((lo, hi));
            for c in cells((lo, hi)) {
                if owned.contains(&c) {
                    continue;
                }
                let o = self.owner_among(c, &fcand);
                if o == Owner::Ground {
                    let g = fcand
                        .iter()
                        .find(|&&k| self.is_fixed(k, c))
                        .map_or(c[1], |&k| self.ground_height(k, c[0], c[2]));
                    fixed.push((c, self.ground.ground_block(c, g).to_string()));
                }
                by_owner.entry(o).or_default().insert(c);
            }
        }
        let voids: Vec<Void> = by_owner
            .iter()
            .flat_map(|(o, cells)| {
                merge_cells(cells).into_iter().map(move |(lo, hi)| Void {
                    lo,
                    hi,
                    owner: o.clone(),
                })
            })
            .collect();
        // Eaves the plan clipped: roof-zone cells outside the shell footprint
        // that lie in a neighbour's shell.
        let mut clipped_by: BTreeMap<NodeId, BTreeSet<[i64; 3]>> = BTreeMap::new();
        if let Some(zone) = b.roof_zone() {
            for c in cells(zone) {
                let over_shell = c[0] >= b.foot[0] - 1
                    && c[0] <= b.foot[1] + 1
                    && c[2] >= b.foot[2] - 1
                    && c[2] <= b.foot[3] + 1;
                if over_shell {
                    continue;
                }
                if let Some(k) = (0..self.boxes.len()).find(|&k| k != i && inside(c, self.shell(k)))
                {
                    clipped_by
                        .entry(self.boxes[k].node.clone())
                        .or_default()
                        .insert(c);
                }
            }
        }
        let clipped: Vec<(Aabb, NodeId)> = clipped_by
            .iter()
            .flat_map(|(n, cells)| merge_cells(cells).into_iter().map(move |r| (r, n.clone())))
            .collect();
        Ownership {
            lo,
            hi,
            owned_cells: owned.len(),
            owned: merge_cells(&owned),
            voids,
            fixed,
            clipped,
        }
    }
}

/// A campaign's resolved plan, owned: what a [`Site`] borrows.
#[derive(Debug, Clone)]
pub struct SitePlan {
    /// The plan's places.
    pub boxes: Vec<PlacedBox>,
    /// The plan's connections.
    pub seams: Vec<PlacedSeam>,
    /// The site's fill.
    pub ground: Ground,
}

impl SitePlan {
    /// Resolve a campaign's plan.
    #[must_use]
    pub fn of(c: &Campaign) -> SitePlan {
        let mut reads = crate::metrics::Reads::new();
        let boxes = super::placed_boxes(c, &mut reads);
        let seams = super::placed_seams(c, &boxes, &mut reads);
        SitePlan {
            boxes,
            seams,
            ground: Ground::of(c),
        }
    }

    /// The ownership view over it.
    #[must_use]
    pub fn site(&self) -> Site<'_> {
        Site::new(&self.boxes, &self.seams, &self.ground)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::EdgeId;
    use crate::siteplan::{Crossing, Face, Roof};

    fn a_box(node: &str, foot: [i64; 4], floor: i64, clearance: u32, open: bool) -> PlacedBox {
        PlacedBox {
            node: NodeId(node.into()),
            foot,
            floor,
            clearance,
            open,
            roof: None,
        }
    }

    fn seam(a: &str, b: &str, axis: usize, plane: i64) -> PlacedSeam {
        PlacedSeam {
            edge: EdgeId(format!("edge/{}-{}", &a[5..], &b[5..])),
            class: "walk",
            a: NodeId(a.into()),
            b: NodeId(b.into()),
            face: Face::East,
            normal_axis: axis,
            plane,
            opening: ([plane, 64, 2], [plane, 66, 3]),
            shared: ([plane, 64, 0], [plane, 67, 7]),
            crossing: Crossing::Portal,
            rise: 0,
            stair_in: None,
            form: "a doorway".into(),
        }
    }

    fn solid() -> Ground {
        Ground::solid("minecraft:stone")
    }

    /// A house (x 0..7) and the street east of it (x 9..16), one cell apart.
    fn house_and_street() -> Vec<PlacedBox> {
        vec![
            a_box("node/house", [0, 7, 0, 7], 64, 4, false),
            a_box("node/street", [9, 16, 0, 7], 64, 6, true),
        ]
    }

    /// **An aloft seam fixes no earth under it; a grade seam still levels**
    /// (spec-0098 §14, a correction). Two treehouses whose floors stand 16
    /// courses over open ground at y 47, joined across the plane x 8 by a
    /// bridge seam whose sill is the deck at y 64: the ring column under the
    /// opening is the terrain's ground (y 47), and the cells between it and
    /// the deck are the owner's, not the whole's. The same pair on ground at
    /// y 63 — the sill one step over it — levels the column to the sill minus
    /// one, as before.
    #[test]
    fn an_aloft_seam_fixes_no_earth_under_it_and_a_grade_seam_levels() {
        let boxes = vec![
            a_box("node/east-house", [0, 7, 0, 7], 64, 4, false),
            a_box("node/west-house", [9, 16, 0, 7], 64, 4, false),
        ];
        let seams = vec![seam("node/east-house", "node/west-house", 0, 8)];
        let low = Ground::open_flat(47, "minecraft:grass_block", "minecraft:dirt");
        let site = Site::new(&boxes, &seams, &low);
        assert_eq!(site.ground_height(0, 8, 2), 47, "the terrain, not the deck");
        assert!(
            !site.is_fixed(0, [8, 55, 2]),
            "no earth column under the bridge"
        );
        assert_eq!(
            site.owner([8, 55, 2]),
            Owner::Place(NodeId("node/east-house".into())),
            "the column between terrain and deck is the seam's a"
        );
        assert!(
            site.is_fixed(0, [8, 47, 2]),
            "the terrain itself is still fixed"
        );
        let grade = Ground::open_flat(62, "minecraft:grass_block", "minecraft:dirt");
        let site = Site::new(&boxes, &seams, &grade);
        assert_eq!(
            site.ground_height(0, 8, 2),
            63,
            "one step up: levelled to the sill"
        );
        assert!(site.is_fixed(0, [8, 63, 2]));
    }

    #[test]
    fn the_party_plane_between_a_house_and_a_street_is_the_facade() {
        let boxes = house_and_street();
        let g = solid();
        let site = Site::new(&boxes, &[], &g);
        assert_eq!(
            site.owner([8, 65, 3]),
            Owner::Place(NodeId("node/house".into()))
        );
        assert_eq!(
            site.owner([8, 63, 3]),
            Owner::Ground,
            "the ring's ground is fixed"
        );
        assert_eq!(
            site.owner([8, 69, 3]),
            Owner::Place(NodeId("node/street".into()))
        );
        assert_eq!(
            site.owner([17, 65, 3]),
            Owner::Place(NodeId("node/street".into()))
        );
        assert_eq!(site.owner([30, 65, 3]), Owner::Nobody);
    }

    #[test]
    fn two_yards_meet_on_the_side_their_connection_names_first() {
        let boxes = vec![
            a_box("node/yard-a", [0, 7, 0, 7], 64, 3, true),
            a_box("node/yard-b", [9, 16, 0, 7], 64, 3, true),
        ];
        let g = solid();
        let site = Site::new(&boxes, &[], &g);
        assert!(
            matches!(site.owner([8, 65, 1]), Owner::Contested(_)),
            "nothing connects them"
        );
        let s = [seam("node/yard-b", "node/yard-a", 0, 8)];
        let site = Site::new(&boxes, &s, &g);
        assert_eq!(
            site.owner([8, 65, 1]),
            Owner::Place(NodeId("node/yard-b".into()))
        );
        let disagree = [
            seam("node/yard-b", "node/yard-a", 0, 8),
            seam("node/yard-a", "node/yard-b", 0, 8),
        ];
        let site = Site::new(&boxes, &disagree, &g);
        assert!(matches!(site.owner([8, 65, 1]), Owner::Contested(_)));
    }

    #[test]
    fn a_stacked_plane_is_the_upper_floor() {
        let lower = a_box("node/crypt", [0, 7, 0, 7], 60, 3, false); // top 62, lid 63
        let upper = a_box("node/chapel", [0, 7, 0, 7], 64, 6, false); // floor course 63
        let boxes = vec![lower, upper];
        let g = solid();
        let site = Site::new(&boxes, &[], &g);
        assert_eq!(
            site.owner([3, 63, 3]),
            Owner::Place(NodeId("node/chapel".into()))
        );
    }

    #[test]
    fn a_roof_zone_is_claimed_and_its_eaves_stop_at_a_neighbour() {
        let mut boxes = house_and_street();
        boxes[0].roof = Some(Roof {
            courses: 4,
            eaves: 1,
        });
        let g = solid();
        let site = Site::new(&boxes, &[], &g);
        assert_eq!(boxes[0].roof_zone(), Some(([-2, 68, -2], [9, 72, 9])));
        assert!(!site.claims(0, [9, 68, 3]));
        assert_eq!(
            site.owner([9, 68, 3]),
            Owner::Place(NodeId("node/street".into()))
        );
        assert!(site.claims(0, [9, 71, 3]));
        assert_eq!(
            site.owner([9, 71, 3]),
            Owner::Place(NodeId("node/house".into()))
        );
        let o = site.ownership(0);
        assert_eq!(o.lo, [-2, 63, -2]);
        assert_eq!(o.hi, [9, 72, 9]);
        assert!(o.clipped.iter().any(|(_, n)| n.0 == "node/street"));
        let owned: BTreeSet<[i64; 3]> = o.owned.iter().flat_map(|a| cells(*a)).collect();
        assert_eq!(owned.len(), o.owned_cells, "the merge is exact");
        for v in &o.voids {
            assert!(!owned.contains(&v.lo));
        }
    }

    /// **Criterion 16, at the ownership rule.** Two roofed places exactly one
    /// cell apart with no connection share the column their rings stand in,
    /// and no rule awards it (rule 3d — `DW0827` refuses the plan). Each remedy
    /// the refusal names parts or awards it: a seam between them awards the
    /// plane to its `a`; standing them two apart leaves their claims disjoint,
    /// the gap the site's fill; one place has no pair.
    #[test]
    fn two_roofed_places_one_apart_are_contested_and_each_remedy_parts_them() {
        let g = solid();
        let a = a_box("node/west-room", [0, 7, 0, 7], 64, 4, false);
        let b = a_box("node/east-room", [9, 16, 0, 7], 64, 4, false);
        let boxes = vec![a.clone(), b.clone()];
        let site = Site::new(&boxes, &[], &g);
        let (contests, _) = site.contests();
        assert_eq!(contests.len(), 1, "one pair contests");
        assert!(!contests[0].1.is_empty(), "and the claims really intersect");
        assert!(
            contests[0].1.iter().all(|c| c[0] == 8),
            "in the shared ring column"
        );

        let s = [seam("node/west-room", "node/east-room", 0, 8)];
        let site = Site::new(&boxes, &s, &g);
        assert!(
            site.contests().0.is_empty(),
            "a connection awards the plane"
        );
        assert_eq!(
            site.owner([8, 65, 5]),
            Owner::Place(NodeId("node/west-room".into()))
        );

        let apart = vec![
            a.clone(),
            a_box("node/east-room", [10, 17, 0, 7], 64, 4, false),
        ];
        let site = Site::new(&apart, &[], &g);
        assert!(site.contests().0.is_empty(), "two apart, nothing is shared");
        assert_eq!(
            site.owner([9, 65, 5]),
            Owner::Place(NodeId("node/east-room".into()))
        );

        let one = vec![a_box("node/one-room", [0, 16, 0, 7], 64, 4, false)];
        assert!(Site::new(&one, &[], &g).contests().0.is_empty());
    }

    #[test]
    fn merge_is_exact_and_deterministic() {
        let cells: BTreeSet<[i64; 3]> = [[0, 0, 0], [0, 0, 1], [1, 0, 0], [1, 0, 1], [5, 5, 5]]
            .into_iter()
            .collect();
        let m = merge_cells(&cells);
        assert_eq!(m, vec![([0, 0, 0], [1, 0, 1]), ([5, 5, 5], [5, 5, 5])]);
        assert_eq!(merge_cells(&cells), m);
    }
}
