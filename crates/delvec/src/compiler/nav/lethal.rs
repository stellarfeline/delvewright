//! How a body reaches a declared volume: the reach query the lethal-volume
//! proofs read.

use super::*;
use std::collections::{BTreeMap, BTreeSet};

/// How a body first got its hitbox into a declared volume, as
/// [`World::reach_into_volumes`] found it: the reached cell it set off from,
/// the cells it passed through on the way in, and the movement in words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VolumeHit {
    /// The root the body started from and every reached cell after it, ending
    /// at the cell it moved into the volume from.
    pub path: Vec<[i32; 3]>,
    /// The cells it moved through on the way in, ending in the first one whose
    /// occupant the volume catches. One cell when it stood or floated there.
    pub way_in: Vec<[i32; 3]>,
    /// How it got in, in words for a report.
    pub how: &'static str,
}

/// What [`World::reach_into_volumes`] measured: how many cells the body could
/// reach, and per volume (in the order the caller listed them) the first way in.
#[derive(Clone, Debug, Default)]
pub struct VolumeReach {
    /// Cells the body can stand or float in, roots included.
    pub reached: BTreeSet<[i32; 3]>,
    /// One entry per volume asked about: `None` when nothing reached it.
    pub hits: Vec<Option<VolumeHit>>,
}

impl World {
    /// A copy of this world with every cell of `cells` made empty — the
    /// counterfactual in which a player has opened the barrier standing there and
    /// left it open (`DW0923`). A door, a trapdoor or a fence gate stands open
    /// with no collision a body is stopped by, so its cell leaves every
    /// collision set; nothing else about the world moves.
    pub fn with_openings_open(&self, cells: &BTreeSet<[i32; 3]>) -> World {
        let mut w = self.clone_world();
        for c in cells {
            w.solid.remove(c);
            w.tall.remove(c);
            w.use_gates.remove(c);
            w.partial.remove(c);
        }
        w
    }

    /// **Can a body moving from `roots` get its hitbox into any of `volumes`?**
    ///
    /// Floods the body's own movement — [`World::mob_moves`] when `mob`,
    /// [`World::body_moves`] otherwise — from `roots`, and asks of every place it
    /// reaches whether its hitbox meets a volume there
    /// ([`delvewright_dsl::metrics::cell_can_meet_volume`], the test the router's
    /// keep-out is written against). Two ways in a flood of standing places cannot
    /// see are asked as well:
    ///
    /// - **A fall through the volume.** From every place it stands, the body can
    ///   step off into a neighbouring column clear at its feet and head and drop
    ///   until something stops it. Every cell of that drop is asked, at any
    ///   depth: a body that falls through a killing box dies in it, however far
    ///   below the floor it would have landed.
    /// - **Sinking**, for a mob only. A mob that enters water is taken to reach
    ///   every cell of that water: the undead sink and walk the bottom, and a
    ///   drowned swims down. Not every mob sinks, so for the ones that float this
    ///   is wider than the truth; it can only refuse a wave, never pass one. A
    ///   player body does not dive here, which is [`World::body_moves`]'s own
    ///   rule.
    ///
    /// `radius`, when given, bounds the flood: a reached place lies within that
    /// many blocks of some root. Asked of a world with no lethal exclusion
    /// ([`World::without_exclusions`]) — on the world the router walks, a
    /// volume's keep-out stops every movement before it, and this would find
    /// nothing.
    ///
    /// Deterministic: breadth-first in `roots` order, each place's moves in
    /// [`World::body_moves`]'s fixed order, the first way into each volume kept
    /// (ADR-0006).
    pub fn reach_into_volumes(
        &self,
        roots: &[[i32; 3]],
        fp: &Footprint,
        mob: bool,
        radius: Option<f64>,
        volumes: &[([i32; 3], [i32; 3])],
    ) -> VolumeReach {
        const HORIZ: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
        debug_assert!(self.lethal.is_empty(), "asked of a lethal-applied world");
        let body = fp.body();
        let keep_outs: Vec<([i32; 3], [i32; 3])> = volumes
            .iter()
            .map(|(lo, hi)| delvewright_dsl::metrics::keep_out_box(body, *lo, *hi))
            .collect();
        // Whether a body in `c` is caught: by its feet where this model puts
        // them, so a body standing on a partial block meets a volume in the
        // course it stands on.
        let meets = |c: [i32; 3]| -> Option<usize> {
            volumes
                .iter()
                .position(|(lo, hi)| self.body_can_meet_volume(c, fp, *lo, *hi))
        };
        // Below this a drop is in no volume's keep-out, so it is not followed.
        let bottom = keep_outs.iter().map(|(lo, _)| lo[1]).min().unwrap_or(0);
        let column_near = |x: i32, z: i32| {
            keep_outs
                .iter()
                .any(|(lo, hi)| lo[0] <= x && x <= hi[0] && lo[2] <= z && z <= hi[2])
        };
        let within = |c: [i32; 3]| {
            radius.is_none_or(|r| {
                roots
                    .iter()
                    .any(|s| (0..3).map(|i| f64::from(c[i] - s[i]).powi(2)).sum::<f64>() <= r * r)
            })
        };
        let mut out = VolumeReach {
            reached: roots.iter().copied().collect(),
            hits: vec![None; volumes.len()],
        };
        let mut pred: BTreeMap<[i32; 3], [i32; 3]> = BTreeMap::new();
        let mut sunk: BTreeSet<[i32; 3]> = BTreeSet::new();
        // Ways in found from the cell being expanded: (volume, way in, how).
        let mut found: Vec<(usize, Vec<[i32; 3]>, &'static str)> = Vec::new();
        let mut queue: std::collections::VecDeque<[i32; 3]> = roots.iter().copied().collect();
        let clear = |c: [i32; 3]| !self.is_occupied(c);
        while let Some(cur) = queue.pop_front() {
            let afloat = self.is_water_surface(cur);
            if let Some(v) = meets(cur) {
                found.push((
                    v,
                    vec![cur],
                    if afloat { "floating in" } else { "walking in" },
                ));
            }
            // Every water cell this one opens onto, for a body that sinks.
            let mut sink_from: Vec<([i32; 3], Vec<[i32; 3]>)> = Vec::new();
            if mob && afloat {
                sink_from.push((cur, Vec::new()));
            }
            if !afloat {
                for (dx, dz) in HORIZ {
                    let (x, z) = (cur[0] + dx, cur[2] + dz);
                    if !column_near(x, z) && !mob {
                        continue;
                    }
                    if !within([x, cur[1], z])
                        || !clear([x, cur[1], z])
                        || !clear([x, cur[1] + 1, z])
                    {
                        continue;
                    }
                    let mut trace: Vec<[i32; 3]> = Vec::new();
                    let mut y = cur[1];
                    while y >= bottom {
                        let c = [x, y, z];
                        if !clear(c) {
                            if mob && self.is_water(c) {
                                sink_from.push((c, trace.clone()));
                            }
                            break;
                        }
                        trace.push(c);
                        if let Some(v) = meets(c) {
                            let how = if trace.len() == 1 {
                                "stepping in"
                            } else {
                                "a fall"
                            };
                            found.push((v, trace.clone(), how));
                            break;
                        }
                        y -= 1;
                    }
                }
            }
            for (start, lead) in sink_from {
                if !sunk.insert(start) {
                    continue;
                }
                let mut water: std::collections::VecDeque<([i32; 3], Vec<[i32; 3]>)> =
                    std::collections::VecDeque::new();
                let mut first = lead;
                first.push(start);
                water.push_back((start, first));
                while let Some((w, trace)) = water.pop_front() {
                    if let Some(v) = meets(w) {
                        found.push((v, trace.clone(), "sinking in water"));
                    }
                    for d in [
                        [0, -1, 0],
                        [-1, 0, 0],
                        [1, 0, 0],
                        [0, 0, -1],
                        [0, 0, 1],
                        [0, 1, 0],
                    ] {
                        let n = [w[0] + d[0], w[1] + d[1], w[2] + d[2]];
                        if self.is_water(n) && within(n) && sunk.insert(n) {
                            let mut t = trace.clone();
                            t.push(n);
                            water.push_back((n, t));
                        }
                    }
                }
            }
            for (v, way_in, how) in found.drain(..) {
                if out.hits[v].is_some() {
                    continue;
                }
                let mut path = vec![cur];
                while let Some(p) = pred.get(path.last().unwrap_or(&cur)) {
                    path.push(*p);
                }
                path.reverse();
                out.hits[v] = Some(VolumeHit { path, way_in, how });
            }
            let next = if mob {
                self.mob_moves(cur, fp)
            } else {
                self.body_moves(cur)
            };
            for n in next {
                if within(n) && out.reached.insert(n) {
                    pred.insert(n, cur);
                    queue.push_back(n);
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod mob_reach_tests {
    use super::*;

    /// A flat yard, feet at y=1 over a stone floor at y=0, `w` × `d` cells.
    fn yard(w: i32, d: i32) -> BTreeSet<[i32; 3]> {
        let mut solid = BTreeSet::new();
        for x in 0..w {
            for z in 0..d {
                solid.insert([x, 0, z]);
            }
        }
        solid
    }

    fn world(
        solid: BTreeSet<[i32; 3]>,
        tall: BTreeSet<[i32; 3]>,
        flooded: BTreeSet<[i32; 3]>,
        partial: BTreeMap<[i32; 3], u8>,
    ) -> World {
        World::from_occupancy(
            crate::compiler::assembled::Occupancy {
                solid,
                tall,
                use_gates: BTreeSet::new(),
                flooded,
                partial,
                waterloggable: BTreeSet::new(),
                lava: BTreeSet::new(),
                climb: Default::default(),
                unheld_climb: Default::default(),
            },
            Premises::geometry_only(),
        )
    }

    fn zombie() -> Footprint {
        entity_footprint("minecraft:zombie")
    }

    /// The jump arc is the one movement a mob does not have: across a one-column
    /// gap a player body lands, a mob does not.
    #[test]
    fn a_mob_makes_no_gap_jump() {
        let mut solid = BTreeSet::new();
        for x in [0, 1, 2, 4, 5, 6] {
            solid.insert([x, 0, 0]);
        }
        // Deep under the gap, so the fall is not a way across either.
        let w = World::from_solid_cells(solid);
        assert!(w.body_moves([2, 1, 0]).contains(&[4, 1, 0]));
        assert!(!w.mob_moves([2, 1, 0], &zombie()).contains(&[4, 1, 0]));
    }

    /// The vesperhold climb, in miniature: a lantern one course high beside a wall
    /// two courses high. A mob steps onto the lantern, jumps from its top onto the
    /// wall's top, and walks along it; the player relation never stands a body on
    /// a wall, and from the floor the wall's top is a 1.5-block rise.
    #[test]
    fn a_mob_climbs_a_lantern_onto_a_wall_top() {
        let mut solid = yard(6, 3);
        solid.insert([1, 1, 1]); // the lantern
        let partial: BTreeMap<[i32; 3], u8> = [([1, 1, 1], 9u8)].into_iter().collect();
        let tall: BTreeSet<[i32; 3]> = [[2, 1, 1], [3, 1, 1]].into_iter().collect();
        let w = world(solid, tall, BTreeSet::new(), partial);
        let fp = zombie();
        assert!(
            w.mob_moves([0, 1, 1], &fp).contains(&[1, 2, 1]),
            "onto the lantern"
        );
        assert!(
            w.mob_moves([1, 2, 1], &fp).contains(&[2, 2, 1]),
            "onto the wall"
        );
        assert!(w.mob_moves([2, 2, 1], &fp).contains(&[3, 2, 1]), "along it");
        assert!(
            !w.mob_moves([2, 1, 0], &fp).contains(&[2, 2, 1]),
            "never from the floor beside it"
        );
        assert!(!w.body_moves([1, 2, 1]).contains(&[2, 2, 1]));
    }

    /// A drop through a volume at any depth is a way in, however far below the
    /// floor the body would have landed — and only a fall finds it.
    #[test]
    fn a_fall_through_a_volume_is_a_way_in_at_any_depth() {
        let mut solid = yard(3, 3);
        solid.remove(&[2, 0, 1]);
        solid.insert([2, -40, 1]);
        let w = World::from_solid_cells(solid);
        let vol = ([2, -30, 1], [2, -30, 1]);
        let r = w.reach_into_volumes(&[[0, 1, 1]], &zombie(), true, Some(16.0), &[vol]);
        let hit = r.hits[0].as_ref().expect("the shaft's volume is reached");
        assert_eq!(hit.how, "a fall");
        assert_eq!(hit.way_in.last(), Some(&[2, -30, 1]));
    }

    /// Water over a volume: a mob sinks to it, a player body (which does not
    /// dive) does not — the zero-binding shape `DW0891` reports.
    #[test]
    fn a_mob_sinks_to_a_volume_under_water_and_a_player_does_not() {
        let mut solid = yard(5, 3);
        let mut flooded = BTreeSet::new();
        for y in -4..=0 {
            solid.remove(&[3, y, 1]);
            flooded.insert([3, y, 1]);
            solid.insert([2, y, 1]);
            solid.insert([4, y, 1]);
            solid.insert([3, y, 0]);
            solid.insert([3, y, 2]);
        }
        solid.remove(&[2, 0, 1]);
        flooded.insert([2, 0, 1]);
        solid.insert([2, -1, 1]);
        solid.insert([3, -5, 1]);
        let w = world(solid, BTreeSet::new(), flooded, BTreeMap::new());
        let vol = ([3, -4, 1], [3, -4, 1]);
        let mob = w.reach_into_volumes(&[[0, 1, 1]], &zombie(), true, Some(16.0), &[vol]);
        assert_eq!(
            mob.hits[0].as_ref().map(|h| h.how),
            Some("sinking in water")
        );
        let player = w.reach_into_volumes(&[[0, 1, 1]], &Footprint::player(), false, None, &[vol]);
        assert!(player.hits[0].is_none(), "{:?}", player.hits[0]);
    }

    /// The follow range bounds the flood: a volume one cell past it is not
    /// reached.
    #[test]
    fn the_follow_range_bounds_the_reach() {
        let w = World::from_solid_cells(yard(20, 1));
        let vol = ([12, 1, 0], [12, 1, 0]);
        let near = w.reach_into_volumes(&[[0, 1, 0]], &zombie(), true, Some(12.0), &[vol]);
        let far = w.reach_into_volumes(&[[0, 1, 0]], &zombie(), true, Some(10.0), &[vol]);
        assert!(near.hits[0].is_some());
        assert!(far.hits[0].is_none());
    }

    /// A barrier a player opens, removed: the cell is passable to every body.
    #[test]
    fn an_opened_barrier_is_an_empty_cell() {
        let solid = yard(5, 1);
        let tall: BTreeSet<[i32; 3]> = [[2, 1, 0]].into_iter().collect();
        let w = world(solid, tall.clone(), BTreeSet::new(), BTreeMap::new());
        assert!(!w.mob_moves([1, 1, 0], &zombie()).contains(&[2, 1, 0]));
        let opened = w.with_openings_open(&tall);
        assert!(opened.mob_moves([1, 1, 0], &zombie()).contains(&[2, 1, 0]));
    }
}
