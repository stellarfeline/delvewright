//! **A place's claim, and who owns a cell two claims cover** (spec-0098 §2).
//!
//! A place owns everything a body can see of it from outside — its floor
//! course, its play space, the one-cell ring its walls stand in and, roofed, its
//! lid and the roof zone the plan declares — and the whole owns only what two
//! places would otherwise both write. This module is the one derivation of both
//! halves: [`PlacedBox::claim_cells`] says what a place would write if nothing
//! stood beside it, [`owner`] says who writes a cell several places claim, and
//! [`Frame::of`](crate::detailplan::Frame::of) is built on nothing else.
//!
//! Every reader — the blockout's holes, the handing, `DW0843`/`DW0844`/
//! `DW0987`, the light pass — calls [`owner`] or a function of it. Two of them
//! deciding "whose cell is this" independently is how a builder and its observer
//! come to agree about a world neither describes.

use std::collections::{BTreeMap, BTreeSet};

use crate::ids::NodeId;

use super::{PlacedBox, PlacedSeam};

/// An inclusive world AABB, `(lo, hi)`.
pub type Aabb = ([i64; 3], [i64; 3]);

/// Who writes a cell.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Owner {
    /// The derivation: structure neither place designs.
    Whole,
    /// The named place — its piece once bound, the derivation while not.
    Place(NodeId),
}

impl Owner {
    /// The spelling the handing and the refusals use.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Owner::Whole => "the whole".to_string(),
            Owner::Place(n) => format!("`{n}`"),
        }
    }
}

fn inside(cell: [i64; 3], (lo, hi): Aabb) -> bool {
    (0..3).all(|i| cell[i] >= lo[i] && cell[i] <= hi[i])
}

impl PlacedBox {
    /// The top course of the play space.
    #[must_use]
    pub fn top(&self) -> i64 {
        self.floor + i64::from(self.clearance) - 1
    }

    /// The shell: the play space grown one cell on every side — floor course,
    /// ring and, on a roofed place, the lid. An open place has no lid, so its
    /// shell stops at the top of its play space.
    #[must_use]
    pub fn shell(&self) -> Aabb {
        let (lo, hi) = self.space();
        (
            [lo[0] - 1, lo[1] - 1, lo[2] - 1],
            [
                hi[0] + 1,
                if self.open { hi[1] } else { hi[1] + 1 },
                hi[2] + 1,
            ],
        )
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
        let (lo, hi) = self.shell();
        let e = i64::from(roof.eaves);
        Some((
            [lo[0] - e, hi[1], lo[2] - e],
            [hi[0] + e, hi[1] + i64::from(roof.courses), hi[2] + e],
        ))
    }

    /// The roof proper — the roof zone's courses over the shell footprint, above
    /// the lid. Never clipped: a course of it inside another place is `DW0988`.
    #[must_use]
    pub fn roof_proper(&self) -> Option<Aabb> {
        let roof = self.roof?;
        if self.open || roof.courses == 0 {
            return None;
        }
        let (lo, hi) = self.shell();
        Some((
            [lo[0], hi[1] + 1, lo[2]],
            [hi[0], hi[1] + i64::from(roof.courses), hi[2]],
        ))
    }

    /// The floor course: the shell footprint at `floor − 1`.
    #[must_use]
    pub fn floor_course(&self) -> Aabb {
        let (lo, hi) = self.shell();
        ([lo[0], lo[1], lo[2]], [hi[0], lo[1], hi[2]])
    }

    /// True when `cell` is one this place would write if nothing stood beside
    /// it: in its shell, or in its roof zone — except an eaves cell standing in
    /// another place's play space, where the eave stops (spec-0098 §3).
    #[must_use]
    pub fn claims(&self, cell: [i64; 3], boxes: &[PlacedBox]) -> bool {
        if inside(cell, self.shell()) {
            return true;
        }
        let Some(zone) = self.roof_zone() else {
            return false;
        };
        if !inside(cell, zone) {
            return false;
        }
        let (slo, shi) = self.shell();
        let over_shell =
            cell[0] >= slo[0] && cell[0] <= shi[0] && cell[2] >= slo[2] && cell[2] <= shi[2];
        if over_shell {
            return true;
        }
        // An eaves cell: stops at a neighbour's play space.
        !boxes
            .iter()
            .any(|o| o.node != self.node && inside(cell, o.space()))
    }

    /// The bounding box of everything this place might claim — what a reader
    /// iterates to find its cells.
    #[must_use]
    pub fn claim_bounds(&self) -> Aabb {
        let (mut lo, mut hi) = self.shell();
        if let Some((zlo, zhi)) = self.roof_zone() {
            for i in 0..3 {
                lo[i] = lo[i].min(zlo[i]);
                hi[i] = hi[i].max(zhi[i]);
            }
        }
        (lo, hi)
    }

    /// Every cell of this place's claim, in a fixed order.
    #[must_use]
    pub fn claim_cells(&self, boxes: &[PlacedBox]) -> Vec<[i64; 3]> {
        let (lo, hi) = self.claim_bounds();
        let mut out = Vec::new();
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    let c = [x, y, z];
                    if self.claims(c, boxes) {
                        out.push(c);
                    }
                }
            }
        }
        out
    }
}

/// **Who writes `cell`** — spec-0098 §2's rule, in its order:
///
/// 1. a cell inside a place's play space is that place's;
/// 2. a cell exactly one claim covers is that place's;
/// 3. a cell several claims cover: in exactly one of their floor courses, that
///    place's (a stacked plane is the upper's floor); else, exactly one of them
///    roofed, that one's (a facade onto an open place); else, every seam the
///    plan allocates between the pair across that plane naming one `a`, that
///    place's (a designed connection is drawn by its first-named side); else
///    the whole's;
/// 4. a cell no claim covers is the whole's.
#[must_use]
pub fn owner(cell: [i64; 3], boxes: &[PlacedBox], seams: &[PlacedSeam]) -> Owner {
    if let Some(b) = boxes.iter().find(|b| inside(cell, b.space())) {
        return Owner::Place(b.node.clone());
    }
    let claimants: Vec<&PlacedBox> = boxes.iter().filter(|b| b.claims(cell, boxes)).collect();
    match claimants.len() {
        0 => return Owner::Whole,
        1 => return Owner::Place(claimants[0].node.clone()),
        _ => {}
    }
    // 3a — a floor course.
    let floors: Vec<&&PlacedBox> = claimants
        .iter()
        .filter(|b| inside(cell, b.floor_course()))
        .collect();
    if floors.len() == 1 {
        return Owner::Place(floors[0].node.clone());
    }
    // 3b — exactly one roofed.
    let roofed: Vec<&&PlacedBox> = claimants.iter().filter(|b| !b.open).collect();
    if roofed.len() == 1 {
        return Owner::Place(roofed[0].node.clone());
    }
    // 3c — the connection's `a` side, where exactly two places contest the cell
    // and every seam between them across this plane agrees.
    if claimants.len() == 2 {
        let (p, q) = (claimants[0], claimants[1]);
        let across: Vec<&PlacedSeam> = seams
            .iter()
            .filter(|s| {
                ((s.a == p.node && s.b == q.node) || (s.a == q.node && s.b == p.node))
                    && cell[s.normal_axis] == s.plane
            })
            .collect();
        if let Some(first) = across.first()
            && across.iter().all(|s| s.a == first.a)
        {
            return Owner::Place(first.a.clone());
        }
    }
    Owner::Whole
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
    /// The cells, inclusive, in world coordinates.
    pub lo: [i64; 3],
    pub hi: [i64; 3],
    /// Who writes them.
    pub owner: Owner,
}

/// A place's owned cells and the voids inside their bounding box, derived from
/// the whole plan. The one computation behind `Frame::of`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ownership {
    /// Inclusive bounding box of the owned cells.
    pub lo: [i64; 3],
    pub hi: [i64; 3],
    /// The owned cells, merged.
    pub owned: Vec<Aabb>,
    /// How many cells the place owns.
    pub owned_cells: usize,
    /// Every cell of the bounding box the place does not own, merged per owner,
    /// in owner order then cell order.
    pub voids: Vec<Void>,
    /// Eaves cells the plan clipped at a neighbour's play space, merged, with
    /// the neighbour.
    pub clipped: Vec<(Aabb, NodeId)>,
}

/// What `b` owns, and what inside its frame it does not.
#[must_use]
pub fn ownership(b: &PlacedBox, boxes: &[PlacedBox], seams: &[PlacedSeam]) -> Ownership {
    let me = Owner::Place(b.node.clone());
    let owned: BTreeSet<[i64; 3]> = b
        .claim_cells(boxes)
        .into_iter()
        .filter(|c| owner(*c, boxes, seams) == me)
        .collect();
    let (lo, hi) = owned
        .iter()
        .fold(([i64::MAX; 3], [i64::MIN; 3]), |(mut lo, mut hi), c| {
            for i in 0..3 {
                lo[i] = lo[i].min(c[i]);
                hi[i] = hi[i].max(c[i]);
            }
            (lo, hi)
        });
    let mut by_owner: BTreeMap<Owner, BTreeSet<[i64; 3]>> = BTreeMap::new();
    if !owned.is_empty() {
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    let c = [x, y, z];
                    if !owned.contains(&c) {
                        by_owner
                            .entry(owner(c, boxes, seams))
                            .or_default()
                            .insert(c);
                    }
                }
            }
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
    // Eaves the plan clipped: roof-zone cells outside the shell footprint that
    // lie in a neighbour's play space.
    let mut clipped_by: BTreeMap<NodeId, BTreeSet<[i64; 3]>> = BTreeMap::new();
    if let Some((zlo, zhi)) = b.roof_zone() {
        let (slo, shi) = b.shell();
        for x in zlo[0]..=zhi[0] {
            for y in zlo[1]..=zhi[1] {
                for z in zlo[2]..=zhi[2] {
                    let over_shell = x >= slo[0] && x <= shi[0] && z >= slo[2] && z <= shi[2];
                    if over_shell {
                        continue;
                    }
                    if let Some(o) = boxes
                        .iter()
                        .find(|o| o.node != b.node && inside([x, y, z], o.space()))
                    {
                        clipped_by
                            .entry(o.node.clone())
                            .or_default()
                            .insert([x, y, z]);
                    }
                }
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
        clipped,
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
            opening: ([plane, 0, 0], [plane, 0, 0]),
            shared: ([plane, 0, 0], [plane, 0, 0]),
            crossing: Crossing::Portal,
            rise: 0,
            stair_in: None,
        }
    }

    /// A house (x 0..7) and the street east of it (x 9..16), one cell apart.
    fn house_and_street() -> Vec<PlacedBox> {
        vec![
            a_box("node/house", [0, 7, 0, 7], 64, 4, false),
            a_box("node/street", [9, 16, 0, 7], 64, 6, true),
        ]
    }

    #[test]
    fn the_party_plane_between_a_house_and_a_street_is_the_facade() {
        let boxes = house_and_street();
        // The wall column x = 8, at floor height, inside both rings.
        assert_eq!(
            owner([8, 65, 3], &boxes, &[]),
            Owner::Place(NodeId("node/house".into()))
        );
        // Above the house's lid the street's ring still reaches (its headroom
        // is 6, the house's 4): one claimant, the street's.
        assert_eq!(
            owner([8, 69, 3], &boxes, &[]),
            Owner::Place(NodeId("node/street".into()))
        );
        // The street's far ring is its own.
        assert_eq!(
            owner([17, 65, 3], &boxes, &[]),
            Owner::Place(NodeId("node/street".into()))
        );
        // Nobody's: the whole's.
        assert_eq!(owner([30, 65, 3], &boxes, &[]), Owner::Whole);
    }

    #[test]
    fn two_yards_meet_on_the_side_their_connection_names_first() {
        let boxes = vec![
            a_box("node/yard-a", [0, 7, 0, 7], 64, 3, true),
            a_box("node/yard-b", [9, 16, 0, 7], 64, 3, true),
        ];
        assert_eq!(
            owner([8, 65, 3], &boxes, &[]),
            Owner::Whole,
            "nothing connects them"
        );
        let s = [seam("node/yard-b", "node/yard-a", 0, 8)];
        assert_eq!(
            owner([8, 65, 3], &boxes, &s),
            Owner::Place(NodeId("node/yard-b".into()))
        );
        let disagree = [
            seam("node/yard-b", "node/yard-a", 0, 8),
            seam("node/yard-a", "node/yard-b", 0, 8),
        ];
        assert_eq!(owner([8, 65, 3], &boxes, &disagree), Owner::Whole);
    }

    #[test]
    fn a_stacked_plane_is_the_upper_floor() {
        let lower = a_box("node/crypt", [0, 7, 0, 7], 60, 3, false); // top 62, lid 63
        let upper = a_box("node/chapel", [0, 7, 0, 7], 64, 6, false); // floor course 63
        let boxes = vec![lower, upper];
        assert_eq!(
            owner([3, 63, 3], &boxes, &[]),
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
        // The house: play 64..67, lid 68, roof 68..72, zone x -2..9.
        let h = &boxes[0];
        assert_eq!(h.roof_zone(), Some(([-2, 68, -2], [9, 72, 9])));
        // An eaves cell over the street's play space (x 9, y 68 is inside the
        // street's headroom 64..69): clipped, so the street owns it alone.
        assert!(!h.claims([9, 68, 3], &boxes));
        assert_eq!(
            owner([9, 68, 3], &boxes, &[]),
            Owner::Place(NodeId("node/street".into()))
        );
        // The same eave above the street's headroom is the house's.
        assert!(h.claims([9, 71, 3], &boxes));
        assert_eq!(
            owner([9, 71, 3], &boxes, &[]),
            Owner::Place(NodeId("node/house".into()))
        );
        let o = ownership(h, &boxes, &[]);
        assert_eq!(o.lo, [-2, 63, -2]);
        assert_eq!(o.hi, [9, 72, 9]);
        assert!(o.clipped.iter().any(|(_, n)| n.0 == "node/street"));
        // Every void inside the frame names an owner, and no owned cell is a void.
        let owned: BTreeSet<[i64; 3]> = o
            .owned
            .iter()
            .flat_map(|(lo, hi)| {
                let mut v = Vec::new();
                for x in lo[0]..=hi[0] {
                    for y in lo[1]..=hi[1] {
                        for z in lo[2]..=hi[2] {
                            v.push([x, y, z]);
                        }
                    }
                }
                v
            })
            .collect();
        assert_eq!(owned.len(), o.owned_cells, "the merge is exact");
        for v in &o.voids {
            assert!(!owned.contains(&v.lo));
        }
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
