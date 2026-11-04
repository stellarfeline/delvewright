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
                solid: (self.solid.clone()).into(),
                tall: (self.tall.clone()).into(),
                use_gates: (BTreeSet::new()).into(),
                flooded: (self.water.clone()).into(),
                partial: Default::default(),
                waterloggable: (BTreeSet::new()).into(),
                lava: (BTreeSet::new()).into(),
                climb: Default::default(),
                unheld_climb: Default::default(),
                material: Default::default(),
                footprints: Default::default(),
                hiding: Default::default(),
                hurting: Default::default(),
                leaves: Default::default(),
            },
            p,
        )
    }
}

/// The walk relation's closure: the relation a route proof walks, which these
/// tests hold the reach graph against.
/// **The snapshot's window is the one rule's** (`window_box`, ADR-0037 §3):
/// the window `Dense::of` builds round a world's modelled cells is the box
/// `window_box` gives for those cells' bounds, under a void and under an
/// ocean — so the cost the engine states before the proofs, which takes the
/// same function over the judged columns, bounds the window the proofs build.
#[test]
fn the_snapshot_window_is_the_one_rules() {
    for sea in [None, Some((54, 62))] {
        let mut b = Build::default();
        for c in [[-3, 60, 7], [20, 63, 7], [5, 70, -9], [5, 61, 40]] {
            b.fill(c, c);
        }
        let ambient = match sea {
            None => Ambient::Void,
            Some((floor_top, level)) => Ambient::Ocean(Sea { level, floor_top }),
        };
        let w = b.world(ambient, Vec::new());
        let d = Dense::of(&w);
        let (lo, hi) = window_box([-3, 60, -9], [20, 70, 40], sea);
        assert_eq!(d.lo, lo, "sea {sea:?}");
        assert_eq!(
            d.dims,
            [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1],
            "sea {sea:?}"
        );
        assert_eq!(hi[1] % 16, 15, "the top is a course under a multiple of 16");
        assert!(lo[0] == -3 - 8 && hi[2] == 40 + 8, "the margin is 8 cells");
        if sea.is_some() {
            assert!(
                lo[1] <= 54 && hi[1] >= 62 + 6,
                "an ocean's window spans the sea"
            );
        }
    }
}

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
    bell_yard_build().world(Ambient::Void, Vec::new())
}

fn bell_yard_build() -> Build {
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
    b
}

/// The flight memo and the node bank are accelerators: every node a graph
/// took from them — a node banked by an earlier graph of the same window, a
/// node re-flown run by run beside the cells that changed, a launch answered
/// from a flight that met the same cells — is the node one pass over every
/// launch, flown here, gives. The Bell yard, then the yard with a doorway
/// opened in its north wall (one window, two configurations).
#[test]
fn a_node_from_the_memo_and_the_bank_is_the_node_flying_gives() {
    let root = [[5, 1, 5]];
    let first = bell_yard();
    let _ = first.body_reach(&root);
    let mut b = bell_yard_build();
    b.solid.remove(&[7, 1, 0]);
    b.solid.remove(&[7, 2, 0]);
    let opened = b.world(Ambient::Void, Vec::new());
    let r = opened.body_reach(&root);
    assert!(r.states.contains_key(&[7, 1, -1]), "the doorway is walked");
    let params = ReachParams::shipped(opened.unbounded_fall);
    let memo = opened.graph_memo(&params);
    let flown: Vec<(FlownKey, Arc<NodeEdges>)> = memo
        .flown
        .entries()
        .into_iter()
        .flat_map(|(c, l)| l.into_iter().map(move |(q, e)| ((c, q), Arc::new(e))))
        .collect();
    for (key, e) in &flown {
        let fresh = opened.expand(&memo, &params, key.0, key.1, None);
        assert_eq!(fresh.kind, e.kind, "{key:?}");
        assert_eq!(fresh.edges, e.edges, "{key:?}");
        assert_eq!(fresh.leaves_world, e.leaves_world, "{key:?}");
        assert_eq!(fresh.dies, e.dies, "{key:?}");
        assert_eq!(fresh.simulated, e.simulated, "{key:?}");
        assert_eq!(fresh.credited, e.credited, "{key:?}");
        assert_eq!(fresh.read, e.read, "{key:?}");
    }
    eprintln!(
        "agreement: {} flown node(s) of the opened yard re-flown and equal; {} node(s) banked twice, {} run(s) taken as written",
        flown.len(),
        memo.bank.revisited(),
        memo.bank.reused_runs()
    );
    // Bound, not vacuous: both graphs flew nodes of the one bank, and some
    // re-flown node took a run as written.
    assert!(!flown.is_empty());
    assert!(memo.bank.revisited() > 0);
    assert!(memo.bank.reused_runs() > 0);
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
    let sea: Vec<[i32; 3]> = r.states.keys().filter(|c| w.ambient_water(*c)).collect();
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

/// A cell shut in on its four sides, two of which meet the yard's corner
/// diagonally: the yard's north wall holds `[1, 1, 1]` and its east wall
/// `[2, 1, 2]`, and the shut cell `[2, 1, 1]` sits across the corner where
/// the two meet, walled on its other two sides. A 0.6-wide body cannot pass
/// between two blocks that meet at a corner.
pub(in crate::compiler::nav) fn corner_cell() -> World {
    let mut b = Build::default();
    b.fill([-4, 0, 0], [3, 0, 7]);
    // The yard, x -3..=1 by z 2..=6, ringed three high.
    for x in -4..=2 {
        b.fill([x, 1, 1], [x, 3, 1]);
        b.fill([x, 1, 7], [x, 3, 7]);
    }
    for z in 1..=7 {
        b.fill([-4, 1, z], [-4, 3, z]);
        b.fill([2, 1, z], [2, 3, z]);
    }
    // The shut cell's own two walls, and the cell itself left open.
    b.fill([2, 1, 0], [2, 3, 0]);
    b.fill([3, 1, 1], [3, 3, 1]);
    b.solid.remove(&[2, 1, 1]);
    b.solid.remove(&[2, 2, 1]);
    b.solid.remove(&[2, 3, 1]);
    b.world(Ambient::Void, Vec::new())
}

/// spec-0113 §4.3: a landing is credited only in the cells the body's exact
/// box rests on. A body pressed into the yard's north-east corner rests in
/// `[1, 1, 2]` alone; the cell across the corner is never credited, and the
/// closure from the yard never reaches it.
#[test]
fn a_landing_is_never_credited_across_a_corner() {
    let w = corner_cell();
    let shut = [2, 1, 1];
    assert!(w.is_standable(shut), "the shut cell is a rest state");
    let r = w.body_reach(&[[-1, 1, 4]]);
    assert!(r.states.contains_key(&[1, 1, 2]), "the yard's corner cell");
    assert!(
        !r.states.contains_key(&shut),
        "the shut cell is reached across the corner"
    );
    let memo = w.graph_memo(&ReachParams::shipped(false));
    let body = Attributes::PLAYER;
    let half = body.width / 2.0;
    let corner = [2.0 - half, 1.0, 2.0 + half];
    assert_eq!(landing_cells(&memo.dense, &body, corner), vec![[1, 1, 2]]);
}

/// A configuration's snapshot keeps its own copy of only the chunks its
/// lineage holds otherwise, and reads every cell as a snapshot of its own
/// would: the Bell yard (a window wider than one chunk), then the yard with a
/// doorway opened in its north wall.
#[test]
fn a_snapshot_shares_every_chunk_its_lineage_holds_alike() {
    let params = ReachParams::shipped(false);
    let mut b = bell_yard_build();
    // A cell far from the doorway, so the two worlds' windows are the yard's
    // and this test's own (no other test's world shares the window).
    b.fill([-6, 0, 40], [-6, 0, 40]);
    let first = b.world(Ambient::Void, Vec::new());
    let base = first.graph_memo(&params);
    b.solid.remove(&[7, 1, 0]);
    b.solid.remove(&[7, 2, 0]);
    let opened = b.world(Ambient::Void, Vec::new());
    let memo = opened.graph_memo(&params);
    let own = Dense::of(&opened);
    assert!(
        own.cells.pages.len() > 2,
        "the window spans {} chunk(s); a test of sharing needs several",
        own.cells.pages.len()
    );
    // Perturbed toward the vacuous shape: a snapshot that shared nothing, or
    // shared a chunk it holds otherwise, reds one of these.
    let mine = memo.dense.cells.own_chunks(&base.dense.cells);
    assert!(
        (1..=2).contains(&mine),
        "the doorway's two cells lie in one or two chunks; the snapshot holds {mine} of its own"
    );
    assert!(memo.extra.cells.own_chunks(&base.extra.cells) <= 2);
    assert_eq!(own.cells.len(), memo.dense.cells.len());
    for i in 0..own.cells.len() {
        assert_eq!(
            own.cells.get(i),
            memo.dense.cells.get(i),
            "cell {:?}",
            own.coords(i)
        );
    }
    let raw = raw_membership(&opened, &own);
    for i in 0..raw.len() {
        assert_eq!(
            raw.get(i),
            memo.extra.cells.get(i),
            "cell {:?}",
            own.coords(i)
        );
    }
    // The lineage's diff is exactly the doorway.
    let (_, diff) = memo
        .lineage
        .as_ref()
        .expect("the yard is the opened yard's lineage");
    assert_eq!(diff, &vec![[7, 1, 0], [7, 2, 0]]);
}

/// The node bank as a file holds it (`persist`): every variant the Bell yard
/// and its opened twin banked, written and read back, answers each node of
/// the opened yard with the node flying gives.
#[test]
fn a_bank_read_back_from_its_file_answers_what_flying_gives() {
    let root = [[5, 1, 5]];
    let _ = bell_yard().body_reach(&root);
    let mut b = bell_yard_build();
    b.solid.remove(&[7, 1, 0]);
    b.solid.remove(&[7, 2, 0]);
    let opened = b.world(Ambient::Void, Vec::new());
    let _ = opened.body_reach(&root);
    let params = ReachParams::shipped(opened.unbounded_fall);
    let memo = opened.graph_memo(&params);
    let held = memo.bank.held();
    // Written in two sections and in one, the bank reads back alike; a file
    // cut short anywhere, or carrying a byte past its end, reads as nothing.
    let half = held.len() / 2;
    let sections = vec![held[..half].to_vec(), held[half..].to_vec()];
    let read: Vec<_> = super::persist::round_trip("key", sections.clone())
        .expect("the file decodes")
        .into_iter()
        .flatten()
        .collect();
    assert_eq!(read.len(), held.len());
    for (a, b) in read.iter().zip(&held) {
        assert_eq!(a.0, b.0);
        assert_eq!(a.1.len(), b.1.len());
    }
    let bytes = super::persist::file_bytes("key", sections);
    assert_eq!(super::persist::read_bytes("key", &bytes), Some(held.len()));
    assert_eq!(super::persist::read_bytes("other", &bytes), None);
    for cut in [8, bytes.len() / 3, bytes.len() / 2, bytes.len() - 1] {
        assert_eq!(
            super::persist::read_bytes("key", &bytes[..cut]),
            None,
            "cut at {cut}"
        );
    }
    let mut longer = bytes.clone();
    longer.push(0);
    assert_eq!(super::persist::read_bytes("key", &longer), None);
    let back = bank::Bank::holding(read);
    let flown: Vec<FlownKey> = memo
        .flown
        .entries()
        .into_iter()
        .flat_map(|(c, l)| l.into_iter().map(move |(q, _)| (c, q)))
        .collect();
    assert!(!flown.is_empty());
    let mut answered = 0;
    for key in flown {
        let volumes = |bx| opened.volumes_near(bx);
        let jumps = opened.jumps_from(key.0);
        let Some(e) = back.find(&key, &memo.dense, &memo.extra, &jumps, &volumes) else {
            continue;
        };
        answered += 1;
        let fresh = opened.expand(&memo, &params, key.0, key.1, None);
        assert_eq!(fresh.kind, e.kind, "{key:?}");
        assert_eq!(fresh.edges, e.edges, "{key:?}");
        assert_eq!(fresh.leaves_world, e.leaves_world, "{key:?}");
        assert_eq!(fresh.open_sea, e.open_sea, "{key:?}");
        assert_eq!(fresh.dies, e.dies, "{key:?}");
        assert_eq!(fresh.simulated, e.simulated, "{key:?}");
        assert_eq!(fresh.credited, e.credited, "{key:?}");
        assert_eq!(fresh.read, e.read, "{key:?}");
    }
    // Bound, not vacuous: the bank read back answers nodes.
    assert!(answered > 0, "the read-back bank answered no node");
}

/// **A column whose every block is overhead is a way out of the world** (the
/// void envelope). A slab 41 wide with one column cut out beside the root,
/// and a single block over the cut at the root's head height plus one — the
/// underside of a ledge, a cliff's lip — and nothing under it: a body walks
/// under the block and falls out of the world. The root's own launches say
/// so ([`World::fatal_launch`]); the closure must fly them and record the
/// root as leaving the world, which `DW0322` reads. The envelope used to ask
/// for a void launch only over a column with nothing in it at all, so the
/// overhead block hid the drop and the closure never flew the launch: the
/// gallery's site-plan point passed `DW0322` with a reached cell at its edge
/// whose step south fell out of the world.
#[test]
fn a_column_with_only_an_overhead_block_is_a_way_out_of_the_world() {
    let mut solid: BTreeSet<[i32; 3]> = BTreeSet::new();
    for x in 0..=40 {
        for z in 0..=40 {
            if [x, z] != [21, 20] {
                solid.insert([x, 0, z]);
            }
        }
    }
    solid.insert([21, 3, 20]);
    let w = World::from_solid_cells(solid);
    let root = [20, 1, 20];
    assert_eq!(w.fatal_launch(root), Some(Death::Void));
    let reach = w.body_reach(&[root]);
    assert!(
        reach.leaves_world.contains(&root),
        "the root's launch under the overhead block leaves the world"
    );
    // The perturbation only the cut answers: fill its column's floor back in
    // and the root leaves the world by no launch (the slab's rim, twenty
    // columns off, is another cell's way out).
    let mut closed: BTreeSet<[i32; 3]> = BTreeSet::new();
    for x in 0..=40 {
        for z in 0..=40 {
            closed.insert([x, 0, z]);
        }
    }
    closed.insert([21, 3, 20]);
    let c = World::from_solid_cells(closed);
    assert_eq!(c.fatal_launch(root), None);
    assert!(!c.body_reach(&[root]).leaves_world.contains(&root));
}

/// What a bank of `before` answers for every node `after`'s graph flew,
/// against the node flying gives in `after`: each node the bank answers is
/// the node flying gives (`hits`), each node whose edges `before` and `after`
/// give differently (`changed`) is one it does not answer, and each node
/// `after`'s graph holds — from its lineage (`before`'s graph), the bank or
/// flown — is the node flying gives.
fn bank_against_an_edit(
    before: &World,
    after: &World,
    root: &[[i32; 3]],
) -> (usize, usize, usize, Arc<Reach>) {
    let params = ReachParams::shipped(before.unbounded_fall);
    let _ = before.body_reach(root);
    let mb = before.graph_memo(&params);
    let back = bank::Bank::holding(mb.bank.held());
    let reach = after.body_reach(root);
    let ma = after.graph_memo(&params);
    assert!(
        Arc::ptr_eq(&mb.bank, &ma.bank),
        "the two worlds share one bank: the edit is not the bank's key"
    );
    let held_nodes: BTreeMap<FlownKey, NodeEdges> = ma
        .flown
        .entries()
        .into_iter()
        .flat_map(|(c, v)| v.into_iter().map(move |(q, e)| ((c, q), e)))
        .collect();
    let flown: Vec<FlownKey> = held_nodes.keys().copied().collect();
    let (mut hits, mut changed) = (0, 0);
    for key in &flown {
        let fresh = after.expand(&ma, &params, key.0, key.1, None);
        let was = before.expand(&mb, &params, key.0, key.1, None);
        let volumes = |bx| after.volumes_near(bx);
        let jumps = after.jumps_from(key.0);
        let hit = back.find(key, &ma.dense, &ma.extra, &jumps, &volumes);
        let held = &held_nodes[key];
        assert_eq!(
            fresh.edges, held.edges,
            "{key:?}: the graph holds a stale node"
        );
        assert_eq!(fresh.read, held.read, "{key:?}");
        if was.edges != fresh.edges {
            changed += 1;
            assert!(
                hit.is_none(),
                "{key:?}: the edit moved its edges and the bank answered"
            );
        }
        if let Some(e) = hit {
            hits += 1;
            assert_eq!(fresh.kind, e.kind, "{key:?}");
            assert_eq!(fresh.edges, e.edges, "{key:?}");
            assert_eq!(fresh.leaves_world, e.leaves_world, "{key:?}");
            assert_eq!(fresh.dies, e.dies, "{key:?}");
            assert_eq!(fresh.read, e.read, "{key:?}");
        }
    }
    (flown.len(), hits, changed, reach)
}

/// A walled yard, floored past its walls, with nothing to climb: the way out
/// is the doorway at `(7, 0)` alone.
fn walled_yard(floor: ([i32; 3], [i32; 3])) -> Build {
    let mut b = Build::default();
    b.fill(floor.0, floor.1);
    for i in 0..=14 {
        for (x, z) in [(i, 0), (i, 14), (0, i), (14, i)] {
            b.fill([x, 1, z], [x, 3, z]);
        }
    }
    b.solid.remove(&[7, 1, 0]);
    b.solid.remove(&[7, 2, 0]);
    b
}

/// **A door edit misses exactly the nodes that read the door.** Two worlds
/// that differ only in a door's leaf — the doorway of a walled yard with a
/// shut iron door's leaf in it, and without — key one bank: a leaf is its
/// cell's fact, not the bank's. The leaf stops the walk through the doorway
/// (no snapshot code carries a shut leaf: the cell relation reads it from the
/// leaf alone); every node whose edges it moves misses the bank built over
/// the open doorway, and every node the bank answers is the node flying
/// gives. The lineage (the open yard's graph) re-flies those nodes too.
#[test]
fn a_leaf_edit_misses_the_nodes_that_read_it_and_no_other() {
    let shut = delvewright_dsl::blockshape::leaf(
        "minecraft:iron_door[facing=south,half=lower,hinge=left,open=false]",
    )
    .expect("an iron door's leaf");
    let b = walled_yard(([-3, 0, -3], [17, 0, 17]));
    // A leaf far from the doorway stands in both worlds.
    let mut open = b.world(Ambient::Void, Vec::new());
    open.floors.leaves.insert([16, 1, 16], shut);
    let mut closed = open.clone_world();
    closed.floors.leaves.insert([7, 1, 0], shut);
    closed.floors.leaves.insert([7, 2, 0], shut);
    let root = [[5, 1, 5]];
    let (flown, hits, changed, _) = bank_against_an_edit(&open, &closed, &root);
    eprintln!("leaf edit: {flown} node(s) flown, {hits} answered by the bank, {changed} moved");
    assert!(
        changed > 0,
        "the edit moved no node: the test binds nothing"
    );
    assert!(
        hits > 0,
        "the bank answered no node: it misses where nothing changed"
    );
}

/// **A door edit misses the nodes beside it.** The walled yard, through the
/// real classifier, with an oak door in its doorway that the story has
/// opened, and then with a shut iron door there instead: the way out is
/// gone, every node the edit moves misses the bank the oak door left, and
/// every node the bank answers is the node flying gives.
#[test]
fn a_door_edit_misses_the_nodes_beside_it() {
    let yard = |oak: bool| -> World {
        let b = walled_yard(([-5, 0, -5], [19, 0, 19]));
        let mut cells: Vec<([i32; 3], String)> = b
            .solid
            .iter()
            .map(|c| (*c, "minecraft:stone".to_string()))
            .collect();
        let door = if oak { "oak_door" } else { "iron_door" };
        for (y, half) in [(1, "lower"), (2, "upper")] {
            cells.push((
                [7, y, 0],
                format!("minecraft:{door}[facing=south,half={half},hinge=left,open=false]"),
            ));
        }
        let opened: BTreeSet<[i32; 3]> = if oak {
            [[7, 1, 0], [7, 2, 0]].into_iter().collect()
        } else {
            BTreeSet::new()
        };
        World::from_occupancy(
            crate::compiler::assembled::occupancy_of(cells.into_iter().collect(), &opened),
            Premises::geometry_only(),
        )
    };
    let root = [[5, 1, 5]];
    let (flown, hits, changed, reach) = bank_against_an_edit(&yard(true), &yard(false), &root);
    eprintln!("door edit: {flown} node(s) flown, {hits} answered by the bank, {changed} moved");
    assert!(
        yard(true)
            .body_reach(&root)
            .states
            .contains_key(&[7, 1, -1]),
        "the opened oak door is walked through"
    );
    assert!(
        !reach.states.contains_key(&[7, 1, -1]),
        "the shut iron door stops the way out"
    );
    assert!(
        changed > 0,
        "the edit moved no node: the test binds nothing"
    );
    assert!(
        hits > 0,
        "the bank answered no node: it misses where nothing changed"
    );
}

/// **Two worlds whose tallest blocks differ by a course share a window**,
/// and so one node bank and a lineage: a block set on the walled yard's wall
/// raises its highest cell and moves no other, and the yard with it re-flies
/// only the nodes beside it.
#[test]
fn a_taller_block_keeps_the_window() {
    let root = [[5, 1, 5]];
    let low = walled_yard(([-2, 0, -2], [16, 0, 16]));
    let mut high = walled_yard(([-2, 0, -2], [16, 0, 16]));
    high.fill([3, 4, 14], [3, 4, 14]);
    let (a, b) = (
        low.world(Ambient::Void, Vec::new()),
        high.world(Ambient::Void, Vec::new()),
    );
    let params = ReachParams::shipped(a.unbounded_fall);
    let _ = a.body_reach(&root);
    let _ = b.body_reach(&root);
    let (ma, mb) = (a.graph_memo(&params), b.graph_memo(&params));
    assert_eq!((ma.dense.lo, ma.dense.dims), (mb.dense.lo, mb.dense.dims));
    assert!(Arc::ptr_eq(&ma.bank, &mb.bank), "one bank");
    let (base, diff) = mb.lineage.as_ref().expect("the low yard is the lineage");
    assert!(Arc::ptr_eq(base, &ma));
    assert_eq!(diff, &vec![[3, 4, 14]]);
}

/// **The cost's bounds hold over the graphs built** (ADR-0037 §3): on the open
/// floor, the Bell yard, the jetty (an ocean), the garden and a ladder cliff,
/// at chunk sides of 32 and 16, the closure's states are at most the census's
/// candidates, every chunk's lists decode to at most the census's bound for a
/// chunk, and the portal edges joined are at most its bound — each world's
/// store read by the census as the cost reads it, over the window the cost
/// takes. And what the graph holds in the reach scratch files — its
/// snapshot's pages, its two node tables and its closure's lists, each read
/// from the records the structures hold (`GraphMemo::scratch_held`) — is at
/// most the cost's per-configuration bound for each (`scratch_bound_of`),
/// red on any of the three passing its term. The factor of each bound over
/// the measure is printed.
#[test]
fn the_costs_bounds_hold_over_the_graphs_built() {
    use crate::compiler::blockstate::{BlockMap, BlockState};
    use crate::compiler::cost::RestCensus;
    fn store_of(b: &Build) -> BlockMap {
        let mut m = BlockMap::new();
        for c in &b.solid {
            m.insert(*c, BlockState::new("minecraft:stone"));
        }
        for c in &b.tall {
            m.insert(*c, BlockState::new("minecraft:oak_fence"));
        }
        for c in &b.water {
            m.insert(*c, BlockState::new("minecraft:water"));
        }
        m
    }
    let mut open = Build::default();
    open.fill([0, 0, 0], [11, 0, 11]);
    let mut jetty = Build::default();
    jetty.fill([0, 64, 0], [9, 64, 4]);
    for x in 0..=9 {
        jetty.tall.insert([x, 65, 0]);
        jetty.tall.insert([x, 65, 4]);
    }
    for z in 1..=3 {
        jetty.tall.insert([9, 65, z]);
        jetty.tall.insert([0, 65, z]);
    }
    jetty.fill([4, 65, 1], [4, 65, 1]);
    let mut garden_b = Build::default();
    garden_b.fill([0, 0, 0], [10, 0, 10]);
    for x in 3..=6 {
        for z in 3..=6 {
            if x == 3 || x == 6 || z == 3 || z == 6 {
                garden_b.tall.insert([x, 1, z]);
            }
        }
    }
    garden_b.fill([2, 1, 4], [2, 1, 4]);
    garden_b.fill([5, 1, 5], [5, 1, 5]);
    let mut cliff: Vec<([i32; 3], &str)> = Vec::new();
    for x in 0..=6 {
        for z in 0..=2 {
            cliff.push(([x, 64, z], "minecraft:stone"));
            if x >= 3 {
                for y in 65..=68 {
                    cliff.push(([x, y, z], "minecraft:stone"));
                }
            }
        }
    }
    for y in 65..=68 {
        cliff.push(([2, y, 1], "minecraft:ladder[facing=west]"));
    }
    let mut cliff_store = BlockMap::new();
    for (c, n) in &cliff {
        cliff_store.insert(*c, BlockState::new(n));
    }
    let sea = Sea {
        level: 62,
        floor_top: 54,
    };
    let band = (sea.floor_top, sea.level);
    // Name, world, its store, its sea band, the root.
    type Case = (&'static str, World, BlockMap, Option<(i32, i32)>, [i32; 3]);
    let worlds: Vec<Case> = vec![
        (
            "the open floor",
            open.world(Ambient::Void, Vec::new()),
            store_of(&open),
            None,
            [5, 1, 5],
        ),
        (
            "the Bell yard",
            bell_yard(),
            store_of(&bell_yard_build()),
            None,
            [5, 1, 5],
        ),
        (
            "the jetty",
            jetty.world(
                Ambient::Ocean(sea),
                vec![("prefab/jetty".to_string(), ([0, 64, 0], [9, 66, 4]))],
            ),
            store_of(&jetty),
            Some(band),
            [2, 65, 2],
        ),
        ("the garden", garden(), store_of(&garden_b), None, [1, 1, 1]),
        (
            "the ladder cliff",
            crate::compiler::nav::testkit::blocks_world(&cliff),
            cliff_store,
            None,
            [0, 65, 1],
        ),
    ];
    let edges = edges_per_node_at_most();
    let (mut bound_states, mut bound_lists, mut bound_portals) = (0usize, 0usize, 0u64);
    let mut bound_scratch = ScratchBound::default();
    for (name, w, store, sea, root) in &worlds {
        let (slo, shi) = store.section_bounds().expect("a store with cells");
        let window = window_box(slo, shi, *sea);
        for side in [32, 16] {
            let census = RestCensus::of(store, window, *sea, side, LAUNCH_REACH, edges);
            chunks::at_side(side, || {
                let r = w.body_reach(&[*root]);
                let states = r.states.len();
                // The scratch the graph holds, against the bound for one
                // configuration (one world, one closure).
                let memo = w.graph_memo(&ReachParams::shipped(w.unbounded_fall));
                // The lists are this closure's own: another test's closure
                // over the same world is kept by the same memo.
                let held = ScratchBound {
                    lists: r.spilled_bytes() as usize,
                    ..memo.scratch_held()
                };
                let cells = [0, 1, 2]
                    .map(|a| (window.1[a] - window.0[a] + 1) as usize)
                    .iter()
                    .product::<usize>();
                // The cost's window is the store's sections with the margins,
                // the graph's the modelled cells with them: the first holds
                // the second, which is what makes the pages term a bound.
                let graph_cells = memo
                    .dense
                    .dims
                    .iter()
                    .map(|d| *d as usize)
                    .product::<usize>();
                assert!(
                    graph_cells <= cells,
                    "{name} (side {side}): the graph's window ({graph_cells} cells) is not inside \
                     the cost's ({cells})"
                );
                let bound = scratch_bound_of(ScratchInputs {
                    states: census.states_at_most(),
                    items: census.items_at_most,
                    chunk_items: census.chunk_items_at_most,
                    cells,
                    height: (window.1[1] - window.0[1] + 1) as usize,
                    launches: launches_per_node(),
                    jumps: w.declared_jumps.len(),
                    volumes: w.lethal_regions.len(),
                });
                for (what, got, at_most) in [
                    ("the snapshot's pages", held.snapshot, bound.snapshot),
                    ("the node tables", held.tables, bound.tables),
                    ("the closure's lists", held.lists, bound.lists),
                    ("the bank's variants", held.bank, bound.bank),
                ] {
                    assert!(
                        got <= at_most,
                        "{name} (side {side}): {what} hold {got} byte(s) in the scratch file over \
                         the cost's {at_most}"
                    );
                }
                eprintln!(
                    "{name} (side {side}): scratch snapshot {} <= {} (x{:.1}); tables {} <= {} \
                     (x{:.1}); lists {} <= {} (x{:.1}); bank {} <= {} (x{:.1})",
                    held.snapshot,
                    bound.snapshot,
                    bound.snapshot as f64 / held.snapshot.max(1) as f64,
                    held.tables,
                    bound.tables,
                    bound.tables as f64 / held.tables.max(1) as f64,
                    held.lists,
                    bound.lists,
                    bound.lists as f64 / held.lists.max(1) as f64,
                    held.bank,
                    bound.bank,
                    bound.bank as f64 / held.bank.max(1) as f64,
                );
                bound_scratch.snapshot += held.snapshot;
                bound_scratch.tables += held.tables;
                bound_scratch.lists += held.lists;
                bound_scratch.bank += held.bank;
                assert!(
                    states <= census.states_at_most(),
                    "{name} (side {side}): {states} states over the census's {}",
                    census.states_at_most()
                );
                let key = |c: [i32; 3]| (c[0].div_euclid(side), c[2].div_euclid(side));
                let mut per_chunk: BTreeMap<(i32, i32), (usize, usize)> = BTreeMap::new();
                // What the lists hold: a plain-sea state's dives are computed
                // and never listed (ADR-0037 §2).
                for c in r.states.keys().filter(|c| !r.states.plain(c)) {
                    let e = per_chunk.entry(key(c)).or_insert((0, 0));
                    e.0 += 1;
                    e.1 += r.succ(c).len();
                }
                let largest = per_chunk
                    .values()
                    .map(|(n, i)| lists_bound_of(*n, *i))
                    .max()
                    .unwrap_or(0);
                assert!(
                    largest <= census.lists_chunk_bound,
                    "{name} (side {side}): a chunk decodes to {largest} over the census's {}",
                    census.lists_chunk_bound
                );
                assert!(
                    r.portals[0] <= census.portal_edges_at_most as u64,
                    "{name} (side {side}): {} portal edge(s) over the census's {}",
                    r.portals[0],
                    census.portal_edges_at_most
                );
                eprintln!(
                    "{name} (side {side}): states {states} <= {} (x{:.1}); largest chunk {largest} <= \
                     {} (x{:.1}); portals {} <= {} (x{:.1})",
                    census.states_at_most(),
                    census.states_at_most() as f64 / states.max(1) as f64,
                    census.lists_chunk_bound,
                    census.lists_chunk_bound as f64 / largest.max(1) as f64,
                    r.portals[0],
                    census.portal_edges_at_most,
                    census.portal_edges_at_most as f64 / r.portals[0].max(1) as f64
                );
                bound_states += states;
                bound_lists += largest;
                bound_portals += r.portals[0];
            });
        }
    }
    assert!(
        bound_states > 0 && bound_lists > 0 && bound_portals > 0,
        "the worlds bind nothing"
    );
    assert!(
        bound_scratch.snapshot > 0
            && bound_scratch.tables > 0
            && bound_scratch.lists > 0
            && bound_scratch.bank > 0,
        "the graphs hold nothing in the scratch files: the scratch bound binds nothing \
         ({bound_scratch:?})"
    );
}

/// **What the reach scratch bound does not count by name is covered by its
/// terms' slack** (ADR-0037 §3, the cost line's declaration): a second world
/// of the same window — the walled yard with a block set on its wall, a
/// premise variant in substance, with its own pages, tables and bank
/// variants — and a second closure over the base world from other roots,
/// both built, and what the two graphs hold in the files together (every
/// closure of both, every table entry, the one bank) is at most the bound for
/// one configuration computed over the base store. Red if a second world or
/// a second closure passes one configuration's bound; the case refuses a run
/// where the second world built no graph of its own.
#[test]
fn a_second_world_and_a_second_closure_stay_under_one_configurations_bound() {
    use crate::compiler::blockstate::{BlockMap, BlockState};
    use crate::compiler::cost::RestCensus;
    let b = walled_yard(([-2, 0, -2], [16, 0, 16]));
    let base = b.world(Ambient::Void, Vec::new());
    let mut edited = walled_yard(([-2, 0, -2], [16, 0, 16]));
    edited.fill([3, 4, 14], [3, 4, 14]);
    let edited = edited.world(Ambient::Void, Vec::new());
    let mut store = BlockMap::new();
    for (cells, name) in [
        (&b.solid, "minecraft:stone"),
        (&b.tall, "minecraft:oak_fence"),
        (&b.water, "minecraft:water"),
    ] {
        for c in cells {
            store.insert(*c, BlockState::new(name));
        }
    }
    let (slo, shi) = store.section_bounds().expect("a store with cells");
    let window = window_box(slo, shi, None);
    let census = RestCensus::of(
        &store,
        window,
        None,
        32,
        LAUNCH_REACH,
        edges_per_node_at_most(),
    );
    let cells = [0, 1, 2]
        .map(|a| (window.1[a] - window.0[a] + 1) as usize)
        .iter()
        .product::<usize>();
    let bound = scratch_bound_of(ScratchInputs {
        states: census.states_at_most(),
        items: census.items_at_most,
        chunk_items: census.chunk_items_at_most,
        cells,
        height: (window.1[1] - window.0[1] + 1) as usize,
        launches: launches_per_node(),
        jumps: 0,
        volumes: 0,
    });
    let params = ReachParams::shipped(base.unbounded_fall);
    let _ = base.body_reach(&[[5, 1, 5]]);
    // A root the first closure does not reach: the wall's top, three courses
    // over the floor, which no jump from it climbs.
    let _ = base.body_reach(&[[0, 4, 0]]);
    let _ = edited.body_reach(&[[5, 1, 5]]);
    let (ma, me) = (base.graph_memo(&params), edited.graph_memo(&params));
    assert!(
        !Arc::ptr_eq(&ma, &me),
        "the edited yard is a graph of its own"
    );
    assert!(Arc::ptr_eq(&ma.bank, &me.bank), "one bank over one window");
    let (a, e) = (ma.scratch_held(), me.scratch_held());
    // A page both hold is held once; the bank is one, counted once.
    let pages = |m: &GraphMemo| -> BTreeSet<(bool, u32)> {
        m.dense
            .cells
            .pages
            .iter()
            .map(|p| (true, *p))
            .chain(m.extra.cells.pages.iter().map(|p| (false, *p)))
            .collect()
    };
    let shared: usize = pages(&ma)
        .intersection(&pages(&me))
        .map(|(code, _)| if *code { 4096 * 4 } else { 4096 * 2 })
        .sum();
    let held = ScratchBound {
        snapshot: a.snapshot + e.snapshot - shared,
        tables: a.tables + e.tables,
        lists: a.lists + e.lists,
        bank: a.bank,
    };
    let closures = ma.closures.lock().expect("closures").len();
    assert!(
        closures >= 2 && e.tables > 0 && e.lists > 0,
        "the case binds nothing: {closures} closure(s) over the base, the edited yard's tables \
         {} and lists {}",
        e.tables,
        e.lists
    );
    for (what, got, at_most) in [
        ("the snapshot's pages", held.snapshot, bound.snapshot),
        ("the node tables", held.tables, bound.tables),
        ("the closures' lists", held.lists, bound.lists),
        ("the bank's variants", held.bank, bound.bank),
    ] {
        assert!(
            got <= at_most,
            "two worlds and two closures: {what} hold {got} byte(s) over one configuration's \
             {at_most}"
        );
        eprintln!(
            "two worlds and two closures: {what} {got} <= {at_most} (x{:.1})",
            at_most as f64 / got.max(1) as f64
        );
    }
}

/// **What the bank records of a node is the same in every schedule**
/// (`bank`'s module docs): a node's variant — the cells its head and its runs
/// read, with what each held and which runs read it, the runs' boxes and
/// what they did — is a function of the world and the node, whichever
/// variant the expansion took runs from, so a build's bank is the same at
/// every thread count. Generated cases: the Bell yard and edits of it that
/// keep its window (wall cells opened, stones set in the yard), each world's
/// nodes expanded with a forced prior in three orders — the worlds in turn,
/// in reverse, and each alone against an empty bank — and every variant
/// compared byte for byte; the two shared banks hold the same variants in the
/// same order. Bound: runs taken as written in both shared orders, and nodes
/// whose variant is compared after one was taken.
#[test]
fn a_node_records_the_same_variant_whichever_graph_banked_first() {
    let root = [[5, 1, 5]];
    let mut worlds = vec![bell_yard()];
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = |n: u64| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n) as i32
    };
    for _ in 0..5 {
        let mut b = bell_yard_build();
        for _ in 0..2 {
            // A wall cell opened two high, on a side the seed picks.
            let i = 1 + next(13);
            let (x, z) = match next(4) {
                0 => (i, 0),
                1 => (i, 14),
                2 => (0, i),
                _ => (14, i),
            };
            b.solid.remove(&[x, 1, z]);
            b.solid.remove(&[x, 2, z]);
            // A stone set in the yard, clear of the root.
            let s = [1 + next(13), 1, 1 + next(13)];
            if s != [5, 1, 5] {
                b.solid.insert(s);
            }
        }
        worlds.push(b.world(Ambient::Void, Vec::new()));
    }
    let params = ReachParams::shipped(worlds[0].unbounded_fall);
    let memos: Vec<_> = worlds
        .iter()
        .map(|w| {
            let _ = w.body_reach(&root);
            w.graph_memo(&params)
        })
        .collect();
    let keys: Vec<Vec<FlownKey>> = memos
        .iter()
        .map(|m| {
            let mut k: Vec<FlownKey> = m
                .flown
                .entries()
                .into_iter()
                .flat_map(|(c, l)| l.into_iter().map(move |(q, _)| (c, q)))
                .collect();
            k.sort();
            k
        })
        .collect();
    type Recorded = BTreeMap<(usize, FlownKey), Vec<u8>>;
    // Every node of the worlds in `order`, each against `bank` (a fresh bank
    // per world where `bank` is `None`): expanded, taking runs from the
    // nearest variant held, and kept.
    let record = |order: &[usize], shared: Option<&bank::Bank>| -> Recorded {
        let mut out = Recorded::new();
        for &wi in order {
            let alone;
            let b = match shared {
                Some(b) => b,
                None => {
                    alone = bank::Bank::holding(Vec::new());
                    &alone
                }
            };
            let (w, m) = (&worlds[wi], &memos[wi]);
            for key in &keys[wi] {
                let volumes = |bx| w.volumes_near(bx);
                let jumps = w.jumps_from(key.0);
                let x = b.expansion(
                    key,
                    &m.dense,
                    &m.extra,
                    &w.node_boxes(key.0),
                    &jumps,
                    &volumes,
                );
                let e = Arc::new(w.expand(m, &params, key.0, key.1, Some(&x)));
                let Some(rb) = e.read else { continue };
                let v = b.keep(*key, x, &m.extra, jumps, w.volumes_near(rb), &e);
                out.insert((wi, *key), super::persist::encode_variant(&v));
            }
        }
        out
    };
    let forward: Vec<usize> = (0..worlds.len()).collect();
    let reverse: Vec<usize> = forward.iter().rev().copied().collect();
    let (fb, rb) = (
        bank::Bank::holding(Vec::new()),
        bank::Bank::holding(Vec::new()),
    );
    let alone = record(&forward, None);
    let fwd = record(&forward, Some(&fb));
    let rev = record(&reverse, Some(&rb));
    let compared = alone.len();
    assert_eq!(fwd.len(), compared);
    assert_eq!(rev.len(), compared);
    let mut moved = Vec::new();
    for (k, a) in &alone {
        for (name, other) in [("in turn", &fwd), ("in reverse", &rev)] {
            if other[k] != *a {
                moved.push(format!(
                    "{name}: world {} node {:?}: {} byte(s) against {} alone",
                    k.0,
                    k.1,
                    other[k].len(),
                    a.len()
                ));
            }
        }
    }
    assert!(
        moved.is_empty(),
        "{} of 2 x {compared} variant(s) moved with the order the graphs banked in: {:?}",
        moved.len(),
        &moved[..moved.len().min(8)]
    );
    let held = |b: &bank::Bank| -> Vec<(FlownKey, Vec<Vec<u8>>)> {
        b.held()
            .into_iter()
            .map(|(k, vs)| {
                (
                    k,
                    vs.iter()
                        .map(|v| super::persist::encode_variant(v))
                        .collect(),
                )
            })
            .collect()
    };
    assert_eq!(held(&fb), held(&rb), "the two orders' banks differ");
    assert_eq!(fb.census(), rb.census());
    let distinct: BTreeSet<(&FlownKey, &Vec<u8>)> =
        alone.iter().map(|((_, k), v)| (k, v)).collect();
    eprintln!(
        "bank schedules: {} world(s), {compared} variant(s) compared in three orders, {} distinct; \
         runs taken as written {} in turn and {} in reverse; banks of {:?} (variants, bytes)",
        worlds.len(),
        distinct.len(),
        fb.reused_runs(),
        rb.reused_runs(),
        fb.census()
    );
    // Bound, not vacuous: both shared orders took runs from a prior.
    assert!(compared > 0);
    assert!(fb.reused_runs() > 0 && rb.reused_runs() > 0);
}
