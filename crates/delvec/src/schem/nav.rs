//! Reading a box of blocks the way a body meets it: what it can stand in, and
//! what it can walk to.
//!
//! # Why the capability lives here
//!
//! "Can a body stand on this cell, and can it walk from that one to this one" is
//! the same question whatever produced the blocks — a grammar expansion, a
//! structure template read off disk, a zone reassembled from tiles. It had been
//! answered privately seven times over, each copy keyed to the type
//! that happened to need it first, so a fix to one reached none of the others
//! and the light probe in `delvec prefab` could not use the walk that
//! `delvec::grammar` had already written.
//!
//! A capability belongs to the object class it acts on (CLAUDE.md), and the
//! object class is not `VoxelModel` and not `Structure` — it is **a box of cells
//! with a passability answer for each**. That is [`Voxels`], and it is small
//! enough that any producer can implement it in a dozen lines. This crate is
//! where it goes because it is the one both the grammar back end and the
//! admission pipeline already depend on, and because it already owns the other
//! two facts shared at this layer: the block registry and the tile-set contract.
//!
//! # What is here, and what is not
//!
//! **Where a body can get to is not answered here.** It is the reach graph's
//! ([`crate::compiler::nav::World::floor_reached`] over
//! [`crate::compiler::nav::World::of_piece`]): the walk, the climb, every fall,
//! every swim and every jump of the launch set, flown by the body simulator.
//! A step walk here would be a second answer to that question, and this
//! module had one until every reader asked the graph.
//!
//! What is here is what a box of cells says without a body moving through
//! it: which cells hold a body ([`standable`]), where a body walks in
//! ([`ground_entry`]), which cells are under a roof ([`sheltered`]), which lump
//! of floor a cell belongs to ([`components`]) and, at the edge of what the
//! graph reached, which step the step rule turned back
//! ([`refused_frontier`]). The step rule those two read is not written here.
//! It is [`delvewright_dsl::metrics::step_allowed`], which is also what
//! `delvec`'s navigation model asks.
//!
//! What is genuinely this module's is the **measurement** of a rise. A box of
//! cells has no collision heights, so a step of one cell is read as a full block
//! (16/16) unless the implementor knows better and says so through
//! [`Voxels::floor_top_16`]. Reading a partial floor as a full one only ever
//! over-states a rise, so the default is the refusing direction — see that
//! method for what it costs and who should override it.

use std::collections::{BTreeSet, VecDeque};

use delvewright_dsl::metrics::{FULL_16, Rise, classify_rise_16, step_allowed};

/// A box of cells, and the one thing a walk needs to know about each.
///
/// Implementors answer for their own block vocabulary: a rule library that
/// places a floor skull on the cell an anchor names counts it passable, a
/// structure template read off disk counts the air family. Deliberately not a
/// block-name predicate in this crate — "what counts as passable" is content,
/// and the walk is mechanism.
pub trait Voxels {
    /// Low corner of the box.
    fn origin(&self) -> [i32; 3];
    /// Extent of the box on each axis.
    fn size(&self) -> [i32; 3];
    /// Can a body's body occupy this cell? **False outside the box, always** —
    /// a body that has left the box has left the thing being measured.
    fn passable(&self, pos: [i32; 3]) -> bool;

    /// One past the high corner on each axis.
    fn maximum(&self) -> [i32; 3] {
        let (o, s) = (self.origin(), self.size());
        [o[0] + s[0], o[1] + s[1], o[2] + s[2]]
    }

    /// Is this cell inside the box at all?
    fn contains(&self, pos: [i32; 3]) -> bool {
        let (o, m) = (self.origin(), self.maximum());
        (0..3).all(|axis| pos[axis] >= o[axis] && pos[axis] < m[axis])
    }

    /// Can a body stand **on** this cell?
    ///
    /// Not the complement of [`Voxels::passable`], and that is the whole reason
    /// it is a separate question: a torch is neither — a body walks through it
    /// and cannot stand on it — and lava is neither, from the other side. The
    /// default answers "anything that is not passable", which is right for a
    /// vocabulary of full blocks and air; a vocabulary with decorations
    /// overrides it.
    fn floor(&self, pos: [i32; 3]) -> bool {
        solid(self, pos)
    }

    /// The walkable top face of the floor block at `support`, in sixteenths above
    /// that block's own cell floor (16 = a full cube). This is the one input the
    /// step rule needs that a boolean box cannot answer, so the default answers
    /// **a full cube** for anything that is a floor at all.
    ///
    /// That default is a measurement, not a rule, and its error has one
    /// direction: a bottom slab read as a full cube turns a 8/16 walk-up into a
    /// 16/16 jump, so the walk **refuses** a step vanilla admits. It never admits
    /// one vanilla refuses. An implementor whose vocabulary has partial-height
    /// blocks — slabs, snow layers, carpets, `dirt_path` — should override this,
    /// and until it does its walk is a conservative reading of its own bytes
    /// rather than a wrong one.
    ///
    /// Deliberately keyed to the SUPPORT cell rather than to the standing cell,
    /// because that is the block whose top face the body rests on, and it is the
    /// same quantity `delvec`'s model reads out of its collision table.
    fn floor_top_16(&self, support: [i32; 3]) -> i64 {
        let _ = support;
        FULL_16
    }
}

/// The true feet height of a body standing in `c`, in sixteenths, absolute — so
/// two standing cells can be differenced directly to get the rise between them.
///
/// The mirror of `delvec`'s own `feet_16_fp` for a single-column body: the cell
/// floor of the support, plus that support's walkable top face.
fn feet_16<V: Voxels + ?Sized>(v: &V, c: [i32; 3]) -> i64 {
    (c[1] as i64 - 1) * FULL_16 + v.floor_top_16([c[0], c[1] - 1, c[2]])
}

/// Can a body step from `from` to `to`, both standable? The engine's one step
/// rule ([`delvewright_dsl::metrics::step_allowed`]) over this box's own reading
/// of the rise.
///
/// The head sweep is `from`'s own column two courses up — a standing body is two
/// cells tall here, so that is the cell it passes through on the way over.
fn can_step<V: Voxels + ?Sized>(v: &V, from: [i32; 3], to: [i32; 3]) -> bool {
    step_allowed(feet_16(v, to) - feet_16(v, from), || {
        v.passable([from[0], from[1] + 2, from[2]])
    })
}

/// Every cell of a box, in `x` → `y` → `z` order (deterministic, ADR-0006).
pub fn positions(origin: [i32; 3], size: [i32; 3]) -> impl Iterator<Item = [i32; 3]> {
    (0..size[0]).flat_map(move |dx| {
        (0..size[1]).flat_map(move |dy| {
            (0..size[2]).map(move |dz| [origin[0] + dx, origin[1] + dy, origin[2] + dz])
        })
    })
}

/// A full block: what a floor is made of, and what stops an eye.
pub fn solid<V: Voxels + ?Sized>(v: &V, pos: [i32; 3]) -> bool {
    v.contains(pos) && !v.passable(pos)
}

/// A cell a player can stand in: two blocks of clearance over a floor.
pub fn standable<V: Voxels + ?Sized>(v: &V, pos: [i32; 3]) -> bool {
    let [x, y, z] = pos;
    v.passable(pos) && v.passable([x, y + 1, z]) && v.floor([x, y - 1, z])
}

/// **A piece's floor, walked in**: its standable cells, the cells a body
/// starts from, and the standable cells the reach graph gets a body to from
/// them — what the light probe measures over and the enclosure survey reports.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnteredFloor {
    /// Every standable cell of the box.
    pub standable: BTreeSet<[i32; 3]>,
    /// The cells a body starts from.
    pub entry: BTreeSet<[i32; 3]>,
    /// The standable cells a body reaches from `entry`.
    pub reached: BTreeSet<[i32; 3]>,
}

/// Every standable cell of the box.
pub fn standable_cells<V: Voxels + ?Sized>(v: &V) -> BTreeSet<[i32; 3]> {
    positions(v.origin(), v.size())
        .filter(|&p| standable(v, p))
        .collect()
}

/// The connected components of a standable set, under the **undirected** step
/// relation: two cells share an edge when a body could make that step in either
/// direction.
///
/// # Why undirected, stated because it used to be free
///
/// The step rule is not symmetric. A body rises a full block only when the cell
/// its head sweeps through is clear; coming back down it asks nothing of the
/// ceiling. So "can walk between" is a *directed* relation and does not partition
/// a set — while a component is only meaningful as a partition, and this
/// function's one job is to hand a caller every lump of floor exactly once.
///
/// A component is therefore **a lump of floor**, not a reachability claim: it
/// answers "which cells belong to the same piece of ground", which is a grouping
/// question, and it is the right question for the one thing it is used for —
/// naming the stranded pockets of a piece so an author can find them. **Which
/// cells a body actually reaches is the reach graph's**
/// ([`crate::compiler::nav::World::floor_reached`]), from real entrances,
/// and a caller that wants that must ask for it: taking the component containing
/// an entrance is only the same answer while the relation is symmetric, which it
/// no longer is.
///
/// Order-independent by construction and deterministic in its output (ADR-0006):
/// the input is a `BTreeSet`, components are grown from its cells in that order,
/// and the result is sorted largest first, ties broken by the component's own
/// minimum cell.
pub fn components<V: Voxels + ?Sized>(
    v: &V,
    cells: &BTreeSet<[i32; 3]>,
) -> Vec<BTreeSet<[i32; 3]>> {
    let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut out: Vec<BTreeSet<[i32; 3]>> = Vec::new();
    for &start in cells {
        if seen.contains(&start) {
            continue;
        }
        // The undirected closure: grow through any step legal either way.
        let mut component = BTreeSet::from([start]);
        let mut queue: VecDeque<[i32; 3]> = VecDeque::from([start]);
        while let Some(cur) = queue.pop_front() {
            let [x, y, z] = cur;
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                for dy in [0, 1, -1] {
                    let next = [x + dx, y + dy, z + dz];
                    if cells.contains(&next)
                        && (can_step(v, cur, next) || can_step(v, next, cur))
                        && component.insert(next)
                    {
                        queue.push_back(next);
                    }
                }
            }
        }
        seen.extend(component.iter().copied());
        out.push(component);
    }
    out.sort_by(|a, b| {
        b.len()
            .cmp(&a.len())
            .then_with(|| a.iter().next().cmp(&b.iter().next()))
    });
    out
}

/// Where a body walks in: the standable cells on the box's four **vertical**
/// boundary faces, at grade.
///
/// A prefab is dropped into a world and met from outside at ground level, so the
/// cells that can be occupied without first being inside the piece are the ones
/// on its sides — and of those, only the ones at grade. Grade is derived, never
/// assumed: it is the lowest `Y` at which any side-face cell is standable, so a
/// piece raised on a plinth or sunk into a cutting finds its own. One course
/// above grade is included because that is the walk's own step height — a
/// threshold one block above the outside ground is stepped onto, not climbed.
///
/// **A belfry louvre is a standable cell on a side face and is deliberately not
/// an entrance.** Seeding the walk from every side-face cell at any height would
/// start the body wherever the building happens to be open, which is how a
/// reachability measure reports a stranded gallery as reached.
///
/// An empty result is a real answer — a sealed piece, or one meant to be entered
/// from above — and its caller reports it as a binding of zero rather than as a
/// reachability of zero.
pub fn ground_entry<V: Voxels + ?Sized>(v: &V) -> BTreeSet<[i32; 3]> {
    let min = v.origin();
    let max = v.maximum();
    let faces: BTreeSet<[i32; 3]> = standable_cells(v)
        .into_iter()
        .filter(|c| c[0] == min[0] || c[0] == max[0] - 1 || c[2] == min[2] || c[2] == max[2] - 1)
        .collect();
    let Some(grade) = faces.iter().map(|c| c[1]).min() else {
        return BTreeSet::new();
    };
    faces.into_iter().filter(|c| c[1] <= grade + 1).collect()
}

/// Is anything solid over this cell, inside the box?
///
/// The one thing a tool *can* say about whether a piece of floor was meant to be
/// walked indoors. A cell under a roof is floor: somebody was supposed to stand
/// there. A cell with open sky over it is a roof, a parapet, a terrace, a
/// courtyard or the ground outside the building, and the geometry alone cannot
/// tell which.
///
/// The scan starts two courses up because the cell and the one above it are the
/// body's own clearance and are passable by definition.
pub fn sheltered<V: Voxels + ?Sized>(v: &V, pos: [i32; 3]) -> bool {
    let [x, y, z] = pos;
    let top = v.maximum()[1];
    (y + 2..top).any(|above| solid(v, [x, above, z]))
}

// ---------------------------------------------------------------------------
// Where the walk stopped, and why
// ---------------------------------------------------------------------------

/// **Why a step the walk tried was refused.**
///
/// The reach graph answers *which cells*; it never says which step it was
/// turned back at, and "this cell is unreachable" is the answer an author can do
/// least with. The step rule is [`delvewright_dsl::metrics::step_allowed`] and it
/// has exactly two ways to say no, so a refusal is one of exactly two things —
/// there is no third arm and no free-text reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepRefusal {
    /// **The rise is one a body could jump, and the takeoff has no room for its
    /// head.** A standing body is two cells tall, so it already owns the cell it
    /// stands in and the one above; a full-block rise makes it jump, and the
    /// third cell — `head`, two courses over the TAKEOFF — has to be clear or it
    /// bonks. The opening is therefore short by exactly one course, and `head` is
    /// the cell to open.
    ///
    /// This is the class that is invisible at every scale below the whole
    /// building: the flight itself is walkable, every render shows a stair with
    /// a floor at the top of it, and the storey above is unreachable.
    Headroom {
        /// The cell two courses over the takeoff — one course of opening.
        head: [i32; 3],
    },
    /// The rise itself is past the jump apex: no headroom opens this one, the
    /// geometry has to come down or a step has to be built.
    Rise {
        /// The rise between the two standing surfaces, in sixteenths.
        rise_16: i64,
    },
}

/// One step at the edge of what a walk reached: a cell it stands on, a standable
/// cell one move away that it never got to, and [`StepRefusal`] saying why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefusedStep {
    /// The reached cell the body is standing on.
    pub from: [i32; 3],
    /// The standable cell one move away that the walk never entered.
    pub to: [i32; 3],
    /// Why the step was refused.
    pub refusal: StepRefusal,
}

/// **Every step a walk was refused at the boundary of what it reached.**
///
/// `cells` is the standable set the walk ran over and `reached` its result. The
/// frontier is every `(from, to)` with `from` reached, `to` standable and not
/// reached, one horizontal move and at most one course apart — and the step
/// rule refusing it. A `to` that several reached cells border appears once per
/// bordering cell, because each is a different place to cut the opening.
///
/// Deterministic (ADR-0006): `reached` is a `BTreeSet`, the neighbour order is
/// the walk's own fixed one, and nothing is sorted afterwards.
pub fn refused_frontier<V: Voxels + ?Sized>(
    v: &V,
    cells: &BTreeSet<[i32; 3]>,
    reached: &BTreeSet<[i32; 3]>,
) -> Vec<RefusedStep> {
    let mut out = Vec::new();
    for &from in reached {
        let [x, y, z] = from;
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            for dy in [0, 1, -1] {
                let to = [x + dx, y + dy, z + dz];
                if !cells.contains(&to) || reached.contains(&to) || can_step(v, from, to) {
                    continue;
                }
                let rise_16 = feet_16(v, to) - feet_16(v, from);
                let refusal = match classify_rise_16(rise_16) {
                    // `can_step` said no, so a Walk rise cannot be here: the rule
                    // admits every one of them unconditionally.
                    Rise::Walk | Rise::Jump => StepRefusal::Headroom {
                        head: [x, y + 2, z],
                    },
                    Rise::Beyond => StepRefusal::Rise { rise_16 },
                };
                out.push(RefusedStep { from, to, refusal });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A box of block names, the smallest thing that can answer [`Voxels`].
    struct Grid {
        size: [i32; 3],
        solid: BTreeSet<[i32; 3]>,
    }

    impl Voxels for Grid {
        fn origin(&self) -> [i32; 3] {
            [0, 0, 0]
        }
        fn size(&self) -> [i32; 3] {
            self.size
        }
        fn passable(&self, pos: [i32; 3]) -> bool {
            self.contains(pos) && !self.solid.contains(&pos)
        }
    }

    /// A flat floor at y=0 over the whole footprint, everything else air.
    fn field(size: [i32; 3]) -> Grid {
        let solid = (0..size[0])
            .flat_map(|x| (0..size[2]).map(move |z| [x, 0, z]))
            .collect();
        Grid { size, solid }
    }

    /// The geometry the compiler's step rule refuses
    /// (`delvec::compiler::nav`, `step_up_needs_head_clearance_to_jump`),
    /// stated here so the two answers can be read side by side. Feet at
    /// `[0,1,0]`, a floor one course higher at `x = 1,2`, and — when
    /// `low_ceiling` — a block on the cell the jumping body's head sweeps
    /// through (`[0,3,0]`). The compiler shifts the same shape up to `y = 64`;
    /// only the origin differs.
    fn head_bonk(low_ceiling: bool) -> Grid {
        let mut solid = BTreeSet::from([[0, 0, 0], [1, 1, 0], [2, 1, 0]]);
        if low_ceiling {
            solid.insert([0, 3, 0]);
        }
        Grid {
            size: [3, 6, 1],
            solid,
        }
    }

    /// The step rule's headroom arm, read off the frontier: with the swept head
    /// cell clear, the full-block rise is no refused step; with it blocked, the
    /// rise is refused as one course of headroom short, at that cell.
    #[test]
    fn a_full_block_rise_is_a_jump_and_needs_the_swept_head_cell_clear() {
        let reached = BTreeSet::from([[0, 1, 0]]);
        let open = head_bonk(false);
        let cells = standable_cells(&open);
        assert!(cells.contains(&[0, 1, 0]) && cells.contains(&[1, 2, 0]));
        assert!(
            refused_frontier(&open, &cells, &reached).is_empty(),
            "open headroom: the jump up is a step"
        );

        let low = head_bonk(true);
        let cells = standable_cells(&low);
        assert!(
            cells.contains(&[0, 1, 0]) && cells.contains(&[1, 2, 0]),
            "both ends are standable — the geometry differs only in the swept cell"
        );
        assert_eq!(
            refused_frontier(&low, &cells, &reached),
            vec![RefusedStep {
                from: [0, 1, 0],
                to: [1, 2, 0],
                refusal: StepRefusal::Headroom { head: [0, 3, 0] },
            }],
            "a ceiling two courses over the feet refuses the jump, at that cell"
        );
    }

    #[test]
    fn a_body_stands_on_a_floor_and_not_in_it() {
        let g = field([3, 4, 3]);
        assert!(standable(&g, [1, 1, 1]));
        assert!(!standable(&g, [1, 0, 1]), "inside the floor");
        assert!(!standable(&g, [1, 3, 1]), "nothing underfoot");
        assert_eq!(standable_cells(&g).len(), 9);
    }

    /// Outside the box is never passable and never solid — a body that has left
    /// the box has left the thing being measured, and a cell beyond the wall is
    /// not a floor to land on.
    #[test]
    fn outside_the_box_is_neither_passable_nor_solid() {
        let g = field([3, 4, 3]);
        assert!(!g.passable([-1, 1, 1]));
        assert!(!solid(&g, [-1, 1, 1]));
        assert!(!standable(&g, [1, 1, -1]));
    }

    /// A wall splits the floor into two lumps — while a one-block step over a
    /// sill joins them.
    #[test]
    fn a_wall_severs_and_a_sill_does_not() {
        let mut g = field([5, 4, 3]);
        for z in 0..3 {
            for y in 1..4 {
                g.solid.insert([2, y, z]);
            }
        }
        let cells = standable_cells(&g);
        assert_eq!(components(&g, &cells).len(), 2);

        // knock the wall down to a single course: now it is a step, not a wall.
        for z in 0..3 {
            g.solid.remove(&[2, 2, z]);
            g.solid.remove(&[2, 3, z]);
        }
        let cells = standable_cells(&g);
        assert_eq!(components(&g, &cells).len(), 1);
    }

    /// A roof over part of the floor is what separates indoors from the ground
    /// outside a free-standing building. Nothing else in the geometry does.
    #[test]
    fn shelter_separates_indoors_from_the_open_ground_beside_it() {
        let mut g = field([5, 6, 3]);
        for z in 0..3 {
            for x in 0..2 {
                g.solid.insert([x, 4, z]);
            }
        }
        assert!(sheltered(&g, [0, 1, 1]), "under the roof");
        assert!(!sheltered(&g, [4, 1, 1]), "open sky above");
    }

    /// A sealed box has no ground entry at all, and that is a real answer: the
    /// caller reports a binding of zero rather than a reachability of zero.
    #[test]
    fn a_sealed_box_has_no_entrance_and_an_opening_gives_it_one() {
        let mut g = field([5, 5, 5]);
        for p in positions([0, 0, 0], [5, 5, 5]) {
            let shell = p[1] == 4 || p[0] == 0 || p[0] == 4 || p[2] == 0 || p[2] == 4;
            if shell && p[1] > 0 {
                g.solid.insert(p);
            }
        }
        assert!(ground_entry(&g).is_empty());

        // carve a doorway through one wall at grade.
        g.solid.remove(&[0, 1, 2]);
        g.solid.remove(&[0, 2, 2]);
        let entry = ground_entry(&g);
        assert_eq!(entry, BTreeSet::from([[0, 1, 2]]));
    }
}
