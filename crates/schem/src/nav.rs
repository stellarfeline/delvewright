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
//! and the light probe in `delve-admit` could not use the walk that
//! `delvewright-grammar` had already written.
//!
//! A capability belongs to the object class it acts on (CLAUDE.md), and the
//! object class is not `VoxelModel` and not `Structure` — it is **a box of cells
//! with a passability answer for each**. That is [`Voxels`], and it is small
//! enough that any producer can implement it in a dozen lines. This crate is
//! where it goes because it is the one both the grammar back end and the
//! admission pipeline already depend on, and because it already owns the other
//! two facts shared at this layer: the block registry and the tile-set contract.
//!
//! # What the walk is, and is not
//!
//! One walker: one cell horizontally at a time, stepping at most one block up or
//! down, and — under [`reachable_with_fall`] only — walking off a ledge and
//! landing on the first floor below.
//!
//! The step rule is not written here. It is [`delvewright_dsl::metrics::step_allowed`],
//! which is also what `delvec`'s navigation model asks, because there is one
//! answer to "can a body get from here to there" and a second implementation of
//! it is a second opinion two gates can disagree about. **This walk had one.** It
//! stepped a whole block up with no headroom condition, so it connected a rise
//! the router refuses — and a prefab's contract-reachability gate proves a
//! *positive* claim over this walk, which is the direction in which being looser
//! than the router admits a piece the compiler will later strand.
//!
//! What is genuinely this module's is the **measurement** of a rise. A box of
//! cells has no collision heights, so a step of one cell is read as a full block
//! (16/16) unless the implementor knows better and says so through
//! [`Voxels::floor_top_16`]. Reading a partial floor as a full one only ever
//! over-states a rise, so the default is the refusing direction — see that
//! method for what it costs and who should override it.
//!
//! **Where this walk is still deliberately generous, it says so per function.**
//! [`reachable_with_fall`] adds a one-way fall edge the router has no model of,
//! and its own doc records the two directions it may and may not be used in.

use std::collections::{BTreeSet, VecDeque};

use delvewright_dsl::metrics::{FULL_16, step_allowed};

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

/// Every standable cell of the box.
pub fn standable_cells<V: Voxels + ?Sized>(v: &V) -> BTreeSet<[i32; 3]> {
    positions(v.origin(), v.size())
        .filter(|&p| standable(v, p))
        .collect()
}

/// Can a walker get from any cell of `from` to any cell of `to`, moving one cell
/// horizontally at a time and stepping at most one cell up or down — with every
/// step decided by [`delvewright_dsl::metrics::step_allowed`]?
///
/// The box is a parameter because the step rule needs one fact the standable set
/// cannot carry: whether the cell a jumping body's head sweeps through is clear.
/// That is the term this walk used to be missing.
pub fn connected<V: Voxels + ?Sized>(
    v: &V,
    cells: &BTreeSet<[i32; 3]>,
    from: &BTreeSet<[i32; 3]>,
    to: &BTreeSet<[i32; 3]>,
) -> bool {
    let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut queue: VecDeque<[i32; 3]> =
        from.iter().copied().filter(|c| cells.contains(c)).collect();
    seen.extend(queue.iter().copied());
    while let Some(cur) = queue.pop_front() {
        if to.contains(&cur) {
            return true;
        }
        let [x, y, z] = cur;
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            for dy in [0, 1, -1] {
                let next = [x + dx, y + dy, z + dz];
                if cells.contains(&next) && can_step(v, cur, next) && seen.insert(next) {
                    queue.push_back(next);
                }
            }
        }
    }
    false
}

/// Every cell of `cells` a walker can reach from `seeds`, under [`connected`]'s
/// walk. Seeds outside `cells` are dropped: a body standing nowhere reaches
/// nowhere.
///
/// This is the set form of [`connected`], and it is the one a *measurement*
/// wants: a probe that reports a minimum over player space has to know how many
/// cells it bound to, not merely whether the set was non-empty.
pub fn reachable_from<V: Voxels + ?Sized>(
    v: &V,
    cells: &BTreeSet<[i32; 3]>,
    seeds: &BTreeSet<[i32; 3]>,
) -> BTreeSet<[i32; 3]> {
    let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut queue: VecDeque<[i32; 3]> = seeds
        .iter()
        .copied()
        .filter(|c| cells.contains(c))
        .collect();
    seen.extend(queue.iter().copied());
    while let Some(cur) = queue.pop_front() {
        let [x, y, z] = cur;
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            for dy in [0, 1, -1] {
                let next = [x + dx, y + dy, z + dz];
                if cells.contains(&next) && can_step(v, cur, next) && seen.insert(next) {
                    queue.push_back(next);
                }
            }
        }
    }
    seen
}

/// [`connected`]'s ±1-step walk, plus a one-way **fall**: stepping off a
/// standable cell into an adjacent column with nothing underfoot, and landing on
/// the first solid floor below however far that is.
///
/// Deliberately *more* permissive than [`connected`], which is why the two are
/// used in opposite directions and never interchangeably: forward, where a piece
/// is entered by stepping off a ledge and `connected` alone would call it
/// severed; and as the adversary, when a piece claims a route is the only route,
/// since a fall edge can carry a body past a wall a walker would have to go
/// round. The negative direction — "there is no way back up" — is asserted under
/// the plain walk instead, because a fall edge only ever points down and proving
/// a negative under the generous model would be circular.
///
/// A landing must be a member of `cells`, not merely standable in the box: a
/// gate proves a route is the only route by deleting cells from the graph, and a
/// fall allowed to land on a deleted cell would walk straight through the cut.
pub fn reachable_with_fall<V: Voxels + ?Sized>(
    v: &V,
    cells: &BTreeSet<[i32; 3]>,
    from: &BTreeSet<[i32; 3]>,
    to: &BTreeSet<[i32; 3]>,
) -> bool {
    let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut queue: VecDeque<[i32; 3]> =
        from.iter().copied().filter(|c| cells.contains(c)).collect();
    seen.extend(queue.iter().copied());
    while let Some(cur) = queue.pop_front() {
        if to.contains(&cur) {
            return true;
        }
        let [x, y, z] = cur;
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            for dy in [0, 1, -1] {
                let next = [x + dx, y + dy, z + dz];
                if cells.contains(&next) && can_step(v, cur, next) && seen.insert(next) {
                    queue.push_back(next);
                }
            }
            let mut fy = y;
            loop {
                fy -= 1;
                if y - fy > 64 {
                    break; // not a real shaft, a runaway search
                }
                let below = [x + dx, fy, z + dz];
                if !v.contains(below) {
                    break; // fell out of the box entirely
                }
                if solid(v, below) {
                    let landing = [x + dx, fy + 1, z + dz];
                    if cells.contains(&landing) && seen.insert(landing) {
                        queue.push_back(landing);
                    }
                    break;
                }
            }
        }
    }
    false
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
/// cells a body actually reaches is [`reachable_from`]**, from real entrances,
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
    /// (`delvewright_compiler::nav`, `step_up_needs_head_clearance_to_jump`),
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

    #[test]
    fn a_full_block_rise_is_a_jump_and_needs_the_swept_head_cell_clear() {
        let open = head_bonk(false);
        let cells = standable_cells(&open);
        assert!(cells.contains(&[0, 1, 0]) && cells.contains(&[2, 2, 0]));
        assert!(
            connected(&open, &cells, &BTreeSet::from([[0, 1, 0]]), &BTreeSet::from([[2, 2, 0]])),
            "open headroom: the jump up is walkable"
        );

        let low = head_bonk(true);
        let cells = standable_cells(&low);
        assert!(
            cells.contains(&[0, 1, 0]) && cells.contains(&[2, 2, 0]),
            "both ends are standable — the geometry differs only in the swept cell"
        );
        assert!(
            !connected(&low, &cells, &BTreeSet::from([[0, 1, 0]]), &BTreeSet::from([[2, 2, 0]])),
            "a ceiling two courses over the feet blocks the jump, so no walk connects them"
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

    /// A wall splits the floor into two components, and the walk refuses to
    /// cross it — while a one-block step over a sill does cross.
    #[test]
    fn a_wall_severs_and_a_sill_does_not() {
        let mut g = field([5, 4, 3]);
        for z in 0..3 {
            for y in 1..4 {
                g.solid.insert([2, y, z]);
            }
        }
        let cells = standable_cells(&g);
        let west: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[0] < 2).collect();
        let east: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[0] > 2).collect();
        assert!(!connected(&g, &cells, &west, &east));
        assert_eq!(components(&g, &cells).len(), 2);

        // knock the wall down to a single course: now it is a step, not a wall.
        for z in 0..3 {
            g.solid.remove(&[2, 2, z]);
            g.solid.remove(&[2, 3, z]);
        }
        let cells = standable_cells(&g);
        assert!(connected(&g, &cells, &west, &east));
        assert_eq!(components(&g, &cells).len(), 1);
    }

    /// `reachable_from` is `connected`'s set form: same edges, and it reports
    /// how many cells were reached — the number a measurement's binding is.
    #[test]
    fn reachable_from_reports_the_set_it_bound_to() {
        let mut g = field([5, 4, 3]);
        for z in 0..3 {
            for y in 1..4 {
                g.solid.insert([2, y, z]);
            }
        }
        let cells = standable_cells(&g);
        let west: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[0] < 2).collect();
        assert_eq!(reachable_from(&g, &cells, &west).len(), 6);
        assert_eq!(reachable_from(&g, &cells, &BTreeSet::new()).len(), 0);
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
        let cells = standable_cells(&g);
        assert_eq!(
            reachable_from(&g, &cells, &entry).len(),
            10,
            "doorway + interior"
        );
    }
}
