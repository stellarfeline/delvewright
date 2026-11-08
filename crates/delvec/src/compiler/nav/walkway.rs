//! **A staged walk follows the declared ways**: an NPC's or an actor's walk
//! between places keeps to the roads, streets, flights and bridges the site
//! plan declares (`boxes[].way`), and steps off them only to reach or leave
//! one, or where none joins its ends.
//!
//! # The model, cited and authored
//!
//! - **An area cost on the one router** (cited: per-area traversal costs on a
//!   navigation graph, Recast/Detour's `dtQueryFilter::setAreaCost`, the
//!   "terrain cost" weighting of game-AI pathfinding). Every step of the walk
//!   graph ([`World::neighbors_fp`]) stays a step, and its cost
//!   ([`super::world::body::step_cost_16`]) is multiplied by the toll of the
//!   cell it enters: one on a way and in the walk's own two places, and
//!   [`OFF_WAY_TOLL`] anywhere else. No route is added or removed; only the
//!   preference among the routes that exist moves, so every proof that reads
//!   the walk still judges a walk the router proves.
//! - **What a way is** (cited, spec-0120 §4.1): the body cells of a way
//!   place's declared clear columns, read by the clear-width check's own
//!   population ([`super::width::way_place_cells`]). The verge outside
//!   `clear` is not the way.
//! - **The walk's own places are free** (authored): a walk that starts and
//!   ends in one hall crosses the hall, never out of one door, along the
//!   street and in at the other; a walk leaves its start place and enters its
//!   goal place by the way it would anywhere. A place is the one the plan
//!   says holds the cell ([`crate::compiler::holders::place_of`]).
//! - **Why eight** (authored): a switchback road on a hillside is two to four
//!   times the length of the straight descent over the flank beside it, so
//!   any toll over four keeps a walk on it; eight keeps it on a way up to
//!   eight times as long as the shortcut, and bounds the detour a walk makes
//!   to keep to a way it barely needs.
//! - **The rendered line stays on the way** (authored): the string-pull that
//!   turns the route into a walked polyline ([`super::smooth_walk`]) never
//!   merges a segment between two way cells that sweeps a column off the way.

use std::collections::BTreeSet;

use crate::compiler::plan::Plan;

/// What a step onto a cell that is neither a way nor the walk's own place
/// costs, as a multiple of the same step on a way.
pub(in crate::compiler::nav) const OFF_WAY_TOLL: u32 = 8;

/// The plan's declared ways, read once per plan.
#[derive(Debug, Clone, Default)]
pub(in crate::compiler::nav) struct Ways {
    cells: BTreeSet<[i32; 3]>,
}

impl Ways {
    /// Every way place's cells.
    pub(in crate::compiler::nav) fn of(plan: &Plan) -> Self {
        Self {
            cells: super::width::way_place_cells(plan),
        }
    }

    /// The tolls one walk from `start` to `goal` is routed under.
    pub(in crate::compiler::nav) fn leg(
        &self,
        plan: &Plan,
        start: [i32; 3],
        goal: [i32; 3],
    ) -> WalkWays<'_> {
        let own = if self.cells.is_empty() {
            Vec::new()
        } else {
            own_places(plan, &[start, goal])
        };
        WalkWays {
            ways: &self.cells,
            own,
        }
    }
}

/// One walk's tolls: the plan's ways and the walk's own places.
#[derive(Debug, Clone)]
pub(in crate::compiler::nav) struct WalkWays<'a> {
    ways: &'a BTreeSet<[i32; 3]>,
    /// The inclusive cells of each own place: its footprint, from its walk
    /// plane through the course over its headroom.
    own: Vec<([i32; 3], [i32; 3])>,
}

impl WalkWays<'_> {
    /// No ways: every toll is one.
    #[cfg(test)]
    pub(in crate::compiler::nav) fn none() -> WalkWays<'static> {
        static EMPTY: BTreeSet<[i32; 3]> = BTreeSet::new();
        WalkWays {
            ways: &EMPTY,
            own: Vec::new(),
        }
    }

    /// A walk over exactly these way cells, with no own place.
    #[cfg(test)]
    pub(in crate::compiler::nav) fn over(ways: &BTreeSet<[i32; 3]>) -> WalkWays<'_> {
        WalkWays {
            ways,
            own: Vec::new(),
        }
    }

    /// Whether the plan declares any way at all.
    pub(in crate::compiler::nav) fn any(&self) -> bool {
        !self.ways.is_empty()
    }

    /// Whether `c` is a cell of a declared way.
    pub(in crate::compiler::nav) fn on_way(&self, c: [i32; 3]) -> bool {
        self.ways.contains(&c)
    }

    /// The toll of entering `c`.
    pub(in crate::compiler::nav) fn toll(&self, c: [i32; 3]) -> u32 {
        if !self.any()
            || self.on_way(c)
            || self
                .own
                .iter()
                .any(|(lo, hi)| (0..3).all(|i| c[i] >= lo[i] && c[i] <= hi[i]))
        {
            1
        } else {
            OFF_WAY_TOLL
        }
    }
}

/// The places holding `cells`, as inclusive cell boxes.
fn own_places(plan: &Plan, cells: &[[i32; 3]]) -> Vec<([i32; 3], [i32; 3])> {
    let Some(b) = plan.blockout.as_ref() else {
        return Vec::new();
    };
    let names: BTreeSet<String> = cells
        .iter()
        .filter_map(|c| crate::compiler::holders::place_of(plan, *c))
        .collect();
    let n = |v: i64| i32::try_from(v).unwrap_or(if v < 0 { i32::MIN } else { i32::MAX });
    b.boxes
        .iter()
        .filter(|bx| names.contains(&bx.node.0))
        .map(|bx| {
            (
                [n(bx.foot[0]), n(bx.floor), n(bx.foot[2])],
                [
                    n(bx.foot[1]),
                    n(bx.floor + i64::from(bx.clearance)),
                    n(bx.foot[3]),
                ],
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::nav::testkit::floored;
    use crate::compiler::nav::world::body::Footprint;
    use crate::compiler::nav::{DEFAULT_SPEED, resample_body, smooth_walk};
    use delvewright_dsl::metrics::PLAYER_WIDTH;

    const Y: i32 = 65;

    /// A U-shaped way on an open floor: west up `x = 1` from `z = 8` to `z = 1`,
    /// east along `z = 1` to `x = 15`, back down `x = 15` to `z = 8`. The two
    /// ends are fourteen cells apart along `z = 8`, the open floor between them;
    /// the way is twenty-eight.
    fn u_way() -> BTreeSet<[i32; 3]> {
        let mut c = BTreeSet::new();
        for z in 1..=8 {
            c.insert([1, Y, z]);
            c.insert([15, Y, z]);
        }
        for x in 1..=15 {
            c.insert([x, Y, 1]);
        }
        c
    }

    /// Which cells of the walked line's per-tick positions lie off `ways`.
    fn off_way_ticks(points: &[[f64; 3]], ways: &BTreeSet<[i32; 3]>) -> Vec<[i32; 3]> {
        points
            .iter()
            .map(|p| [p[0].floor() as i32, Y, p[2].floor() as i32])
            .filter(|c| !ways.contains(c))
            .collect()
    }

    /// **A walk keeps to the way beside a straight shortcut**: routed and drawn
    /// with the way's tolls, every cell the route enters and every per-tick
    /// position the body is drawn at lies on the way, though the open floor
    /// straight between the ends is half as long.
    #[test]
    fn a_walk_keeps_to_the_way_beside_a_straight_shortcut() {
        let world = floored(20, 12, Y, &[]);
        let fp = Footprint::player();
        let (start, goal) = ([1, Y, 8], [15, Y, 8]);
        let ways = u_way();
        let walk = WalkWays::over(&ways);
        // The shortcut exists and is what the router took before ways.
        let plain = world.find_path_fp(start, goal, &fp).expect("open floor");
        assert_eq!(plain.len(), 15, "the shortcut is the straight fourteen");
        assert!(
            plain.iter().any(|c| !ways.contains(c)),
            "the fixture is vacuous: the shortcut is on the way"
        );
        let route = world
            .find_walk_fp(start, goal, &fp, &walk)
            .expect("the way joins the ends");
        let off: Vec<_> = route.iter().filter(|c| !ways.contains(*c)).collect();
        assert!(off.is_empty(), "the route steps off the way at {off:?}");
        assert_eq!(route.len(), 29, "the way, all twenty-eight steps of it");
        let line = smooth_walk(&world, &route, &fp, PLAYER_WIDTH, &walk);
        let (ticks, _) = resample_body(&line, DEFAULT_SPEED, PLAYER_WIDTH);
        let off = off_way_ticks(&ticks, &ways);
        assert!(
            off.is_empty(),
            "the drawn line cuts off the way through {} tick(s), first {:?}",
            off.len(),
            off.first()
        );
    }

    /// **The drawn line keeps the corner the route kept.** The same route
    /// string-pulled without the way's cells cuts the U's corners across the
    /// open floor — the shape the guard in [`smooth_walk`] refuses.
    #[test]
    fn the_line_without_the_way_cuts_the_corner() {
        let world = floored(20, 12, Y, &[]);
        let fp = Footprint::player();
        let ways = u_way();
        let route = world
            .find_walk_fp([1, Y, 8], [15, Y, 8], &fp, &WalkWays::over(&ways))
            .expect("the way joins the ends");
        let line = smooth_walk(&world, &route, &fp, PLAYER_WIDTH, &WalkWays::none());
        let (ticks, _) = resample_body(&line, DEFAULT_SPEED, PLAYER_WIDTH);
        assert!(
            !off_way_ticks(&ticks, &ways).is_empty(),
            "the fixture is vacuous: an unguarded line keeps to the way too"
        );
    }

    /// **Where no way joins the ends, the walk still exists**: the U with its
    /// east leg gone reaches no closer than `x = 15, z = 1`, and the walk to
    /// `[15, 65, 8]` is found, leaving the way to cross the floor it must.
    #[test]
    fn where_no_way_joins_the_ends_the_walk_still_exists() {
        let world = floored(20, 12, Y, &[]);
        let fp = Footprint::player();
        let mut ways = u_way();
        ways.retain(|c| !(c[0] == 15 && c[2] > 1));
        let walk = WalkWays::over(&ways);
        let route = world
            .find_walk_fp([1, Y, 8], [15, Y, 8], &fp, &walk)
            .expect("the floor joins the ends");
        assert_eq!(route.first(), Some(&[1, Y, 8]));
        assert_eq!(route.last(), Some(&[15, Y, 8]));
        // Every step is one the router proves: the route is a chain of its
        // own neighbours.
        for pair in route.windows(2) {
            assert!(
                world.neighbors_fp(pair[0], &fp).contains(&pair[1]),
                "{pair:?} is no step of the walk graph"
            );
        }
        // And a wall that severs the floor still severs the walk.
        let walled: Vec<[i32; 3]> = (0..12).flat_map(|z| [[8, Y, z], [8, Y + 1, z]]).collect();
        let cut = floored(20, 12, Y, &walled);
        assert!(
            cut.find_walk_fp([1, Y, 8], [15, Y, 8], &fp, &walk)
                .is_none()
        );
    }

    /// **No way, no change**: with no way declared every toll is one, and the
    /// walk is the router's own route, cell for cell.
    #[test]
    fn with_no_way_the_walk_is_the_routers_route() {
        let world = floored(20, 12, Y, &[[5, Y, 3], [5, Y + 1, 3], [9, Y, 6]]);
        let fp = Footprint::player();
        let mut compared = 0;
        for (a, b) in [
            ([1, Y, 1], [15, Y, 9]),
            ([1, Y, 8], [15, Y, 8]),
            ([18, Y, 0], [0, Y, 11]),
        ] {
            assert_eq!(
                world.find_walk_fp(a, b, &fp, &WalkWays::none()),
                world.find_path_fp(a, b, &fp),
                "{a:?} -> {b:?}"
            );
            compared += 1;
        }
        assert_eq!(compared, 3);
    }
}
