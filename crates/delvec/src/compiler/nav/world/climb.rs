//! **How a body climbs** (spec-0099): which cells hold a climbing body, and the
//! moves a climb adds to the walk, the fall and the route.
//!
//! A climbable cell is empty to a body ([`delvewright_dsl::blockshape::Collision::Climbable`]):
//! a ladder's panel stands against its support, a vine has no box. What makes it
//! a place is the climb — vanilla gives a body whose feet are in one a vertical
//! speed while it pushes or holds jump, slows its fall to a slide, holds it still
//! while it sneaks, and forgets the fall above it ([`delvewright_dsl::metrics`]'s
//! climb constants carry the citations). So the model carries a fourth kind of
//! cell beside standing, floating and falling: **holding**, in a climb cell
//! ([`World::climb_cell_fp`]), and every proof that moves a body moves it
//! through these cells by the arms below and by nothing else.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use delvewright_dsl::metrics::{FULL_16, climb_catch_fall_blocks};

use crate::compiler::assembled::{ClimbHold, keep_climbs};
use crate::compiler::nav::world::{Footprint, World};

/// The climbables the assembled world keeps, with what keeps each — shared by
/// every view derived from one world.
pub type ClimbHolds = Arc<BTreeMap<[i32; 3], ClimbHold>>;

/// The climbables the block map holds and the world does not keep, with their
/// blocks.
pub type UnheldClimbs = Arc<BTreeMap<[i32; 3], String>>;

/// The four cardinal horizontal steps, in the fixed order every arm here walks
/// them (ADR-0006).
const HORIZ: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

/// One climb a route takes: the cells it holds on, from the cell it enters at
/// to the cell it leaves from, and the block it climbs. What the waypoint
/// export carries per leg, so the harness drives the climb the compiler proved
/// rather than discovering one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClimbRun {
    /// The route cell the body is in just before it takes hold: where it
    /// stands (or holds) before the first climb move.
    pub from: [i32; 3],
    /// The route cell it is in just after it lets go.
    pub to: [i32; 3],
    /// The climb cells it holds in, in route order.
    pub cells: Vec<[i32; 3]>,
    /// The block of the first climb cell, as the map holds it.
    pub block: String,
    /// For a ladder: the way it faces. A body pushing against it faces the
    /// opposite way.
    pub facing: Option<delvewright_dsl::blockshape::Face>,
}

impl World {
    /// **Can a body with footprint `fp` hold on a climb in `c`?** Its feet cell
    /// is a climbable the world keeps (vanilla reads only the feet cell:
    /// `onClimbable()` asks the block at `blockPosition()`), every cell of its
    /// body is unoccupied, and no declared killing volume reaches it.
    ///
    /// A climb cell need not be standable: a body halfway up a ladder stands on
    /// nothing. One that is (the foot of a ladder on a floor) is both.
    pub(in crate::compiler::nav) fn climb_cell_fp(&self, c: [i32; 3], fp: &Footprint) -> bool {
        self.climb.contains(&c)
            && !self.meets_lethal_fp(c, fp)
            && fp.cols.iter().all(|&[dx, dz]| {
                (0..fp.height).all(|dy| !self.is_occupied([c[0] + dx, c[1] + dy, c[2] + dz]))
            })
    }

    /// [`World::climb_cell_fp`] at the player's footprint.
    pub fn is_climb_cell(&self, c: [i32; 3]) -> bool {
        self.climb_cell_fp(c, &Footprint::player())
    }

    /// **Can a body be in `c` at all** — standing on a floor or holding on a
    /// climb. Every cell a route proof puts a body in answers yes.
    pub fn holds_body(&self, c: [i32; 3]) -> bool {
        self.is_standable(c) || self.is_climb_cell(c)
    }

    /// Whether the world holds any climbable at all — the cheap guard that keeps
    /// a campaign without one on exactly its old moves.
    pub fn has_climbs(&self) -> bool {
        !self.climb.is_empty()
    }

    /// How many climbables this view keeps, and how many the block map held
    /// that it does not — the binding counts a climb proof states.
    pub fn climb_census(&self) -> (usize, usize) {
        (self.climb.len(), self.unheld_climb.len())
    }

    /// The climbables the block map holds and the world does not keep, in
    /// cell order, with their blocks.
    pub fn unheld_climbs(&self) -> impl Iterator<Item = ([i32; 3], &str)> {
        self.unheld_climb.iter().map(|(c, b)| (*c, b.as_str()))
    }

    /// Whether the block map held any climbable the world does not keep.
    pub(in crate::compiler::nav) fn has_unheld_climbs(&self) -> bool {
        !self.unheld_climb.is_empty()
    }

    /// The unheld climbables among `cells`, in words: each cell, its block, and
    /// the hold its rule asks for (spec-0099 §3.2).
    pub(in crate::compiler::nav) fn unheld_climbs_words(&self, cells: &[[i32; 3]]) -> String {
        use delvewright_dsl::blockshape::{Climbable, climbable};
        let words: Vec<String> = cells
            .iter()
            .filter_map(|c| self.unheld_climb.get(c).map(|b| (*c, b)))
            .map(|(c, b)| {
                let needs = match climbable(b) {
                    Some(Climbable::Ladder { facing }) => {
                        let o = facing.opposite().offset();
                        format!(
                            "a sturdy {} face on the block at {:?}, behind it",
                            facing.name(),
                            [c[0] + o[0], c[1] + o[1], c[2] + o[2]]
                        )
                    }
                    Some(Climbable::Vine { .. }) => {
                        "a full face beside it on a face it sets, or the vine above carrying the \
                         same face"
                            .to_string()
                    }
                    Some(Climbable::Growing(g)) => {
                        let o = g.growth().opposite().offset();
                        format!(
                            "the same plant, or a sturdy {} face, at {:?}",
                            g.growth().name(),
                            [c[0] + o[0], c[1] + o[1], c[2] + o[2]]
                        )
                    }
                    _ => "a hold".to_string(),
                };
                format!("`{b}` at {c:?}, which needs {needs}")
            })
            .collect();
        if words.is_empty() {
            "a climbable the route reaches through a cell its counterfactual credited".to_string()
        } else {
            words.join("; ")
        }
    }

    /// This view with the unheld climbables credited as if they hung where they
    /// are — the counterfactual `DW0991` routes over to learn that a missing
    /// hold, not the geometry, closed a leg. Only ever a question, never a
    /// world a proof passes over.
    ///
    /// Each credited climbable is held by its own cell, so it survives every
    /// runtime write that leaves it alone and goes with one that overwrites it
    /// — the counterfactual changes the hold and nothing else.
    pub(in crate::compiler::nav) fn with_unheld_climbs(&self) -> World {
        let mut w = self.with_cleared(&BTreeSet::new());
        if self.unheld_climb.is_empty() {
            return w;
        }
        let mut holds: BTreeMap<[i32; 3], ClimbHold> = (*self.climb_holds).clone();
        for (c, block) in self.unheld_climb.iter() {
            let Some(kind) = delvewright_dsl::blockshape::climbable(block) else {
                continue;
            };
            holds.insert(
                *c,
                ClimbHold {
                    block: block.clone(),
                    kind,
                    holds: vec![(None, crate::compiler::assembled::Hold::Block(*c))],
                },
            );
            w.climb.insert(*c);
        }
        w.climb_holds = Arc::new(holds);
        w
    }

    /// Take away every climbable a runtime write leaves with nothing to hang
    /// on: one whose own cell the write touched, or whose hold it did —
    /// transitively, to the same fixed point the assembled world was read to
    /// ([`keep_climbs`]). A write is read as removing whatever it touches,
    /// whatever block it lays: the derived world carries cells, not block
    /// states, so a fill that relaid a sturdy face is credited with nothing.
    /// That can only take a climb away the game keeps, never keep one it
    /// removes.
    pub(in crate::compiler::nav) fn drop_unheld_climbs(&mut self, touched: &BTreeSet<[i32; 3]>) {
        if self.climb.is_empty() || touched.is_empty() {
            return;
        }
        let alive: BTreeSet<[i32; 3]> = self.climb.iter().collect();
        let kept = keep_climbs(alive, &self.climb_holds, touched);
        let mut climb = crate::compiler::cellset::CellSet::new();
        for c in kept {
            climb.insert(c);
        }
        self.climb = climb;
    }

    /// The climb's own moves from `c`, for a body with footprint `fp` — every
    /// one a thing a body does **on cue** (spec-0099 §4.1), so the route proof
    /// may take them:
    ///
    /// * **up** and **down** a climb column, a cell at a time;
    /// * **off the side**, onto a standable cell level with the climb cell or
    ///   one lower, the body's column clear over it;
    /// * **over the top**: from the top climb cell, onto a standable cell one
    ///   higher beside it — the body rises clear of the climbable by at least
    ///   `0.15` blocks before it falls back (`climb_blocks_per_tick` then one
    ///   tick of gravity), which is over any floor whose top is the climb cell's
    ///   top, and the column above the climb cell must hold the risen body;
    /// * **off the bottom**, onto a standable cell directly under a climb cell
    ///   that stands on nothing — a one-block drop;
    /// * **in**, from a standable cell, onto a climb cell beside it at the same
    ///   level, or one lower when the cell over it is clear (a step off the
    ///   brink that the climbable catches within a block).
    ///
    /// The fall onto a climb from higher up is not here: a body does not fall on
    /// cue. [`World::body_moves`] takes it ([`World::catch_fp`]).
    pub(in crate::compiler::nav) fn climb_moves_fp(
        &self,
        c: [i32; 3],
        fp: &Footprint,
    ) -> Vec<[i32; 3]> {
        if self.climb.is_empty() {
            return Vec::new();
        }
        let clear = |x: i32, y: i32, z: i32| {
            fp.cols
                .iter()
                .all(|&[dx, dz]| !self.is_occupied([x + dx, y, z + dz]))
        };
        let mut out = Vec::new();
        if self.climb_cell_fp(c, fp) {
            let up = [c[0], c[1] + 1, c[2]];
            let down = [c[0], c[1] - 1, c[2]];
            if self.climb_cell_fp(up, fp) {
                out.push(up);
            }
            // Down the run, or off its bottom onto the floor one under it.
            if self.climb_cell_fp(down, fp)
                || (!self.standable_fp(c, fp) && self.standable_fp(down, fp))
            {
                out.push(down);
            }
            let top = !self.climb_cell_fp(up, fp)
                && (1..=fp.height).all(|dy| clear(c[0], c[1] + dy, c[2]));
            for (dx, dz) in HORIZ {
                let level = [c[0] + dx, c[1], c[2] + dz];
                if self.standable_fp(level, fp) {
                    out.push(level);
                }
                let lower = [c[0] + dx, c[1] - 1, c[2] + dz];
                if self.standable_fp(lower, fp)
                    && (0..fp.height).all(|dy| clear(lower[0], c[1] + dy, lower[2]))
                {
                    out.push(lower);
                }
                let over = [c[0] + dx, c[1] + 1, c[2] + dz];
                if top && self.standable_fp(over, fp) {
                    out.push(over);
                }
            }
        }
        if self.standable_fp(c, fp) {
            for (dx, dz) in HORIZ {
                let level = [c[0] + dx, c[1], c[2] + dz];
                if self.climb_cell_fp(level, fp) {
                    out.push(level);
                }
                let below = [c[0] + dx, c[1] - 1, c[2] + dz];
                if self.climb_cell_fp(below, fp)
                    && !self.climb_cell_fp(level, fp)
                    && (0..fp.height).all(|dy| clear(level[0], c[1] + dy, level[2]))
                {
                    out.push(below);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// The climb cell a body dropping down column `(x, z)` is **caught** in, if
    /// it is caught at all: the first climb cell at or under `top` whose top
    /// face is no more than [`climb_catch_fall_blocks`] under the body's feet
    /// (`from`, in sixteenths) — a body falling that far still moves less than a
    /// block a tick, so its feet are inside the cell on some tick and vanilla
    /// stops the fall there. `None` when the column meets anything occupied
    /// first, or no climb cell within the bound.
    pub(in crate::compiler::nav) fn catch_fp(
        &self,
        x: i32,
        z: i32,
        top: i32,
        from: i64,
        fp: &Footprint,
    ) -> Option<[i32; 3]> {
        if self.climb.is_empty() {
            return None;
        }
        let bound = i64::from(climb_catch_fall_blocks()) * FULL_16;
        let mut y = top;
        while from - (i64::from(y) + 1) * FULL_16 <= bound {
            let cell = [x, y, z];
            if self.is_occupied(cell) {
                return None;
            }
            if self.climb_cell_fp(cell, fp) {
                return Some(cell);
            }
            y -= 1;
        }
        None
    }

    /// The bottom of the climb run `c` is in: the lowest climb cell reached by
    /// sliding down from `c` without letting go.
    pub(in crate::compiler::nav) fn climb_bottom(&self, c: [i32; 3]) -> [i32; 3] {
        let fp = Footprint::player();
        let mut b = c;
        while self.climb_cell_fp([b[0], b[1] - 1, b[2]], &fp) {
            b[1] -= 1;
        }
        b
    }

    /// The climbs a route takes, as runs: each maximal stretch of consecutive
    /// route cells joined by a climb move ([`World::climb_moves_fp`]) rather
    /// than a walk, cut at every standable cell the stretch passes through after
    /// it has held on. Empty for a route that climbs nothing — every route in a
    /// world without a climbable.
    ///
    /// **A run is one column.** A climb move never crosses columns while the
    /// body holds on; it changes column only by letting go onto a standable cell
    /// (off the side, over the top) and taking hold again. So a stretch that
    /// tops out of one ladder straight onto the foot of the next is two climbs:
    /// the first lets go onto that cell (which it does not hold in), the second
    /// takes hold from it. The harness drives a climb up or down one column
    /// (`harness/src/executor/climb.ts`) and refuses a record whose lowest and
    /// highest held cells are not one column.
    pub fn climb_runs(&self, cells: &[[i32; 3]]) -> Vec<ClimbRun> {
        if self.climb.is_empty() || cells.len() < 2 {
            return Vec::new();
        }
        let fp = Footprint::player();
        // A step is a walk when the body stands at `a` and the walk takes it to
        // `b`; any other step touching a climb cell is the climb's.
        let is_climb_step = |a: [i32; 3], b: [i32; 3]| {
            (self.climb_cell_fp(a, &fp) || self.climb_cell_fp(b, &fp))
                && !(self.standable_fp(a, &fp) && self.neighbors_walk_fp(a, &fp).contains(&b))
        };
        let mut out: Vec<ClimbRun> = Vec::new();
        let mut i = 0;
        while i + 1 < cells.len() {
            if !is_climb_step(cells[i], cells[i + 1]) {
                i += 1;
                continue;
            }
            let mut start = i;
            while i + 1 < cells.len() && is_climb_step(cells[i], cells[i + 1]) {
                i += 1;
                // A standable cell inside the stretch, reached after the body
                // has held on, is where one climb lets go and the next takes
                // hold — unless the stretch ends here anyway.
                let held_before = cells[start..i].iter().any(|c| self.climb_cell_fp(*c, &fp));
                if i + 1 < cells.len()
                    && is_climb_step(cells[i], cells[i + 1])
                    && held_before
                    && self.standable_fp(cells[i], &fp)
                {
                    out.push(self.climb_run(&cells[start..=i], false, &fp));
                    start = i;
                }
            }
            out.push(self.climb_run(&cells[start..=i], true, &fp));
        }
        out
    }

    /// One [`ClimbRun`] over the route stretch `span`, from its first cell to
    /// its last. `holds_last` is false when the last cell is a standable cell
    /// the body lets go onto to take hold of the next climb: it is that climb's
    /// cell, not this one's.
    fn climb_run(&self, span: &[[i32; 3]], holds_last: bool, fp: &Footprint) -> ClimbRun {
        let last = span.len() - 1;
        let held: Vec<[i32; 3]> = span
            .iter()
            .enumerate()
            .filter(|&(k, c)| (holds_last || k != last) && self.climb_cell_fp(*c, fp))
            .map(|(_, c)| *c)
            .collect();
        let first = held.first().copied().unwrap_or(span[0]);
        let hold = self.climb_holds.get(&first);
        ClimbRun {
            from: span[0],
            to: span[last],
            cells: held,
            block: hold.map(|h| h.block.clone()).unwrap_or_default(),
            facing: hold.and_then(|h| h.ladder_facing()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::nav::testkit::blocks_world;
    use crate::compiler::nav::{Premises, World};
    use std::collections::BTreeMap;

    /// A floor of stone at y = 64 over x 0..=6, z 0..=2, and a solid mass at
    /// x 3..=6 rising to y = 68, so its top is stood on at y = 69 — four courses
    /// over the floor, past any step or jump. `ladder` is laid on the mass's
    /// west face, x = 2, y 65..=68, at z = 1, in the block state given.
    fn cliff(ladder: Option<&str>, mass: &str) -> Vec<([i32; 3], String)> {
        let mut cells: Vec<([i32; 3], String)> = Vec::new();
        for x in 0..=6 {
            for z in 0..=2 {
                cells.push(([x, 64, z], "minecraft:stone".to_string()));
                if x >= 3 {
                    for y in 65..=68 {
                        cells.push(([x, y, z], mass.to_string()));
                    }
                }
            }
        }
        if let Some(l) = ladder {
            for y in 65..=68 {
                cells.push(([2, y, 1], l.to_string()));
            }
        }
        cells
    }

    fn world(cells: &[([i32; 3], String)]) -> World {
        let refs: Vec<([i32; 3], &str)> = cells.iter().map(|(c, n)| (*c, n.as_str())).collect();
        blocks_world(&refs)
    }

    const LADDER_WEST: &str = "minecraft:ladder[facing=west]";

    /// **A body climbs a ladder, and only a ladder that hangs.** Up a four-course
    /// face no step or jump reaches: with a ladder on it the route exists and
    /// holds in every ladder cell, and over the top onto the mass; without one
    /// it does not; with the ladder turned to face INTO the mass — its back to
    /// the open floor, nothing behind it — the world does not keep it and the
    /// route is gone again.
    #[test]
    fn a_body_climbs_a_ladder_that_hangs() {
        let (foot, top) = ([1, 65, 1], [4, 69, 1]);
        let w = world(&cliff(Some(LADDER_WEST), "minecraft:stone"));
        assert_eq!(w.climb_census(), (4, 0));
        let path = w.find_path(foot, top).expect("the ladder is a way up");
        for y in 65..=68 {
            assert!(
                path.contains(&[2, y, 1]),
                "the route holds at [2, {y}, 1]: {path:?}"
            );
            assert!(w.is_climb_cell([2, y, 1]));
        }
        assert!(
            !w.is_standable([2, 67, 1]),
            "halfway up a ladder stands on nothing"
        );
        assert!(w.holds_body([2, 67, 1]));

        let bare = world(&cliff(None, "minecraft:stone"));
        assert!(
            bare.find_path(foot, top).is_none(),
            "four courses are no step"
        );

        let backwards = world(&cliff(
            Some("minecraft:ladder[facing=east]"),
            "minecraft:stone",
        ));
        assert_eq!(backwards.climb_census(), (0, 4), "nothing is behind it");
        assert!(backwards.find_path(foot, top).is_none());
        assert!(
            backwards
                .with_unheld_climbs()
                .find_path(foot, top)
                .is_some()
        );
    }

    /// **The face is the jar's.** Leaves have an empty support shape and a full
    /// collision box: a ladder on them does not hang, a vine on them does.
    #[test]
    fn leaves_hold_a_vine_and_not_a_ladder() {
        let ladder = world(&cliff(
            Some(LADDER_WEST),
            "minecraft:oak_leaves[persistent=true]",
        ));
        assert_eq!(ladder.climb_census(), (0, 4));
        let vine = world(&cliff(
            Some("minecraft:vine[east=true]"),
            "minecraft:oak_leaves[persistent=true]",
        ));
        assert_eq!(vine.climb_census(), (4, 0));
        assert!(vine.find_path([1, 65, 1], [4, 69, 1]).is_some());
    }

    /// **A chain hangs from its top or not at all.** Weeping vines grown down
    /// from a ceiling are kept; the same column with its ceiling taken away is
    /// kept nowhere — the game's own check reads one block up, and the update
    /// that removes the top reaches each one below in turn.
    #[test]
    fn a_hanging_chain_is_kept_from_its_top() {
        let mut cells: Vec<([i32; 3], &str)> = vec![([0, 70, 0], "minecraft:stone")];
        for y in 65..=69 {
            cells.push(([0, y, 0], "minecraft:weeping_vines_plant"));
        }
        assert_eq!(blocks_world(&cells).climb_census(), (5, 0));
        cells.remove(0);
        assert_eq!(blocks_world(&cells).climb_census(), (0, 5));
        // A vine face held by the vine above it, to a held top.
        let mut vines: Vec<([i32; 3], &str)> = vec![([1, 69, 0], "minecraft:stone")];
        for y in 66..=69 {
            vines.push(([0, y, 0], "minecraft:vine[east=true]"));
        }
        vines.push(([1, 66, 0], "minecraft:air"));
        let w = blocks_world(&vines);
        assert_eq!(w.climb_census(), (4, 0), "three hang by the face above");
    }

    /// **A fall onto a climbable stops there, within the catch bound.** A body
    /// stepping off a ledge over a ladder column is caught by the first ladder
    /// cell it reaches, however far down the ledge stood, as long as that is
    /// within [`climb_catch_fall_blocks`] — and a body that does not hold on
    /// slides to the bottom of the run and falls on from there.
    #[test]
    fn a_ladder_catches_a_fall_and_the_body_slides_on() {
        let catch = climb_catch_fall_blocks() as i32;
        assert_eq!(catch, 7, "the derived bound");
        // A wall of stone at x = 1, z -1..=1, from y = 40 up to y = 60, a ledge
        // at x = 0 level with its top, and a ladder down the wall's east face in
        // column x = 2, z = 0, y 50..=55, over void.
        let mut cells: Vec<([i32; 3], String)> = Vec::new();
        for z in -1..=1 {
            for y in 40..=60 {
                cells.push(([1, y, z], "minecraft:stone".to_string()));
            }
            cells.push(([0, 60, z], "minecraft:stone".to_string()));
        }
        for y in 50..=55 {
            cells.push(([2, y, 0], "minecraft:ladder[facing=east]".to_string()));
        }
        let w = world(&cells);
        assert_eq!(w.climb_census(), (6, 0));
        // From the pillar top ([1, 61, 0]) a body steps east into column 2 and
        // falls: the ladder top at y = 55 is 5 blocks under its feet — caught.
        assert!(w.body_moves([1, 61, 0]).contains(&[2, 55, 0]));
        // Nothing under the run: the step off is fatal all the same, because a
        // body that does not hold on slides off the bottom into the void.
        assert_eq!(
            w.fatal_step_off([1, 61, 0]).map(|(_, lava)| lava),
            Some(false)
        );
        // Put a floor under the run and the same step is survived.
        let mut floored = cells.clone();
        floored.push(([2, 48, 0], "minecraft:stone".to_string()));
        let f = world(&floored);
        assert_eq!(f.fatal_step_off([1, 61, 0]), None, "caught, slid, stood");
    }

    /// **A runtime write that takes the support takes the ladder.** Clearing the
    /// block behind one ladder cell drops that cell; the cells above it still
    /// hang on their own support.
    #[test]
    fn clearing_the_support_drops_the_climb() {
        let w = world(&cliff(Some(LADDER_WEST), "minecraft:stone"));
        let cleared = w.with_cleared(&[[3, 66, 1]].into_iter().collect());
        assert!(!cleared.climb.contains(&[2, 66, 1]));
        assert!(cleared.climb.contains(&[2, 67, 1]));
        assert!(
            cleared.find_path([1, 65, 1], [4, 69, 1]).is_none(),
            "a rung is gone"
        );
    }

    /// **A world with no climbable moves exactly as before.** The guard every arm
    /// takes: no climb cell, no climb move, no catch.
    #[test]
    fn no_climbable_no_climb() {
        let w = World::from_occupancy(
            crate::compiler::assembled::occupancy_of(
                BTreeMap::from([([0, 64, 0], "minecraft:stone".to_string())]),
                &BTreeSet::new(),
            ),
            Premises::geometry_only(),
        );
        assert!(!w.has_climbs());
        assert!(
            w.climb_moves_fp([0, 65, 0], &Footprint::player())
                .is_empty()
        );
        assert_eq!(w.climb_runs(&[[0, 65, 0]]), Vec::new());
    }

    /// **The runs a route takes are read off it.** One run up the ladder, from
    /// the floor cell the body takes hold at to the mass top it steps onto.
    #[test]
    fn the_route_names_its_climb() {
        let w = world(&cliff(Some(LADDER_WEST), "minecraft:stone"));
        let path = w.find_path([0, 65, 1], [5, 69, 1]).expect("routes");
        let runs = w.climb_runs(&path);
        assert_eq!(runs.len(), 1, "{runs:?}");
        let r = &runs[0];
        assert_eq!(r.to, [3, 69, 1]);
        assert_eq!(r.cells.first(), Some(&[2, 65, 1]));
        assert_eq!(r.cells.last(), Some(&[2, 68, 1]));
        assert_eq!(r.facing, Some(delvewright_dsl::blockshape::Face::West));
        assert!(r.block.starts_with("minecraft:ladder"));
    }

    /// **A stack of two ladders is two climbs, each one column.** The lower
    /// ladder (x = 2, y 65..=68) tops out over a step onto `[3, 69, 1]` — a
    /// floor on the lower mass, and the foot of the upper ladder (x = 3,
    /// y 69..=72) hung on the upper mass. The route never walks between them:
    /// it lets go straight onto the second ladder's foot and takes hold again.
    /// The export is two runs, the first letting go at the shared cell and the
    /// second taking hold from it; neither holds in two columns.
    #[test]
    fn a_stack_of_two_ladders_is_two_one_column_climbs() {
        let mut cells = cliff(Some(LADDER_WEST), "minecraft:stone");
        for x in 4..=6 {
            for z in 0..=2 {
                for y in 69..=72 {
                    cells.push(([x, y, z], "minecraft:stone".to_string()));
                }
            }
        }
        for y in 69..=72 {
            cells.push(([3, y, 1], LADDER_WEST.to_string()));
        }
        let w = world(&cells);
        let path = w
            .find_path([0, 65, 1], [5, 73, 1])
            .expect("the stack routes");
        let shared = [3, 69, 1];
        assert!(w.is_standable(shared) && w.is_climb_cell(shared));
        let runs = w.climb_runs(&path);
        assert_eq!(runs.len(), 2, "{path:?} -> {runs:?}");
        for r in &runs {
            let cols: BTreeSet<(i32, i32)> = r.cells.iter().map(|c| (c[0], c[2])).collect();
            assert_eq!(cols.len(), 1, "one column per climb: {r:?}");
        }
        assert_eq!(runs[0].to, shared);
        assert!(!runs[0].cells.contains(&shared));
        assert_eq!(runs[1].from, shared);
        assert_eq!(runs[1].cells.first(), Some(&shared));
        assert_eq!(runs[1].cells.last(), Some(&[3, 72, 1]));
    }
}
