use super::*;
use crate::compiler::nav::world::{Premises, Sea};
use std::collections::BTreeSet;

/// A synthetic world: full cubes, barriers and water, under `ambient`.
#[derive(Default)]
pub(in crate::compiler::nav) struct Build {
    pub solid: BTreeSet<[i32; 3]>,
    pub tall: BTreeSet<[i32; 3]>,
    pub water: BTreeSet<[i32; 3]>,
}

impl Build {
    pub fn fill(&mut self, lo: [i32; 3], hi: [i32; 3]) -> &mut Self {
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    self.solid.insert([x, y, z]);
                }
            }
        }
        self
    }

    pub fn world(
        &self,
        ambient: Ambient,
        built: Vec<crate::compiler::nav::world::BuiltPiece>,
    ) -> World {
        let mut p = Premises::geometry_only();
        p.ambient = ambient;
        p.built = built;
        World::from_occupancy(
            crate::compiler::assembled::Occupancy {
                solid: self.solid.clone(),
                tall: self.tall.clone(),
                use_gates: BTreeSet::new(),
                flooded: self.water.clone(),
                partial: Default::default(),
                waterloggable: BTreeSet::new(),
                lava: BTreeSet::new(),
                climb: Default::default(),
                unheld_climb: Default::default(),
                material: Default::default(),
            },
            p,
        )
    }
}

/// The walk relation's closure: the relation a route proof walks, which these
/// tests hold the reach graph against.
fn old_reach(w: &World, roots: &[[i32; 3]]) -> BTreeSet<[i32; 3]> {
    w.walk_region(roots)
}

#[test]
fn an_open_floor_is_reached_and_credited_from_the_template() {
    let mut b = Build::default();
    b.fill([0, 0, 0], [11, 0, 11]);
    let w = b.world(Ambient::Void, Vec::new());
    let t = std::time::Instant::now();
    let r = w.body_reach(&[[5, 1, 5]]);
    eprintln!(
        "open floor: {} states, {} simulated, {} credited, {:?}",
        r.states.len(),
        r.simulated,
        r.credited,
        t.elapsed()
    );
    assert_eq!(r.states.len(), 144);
    assert!(r.credited > 0);
    assert!(
        !r.leaves_world.is_empty(),
        "the platform's rim drops into the void"
    );
}

/// The Bell yard (spec-0113 §9): a yard walled three high, a stone inside,
/// and a buttress against the wall one cell diagonally across from the stone.
pub(in crate::compiler::nav) fn bell_yard() -> World {
    let mut b = Build::default();
    b.fill([-6, 0, -6], [20, 0, 20]);
    // The wall ring, three high, around [0..14]².
    for i in 0..=14 {
        for (x, z) in [(i, 0), (i, 14), (0, i), (14, i)] {
            b.fill([x, 1, z], [x, 3, z]);
        }
    }
    // The stone, one high, and the buttress, two high, against the east wall.
    b.fill([10, 1, 10], [10, 1, 10]);
    b.fill([12, 1, 12], [13, 2, 12]);
    b.world(Ambient::Void, Vec::new())
}

#[test]
fn the_bell_yard_is_left_by_two_jumps_the_old_relation_does_not_see() {
    let w = bell_yard();
    let root = [[5, 1, 5]];
    let outside = [17, 1, 7];
    assert!(
        !old_reach(&w, &root).contains(&outside),
        "the old relation reached outside"
    );
    let t = std::time::Instant::now();
    let r = w.body_reach(&root);
    eprintln!(
        "bell yard: {} states, {} simulated, {} credited, {:?}",
        r.states.len(),
        r.simulated,
        r.credited,
        t.elapsed()
    );
    assert!(r.states.contains_key(&[12, 3, 12]), "the buttress top");
    assert!(
        r.states.contains_key(&outside),
        "the graph did not reach outside the wall"
    );
}

/// The jetty (spec-0113 §9): a plank deck two over an ocean, fenced, with a
/// crate beside the fence a body jumps over it from.
pub(in crate::compiler::nav) fn jetty() -> World {
    let mut b = Build::default();
    // The deck: sea level 62, the deck at 64 (its top two over the water's).
    b.fill([0, 64, 0], [9, 64, 4]);
    for x in 0..=9 {
        b.tall.insert([x, 65, 0]);
        b.tall.insert([x, 65, 4]);
    }
    for z in 1..=3 {
        b.tall.insert([9, 65, z]);
        b.tall.insert([0, 65, z]);
    }
    b.fill([4, 65, 1], [4, 65, 1]);
    let built = vec![("prefab/jetty".to_string(), ([0, 64, 0], [9, 66, 4]))];
    b.world(
        Ambient::Ocean(Sea {
            level: 62,
            floor_top: 54,
        }),
        built,
    )
}

#[test]
fn the_jetty_is_left_into_the_sea_by_a_jump_and_not_returned_to() {
    let w = jetty();
    let root = [[2, 65, 2]];
    let old = old_reach(&w, &root);
    assert!(
        !old.iter().any(|c| w.ambient_water(*c)),
        "the old relation entered the sea"
    );
    let t = std::time::Instant::now();
    let r = w.body_reach(&root);
    eprintln!(
        "jetty: {} states, {} simulated, {} credited, {:?}",
        r.states.len(),
        r.simulated,
        r.credited,
        t.elapsed()
    );
    let sea: Vec<[i32; 3]> = r
        .states
        .keys()
        .copied()
        .filter(|c| w.ambient_water(*c))
        .collect();
    assert!(!sea.is_empty(), "the graph did not enter the sea");
    assert!(!r.open_sea.is_empty(), "the sea is open");
}

/// spec-0113 A6: the two escapes the playtests found are seen. The Bell yard
/// is left by two jumps to ground outside the wall the old relation never
/// reaches, and the place is one a body cannot leave (`DW0921`); the jetty is
/// left by a jump into a sea with no way back (`DW0322`), which the old
/// relation never enters.
#[test]
fn the_two_escapes_are_seen() {
    let yard = bell_yard();
    let route: Vec<[i32; 3]> = (2..=12)
        .flat_map(|x| (2..=12).map(move |z| [x, 1, z]))
        .filter(|c| yard.is_standable(*c))
        .collect();
    let ways: BTreeSet<[i32; 3]> = route.iter().copied().collect();
    let old = old_reach(&yard, &route);
    assert!(
        !old.iter()
            .any(|c| c[0] > 14 || c[2] > 14 || c[0] < 0 || c[2] < 0),
        "the old relation left the yard"
    );
    let judged = yard.trapped_places(&route, &ways, None);
    let outside: usize = judged
        .pockets
        .iter()
        .flatten()
        .filter(|c| c[0] > 14)
        .count();
    assert!(
        outside > 0,
        "no pocket outside the wall: {:?}",
        judged.pockets.len()
    );

    let jetty = jetty();
    let old = old_reach(&jetty, &[[2, 65, 2]]);
    assert!(
        !old.iter().any(|c| jetty.ambient_water(*c)),
        "the old relation entered the sea"
    );
    let roots = [crate::compiler::nav::AnchorRoot {
        at: [2, 65, 2],
        within: ([0, 64, 0], [9, 66, 4]),
    }];
    let err = crate::compiler::nav::verify_boundary_safety(&jetty, &roots)
        .expect_err("the jetty strands a body");
    assert_eq!(err.code.id(), "DW0322");
    assert!(err.message.contains("jump"), "{}", err.message);
}

/// spec-0113 A10 / §5.3: the budget is refused, never truncated. The Bell
/// yard's closure at the shipped budget completes; at a budget under its own
/// work it stops, says so, and `DW0921`'s judgement refuses it as `DW1042`.
#[test]
fn a_graph_over_budget_is_refused() {
    let w = bell_yard();
    let full = w.body_reach(&[[5, 1, 5]]);
    assert_eq!(full.over_budget, None);
    let work = full.simulated + full.credited;
    assert!(
        work < delvewright_dsl::metrics::REACH_BUDGET_STATES,
        "{work}"
    );
    let mut p = ReachParams::shipped(false);
    p.budget = work / 2;
    let partial = w.body_reach_at(&[[5, 1, 5]], &p);
    assert_eq!(partial.over_budget, Some(work / 2));
    let mut b = ReachBinding::default();
    b.add(&w, &partial);
    let f = crate::compiler::nav::reach_budget_failure(&b, work / 2);
    assert_eq!(f.code.id(), "DW1042");
    assert!(f.message.contains("past the ceiling"), "{}", f.message);
}

/// The finer launch set the convergence test (spec-0113 A7) builds beside the
/// shipped one: a half-degree facing step and strafe at `±0.5` as well.
pub(in crate::compiler::nav) fn finer() -> ReachParams {
    ReachParams {
        facing_step: FACING_STEP_DEG / 2.0,
        strafes: vec![-1.0, -0.5, 0.0, 0.5, 1.0],
        ..ReachParams::shipped(false)
    }
}

/// spec-0113 A7, over both escape fixtures: the finer launch set adds no rest
/// state to the shipped one's closure, and it is at least twice as many
/// launches. The gallery's arm is `tests/reach_graph.rs`.
#[test]
fn the_facing_step_has_converged() {
    let coarse = launch_set(false, FACING_STEP_DEG, &ReachParams::shipped(false).strafes).len();
    let fine = launch_set(false, finer().facing_step, &finer().strafes).len();
    assert!(fine >= 2 * coarse, "{fine} against {coarse}");
    for (name, w, root) in [
        ("the Bell yard", bell_yard(), [5, 1, 5]),
        ("the jetty", jetty(), [2, 65, 2]),
    ] {
        let a = w.body_reach(&[root]).cells();
        let b = w.body_reach_at(&[root], &finer()).cells();
        let added: Vec<&[i32; 3]> = b.difference(&a).collect();
        assert!(added.is_empty(), "{name}: the finer set adds {added:?}");
    }
}

/// The gallery's fenced garden (spec-0113 §9): a fence ring four by four, a
/// boulder outside its west side, a step inside against the east fence.
pub(in crate::compiler::nav) fn garden() -> World {
    let mut b = Build::default();
    b.fill([0, 0, 0], [10, 0, 10]);
    for x in 3..=6 {
        for z in 3..=6 {
            if x == 3 || x == 6 || z == 3 || z == 6 {
                b.tall.insert([x, 1, z]);
            }
        }
    }
    b.fill([2, 1, 4], [2, 1, 4]);
    b.fill([5, 1, 5], [5, 1, 5]);
    b.world(Ambient::Void, Vec::new())
}

/// spec-0113 §9: the bed is got into over the fence from the boulder, and
/// left by the step — every bed cell reaches the ground outside the ring.
#[test]
fn the_garden_bed_is_got_into_and_left() {
    let w = garden();
    let r = w.body_reach(&[[1, 1, 1]]);
    let bed = [[4, 1, 4], [4, 1, 5], [5, 1, 4]];
    for c in bed {
        assert!(
            r.states.contains_key(&c),
            "the bed cell {c:?} is not reached"
        );
    }
    assert!(
        !old_reach(&w, &[[1, 1, 1]]).contains(&[4, 1, 4]),
        "the walk relation entered the bed"
    );
    for c in bed {
        let back = w.body_reach(&[c]);
        assert!(
            back.states.contains_key(&[1, 1, 1]),
            "{c:?} cannot leave the bed"
        );
    }
}
